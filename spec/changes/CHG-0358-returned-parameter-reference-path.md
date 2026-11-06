# CHG-0358 — A reference parameter passed on to a function that returns it keeps its path

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0018, D-0060, D-0188, D-0205
Affects: `spec/conformance.md` (one row, 3.261.0); `impl/cobc/src/lower/user_calls.rs` (`lower_user_call`);
`impl/cobc/src/lower/layout.rs` (`may_carry_ref_to`)

## What changed

No rule changed. `cobc` passes a reference binding given whole as an argument (`f(r)`) with the binding's
own path, minting none. When the callee returns that reference itself (`fn same(ref<E, shared> e) :
ref<E, shared> { e }`), the runtime hands the returned path to the calling statement, which owns its
result and ends it when it ends. The path was the binding's own, so it ended too, and the next use of the
binding faulted with `diag.stale-binding` though nothing had invalidated it: `peek(same(base))` followed by
`peek(base)`, `base` a parameter. `coby` ran it. Now a reference binding `ref<T, m>` passed whole to a
function whose result can hold a `ref<T, _>` (or a reference to a type parameter still open, or a guard)
is passed with a path derived for the call, and the returned reference is that path; the binding's own is
untouched. A result that can hold no such reference cannot carry the binding's own path back (a field, an
element or a slice of the referent is a path of its own), so such calls are passed as before: a first
form of this change derived a path for every result holding any reference, and `String::view(s, …)`, whose
`StringView` holds a slice, then cost one more borrow per call (perfguide e04 b +44%, e08 b +39%
instructions). The new row `conf.returned-parameter-reference-passed-on` covers a
shared and an exclusive parameter.

## What changed for programs

A program `cobc` faulted on with that `diag.stale-binding` runs as under `coby`.

## Found by

The type checker written in CobaltC, compiled with `cobc`: its `slice_source` passes
`strip_parens_tc(base)` (which returns `base` itself when there are no parentheses) to `single_tracked`,
then uses `base`; every `&v[0 .. $]` checked faulted; 2026-10-20.
