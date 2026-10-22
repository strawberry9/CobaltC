# CHG-0007 — Unit Type Spelled `void`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §21

## Problem / motivation

`spec/22` 2.5.0 spelled the unit type `()` in type position — neither
C nor Rust-authentic as a *type* spelling (Rust's own unit type is
also `()`; C spells "no value" `void`). This was raised early in the
surface-syntax redesign and left open when the human owner chose to
keep `fn` rather than go fully prefix-style. The human owner directed
adopting `void` now.

## Affected entities

No `rule.*`, `inv.*`, `type.*`, `state.*`, `term.*`, or `D-XXXX`
entity's semantics changed — `type.unit`'s id, represented domain
(the single value `()`), and every rule that produces or consumes it
are unaffected; only its type-position surface spelling changed.
Directly touched: `spec/22-surface-syntax.md` (2.6.0: keyword list,
`type`'s unit alternative, one prose clarification);
`spec/13-expression-semantics.md` (1.2.0: `type.unit`'s definition
notes the split spelling); `spec/20-trust-boundaries.md` (1.2.0:
`FfiType`'s `unit` alternative, which was the one place elsewhere
still spelling the type with the bare word rather than a literal
token, inconsistent with `FfiType`'s other literal-token alternatives
`f32`/`f64`/`bool`/`rawptr<τ>`).

## Previous semantics (concrete syntax; no rule semantics involved)

`type ::= ... | '(' ')'` — `()` served as both the unit type and the
unit value, disambiguated only by grammatical position (type vs.
expression).

## New semantics (concrete syntax; no rule semantics involved)

`type ::= ... | 'void'` — the unit type is now `void` wherever a type
is written (`fn f(i32 x) : void { ... }`). The unit *value* is
unchanged: `primary`'s `'(' ')'` alternative (`spec/22` §2) still
governs it, since C's `void` is never itself a value to borrow a
value-position spelling from — this is a deliberate two-spelling
split for one type, not an oversight. A function without `: type`
still returns `unit`; writing `: void` explicitly is now equivalent,
never required.

## Affected invariants

None. `type.unit`'s represented domain, `is-resource = false`, and
every position that produces it (assignment, `if` without `else`,
`while`, a trailing-expression-less block, a function with no
declared return type) are unaffected by which token names the type.

## Dependency impact

None. No `Depends on`/`Affects` line changes anywhere.

## Compatibility classification

Source-breaking, semantics-preserving, same shape as `CHG-0001`–
`CHG-0006`: any program that wrote `()` explicitly as a return type
(none found in this corpus — see below) would need `void` instead; no
program's meaning changes.

## Migration implications

Mechanical: replace `()` with `void` wherever it names a *type*
(return-type position, or any other type position); leave every `()`
that names the unit *value* untouched. Verified before making this
change that `spec/examples.md`, `spec/conformance.md`, and `spec/21`
never wrote `()` explicitly as a return type in the first place (every
function needing `unit` simply omitted `: type`, which already meant
unit and still does) — so no cascade was needed in any of them.

## Example changes

None required (see above).

## Conformance changes

None required, for the same reason.

## Future implementation implications

None beyond an ordinary keyword-table addition (`void`) and a
one-line grammar change in the type parser; the represented-domain,
layout (`sizeof(unit) = 0`), and value semantics of `type.unit` are
completely unaffected.

## Prior-art status

Not applicable in the D-XXXX sense; a surface-syntax preference change
directed by the human owner, continuing `CHG-0001`–`CHG-0006`'s
redesign, resolving an item left open since the `fn`-keyword decision
early in that redesign.

## Revisit conditions

None beyond `CHG-0001`'s own: a further syntax change layered on top
of this one is a new Change record, not an edit to this one.
