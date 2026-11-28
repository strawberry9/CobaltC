# D-0018 — Access-Path Families, Use-Time Alias Checking, and Container-Bound Reference Lifetime

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §7, §9, §11, §12, §17
(Aliasing and Concurrent Access; Temporal Validity)
Depends on: inv.alias-validity, inv.temporal-validity, inv.identity,
term.access-path, D-0003, D-0004, D-0008, D-0016
Affects: D-0015, inv.alias-validity, inv.temporal-validity,
state.access-paths, state.suspended, state.temp-scope-stack,
rule.alias.borrow, rule.value-object.read, rule.value-object.write,
rule.agg.field-access, rule.agg.index, rule.control.block,
rule.control.stmt, rule.temporal.elision,
diag.aliasing-conflict, diag.borrow-exceeds-source

**Supersedes D-0015** (`spec/decisions/D-0015-access-path-lifetime.md`),
whose Status becomes `REMOVED`. Realizes the fix for `spec/AUDIT-2.md`
B-01, B-02, B-03, B-04, B-08, B-15, B-16, and resolves D-0015's two
Revisit conditions (disjoint-field borrows, references stored in
aggregate fields — the first audit's A-32/A-33).

## Problem

D-0015 answered "how long does an access path live" with two
scope-exit sweeps and a syntactic `durable` flag, and answered "which
simultaneous accesses conflict" by exempting whole base/projection
families from the pairwise check while suspending the source of every
borrow. Deriving the specification's own examples under those rules
(`spec/AUDIT-2.md` §1) shows:

1. A projection is formed without any conflict check and is exempt from
   the invariant against its own family, so `let r = &mut p.a; p.a = 5;`
   writes through a fresh projection while `r` is live — a genuine
   violation of `inv.alias-validity`, introduced by D-0015 (B-01).
2. Every binding has mode `exclusive`, and any borrow whose pair is not
   shared/shared suspends its source, so `let r = &v;` makes `v`
   unusable for as long as `r` lives. D-0004's constraint that
   read-only data be passable to several readers is violated for every
   named variable (B-02).
3. The reactivated set is subtracted from the invalidation set, so a
   temporary borrow outlives its statement and blocks a later `drop`
   (B-03).
4. Only `[Borrow]` was updated to record `frame`/`temp-scope`/`base`;
   bindings, projections, `Reclaim`, and `Lock` were not, so the sweeps
   and `related` are undefined for them (B-04).
5. `durable` is decided from the shape of a `let` initializer, so a
   reference stored in a struct field, an array element, or a closure's
   capture struct dies at the end of its own construction statement
   (B-08, D-0015's own Revisit condition).
6. `[Borrow]` never requires the requested mode to be no stronger than
   the source's, so an exclusive path can be derived from a shared
   reference (B-15).

These are symptoms of two unsettled questions: *what* an alias check
compares (which paths count as competing), and *what* a reference's
validity is tied to (a syntactic scope, or the storage that holds it).

## Constraints

- §6: "checked once" is not "still valid now" — the guarantee must hold
  at every reliance point (every read and write), not only at the
  moment a path is formed.
- §17 (Aliasing): the specification must say which simultaneous accesses
  may coexist; §17 (Temporal Validity): access relationships must cease
  to be usable when the state they depend on is no longer valid.
- §17 (Function Interfaces): a `ref<τ, shared>` parameter must mean
  what its type says — the callee cannot mutate through it.
- §9: one mechanism, applied uniformly to bindings, borrows,
  projections, reclaimed storage, and lock guards; no special cases per
  forming rule.
- §12: the static/dynamic split of the resulting checks must be the one
  `stat.flow-analysis` (`spec/14` §6) fixes, never implementation
  quality.
- D-0004 stands: modes are `shared`/`exclusive`; several shared may
  coexist; an exclusive access must be alone. D-0003/D-0008/D-0016
  stand: destroy authority is single-holder, swept at scope end, keyed
  on the current holder.

## Candidate mechanisms

### For "which paths compete"

1. **Formation-time check only, whole families exempt (D-0015).**
   Unsound for projections formed after a borrow (problem 1). Rejected.
2. **Formation-time check for every path-forming rule, including
   projections, with source suspension.** Sound, but a projection would
   then have to suspend its base, making `p.a` unusable while `p.b` is
   read, and a shared borrow would still suspend an exclusive binding
   (problem 2) unless suspension became a mode downgrade with its own
   restore bookkeeping — a second mechanism layered on the first.
   Rejected.
3. **Ancestor exemption with use-time checking.** Every access path
   records the path it was derived from (`base`): a borrow's base is the
   path it borrowed from; a projection's base is the path it narrows.
   The chain from a path to its root is its *ancestors*. An access of
   mode `m` through `a` conflicts with a live path `a'` iff `a'` is not
   an ancestor of `a`, targets the same object with an overlapping
   sub-range, and the pair `(m, mode(a'))` is not shared/shared. This
   predicate is checked when a borrow is *formed* (early diagnostic) and
   again at every *read and write* (the guarantee). Ancestors are exempt
   because they are the source of `a`'s own authority — a child never
   competes with its parent — while descendants and cousins are checked,
   so a parent cannot be used in a way that conflicts with a live child.
   No suspension, reactivation, or downgrade state is needed: the
   parent's use is simply denied while a conflicting child lives, and
   permitted again once the child is gone. Overlap is by target
   sub-range, so exclusive borrows of disjoint fields coexist.
   **Selected.**

### For "what a reference's validity is tied to"

1. **Syntactic scope of the forming statement, with a `durable` flag
   (D-0015).** Cannot see a reference stored anywhere but a bare `let`
   (problem 5). Rejected.
2. **Escape analysis at formation** (decide, when the borrow is formed,
   which object will eventually hold it). Requires forward reasoning
   about the rest of the statement; a second analysis alongside
   `stat.flow-analysis`. Rejected.
3. **Container-bound lifetime.** A reference value that is written into
   storage is *held by* the object written; a path is valid while any
   holder is alive, or — if it is held by nothing — until the end of
   the statement that formed it, unless it is that statement's own
   result (in which case it flows onward and its next holder is decided
   by whoever stores it). Ending an object invalidates every path that
   targets it and every path it was the last holder of. Bindings are
   the root case: a binding's path is held by nothing and ends when its
   object ends at frame exit. This is one rule for `let`, parameters,
   struct/array/enum fields, closure captures, and returned references,
   with no syntactic special case. **Selected.**

## Selected design

**`AccessPathRecord`** (`spec/04-abstract-state.md`) becomes
`{ target: Set(Address), of: Identity, type: Type, mode: Mode, thread:
Thread, valid: Bool, formed-at: Event, temp-scope: TempScopeId, base:
AccessPathToken?, held-by: Set(Identity) }`. `frame`, `durable`, and
`state.suspended` are removed; `type` is added (the type of what the
path targets — the object's type for a root, the field/element type for
a projection). Every path-forming rule sets every field:

| Forming rule | `base` | `mode` | `held-by` |
|---|---|---|---|
| `[Binding-Form]` (a `let`/parameter/pattern binding) | `None` | `exclusive` | `∅` |
| `[Borrow]` from `a0` with `m` | `a0` | `m` | `∅` |
| `[Field-Access]`/`[Index-Checked]` from `a0` | `a0` | `mode(a0)` | `∅` |
| `[Reclaim]`, `[Lock]` | `None` | `exclusive` | `∅` |

**Predicates** (`spec/04` §2):

    ancestors(a, Σ) ≝ {a} ∪ (ancestors(base(a), Σ) if base(a) ≠ None)
    overlap(a1, a2, Σ) ≝ target(a1) ∩ target(a2) ≠ ∅
    clash(a, m, Σ) ≝ ∃ a'. temporally-valid(a', Σ) ∧ of(a') = of(a)
                          ∧ a' ∉ ancestors(a, Σ) ∧ overlap(a, a', Σ)
                          ∧ ¬(m = shared ∧ mode(a') = shared)

**`inv.alias-validity`** is restated: for every access of mode `m`
through `a`, `¬clash(a, m, Σ)`. This replaces D-0015's `related`
exclusion (`related` is removed).

**`[Borrow]`** (`spec/08` §2) requires `temporally-valid(a0)`,
`m = exclusive ⇒ mode(a0) = exclusive` (else
`diag.borrow-exceeds-source`, new), and `¬clash(a0, m, Σ)` (else
`diag.aliasing-conflict`). It forms `a_new` with `base := a0`, `mode :=
m`, `held-by := ∅`. It suspends nothing.

**`[Read]`** through `a` requires `¬clash(a, shared, Σ)`; **`[Write]`**
through `a` requires `mode(a) = exclusive ∧ ¬clash(a, exclusive, Σ)`;
both yield `diag.aliasing-conflict` otherwise (`spec/05`). Projections
(`spec/16` §1–§2) perform no check at formation: they inherit their
base's mode and are checked at use like any other path.

**Lifetime** (`spec/14` §1–§1a, `spec/05` `[Write]`/`[Object-End]`):

- `[Write]` of a value containing reference tokens adds the written
  object to each token's `held-by`; overwriting a slot removes the
  object from the `held-by` of each token the old contents contained.
- `[Object-End]` invalidates every path `a` with `of(a) = o`, removes
  `o` from every `held-by`, and invalidates every path whose `held-by`
  thereby becomes `∅` and whose forming statement has already ended.
- `[Stmt-Exit]` invalidates every path formed in the exiting statement
  scope whose `held-by = ∅` and which is not the statement's result;
  it also destroys/ends every *temporary object* (D-0019) created in
  that scope that is not the statement's result.
- `[Block-Exit]` ends every object whose holder binding belongs to the
  exiting frame (destroying resources first, in reverse establishment
  order, per D-0008/D-0016), excluding the block's result; ending those
  objects is what invalidates the frame's bindings and everything they
  held.

**`[Destroy]`** (`spec/07`) keeps its solitary-access precondition:
no other valid path on `o`, ancestors included.

**D-0011's `[Call-Elided-Lifetime]`** (`spec/10` §3) loses its
`Σ`-level suspend effect: the returned reference is an ordinary path
formed inside the callee, checked at use like any other; the rule keeps
only its static escape check.

## Rejected alternatives

Formation-time-only checking with family exemption (D-0015); universal
formation-time checking with suspension; syntactic `durable`; escape
analysis at formation.

## Semantic rationale

`inv.alias-validity` is a statement about simultaneous *accesses*
(§17), so it is discharged where accesses happen. Formation-time
checking in `[Borrow]` is retained only because §12 prefers an early,
static diagnostic where one is available; it is not what makes the
system sound. The ancestor exemption is the precise statement of what
D-0015's prose meant by "the same access, narrowed": a derived path is
the same authority as its source, so the two never compete — but two
derived paths from the same source do, and a source competes with its
own derived paths. Tying a reference to its containers is the direct
reading of `inv.temporal-validity`'s "access relationships cease to be
usable when the state they depend on is no longer valid": what a stored
reference depends on is the storage holding it and the object it
targets, nothing syntactic.

## Usability implications

`let r = &v; use(&v); use(v.len)` is well-formed; `push(&mut v, 1)`
twice in a row is well-formed; `let r1 = &mut p.a; let r2 = &mut p.b;`
is well-formed; a reference in a struct field, array element, or closure
lives as long as its container. A parent binding is unusable only in
the specific ways that conflict with a live child (writing while a
shared child lives; any access while an exclusive child lives), with
`diag.aliasing-conflict` naming the child.

## Explainability implications

Every conflict names the competing path and the relation between the
two (descendant, cousin) directly from `base`. Every invalidation names
its event: object end, last holder ended, statement end, transfer.

## Implementation-feasibility implications

`clash` is a scan over live paths of one object; `stat.flow-analysis`
resolves it statically in straight-line code, and the dynamic fallback
is a per-object list of live paths. `held-by` is updated at writes of
reference-typed values only. No whole-program analysis is required.

## Compatibility impact

Supersedes D-0015 (`REMOVED`). Refines D-0004's "checked once, at
formation" to "checked at formation and at every access" without
changing D-0004's mode vocabulary or `permitted`. Revises `spec/03`
(`inv.alias-validity`, `inv.temporal-validity`), `spec/04`, `spec/05`,
`spec/08`, `spec/09` (`[Ref-Identity-Eq]` compares object and target,
B-16), `spec/10` §3, `spec/14` §1–§1a, `spec/15` §6, `spec/16` §1–§2,
`spec/19` §2, `spec/20` §2 — all `PROVISIONAL`, in place.

## Prior-art status

Independently derived from `inv.alias-validity`'s pairwise shape and
from the requirement that stored references survive their forming
statement. The resulting discipline resembles reborrowing-with-
two-phase-use in prior art; retained on its merits, not adopted for
familiarity.

## Invariant traceability

`inv.alias-validity` (restated as `¬clash` at every access);
`inv.temporal-validity` (container-bound lifetime; object end; statement
end); `inv.identity` (`[Ref-Identity-Eq]`).

## Revisit conditions

Revisit if a construct needs a reference to outlive every container
holding it (would require a heap-owned reference cell, not designed);
revisit `clash`'s overlap test if a type needs finer-than-field
disjointness (bit fields, not designed).
