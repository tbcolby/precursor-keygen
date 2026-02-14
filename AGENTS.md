# Key Ceremony — Agent Evolution Report

## Agents Used
1. **ideation.md** — Generator type selection, entropy display
2. **architecture.md** — State machine, generator pattern
3. **encoding.md** — Hex and base64 encoding
4. **randomness.md** — TRNG wrapper, Fisher-Yates shuffle, rejection sampling
5. **storage.md** — Saved keys in PDDB
6. **build.md** — Standard Cargo.toml with TRNG dep
7. **review.md** — Standards compliance

## New Patterns
- **Diceware-style passphrase**: ~630 common 4-letter English words. TRNG picks N words, joined by hyphens. ~9.3 bits entropy per word.
- **Entropy estimation without float**: Approximate bits-per-unit as integers to avoid soft-float on RISC-V. Password: 6 bits/char, PIN: 3 bits/digit, etc.
- **Character class guarantee**: Password generator ensures at least one lowercase, uppercase, digit, and symbol, then shuffles all positions. Prevents "all lowercase" edge cases.
- **Base64 encoding**: Manual 3-byte-to-4-char encoding without external crate.

## Metrics
| Metric | Value |
|--------|-------|
| Source files | 6 |
| Estimated LOC | ~1,800 |
| States | 4 |
| Generator types | 6 |
| Word dictionary | ~630 words |
| Toolkit agents used | 7 of 12 |
