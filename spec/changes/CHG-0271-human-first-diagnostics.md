# CHG-0271 — Diagnostics short by default, located to the column

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14; under D-0228)
Governed by: `CobaltC_Master_Instructions.md` §19, §20
Depends on: D-0228
Affects: `spec/registry/diagnostics.md` (1.53.0); `impl/src/diagnostics.rs` (shared by both tools),
`impl/src/{ast,parser,lexer,typecheck,lib,main,check,modres,views,derive,consts,each}.rs`,
`impl/cbrt/src/lib.rs`, `impl/cobc/src/{main,lower}.rs`, `impl/model/src/compare.rs` (the model's
comparison reads the new form); tests; the guide's tour

## What changed

- **The form.** By default, `error[diag.x]` or `fault[diag.x]` with the message, then `--> file:line:col`,
  the source line with carets, and `= repair:`. `--explain` (both tools) or `COBALTC_EXPLAIN=1` adds the
  registry's entry. Colour is as before (`NO_COLOR` respected), and now covers the location, the gutter
  and the carets.
- **The registry.**
  - Every entry has a **Summary**, the message when an occurrence gives none: 69 entries.
  - **Location** is now a field the renderer separates out. Before, its text ran into the Repair shown.
  - The syntax error's Repair no longer mentions a column in the message.
  - "How a diagnostic is shown" describes the forms.
- **Columns.**
  - The AST carries `Expr::col` and `Stmt::Let`'s `col`, the column of the first token.
  - Static diagnostics carry `@line:col`.
  - Dynamic ones are located by line, with the caret under the line's first token (D-0228 says why). So
    the generated C and the runtime are unchanged.
  - Columns count characters; a tab stays a tab in the caret's padding.
- **A non-ASCII character outside a literal** is now named whole (`'é'`, not its first byte `'Ã'`), with
  why it is not allowed.
- **The runners.** They read the id and the phase from the first line through
  `coby::diagnostics::first_line_id`: `tests/cb_conformance_suite.rs`, `cobc/tests/compiled_suite.rs`,
  `compiled_spec_rows.rs`, `differential.rs`, `output_order.rs`.
- **`thread_is_finished_ok.cb`** reads a failure's second line, now `--> FILE:LINE:COL`.
- **Tests.** `impl/tests/diagnostic_form.rs` covers a static error, a fault, a syntax error, a fault
  inside `std`, a file-backed module, tabs and wide characters, a non-ASCII character, and `--explain`. The
  renderer's unit tests cover the summary of every entry, the caret, `first_line_id` and the colour.

## Checked

`coby`'s and `cobc --run`'s stderr were compared over 651 conformance cases. All were identical, apart from
the one case that links C (which `coby` refuses) and two cases too slow for that comparison's 120-second
limit. The compiled suite compares all of them.

## What changed for programs

Nothing in their meaning. A program that parses a diagnostic's text (`ThreadFailure::text`) sees the new
form. The id (`ThreadFailure::diagnostic`) is unchanged.
