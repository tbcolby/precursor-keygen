//! State machine for Key Ceremony.
//!
//! States:
//!   TypeSelect  — choose generator type
//!   Configure   — set length parameter
//!   Result      — view generated key with entropy estimate
//!   Saved       — view previously saved keys

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::generators::*;
use crate::rng::Rng;
use crate::storage::{SavedKey, Storage};

const KEY_UP: char = '\u{F700}';
const KEY_DOWN: char = '\u{F701}';
const KEY_LEFT: char = '\u{F702}';
const KEY_RIGHT: char = '\u{F703}';
const KEY_ENTER: char = '\u{000D}';
const KEY_BACKSPACE: char = '\u{0008}';
const KEY_MENU: char = '\u{2234}';

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    TypeSelect,
    Configure,
    Result,
    Saved,
}

pub struct KeygenApp {
    pub state: AppState,
    pub needs_redraw: bool,

    // Type selection
    pub type_cursor: usize,
    pub selected_type: GenType,

    // Configuration
    pub length: usize,

    // Generated result
    pub result_value: String,
    pub result_entropy: u32,

    // Saved keys
    pub saved: Vec<SavedKey>,
    pub saved_cursor: usize,

    // Storage
    storage: Option<Storage>,
}

impl KeygenApp {
    pub fn new() -> Self {
        Self {
            state: AppState::TypeSelect,
            needs_redraw: true,
            type_cursor: 0,
            selected_type: GenType::Password,
            length: 16,
            result_value: String::new(),
            result_entropy: 0,
            saved: Vec::new(),
            saved_cursor: 0,
            storage: None,
        }
    }

    pub fn init_storage(&mut self) {
        if let Ok(mut st) = Storage::new() {
            self.saved = st.load_saved();
            self.storage = Some(st);
        }
    }

    pub fn save_state(&mut self) {
        if let Some(ref mut st) = self.storage {
            st.save_keys(&self.saved);
        }
    }

    pub fn handle_key(&mut self, key: char, rng: &Rng) -> bool {
        self.needs_redraw = true;
        match self.state {
            AppState::TypeSelect => self.handle_type_select(key),
            AppState::Configure => self.handle_configure(key, rng),
            AppState::Result => self.handle_result(key, rng),
            AppState::Saved => self.handle_saved(key),
        }
    }

    fn handle_type_select(&mut self, key: char) -> bool {
        let types = GenType::all();
        match key {
            KEY_MENU => return false,
            KEY_UP => {
                if self.type_cursor > 0 {
                    self.type_cursor -= 1;
                }
            }
            KEY_DOWN => {
                if self.type_cursor < types.len() - 1 {
                    self.type_cursor += 1;
                }
            }
            KEY_ENTER => {
                self.selected_type = types[self.type_cursor];
                self.length = self.selected_type.default_length();
                self.state = AppState::Configure;
            }
            's' | 'S' => {
                self.saved_cursor = 0;
                self.state = AppState::Saved;
            }
            _ => {}
        }
        true
    }

    fn handle_configure(&mut self, key: char, rng: &Rng) -> bool {
        match key {
            KEY_MENU => {
                self.state = AppState::TypeSelect;
            }
            KEY_LEFT => {
                let min = self.selected_type.min_length();
                if self.length > min {
                    self.length -= 1;
                }
            }
            KEY_RIGHT => {
                let max = self.selected_type.max_length();
                if self.length < max {
                    self.length += 1;
                }
            }
            KEY_ENTER | ' ' => {
                self.result_value = generate(rng, self.selected_type, self.length);
                self.result_entropy = entropy_display(self.selected_type, self.length);
                self.state = AppState::Result;
            }
            _ => {}
        }
        true
    }

    fn handle_result(&mut self, key: char, rng: &Rng) -> bool {
        match key {
            KEY_MENU | KEY_LEFT => {
                self.state = AppState::TypeSelect;
            }
            'r' | 'R' | ' ' => {
                // Regenerate
                self.result_value = generate(rng, self.selected_type, self.length);
                self.result_entropy = entropy_display(self.selected_type, self.length);
            }
            's' | 'S' => {
                // Save
                let saved_key = SavedKey {
                    gen_type: String::from(self.selected_type.label()),
                    length: self.length,
                    entropy_bits: self.result_entropy,
                    value: self.result_value.clone(),
                };
                self.saved.push(saved_key);
                self.save_state();
            }
            _ => {}
        }
        true
    }

    fn handle_saved(&mut self, key: char) -> bool {
        match key {
            KEY_MENU | KEY_LEFT => {
                self.state = AppState::TypeSelect;
            }
            KEY_UP => {
                if self.saved_cursor > 0 {
                    self.saved_cursor -= 1;
                }
            }
            KEY_DOWN => {
                if !self.saved.is_empty() && self.saved_cursor < self.saved.len() - 1 {
                    self.saved_cursor += 1;
                }
            }
            'd' | 'D' => {
                if self.saved_cursor < self.saved.len() {
                    self.saved.remove(self.saved_cursor);
                    if self.saved_cursor > 0 && self.saved_cursor >= self.saved.len() {
                        self.saved_cursor = self.saved.len().saturating_sub(1);
                    }
                    self.save_state();
                }
            }
            _ => {}
        }
        true
    }
}
