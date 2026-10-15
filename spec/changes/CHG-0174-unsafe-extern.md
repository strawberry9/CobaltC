# CHG-0174 — `unsafe extern fn`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0146)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0146
Affects: `spec/22` (2.46.0) §3; `spec/20` (1.12.0) §3; `spec/21` listings; `spec/conformance.md` (3.136.0); `spec/examples.md`; the guide; `impl/src/parser.rs`; `impl/std/std.cb`, `io.cb`, `sys.cb`; conformance cases; `impl/tests/conformance.rs`, `impl/cobc/tests/extern_code.rs`; showcase Tier 4

## What changed

- **`spec/22` §3:** `extern-decl ::= vis 'unsafe' 'extern' 'fn' …`; plain `extern fn`, and `unsafe` before `extern "…";`, are `diag.syntax-error`.
- **`spec/20` §3:** `[Extern-Decl]` (`trusted-unchecked`): the signature's claim, stated at the declaration; the prose says why.
- **`spec/21`, `spec/examples.md`, `spec/conformance.md`:** every declaration of a foreign function written `unsafe extern fn`.
- **Both tools (`impl/src/parser.rs`, shared):** the new form parsed, the old one refused with the repair.
- **`std`, cases, tests, guide, showcases:** every `extern fn` declaration written `unsafe extern fn`.
- **Rows:** `conf.extern-without-unsafe-rejected`, `conf.unsafe-extern-code-rejected`.

## Compatibility classification

Breaking (source): a declaration without `unsafe` is rejected; meaning unchanged.
