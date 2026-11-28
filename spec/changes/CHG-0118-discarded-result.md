# CHG-0118 — A discarded `Result` is an error; `_ = e;`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30)
Governed by: D-0099
Affects: spec/13 §3, spec/22 `statement`, `spec/registry/diagnostics.md` (1.39.0)

## What changed

- **spec/13:** `[Stmt-Result-Discarded]` and `[Discard]`.
- **spec/22:** `statement ::= … | '_' '=' expr ';'`.
- **Registry:** `diag.result-discarded` (static).
- **Implementations:** the shared parser desugars `_ = e;`; the shared
  checker rejects the discarded `Result`.
- **Programs:** every repository site that discarded a `Result` now
  handles it or writes `_ =`.
- **Rows:** `conf.result-discarded-rejected`,
  `conf.result-discarded-void-rejected`, `conf.discard-underscore`,
  `conf.discard-underscore-destroys-now`.

## Compatibility classification

Breaking for programs that discarded a `Result`; mechanical to fix.
