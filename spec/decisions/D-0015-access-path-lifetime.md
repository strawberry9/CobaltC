# D-0015 — Access-Path Lifetime: Frame-Exit Invalidation, Temporary Scope, and the Base/Projection Relation

Status: REMOVED — superseded by `spec/decisions/D-0018-access-path-families-and-use-time-checking.md` (per `spec/02-schema.md` §6 this file is kept unedited below this line; D-0018 states why: `spec/AUDIT-2.md` B-01, B-02, B-03, B-04, B-08)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §7, §9, §11, §17
(Identity, Origin, and Spatial Validity; Temporal Validity), §12
Depends on: inv.temporal-validity, inv.alias-validity, inv.identity,
term.access-path, D-0003, D-0004, D-0008
Affects: inv.alias-validity, inv.temporal-validity, state.access-paths,
rule.control.block-exit, rule.alias.borrow, rule.agg.field-access

Realizes the fix for `spec/AUDIT.md` A-01, A-02, A-03 (worked as one
problem per `spec/AUDIT.md` §6 step 1).

## Problem

`inv.temporal-validity`'s registry entry (`spec/03-invariants.md`)
requires an access path to become invalid when the state it depends on
is no longer valid, and `inv.alias-validity` requires every pair of
*genuinely distinct* simultaneous accesses to be jointly permitted. Three
concrete gaps were found by checking the specification's own example
programs against its full rule set (`spec/AUDIT.md` A-01–A-03):

1. No rule ever sets an access path's `valid` field to `false` at frame
   exit. `rule.control.block-exit` (`spec/14-control-flow.md` §1)
   restricts `state.bindings` and reactivates suspended borrows, but
   never touches `state.access-paths` directly. A binding, borrow, or
   projection formed inside a frame therefore remains "live" forever,
   including after the frame that formed it has popped — which is
   unsound (a dangling access path reads as though still valid) and also
   over-restrictive (a stale access path can block a later, legitimate
   destroy of an object it once referenced, because `rule.resauth.destroy`
   correctly refuses to destroy an object with any other live access
   path). The specification's own `ex.destroy-while-borrowed` "corrected
   form" — `{ let v = Vec::new(); }` relying on scope end to destroy `v`
   after an earlier borrow's own scope has closed — is rejected by the
   rules as written.
2. Nothing gives a borrow formed directly in an expression position
   (most commonly, a reference-typed call argument, e.g. `&mut nums` in
   `push(&mut nums, 10)`) a *temporary* scope shorter than its enclosing
   frame. `rule.alias.borrow`'s `[Borrow-Reactivate]` fires only at
   `Block-Exit`, so two sequential exclusive-borrow calls on the same
   object in the same block — `push(&mut nums, 10); push(&mut nums,
   20);` — reject the second call: the first call's borrow is still
   "live" by every rule in force, because nothing ends it before the
   block itself ends.
3. `rule.agg.field-access` (`spec/16-aggregates.md` §1) forms a new
   access path from a struct's base access path without going through
   `rule.alias.borrow`'s conflict check, and without suspending the
   base. This leaves the base and every projection formed from it
   simultaneously "live" over the *same* object identity `o`, which
   `inv.alias-validity`'s proposition, as originally stated, has no
   exception for — a projection of an exclusive-mode base is, on the
   invariant's literal quantification, an unpermitted pair against its
   own base. `spec/08-alias-validity.md` §2's "by construction" claim
   for `inv.alias-validity` is therefore false as stated: `Field-Access`
   is a second access-path-introducing rule the claim does not account
   for.

These three are one problem, not three independent ones: all three are
instances of the same missing concept — CobaltC has never actually
decided how long an access path (as opposed to the object it targets)
remains valid, only how long the *object* does.

## Constraints

- §6: representation validity does not imply semantic validity; a stale
  access path whose target object still exists is still invalid the
  moment its own scope ends — the object's continued existence is not
  sufic ient license to keep using every path that once pointed at it.
- §9: reuse the mechanism already at hand (`Block-Exit`'s existing
  push/pop/fold shape, `rule.alias.borrow`'s existing suspend/reactivate
  shape) rather than inventing unrelated new machinery for what is, in
  each case, a narrower instance of "a scope ended; something formed in
  it needs cleaning up."
