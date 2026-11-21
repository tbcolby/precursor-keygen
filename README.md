# precursor-keygen: Key Ceremony

Password, PIN, passphrase, BIP-39 mnemonic and key-material generator for the [Precursor](https://precursor.dev), using the device's hardware TRNG.

## What it does

| Type | Output | Length range | Entropy |
|---|---|---|---|
| Password | 88 chars (`a-z A-Z 0-9` + 26 symbols), at least one of each class | 8–64 chars | exact count of valid passwords, e.g. 103.1 bits at 16, 50.6 at 8 |
| PIN | digits | 4–12 | 3.32 bits/digit |
| Passphrase | words from the 2048-word BIP-39 English list, joined by `-` | 3–12 words | 11 bits/word |
| BIP-39 mnemonic | seed phrase with checksum | 12/15/18/21/24 words | 128–256 bits |
| Hex key | random bytes, uppercase hex | 8–64 bytes | 8 bits/byte |
| Base64 key | random bytes, RFC 4648 base64 | 8–64 bytes | 8 bits/byte |
| WPA key | 76 chars (letters, digits, 14 symbols) | 8–63 | 6.24 bits/char |

- **Entropy shown honestly.** The figure is the base-2 log of how many equally likely values the generator can produce, rounded down to 0.1 bit. Passwords are drawn uniformly and redrawn until they contain every class, so the count is exact (inclusion–exclusion), not a per-character estimate. Strength labels: under 40 bits WEAK, under 64 MODERATE, under 80 GOOD, under 112 STRONG, otherwise EXCELLENT.
- **Unbiased.** Every character, digit and word is picked with rejection sampling (no modulo bias).
- **Fails closed.** If the TRNG returns an error the app shows "TRNG error: nothing was generated" and displays no value. A source that looks stuck (endless rejections) is treated the same way. The configure screen also shows the TRNG server's logged health-test error count.
- **BIP-39** output is checked against the reference test vectors and the vector in xous-core's own `gam` BIP-39 code.
- **Save to vault** with an optional label. Saving the same value twice is refused ("Already saved"). The label screen shows which PDDB basis new keys go to, and the confirmation says which basis it landed in.
- **Saved keys** list shows label, type and entropy (never the value). Enter shows the full value; D deletes it after confirmation.
- **Secrets leave the screen** when another app comes to the front, and after 2 minutes without a key press.

## Keys

| Screen | Key | Action |
|---|---|---|
| Type list | ↑ ↓ | Move |
| | Enter | Configure the selected type |
| | S or F2 | Saved keys |
| | Backspace / F4 | Leave the app |
| Configure | ← → (or ↓ ↑) | Shorter / longer |
| | Enter or Space | Generate |
| | Backspace / F4 | Back |
| Result | R or Space | Generate again |
| | S or F2 | Save (asks for a label) |
| | Backspace / F4 | Back (value is wiped) |
| Label | type | Label, up to 24 chars |
| | Enter | Save |
| | Backspace on empty / F4 | Cancel |
| Saved list | ↑ ↓, Enter | Move, view |
| | D or F3 | Delete (asks Y to confirm) |
| | Backspace / F4 | Back |
| View | D or F3 | Delete |
| | Enter / Backspace / F4 | Back |

The menu key `∴` opens the system menu; the app never sees it.

## Storage

PDDB dictionary `keygen.vault`, one key per saved value named `k.<16 random hex>`, holding JSON `{"version":1, "seq", "label", "kind", "length", "bits_tenths", "value"}`.

- New keys go to the most recently unlocked basis; an existing key is only ever rewritten in its own basis.
- When a basis is locked or unlocked the list is reloaded, so keys from a locked basis disappear from memory and screen.
- A record that can't be parsed, or that a newer version wrote, is counted as "unreadable" and left untouched.
- Delete removes that PDDB key and syncs. The PDDB does not overwrite the old flash pages immediately, so treat deletion as "gone from the app", not as secure erasure.
- Values are protected by PDDB encryption only. Anyone who can unlock the basis can read them.

## Limitations

- Randomness comes from the Precursor TRNG server (avalanche and ring-oscillator noise, conditioned in the server). The app checks for IPC errors and shows the server's health-test error count, but it can't independently test the noise source.
- Secrets are wiped from the app's own buffers on drop where practical (`zeroize`). Copies made by GAM when drawing text, and transient buffers inside `serde_json` and the PDDB, are outside the app's control.
- The 0.1.0 key `generated` (a JSON array) is not migrated: that version never compiled, so no device has such data.
- Renode capture not yet run. `capture.py` is written but not executed; verification so far is the hosted build, clippy, lint and 42 host tests.

## Build and test

```bash
cd core && cargo test                                   # host tests
cd xous-dev-toolkit && bin/integrate --xous-root ~/xous-core --grade ../precursor-keygen
cd ~/xous-core && cargo xtask run keygen                 # hosted mode
```

Layout: `core/` (`keygen-core`: generators, entropy, BIP-39, record format, screen state machine; no Xous deps), `src/main.rs` (message routing), `src/app.rs` (drawing, PDDB, TRNG on `precursor-appkit`).

---

Made by Tyler Colby, [Colby's Data Movers, LLC](https://colbysdatamovers.com). Issues: [GitHub](https://github.com/tbcolby/precursor-keygen/issues).

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
