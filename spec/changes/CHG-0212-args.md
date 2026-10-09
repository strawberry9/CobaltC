# CHG-0212 — `Args`: command-line options

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-07; D-0184)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0184
Affects: `spec/21` (4.44.0) §0, §2c; `spec/conformance.md` (3.174.0); the guide; `impl/std/env.cb`

## What changed

- **`spec/21` §2c `rule.stdlib.args`:** `[Args-Flag]`, `[Args-Option]`, `[Args-Finish]`, `[Args-Help]`; the listing.
- **`spec/21` §0:** `Args` and its functions under `std::env`.
- **Rows:** `conf.args-flags-options`, `conf.args-finish`, `conf.args-help`.

## Compatibility classification

Additive.
