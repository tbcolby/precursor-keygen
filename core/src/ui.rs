//! Screen state machine. Pure: the app crate maps device keys to [`Input`],
//! performs the returned [`Effect`] (storage, leaving), reports the outcome
//! back, and draws from the public state.

use zeroize::Zeroize;

use crate::entropy::Entropy;
use crate::gen::{entropy_tenths, generate, Kind};
use crate::record::{new_key, Record, MAX_LABEL, RECORD_VERSION};
use crate::secret::Secret;

/// Secrets on screen are hidden after this many seconds without a key press.
pub const REVEAL_SECS: u32 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Backspace,
    F(u8),
    Char(char),
}

impl Input {
    fn is_back(self) -> bool { matches!(self, Input::Backspace | Input::F(4)) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Types,
    Configure,
    Result,
    Label,
    Saved,
    View,
    ConfirmDelete,
}

/// Work for the app crate.
#[derive(Debug)]
pub enum Effect {
    None,
    /// Hand focus back to GAM.
    Leave,
    /// Write this record under this PDDB key, then call `on_saved` / `on_save_failed`.
    Save(String, Record),
    /// Delete this PDDB key, then call `on_deleted` / `on_delete_failed`.
    Delete(String),
}

pub struct Saved {
    pub key: String,
    pub record: Record,
}

pub struct Ui {
    pub screen: Screen,
    pub type_cursor: usize,
    pub kind: Kind,
    pub length: usize,
    /// The value on the Result screen.
    pub current: Option<Secret>,
    pub current_saved: bool,
    pub label: String,
    pub saved: Vec<Saved>,
    pub saved_cursor: usize,
    /// Records present in storage that couldn't be read (never rewritten).
    pub unreadable: usize,
    /// One-line message for the user (errors, "Saved").
    pub status: String,
    reveal_secs: u32,
}

impl Default for Ui {
    fn default() -> Self { Self::new() }
}

impl Ui {
    pub fn new() -> Self {
        Ui {
            screen: Screen::Types,
            type_cursor: 0,
            kind: Kind::Password,
            length: Kind::Password.default_length(),
            current: None,
            current_saved: false,
            label: String::new(),
            saved: Vec::new(),
            saved_cursor: 0,
            unreadable: 0,
            status: String::new(),
            reveal_secs: 0,
        }
    }

    /// Replace the saved list (startup and after a basis change). Anything on
    /// screen that came from the old list is dropped.
    pub fn set_saved(&mut self, mut saved: Vec<Saved>, unreadable: usize) {
        saved.sort_by_key(|s| s.record.seq);
        self.saved = saved;
        self.unreadable = unreadable;
        self.saved_cursor = self.saved_cursor.min(self.saved.len().saturating_sub(1));
        if matches!(self.screen, Screen::View | Screen::ConfirmDelete) {
            self.screen = Screen::Saved;
        }
    }

    pub fn entropy_tenths(&self) -> u32 { entropy_tenths(self.kind, self.length) }

    /// True while a secret is on screen (the app runs a 1 s tick for the timeout).
    pub fn showing_secret(&self) -> bool {
        matches!(self.screen, Screen::Result | Screen::Label | Screen::View | Screen::ConfirmDelete)
    }

    /// One second passed while foregrounded.
    pub fn on_tick(&mut self) -> bool {
        if !self.showing_secret() {
            self.reveal_secs = 0;
            return false;
        }
        self.reveal_secs += 1;
        if self.reveal_secs >= REVEAL_SECS {
            self.hide_secrets();
            self.status = "Hidden after 2 minutes".into();
            return true;
        }
        false
    }

    pub fn seconds_left(&self) -> u32 { REVEAL_SECS.saturating_sub(self.reveal_secs) }

    /// Another app took focus: take secrets off the screen.
    pub fn on_background(&mut self) {
        if self.showing_secret() {
            self.hide_secrets();
        }
    }

    fn hide_secrets(&mut self) {
        self.current = None;
        self.label.zeroize();
        self.reveal_secs = 0;
        self.screen = match self.screen {
            Screen::View | Screen::ConfirmDelete => Screen::Saved,
            Screen::Result | Screen::Label => Screen::Configure,
            s => s,
        };
    }

