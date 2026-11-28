# CHG-0152 — `Vec::reserve` and `String::reserve`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-01; D-0129)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0129
Affects: `spec/21` (3.51.0), `spec/conformance.md` (3.120.0), the guide §21, `impl/src/prelude.rs`

## What changed

- **`spec/21` §1 `rule.stdlib.vec`:** `Vec::reserve` in the listing, and a paragraph on it: room for `n` more (`cap >= len + n` afterwards), growing at most once to the larger of `len + n` and twice the capacity; `diag.arith-overflow` when `len + n` overflows, before anything is allocated; the elements move as `grow` moves them; `std`'s builders reserve.
- **`spec/21` §2 `rule.stdlib.string`:** `String::reserve` (`Vec::reserve` on the bytes) in the listing; `String::from_str` reserves its length before filling. Inside `std`, `String::clone` (through its private `append_string`) and `Vec::from_slice` reserve too; their results are unchanged.
- **`std` (`impl/src/prelude.rs`):** the same, written in CobaltC; both tools run them as written.
- **Rows:** `conf.vec-reserve`, `conf.vec-reserve-keeps`, `conf.vec-reserve-overflow`, `conf.vec-reserve-stale`, `conf.string-reserve`.

## Compatibility classification

Additive: two new names in `std`, shadowed by a program's own; the
builders' results are unchanged.
