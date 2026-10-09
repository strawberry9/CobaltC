# CobaltC Alias-Validity and Concurrent Access

Status: normative artifact
Version: 1.2.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.alias.*`)
Governed by: `CobaltC_Master_Instructions.md` §6, §17 (Aliasing and
Concurrent Access), §23
Realizes: D-0004 (modes, `permitted`), D-0018 (families, use-time
checking)

## 1. Grounding

    Mode ::= shared | exclusive                      -- D-0004
    permitted(m1, m2)  ≝  m1 = shared ∧ m2 = shared
    clash(a, m, Σ)     ≝  ∃ a'. temporally-valid(a',Σ) ∧ of(a',Σ) = of(a,Σ)
                             ∧ a' ∉ ancestors(a,Σ) ∧ overlap(a,a',Σ)
                             ∧ ¬permitted(m, mode(a',Σ))            -- spec/04 §2

`inv.alias-validity` (`spec/03`) is the proposition that every access
of mode `m` through `a` satisfies `¬clash(a, m, Σ)` at the moment of
the access. `synchronized` is grounded in `spec/04` §2 and extended
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
            ⟦ ¬clash(a0, m, Σ) ⟧ discharge: static (rule.control.flow-analysis; dynamic where unknown)

    [Borrow-Stale]            disposition: checked   ¬temporally-valid(a0, Σ)                    ⟨borrow(a0, m), Σ⟩ ↛ diag.stale-binding
    [Borrow-Exceeds-Source]   disposition: rejected  m = exclusive ∧ mode(a0,Σ) = shared         ill-formed; diag.borrow-exceeds-source
    [Borrow-Denied]           disposition: checked   clash(a0, m, Σ)                             ⟨borrow(a0, m), Σ⟩ ↛ diag.aliasing-conflict

`Borrow` derives a new access path of mode `m` from `a0`, recording
`a0` as its `base`. An exclusive path can only be derived from an
exclusive one (`spec/AUDIT-2.md` B-15), so a `ref<τ, shared>` value can
never be a source of mutation — this is the interface guarantee Master
Instructions §17 requires of a shared-reference parameter. The
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
`rule.fn.bind-param`, and every intrinsic reduce to them. The
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
  target), closing `spec/AUDIT-STATUS.md` A-33.
- A reference stored in an aggregate or closure lives with its
  container (D-0018), closing A-32.
- **A path's standing changes only in its own thread** (D-0188). Let a
  path `a` held by thread `ℓ`, not lock-derived, be temporally valid
  with `¬clash(a, mode(a), Σ)`. Until `ℓ` itself forms a path from
  `a`, hands `a` on, or leaves the frame or scope that holds it, no step
  of another thread makes `a` invalid or makes `clash(a, ·)` hold, except
  for an instant, through a projection that the other thread's own access
  then faults on. Ending `a` from outside (destroying or moving its object)
  needs `solitary`, which `a` refutes (above). A lasting path that would
  clash with `a` is formed by `[Borrow]`, whose check sees `a`
  (`[Borrow-Denied]` in the forming thread). The projections
  (`rule.agg.field-access`, `rule.agg.index`) form a path unchecked, but
  only from one the other thread holds. That path cannot conflict with `a`,
  so `¬clash(a)` holds, unless it is an ancestor of `a`. The projection
  then exists only for the access it is formed for, and that access
  clashes with `a` and faults. `a`'s own accesses read and write plain
  cells the projection has not touched, so they pass in the interleaving
  that runs them before the projection (`[Thread-Step]`).

  So an implementation may check such paths once, where they arrive (a
  function's reference arguments at its entry), and omit `[Read]`/`[Write]`'s
  use-time `clash` test on them while the condition holds and its own
  thread does nothing through them that could change their standing. A
  program run this way gives a result the rules admit, and every
  conflict involving such a path is still diagnosed, at the other
  thread's access. Arguments
  that are all pending (D-0107) are not yet checked against one another,
  so the entry check must also find them pairwise free of clash. A
  lock-derived path is excluded: its standing is `sync-exempt`'s
  (`spec/19` §2), which another thread's `[Lock]` changes. An access
  through a slice also reaches the objects that lie over the cells it
  covers (a `Vec`'s elements that have been given objects, reclaimed in
  its storage), and `clash(a, ·)` concerns `a`'s own object only: those
  objects' paths are met at the access, not when the slice is formed. So
  the entry check must also find no live object over a slice's cells.

## Change Log

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
