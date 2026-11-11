# CHG-0206 — Standard output is buffered; `flush_stdout`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-06; D-0178)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0178
Affects: `spec/21` (4.42.0) §0, §2a, §2f; `spec/conformance.md` (3.170.0); the guide; `impl/src/outbuf.rs`, `impl/cbrt`, `impl/std/`

## What changed

- **`spec/21` §2a:** `[Print-Buffer]`: when the bytes written to standard output reach the operating system at the latest.
- **`spec/21` §0:** `flush_stdout` in the submodule table and the prelude table.
- **`spec/21` §2f:** `eprintf` writes standard output's buffer out first.
- **Rows:** `conf.stdout-buffer-order`, `conf.flush-stdout`, `conf.stdout-buffer-boundary`, `conf.stdout-buffer-child`, `conf.stdout-flush-private`.

## Compatibility classification

Additive: the same bytes in the same order relative to standard error and standard input; only when output reaches a pipe or a file changes.
