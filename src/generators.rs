//! Key/password/passphrase generators.
//!
//! All randomness from hardware TRNG.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::rng::Rng;

/// Generator types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GenType {
    Password,
    Pin,
    Passphrase,
    HexKey,
    Base64Key,
    WpaKey,
}

impl GenType {
    pub fn all() -> &'static [GenType] {
        &[
            GenType::Password,
            GenType::Pin,
            GenType::Passphrase,
            GenType::HexKey,
            GenType::Base64Key,
            GenType::WpaKey,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            GenType::Password => "Password",
            GenType::Pin => "PIN Code",
            GenType::Passphrase => "Passphrase",
            GenType::HexKey => "Hex Key",
            GenType::Base64Key => "Base64 Key",
            GenType::WpaKey => "WPA Key",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            GenType::Password => "Mixed chars: A-Z, a-z, 0-9, symbols",
            GenType::Pin => "Numeric only: 4-12 digits",
            GenType::Passphrase => "Random word sequence (Diceware-style)",
            GenType::HexKey => "Random hex bytes (for crypto keys)",
            GenType::Base64Key => "Random base64-encoded bytes",
            GenType::WpaKey => "63-char max ASCII for WPA2",
        }
    }

    pub fn default_length(&self) -> usize {
        match self {
            GenType::Password => 16,
            GenType::Pin => 6,
            GenType::Passphrase => 6, // words
            GenType::HexKey => 32,    // bytes (64 hex chars)
            GenType::Base64Key => 32, // bytes
            GenType::WpaKey => 20,
        }
    }

    pub fn min_length(&self) -> usize {
        match self {
            GenType::Password => 8,
            GenType::Pin => 4,
            GenType::Passphrase => 3,
            GenType::HexKey => 8,
            GenType::Base64Key => 8,
            GenType::WpaKey => 8,
        }
    }

    pub fn max_length(&self) -> usize {
        match self {
            GenType::Password => 64,
            GenType::Pin => 12,
            GenType::Passphrase => 12,
            GenType::HexKey => 64,
            GenType::Base64Key => 64,
            GenType::WpaKey => 63,
        }
    }

    pub fn length_unit(&self) -> &'static str {
        match self {
            GenType::Passphrase => "words",
            GenType::HexKey | GenType::Base64Key => "bytes",
            _ => "chars",
        }
    }
}

const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{}|;:,.<>?";
const HEX_CHARS: &[u8] = b"0123456789ABCDEF";
const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const ASCII_PRINTABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+";

