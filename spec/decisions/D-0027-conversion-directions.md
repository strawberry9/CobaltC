# D-0027 — Conversion Directions: `widen` Only Where Every Value Fits, `narrow` Between Any Integers

Status: ACCEPTED (2026-09-24, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §5, §9, §17
Depends on: D-0006, D-0026, rule.arith.convert
Affects: rule.arith.convert (`[Narrow-Checked]`, `[Narrow-Wrapping]`, `[T-Convert]`)

## Problem

`rule.arith.convert` gives each conversion a direction: `[Widen]` where
the source's domain is contained in the target's, `[Narrow-*]` where it
is not, `[Reinterpret-Sign]` between the two signednesses of one width.
A conversion outside its direction had no rule, and neither
implementation rejected it:

- `widen<u64>(-5)` faulted `diag.type-mismatch` at run time in `coby`,
  and in `cobc` returned 18446744073709551611, silently.
- `widen<i64>(n : usize)` was accepted, although a `usize` above
  `i64`'s maximum has no `i64` value.
- `narrow<usize>(x : u64)` had no rule on a 64-bit implementation
  (`u64` ⊆ `usize` there) but was a real narrowing on a 32-bit one.

Three of the repository's example programs used `widen` outside its
direction.

The last case shows that a strict reading of `[Narrow-*]` would make
portable code impossible: whether a conversion between `usize`/`isize`
and a fixed-width type is a widening depends on `AddrWidth`, which is
implementation-defined.

## Constraints

- `widen` never faults and never loses a value (its rule and the guide
  say so).
- A program should be well-formed on every implementation where
  possible (`AddrWidth` ∈ {16, 32, 64, 128}).
- No new tokens.

## Candidate mechanisms

1. **Enforce every premise as written.** `widen` only where contained,
   `narrow` only where not. Conversions involving `usize`/`isize` must
   then be spelled differently per implementation.
2. **Enforce `widen`'s and `reinterpret`'s premises; let `narrow` and
   `narrow_wrapping` take any two integer types.** **Selected.**
3. **Let `widen` take any pair and check at run time.** It would be
   `narrow` under another name, and `widen`'s promise would be gone.

## Selected design

Candidate 2:
- `[T-Convert]` checks `[Widen]`'s containment and `[Reinterpret-Sign]`'s
  width and signedness, statically, at each instantiation:
  `diag.type-mismatch`.
- `[Narrow-Checked]` and `[Narrow-Wrapping]` drop the non-containment
  premise. A `narrow` whose source always fits never faults.

A portable program converts `usize` to `u64` with `narrow`, which
cannot fault on an implementation where the two have the same width.
`widen<u64>(n : usize)` remains well-formed where `AddrWidth ≤ 64`,
and nowhere else.

## Rejected alternatives

- **1:** portable code could not convert between `usize` and a
  fixed-width type at all.
- **3:** `widen` exists to say "this cannot fail". Checking it at run
  time removes the reason to have it.
- **Portable containment for `widen`** (contained at every
  `AddrWidth`): `widen<usize>(x : u32)` would then be rejected
  everywhere, because of a 16-bit `AddrWidth` few programs target.

## Semantic rationale

Every accepted `widen` preserves its value, and every accepted
`reinterpret` preserves its bits. `narrow` keeps its run-time check
wherever a value might not fit.

## Usability

The one visible change is for code that used `widen` where a value
could be lost. The diagnostic's repair points to `narrow`.

## Explainability

"`widen` when every value fits, `narrow` otherwise, `narrow` always
works" is one sentence.

## Implementation-feasibility

A check in the shared checker, beside the one D-0026 added. `cobc`
already emits no check for a `narrow` that cannot fail.

## Compatibility impact

- **Newly rejected:** `widen` outside containment; `reinterpret`
  between types of different widths or the same signedness. Three
  example programs are updated to `narrow`.
- **Newly well-formed:** `narrow` and `narrow_wrapping` where the
  source always fits.

## Prior-art status

- **Rust:** `From` (lossless) against `TryFrom` (checked); `u64:
  From<usize>` is deliberately not implemented, for this portability
  reason, while `TryFrom` works between any pair.
- **C#:** implicit conversions only where lossless, explicit `checked`
  casts otherwise.

## Invariant traceability

`inv.arith.range-validity`: a `widen` result is always in its target's
represented domain, now by static check rather than by assumption.

## Revisit conditions

- A need for a conversion that is lossless on every `AddrWidth`.
