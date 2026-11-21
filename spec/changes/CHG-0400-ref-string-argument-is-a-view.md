# CHG-0400 — `&s` where a `StringView` is declared is the view of `s` (D-0302)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-01)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0302, D-0006, D-0134, D-0135, D-0164, D-0225
Affects: `spec/21-standard-library-semantics.md` §2h (4.73.0: `[Ref-String-View]`, `[Str-Value-View]`; §2a's
note and the naming conventions), `spec/12-type-system.md` (1.26.0: `rule.type.expected`),
`spec/22-surface-syntax.md` §5 (2.57.0), `spec/conformance.md` (3.288.0); `impl/src/typecheck.rs`
(`view_of_text`, the checks of a call's arguments, a local's initializer, a struct literal's field and a
variant's payload, `conversion_hint`), `impl/src/views.rs` (the rewrites); `impl/selfhost/front/typecheck.cb`
(the same); the guide (`content_a.py`, `content_c.py`); `impl/conformance/21-standard-library-semantics/
ref_string_view_*.cb` (new), `view_of_str_binding_ok.cb` and `str_binding_view_in_variant_ok.cb` (were
`…_rejected.cb`)

## What changed

1. **`&s` is a view where one is declared.** In the positions of `[Str-Literal-View]`, an expression `&e` of
   type `ref<String, shared>` is `&e[0..$]`, a binding or field `r` of that type is `&(*r)[0..$]`, and a value
   of type `str` is `StringView::of(t)`. The checker records the rewrite and checks the program again in the
   written form, so every rule (the borrow and its clashes, a temporary's end, a view's end at its last use,
   the tiers) and both tools see that form.
2. **What is not admitted** is unchanged: `&mut s` (now told to pass `&s`), a `String` by value, a parameter
   whose type is a type parameter fixed as `StringView`, a function value's parameter.
3. **Conformance.** New: `conf.ref-string-view`, `conf.str-value-view`, `conf.ref-string-view-mut-rejected`,
   `conf.ref-string-view-by-value-rejected`, `conf.ref-string-view-temporary-kept-rejected`,
   `conf.ref-string-view-borrow-clash-rejected`, `conf.ref-string-view-type-parameter-rejected`. Admitted now,
   and positive: `conf.view-of-str-binding` (`StringView v = t;`, the file renamed `view_of_str_binding_ok.cb`),
   `conf.str-binding-view-in-variant` (was `conf.str-binding-not-view-in-variant`; `Some(t)` for an
   `Option<StringView>`), and the inline row `conf.str-value-view-local` (was `conf.view-of-str-binding-rejected`).

## Verification

- Found while testing: the checker kept its rewrites (D-0162, D-0164) in one table for the process, so two
  programs checked at once on two threads (the test harnesses) could take each other's; the tables are per
  thread now. And `&e[0..$]` on a generic call's `String` result (`&Option::unwrap_or(o, d)[0..$]`) was a type
  mismatch, because the view form asked for the base's type without binding the call's type parameters; it
  asks `value_shape` too now (`view_base`, both checkers), so the form D-0302 writes for `&Option::unwrap_or(…)`
  is a view.
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