    pub fn on_key<E: Entropy + ?Sized>(&mut self, key: Input, rng: &mut E) -> Effect {
        self.reveal_secs = 0;
        self.status.clear();
        match self.screen {
            Screen::Types => self.key_types(key),
            Screen::Configure => self.key_configure(key, rng),
            Screen::Result => self.key_result(key, rng),
            Screen::Label => self.key_label(key, rng),
            Screen::Saved => self.key_saved(key),
            Screen::View => self.key_view(key),
            Screen::ConfirmDelete => self.key_confirm(key),
        }
    }

    fn key_types(&mut self, key: Input) -> Effect {
        let n = Kind::ALL.len();
        match key {
            Input::Up => self.type_cursor = self.type_cursor.saturating_sub(1),
            Input::Down => self.type_cursor = (self.type_cursor + 1).min(n - 1),
            Input::Enter => {
                self.kind = Kind::ALL[self.type_cursor];
                self.length = self.kind.default_length();
                self.screen = Screen::Configure;
            }
            Input::Char('s') | Input::Char('S') | Input::F(2) => self.screen = Screen::Saved,
            k if k.is_back() => return Effect::Leave,
            _ => {}
        }
        Effect::None
    }

    fn generate_now<E: Entropy + ?Sized>(&mut self, rng: &mut E) {
        self.current = None; // drop (and wipe) the previous value first
        match generate(rng, self.kind, self.length) {
            Ok(v) => {
                self.current = Some(v);
                self.current_saved = false;
                self.screen = Screen::Result;
            }
            Err(_) => {
                self.screen = Screen::Configure;
                self.status = "TRNG error: nothing was generated".into();
            }
        }
    }

    fn key_configure<E: Entropy + ?Sized>(&mut self, key: Input, rng: &mut E) -> Effect {
        match key {
            Input::Left | Input::Down => self.length = self.kind.adjust(self.length, false),
            Input::Right | Input::Up => self.length = self.kind.adjust(self.length, true),
            Input::Enter | Input::Char(' ') => self.generate_now(rng),
            k if k.is_back() => self.screen = Screen::Types,
            _ => {}
        }
        Effect::None
    }

    fn key_result<E: Entropy + ?Sized>(&mut self, key: Input, rng: &mut E) -> Effect {
        match key {
            Input::Char('r') | Input::Char('R') | Input::Char(' ') => self.generate_now(rng),
            Input::Char('s') | Input::Char('S') | Input::F(2) => {
                if self.current_saved {
                    self.status = "Already saved".into();
                } else {
                    self.label.zeroize();
                    self.screen = Screen::Label;
                }
            }
            k if k.is_back() => {
                self.current = None;
                self.screen = Screen::Configure;
            }
            _ => {}
        }
        Effect::None
    }

    fn key_label<E: Entropy + ?Sized>(&mut self, key: Input, rng: &mut E) -> Effect {
        match key {
            Input::Char(c) if (' '..='~').contains(&c) => {
                if self.label.len() < MAX_LABEL {
                    self.label.push(c);
                }
            }
            Input::Backspace if !self.label.is_empty() => {
                self.label.pop();
            }
            k if k.is_back() => self.screen = Screen::Result,
            Input::Enter => {
                let Some(value) = self.current.as_ref().map(Secret::duplicate) else {
                    self.screen = Screen::Configure;
                    return Effect::None;
                };
                let key = match new_key(rng) {
                    Ok(k) => k,
                    Err(_) => {
                        self.status = "TRNG error: not saved".into();
                        self.screen = Screen::Result;
                        return Effect::None;
                    }
                };
                let seq = self.saved.iter().map(|s| s.record.seq).max().map_or(1, |m| m + 1);
                let record = Record {
                    version: RECORD_VERSION,
                    seq,
                    label: self.label.trim().to_string(),
                    kind: self.kind,
                    length: self.length,
                    bits_tenths: self.entropy_tenths(),
                    value,
                };
                self.label.zeroize();
                self.screen = Screen::Result;
                return Effect::Save(key, record);
            }
            _ => {}
        }
        Effect::None
    }

    /// The app wrote the record. `basis` is where it landed.
    pub fn on_saved(&mut self, key: String, record: Record, basis: Option<&str>) {
        self.saved.push(Saved { key, record });
        self.current_saved = true;
        self.status = match basis {
            Some(b) => format!("Saved to basis {b}"),
            None => "Saved".into(),
        };
    }

    pub fn on_save_failed(&mut self, why: &str) { self.status = format!("Save failed: {why}"); }

