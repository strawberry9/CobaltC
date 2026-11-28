# D-0109 — A nested pattern looks through a reference payload

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 proposal P2, option A)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §12
Depends on: D-0046 (match by reference), D-0056 (nested patterns), D-0057 (literal patterns), D-0088
Affects: spec/16 `rule.agg.match`

## Problem

`HashMap::get` gives `Option<ref<V, shared>>`. With `V` an enum, the
natural pattern `Some(Num(v))` was `diag.type-mismatch`: a nested
pattern went only into a payload whose type is an enum, and this
payload is a reference to one. Programs split every such lookup into
two `match`es (`Some(r) : match (r) { Num(v) : …, … }`), three times in
one round-6 program alone.

## Candidate mechanisms

- **A: a pattern level whose payload is `ref<E, m>`, E an enum, matches
  E's variants through the reference, and the binder below is a
  reference of mode `m`**, as `[Match-By-Ref]` already does at the top
  level. Selected: no syntax, and coverage is E's variant set.
- **B: keep the two-level form** (with the message that names it).

## Selected design

- Below the first level, a payload of type `ref<E, m>` (E an enum) is
  matched as the E it refers to; a literal below a variant compares
  through a `ref<τ, m>` to an integer or `bool`.
- The binder is then `ref<τ, m>` to what the pattern reached, derived
  from the reference looked through (its borrow is the binder's), in
  the weaker mode when there are several references (or one and a
  `match` by reference).
- Nothing is moved through a reference: a resource payload is bound by
  reference.
- Exhaustiveness splits the referent's variants.

## Compatibility impact

Additive: patterns that were rejected are now accepted; nothing that
was accepted changes.

## Revisit conditions

None.