- §12: the boundary between what is caught early (statically, or by an
  earlier/finer-grained dynamic check) and what falls through to a
  later, coarser one must be a fixed rule, not a matter of
  implementation quality — this decision's job is to place two new,
  precisely-scoped checkpoints (statement exit, frame exit), not to
  leave "when does a temporary die" implementation-defined.
- §8 priority order: composability (a maximally permissive aliasing
  discipline) is subordinate to safety and semantic consistency; a
  conservative resolution of the base/projection relation that still
  rejects every case the invariant registry requires rejected, at the
  cost of also rejecting some cases a more elaborate mechanism could
  permit, is acceptable — and preferred over inventing a disjoint-range
  permission mechanism nothing currently forces (§9, §24).

## Candidate mechanisms

### For (1), frame-exit invalidation

1. **Leave `Block-Exit` as-is, rely on `Read`/`Write`'s dynamic
   `alive(o,Σ)` check alone.** Insufficient: `alive(o,Σ)` is about the
   *object*, not the access path; a stale access path to a still-live
   outer object (the escaping-projection case, `spec/AUDIT.md` A-01's
   `v.len` example) passes `alive` fine and is exactly the unsoundness
   in question. Rejected.
2. **Invalidate every access path formed in the exiting frame,
   unconditionally, at `Block-Exit`.** Simple, but would invalidate the
   very access paths `Block-Exit`'s own destruction sweep needs to use
   (an object's holding access path is typically formed in the same
   frame as the object itself). Rejected as stated; retained as the
   right *shape*, refined by option 3.
3. **Invalidate every access path formed in the exiting frame *except*
   each swept object's current holding access path, before the
   destruction sweep runs.** Removes exactly the stale paths (borrows,
   projections, superseded bindings) while leaving each object's
   `rule.resauth.destroy`-bound access path intact for the sweep that
   follows. Doing this *before* the sweep (rather than after) is what
   makes `{ let v = Vec::new(); let r = &v; }` sound: `r`'s borrow,
   formed in the same frame as `v`, is cleared first, so `rule.resauth.
   destroy`'s solitary-access precondition is satisfied when the sweep
   reaches `v`. **Selected.**

### For (2), temporary scope

1. **No temporary scope; require the programmer to bind every borrow to
   a name and let it live to frame end.** Rejected outright — this is
   the status quo the finding reports as unsound-by-over-restriction:
   `push(&mut nums, 10); push(&mut nums, 20);` becomes ill-formed, which
   contradicts §8's composability/comprehensibility priorities for a
   pattern this common.
