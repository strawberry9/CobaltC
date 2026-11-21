# CHG-0455 — Errors in a statement's blocks (D-0323 amended)

Status: implemented (2026-11-14), D-0323, D-0296; Update 190
Affects: `impl/src/typecheck.rs` (`Body::recover_nested`, `check_block`, `check_nested_recovering`,
`is_expansion`, `check_block_stmts_recovering`, `check_closure` and `check_closure_body`); their twins in
`impl/selfhost/front/typecheck.cb` (`RecoveredBlock`, `check_nested_recovering`, `is_expansion`,
`check_closure_body`); `spec/decisions/D-0323-several-errors-in-a-body.md` (clauses 5 to 7, the measure);
`spec/registry/diagnostics.md` (1.70.0); `impl/conformance/22-surface-syntax/several_errors_in_nested_blocks_rejected.cb`
(new); `spec/conformance.md` (3.313.0)

## Why

The plan's stage B of item 1 (`private/EASE-PLAN-2-2026-11-14.md` §3), conditional on the measure finding no
follow-on. Update 189 built a first form, which recovered a statement's blocks as the body's own block is
recovered, measured it (20 single injections gave a false second error: the hidden locals of a compound
assignment's expansion, taken for the program's, and states a loop body's fixed-point walk carried round from
an undone statement) and left it out.

## What changes

While a statement in statement position that holds blocks is checked on a failed body's second walk
(`recover_nested`; not in a closure's body, nor for a block in value position), its blocks are checked a
statement at a time. A failed statement is undone and recorded and ends its block: the rest is not checked and
the point is unreachable, so the undone state reaches no join (the end of an `if`, a loop's next round). A
block whose first statement declares a hidden `$` name, a compound assignment's expansion, is one statement.
The body's statement that holds a block with an error is then a failed statement itself: its state undone,
its locals poisoned by D-0323's clause 3, its blocks' errors kept.

## Checked

`impl/tools/error_mutants.py` on the 2,550 mutants: no follow-on; 1,399 of the 1,688 with two or three
injections report every injection's errors (1,348 without the blocks), 118 one or none; one injection alone
gives two errors, both true (it renamed a loop variable's declaration, so the name is undeclared at two uses).
cbc's reports equal coby's on 731 rejected programs; the cold drafts and the foreign forms report as before.
