# CHG-0291 — The model: `--as-licensed`, forgetting, take-over, `swap` and `replace`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0241)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0241, D-0236, D-0237, D-0235
Affects: `impl/model`, `impl/battery.sh`, `impl/tools/suites.sh`

## What changed

- `impl/model`: the static half records proven accesses and forgets (D-0236); `check --as-licensed` and
  `--pre-d0236`; the machine's `Ins::Licensed`, `model.proven-but-false`, `[Thread-Affine]`'s take-over making
  a thread lent (D-0237), `swap`/`replace` (D-0235); the helper `swap_first`; the alphabet; six sample
  programs; the README.
- Battery stage 1 and `suites.sh`: `cobmodel check --as-licensed` over the sample.

## What changed for programs

Nothing: evidence.