2. **Tie a borrow's temporary scope to its enclosing function-call's own
   push/pop of the *callee's* frame.** Under-scopes: `f(&mut a) + g(&mut
   a)` within a single statement would let `g`'s borrow proceed the
   moment `f`'s call returns, silently permitting two temporaries of one
   full expression to be treated as non-overlapping when they are, in
   the ordinary reading of "temporary," simultaneously alive for the
   whole expression's duration. Rejected.
3. **A new, finer-grained scope stack (`state.temp-scope-stack`), pushed
   at each statement's entry and popped at its exit, mirroring
   `Block-Exit`'s reactivate-then-invalidate shape at statement
   granularity; a borrow is a *temporary* of its enclosing statement
   unless it is syntactically the bare top-level initializer of a `let`
   or assignment statement (in which case it is *durable*, governed only
   by ordinary frame-exit rules).** Matches "ending when the enclosing
   full expression/statement completes" exactly; the durable/temporary
   split is a static, syntactic fact (decidable from the statement's own
   AST shape), not a dataflow or reachability analysis. **Selected.**

The durable/temporary split's scope is intentionally narrow: it exempts
only the case where a borrow expression *is* (not merely appears within)
a `let`/assignment statement's entire initializer. A borrow nested inside
a call argument, an operator, or an aggregate-literal field (e.g. a
struct-literal field of reference type) is *not* exempted, and is
therefore invalidated at the enclosing statement's end even where a
finer analysis could in principle prove it durable. No example or
conformance case in this specification stores a reference inside an
aggregate field, so this is a real, named restriction rather than a
silently-handled gap — recorded under Revisit conditions, below, in the
same style as `feat.explicit-lifetime-parameters`.

### For (3), the base/projection relation

1. **`Field-Access` goes through the same suspend/reactivate discipline
   as `Borrow`.** Would work, but treats a projection as if it were a
   *second, independent* access to the object — which is semantically
   wrong (reading `v.len` is not a competing access against reading `v`
   itself; it is the same access, narrowed) and would suspend a
   struct's own base binding every time any field of it is merely
   accessed, which is far more restrictive than anything `spec/16`'s
   existing examples need or `inv.alias-validity` actually requires.
   Rejected.
2. **Revise `inv.alias-validity`'s proposition to exclude pairs related
   by a base/projection derivation chain from its pairwise
   quantification, and define conflicts between *distinct* projection
   families (two separately-formed access paths, even if their target
   sub-ranges happen to be disjoint) as ordinary pairwise conflicts,
   unchanged.** Matches the existing prose rationale in `spec/16` §1
   ("field access... is the same access, narrowed") by finally stating
   it as part of the invariant itself, and requires no change to
   `Field-Access`/`Index-Checked`'s already-`ACCEPTED`-shaped rule
   bodies. Does not admit simultaneous disjoint-field exclusive
   borrowing (`let r1=&mut p.a; let r2=&mut p.b;` remains rejected,
   conservatively) — nothing forces that finer discipline yet. **Selected.**

## Selected design

- **`state.access-paths`' `AccessPathRecord`** gains three fields
  (`spec/04-abstract-state.md`, still `PROVISIONAL`, in-place revision):
  `frame: Frame` (which frame's execution formed this access path),
  `temp-scope: TempScopeId` (which statement-level scope formed it), and
  `base: AccessPathToken?` (the immediate access path this one was
  projected from, `None` for a root access path formed by
  `Binding-Form`/`Borrow`/`Reclaim`/`Lock`).
- **`related(a1, a2, Σ) ≝ root(a1, Σ) = root(a2, Σ)`**, where `root`
  follows the `base` chain to its end. `inv.alias-validity`
  (`spec/03-invariants.md`) is revised:

      ∀ a1, a2, o. alive(o, Σ) ∧ (a1, a2 both access o simultaneously)
          ∧ ¬related(a1, a2, Σ)
          ⇒  permitted(a1, a2, o, Σ)

  `rule.alias.borrow`'s conflict scan (`spec/08-alias-validity.md` §2)
  is generalized to match: it excludes every `a'` with `related(a', a0,
  Σ)` from the scan (previously: only `a' = a0` literally), so that
  borrowing from a projection (`&mut p.a`) is not rejected merely
  because `p`'s own base binding, or a sibling projection's ancestor, is
  still nominally live. `rule.resauth.destroy`'s solitary-access check
  is deliberately **not** relaxed by `related`: destroying `p` while
  `p.a`'s projection is still live remains rejected — that check is
  about whether *any* other access path can still reach storage about
  to end, which `related` families do not exempt.
- **`state.temp-scope-stack: List(TempScopeId)`**, pushed at each
  statement's entry, popped at its exit, exactly mirroring
  `state.frame-stack`'s existing shape (`spec/04-abstract-state.md`).
  Every access-path-forming rule stamps the newly-formed record's
  `temp-scope` with `top(Σ.temp-scope-stack)`.
- **`rule.control.block-exit` (`spec/14-control-flow.md` §1)** is
  revised: after reactivating suspended borrows and *before* the
  destruction sweep, invalidate every access path formed in the exiting
  frame `f` except each swept object's current holding access path
  (`state.holder`, D-0016). This closes A-01.
