# D-0064 — Field and element access through a guard

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0018, D-0063, rule.agg.field, rule.conc.lock
Affects: rule.agg.field (`[Guard-Auto-Deref]`), rule.conc.lock

## Problem

A field of a locked value was written `(*g).head`: `[Field-Access-Auto-
Deref]` reads through a reference, not a guard. `std`'s `Channel`
(D-0063) spelled out `(*g).` eleven times. Worse, the tools disagreed on
`g.head`: `coby` ran it, the checker left it untyped (so `g.head + 1`
took the literal's default type and was rejected), and `cobc` failed
with an internal error.

## Constraints

- No new tokens; the fewest symbols a program writes.
- A guard is a resource: reaching through it must not read or move it.

## Candidate mechanisms

1. **`g.f` and `g[i]` are `(*g).f` and `(*g)[i]`**, as for a reference.
   **Selected.**
2. Reject `g.f`: `(*g).f` only.

## Selected design

Candidate 1: `[Guard-Auto-Deref]` (`spec/16`). The guard is in place
position — reached, not read — and the access goes through its lock
path, so every check is `[Guard-Deref]`'s. `*g` is still how the whole
value is read or written.

## Rejected alternatives

- **2:** syntax a guard's user must remember for no semantic gain; a
  guard already behaves as a reference to the locked value in every
  other way.

## Semantic rationale

`*g` names the locked value's place; a projection from it is a
projection from that place. Auto-dereferencing only abbreviates it.

## Usability

    auto g = lock(&hub.state);
    g.count = g.count + 1;
    Vec::push(&mut g.items, x);

## Implementation-feasibility

The checker types a field or index through `guard<τ>` as through
`ref<τ, m>`; `cobc` lowers a guard-typed base through the lock path
(`guard_through`); `coby` already did.

## Compatibility impact

Extension.

## Prior-art status

- **Rust:** `MutexGuard<T>: DerefMut<Target = T>`, so `g.field` works.
- **C++:** `std::lock_guard` guards a separate object; no access through it.

## Revisit conditions

None.
