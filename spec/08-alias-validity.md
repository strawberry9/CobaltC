# CobaltC Alias-Validity and Concurrent Access

Status: normative artifact
Version: 1.8.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.alias.*`)
Governed by: `CobaltC_Master_Instructions.md` §6, §17 (Aliasing and
Concurrent Access), §23
Realizes: D-0004 (modes, `permitted`), D-0018 (families, use-time
checking)

## 1. Grounding

    Mode ::= shared | exclusive                      -- D-0004
    permitted(m1, m2)  ≝  m1 = shared ∧ m2 = shared
    clash(a, m, Σ)     ≝  ∃ a'. temporally-valid(a',Σ) ∧ ¬pending(a',Σ) ∧ of(a',Σ) = of(a,Σ)
                             ∧ a' ∉ ancestors(a,Σ) ∧ overlap(a,a',Σ)
                             ∧ ¬permitted(m, mode(a',Σ)) ∧ ¬sync-exempt(a, m, a', Σ)
                                                                    -- spec/04 §2, the definition; `pending`
                                                                    -- D-0107 and D-0242, `sync-exempt` spec/19 §2

`inv.alias-validity` (`spec/03`) is the proposition that every access
of mode `m` through `a` satisfies `¬clash(a, m, Σ)` at the moment of
the access, and, when `a` is exclusive, that `a` is the accessing
thread's (`[Thread-Affine]`, below, D-0213). `synchronized` is grounded in `spec/04` §2 and extended
for mutex-guarded objects in `spec/19` §2.

**Capability.** `shared` permits read; `exclusive` permits read, write,
and (with authority) destroy. A path's mode is fixed at formation.

## 2. Rules

### `rule.alias.borrow`
**Status:** ACCEPTED

    [Borrow]
        temporally-valid(a0, Σ);  m ∈ Mode
        m = exclusive ⇒ mode(a0,Σ) = exclusive
        ¬clash(a0, m, Σ)
        a_new ∉ dom(Σ.access-paths)
        ────────────────────────────────────────────
        ⟨borrow(a0, m), Σ⟩ →^ℓ
            ⟨a_new, Σ[ access-paths(a_new) := { target: target(a0,Σ), of: of(a0,Σ), type: type(a0,Σ),
                                                 mode: m, thread: ℓ, valid: true,
                                                 formed-at: this-event,
                                                 frame: current-frame(ℓ,Σ), temp-scope: current-scope(ℓ,Σ),
                                                 base: a0, held-by: ∅ } ]⟩
        side-conditions:
            ⟦ temporally-valid(a0, Σ) ⟧ discharge: static (rule.control.flow-analysis; dynamic where unknown)
            ⟦ m = exclusive ⇒ mode(a0,Σ) = exclusive ⟧ discharge: static
            ⟦ ¬clash(a0, m, Σ) ⟧ discharge: static (rule.control.flow-analysis, rule.control.frame-references; dynamic where unknown)

    [Borrow-Stale]            disposition: checked   ¬temporally-valid(a0, Σ)                    ⟨borrow(a0, m), Σ⟩ ↛ diag.stale-binding
    [Borrow-Exceeds-Source]   disposition: rejected  m = exclusive ∧ mode(a0,Σ) = shared         ill-formed; diag.borrow-exceeds-source
    [Borrow-Denied]           disposition: checked   clash(a0, m, Σ)                             ⟨borrow(a0, m), Σ⟩ ↛ diag.aliasing-conflict

`Borrow` derives a new access path of mode `m` from `a0`, recording
`a0` as its `base`. An exclusive path can only be derived from an
exclusive one (`spec/AUDIT-2.md` B-15), and an exclusive reference
stored in what a shared path reaches is read out as a shared one
(`rule.alias.read-through-shared`, D-0218), so a `ref<τ, shared>` value
can never be a source of mutation, at any depth — this is the interface
guarantee Master Instructions §17 requires of a shared-reference
parameter. The
formation-time `clash` check is the early diagnostic: it rejects
`auto r2 = &mut x` while `auto r1 = &x` lives, at the borrow, rather
than at the first use. Nothing is suspended: `a0` and its ancestors
remain valid, and their later uses are governed by the same `clash`
predicate at `rule.value-object.read`/`write` — a parent may read
while a shared child lives, and may do nothing that conflicts with an
exclusive child until that child is invalid (D-0018).

**Depends on:** inv.alias-validity, inv.temporal-validity, term.alias,
term.access-path, D-0004, D-0018, rule.control.flow-analysis
**Affects:** state.access-paths

### `rule.alias.thread-affine`
**Status:** ACCEPTED

    [Thread-Affine]   disposition: checked   -- D-0213: a premise of every rule that accesses through a
                                             -- path or borrows from one ([Read], [Write], [Borrow], the
                                             -- projections, and through [Write] the std-only intrinsics
                                             -- [Swap-Places] and [Lend-Places], D-0235), beside ¬clash
        a step of thread ℓ accesses through a, or forms a [Borrow] from a;  mode(a, Σ) = exclusive
        ¬affine(a, ℓ, Σ)
        ────────────────────────────────────────────
        that step ↛ diag.aliasing-conflict

    affine(a, ℓ, Σ)  ≝  thread-of(a,Σ) = ℓ ∨ thread-of(a,Σ) = transit ∨ ¬running(thread-of(a,Σ), Σ)

    -- A step that passes with thread-of(a,Σ) ≠ ℓ also sets access-paths(a).thread := ℓ: a path lent
    -- at a thread boundary (spec/19 [Thread-Lend]) is taken by the first thread that accesses through
    -- it, and a path of a thread that has ended by the next thread that does.

An exclusive reference is freely copyable (`type.ref`), and every copy
carries the same path, which `clash` cannot tell from itself: two copies
in one thread are used one after the other, but two copies in two
threads would be two unsynchronized writers no `clash` sees. So an
exclusive path belongs to one thread — the one that formed it
(`[Borrow]`, `rule.conc.lock`), or the one it was lent to — and only that
thread accesses through it. A reference reaches another thread lent:
`spawn` lends the exclusive references among its arguments and a `move`
closure's captures, and `Channel::send` those in the value it carries
(`spec/19` `[Thread-Lend]`), each replaced by a path borrowed from it,
which the sender's own copies then clash with while it lives. An
exclusive reference that reaches another thread any other way — stored in
a struct the thread is lent a reference to, in a `Vec` moved to it, in a
`mutex` it locks — is the forming thread's, and an access through it
faults while that thread runs (`conf.exclusive-ref-of-another-thread`;
reached through a shared reference, a read of it borrows from it and a
write is rejected statically, `rule.alias.read-through-shared`).
Shared paths have no such restriction: two threads reading through one
shared path do not conflict.

**Depends on:** rule.alias.borrow, state.access-paths, state.threads, D-0213
**Affects:** state.access-paths

### `rule.alias.read-through-shared`
**Status:** ACCEPTED

    via-shared(e)  ≝  e is a place reached through a shared path: `*r` with r : ref<τ, shared>;
                      a field, element or payload of such a place (spec/16 [Field-Access-Auto-Deref] included);
                      or `*r` with r itself read from such a place                     -- D-0218

    [Read-Ref-Through-Shared]   (D-0218)
        via-shared(e);  e : ref<τ, exclusive> (or slice<τ, exclusive>), read as a value — bound, passed,
        returned, stored, compared, or dereferenced (`*e`)
        ⟨e, Σ⟩ →* ⟨a, Σ1⟩                                     -- the stored reference
        ⟨borrow(a, shared), Σ1⟩ →^ℓ ⟨a', Σ2⟩                    -- [Borrow]: ¬clash, [Thread-Affine] on a
        ────────────────────────────────────────────
        ⟨e, Σ⟩ →^ℓ ⟨a' : ref<τ, shared> (slice<τ, shared>), Σ2⟩
        side-conditions:
            ⟦ the value's type ⟧ discharge: static (rule.type.typing: e read as a value has type ref<τ, shared>)

    [Read-Exclusive-Through-Shared]   disposition: rejected   (D-0218)
        via-shared(e);  e read as a value (a copy, spec/05 [Read]);  τ = type(e) holds an exclusive
        reference or slice — a field, an array element or a payload, at any depth, not behind a reference
        ────────────────────────────────────────────
        ill-formed; diag.borrow-exceeds-source
        -- also: a `match` by value on such a place whose arm binds such a payload (spec/16 [Match]);
        -- `clone(r)` of such a τ (spec/12 clone-holds: an exclusive reference is not `clone`)

    [Read-Fn-Through-Shared]   disposition: checked   (D-0218)
        via-shared(e);  type(e) holds `fn` values;  e read as a value, or called in place
        (spec/15 [Closure-Call]);  some closure in the value has a capture that is or holds an
        exclusive reference (rule.fn.closure: a borrow capture the body writes, or a moved exclusive one),
        or writes itself (spec/15 writes-self: a `move` closure whose body writes a capture, D-0239)
        ────────────────────────────────────────────
        ⟨e, Σ⟩ ↛ diag.borrow-exceeds-source

What a shared reference reaches is read-only at every depth: the
guarantee of `rule.alias.borrow` holds of the object a shared reference
reaches and of everything reached from it through references the object
holds. `[Borrow-Exceeds-Source]` stops an exclusive borrow *from* a
shared path; these rules stop an exclusive reference *stored* in what a
shared path reaches from being used as one. Read out, it is a shared
borrow of the stored path, so `*s.inner = 1` is `[Write-Not-Exclusive]`
and `&mut *s.inner` is `[Borrow-Exceeds-Source]`, both static, while
`*s.inner + 1`, `auto r = s.inner;` (a `ref<τ, shared>`) and passing
`s.inner` where a shared reference is expected stay legal. A copy that
would carry an exclusive reference out (`S t = *s;`, a payload bound by
value, `clone`) is rejected. A `fn` type does not show its closure's
captures, so that case is checked where the value is read or called: a
closure that writes through a captured exclusive reference, stored in a
struct reached through a shared reference, faults instead of writing.
Mutation through a shared handle has two routes, `mutex<_>` (`spec/19` §2):
a lock path is exclusive whatever reference reached the mutex; and
`Atomic<_>` (`spec/19` §2a, D-0249), whose operations form no path and
reach only an integer no path targets.

**Depends on:** rule.alias.borrow, rule.alias.thread-affine, rule.ref.deref, rule.fn.closure, D-0218
**Affects:** state.access-paths

## 3. Where `inv.alias-validity` is discharged

Five rules form access paths:

| Rule | `base` | Check at formation | Check at use |
|---|---|---|---|
| `rule.value-object.binding-form` | `None` | none needed: `o` is fresh or a temporary with no other path | `[Read]`/`[Write]` |
| `rule.alias.borrow` | `a0` | `¬clash(a0, m)` | `[Read]`/`[Write]` |
| `rule.agg.field-access`, `rule.agg.index` | `a0` | none: inherits `a0`'s mode, narrows its target | `[Read]`/`[Write]` |
| `rule.trust.rawptr` | `None` | asserted (`discharge: trusted`) | `[Read]`/`[Write]` |
| `rule.conc.lock` | `None` | mutual exclusion (`spec/19` §2) | `[Read]`/`[Write]` |

The invariant's guarantee rests on the use-time checks in
`rule.value-object.read` (`¬clash(a, shared)`) and
`rule.value-object.write` (`¬clash(a, exclusive)`), which every access
passes through — `rule.ref.deref`, `rule.agg.match`,
`rule.fn.bind-param`, and every intrinsic reduce to them; the two
std-only intrinsics that write a place they are handed, `[Swap-Places]`
(beneath `swap`, `replace`, `overwrite`, `Vec::swap`) and
`[Lend-Places]`, take `[Write]`'s premises for it (`spec/21` §3b,
D-0235: before D-0235 they did not, and a reference passed on whole to
`swap` changed what a live shared reborrow of it reached). The
formation-time check in `[Borrow]` and the projection rules' lack of
one are therefore not what soundness depends on; they only decide how
early a conflict is reported. This replaces the earlier "by
construction, because `Borrow` is the sole path-introducing rule"
argument, which `spec/AUDIT.md` A-03 and `spec/AUDIT-2.md` B-01 showed
did not hold.

## 4. Consequences elsewhere

- `rule.resauth.destroy` and `rule.resauth.transfer` require
  `solitary(a)`: no other valid path on the object at all.
- Two exclusive borrows of *disjoint* fields coexist (`overlap` is by
  target, or by steps where a vector's storage is reached, `spec/04` §2,
  D-0238), closing `spec/AUDIT-STATUS.md` A-33.
- A reference stored in an aggregate or closure lives with its
  container (D-0018), closing A-32.
- **`[Standing-Local]`: a path's standing changes only in its own thread**
  (D-0188, D-0209). Let `a` be a path held by thread `ℓ` such that:
  - (H1) `a` is not lock-derived: its standing is not `sync-exempt`'s
    (`spec/19` §2), which another thread's `[Lock]` changes;
  - (H2) `a` is not pending (D-0107): it has been checked against the
    other arguments of its call;
  - (H3) `temporally-valid(a, Σ)` and `¬clash(a, mode(a), Σ)`;
  - (H4) if `a` is a slice of a `Vec` (`steps(a)` ends in a `range(i, j)`
    step, `spec/04` §2, D-0238), no element at a position in `[i, j)` has a
    live object (one `[Reclaim]` gave it): an access through the slice also
    meets that element's object, and `clash(a, ·)` concerns `of(a)`, the
    vector's object, only;
  - (H5) `storage-kind(of(a)) = fresh`, or `ℓ` is the only live thread;
  - (H6) `ℓ` itself forms no path from `a`, hands `a` on, and does not
    leave the frame or scope that holds `a`.

  Then after any step `→^ℓ'` of a thread `ℓ' ≠ ℓ`, from `Σ` to `Σ'`,
  `temporally-valid(a, Σ')` and `¬clash(a, mode(a), Σ')` still hold,
  except for an instant, through a projection `ℓ'` forms for one access,
  which that access faults on. By the kind of `ℓ'`'s step:
  - `[Borrow]`: a lasting path that would clash with `a` is refused —
    `[Borrow-Denied]` sees `a` (§1).
  - a projection (`rule.agg.field-access`, `rule.agg.index`): formed
    unchecked, but from a path `ℓ'` holds; it cannot conflict with `a`
    unless it is an ancestor of `a`'s, and then it exists only for its
    access, pending until then (D-0242), and that access clashes with `a`
    and faults (`[Read-Conflict]`,
    `[Write-Conflict]`); `a`'s own accesses read and write cells the
    projection has not touched, so they pass in the interleaving that
    runs them before it (`[Thread-Step]`).
  - `[Read]`, `[Write]` through `ℓ'`'s own paths: cells of other objects,
    or of `a`'s under a path whose `[Borrow]` was refused — `a` unchanged.
  - `[Destroy]`, `[Authority-Transfer]`, `[Relocate-Out]`,
    `[Store-Binding-Place-Transfer]` — ending or moving `a`'s object:
    require `solitary(of(a))`, which `a` refutes
    (`[Destroy-Not-Solitary]`, `[Authority-Transfer-Aliased]`).
  - `[Write-Quiet-Replace]`, an overwrite of `a`'s object: needs an
    exclusive path of `ℓ'` overlapping `a`, refused as above.
  - `[Thread-Affine]` (D-0213): `ℓ'` accesses through no copy of `a`
    itself, nor through a copy of an exclusive ancestor of `a` that is
    `ℓ`'s: either is `ℓ`'s while `ℓ` runs.
  - `[Release]`, `[Deallocate]` (`rule.trust.rawptr`,
    `rule.stdlib.prelude`): end every reclaimed object over the released
    cells, and need no `solitary`. This is what H5 excludes. (Since
    D-0214, `[Release]` faults instead when the path is another thread's
    and exclusive — `foreign-held`, `spec/20` §2 — so H5 is needed only for
    shared paths and for paths of `ℓ`'s own.) An object
    over raw storage — a `Vec`'s element, a map's entry — is ended by
    whoever holds an exclusive path to its container (`Vec::push`'s
    `grow`, `pop`, `clear`, a rehash), and that path does not clash with
    `a`: the container's own cells and the element's are disjoint. A
    fresh-storage object has no such owner: it ends with its frame, in
    `ℓ`, or by `[Destroy]` above. When `ℓ` is the only live thread as `a`
    arrives, no other thread can hold or form such a path while `ℓ` stays
    in the frame: a thread `ℓ` spawns meanwhile receives only what `ℓ`
    passes it, never the container (H6).
  - `[Lock]`, `[Guard-Drop]`: change a mutex's `sync-state`; `a` is not
    lock-derived (H1).
  - `[Spawn]`: its arguments are held from the spawn, each formed by
    `[Borrow]` above.
  - `[Join]`, `[Thread-Body-Done]`, `[Handle-Destructor]`,
    `[Fault-Unwind]`, `[Fault-Contain]`, `[Cancel]`, `[Cancelled-Wait]`:
    end or unwind the frames of `ℓ'`, or of a thread it joins; `a` is held
    in `ℓ`'s frame (H6), which none of them touches; a program-wide
    termination takes no further step anywhere.
  - `[Rawptr-Write]`, `[Rawptr-Move-In]`, `[Copy-Raw]` (trusted): write
    no cell a live path reaches, by their side-conditions — the trusted
    library base's claims (`spec/21` §0), true in a conforming
    implementation.

  So an implementation may check such paths once, where they arrive (a
  function's reference arguments at its entry, after every argument of
  the call is evaluated, so that a lasting path a later argument forms —
  a lend to a thread spawned among the arguments — is seen there,
  `CHG-0342`: valid, clash-free, not
  lock-derived, pairwise disjoint where one is exclusive, pending ones
  checked against one another, a slice's range free of live objects, and
  — unless the thread is alone — not over raw storage), and omit
  `[Read]`/`[Write]`'s use-time `clash` test on them while H6 holds. A
  program run this way gives a result the rules admit, and every
  conflict involving such a path is still diagnosed, at the other
  thread's access. Before D-0209 this property was stated without H5,
  and `cobc` relied on it for an element reference while another thread
  grew the vector (`conf.direct-param-element-grown-by-other-thread`).

## Change Log

- 1.8.0 — `CHG-0373` (D-0282): `[Borrow]`'s `¬clash` side-condition is also discharged by `rule.control.frame-references` (spec/14 §6), which refutes conflicts of frame-held references across calls before the run.
- 1.7.2 — `CHG-0342`: §4's closing paragraph: the arrival test is made at the entry, after every argument. Clarification.
- 1.7.1 — `CHG-0306` (D-0249): `rule.alias.read-through-shared`'s prose names `Atomic` as the second route. Clarification.
- 1.7.0 — `CHG-0294` (D-0242): §1's `pending` and §4's projection bullet: a path formed for one access is pending until it.
- 1.6.0 — `CHG-0284` (D-0235): `[Thread-Affine]`'s list and §3 name `[Swap-Places]` and `[Lend-Places]`; `CHG-0287` (D-0238): §4's overlap sentence and H4 by the `range` step; `CHG-0288` (D-0239): `[Read-Fn-Through-Shared]` also for a self-writing closure.
- 1.5.1 — `CHG-0279` (D-0233): §1 restates `clash` with `¬pending` and `¬sync-exempt`, as `spec/04` §2 defines it.
- 1.5.0 — `CHG-0257` (D-0218): `rule.alias.read-through-shared`
  (`[Read-Ref-Through-Shared]`, `[Read-Exclusive-Through-Shared]`,
  `[Read-Fn-Through-Shared]`): what a shared reference reaches is read-only
  at every depth; `rule.alias.borrow`'s interface sentence and
  `[Thread-Affine]`'s prose cite it. Programs that wrote through an
  exclusive reference reached through a shared one are now rejected.
- 1.4.1 — `CHG-0252` (D-0214): `[Standing-Local]`'s `[Release]` case
  notes that another thread's exclusive path now makes the release fault.
- 1.4.0 — `CHG-0251` (D-0213): `rule.alias.thread-affine`
  (`[Thread-Affine]`): an exclusive path is accessed only by the thread it
  belongs to; a lent path is taken by the first thread to access through
  it, an ended thread's by the next. §1 and `[Standing-Local]` cite it.
- 1.3.0 — `CHG-0247` (D-0209): §4's property stated as the lemma
  `[Standing-Local]` with its hypotheses listed and a case per transition
  kind; the new hypothesis H5 — the path's object is fresh-storage, or its
  thread is alone — excludes a `[Release]` of a reclaimed object by another
  thread, which needs no `solitary`. No rule changed.
- 1.2.0 — `CHG-0216` (D-0188): §4 "A path's standing changes only in
  its own thread", the property an implementation relies on when it
  checks reference arguments once at entry (`cobc`'s dual bodies). A
  consequence of the existing rules; no rule changed.
- 1.1.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"): §3
  cited `rule.ref.deref-*`; the rule is `rule.ref.deref`.
- 1.1.0 — `CHG-0001`: illustrative fragment re-spelled to `spec/22`
  2.0.0 syntax (`auto` instead of `let`); no rule semantics changed.
- 1.0.0 — Rewritten per D-0018 (`spec/AUDIT-2.md` B-01, B-02, B-03,
  B-15): `[Borrow]` no longer suspends its source, requires mode
  monotonicity (`diag.borrow-exceeds-source`), and checks `clash`
  against non-ancestor paths; `[Borrow-Reactivate]` and
  `state.suspended` removed; §3 restates where the invariant is
  discharged. All entities `ACCEPTED`.
- 0.3.0 and earlier — superseded.