- **A new statement-scope rule pair** (`spec/14-control-flow.md`, new
  §1a) pushes/pops `state.temp-scope-stack` around each statement in a
  block's `(statement)* expr?` sequence; on pop, it reactivates
  suspended borrows formed in that scope and invalidates every access
  path formed in that scope **unless** it is marked `durable` — a static
  fact set at formation time by whichever rule evaluates a statement's
  bare top-level `let`/assignment initializer, when that initializer is
  syntactically exactly `&e'`/`&mut e'` or a call whose return type is a
  reference (the D-0011 elision case, whose synthesized borrow is
  likewise the statement's own bound value, not a nested temporary).
  This closes A-02.
- **`inv.alias-validity`'s revised proposition** (above) closes A-03,
  with no change required to `rule.agg.field-access` itself.

## Rejected alternatives

Frame-exit: no new invalidation (1); unconditional invalidation without
protecting the sweep's own holding paths (2). Temporary scope: none (1);
call-boundary-scoped (2). Base/projection: suspend/reactivate parity with
`Borrow` (1).

## Semantic rationale

All three fixes are the same move at different granularities: a scope
(frame, statement) ending invalidates what it formed, except what a
concurrent, coarser-grained mechanism (the destruction sweep) still needs
in the same instant. The base/projection fix is different in kind — it
is not a new invalidation point but a correction to what "simultaneous
access" the invariant was ever supposed to be quantifying over — but is
grouped with the other two because all three were found by the same
process (checking whether an access path outlives what it should) and
because the frame-exit fix's own `Held`/holder computation
(D-0016) is what the base/projection fix's `related` family also relies
on to state precisely which access path a destroy sweep is entitled to
use.

## Usability implications

Sequential exclusive-borrow calls in one block need no restructuring.
Reborrows and projections through an owned struct's fields work without
an explicit intermediate binding. The one added authoring cost: a
reference stored inside an aggregate-literal field does not yet survive
past its own construction statement (see Revisit conditions) — a real,
narrow restriction, not present in any example this specification
currently ships.

## Explainability implications

A rejected reuse of a stale access path now has a precise story: it was
invalidated at [statement end | frame end], distinguishable in a
diagnostic from the pre-existing `diag.stale-binding` cases (object end,
transfer) by which invalidating event fired.

## Implementation-feasibility implications

Both new invalidation points are structural: a fixed set of push/pop
events already present in any block-structured evaluator (one per
frame — already required; one per statement — a strict refinement of the
same idea). The `related` computation is a bounded parent-pointer walk.
No new whole-program analysis is required for any of the three.

## Compatibility impact

Revises `rule.control.block-exit` (`spec/14-control-flow.md`, still
`PROVISIONAL`), `rule.alias.borrow` (`spec/08-alias-validity.md`, still
`PROVISIONAL`), and `inv.alias-validity` (`spec/03-invariants.md`, no
individual Status field yet assigned — treated as `PROVISIONAL` per
`spec/AUDIT.md` A-14, in-place revision, not itself an `ACCEPTED` entity
being edited). Does not reverse any `ACCEPTED` Decision's selected
design: D-0003's single-holder authority mechanism, D-0004's
formation-time `Borrow` check, and D-0008's automatic-destruction trigger
are all unchanged in shape; this decision only adds the missing
scope-exit and base/projection bookkeeping those decisions already
presupposed but left to later artifacts.

## Prior-art status

Full-expression temporary lifetime (2) and scope-based invalidation are
independently re-derived here from `inv.temporal-validity`'s own
"invalidated when its scope ends" requirement; that similarly-shaped
mechanisms exist elsewhere is not the reason for the choice (§7).

## Invariant traceability

Directly closes the A-01/A-02 gaps in `inv.temporal-validity`'s
enforcement and the A-03 gap in `inv.alias-validity`'s "by construction"
claim (`spec/08-alias-validity.md` §2, corrected alongside this
decision).

## Revisit conditions

Revisit the base/projection exemption's narrowness if a future artifact
needs simultaneous exclusive borrows of disjoint fields of the same
struct (would need a sub-range-disjointness refinement to `related`, not
designed here). Revisit the durable/temporary split if a future artifact
needs a reference-typed aggregate-literal field to survive past its own
construction statement (would need `durable` to propagate through
aggregate construction the way it already does through `let`/assignment,
not designed here — this is the one named limitation this decision
leaves open, analogous in status to `feat.explicit-lifetime-parameters`).
