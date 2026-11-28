# CHG-0450 — Within a body, an error for each statement that stands on its own (D-0323)

Status: implemented (2026-11-14), D-0323, D-0296; Update 189
Affects: `impl/src/typecheck.rs` (`Body` cloneable, its `recover`, `recovered`, `poisoned`, `skipped` and
`derived`; `Ck::run`'s `make_body` and second walk; `check_fn_body`; `check_block_stmts_recovering`,
`after_failed`, `stmt_declares`, `stmt_ends_block`), `impl/src/lib.rs` (`render_static`, `UNCHECKED_NOTE`,
`UNCHECKED_TEXT`); their twins `impl/selfhost/front/typecheck.cb` (`new_body`, `body_snapshot`,
`body_restore`, `check_block_stmts_recovering`, `after_failed`, `stmt_declares`, `stmt_ends_block`,
`mentions_poisoned`, `error_place_key`, `UNCHECKED_NOTE`), `impl/selfhost/cbc/diags.cb` (`render_static`);
`impl/tools/error_mutants.py` (new); `impl/selfhost/compare_reports.sh` (new); `impl/tests/cli.rs`;
`impl/conformance/22-surface-syntax/several_errors_in_a_body_first_rejected.cb` (new);
`spec/registry/diagnostics.md` (1.69.0); `spec/conformance.md` (3.312.0)

## What changes

A body that fails is checked a second time with a fresh state (an accepted program pays nothing). On that walk
the body's own block is checked a statement at a time: before each, the state is copied; a statement that fails
is recorded and the copy put back, and `after_failed` declares what it declared with a written type and
poisons what D-0323 §3 names; a statement mentioning a poisoned local is not checked. In the self-hosted
checker the copy leaves out the facts, the report and the fresh ids, which go on as coby's interior tables do.
The second walk's errors stand only when its first is the error the first walk found; each is keyed by its
function and place, so a generic body's statement gives one error over its instantiations. A body with a
statement not checked adds the note, which `render_static` prints after the count. The second walk's access report
is marked rejected, as the first walk's is, so `--certificate` still names the body `rejected` (the gate's first
run found the certificate of a rejected program calling it `decided`).

`impl/tools/error_mutants.py make OUT FILE…` writes mutants with one to three injected errors and each
injection alone; `run OUT` checks them and lists every diagnostic of a mutant that no injection alone gives (a
follow-on), every single injection that gives two errors, and how many mutants report every injection's
errors. `impl/selfhost/compare_reports.sh` holds cbc's report to coby's, byte for byte, on any set of files.

## Checked

Of the 1,688 mutants with two or three injections, none reports an error that its injections taken alone do not give, and no injection alone gives two errors; 1,348 report every injection's errors, 133 report one or none (the injections in one compound statement, which is one statement, or one caught in an earlier phase, which stops the bodies' phase), and 8 of the 2,550 were accepted (`true` where any value fitted). `cli.rs`: `check_reports_each_statement_s_error` (D-0321's test, inverted),
`check_reports_no_error_that_follows_from_another` (a typed declaration, `auto`, a value given on one path, a
failed `return`, eleven errors), `check_reports_a_generic_statement_once`.
