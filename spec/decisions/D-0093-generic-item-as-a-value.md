# D-0093 — A generic item as a value needs its type arguments fixed

Status: ACCEPTED (2026-09-29, owner: "accept both gap recommendations, D-0093 and D-0094")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §20
Depends on: D-0010, D-0012, D-0013, D-0089
Affects: `spec/12` §5 (`rule.type.typing` `[T-Item]`), the shared checker, conformance

## Problem

`[T-Item]` types a *concrete* function item as a value of its
`fn(...) : τ` type. For a generic item, no rule gave the value a type
at all, and the two implementations answered differently from the same
text — the exact incompleteness test of the Master Instructions §20,
failed in the wild:

    fn id<T>(T x) : T { x }
    auto f = id;        // coby: ran f polymorphically (f(3), then f(true));
                        // cobc: exit 3, "internal error"

The positions with evidence already agreed and worked in both:
`auto g = id<i64>;` (explicit arguments) and `fn(i32) : i32 f = id;`
(the expected fn type) — the conformance case
`generic_fn_item_as_value_ok.cb` pins both.

## Candidate mechanisms

1. **Bless what coby happened to do**: a polymorphic value binding.
   Rank-1 polymorphic locals are a large type-system commitment
   (every use site instantiates; cobc must monomorphize per use) for
   no demonstrated capability. Rejected.
2. **Leave it undocumented.** Keeps the §20 failure. Rejected.
3. **Make the rejection explicit**: a generic item as a value is
   instantiated by explicit type arguments or by the expected
   `fn(...)` type; with neither at the binding, it is ill-formed —
   `diag.cannot-infer-type-parameter`, exactly as `auto v =
   Vec::new();` is `[Generic-Call-Uninferable]`. In an argument
   position the call's own rules (`rule.fn.generic-call`,
   `[T-Spawn]`) instantiate it, so nothing is taken away there. No
   new tokens, no new capability lost. Selected.

## Selected design

- `[T-Item-Generic]`: a generic `path` as a value has the type of its
  instantiation when explicit type arguments (`path<σ1..σn>`, one per
  parameter) or the expected `fn(...)` type fix every `Ti`; the
  instantiation must agree with the expected type exactly.
- `[T-Item-Value-Uninferable]` (disposition: rejected): binding a
  generic `path` with neither — `auto f = id;` —
  is ill-formed, `diag.cannot-infer-type-parameter`.
- An argument position is not a binding: the call, spawn included,
  instantiates its callee-shaped arguments by its own rules, as
  today.

## Compatibility impact

Programs that bound a generic item with nothing to fix it — accepted
and run polymorphically by coby only — are now rejected. No suite,
showcase, guide or stress program did this; it was found by a
workstream-3 probe.

## Implementation notes (informative)

One fix in the shared front end serves both tools. cobc's residual
lowering limit — a generic item as an argument whose instantiation
cobc's inference cannot yet solve from the call (`twice(id, 10)`
where `twice<T>(fn(T) : T f, T x)`) — remains exit 3 `unsupported`
(no longer mislabeled "internal error"); coby runs it. That is an
implementation limit under this decision's carve-out, not a semantic
divergence. (Closed 2026-10-02: cobc lowers such an item after the
other arguments, as it does a bare `None`, and compiles the call;
`impl/conformance/12-type-system/generic_item_value_inferred_from_call_ok.cb`.)

## Revisit conditions

If a real program wants a polymorphic fn value (mechanism 1), that is
a new capability proposal, not a reopening of this rejection.
