# CHG-0435 — A function's `requires` block (D-0317)

Status: implemented (2026-11-11), D-0317, D-0296; Update 180
Affects: `spec/22` §3, `spec/15` §2b (`rule.fn.requires`), `spec/14` §6 (the requirement row),
`spec/registry/diagnostics.md` (`diag.precondition-failed`, `diag.requires-misplaced`), `spec/registry/features.md`
(`feat.requires-block`), `spec/conformance.md` (19 rows, one changed); `impl/src` (`ast.rs`, `parser.rs`, `astdump.rs`,
`modres.rs`, `typecheck.rs`, `interp/calls.rs`, `interp/exec.rs`, `interp/intrinsics_runtime.rs`,
`interp/intrinsics_threads.rs`); `impl/cobc/src/lower` (`intrinsics.rs`, `functions.rs`, `user_calls.rs`,
`frames.rs`); their twins in `impl/selfhost/front` and `impl/selfhost/back`; both formatters; `impl/tools/cobdoc.py`;
`impl/std` (24 functions).

## What changes

**The block.** `fn f(…) : τ requires { cond, diag, "message", args; … } { … }`: one optional block between the
signature (and its `from` clause) and the body, each requirement a condition, optionally the run-time diagnostic it
is and a message. `requires` is a contextual word.

**How it is built.** Both front ends' `modres` write each requirement as the body's first statements, in order:
`if (!(c)) { $require_fail(d, message) }`, `d` `precondition_failed` where none is named. Everything after
`modres` (the checkers, the flow and decided-program analyses, both back ends' dual bodies and variants) sees
ordinary code. `$require_fail` is the one new intrinsic: a fault located at the call.

**Where the fault is reported.** `coby`: the fault leaves the callee marked as located at a line no program has,
and each of the three call paths (`call_user_fn_expected`, `invoke_callable`, `run_body`) replaces the mark with
the call's line as the callee returns (a prelude line drops it, so a callback `std` made takes the program's line,
as any fault in `std` does); a self tail call's iteration takes the tail call's line, and a spawned body the
`spawn`'s. `cobc` and `cbc`: a caller stores its position before calling a function with a block (as it does
before a `std` call), the body reads it on entry (`cb_where` into `cb_rq_f`, `cb_rq_l`) and the fault names it;
a self tail call sets it to its own line before the jump. In `std`'s code, which stores no position, the fault
uses `CB_NOLOC`, as `std`'s `fault` calls did.

**The static row.** At a call that names the function by its path, foldable requirements over arguments of
known value are evaluated in order, each step checked against its parameter's type; `false` rejects the call.

**`std`.** The 24 functions whose body began with `if (c) { fault(d); }` or an `assert` on their parameters say
it as a requirement naming the same diagnostic (`assert_failed` for the three `assert`s, with their messages): no
outcome changes, except that `Barrier::new(0)`, `Channel::new(0)`, `String::push_ascii(s, 200)` and
`StringView::parse_radix(v, 1)` written with literals are now rejected before the run. One conformance case
(`conf.barrier-zero-parties`) makes its 0 at run time; `conf.requires-std-literal` is the literal one.

**Tools.** Both formatters lay a block out on lines of its own, one level under the signature, the body's `{`
back at it; `cobdoc.py`'s records carry `requires` (each requirement's text, diagnostic and message), and a
signature stops before the block. `coby --prelude-summary` keeps a `std` function's block in its declaration.

## Measured

The 19 conformance cases give the same output, byte for byte, under `coby` and under `cobc` with GCC and Clang
(locations included). A converted `std` function's check is the `if` its body began with, its condition written
the other way round (`!(i < v.len)` for `i >= v.len`), and it faults with no location as before: the gate's
bench and perf-guide counts compare each program's instructions with Update 179's.
