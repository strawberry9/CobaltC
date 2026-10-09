# CHG-0023 — `feat.consolidate-join-typing` Decided: `REJECTED` (`[T-Join]` Is Not Redundant)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §18, §21
Depends on: rule.type.typing, rule.fn.generic-call, rule.conc.join, state.items, rule.module.resolve, D-0010, D-0012
Affects: feat.consolidate-join-typing, rule.type.typing, state.items

## Problem / motivation

`CHG-0008` left open whether `[T-Join]` (`Γ ⊢ e : handle<τ> ⇒ Γ ⊢
join(e) : τ`, `spec/12`) is redundant with ordinary `[T-Call]`/
`[Generic-Call-Inferred]` typing applied to `join`'s own prelude
signature `join<T>(handle<T> h) : T`. Tracing every case by hand found
no divergence, but `spec/registry/features.md` recorded the question
as blocked on a modeling choice no existing rule settled: is a
body-less prelude intrinsic like `join` registered in `state.items`
(`spec/04` §1), where ordinary `[T-Item]`/`[T-Call]` could reach it,
or resolved through some other path entirely? The human owner directed
a decision rather than leaving this prepared-but-undecided, delegating
it to the design agent.

## Decision

**`REJECTED` — `[T-Join]` is retained; the redundancy does not hold,
and the blocking question is now settled rather than merely assessed.**

The modeling question resolves from already-`ACCEPTED` text, not from
a new choice:

1. `state.items`'s `Item` type (`spec/04` §1) has exactly four
   alternatives — `fn-item | extern-item | struct-item | enum-item |
   mod-item` — each produced only by the `item` grammar production
   (`spec/22` §3: `fn-decl | extern-decl | struct-decl | enum-decl |
   module-decl | import-decl`). There is no "intrinsic-item"
   alternative, and `join`'s call form is not produced by that
   grammar production at all — it is recognized directly as an
   expression form, the same as `spawn`, `sizeof`, `widen`, `reclaim`,
   and every other body-less row in `spec/21` §0's intrinsics table.
2. `spec/12` §5's own opening line states `Γ` maps "item paths to
   their signatures (`Σ_0.items`)" — `Γ`'s *only* source of item-path
   bindings is `Σ_0.items`. `[T-Item]` (`Γ(path) = (τ1..τn) -> τr`)
   therefore has nothing to look up for the bare name `join`,
   regardless of whether `join`'s arity is fixed (unlike `spawn`,
   whose variable arity was already known to block this route —
   `join`'s fixed arity does not change the outcome, because the
   blocking fact is `Σ.items` membership, not arity).
3. `[T-Call]`'s premise `callable(type-of(e_f), …)` needs `type-of(e_f)`
   from `[T-Item]`/`[T-Name]` in the first place; with no `Σ.items`
   entry for `join`, `type-of(join)` is never derived, so `[T-Call]`
   can never fire on it either.

`[T-Join]` is consequently not a redundant convenience kept for
symmetry with `[T-Spawn]` — it is, like `[T-Spawn]`, the *only* rule
that gives its call form a type at all. The "what blocks confirming
this outright" question in the original write-up is answered: prelude
intrinsics without a body are never `Σ.items` entries; only a prelude
function *with* a body (`map_err`, `spec/21` §0's one bodied intrinsic
row) and the library types' own methods (`Vec::push`, `Rc::clone`, …)
are ordinary `fn-item` entries reachable by `[T-Item]`/`[T-Call]`.

## What changed

**No rule changed.** `[T-Join]` and `[T-Call]` are both unchanged;
this record adds no new typing behavior — it establishes that the two
were never actually in competition.

**`spec/registry/features.md` 1.3.0**: `feat.consolidate-join-typing`
moved `UNDER_INVESTIGATION → REJECTED`, citing this record.

**`spec/04-abstract-state.md` 1.3.0** (`state.items`, non-normative):
gains a sentence stating that `Item`'s alternatives come only from the
`item` grammar production, so a body-less intrinsic is never an entry,
with `map_err`/library methods named as the bodied exception.

**`spec/12-type-system.md` 1.6.1** (§5, non-normative): gains a
paragraph stating the structural argument above directly under
`[T-Join]`'s definition, so a future reader of this specific rule sees
why it exists without following the citation chain.

## Rule changes

None.

## Affected invariants

None restated. No invariant depended on this question being open.

## Dependency impact

`feat.consolidate-join-typing` gains a `Depends on` citation of this
record; `state.items` and `rule.type.typing` (`spec/12` §5) each gain
one, for the clarifying prose each now carries.

## Compatibility classification

None (no rule, example, or conformance case changes; clarifying prose
only, stating a fact already true of the accepted rules).

## Migration implications

None.

## Example changes

None. No existing example exercises this distinction in a way that
needed correction; `spec/conformance.md`'s existing `spawn`/`join`
derivations were already consistent with this reading.

## Conformance changes

None.

## Future implementation implications

None. An implementation was always going to type `join` by its own
dedicated logic (there is no other way to type it, per the argument
above) — this record does not add or remove an implementation
obligation, only documents why the obligation is what it already was.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

None outstanding. If a future feature ever wants prelude intrinsics to
*become* ordinary `Σ.items` entries (e.g. to make them shadow-able,
importable, or otherwise first-class the way user functions are),
that would be a new, separately-proposed feature changing `Item`'s
grammar and `state.items`'s population rule — not a reopening of this
question, which concerns only the *current*, unchanged rules.
