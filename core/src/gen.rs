//! Generators and their entropy.
//!
//! Every generator draws from an [`Entropy`] source and either returns a
//! complete value or [`EntropyError`]. The entropy shown to the user is the
//! exact base-2 log of the number of values a generator can produce with equal
//! probability, rounded *down* to 0.1 bit.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::entropy::{fill, pick, Entropy, EntropyError};
use crate::secret::Secret;
use crate::wordlist::WORDS;

pub const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
pub const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const DIGITS: &[u8] = b"0123456789";
pub const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{}|;:,.<>?";
/// The WPA set: letters, digits and 14 symbols that are easy to type on any keyboard.
pub const WPA_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+";
const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";
const BASE64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// A password must contain one character from each class. Generation draws
/// the whole password uniformly from the union and redraws until it does, so
/// every valid password is equally likely and the entropy is exact.
const CLASSES: [&[u8]; 4] = [LOWER, UPPER, DIGITS, SYMBOLS];
/// Password redraws before the source is declared broken. Acceptance is at
/// least 45% per draw at the minimum length, so a working TRNG never hits it.
const MAX_PASSWORD_DRAWS: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Password,
    Pin,
    Passphrase,
    Mnemonic,
    HexKey,
    Base64Key,
    WpaKey,
}

impl Kind {
    pub const ALL: [Kind; 7] =
        [Kind::Password, Kind::Pin, Kind::Passphrase, Kind::Mnemonic, Kind::HexKey, Kind::Base64Key, Kind::WpaKey];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Password => "Password",
            Kind::Pin => "PIN",
            Kind::Passphrase => "Passphrase",
            Kind::Mnemonic => "BIP-39 mnemonic",
            Kind::HexKey => "Hex key",
            Kind::Base64Key => "Base64 key",
            Kind::WpaKey => "WPA key",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Kind::Password => "88 chars: a-z A-Z 0-9 symbols, at least one of each",
            Kind::Pin => "Digits 0-9",
            Kind::Passphrase => "Random words from the BIP-39 list, 11 bits each",
            Kind::Mnemonic => "BIP-39 seed phrase with checksum",
            Kind::HexKey => "Random bytes as hex",
            Kind::Base64Key => "Random bytes as base64 (RFC 4648)",
            Kind::WpaKey => "76 chars: letters, digits, 14 symbols",
        }
    }

    pub fn default_length(self) -> usize {
        match self {
            Kind::Password => 16,
            Kind::Pin => 6,
            Kind::Passphrase => 6,
            Kind::Mnemonic => 24,
            Kind::HexKey | Kind::Base64Key => 32,
            Kind::WpaKey => 20,
        }
    }

    pub fn min_length(self) -> usize {
        match self {
            Kind::Password | Kind::HexKey | Kind::Base64Key | Kind::WpaKey => 8,
            Kind::Pin => 4,
            Kind::Passphrase => 3,
            Kind::Mnemonic => 12,
        }
    }

    pub fn max_length(self) -> usize {
        match self {
            Kind::Password | Kind::HexKey | Kind::Base64Key => 64,
            Kind::WpaKey => 63,
            Kind::Pin | Kind::Passphrase => 12,
            Kind::Mnemonic => 24,
        }
    }

    /// Length change per ←/→ press. Mnemonics come in multiples of 3 words.
    pub fn step(self) -> usize {
        if self == Kind::Mnemonic {
            3
        } else {
            1
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Kind::Passphrase | Kind::Mnemonic => "words",
            Kind::HexKey | Kind::Base64Key => "bytes",
            Kind::Pin => "digits",
            _ => "chars",
        }
    }

    /// Clamp a length into range (and onto the step grid).
    pub fn clamp(self, len: usize) -> usize {
        let l = len.clamp(self.min_length(), self.max_length());
        l - (l - self.min_length()) % self.step()
    }

    /// Next length up or down, clamped.
    pub fn adjust(self, len: usize, up: bool) -> usize {
        let len = self.clamp(len);
        if up {
            self.clamp(len + self.step())
        } else {
            self.clamp(len.saturating_sub(self.step()))
        }
    }
}

/// Exact entropy in bits of `kind` at `len` (after clamping).
pub fn entropy_bits(kind: Kind, len: usize) -> f64 {
    let len = kind.clamp(len);
    let l = len as f64;
    match kind {
        Kind::Password => password_bits(len),
        Kind::Pin => l * 10f64.log2(),
        Kind::Passphrase => l * 11.0,
        // 32 bits of entropy per 3 words; the other bit per word is the checksum.
        Kind::Mnemonic => (len * 32 / 3) as f64,
        Kind::HexKey | Kind::Base64Key => l * 8.0,
        Kind::WpaKey => l * (WPA_CHARS.len() as f64).log2(),
    }
}

