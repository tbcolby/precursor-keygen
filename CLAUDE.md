# Key Ceremony — Build Notes

## Architecture
- 4-state machine: TypeSelect, Configure, Result, Saved
- 6 generator types with configurable length
- Entropy estimation using integer math (no float)
- ~630-word dictionary for Diceware passphrases

## Key Patterns
**Generator dispatch** — generate() dispatches to type-specific functions
**Entropy without float** — approximate bits/unit as integers (6 bits/char for passwords, etc.)
**Guaranteed diversity** — passwords ensure at least one of each char class, then shuffle
**Diceware dictionary** — compact 4-letter word list embedded in binary

## Build
```bash
cargo build -p keygen --target riscv32imac-unknown-xous-elf
```
