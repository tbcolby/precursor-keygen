//! Pure logic for Key Ceremony: generators with exact entropy, BIP-39,
//! saved-record format, and the screen state machine. No Xous dependencies;
//! everything here is unit-tested on the host.

mod brute_check;
pub mod entropy;
pub mod gen;
pub mod record;
pub mod secret;
pub mod ui;
mod wordlist;

pub use entropy::{Entropy, EntropyError};
pub use gen::Kind;
pub use record::Record;
pub use secret::Secret;
pub use ui::{Effect, Input, Saved, Screen, Ui};
