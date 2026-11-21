//! Fallible randomness, re-exported from `precursor-appkit-core` (shared by
//! every Precursor app). A failed draw aborts generation with
//! [`EntropyError`]; nothing partially random is returned. On the device the
//! source is `precursor_appkit::rng::Trng`, which implements [`Entropy`].

pub use precursor_appkit_core::entropy::{below, fill, pick, shuffle, Entropy, EntropyError, MAX_REJECTIONS};

#[cfg(test)]
pub(crate) use precursor_appkit_core::entropy::testing;
