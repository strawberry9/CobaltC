# CHG-0399 — A pattern binder through a `Box`: `V(*x)` (D-0301)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-01)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0301, D-0045, D-0046, D-0109, D-0164, D-0224, D-0298
Affects: `spec/16-aggregates.md` §4 (1.33.0: `pat`, `[Match-Through-Box]`, `[Pattern-Type]`),
`spec/22-surface-syntax.md` §2 (2.57.0: `pattern`), `spec/conformance.md` (3.288.0); `impl/src/parser.rs`
(`parse_pattern`, `StructPat::through_box`, `pattern_destructure`), `impl/src/typecheck.rs` (`check_call`,
`Rewrite::BoxGetMut`, `Rewrite::BoxIntoInner`), `impl/src/views.rs` (the rename), `impl/src/diagnostics.rs`
(`unhide_binders`), `try_condition_pattern` in both parsers; `impl/selfhost/front/parser.cb`, `impl/selfhost/front/typecheck.cb` (`box_through_callee`,
`rw_box_call`, `unhide_binders`), `impl/selfhost/cbc/diags.cb` and `front/check.cb` (which call it), `impl/src/check.rs`; the guide (`content_b.py`, `content_c.py`);
`impl/conformance/16-aggregates/match_through_box_*.cb` (new)

## What changed

1. **The pattern `V(*x)`.** Where a variant's payload is a `Box<τ>`, `*x` binds `x` through the box: a
   `ref<τ, m>` when the match is through a reference of mode `m`, the `τ` itself when it is by value (the box
   consumed). `*x` stands only as a payload's whole pattern; at the top of a pattern, or followed by anything
   but `)`, it is a syntax error.
2. **As the arm it stands for.** The parser writes the arm as `V($box<n>_x)`, a hidden binder, beginning
   `auto x = Box::get($box<n>_x);`; the checker makes the call `Box::get_mut` or `Box::into_inner` by the hidden
   binder's type and records the rename as a rewrite (D-0164), so the program is checked again in its
   written form and `coby`, `cobc`, the model and the certificate see nothing new. A payload that is not a
   `Box` is `diag.type-mismatch`.
3. **Diagnostics name `x`.** Both renderers print the hidden binder as the name the program wrote.
4. **Five conformance cases**: `conf.match-through-box`, `conf.match-through-box-not-box-rejected`,
   `conf.match-through-box-binder-borrows-rejected`, `conf.match-through-box-top-level-rejected`,
   `conf.match-through-box-out-of-field-rejected`.

## Verification

- The C `cobc` writes for an arm with `*x` is the C of the same arm written by hand with `auto x = Box::get(b);`
  (`get_mut`, `into_inner`), the names of the binders aside.
- A pattern parsed and given up on in an `if` condition (`if (f(*p) == x)`) no longer advances the parser's
  hidden-name counter: `try_condition_pattern` restores it, in both parsers, so the counter after the prelude
  is the 55 `cbc` is built with.
- Under `coby`, and `cobc --run` with GCC and with Clang: every new case gives its stated outcome, the same
  output and the same diagnostic. `cargo test --release --no-fail-fast -p coby -p cobc`: every target passes
  but one row of `compiled_spec_rows`, `conf.exclusive-ref-lent-twice-dynamic` under Clang, which reported the
  fault at the thread's write instead of the second `spawn` while the machine was loaded. Its thread writes
  before it waits, so the place reported depends on the schedule; the case passed in the run before and 40
  times in 40 alone, and touches neither rule. Not repaired here.
- `impl/selfhost` rebuilt with `build.sh`: `compare_c.sh` gives the same C from `cbc` and `cobc` on every new
  program (11 same, 0 DIFF), and `coby --dump-checked`, `cbparse --dump-checked` and both `--emit-c` drivers print
  the same diagnostic for every new rejection; `coby --prelude-summary` is unchanged.
- The guide (`gen.py --check`: 412 examples, 0 failures under `coby` and `cobc` with GCC and Clang), the book
  (`check.sh`: 72 examples, 0 problems), `cobfmt --check` over every formatted tree, `speccheck.py` (0 errors),
  `proofcover.py --check`, `lake build` (no `sorry`), the model replay over the sample (315 programs, 0 refused,
  64 gaps, as before).
