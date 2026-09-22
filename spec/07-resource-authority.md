# CobaltC Resource Authority and Destruction

Status: normative artifact
Version: 1.2.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.resauth.*`)
Governed by: `CobaltC_Master_Instructions.md` §5, §17 (Resource
Authority), §23
Realizes: D-0003, D-0008 (via `spec/14`), D-0016, D-0019

## Purpose

Defines transfer, relocation, and destruction of destroy-capable
authority for the single-holder discipline D-0003 selected, including
composite sub-resources and user-declared destructors. Establishment
(the grant) is `rule.value-object.object-establish`; the automatic
trigger at scope end is `rule.control.block`/`stmt-exit`.

## 1. Destructors

A type `τ` **has a destructor** iff `Σ.items` contains an associated
function `τ::drop` (`spec/17` §1) declared `fn τ::drop(ref<τ, exclusive>
self)` with no return type — one exclusive-reference parameter, result
`unit` (`spec/22` §3). `destructor(τ)` denotes that item or
`None`. Declaring `drop` with any other signature is ill-formed
(`diag.bad-destructor-signature`, static). A destructor is only ever
invoked by `[Run-Destructor]` below (from `[Destroy]` for a whole
resource, from `[Destroy-Composite]` for a sub-resource) — never called
directly (a direct call
`τ::drop(...)` is ill-formed, `diag.direct-destructor-call`, static) —
so every resource's type-specific cleanup runs exactly once, at the
point its authority is consumed. For the built-in resource types
`handle<τ>`, `mutex<τ>`, and `guard<τ>` (`spec/19`), `destructor(τ)`
denotes the built-in rule — `[Handle-Destructor]`, the mutex's
`destroy-composite` of `inner` (nothing else), `[Guard-Drop]` — and
`[Run-Destructor]` applies that rule in place of the borrow-and-call.

## 2. Transfer

### `rule.resauth.transfer`
**Status:** ACCEPTED

    [Authority-Transfer]
        temporally-valid(a1, Σ);  o = of(a1,Σ);  target(a1,Σ) = extent(o,Σ)
        authority(ℓ, destroy, o, Σ)
        a2 ∈ dom(Σ.access-paths), of(a2,Σ) = o, base(a2,Σ) = None,
            a2 formed in thread ℓ2 (ℓ2 = ℓ except for rule.conc.spawn)
        ∀ a' ∉ {a1, a2}. temporally-valid(a',Σ) ⇒ of(a',Σ) ≠ o      -- "solitary(a1) modulo a2"
        ────────────────────────────────────────────
        ⟨transfer(a1, a2), Σ⟩ →^ℓ
            ⟨(), Σ[ authority(ℓ, destroy, o).consumed := true      if ℓ2 ≠ ℓ,
                    authority(ℓ2, destroy, o) := {consumed: false} if ℓ2 ≠ ℓ,
                    access-paths(a1).valid := false,
                    holder(o) := a2 ]⟩
        side-conditions:
            ⟦ temporally-valid(a1,Σ) ∧ authority(ℓ, destroy, o, Σ) ⟧
                discharge: static (rule.control.flow-analysis; dynamic where unknown)
            ⟦ solitary(a1, Σ) ⟧
                discharge: static (rule.control.flow-analysis; dynamic where unknown)

    [Authority-Transfer-Unauthorized]   disposition: checked
        ¬temporally-valid(a1,Σ) ∨ ¬authority(ℓ, destroy, o, Σ)
        ────────────────────────────────────────────
        ⟨transfer(a1, a2), Σ⟩ ↛ diag.transfer-without-authority

    [Authority-Transfer-Aliased]   disposition: checked
        ∃ a' ∉ {a1, a2}. temporally-valid(a',Σ) ∧ of(a',Σ) = o
        ────────────────────────────────────────────
        ⟨transfer(a1, a2), Σ⟩ ↛ diag.move-while-aliased

Transfer moves destroy authority and ownership of `o` from root path
`a1` to root path `a2` (a fresh binding, `rule.value-object.store`
`[Store-Binding-Place-Transfer]`, or a spawned thread's parameter).
`o`'s identity, origin, and storage are untouched; `a1` is invalidated,
which is what makes this a move. Moving while any other path reaches
`o` is rejected: those paths would otherwise outlive the binding they
were derived from with nothing to invalidate them (D-0018). Within one
thread the authority token is unchanged — the performer is the thread
(`spec/04`), so only the holder changes.

**Depends on:** inv.resource-authority, inv.temporal-validity,
inv.alias-validity, inv.identity, D-0003, D-0016, D-0018,
rule.control.flow-analysis
**Affects:** state.authority, state.access-paths, state.holder

## 3. Destruction

### `rule.resauth.destroy`
**Status:** ACCEPTED

    [Destroy]
        temporally-valid(a, Σ);  o = of(a,Σ);  target(a,Σ) = extent(o,Σ);  τ = type-of(o,Σ)
        authority(ℓ, destroy, o, Σ)
        solitary(a, Σ)
        Σ1 = Σ[ authority(ℓ, destroy, o).consumed := true,
                destruction-obligations := Σ.destruction-obligations \ {o} ]
        ⟨run-destructor(a, τ), Σ1⟩ →^ℓ ⟨(), Σ2⟩                          -- below
        ⟨destroy-composite(o), Σ2⟩ →^ℓ ⟨(), Σ3⟩                           -- rule.resauth.destroy-composite
        ⟨end-object(o), Σ3⟩ →^ℓ ⟨(), Σ4⟩                                  -- rule.value-object.object-end
        ────────────────────────────────────────────
        ⟨destroy(a), Σ⟩ →^ℓ ⟨(), Σ4⟩
        side-conditions:
            ⟦ temporally-valid(a,Σ) ∧ authority(ℓ, destroy, o, Σ) ⟧
                discharge: static (rule.control.flow-analysis; dynamic where unknown)
            ⟦ solitary(a, Σ) ⟧
                discharge: static (rule.control.flow-analysis; dynamic where unknown)

    [Run-Destructor-None]     destructor(τ) = None          ⟨run-destructor(a, τ), Σ⟩ →^ℓ ⟨(), Σ⟩
    [Run-Destructor]
        destructor(τ) = d
        ⟨borrow(a, exclusive), Σ⟩ →^ℓ ⟨a_d, Σ1⟩                           -- rule.alias.borrow; succeeds: solitary(a)
        ⟨call(d, [a_d]), Σ1⟩ →^ℓ ⟨(), Σ2⟩                                  -- rule.fn.call
        ────────────────────────────────────────────
        ⟨run-destructor(a, τ), Σ⟩ →^ℓ ⟨(), Σ2[ access-paths(a_d).valid := false ]⟩

    [Destroy-Plain]           -- drop of a non-resource object: ends it, nothing else to do
        temporally-valid(a, Σ);  o = of(a,Σ);  target(a,Σ) = extent(o,Σ);  ¬is-resource(type-of(o,Σ));  solitary(a, Σ)
        ⟨end-object(o), Σ⟩ →^ℓ ⟨(), Σ'⟩
        ────────────────────────────────────────────
        ⟨destroy(a), Σ⟩ →^ℓ ⟨(), Σ'⟩

    [Destroy-Stale]           disposition: checked   ¬temporally-valid(a, Σ)          ⟨destroy(a), Σ⟩ ↛ diag.stale-binding
    [Destroy-No-Authority]    disposition: checked   is-resource ∧ ¬authority(ℓ, destroy, o, Σ)   ⟨destroy(a), Σ⟩ ↛ diag.no-destroy-authority
    [Destroy-Not-Solitary]    disposition: checked   ∃ a' ≠ a live on o               ⟨destroy(a), Σ⟩ ↛ diag.destroy-while-aliased
    [Destroy-Projection]      disposition: rejected  target(a,Σ) ⊂ extent(o,Σ)          ill-formed; diag.move-out-of-field

Destruction consumes the single-use authority, removes the obligation,
runs the type's destructor through a temporary exclusive borrow (so the
destructor can only mutate, never move the object out), destroys every
composite sub-resource, and ends the object. The destructor's borrow is
invalidated explicitly afterwards so `solitary` holds for the
sub-resource destroys and the end. `[Destroy-No-Authority]` covers
both double-destroy (consumed) and destroy-without-authority; in
practice a double `drop(v)` through a binding is caught earlier as
`diag.stale-binding` by `[Destroy-Stale]`, since ending `o` invalidated
`v`'s path (`spec/AUDIT-2.md` B-14 — the conformance suite says so).

`destroy(a)` is reachable from source as the intrinsic `drop(place)`
(`spec/21` §0) and is invoked by `rule.control.block`/`stmt-exit`
for owned and temporary resources.

**Depends on:** inv.resource-authority, inv.temporal-validity,
inv.alias-validity, inv.identity, term.resource, D-0003, D-0004,
D-0018, rule.alias.borrow, rule.fn.call, rule.value-object.object-end,
rule.resauth.destroy-composite, rule.control.flow-analysis
**Affects:** state.authority, state.destruction-obligations,
state.objects, state.access-paths

## 4. Composite sub-resources

`ResourceId ::= Identity | (Identity × Path)` (`spec/04`). A
resource-bearing field, element, or payload of a live struct/array/enum
object `o` is tracked under `(o, path)`, never as its own object.
`sub-range(o, path, Σ)` and `sub-type(o, path, Σ)` are the addresses
and type the path locates within `o` per `rule.agg.layout`.

### `rule.resauth.relocate-in`
**Status:** ACCEPTED

    [Relocate-In]
        alive(o_src, Σ);  τ = type-of(o_src,Σ);  init-state(o_src,Σ) = valid
        alive(o, Σ);  sub-type(o, path, Σ) = τ
        every valid path a' with of(a',Σ) = o_src satisfies a' = holder(o_src, Σ)   -- at most the owner
        Σ.storage(sub-range(o,path,Σ)) := Σ.storage(extent(o_src,Σ))        (cell-wise, in order)
        Obl = { (o_src, q) ∈ Σ.destruction-obligations } ∪ ({o_src} ∩ Σ.destruction-obligations)
        ────────────────────────────────────────────
        ⟨relocate-in(o_src, o, path), Σ⟩ →^ℓ ⟨(), Σ3⟩
        where:
            Σ1 = Σ[ storage as above,
                    destruction-obligations := (Σ.destruction-obligations \ Obl)
                        ∪ { (o, path·q) | (o_src, q) ∈ Obl } ∪ { (o, path) | o_src ∈ Obl },
                    authority(ℓ, destroy, (o, path·q)) := Σ.authority(ℓ, destroy, (o_src, q))  for (o_src,q) ∈ Obl,
                    authority(ℓ, destroy, (o, path)) := Σ.authority(ℓ, destroy, o_src)           if o_src ∈ Obl,
                    authority := authority with the (ℓ, destroy, o_src…) entries removed,
                    access-paths(a').held-by := (held-by \ {o_src}) ∪ {o}  for every a' with o_src ∈ held-by ]
            Σ2 = Σ1[ holder := Σ1.holder \ {o_src} ]
            Σ3 = result of ⟨end-object(o_src), Σ2⟩    -- invalidates o_src's owner path; releases o_src's storage

    [Relocate-In-Aliased]   disposition: checked
        ∃ a' ≠ holder(o_src,Σ) live on o_src
        ────────────────────────────────────────────
        ⟨relocate-in(o_src, o, path), Σ⟩ ↛ diag.move-while-aliased

Moves the value and every obligation of a whole object into a
sub-range of another object: cells are copied, obligations and
authorities are re-keyed under `(o, path·…)`, references the moved
value contained are now held by `o`, and the source object ends. A
plain (non-resource) source has `Obl = ∅` and simply moves. This is
the only rule that changes where a value lives without changing its
identity's meaning: the source identity ends and the value continues
under `o`'s identity (`inv.origin-stability` is preserved — no
`origin` is rewritten).

**Depends on:** inv.resource-authority, inv.origin-stability,
inv.temporal-validity, rule.agg.layout, rule.value-object.object-end,
D-0016, D-0019
**Affects:** state.storage, state.destruction-obligations,
state.authority, state.access-paths, state.holder, state.objects

### `rule.resauth.relocate-out`
**Status:** ACCEPTED

    [Relocate-Out]
        alive(o, Σ);  τ = sub-type(o, path, Σ);  init-state(o,Σ) = valid
        (is-resource(τ) ⇒ (o, path) ∈ Σ.destruction-obligations ∧ authority(ℓ, destroy, (o,path), Σ))
        ⟨establish(τ), Σ⟩ →^ℓ ⟨o_new, Σ1⟩                                -- rule.value-object.object-establish,
                                                                          -- ignoring its authority/obligation effects
                                                                          -- for o_new, which are set below
        ────────────────────────────────────────────
        ⟨relocate-out(o, path), Σ⟩ →^ℓ ⟨temp o_new, Σ2⟩
        where Σ2 = Σ1[ storage(extent(o_new,Σ1)) := Σ.storage(sub-range(o,path,Σ)),
                       init(o_new) := valid,
                       destruction-obligations := (Σ1.destruction-obligations \ {(o,path)} \ {(o, path·q)})
                           ∪ {o_new | is-resource(τ)} ∪ { (o_new, q) | (o, path·q) ∈ Σ.destruction-obligations },
                       authority(ℓ, destroy, o_new) := Σ.authority(ℓ, destroy, (o,path))   if is-resource(τ),
                       authority(ℓ, destroy, (o_new, q)) := Σ.authority(ℓ, destroy, (o, path·q)),
                       authority entries for (o,path…) removed,
                       access-paths(a').held-by := (held-by \ {o}) ∪ {o_new}
                           for every a' held by o whose ref cell lies within sub-range(o,path,Σ),
                       storage(sub-range(o,path,Σ)) := uninit ]

The inverse of `[Relocate-In]`: promotes a sub-range to a fresh
temporary object, moving its obligations and authority out. Used by
`rule.agg.match` for a resource-bearing payload. The vacated sub-range
is marked `uninit`; `rule.agg.match` ends the source enum object in the
same step, so no path ever reads it.

**Depends on:** inv.resource-authority, rule.value-object.object-establish,
rule.agg.layout, D-0019
**Affects:** state.storage, state.destruction-obligations,
state.authority, state.access-paths, state.objects, state.init

### `rule.resauth.destroy-composite`
**Status:** ACCEPTED

    [Destroy-Composite]
        P = [ p | (o, p) ∈ Σ.destruction-obligations ], ordered longest path first,
            ties by reverse declaration/index order
        ────────────────────────────────────────────
        ⟨destroy-composite(o), Σ⟩ →^ℓ ⟨(), Σ_n⟩
        where Σ_0 = Σ and, for p_i ∈ P in order:
            τ_i = sub-type(o, p_i, Σ)
            a_i = a fresh root path { target: sub-range(o,p_i,Σ), of: o, type: τ_i, mode: exclusive,
                                      thread: ℓ, valid: true, formed-at: this-event,
                                      frame: current-frame(ℓ,Σ), temp-scope: current-scope(ℓ,Σ), base: None, held-by: ∅ }
            Σ_i = ⟨run-destructor(a_i, τ_i), Σ_{i-1}[ access-paths(a_i) := …,
                        authority(ℓ, destroy, (o,p_i)).consumed := true,
                        destruction-obligations := … \ {(o, p_i)} ]⟩ then access-paths(a_i).valid := false

Destroys every outstanding sub-resource of `o`, innermost first, by
running each one's destructor through a temporary exclusive root path
into the sub-range; no `end-object` per sub-resource, since the storage
is `o`'s. Every `(o, p)` key reaches here exactly once; nested keys
were flattened at `[Relocate-In]`. Applies uniformly to struct, array,
and enum objects (an enum has at most one live payload key).

**Depends on:** inv.resource-authority, rule.resauth.destroy,
rule.agg.layout
**Affects:** state.authority, state.destruction-obligations,
state.access-paths

## 5. Leak freedom

`rule.resauth.leak` (0.1.0–0.4.0) is retired: D-0008 made leak
freedom structural. Every resource object is either owned (`holder`
defined — destroyed by `rule.control.block` when its owning frame
exits) or a temporary (destroyed by `rule.control.stmt` at the end
of the statement that created it unless it is that statement's
result, in which case the receiving construct owns it). Every
sub-resource is destroyed by `[Destroy-Composite]` when its container
is. No obligation can therefore survive its scope; a leak diagnostic
does not exist because the state it would report is unreachable.

## Change Log

- 1.2.0 — `CHG-0010`: `destructor(τ)` defined for the built-in resource
  types, which `[Run-Destructor]` could not otherwise reach (no
  `τ::drop` item exists for them; `conf.mutex-lock-unlock`,
  `conf.unjoined-handle-waits`).
- 1.1.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"): §1's
  destructor signature written in the current declaration syntax (it
  mixed `:` with the type name `unit`, which `spec/22` 2.6.0 spells
  `void`); §1 now also names `[Destroy-Composite]` as the other caller
  of `[Run-Destructor]`, which §4 always was.
- 1.1.0 — `CHG-0001`: destructor signature illustration re-spelled to
  `spec/22` 2.0.0 syntax (`:` instead of `->`); no rule semantics
  changed.
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-06, B-07,
  B-17): destructors defined (§1) and invoked by `[Destroy]` through
  a temporary exclusive borrow; `[Field-Transfer-In]`/`-Out]` replaced
  by `[Relocate-In]`/`[Relocate-Out]`, which move cells, obligations,
  authority, and held references and are stated over `Σ`;
  `[Destroy-Composite]` restated over `Σ`; transfer rejects moving an
  aliased object (`diag.move-while-aliased`); `rule.resauth.leak`
  retired (B-21). All entities `ACCEPTED`.
- 0.4.0 and earlier — superseded.
