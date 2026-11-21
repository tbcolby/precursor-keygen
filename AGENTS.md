# Key Ceremony — agent notes

Rebuilt 2026-11-20 on `precursor-appkit` (strategy R from the portfolio
remediation plan). Read `CLAUDE.md` for layout. Rules that matter here:

- Never produce a value from a failed or stuck entropy source; core
  generators return `Err(EntropyError)` and tests cover every kind.
- Displayed entropy must be exact (rounded down), not a per-char estimate.
- No secret in logs or `Debug`; keep secret buffers in `Secret`/`Zeroizing`.
- Every screen exits on Backspace or F4; `∴` is never delivered to apps.
