# Key Ceremony — Build Notes

## Architecture
- `core/` (`keygen-core`, no Xous deps, host-tested): generators, exact
  entropy accounting, BIP-39, saved-record format, and the screen state
  machine (`Ui`, which returns `Effect`s for storage and leaving).
- `src/main.rs` routes GAM messages (template pattern); `src/app.rs` maps
  keys to `Ui`, performs effects via `appkit::store::Store`, draws via
  `appkit::screen::Screen`.
- Entropy source: `appkit::rng::Trng::try_u32` behind the core `Entropy`
  trait. A failed draw aborts the whole value (fail closed).
- Secrets are `keygen_core::Secret` (wiped on drop, redacted `Debug`).
- One PDDB key per saved value (`keygen.vault` / `k.<16 hex>`), so delete
  removes the key and each record stays in its own basis.

## Test / gate
```bash
cd core && cargo test
cd ../xous-dev-toolkit && bin/integrate --xous-root <xous-core> --grade -v ../precursor-keygen
```
