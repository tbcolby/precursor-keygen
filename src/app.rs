//! Drawing, storage and TRNG plumbing for Key Ceremony. All decisions are
//! made by `keygen_core::Ui`; this file maps keys to it, carries out its
//! effects, and draws its state.

use gam::menu::Point;
use gam::GlyphStyle;
use keygen_core::gen::{format_bits, grouped, strength};
use keygen_core::record::is_record_key;
use keygen_core::{Effect, Entropy, EntropyError, Input, Kind, Record, Saved, Screen, Ui};
use precursor_appkit::keys::Key;
use precursor_appkit::layout::ListState;
use precursor_appkit::rng::Trng;
use precursor_appkit::screen::{Screen as Canvas, MARGIN};
use precursor_appkit::store::Store;
use zeroize::Zeroizing;

/// PDDB dictionary. One key per saved value (`k.<16 hex>`).
pub const DICT: &str = "keygen.vault";

#[derive(PartialEq, Eq)]
pub enum Action {
    Stay,
    Leave,
}

/// The hardware TRNG as a fallible core entropy source. An IPC error becomes
/// `EntropyError` and the core discards the whole value.
struct TrngSource<'a>(&'a Trng);

impl Entropy for TrngSource<'_> {
    fn try_u32(&mut self) -> Result<u32, EntropyError> { self.0.try_u32().map_err(|_| EntropyError) }
}

pub struct App {
    canvas: Canvas,
    store: Store,
    trng: Trng,
    health_conn: trng::Trng,
    health: String,
    ui: Ui,
    types_list: ListState,
    saved_list: ListState,
    visible_rows: usize,
}

fn to_input(key: Key) -> Input {
    match key {
        Key::Up => Input::Up,
        Key::Down => Input::Down,
        Key::Left => Input::Left,
        Key::Right => Input::Right,
        Key::Enter => Input::Enter,
        Key::Backspace => Input::Backspace,
        Key::F(n) => Input::F(n),
        Key::Char(c) => Input::Char(c),
    }
}

/// Summarise the TRNG server's error counters for the configure screen.
fn trng_health(t: &trng::Trng) -> String {
    match t.get_error_stats() {
        Ok(e) => {
            let n = e.excursion_errs.iter().filter(|x| x.is_some()).count() as u32
                + [e.av_repcount_errs, e.av_adaptive_errs, e.ro_repcount_errs, e.ro_adaptive_errs]
                    .iter()
                    .map(|x| x.unwrap_or(0) as u32)
                    .sum::<u32>()
                + e.nist_errs;
            if n == 0 {
                "TRNG health: no errors logged".into()
            } else {
                format!("TRNG health: {n} errors logged since boot")
            }
        }
        Err(_) => "TRNG health: unavailable".into(),
    }
}

impl App {
    pub fn new(xns: &xous_names::XousNames, canvas: Canvas, store: Store) -> Self {
        let trng = Trng::new(xns).expect("can't connect to TRNG");
        let health_conn = trng::Trng::new(xns).expect("can't connect to TRNG");
        let health = trng_health(&health_conn);
        let mut app = Self {
            canvas,
            store,
            trng,
            health_conn,
            health,
            ui: Ui::new(),
            types_list: ListState::default(),
            saved_list: ListState::default(),
            visible_rows: 1,
        };
        app.reload();
        app
    }

    pub fn screen(&self) -> &Canvas { &self.canvas }

    pub fn needs_tick(&self) -> bool { self.ui.showing_secret() }

