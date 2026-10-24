# CHG-0008 — `spawn`/`join` Demoted to Prelude Intrinsics

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §21

## Problem / motivation

`spec/22` 2.6.0 gave `spawn` and `join` dedicated keyword status and
dedicated `primary` grammar productions, even though both already
used ordinary call syntax (`spawn(f, args)`, `join(h)`) and nothing
about their behavior required reserving the words. Every other
special operation in the language (`drop`, `sizeof`, `allocate`,
`reclaim`, `lock`, …) is an ordinary prelude intrinsic — a named
identifier whose call is governed by a rule instead of a body — not a
keyword. The human owner directed reclassifying `spawn`/`join` the
same way, matching that precedent and, notably, matching how even
Rust's own `thread::spawn` is an ordinary function, not a keyword.

## Affected entities

No `rule.*`, `inv.*`, `type.*`, `state.*`, `term.*`, or `D-XXXX`
entity's semantics changed — `rule.conc.spawn`, `rule.conc.join`,
`[T-Spawn]`, `[T-Join]` (`spec/12`), and `type.handle` are completely
unaffected; their reduction subjects (`⟨spawn(e_f, e1..en), Σ⟩`,
`⟨join(e), Σ⟩`) were already call-shaped text and did not need to
change. Only the *classification* changed: dedicated core syntax →
prelude intrinsic. Touched: `spec/22-surface-syntax.md` (2.7.0:
keyword list, `primary`, correspondence table); `spec/13-expression-
semantics.md` (1.3.0: two now-redundant evaluation-context entries
removed — see below, this is the one place with an actual, if purely
presentational, simplification); `spec/21-standard-library-
semantics.md` (2.4.0: `spawn`/`join` added to the §0 intrinsics
table); `spec/19-concurrency.md` (1.2.0: one clarifying note, no rule
text changed).

`spec/13`'s evaluation-context grammar (`rule.expr.context`) had
dedicated entries `join(E)` and `spawn(E, e1..en) | spawn(r, r1..,
E, ..)` describing their evaluation order (callee first, then
arguments left to right). Once `spawn`/`join` are ordinary calls, this
order is already fully specified by the pre-existing generic entry
`E(e1..en) | r(r1..r_{i-1}, E, e_{i+1}..en)` — the dedicated entries
became a redundant restatement of the same fact, not additional
information, and were removed. This is the only edit in this record
that removes normative-looking text rather than purely reclassifying
or documenting; `rule.expr.context`'s actual guarantee (D-0007) is
identical before and after for every construct, `spawn`/`join`
included.

## Previous semantics (concrete syntax; no rule semantics involved)

`spawn`/`join` were reserved keywords with dedicated `primary`
alternatives (`'spawn' '(' expr (',' expr)* ')'`, `'join' '(' expr
')'`), listed in `spec/22`'s correspondence table, and given their own
evaluation-context entries in `spec/13`.

## New semantics (concrete syntax; no rule semantics involved)

`spawn`/`join` are ordinary identifiers naming prelude intrinsics
(`spec/21` §0), resolved and called through the same `e(args)`
grammar and `rule.fn.call`/generic-call machinery as any other
function value, with their special typing/reduction behavior supplied
by `rule.conc.spawn`/`rule.conc.join`/`[T-Spawn]`/`[T-Join]` exactly
as before. They are no longer reserved words. `spawn`'s intrinsic
entry documents that its argument count and types match its callable
argument's own signature — not a fixed arity, unlike every other
intrinsic in the table — which is why `[T-Spawn]` remains a dedicated
typing rule rather than folding into ordinary generic-call inference;
`join<T>(handle<T> h) : T` has an ordinary fixed-arity generic shape,
and `[T-Join]` is kept alongside it for symmetry and because this
record does not audit whether it is fully redundant with ordinary
`[T-Call]`/`[Generic-Call-Inferred]` typing — a question left to a
future record if it matters.

## Affected invariants

None. `inv.resource-authority` (the handle) and `inv.concurrency-
validity` (spawn/join's interaction with `synchronized(...)`) are
defined over `rule.conc.spawn`/`rule.conc.join`'s behavior, which is
unchanged.

## Dependency impact

`spec/21` §0's `Depends on` gains `rule.conc.spawn`, `rule.conc.join`
(the intrinsics table now documents them). No other `Depends on`/
`Affects` line changes.

## Compatibility classification

Semantics-preserving and, uniquely among `CHG-0001`–`CHG-0008`,
**not source-breaking**: every existing `spawn(...)`/`join(...)` call
was already written in the exact syntax this record requires, since
both already used call notation. The only behavioral surface change
is that `spawn`/`join` stop being reserved words — a program that
previously could not name anything `spawn` or `join` now can (and, if
it does, shadows the intrinsic in that scope, the same risk every
other intrinsic already carries).

## Migration implications

None for call sites. A program that happened to rely on `spawn`/
`join` being unshadowable reserved words (not observed anywhere in
this corpus) would need to avoid reusing those names locally, exactly
as it already should for `drop`/`sizeof`/etc.

## Example changes

None. Verified before making this change that every existing
`spawn`/`join` usage in `spec/examples.md` already used ordinary call
syntax and needed no edits.

## Conformance changes

None, for the same reason — every `conf.spawn-*`/`conf.*-join-*` row
was already written in call syntax and remains valid verbatim.

## Future implementation implications

An implementation drops `spawn`/`join` from its reserved-word set and
resolves them via ordinary name resolution into the prelude, same as
`drop`/`sizeof`. `spawn`'s variadic-matching-callable-arity typing
still requires bespoke handling in the type checker (`[T-Spawn]`),
exactly as it already did when `spawn` was dedicated syntax — this
record does not reduce that implementation burden, only relocates
where it's documented.

## Prior-art status

Not applicable in the D-XXXX sense; a surface-syntax/classification
preference change directed by the human owner, continuing `CHG-0001`–
`CHG-0007`'s redesign. Notably motivated by prior art in the opposite
direction from most of this redesign's other changes: Rust's own
`thread::spawn` is an ordinary function, not a keyword, and this
record moves CobaltC toward that shape rather than away from it.

## Revisit conditions

Revisit `[T-Join]` if a future audit confirms it is fully redundant
with ordinary `[T-Call]`/`[Generic-Call-Inferred]` typing over the
`spec/21` §0 signature — this record deliberately did not make that
determination. Otherwise, none beyond `CHG-0001`'s own: a further
syntax change layered on top of this one is a new Change record, not
an edit to this one.