/// Diceware-style word list (compact subset for embedded use).
const WORDS: &[&str] = &[
    "about", "above", "acid", "aged", "also", "area", "army", "away",
    "baby", "back", "ball", "band", "bank", "base", "bath", "bean",
    "bear", "beat", "been", "bell", "belt", "best", "bill", "bird",
    "bite", "blow", "blue", "boat", "body", "bolt", "bomb", "bond",
    "bone", "book", "born", "boss", "both", "bowl", "bulk", "burn",
    "cake", "call", "calm", "came", "camp", "card", "care", "case",
    "cash", "cast", "cell", "chat", "chip", "city", "clay", "club",
    "coal", "coat", "code", "coin", "cold", "come", "cook", "cool",
    "cope", "copy", "core", "cost", "crew", "crop", "crow", "cure",
    "dark", "data", "date", "dawn", "dead", "deal", "dear", "debt",
    "deep", "deny", "desk", "dial", "diet", "dirt", "dish", "disk",
    "dock", "does", "dome", "done", "door", "dose", "down", "draw",
    "drew", "drop", "drum", "dual", "duke", "dump", "dust", "duty",
    "each", "earn", "ease", "east", "edge", "else", "even", "ever",
    "exam", "evil", "exit", "face", "fact", "fail", "fair", "fall",
    "fame", "farm", "fast", "fate", "fear", "feed", "feel", "feet",
    "fell", "felt", "file", "fill", "film", "find", "fine", "fire",
    "firm", "fish", "fist", "five", "flag", "flat", "fled", "flew",
    "flip", "flow", "fold", "folk", "food", "foot", "ford", "form",
    "fort", "foul", "four", "free", "from", "fuel", "full", "fund",
    "gain", "game", "gang", "gate", "gave", "gear", "gene", "gift",
    "girl", "give", "glad", "glow", "glue", "goal", "goat", "goes",
    "gold", "golf", "gone", "good", "grab", "gray", "grew", "grid",
    "grip", "grow", "gulf", "guru", "hair", "half", "hall", "halt",
    "hand", "hang", "hard", "harm", "hate", "have", "head", "heal",
    "heap", "hear", "heat", "help", "herb", "here", "hero", "hide",
    "high", "hike", "hill", "hint", "hire", "hold", "hole", "holy",
    "home", "hope", "horn", "host", "hour", "huge", "hung", "hunt",
    "hurt", "idea", "inch", "into", "iron", "item", "jack", "jail",
    "jazz", "jean", "jobs", "join", "joke", "jump", "june", "jury",
    "just", "keen", "keep", "kept", "kick", "kill", "kind", "king",
    "knee", "knew", "knit", "knot", "know", "lack", "laid", "lake",
    "lamp", "land", "lane", "last", "late", "lawn", "lead", "leaf",
    "lean", "left", "lend", "lens", "less", "lick", "life", "lift",
    "like", "limb", "lime", "line", "link", "lion", "list", "live",
    "load", "loan", "lock", "logo", "long", "look", "lord", "lose",
    "loss", "lost", "loud", "love", "luck", "lump", "lung", "made",
    "mail", "main", "make", "male", "mall", "many", "mark", "mask",
    "mass", "math", "maze", "meal", "mean", "meat", "meet", "melt",
    "menu", "mere", "mild", "mile", "milk", "mill", "mind", "mine",
    "mint", "miss", "mode", "mood", "moon", "more", "most", "move",
    "much", "must", "myth", "nail", "name", "navy", "near", "neat",
    "neck", "need", "nest", "news", "next", "nine", "node", "none",
    "noon", "norm", "nose", "note", "noun", "odds", "okay", "once",
    "only", "onto", "open", "oral", "oven", "over", "pace", "pack",
    "page", "paid", "pain", "pair", "pale", "palm", "park", "part",
    "pass", "past", "path", "peak", "peer", "pick", "pile", "pine",
    "pink", "pipe", "plan", "play", "plot", "plug", "plus", "poem",
    "poet", "poll", "pond", "pool", "poor", "pope", "port", "pose",
    "post", "pour", "pray", "pull", "pump", "pure", "push", "quit",
    "race", "rage", "raid", "rail", "rain", "rank", "rare", "rate",
    "read", "real", "rear", "rely", "rent", "rest", "rice", "rich",
    "ride", "ring", "rise", "risk", "road", "rock", "rode", "role",
    "roll", "roof", "room", "root", "rope", "rose", "ruin", "rule",
    "rush", "safe", "said", "sake", "sale", "salt", "same", "sand",
    "sang", "save", "seal", "seat", "seed", "seek", "seem", "seen",
    "self", "sell", "send", "sent", "sept", "shed", "shin", "ship",
    "shop", "shot", "show", "shut", "sick", "side", "sign", "silk",
    "sing", "sink", "site", "size", "skin", "slam", "slid", "slim",
    "slip", "slot", "slow", "snap", "snow", "soap", "sock", "soft",
    "soil", "sold", "sole", "some", "song", "soon", "sort", "soul",
    "span", "spin", "spot", "star", "stay", "stem", "step", "stir",
    "stop", "such", "suit", "sure", "swim", "tail", "take", "tale",
    "talk", "tall", "tank", "tape", "task", "taxi", "team", "tell",
    "tend", "tent", "term", "test", "text", "than", "that", "them",
    "then", "they", "thin", "this", "thus", "tick", "tide", "tidy",
    "tied", "tier", "tile", "till", "time", "tiny", "tire", "toad",
    "told", "toll", "tone", "took", "tool", "tops", "tore", "torn",
    "tour", "town", "trap", "tree", "trim", "trip", "true", "tube",
    "tuck", "tune", "turn", "twin", "type", "ugly", "undo", "unit",
    "upon", "urge", "used", "user", "vale", "vary", "vast", "verb",
    "very", "veto", "vice", "view", "vine", "visa", "void", "volt",
    "vote", "wade", "wage", "wait", "wake", "walk", "wall", "want",
    "ward", "warm", "warn", "wash", "vast", "wave", "weak", "wear",
    "weed", "week", "well", "went", "were", "west", "what", "when",
    "whom", "wide", "wife", "wild", "will", "wind", "wine", "wing",
    "wire", "wise", "wish", "with", "woke", "wolf", "wood", "wool",
    "word", "wore", "work", "worm", "worn", "wrap", "writ", "yard",
    "yeah", "year", "yell", "yoga", "zero", "zone", "zoom",
];

