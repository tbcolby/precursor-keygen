//! A generated value that is wiped on drop and never printed by `Debug`.

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroize;

/// Generated key material. The bytes are overwritten when the value is
/// dropped, and `Debug` prints only the length, so a stray `{:?}` in a log
/// line can't leak it. There is deliberately no `Clone`; use
/// [`Secret::duplicate`] where a second copy is really needed.
pub struct Secret(String);

impl Secret {
    /// Take ownership of `s`. Build `s` with its final capacity so no
    /// reallocation leaves an unwiped copy behind.
    pub fn new(s: String) -> Self { Secret(s) }

    /// Build from ASCII bytes without copying them (the buffer becomes the
    /// secret's storage). Non-UTF-8 input is wiped and rejected.
    pub fn from_ascii(mut bytes: Vec<u8>) -> Option<Self> {
        if !bytes.is_ascii() {
            bytes.zeroize();
            return None;
        }
        // is_ascii() guarantees valid UTF-8, so this never fails.
        String::from_utf8(bytes).ok().map(Secret)
    }

    pub fn expose(&self) -> &str { &self.0 }

    pub fn len(&self) -> usize { self.0.len() }

    pub fn is_empty(&self) -> bool { self.0.is_empty() }

    /// An explicit second copy (e.g. the record that goes to storage).
    pub fn duplicate(&self) -> Secret {
        let mut s = String::with_capacity(self.0.len());
        s.push_str(&self.0);
        Secret(s)
    }
}

impl Drop for Secret {
    fn drop(&mut self) { self.0.zeroize(); }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Secret(<{} chars redacted>)", self.0.len()) }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_str(&self.0) }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> { String::deserialize(d).map(Secret) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let s = Secret::new("hunter2".to_string());
        let shown = format!("{:?}", s);
        assert!(!shown.contains("hunter2"), "{}", shown);
        assert!(shown.contains('7'));
    }

    #[test]
    fn serde_roundtrip() {
        let s = Secret::new("abc".to_string());
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(j, "\"abc\"");
        let back: Secret = serde_json::from_str(&j).unwrap();
        assert_eq!(back.expose(), "abc");
    }

    #[test]
    fn from_ascii_rejects_non_ascii() {
        assert!(Secret::from_ascii(vec![0xC3, 0xA9]).is_none());
        assert_eq!(Secret::from_ascii(b"ok".to_vec()).unwrap().expose(), "ok");
    }

    #[test]
    fn duplicate_is_equal() {
        let s = Secret::new("xyz".to_string());
        assert_eq!(s.duplicate().expose(), "xyz");
    }
}