/// Entropy in tenths of a bit, rounded down (what the UI shows).
pub fn entropy_tenths(kind: Kind, len: usize) -> u32 {
    // The small epsilon keeps exact values like 128.0 from flooring to 127.9.
    ((entropy_bits(kind, len) + 1e-9) * 10.0).floor() as u32
}

/// "51.6 bits".
pub fn format_bits(tenths: u32) -> String { format!("{}.{} bits", tenths / 10, tenths % 10) }

/// log2 of the number of length-`len` strings over the 88-char union that
/// contain every class, by inclusion-exclusion, computed in the log domain.
fn password_bits(len: usize) -> f64 {
    let total: usize = CLASSES.iter().map(|c| c.len()).sum();
    let mut sum = 0.0f64;
    for mask in 0u32..(1 << CLASSES.len()) {
        let excluded: usize = (0..CLASSES.len()).filter(|i| mask & (1 << i) != 0).map(|i| CLASSES[i].len()).sum();
        let term = ((total - excluded) as f64 / total as f64).powi(len as i32);
        if mask.count_ones() % 2 == 0 {
            sum += term;
        } else {
            sum -= term;
        }
    }
    len as f64 * (total as f64).log2() + sum.log2()
}

/// Strength label from entropy. Thresholds in bits: <40 weak, <64 moderate,
/// <80 good, <112 strong, otherwise excellent.
pub fn strength(tenths: u32) -> &'static str {
    match tenths / 10 {
        0..=39 => "WEAK",
        40..=63 => "MODERATE",
        64..=79 => "GOOD",
        80..=111 => "STRONG",
        _ => "EXCELLENT",
    }
}

/// Generate a value. `len` is clamped into the kind's range.
pub fn generate<E: Entropy + ?Sized>(e: &mut E, kind: Kind, len: usize) -> Result<Secret, EntropyError> {
    let len = kind.clamp(len);
    match kind {
        Kind::Password => password(e, len),
        Kind::Pin => from_charset(e, DIGITS, len),
        Kind::Passphrase => passphrase(e, len),
        Kind::Mnemonic => {
            let mut ent = Zeroizing::new(vec![0u8; len * 4 / 3]);
            fill(e, &mut ent)?;
            Ok(mnemonic(&ent).expect("mnemonic length is valid by construction"))
        }
        Kind::HexKey => {
            let mut bytes = Zeroizing::new(vec![0u8; len]);
            fill(e, &mut bytes)?;
            Ok(hex(&bytes))
        }
        Kind::Base64Key => {
            let mut bytes = Zeroizing::new(vec![0u8; len]);
            fill(e, &mut bytes)?;
            Ok(base64(&bytes))
        }
        Kind::WpaKey => from_charset(e, WPA_CHARS, len),
    }
}

fn from_charset<E: Entropy + ?Sized>(e: &mut E, set: &[u8], len: usize) -> Result<Secret, EntropyError> {
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        match pick(e, set) {
            Ok(c) => out.push(*c),
            Err(err) => {
                out.zeroize();
                return Err(err);
            }
        }
    }
    Ok(Secret::from_ascii(out).expect("charset is ASCII"))
}

fn password<E: Entropy + ?Sized>(e: &mut E, len: usize) -> Result<Secret, EntropyError> {
    let union: Vec<u8> = CLASSES.concat();
    for _ in 0..MAX_PASSWORD_DRAWS {
        let candidate = from_charset(e, &union, len)?;
        let b = candidate.expose().as_bytes();
        if CLASSES.iter().all(|class| b.iter().any(|c| class.contains(c))) {
            return Ok(candidate);
        }
    }
    Err(EntropyError)
}

fn passphrase<E: Entropy + ?Sized>(e: &mut E, words: usize) -> Result<Secret, EntropyError> {
    let mut out = String::with_capacity(words * 9);
    for i in 0..words {
        let w = match pick(e, &WORDS) {
            Ok(w) => w,
            Err(err) => {
                out.zeroize();
                return Err(err);
            }
        };
        if i > 0 {
            out.push('-');
        }
        out.push_str(w);
    }
    Ok(Secret::new(out))
}

/// Uppercase hex.
pub fn hex(bytes: &[u8]) -> Secret {
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_CHARS[(b >> 4) as usize]);
        out.push(HEX_CHARS[(b & 0x0F) as usize]);
    }
    Secret::from_ascii(out).expect("hex is ASCII")
}

/// Standard base64 with `=` padding (RFC 4648 section 4).
pub fn base64(bytes: &[u8]) -> Secret {
    let mut out = Vec::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(BASE64_CHARS[b0 >> 2]);
        out.push(BASE64_CHARS[((b0 & 0x03) << 4) | (b1 >> 4)]);
        out.push(if chunk.len() > 1 { BASE64_CHARS[((b1 & 0x0F) << 2) | (b2 >> 6)] } else { b'=' });
        out.push(if chunk.len() > 2 { BASE64_CHARS[b2 & 0x3F] } else { b'=' });
    }
    Secret::from_ascii(out).expect("base64 is ASCII")
}

