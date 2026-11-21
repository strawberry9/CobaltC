# CHG-0273 — Documentation comments; no project file

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14; under D-0229 and D-0231)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0229, D-0231
Affects: `spec/22` §1 (2.49.0); `impl/tools/cobdoc.py` (new); `impl/cobfmt/tests/cli.rs`; the guide's
"Running a program"

## What changed

- **D-0229.**
  - `spec/22` §1 says that `///` before an item is its documentation. This is non-normative, and the
    grammar is unchanged.
  - `impl/tools/cobdoc.py` lists a program's or `std`'s exported items as JSON: module, kind, signature,
    documentation, summary, file, line and examples. `--missing` lists the undocumented items: all 670 of
    `std`'s today, which WI-18 will fill.
  - A cobfmt test pins that a `///` block stays directly before its item.
- **D-0231.** The guide's "Running a program" says that a program's files and C code are named in its
  source (`module`, `extern`) and that `cobc` is the build.

## What changed for programs

Nothing.
