//! Saved keys. Each one is its own PDDB key (`k.<16 hex>`) so that deleting
//! a saved key deletes its PDDB key, and a record lives in exactly one basis.

use serde::{Deserialize, Serialize};

use crate::entropy::{Entropy, EntropyError};
use crate::gen::Kind;
use crate::secret::Secret;

/// Bump when the format changes. Records with a newer version are shown as
/// unreadable and never rewritten.
pub const RECORD_VERSION: u32 = 1;
pub const KEY_PREFIX: &str = "k.";
pub const MAX_LABEL: usize = 24;

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub version: u32,
    /// Ordering only (largest existing + 1); collisions are harmless.
    pub seq: u64,
    pub label: String,
    pub kind: Kind,
    pub length: usize,
    pub bits_tenths: u32,
    pub value: Secret,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordError {
    Unparsable,
    NewerVersion(u32),
}

impl Record {
    /// Parse stored bytes. The caller wipes `bytes` afterwards.
    pub fn parse(bytes: &[u8]) -> Result<Record, RecordError> {
        #[derive(Deserialize)]
        struct V {
            version: u32,
        }
        let v: V = serde_json::from_slice(bytes).map_err(|_| RecordError::Unparsable)?;
        if v.version > RECORD_VERSION {
            return Err(RecordError::NewerVersion(v.version));
        }
        serde_json::from_slice(bytes).map_err(|_| RecordError::Unparsable)
    }

    /// Display name in the saved list. Never includes the value.
    pub fn title(&self) -> String {
        if self.label.is_empty() {
            format!("{} ({} {})", self.kind.label(), self.length, self.kind.unit())
        } else {
            self.label.clone()
        }
    }
}

/// True for PDDB key names that this app wrote.
pub fn is_record_key(name: &str) -> bool {
    name.len() == KEY_PREFIX.len() + 16
        && name.starts_with(KEY_PREFIX)
        && name[KEY_PREFIX.len()..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// A fresh random key name. Random rather than a counter, so it can't collide
/// with a record in a basis that is currently locked.
pub fn new_key<E: Entropy + ?Sized>(e: &mut E) -> Result<String, EntropyError> {
    let hi = e.try_u32()?;
    let lo = e.try_u32()?;
    Ok(format!("{KEY_PREFIX}{hi:08x}{lo:08x}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entropy::testing::*;

    fn sample() -> Record {
        Record {
            version: RECORD_VERSION,
            seq: 3,
            label: "wifi".into(),
            kind: Kind::WpaKey,
            length: 20,
            bits_tenths: 1249,
            value: Secret::new("abcDEF123!".into()),
        }
    }

    #[test]
    fn roundtrip() {
        let j = serde_json::to_vec(&sample()).unwrap();
        let r = Record::parse(&j).unwrap();
        assert_eq!(r.value.expose(), "abcDEF123!");
        assert_eq!(r.kind, Kind::WpaKey);
        assert_eq!(r.seq, 3);
    }

    #[test]
    fn debug_does_not_leak_value() {
        assert!(!format!("{:?}", sample()).contains("abcDEF123!"));
    }

    #[test]
    fn newer_version_is_refused() {
        let j = br#"{"version":9,"anything":1}"#;
        assert_eq!(Record::parse(j).unwrap_err(), RecordError::NewerVersion(9));
    }

    #[test]
    fn garbage_is_unparsable_not_empty() {
        // Regression: a corrupt store used to load as an empty list and then be
        // overwritten. Here it is reported, and nothing rewrites it.
        assert_eq!(Record::parse(b"{\"version\":1,\"seq\"").unwrap_err(), RecordError::Unparsable);
        // Stale tail from an in-place shorter rewrite (the old set_len bug).
        let mut j = serde_json::to_vec(&sample()).unwrap();
        j.extend_from_slice(b"ue\"}");
        assert_eq!(Record::parse(&j).unwrap_err(), RecordError::Unparsable);
    }

    #[test]
    fn keys() {
        let k = new_key(&mut Seq(vec![0xdead_beef, 1], 0)).unwrap();
        assert_eq!(k, "k.deadbeef00000001");
        assert!(is_record_key(&k));
        assert!(!is_record_key("generated"));
        assert!(!is_record_key("k.xyz"));
        assert!(new_key(&mut Broken).is_err());
    }

    #[test]
    fn title_never_shows_value() {
        let mut r = sample();
        r.label.clear();
        assert_eq!(r.title(), "WPA key (20 chars)");
    }
}