/// BIP-39 mnemonic for 16, 20, 24, 28 or 32 bytes of entropy. `None` for any
/// other length.
pub fn mnemonic(entropy: &[u8]) -> Option<Secret> {
    if !(16..=32).contains(&entropy.len()) || entropy.len() % 4 != 0 {
        return None;
    }
    let ent_bits = entropy.len() * 8;
    let cs_bits = ent_bits / 32;
    let hash = Sha256::digest(entropy);
    let mut bits = Zeroizing::new(Vec::with_capacity(entropy.len() + 1));
    bits.extend_from_slice(entropy);
    bits.push(hash[0]);
    let words = (ent_bits + cs_bits) / 11;
    let mut out = String::with_capacity(words * 9);
    for w in 0..words {
        let mut idx = 0usize;
        for i in 0..11 {
            let bit = w * 11 + i;
            let set = bits[bit / 8] & (0x80 >> (bit % 8)) != 0;
            idx = (idx << 1) | set as usize;
        }
        if w > 0 {
            out.push(' ');
        }
        out.push_str(WORDS[idx]);
    }
    Some(Secret::new(out))
}

/// Insert a space every `n` characters, for reading long hex/base64 values aloud.
pub fn grouped(s: &str, n: usize) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(s.len() + s.len() / n.max(1)));
    for (i, c) in s.chars().enumerate() {
        if i > 0 && n > 0 && i % n == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entropy::testing::*;

    fn unhex(s: &str) -> Vec<u8> { (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect() }

    #[test]
    fn wordlist_is_bip39_and_unique() {
        // Regression: the old list had 631 entries with "vast" twice.
        let mut w: Vec<&str> = WORDS.to_vec();
        w.sort();
        w.dedup();
        assert_eq!(w.len(), 2048);
        assert_eq!(WORDS[0], "abandon");
        assert_eq!(WORDS[2047], "zoo");
    }

    #[test]
    fn bip39_reference_vectors() {
        // Trezor reference vectors (python-mnemonic vectors.json).
        let cases = [
            ("00000000000000000000000000000000", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"),
            ("7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f", "legal winner thank year wave sausage worth useful legal winner thank yellow"),
            ("80808080808080808080808080808080", "letter advice cage absurd amount doctor acoustic avoid letter advice cage above"),
            ("ffffffffffffffffffffffffffffffff", "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong"),
            ("000000000000000000000000000000000000000000000000", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon agent"),
            ("ffffffffffffffffffffffffffffffffffffffffffffffff", "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo when"),
            ("0000000000000000000000000000000000000000000000000000000000000000", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"),
            ("7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f", "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title"),
            ("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote"),
        ];
        for (ent, want) in cases {
            assert_eq!(mnemonic(&unhex(ent)).unwrap().expose(), want, "entropy {}", ent);
        }
    }

    #[test]
    fn bip39_matches_xous_gam_vector() {
        // services/gam/src/bip39.rs test_bytes_to_bip39 at the anchor.
        let n: u128 = 0b00000110001101100111100111001010000110110010100010110101110011111101101010011100000110000110101100110110011111100010011100011110;
        assert_eq!(
            mnemonic(&n.to_be_bytes()).unwrap().expose(),
            "alert record income curve mercy tree heavy loan hen recycle mean devote"
        );
    }

    #[test]
    fn bip39_rejects_bad_lengths() {
        assert!(mnemonic(&[0u8; 15]).is_none());
        assert!(mnemonic(&[0u8; 18]).is_none());
        assert!(mnemonic(&[0u8; 36]).is_none());
    }

    #[test]
    fn base64_rfc4648_vectors() {
        for (input, want) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(input.as_bytes()).expose(), want);
        }
    }

    #[test]
    fn hex_encoding() {
        assert_eq!(hex(&[0x00, 0x4a, 0xff]).expose(), "004AFF");
    }

    #[test]
    fn grouping() {
        assert_eq!(grouped("ABCDEFGHIJ", 4).as_str(), "ABCD EFGH IJ");
    }

    #[test]
    fn password_has_every_class_and_right_length() {
        let mut r = XorShift(7);
        for len in [8, 16, 64] {
            for _ in 0..200 {
                let p = generate(&mut r, Kind::Password, len).unwrap();
                let b = p.expose().as_bytes();
                assert_eq!(b.len(), len);
                for class in CLASSES {
                    assert!(b.iter().any(|c| class.contains(c)), "{:?}", p);
                }
            }
        }
    }

    #[test]
    fn password_draws_are_uniform_over_the_union() {
        // Rejection (not forced placement) means each position is uniform over
        // all 88 chars; check one position's frequencies.
        let mut r = XorShift(99);
        let union: Vec<u8> = CLASSES.concat();
        let mut counts = [0u32; 88];
        for _ in 0..20_000 {
            let p = generate(&mut r, Kind::Password, 16).unwrap();
            let c = p.expose().as_bytes()[0];
            counts[union.iter().position(|&u| u == c).unwrap()] += 1;
        }
        let expect = 20_000.0 / 88.0;
        for c in counts {
            assert!((c as f64 - expect).abs() < expect * 0.35, "{:?}", counts);
        }
    }

    #[test]
    fn password_entropy_is_exact_and_below_naive() {
        // Regression: the old UI showed 6 bits/char. Exact value for 8 chars:
        // log2(sum over class subsets of (-1)^|S| (88 - |S|)^8).
        let naive = 8.0 * 88f64.log2();
        let exact = password_bits(8);
        assert!(exact < naive && exact > naive - 1.2, "{}", exact);
        // Cross-check the log-domain computation against a direct count.
        let sizes = [26usize, 26, 10, 26];
        let mut exact_count = 0f64;
        for mask in 0u32..16 {
            let ex: usize = (0..4).filter(|i| mask & (1 << i) != 0).map(|i| sizes[i]).sum();
            let t = ((88 - ex) as f64).powi(8);
            exact_count += if mask.count_ones() % 2 == 0 { t } else { -t };
        }
        assert!((exact_count.log2() - exact).abs() < 1e-9);
    }

    #[test]
    fn displayed_entropy_values() {
        assert_eq!(entropy_tenths(Kind::Pin, 6), 199); // 19.93
        assert_eq!(entropy_tenths(Kind::Password, 8), 506); // 50.63, not 8 x 6.46
        assert_eq!(entropy_tenths(Kind::Password, 16), 1031);
        assert_eq!(entropy_tenths(Kind::Passphrase, 6), 660);
        assert_eq!(entropy_tenths(Kind::Mnemonic, 12), 1280);
        assert_eq!(entropy_tenths(Kind::Mnemonic, 24), 2560);
        assert_eq!(entropy_tenths(Kind::HexKey, 32), 2560);
        assert_eq!(entropy_tenths(Kind::WpaKey, 20), 1249); // 20 * 6.2479
        assert_eq!(format_bits(1249), "124.9 bits");
        assert_eq!(strength(199), "WEAK");
        assert_eq!(strength(2560), "EXCELLENT");
    }

    #[test]
    fn every_kind_generates_at_every_length() {
        let mut r = XorShift(3);
        for kind in Kind::ALL {
            let mut len = kind.min_length();
            loop {
                let v = generate(&mut r, kind, len).unwrap();
                let n = match kind {
                    Kind::Passphrase => v.expose().split('-').count(),
                    Kind::Mnemonic => v.expose().split(' ').count(),
                    Kind::HexKey => v.len() / 2,
                    Kind::Base64Key => len, // checked by vectors
                    _ => v.len(),
                };
                assert_eq!(n, len, "{:?} {}", kind, len);
                let next = kind.adjust(len, true);
                if next == len {
                    break;
                }
                len = next;
            }
        }
    }

    #[test]
    fn mnemonic_lengths_step_by_three() {
        assert_eq!(Kind::Mnemonic.adjust(12, true), 15);
        assert_eq!(Kind::Mnemonic.adjust(24, true), 24);
        assert_eq!(Kind::Mnemonic.adjust(12, false), 12);
        assert_eq!(Kind::Mnemonic.clamp(13), 12);
        assert_eq!(Kind::Pin.adjust(4, false), 4);
    }

    #[test]
    fn broken_trng_generates_nothing() {
        // Regression for the fail-open RNG: every kind must return Err, never a
        // constant value.
        for kind in Kind::ALL {
            assert!(generate(&mut Broken, kind, kind.default_length()).is_err(), "{:?}", kind);
            assert!(generate(&mut FailAfter(XorShift(5), 3), kind, kind.default_length()).is_err(), "{:?}", kind);
        }
    }

    #[test]
    fn stuck_trng_password_fails_instead_of_looping() {
        // A constant word produces the same char everywhere: no password can
        // ever contain all four classes. Must terminate with an error.
        assert!(generate(&mut Stuck(0x8000_0000), Kind::Password, 16).is_err());
    }

    #[test]
    fn passphrase_words_are_from_the_list() {
        let mut r = XorShift(11);
        let p = generate(&mut r, Kind::Passphrase, 12).unwrap();
        for w in p.expose().split('-') {
            assert!(WORDS.contains(&w));
        }
    }
}
