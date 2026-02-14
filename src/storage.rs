//! PDDB storage for Key Ceremony.
//!
//! Dictionary: keygen.vault
//! Keys: generated — JSON array of saved generated keys

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

const DICT: &str = "keygen.vault";
const KEY_SAVED: &str = "generated";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedKey {
    pub gen_type: String,
    pub length: usize,
    pub entropy_bits: u32,
    pub value: String,
}

pub struct Storage {
    pddb: pddb::Pddb,
}

impl Storage {
    pub fn new() -> Result<Self, ()> {
        let pddb = pddb::Pddb::new();
        pddb.is_mounted_blocking();
        Ok(Self { pddb })
    }

    fn read_key(&mut self, key: &str) -> Option<Vec<u8>> {
        let mut handle = self
            .pddb
            .get(DICT, key, None, false, false, None, None::<fn()>)
            .ok()?;
        let mut buf = Vec::new();
        use std::io::Read;
        handle.read_to_end(&mut buf).ok()?;
        if buf.is_empty() { None } else { Some(buf) }
    }

    fn write_key(&mut self, key: &str, data: &[u8]) {
        if let Ok(mut handle) = self.pddb.get(
            DICT, key, None, true, true, Some(data.len()), None::<fn()>,
        ) {
            use std::io::{Seek, Write};
            handle.seek(std::io::SeekFrom::Start(0)).ok();
            handle.write_all(data).ok();
            handle.set_len(data.len() as u64).ok();
        }
        self.pddb.sync().ok();
    }

    pub fn load_saved(&mut self) -> Vec<SavedKey> {
        self.read_key(KEY_SAVED)
            .and_then(|buf| serde_json::from_slice(&buf).ok())
            .unwrap_or_default()
    }

    pub fn save_keys(&mut self, keys: &[SavedKey]) {
        let data = serde_json::to_vec(keys).unwrap_or_default();
        self.write_key(KEY_SAVED, &data);
    }
}
