# precursor-keygen

TRNG-powered key ceremony tool for the [Precursor](https://precursor.dev) hardware platform.

## What This Is

A cryptographic key and password generator powered by Precursor's hardware true random number generator. Generate passwords, PINs, passphrases, hex keys, base64 keys, and WPA keys — all from quantum noise, not algorithms. Save generated keys in hardware-encrypted flash.

## Generators

| Type | Output | Example |
|------|--------|---------|
| Password | A-Z, a-z, 0-9, symbols | `kR7!mZ2@pL9#nW4$` |
| PIN Code | Digits only | `384729` |
| Passphrase | Diceware-style words | `calm-hero-dust-vine-peak-sold` |
| Hex Key | Hex bytes | `4A8F2C71D3E5B096` |
| Base64 Key | Base64-encoded bytes | `So8scdfls0CA+w==` |
| WPA Key | Printable ASCII | `xK7mR2nW4pL9qT5v` |

## Features

- **6 Generator Types** — from simple PINs to cryptographic keys
- **Configurable Length** — adjust with arrow keys, see entropy in real-time
- **Entropy Estimation** — bits of entropy displayed for each configuration
- **Strength Rating** — WEAK / MODERATE / GOOD / STRONG / EXCELLENT
- **Regenerate** — one-key regenerate with R or Space
- **Save to Vault** — store generated keys in PDDB encrypted flash
- **~630 Word Dictionary** — Diceware-style passphrase generation

## Controls

| Key | Action |
|-----|--------|
| ↑/↓ | Navigate types |
| ←/→ | Adjust length |
| Enter/Space | Generate |
| R | Regenerate |
| S | Save to vault / View saved |
| D | Delete saved key |
| Menu (∴) | Back/quit |

## Build

```bash
cargo build -p keygen --target riscv32imac-unknown-xous-elf
```

---

## Development

This app was developed using the methodology described in [xous-dev-toolkit](https://github.com/tbcolby/xous-dev-toolkit) — an LLM-assisted approach to Precursor app development on macOS ARM64.

---

## Author

Made by Tyler Colby — [Colby's Data Movers, LLC](https://colbysdatamovers.com)

Contact: [tyler@colbysdatamovers.com](mailto:tyler@colbysdatamovers.com) | [GitHub Issues](https://github.com/tbcolby/precursor-keygen/issues)

---

## License

Licensed under the Apache License, Version 2.0.

See [LICENSE](LICENSE) for the full text.
