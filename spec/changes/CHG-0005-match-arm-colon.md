# CHG-0005 — Match-Arm Separator Changed from `=>` to `:`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §21

## Problem / motivation

`spec/22` 2.3.0's `match` arms used `=>` (Rust/Scala/ML's fat arrow)
between a pattern and its consequence. The human owner directed
switching to `:`, matching C's own `case X:` idiom, while keeping the
`match` keyword itself (rather than `switch`, which would wrongly
suggest fallthrough/`break` semantics CobaltC does not have).

## Affected entities

No `rule.*`, `inv.*`, `type.*`, `state.*`, `term.*`, or `D-XXXX`
entity's semantics changed — `rule.agg.match`'s binding, consumption,
and exhaustiveness behavior (`[Match]`, `[Match-Move-Through-Ref]`,
`[Match-Non-Exhaustive]`, `[Match-Conflict]`) is unaffected; only the
token separating a pattern from its arm body changed. Directly
touched: `spec/22-surface-syntax.md` (2.4.0, `arm` grammar);
`spec/16-aggregates.md` (1.2.0, `match`'s grammar line and `[Match]`'s
arm-shape prose); `spec/12-type-system.md` (1.3.0, `[T-Match]`);
`spec/18-error-failure-semantics.md` (1.1.0, `[Propagate-Ok]`'s
desugaring target — see note below); `spec/21-standard-library-
semantics.md` (2.2.0, `map_err`, `Vec::grow`, `Rc::new`); `spec/
examples.md` (3.3.0); `spec/conformance.md` (3.3.0).

While re-spelling `spec/18`, its `match` scrutinee was found still
unparenthesized (`match r { … }`) — a gap `CHG-0003` should have
closed but missed, since that pass never checked `spec/18`. Fixed in
the same edit as this record's own change, since both touched the
same line; noted here for an accurate history rather than folded
silently into `CHG-0003`.

Separately (not part of this record, recorded for completeness): while
verifying this change, `spec/13-expression-semantics.md` was found to
have been missed by *every* earlier re-spelling pass (`CHG-0001` and
`CHG-0003` both) — its evaluation-context grammar still had
`Name{f1: r1, …}`-style field-init and unparenthesized `if`/`match`.
Fixed directly as a retroactive catch-up, logged in that file's own
Change Log rather than attributed to this record, since it isn't a
consequence of the `=>`-to-`:` decision.

## Previous semantics (concrete syntax; no rule semantics involved)

`arm ::= path '(' (identifier | '_') ')' '=>' expr | path '=>' expr |
'_' '=>' expr`.

## New semantics (concrete syntax; no rule semantics involved)

`arm ::= path '(' (identifier | '_') ')' ':' expr | path ':' expr |
'_' ':' expr`. No new ambiguity: an arm list is already a
self-contained grammatical context (inside `match (e) { … }`), so `:`
overloading with the return-type-introducing colon used elsewhere
never requires disambiguation — the two never appear in the same
syntactic position.

## Affected invariants

None. `match`'s exhaustiveness, consumption, and binding rules are
defined over the pattern/body pair, not the token separating them.

## Dependency impact

None. No `Depends on`/`Affects` line changes anywhere.

## Compatibility classification

Source-breaking, semantics-preserving, same shape as `CHG-0001`–
`CHG-0004`: every match arm in existing CobaltC source must have its
`=>` replaced with `:`; no program's meaning changes.

## Migration implications

Mechanical and total: replace every arm's `=>` with `:`. No new fact
is introduced — a pure token substitution, sound for every program
accepted under 2.3.0's grammar.

## Example changes

`ex.exhaustive-match` and `ex.resource-payload` re-spelled; no `ex.*`
id, category, or semantic content changed.

## Conformance changes

Every `conf.match-*` row, plus `conf.vec-pop-resource` and
`conf.string-from-utf8-ok`, re-spelled; no row's id, outcome, or
derivation changed.

## Future implementation implications

None beyond ordinary parsing: `:` closing an arm is resolved within
the already-delimited `match (...) { ... }` body, the same bounded
context that already resolves every other construct in this
specification's grammar.

## Prior-art status

Not applicable in the D-XXXX sense; a surface-syntax preference change
directed by the human owner, continuing `CHG-0001`–`CHG-0004`'s
redesign.

## Revisit conditions

None beyond `CHG-0001`'s own: a further syntax change layered on top
of this one is a new Change record, not an edit to this one.
