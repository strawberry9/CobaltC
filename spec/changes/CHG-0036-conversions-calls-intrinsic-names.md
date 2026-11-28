# CHG-0036 — Conversion Directions, Call Arity, and Intrinsic Names

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-24, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0027, rule.arith.convert, rule.type.typing, rule.module.resolve, rule.stdlib.prelude
Affects: rule.arith.convert, rule.stdlib.prelude, the §1 cases listed below

## Problem / motivation

Three gaps found while implementing D-0026:
1. **Conversion directions** were not checked (D-0027). `widen<u64>(-5)`
   gave different results in the two implementations.
2. **`[T-Call]`** was not enforced: a call with too many or too few
   arguments was accepted by both implementations, and a call of a
   non-callable value faulted at run time in `coby` and was an internal
   error in `cobc`.
3. **Intrinsic names.** A program's own `fn widen()` crashed `coby`,
   which looked up intrinsics before items. The checker and `cobc`
   looked up items first. No rule said which was right.

## Decision

1. D-0027.
2. None needed: `[T-Call]` already requires a callable callee and one
   argument per parameter. The implementations now enforce it.
3. An intrinsic is not an item (`spec/21` §0), so `[Resolve-Unqualified]`
   never yields one, and a local binding comes before any item. The
   intrinsic is therefore what a name means only where no binding or
   item is found. `spec/21` §0 now says so.

## What changed

- **`spec/06` 1.7.0:** `[Narrow-Checked]` and `[Narrow-Wrapping]` apply
  to any two integer types; `[T-Convert]` checks `[Widen]`'s containment
  and `[Reinterpret-Sign]`'s width and signedness.
- **`spec/21` 3.2.0:** §0's paragraph on intrinsic names.
- **`spec/conformance.md` 3.21.0:** nine §1 cases.
- **Implementations:**
  - the shared checker enforces `[T-Convert]`'s directions and
    `[T-Call]`'s callability and arity, for functions, `extern`
    functions, `fn` values and closures bound by a declaration;
  - the checker and `coby` look a callee up as a local binding first,
    then as an item, then as an intrinsic, as `cobc` already did.
- **Examples:** `06_errors.cb`, `07_generic_stack.cb` and
  `08_closures.cb` use `narrow` where they used `widen` outside its
  direction.

## Affected entities

`rule.arith.convert` (premises); `rule.stdlib.prelude` (text).

## Previous semantics

    [Narrow-Checked]    τ, τ' integer, represented-domain(τ) ⊄ represented-domain(τ'), v ∈ represented-domain(τ')
    [Narrow-Wrapping]   τ, τ' integer, represented-domain(τ) ⊄ represented-domain(τ')

## New semantics

    [Narrow-Checked]    τ, τ' integer, v ∈ represented-domain(τ')
    [Narrow-Wrapping]   τ, τ' integer
    [T-Convert]         … for `widen`, represented-domain(τ) ⊆ represented-domain(τ');
                        for `reinterpret`, the same bitwidth and differing signedness
                        otherwise diag.type-mismatch (static)

## Affected invariants

`inv.arith.range-validity`: `widen`'s results are in range by static
check.

## Dependency impact

`rule.arith.convert` depends on D-0027.

## Compatibility classification

- **Extension:** `narrow` where the source always fits.
- **Breaking:** `widen` and `reinterpret` outside their directions, and
  calls with the wrong number of arguments or of a non-callable value,
  are rejected statically. The first two had no rule before; the last
  two broke `[T-Call]`.

## Migration implications

Replace `widen` by `narrow` where the source's domain is not contained
in the target's. In this repository, four calls in three example
programs.

## Example changes

None in `spec/examples.md`.

## Conformance changes

**Added:**
- `conf.widen-not-contained-rejected`
- `conf.narrow-contained-ok`
- `conf.reinterpret-same-sign-rejected`
- `conf.call-too-many-args`
- `conf.call-too-few-args`
- `conf.closure-call-arity-rejected`
- `conf.call-non-callable-rejected`
- `conf.item-shadows-intrinsic`
- `conf.local-shadows-intrinsic`

## Future implementation implications

- **Callee lookup order:** a local binding, then an item, then an
  intrinsic.
- **Closure arity:** a closure's type cannot be written, so it is
  called only through a binding initialized by a closure literal or by
  another such binding (`auto d = c;`). The checker follows the
  parameter count through both. `coby` also checks it when it calls a
  closure, as a backstop.

## Prior-art status

See D-0027.

## Revisit conditions

See D-0027.