    fn key_saved(&mut self, key: Input) -> Effect {
        let n = self.saved.len();
        match key {
            Input::Up => self.saved_cursor = self.saved_cursor.saturating_sub(1),
            Input::Down => self.saved_cursor = (self.saved_cursor + 1).min(n.saturating_sub(1)),
            Input::Enter if n > 0 => self.screen = Screen::View,
            Input::Char('d') | Input::Char('D') | Input::F(3) if n > 0 => self.screen = Screen::ConfirmDelete,
            k if k.is_back() => self.screen = Screen::Types,
            _ => {}
        }
        Effect::None
    }

    fn key_view(&mut self, key: Input) -> Effect {
        match key {
            Input::Char('d') | Input::Char('D') | Input::F(3) => self.screen = Screen::ConfirmDelete,
            k if k.is_back() || k == Input::Enter => self.screen = Screen::Saved,
            _ => {}
        }
        Effect::None
    }

    fn key_confirm(&mut self, key: Input) -> Effect {
        match key {
            Input::Char('y') | Input::Char('Y') => match self.saved.get(self.saved_cursor) {
                Some(s) => {
                    self.screen = Screen::Saved;
                    Effect::Delete(s.key.clone())
                }
                None => {
                    self.screen = Screen::Saved;
                    Effect::None
                }
            },
            _ => {
                self.screen = Screen::Saved;
                Effect::None
            }
        }
    }

    /// The app deleted the PDDB key.
    pub fn on_deleted(&mut self, key: &str) {
        self.saved.retain(|s| s.key != key);
        self.saved_cursor = self.saved_cursor.min(self.saved.len().saturating_sub(1));
        self.screen = Screen::Saved;
        self.status = "Deleted".into();
    }

    pub fn on_delete_failed(&mut self, why: &str) { self.status = format!("Delete failed: {why}"); }

