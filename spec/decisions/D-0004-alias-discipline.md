# D-0004 — Alias-Validity Discipline

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §7, §9, §11, §17
(Aliasing and Concurrent Access), §12
Depends on: inv.alias-validity, inv.concurrency-validity, D-0003

## Problem

`permitted(a1, a2, o, Σ)` was left deferred by
`spec/04-abstract-state.md` and used but not defined by
`rule.value-object.read`/`write` (`spec/05-value-object-semantics.md`).
CobaltC needs an actual rule for which simultaneous access paths to the
same object are jointly valid, without presupposing borrowing,
lifetimes, or a specific concurrency model (§7). This is independent of
`inv.resource-authority` (D-0003): who may eventually destroy a resource
is settled; this decision is about who may concurrently read or write
it right now.

## Constraints

- §20's attack list names "alias conflicts" and "concurrency" explicitly
  as cases to actively attack.
- §6: a false semantic claim (e.g. a value read while being concurrently
  mutated) must not become trusted.
- §9: prefer reusing the enforcement-strategy shape already established
  (D-0003's static-default/dynamic-fallback) over inventing a new one,
  and prefer one mechanism serving more than one invariant where
  genuinely possible.
- Must remain usable for the overwhelmingly common case of passing
  read-only data to multiple simultaneous readers (§8 priority #7,
  human comprehensibility; #4, composability) — a discipline that
  forbids this is too restrictive to be worth adopting regardless of its
  safety.

## Candidate mechanisms

1. **No tracking (unrestricted aliasing).** Rejected outright — directly
   fails to prevent the alias conflicts §20 requires attacking; the
   status quo the invariant exists to rule out.
2. **Exclusive-only (at most one access path to an object, ever).**
   Maximally safe, but forbids ordinary concurrent read sharing, failing
   composability/comprehensibility for no corresponding safety gain over
   option 3 below. Rejected as the general discipline; its shape
   survives as the "exclusive" mode value within option 3.
3. **Shared-read XOR exclusive-write**: any number of `shared`-mode
   access paths may coexist; an `exclusive`-mode access path must be the
   sole live access path to that object. This is the minimal discipline
   that rules out every conflict pattern §20 names (concurrent
   read-during-write, concurrent write-during-write, iterator-style
   invalidation via a stale alias observing a mutation) while still
   admitting the common concurrent-read case. **Selected.**
4. **Fractional/capability permissions** (arbitrary splittable
   permission weights). Strictly more expressive than 3, but nothing in
   the Invariant Registry or §20's attack list motivates the extra
   expressiveness; adopting it would spend permanent complexity budget
   (§9) against no identified requirement. Rejected.
5. **Dynamic-only enforcement** (no static rejection; every conflict
   caught at runtime). Valid as a *fallback* — retained, mirroring
   D-0003 — but rejected as the *default*, for the same reason D-0003
   rejected a dynamic-only resource-authority default: §12 prefers
   static rejection wherever the fact is knowable without execution,
   and a runtime-only discipline makes every aliasing bug a runtime
   fault instead of a compile-time diagnostic in the common case.

## Selected design

- **Mode vocabulary:** `mode ∈ {shared, exclusive}`, attached to each
  access path (`Σ.access-paths(a).mode`, per
  `spec/04-abstract-state.md`).
- **`permitted(a1, a2, o, Σ) ≝ mode(a1, Σ) = shared ∧ mode(a2, Σ) = shared`**
  — the pairwise definition of "shared XOR exclusive": any pair
  involving an `exclusive` access path is not permitted, which
  transitively forces `exclusive` to be solitary across any number of
  simultaneous access paths, since `inv.alias-validity` quantifies over
  *all* simultaneous pairs.
- **Capability, not just coexistence:** `shared` mode permits read only;
  `exclusive` mode permits read and write (and, together with
  `inv.resource-authority`'s authority token, destroy).
- **Enforcement is checked once, at access-path formation** (the
  `Borrow` operation, `spec/08-alias-validity.md`), against every
  currently live access path to the same object — not re-checked at
  every subsequent read/write. This is what makes `inv.alias-validity`
  hold **by construction** once `Borrow` is the sole way to introduce an
  additional access path (mirroring the `inv.identity`/
  `inv.origin-stability` structural guarantees already recorded in
  `spec/04-abstract-state.md` §3): if `Borrow` never admits a
  conflicting pair, no conflicting pair can ever become live.
- **Cross-thread case:** `permitted(...)` is defined without reference
  to `thread-of(a)` — the discipline applies identically within and
  across threads. This lets `synchronized(a1, a2, o, Σ)`
  (`inv.concurrency-validity`) be **grounded as exactly
  `permitted(a1, a2, o, Σ)`** for ordinary tracked access paths: the
  same mechanism discharges both invariants, per §9's preference above.
  This is necessary but not sufficient for concurrency in general — the
  full Concurrency artifact (§23, later) must still add
  synchronization-mediated modes (e.g. a mutex-guarded access path) for
  legitimate patterns this static discipline alone is too restrictive
  for; this decision only fixes the *default*, unsynchronized-access
  baseline.

## Rejected alternatives

Unrestricted aliasing (1); exclusive-only (2, retained as the shape of
the `exclusive` mode value); fractional permissions (4); dynamic-only
enforcement as the default (5, retained as the fallback for statically
unresolvable cases).

## Semantic rationale

The invariant's own pairwise, quantify-over-all-simultaneous-accesses
shape (`spec/03-invariants.md`) makes a pairwise `permitted` definition
the direct, non-encoded expression of "shared reads coexist, writes
don't share" — no separate global bookkeeping is needed beyond what
`Borrow` already checks at formation.

## Usability implications

Callers explicitly choose `shared` or `exclusive` when forming an
additional access path; reading from several places at once remains
unrestricted, which is the dominant real-world pattern this decision
was weighed against.

## Explainability implications

A denied `Borrow` can report exactly which existing access path
conflicts and in which mode, directly from `Σ.access-paths`.

## Implementation-feasibility implications

Checking a proposed access path's mode against every currently live
access path to the same object at formation time is a bounded,
mechanically checkable condition; §20 analysis finds no gap for the
static case, and the dynamic fallback covers the rest.

## Compatibility impact

None yet. Requires revising `rule.value-object.read`/`write`
(`spec/05-value-object-semantics.md`) to drop their now-redundant
per-access `permitted(...)` re-check in favor of a formation-time-only
check plus a simple mode-capability check, and
`rule.resauth.destroy` (`spec/07-resource-authority.md`) to add the
exclusivity precondition it was missing. Both source rules are still
`PROVISIONAL`; these are in-place revisions per `spec/02-schema.md` §6,
not Change records.

## Prior-art status

Independently derived from the invariant's own shape and §20's attack
list; the shared/exclusive shape is a known pattern elsewhere, retained
per §7 as the strongest available fit, not assumed as a starting point.

## Invariant traceability

Realizes `inv.alias-validity`; grounds `synchronized(...)` for
`inv.concurrency-validity`'s unsynchronized baseline (full concurrency
mechanism still open).

## Revisit conditions

Revisit if the Concurrency artifact finds `mode ∈ {shared, exclusive}`
insufficient to express a needed synchronization-mediated pattern
without contradicting this decision's shape, or if the destroy-time
exclusivity precondition this decision motivates proves too restrictive
once real destruction patterns are examined.
