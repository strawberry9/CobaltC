# D-0130 — `?` on an `Option` in a function returning an `Option`

Status: ACCEPTED (2026-10-01, the owner chose to adopt the behaviour both implementations already had, over making them reject it)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §17
Depends on: D-0006 (`?` and the error type), D-0060 (a temporary operand ends at the `?`), D-0114 (`Option::ok_or`)
Revisits: D-0114, candidate 2
Affects: `spec/18` §2 `rule.fail.propagate` (1.5.0), `spec/registry/diagnostics.md`, `spec/conformance.md` (3.120.0), the guide §21, `impl/src/typecheck.rs`

## Problem

`spec/18`'s `[Propagate]` gives `e?` a meaning only for a `Result`, and
D-0114 chose `Option::ok_or(o, e)?` over a second rule for `Option`.
Both implementations nevertheless accepted `?` on an `Option` inside a
function that returns an `Option`, returning `None` on `None` — and five
programs had come to rely on it, three of them showcases: checked
arithmetic (`checked_add`, `checked_mul` return an `Option`) and lookups
chained inside `Option`-returning helpers:

    fn Arena::alloc(ref<Arena, exclusive> a, usize n, usize align) : Option<rawptr<u8>>
    {
        usize start = (a.used + align - 1) / align * align;
        usize end = checked_add(start, n)?;
        …

The divergence was found while modernising the stress programs: the
implementations accepted a program the specification calls ill-typed.

## Candidate mechanisms

1. **Make the implementations follow the specification**: `?` on an
   `Option` rejected, the five programs rewritten with an `if`-pattern
   or a `match` returning `None`.
2. **Adopt the implementations' behaviour as a rule**: `?` on an
   `Option` inside a function returning an `Option` returns `None` on
   `None`. Selected.

D-0114 set candidate 2 aside as "a rule for a rare case" (8 sites then).
The case is not rare where it applies: every checked-arithmetic helper
and every chain of lookups in an `Option`-returning function is one,
and writing `if (Some(e) = checked_add(start, n)) { e } else { return None; }`
for each says less than `checked_add(start, n)?`.

## Decision

    [Propagate-Option]
        ⟨e, Σ⟩ →* ⟨o, Σ1⟩,  o : Option<τ>,  the enclosing function returns Option<τ'>
        ────────────────────────────────────────────
        ⟨propagate(e), Σ⟩ → ⟨match (o) { Some(v) : v, None : return None }, Σ1⟩

- **Only within an `Option`-returning function.** `?` on an `Option` in
  a function returning a `Result` (or anything else) is
  `diag.propagate-outside-fallible-context` (static), with a message
  pointing to `Option::ok_or(o, e)?`, which stays the bridge: it names
  the error to return. A `Result`'s `?` in an `Option`-returning
  function stays rejected as before (`[Propagate-Err-Mismatch]`). There
  is no implicit conversion between the two.
- **As `[Propagate]` otherwise**: the same desugaring through `match`,
  the same consumption of a resource-bearing payload, and a temporary
  operand ends at the `?` (D-0060).
- **A closure** follows its own declared or inferred result type, as
  `return` inside it does.

## Compatibility

Additive for the specification: programs it rejected are now accepted
with the meaning both implementations already gave them. No accepted
program changes meaning. The implementations change only in rejecting
`?` on an `Option` outside an `Option`-returning function (which they
had let through) with the diagnostic above.

## Revisit

If a refutable binding with an `else` block (D-0114 candidate 3) is ever
adopted, both forms stay: `?` for "or return `None`", the binding for
"or do this".