    /// The selected saved record (View / ConfirmDelete).
    pub fn selected(&self) -> Option<&Saved> { self.saved.get(self.saved_cursor) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entropy::testing::*;

    fn rng() -> XorShift { XorShift(1234) }

    fn press(ui: &mut Ui, keys: &[Input]) -> Effect {
        let mut r = rng();
        let mut last = Effect::None;
        for &k in keys {
            last = ui.on_key(k, &mut r);
        }
        last
    }

    fn to_result(ui: &mut Ui) {
        press(ui, &[Input::Enter, Input::Enter]);
        assert_eq!(ui.screen, Screen::Result);
    }

    #[test]
    fn every_screen_exits_with_backspace_and_f4() {
        // Regression: Configure/Result/Saved could only be left with ∴, which
        // GAM never delivers.
        for back in [Input::Backspace, Input::F(4)] {
            let mut ui = Ui::new();
            to_result(&mut ui);
            press(&mut ui, &[Input::Char('s')]);
            assert_eq!(ui.screen, Screen::Label);
            press(&mut ui, &[back]);
            assert_eq!(ui.screen, Screen::Result);
            press(&mut ui, &[back]);
            assert_eq!(ui.screen, Screen::Configure);
            press(&mut ui, &[back]);
            assert_eq!(ui.screen, Screen::Types);
            assert!(matches!(press(&mut ui, &[back]), Effect::Leave));

            let mut ui = Ui::new();
            press(&mut ui, &[Input::Char('s')]);
            assert_eq!(ui.screen, Screen::Saved);
            press(&mut ui, &[back]);
            assert_eq!(ui.screen, Screen::Types);
        }
    }

    #[test]
    fn arrows_move_and_adjust() {
        // Regression: PUA arrow codes meant only "Password" was reachable and
        // the length was fixed.
        let mut ui = Ui::new();
        press(&mut ui, &[Input::Down, Input::Down, Input::Enter]);
        assert_eq!(ui.kind, Kind::Passphrase);
        press(&mut ui, &[Input::Right, Input::Right]);
        assert_eq!(ui.length, 8);
        press(&mut ui, &[Input::Left]);
        assert_eq!(ui.length, 7);
    }

    #[test]
    fn broken_trng_shows_error_and_no_value() {
        let mut ui = Ui::new();
        press(&mut ui, &[Input::Enter]);
        ui.on_key(Input::Enter, &mut Broken);
        assert_eq!(ui.screen, Screen::Configure);
        assert!(ui.current.is_none());
        assert!(ui.status.contains("TRNG"));
    }

    #[test]
    fn save_flow_with_label_and_dedupe() {
        // Regression: S appended a new copy on every press with no feedback.
        let mut ui = Ui::new();
        to_result(&mut ui);
        let value = ui.current.as_ref().unwrap().expose().to_string();
        let eff = press(&mut ui, &[Input::Char('s'), Input::Char('w'), Input::Char('i'), Input::Enter]);
        let Effect::Save(key, rec) = eff else { panic!("expected save") };
        assert_eq!(rec.label, "wi");
        assert_eq!(rec.value.expose(), value);
        assert_eq!(rec.bits_tenths, ui.entropy_tenths());
        ui.on_saved(key, rec, Some("secret"));
        assert!(ui.status.contains("secret"));
        assert!(matches!(press(&mut ui, &[Input::Char('s')]), Effect::None));
        assert_eq!(ui.status, "Already saved");
        assert_eq!(ui.saved.len(), 1);
        // Regenerating allows saving the new value.
        press(&mut ui, &[Input::Char('r'), Input::Char('s')]);
        assert_eq!(ui.screen, Screen::Label);
    }

    #[test]
    fn saved_value_is_viewable_in_full() {
        // Regression: the list showed value[..20] and there was no detail view.
        let mut ui = Ui::new();
        press(&mut ui, &[Input::Down, Input::Down, Input::Down, Input::Down, Input::Enter, Input::Enter]);
        assert_eq!(ui.kind, Kind::HexKey);
        let Effect::Save(k, r) = press(&mut ui, &[Input::Char('s'), Input::Enter]) else { panic!() };
        ui.on_saved(k, r, None);
        press(&mut ui, &[Input::F(4), Input::F(4), Input::F(4), Input::Char('s'), Input::Enter]);
        assert_eq!(ui.screen, Screen::View);
        assert_eq!(ui.selected().unwrap().record.value.len(), 64);
    }

    #[test]
    fn delete_needs_confirmation_and_names_the_key() {
        let mut ui = Ui::new();
        to_result(&mut ui);
        let Effect::Save(k, r) = press(&mut ui, &[Input::Char('s'), Input::Enter]) else { panic!() };
        ui.on_saved(k.clone(), r, None);
        press(&mut ui, &[Input::F(4), Input::F(4), Input::F(4), Input::Char('s'), Input::Char('d')]);
        assert_eq!(ui.screen, Screen::ConfirmDelete);
        // Anything but y cancels.
        press(&mut ui, &[Input::Char('n')]);
        assert_eq!(ui.screen, Screen::Saved);
        let Effect::Delete(dk) = press(&mut ui, &[Input::Char('d'), Input::Char('y')]) else { panic!() };
        assert_eq!(dk, k);
        ui.on_deleted(&dk);
        assert!(ui.saved.is_empty());
    }

    #[test]
    fn secrets_hidden_on_background_and_timeout() {
        let mut ui = Ui::new();
        to_result(&mut ui);
        ui.on_background();
        assert!(ui.current.is_none());
        assert_eq!(ui.screen, Screen::Configure);

        let mut ui = Ui::new();
        press(&mut ui, &[Input::Enter]);
        press(&mut ui, &[Input::Enter]);
        for _ in 0..REVEAL_SECS - 1 {
            assert!(!ui.on_tick());
        }
        assert!(ui.on_tick());
        assert!(ui.current.is_none());
        assert!(!ui.showing_secret());
    }

    #[test]
    fn key_press_resets_timeout() {
        let mut ui = Ui::new();
        press(&mut ui, &[Input::Enter, Input::Enter]);
        for _ in 0..REVEAL_SECS - 1 {
            ui.on_tick();
        }
        press(&mut ui, &[Input::Up]);
        assert!(!ui.on_tick());
        assert!(ui.current.is_some());
    }

    #[test]
    fn reload_drops_view_of_old_basis() {
        let mut ui = Ui::new();
        to_result(&mut ui);
        let Effect::Save(k, r) = press(&mut ui, &[Input::Char('s'), Input::Enter]) else { panic!() };
        ui.on_saved(k, r, None);
        press(&mut ui, &[Input::F(4), Input::F(4), Input::F(4), Input::Char('s'), Input::Enter]);
        assert_eq!(ui.screen, Screen::View);
        ui.set_saved(Vec::new(), 2);
        assert_eq!(ui.screen, Screen::Saved);
        assert_eq!(ui.unreadable, 2);
        assert!(ui.selected().is_none());
    }
}