/// Estimate entropy in bits.
pub fn estimate_entropy(gen_type: GenType, length: usize) -> f64 {
    match gen_type {
        GenType::Password => {
            // ~72 possible chars
            let pool = (LOWER.len() + UPPER.len() + DIGITS.len() + SYMBOLS.len()) as f64;
            length as f64 * pool.log2()
        }
        GenType::Pin => {
            length as f64 * 10.0_f64.log2()
        }
        GenType::Passphrase => {
            length as f64 * (WORDS.len() as f64).log2()
        }
        GenType::HexKey | GenType::Base64Key => {
            (length * 8) as f64 // full bytes of entropy
        }
        GenType::WpaKey => {
            let pool = ASCII_PRINTABLE.len() as f64;
            length as f64 * pool.log2()
        }
    }
}

/// Format entropy for display (integer bits).
pub fn entropy_display(gen_type: GenType, length: usize) -> u32 {
    // Avoid float — approximate with integer math
    match gen_type {
        GenType::Password => (length as u32) * 6, // ~6.17 bits/char
        GenType::Pin => (length as u32) * 3,      // ~3.32 bits/digit
        GenType::Passphrase => (length as u32) * 9, // ~9.3 bits/word for ~630 words
        GenType::HexKey | GenType::Base64Key => (length as u32) * 8,
        GenType::WpaKey => (length as u32) * 6,
    }
}

/// Generate a random password.
pub fn gen_password(rng: &Rng, length: usize) -> String {
    let mut charset: Vec<u8> = Vec::new();
    charset.extend_from_slice(LOWER);
    charset.extend_from_slice(UPPER);
    charset.extend_from_slice(DIGITS);
    charset.extend_from_slice(SYMBOLS);

    let mut result = String::with_capacity(length);
    // Ensure at least one of each category
    if length >= 4 {
        result.push(*rng.pick(LOWER) as char);
        result.push(*rng.pick(UPPER) as char);
        result.push(*rng.pick(DIGITS) as char);
        result.push(*rng.pick(SYMBOLS) as char);
        for _ in 4..length {
            result.push(*rng.pick(&charset) as char);
        }
        // Shuffle to randomize positions
        let mut chars: Vec<char> = result.chars().collect();
        rng.shuffle(&mut chars);
        result = chars.into_iter().collect();
    } else {
        for _ in 0..length {
            result.push(*rng.pick(&charset) as char);
        }
    }
    result
}

/// Generate a random PIN.
pub fn gen_pin(rng: &Rng, length: usize) -> String {
    (0..length).map(|_| *rng.pick(DIGITS) as char).collect()
}

/// Generate a Diceware-style passphrase.
pub fn gen_passphrase(rng: &Rng, word_count: usize) -> String {
    let mut words: Vec<&str> = Vec::new();
    for _ in 0..word_count {
        words.push(rng.pick(WORDS));
    }
    let mut result = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            result.push('-');
        }
        result.push_str(w);
    }
    result
}

/// Generate random hex key.
pub fn gen_hex_key(rng: &Rng, byte_count: usize) -> String {
    let bytes = rng.bytes(byte_count);
    let mut hex = String::with_capacity(byte_count * 2);
    for &b in bytes.iter() {
        hex.push(HEX_CHARS[(b >> 4) as usize] as char);
        hex.push(HEX_CHARS[(b & 0x0F) as usize] as char);
    }
    hex
}

/// Generate random base64 key.
pub fn gen_base64_key(rng: &Rng, byte_count: usize) -> String {
    let bytes = rng.bytes(byte_count);
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as usize;
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] as usize } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] as usize } else { 0 };

        result.push(BASE64_CHARS[(b0 >> 2)] as char);
        result.push(BASE64_CHARS[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        if i + 1 < bytes.len() {
            result.push(BASE64_CHARS[((b1 & 0x0F) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }
        if i + 2 < bytes.len() {
            result.push(BASE64_CHARS[b2 & 0x3F] as char);
        } else {
            result.push('=');
        }
        i += 3;
    }
    result
}

/// Generate WPA key (printable ASCII).
pub fn gen_wpa_key(rng: &Rng, length: usize) -> String {
    (0..length).map(|_| *rng.pick(ASCII_PRINTABLE) as char).collect()
}

/// Master generator dispatch.
pub fn generate(rng: &Rng, gen_type: GenType, length: usize) -> String {
    match gen_type {
        GenType::Password => gen_password(rng, length),
        GenType::Pin => gen_pin(rng, length),
        GenType::Passphrase => gen_passphrase(rng, length),
        GenType::HexKey => gen_hex_key(rng, length),
        GenType::Base64Key => gen_base64_key(rng, length),
        GenType::WpaKey => gen_wpa_key(rng, length),
    }
}
