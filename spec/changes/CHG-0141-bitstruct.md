# CHG-0141 — `bitstruct`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; D-0118)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0118
Affects: `spec/16` (1.19.0), `spec/22` (2.41.0), `spec/registry/diagnostics.md` (1.41.0), `spec/conformance.md` (3.114.0), the guide §16 and §22, `impl/src/{lexer,parser,ast,derive,typecheck,interp}.rs`, `impl/cobc/src/lower.rs`

## What changed

- **`spec/22`:** `bitstruct` keyword; `bitstruct-decl` and `bitfield`.
- **`spec/16` §2a:** `type.bitstruct`, `[Repr-Bitstruct]`,
  `[Bitstruct-Construct]`, `[Bitfield-Read]`, `[Bitfield-Write]`,
  `[Bitfield-Overflow]`, `[Bitfield-No-Place]`, `[Bitstruct-Whole]`,
  `[Bitstruct-Bits]`, `[Bitstruct-From-Bits]`.
- **Registry:** the three diagnostics the rules reuse.
- **Front end:** the lexer's keyword; `parse_bitstruct` (widths checked
  to fill the backing type); `StructDecl.bits`, `FieldDecl.width`;
  `derive.rs` adds `Name::bits` and `Name::from_bits`; the checker
  refutes literal overflows, rejects a borrow of a field and a
  destructuring.
- **`coby`:** layout = the backing integer; byte image and key bytes
  packed; raw-memory field reads and writes through the whole; dynamic
  `[Bitfield-Overflow]` at writes and literals.
- **`cobc`:** `struct { uN bits; }`; field reads as shift-and-mask,
  writes as read-modify-write with the overflow check, literals packed;
  equality and key bytes on the integer; no tracked fields.
- **Rows:** `conf.bitstruct-fields`, `conf.bitstruct-bits-roundtrip`,
  `conf.bitstruct-raw-image`, `conf.bitstruct-through-reference`,
  `conf.bitstruct-key`, `conf.bitstruct-literal-overflow-rejected`,
  `conf.bitstruct-write-overflow-faults`, `conf.bitstruct-field-borrow-rejected`,
  `conf.bitstruct-fill-rejected`, `conf.bitstruct-destructure-rejected`.
- **Guide:** §16's bitstruct section and §22's grammar reference.

## Compatibility classification

Additive: every program accepted before is accepted with the same
meaning, save one naming an item `bitstruct` (none did).
