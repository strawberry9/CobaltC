# CHG-0158 — Names that follow `std`'s conventions

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0134)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0134
Affects: `spec/21` §0, §1, §2b, §2d, §2g, §3; `spec/20` (the `[Extern-Call]` example); `spec/examples.md` (`ex.extern-write`); `spec/conformance.md`; the guide; `impl/src/prelude.rs`, `impl/src/modres.rs`, `impl/src/typecheck.rs`, `impl/src/interp.rs`, `impl/src/each.rs`, `impl/cobc/src/lower.rs`

## What changed

| Before | After |
|---|---|
| `Rng::seed(u64)` | `Rng::new(u64)` |
| `map_err(r, f)` | `Result::map_err(r, f)` |
| `parse<T>(&s)` | `String::parse<T>(&s)` |
| `HashSet::at(&s, i)` | `HashSet::key_at(&s, i)` |
| `extern fn write` / `read` (exported) | `stdout_write` / `stdin_read` |
| `extern fn write_err` (private) | `stderr_write` |

Meanings are unchanged.

- **Rows:** `conf.write-err-private` is now `conf.stderr-write-private`; every row naming an old name rewritten.

## Compatibility classification

Breaking (source): an old name is `diag.unbound-name`. No behaviour changes.