    /// (Re)load saved keys. Runs at start and whenever a basis is locked or
    /// unlocked, so nothing from a just-locked basis stays in memory or on screen.
    pub fn reload(&mut self) {
        let mut saved = Vec::new();
        let mut unreadable = 0;
        match self.store.keys() {
            Ok(names) => {
                for name in names.into_iter().filter(|n| is_record_key(n)) {
                    match self.store.read_bytes(&name) {
                        Ok(Some(bytes)) => {
                            let bytes = Zeroizing::new(bytes);
                            match Record::parse(&bytes) {
                                Ok(record) => saved.push(Saved { key: name, record }),
                                Err(e) => {
                                    log::warn!("record {} unreadable: {:?}", name, e);
                                    unreadable += 1;
                                }
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log::warn!("couldn't read {}: {:?}", name, e);
                            unreadable += 1;
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("couldn't list keys: {:?}", e);
                self.ui.status = "Couldn't read saved keys".into();
            }
        }
        self.ui.set_saved(saved, unreadable);
        self.draw();
    }

    pub fn on_key(&mut self, key: Key) -> Action {
        let effect = self.ui.on_key(to_input(key), &mut TrngSource(&self.trng));
        match effect {
            Effect::None => {}
            Effect::Leave => return Action::Leave,
            Effect::Save(key, record) => self.save(key, record),
            Effect::Delete(key) => match self.store.delete(&key) {
                Ok(()) => self.ui.on_deleted(&key),
                Err(e) => {
                    log::error!("delete failed: {:?}", e);
                    self.ui.on_delete_failed(&e.kind().to_string());
                }
            },
        }
        if self.ui.screen == Screen::Configure {
            self.health = trng_health(&self.health_conn);
        }
        Action::Stay
    }

    fn save(&mut self, key: String, record: Record) {
        let bytes = match serde_json::to_vec(&record) {
            Ok(b) => Zeroizing::new(b),
            Err(_) => {
                self.ui.on_save_failed("encoding");
                return;
            }
        };
        match self.store.write_bytes(&key, &bytes) {
            Ok(()) => {
                let basis = self.store.basis_of(&key).ok().flatten();
                self.ui.on_saved(key, record, basis.as_deref());
            }
            Err(e) => {
                log::error!("save failed: {:?}", e.kind());
                self.ui.on_save_failed(&e.kind().to_string());
            }
        }
    }

    pub fn on_tick(&mut self) {
        let hid = self.ui.on_tick();
        if hid || self.ui.seconds_left() % 10 == 0 {
            self.draw();
        }
    }

    pub fn on_foreground(&mut self) {
        self.canvas.refresh_size();
        self.health = trng_health(&self.health_conn);
        self.draw();
    }

    /// Take secrets off the display when another app comes to the front.
    pub fn on_background(&mut self) { self.ui.on_background(); }

    fn footer_and_status(&self, hint: &str) -> isize {
        let bottom = self.canvas.footer(hint);
        if self.ui.status.is_empty() {
            return bottom;
        }
        let h = self.canvas.line_height(GlyphStyle::Small);
        self.canvas.text(bottom - h, GlyphStyle::Small, &self.ui.status);
        bottom - h
    }

    /// The value as it should be read: hex/base64 in groups of 4, mnemonic
    /// words numbered.
    fn display_value(kind: Kind, value: &str) -> Zeroizing<String> {
        match kind {
            Kind::HexKey | Kind::Base64Key => grouped(value, 4),
            Kind::Mnemonic => {
                let mut s = Zeroizing::new(String::with_capacity(value.len() + 72));
                for (i, w) in value.split(' ').enumerate() {
                    if i > 0 {
                        s.push_str("  ");
                    }
                    s.push_str(&format!("{}.{}", i + 1, w));
                }
                s
            }
            _ => {
                let mut s = Zeroizing::new(String::with_capacity(value.len()));
                s.push_str(value);
                s
            }
        }
    }

    fn draw_value(&self, top: isize, bottom: isize, kind: Kind, value: &str) -> isize {
        let shown = Self::display_value(kind, value);
        let w = self.canvas.size().x;
        let used = self.canvas.text_in(Point::new(MARGIN, top), Point::new(w - MARGIN, bottom), GlyphStyle::Monospace, &shown);
        top + used + 6
    }

    pub fn draw(&mut self) {
        self.canvas.clear();
        match self.ui.screen {
            Screen::Types => self.draw_types(),
            Screen::Configure => self.draw_configure(),
            Screen::Result => self.draw_result(),
            Screen::Label => self.draw_label(),
            Screen::Saved => self.draw_saved(),
            Screen::View => self.draw_view(),
            Screen::ConfirmDelete => self.draw_confirm(),
        }
        self.canvas.flush();
    }

    fn draw_types(&mut self) {
        let top = self.canvas.header("Key Ceremony");
        let bottom = self.footer_and_status("↑↓ move · ⏎ select · S saved · ⌫ quit");
        let labels: Vec<&str> = Kind::ALL.iter().map(|k| k.label()).collect();
        self.types_list.selected = self.ui.type_cursor;
        self.types_list.clamp(labels.len(), self.visible_rows);
        self.visible_rows = self.canvas.list(top, bottom, &labels, &self.types_list).max(1);
    }

    fn draw_configure(&self) {
        let k = self.ui.kind;
        let mut y = self.canvas.header(k.label());
        let bottom = self.footer_and_status("←→ length · ⏎ generate · ⌫ back");
        let t = self.ui.entropy_tenths();
        y += self.canvas.text(y, GlyphStyle::Small, k.description());
        y += 6;
        y += self.canvas.text(y, GlyphStyle::Regular, &format!("Length: {} {}", self.ui.length, k.unit()));
        y += self.canvas.text(y, GlyphStyle::Regular, &format!("Entropy: {}", format_bits(t)));
        y += self.canvas.text(y, GlyphStyle::Regular, &format!("Strength: {}", strength(t)));
        y += 6;
        if y < bottom {
            self.canvas.text(y, GlyphStyle::Small, &self.health);
        }
    }

    fn draw_result(&self) {
        let k = self.ui.kind;
        let top = self.canvas.header(&format!("{} · {} {}", k.label(), self.ui.length, k.unit()));
        let bottom = self.footer_and_status("R new · S save · ⌫ back");
        let Some(v) = self.ui.current.as_ref() else { return };
        let t = self.ui.entropy_tenths();
        let mut y = self.draw_value(top, bottom - 60, k, v.expose());
        y += self.canvas.text(y, GlyphStyle::Small, &format!("{} · {}", format_bits(t), strength(t)));
        self.canvas.text(y, GlyphStyle::Small, &format!("Hides in about {} s", self.ui.seconds_left()));
    }

    fn draw_label(&self) {
        let mut y = self.canvas.header("Save to vault");
        self.footer_and_status("⏎ save · F4 cancel");
        y += self.canvas.text(y, GlyphStyle::Regular, "Label (optional):");
        let mut line = Zeroizing::new(self.ui.label.clone());
        line.push('_');
        y += self.canvas.text(y, GlyphStyle::Regular, &line);
        y += 6;
        let basis = self.store.target_basis().unwrap_or_else(|| "?".into());
        self.canvas.text(y, GlyphStyle::Small, &format!("New keys go to basis: {basis}"));
    }

    fn draw_saved(&mut self) {
        let n = self.ui.saved.len();
        let top = self.canvas.header(&format!("Saved keys ({n})"));
        let mut bottom = self.footer_and_status("⏎ view · D delete · ⌫ back");
        if self.ui.unreadable > 0 {
            let h = self.canvas.line_height(GlyphStyle::Small);
            self.canvas.text(bottom - h, GlyphStyle::Small, &format!("{} unreadable record(s) left untouched", self.ui.unreadable));
            bottom -= h;
        }
        if n == 0 {
            self.canvas.text(top + 10, GlyphStyle::Regular, "No saved keys. Generate one and press S.");
            return;
        }
        let labels: Vec<String> = self
            .ui
            .saved
            .iter()
            .map(|s| format!("{} · {}", s.record.title(), format_bits(s.record.bits_tenths)))
            .collect();
        self.saved_list.selected = self.ui.saved_cursor;
        self.saved_list.clamp(n, self.visible_rows);
        self.visible_rows = self.canvas.list(top, bottom, &labels, &self.saved_list).max(1);
    }

    fn draw_view(&self) {
        let Some(s) = self.ui.selected() else { return };
        let r = &s.record;
        let mut y = self.canvas.header(&r.title());
        let bottom = self.footer_and_status("D delete · ⌫ back");
        y += self.canvas.text(
            y,
            GlyphStyle::Small,
            &format!("{} · {} {} · {}", r.kind.label(), r.length, r.kind.unit(), format_bits(r.bits_tenths)),
        );
        let y = self.draw_value(y + 4, bottom - 30, r.kind, r.value.expose());
        self.canvas.text(y, GlyphStyle::Small, &format!("Hides in about {} s", self.ui.seconds_left()));
    }

    fn draw_confirm(&self) {
        let Some(s) = self.ui.selected() else { return };
        let mut y = self.canvas.header("Delete saved key?");
        self.footer_and_status("Y delete · any other key cancels");
        y += self.canvas.text(y, GlyphStyle::Regular, &s.record.title());
        y += 6;
        let note = "Removes the PDDB key. Old flash pages are not overwritten immediately.";
        self.canvas.text_in(Point::new(MARGIN, y), Point::new(self.canvas.size().x - MARGIN, y + 80), GlyphStyle::Small, note);
    }
}
