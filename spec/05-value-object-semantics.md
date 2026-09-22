# CobaltC Value and Object Semantics

Status: normative artifact
Version: 1.3.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.value-object.*`)
Governed by: `CobaltC_Master_Instructions.md` §17, §23
Realizes: D-0003 (establishment grants authority), D-0018 (use-time
alias checking), D-0019 (results, storage, store)

## Purpose

Defines the generic mechanics of objects: establishment with storage
allocation, binding formation and lookup, reading and writing through
an access path, the single `store` operation every value-placing
construct reuses, and object end. Every rule is stated over `Σ`
(`spec/04-abstract-state.md`) in CFN (`spec/01-metalanguage.md`).

**Evaluation results (D-0019).** Every expression reduces to exactly
one of three result forms, written `r`:

    r ::= v            -- a value of some type τ (spec/06, spec/12)
        | place a      -- an access path (a name, a projection, *e)
        | temp o       -- a temporary object: alive, holder undefined

`spec/13` §1 fixes which syntactic forms produce which; this artifact
defines what each may be stored as.

---

### `rule.value-object.object-establish`
**Status:** ACCEPTED

    [Object-Establish-Resource]   outcome: unspecified { any fresh aligned range }
        is-resource(τ)                                    -- spec/12 §3
        o ∉ dom(Σ.objects)
        addrs ⊆ Address, |addrs| = sizeof(τ), addrs contiguous,
            min(addrs) mod alignof(τ) = 0, addrs ∩ dom(Σ.storage) = ∅
        ────────────────────────────────────────────
        ⟨establish(τ), Σ⟩ →^ℓ
            ⟨o, Σ[ objects(o) := {origin: this-event, extent: addrs, type: τ,
                                   storage-kind: fresh, temp-scope: current-scope(ℓ,Σ)},
                   storage(x) := uninit  for every x ∈ addrs,
                   init(o) := uninitialized,
                   destruction-obligations := Σ.destruction-obligations ∪ {o},
                   authority(ℓ, destroy, o) := {consumed: false} ]⟩

    [Object-Establish-Plain]   outcome: unspecified { any fresh aligned range }
        ¬is-resource(τ)
        o ∉ dom(Σ.objects);  addrs as above
        ────────────────────────────────────────────
        ⟨establish(τ), Σ⟩ →^ℓ
            ⟨o, Σ[ objects(o) := {origin: this-event, extent: addrs, type: τ,
                                   storage-kind: fresh, temp-scope: current-scope(ℓ,Σ)},
                   storage(x) := uninit  for every x ∈ addrs,
                   init(o) := uninitialized ]⟩

A fresh identity `o` becomes a live object over freshly allocated,
aligned storage with `init = uninitialized`; the address choice is
unobservable to a safe program (no rule converts a `ref` to an
address), so the freedom is `unspecified`, not `impl-defined`. A
resource-bearing type additionally grants the establishing thread
single-use destroy authority and a destruction obligation (D-0003).
Neither rule carries a `disposition`: the premises are all decidable
by construction (`spec/01` §5; resolves `spec/AUDIT-2.md` B-24).
`sizeof`/`alignof`: `rule.arith.sizeof` (`spec/06` §7).

**Depends on:** term.object, term.storage, term.type, term.origin,
inv.identity, inv.origin-stability, inv.resource-authority, D-0003,
D-0019, rule.arith.sizeof, rule.type.is-resource
**Affects:** state.objects, state.storage, state.init,
state.destruction-obligations, state.authority

---

### `rule.value-object.binding-form`
**Status:** ACCEPTED

    [Binding-Form]
        alive(o, Σ);  a ∉ dom(Σ.access-paths)
        ────────────────────────────────────────────
        ⟨bind(name, o), Σ⟩ →^ℓ
            ⟨a, Σ[ access-paths(a) := {target: extent(o,Σ), of: o, type: type-of(o,Σ),
                                        mode: exclusive, thread: ℓ, valid: true,
                                        formed-at: this-event,
                                        frame: current-frame(ℓ,Σ), temp-scope: current-scope(ℓ,Σ), base: None,
                                        held-by: ∅},
                   bindings(current-frame(ℓ,Σ), name) := a,
                   holder(o) := a ]⟩

Forms the root access path that owns `o` and records it under `name`
in the current frame. Every binding is `exclusive`: restricting access
is what a borrow does (`spec/08`), never something a binding is
declared with — there is no separate mutability qualifier. Setting `holder(o)` makes `o`
owned by this frame (D-0016/D-0019); `[Binding-Form]` is only ever
invoked by `rule.value-object.store`, `rule.init.let` `[Let-Uninit]`,
and `rule.resauth.transfer`'s destination formation, each of which
guarantees `o` is either fresh, a temporary, or (transfer) about to
lose its previous owner at that moment.
If `name` was already bound in this frame, the previous binding is
shadowed: its access path stays valid until its object ends at frame
exit.

**Depends on:** term.binding, term.access-path, inv.temporal-validity,
D-0018, D-0019
**Affects:** state.bindings, state.access-paths, state.holder

---

### `rule.value-object.binding-lookup`
**Status:** ACCEPTED

    [Binding-Lookup]   disposition: checked
        (f, name) ∈ dom(Σ.bindings) for the innermost f ∈ Σ.frame-stack(ℓ)
            such that (f, name) ∈ dom(Σ.bindings)
        a = Σ.bindings(f, name)
        temporally-valid(a, Σ)
        ────────────────────────────────────────────
        ⟨name, Σ⟩ →^ℓ ⟨place a, Σ⟩
        side-conditions:
            ⟦ temporally-valid(a, Σ) ⟧ discharge: static (rule.control.flow-analysis;
                                                  discharge: dynamic where unknown)

    [Binding-Lookup-Unbound]   disposition: rejected
        no f ∈ Σ.frame-stack(ℓ) has (f, name) ∈ dom(Σ.bindings),
            and name is not an item path (rule.module.resolve, spec/17)
        ────────────────────────────────────────────
        ill-formed; diag.unbound-name

    [Binding-Lookup-Stale]   disposition: checked
        a as in [Binding-Lookup];  ¬temporally-valid(a, Σ)
        ────────────────────────────────────────────
        ⟨name, Σ⟩ ↛ diag.stale-binding

A name resolves to the nearest enclosing frame's binding (lexical
scoping: frames are pushed in nesting order, `spec/14` §1). An unbound
name is a static fact; a stale binding — its object ended, its
authority transferred away — is caught statically where
`rule.control.flow-analysis` proves it and dynamically otherwise.

**Depends on:** term.binding, inv.temporal-validity,
rule.control.flow-analysis, rule.module.resolve

---

### `rule.value-object.read`
**Status:** ACCEPTED

    [Read]   disposition: checked
        alive(of(a,Σ), Σ);  temporally-valid(a, Σ)
        init-state(of(a,Σ), Σ) = valid                     -- op-requires(read)
        ¬is-resource(type(a,Σ))
        ¬clash(a, shared, Σ)                                -- inv.alias-validity, D-0018
        v = value-at(type(a,Σ), Σ.storage, target(a,Σ))     -- rule.arith.represent
        ────────────────────────────────────────────
        ⟨read(a), Σ⟩ →^ℓ ⟨v, Σ⟩
        side-conditions:
            ⟦ temporally-valid(a, Σ) ∧ alive(of(a,Σ), Σ) ⟧
                discharge: static (rule.control.flow-analysis; dynamic where unknown)
            ⟦ init-state(of(a,Σ), Σ) = valid ⟧
                discharge: static (rule.init.definite-assignment; dynamic where unknown)
            ⟦ ¬clash(a, shared, Σ) ⟧
                discharge: static (rule.control.flow-analysis; dynamic where unknown)
            ⟦ ¬is-resource(type(a,Σ)) ⟧ discharge: static
            ⟦ mode(a,Σ) ∈ {shared, exclusive} ⟧ discharge: static   -- always true

    [Read-Stale]            disposition: checked   ¬temporally-valid(a,Σ) ∨ ¬alive(of(a,Σ),Σ)   ⟨read(a), Σ⟩ ↛ diag.stale-binding
    [Read-Uninitialized]    disposition: checked   init-state(of(a,Σ), Σ) ≠ valid            ⟨read(a), Σ⟩ ↛ diag.use-of-uninitialized
    [Read-Conflict]         disposition: checked   clash(a, shared, Σ)                        ⟨read(a), Σ⟩ ↛ diag.aliasing-conflict
    [Read-Resource-Rejected] disposition: rejected  is-resource(type(a,Σ))                     ill-formed; diag.read-of-resource

Reading through `a` yields an owned copy of the value its target
currently holds. A resource-bearing value is never copied this way: an
owned copy would duplicate authority `state.authority` tracks
(D-0003); resources move by `rule.resauth.transfer` or
`rule.resauth.relocate-in`, or are reached through a `ref`. `Read`
copies reference tokens contained in `v` as values; storing `v`
(`[Write]`) is what records the new holder. `[Read-Conflict]` is the
use-time half of `inv.alias-validity` (D-0018): a read is denied while
an exclusive path that is not an ancestor of `a` reaches the same
storage.

**Depends on:** term.access, inv.spatial-validity,
inv.initialization-validity, inv.alias-validity, inv.resource-authority,
inv.temporal-validity, rule.type.is-resource, rule.arith.represent,
rule.control.flow-analysis, rule.init.definite-assignment, D-0018

---

### `rule.value-object.write`
**Status:** ACCEPTED

    [Write]   disposition: checked
        o = of(a,Σ);  alive(o, Σ);  temporally-valid(a, Σ)
        v : type(a,Σ)                                          -- rule.type.typing
        mode(a,Σ) = exclusive
        ¬clash(a, exclusive, Σ)                                -- inv.alias-validity
        init-ok(a, o, Σ) ≝  (target(a,Σ) = extent(o,Σ) ∧ init-state(o,Σ) ∈ {uninitialized, valid})
                          ∨ (target(a,Σ) ⊂ extent(o,Σ) ∧ init-state(o,Σ) = valid)
        ¬live-resource-at(a, Σ)                                 -- see below
        old = refs-in(value-at(type(a,Σ), Σ.storage, target(a,Σ)))  if init-state(o,Σ) = valid, else ∅
        ────────────────────────────────────────────
        ⟨write(a, v), Σ⟩ →^ℓ
            ⟨(), Σ[ storage(target(a,Σ)) := represent(type(a,Σ), v),
                    init(o) := valid,
                    access-paths(a').held-by := Σ.access-paths(a').held-by \ {o}   for a' ∈ old \ refs-in(v),
                    access-paths(a').held-by := Σ.access-paths(a').held-by ∪ {o}   for a' ∈ refs-in(v),
                    access-paths(a').valid := false   for a' ∈ old \ refs-in(v) with
                        Σ.access-paths(a').held-by = {o} ∧ a'.temp-scope ∉ { t | (t, _) ∈ Σ.temp-scope-stack(ℓ) } ]⟩
        side-conditions:
            ⟦ temporally-valid(a,Σ) ∧ alive(o,Σ) ⟧ discharge: static (flow-analysis; dynamic where unknown)
            ⟦ v : type(a,Σ) ⟧ discharge: static
            ⟦ mode(a,Σ) = exclusive ⟧ discharge: static
            ⟦ ¬clash(a, exclusive, Σ) ⟧ discharge: static (flow-analysis; dynamic where unknown)
            ⟦ init-ok(a, o, Σ) ⟧ discharge: static (definite-assignment; dynamic where unknown)
            ⟦ ¬live-resource-at(a, Σ) ⟧ discharge: static (flow-analysis; dynamic where unknown)

    where live-resource-at(a, Σ) ≝ is-resource(type(a,Σ)) ∧ init-state(o,Σ) = valid
                                   ∧ resource-id(a,Σ) ∈ Σ.destruction-obligations
                                   ∧ owns(type(a,Σ), value-at(type(a,Σ), Σ.storage, target(a,Σ)))
          owns(τ, v) ≝ is-resource(τ) ∧ (owner(τ) ∨ τ ∈ {handle, mutex, guard, closure types}
                          ∨ τ a struct: ∃ field f. owns(τ_f, v.f)
                          ∨ τ an enum, v = Vi(w): Vi has a payload and owns(τi, w)
                          ∨ τ = array<τ',N>: ∃ i. owns(τ', v[i]))                          -- D-0049
          owner(τ) ≝ τ is declared `resource`, or destructor(τ) ≠ None (spec/07 §1)
          resource-id(a, Σ) ≝ o if target(a,Σ) = extent(o,Σ), else (o, path(a,Σ))
          path(a, Σ) ≝ the Path of projections from root(a) to a (rule.agg.field-access/index)

    [Write-Stale]             disposition: checked   ¬temporally-valid(a,Σ) ∨ ¬alive(o,Σ), a not a whole binding written by `x = e`     ⟨write(a,v), Σ⟩ ↛ diag.stale-binding
    [Write-Not-Exclusive]     disposition: rejected  mode(a,Σ) = shared                      ill-formed; diag.write-through-shared
    [Write-Conflict]          disposition: checked   clash(a, exclusive, Σ)                   ⟨write(a,v), Σ⟩ ↛ diag.aliasing-conflict
    [Write-Partial-Init]      disposition: checked   target(a,Σ) ⊂ extent(o,Σ) ∧ init-state(o,Σ) = uninitialized   ⟨write(a,v), Σ⟩ ↛ diag.use-of-uninitialized
    [Write-Resource-Overwrite-Rejected]   disposition: checked   live-resource-at(a, Σ)      ⟨write(a,v), Σ⟩ ↛ diag.overwrite-of-live-resource

Writing establishes or replaces the value at `a`'s target. An
assignment `x = e` to a whole binding whose value was moved away or
ended is not `[Write-Stale]`: it re-establishes `x`
(`spec/11` `[Assign-Reestablish]`, D-0033). Three
guards beyond validity and typing: the path must be `exclusive` (a
`shared` path is read-only — a static fact about the path's mode,
`spec/08` §1); no non-ancestor path may currently reach the same
storage (D-0018); and the target must not hold a live resource whose
destruction obligation is outstanding — overwriting one would strand
its obligation, so it must be destroyed first (`rule.resauth.destroy`,
then this rule). What counts is the value there now (D-0049): a
`None` of an `Option<Box<T>>`, or a struct whose resource-bearing
fields are all such values, owns nothing and may be written over; a
type declared `resource` or with a destructor always owns. A whole-object write may initialize; a projection
write requires the object already `valid` (D-0019: partial
initialization is rejected, so `rule.init.definite-assignment` reasons about
whole bindings only). The `held-by` bookkeeping records every
reference the new value contains as held by `o`, and releases every
reference the old value contained; a released reference whose only
holder was `o` and whose forming statement has already ended is
invalidated at once (D-0018).

**Depends on:** term.access, inv.spatial-validity,
inv.initialization-validity, inv.alias-validity, inv.resource-authority,
inv.temporal-validity, rule.type.typing, rule.type.is-resource,
rule.arith.represent, rule.control.flow-analysis,
rule.init.definite-assignment, D-0018, D-0019
**Affects:** state.storage, state.init, state.access-paths

---

### `rule.value-object.store`
**Status:** ACCEPTED

`store` places an evaluation result `r` into a *slot*. Slots:

    slot ::= binding(name)        -- a local-declaration/parameter/pattern binding in the current frame
           | sub(o, path, τ)      -- a field/element/payload sub-range of an object under construction
           | result               -- the value position of return / block result / statement result

    [Store-Binding-Value]
        r = v,  v : τ
        ⟨establish(τ), Σ⟩ →^ℓ ⟨o, Σ1⟩                                -- object-establish
        ⟨bind(name, o), Σ1⟩ →^ℓ ⟨a, Σ2⟩                               -- binding-form
        ⟨write(a, v), Σ2⟩ →^ℓ ⟨(), Σ3⟩                                -- write (whole object)
        ────────────────────────────────────────────
        ⟨store(binding(name), v), Σ⟩ →^ℓ ⟨(), Σ3⟩

    [Store-Binding-Place-Copy]
        r = place a0,  ¬is-resource(type(a0,Σ))
        ⟨read(a0), Σ⟩ →^ℓ ⟨v, Σ⟩
        ⟨store(binding(name), v), Σ⟩ →^ℓ ⟨(), Σ'⟩
        ────────────────────────────────────────────
        ⟨store(binding(name), place a0), Σ⟩ →^ℓ ⟨(), Σ'⟩

    [Store-Binding-Place-Transfer]
        r = place a0,  is-resource(type(a0,Σ)),  o = of(a0,Σ)
        target(a0,Σ) = extent(o,Σ)                                     -- whole object only
        ⟨bind(name, o), Σ⟩ →^ℓ ⟨a, Σ1⟩
        ⟨transfer(a0, a), Σ1⟩ →^ℓ ⟨(), Σ2⟩                            -- rule.resauth.transfer
        ────────────────────────────────────────────
        ⟨store(binding(name), place a0), Σ⟩ →^ℓ ⟨(), Σ2⟩

    [Store-Binding-Place-Transfer-Sub]   disposition: rejected
        r = place a0,  is-resource(type(a0,Σ)),  target(a0,Σ) ⊂ extent(of(a0,Σ),Σ)
        ────────────────────────────────────────────
        ill-formed; diag.move-out-of-field

    [Store-Binding-Temp]
        r = temp o,  temporary(o, Σ)
        ⟨bind(name, o), Σ⟩ →^ℓ ⟨a, Σ1⟩                               -- adoption: holder(o) := a
        ────────────────────────────────────────────
        ⟨store(binding(name), temp o), Σ⟩ →^ℓ ⟨(), Σ1⟩

    [Store-Sub-Value]
        r = v,  v : τ,  sub-type(o, path, Σ) = τ            -- rule.agg.layout
        ────────────────────────────────────────────
        ⟨store(sub(o, path, τ), v), Σ⟩ →^ℓ
            ⟨(), Σ[ storage(sub-range(o,path,Σ)) := represent(τ, v),
                    access-paths(a').held-by := Σ.access-paths(a').held-by ∪ {o}  for a' ∈ refs-in(v) ]⟩

    [Store-Sub-Place-Copy]
        r = place a0,  ¬is-resource(type(a0,Σ)),  ⟨read(a0), Σ⟩ →^ℓ ⟨v, Σ⟩
        ⟨store(sub(o,path,τ), v), Σ⟩ →^ℓ ⟨(), Σ'⟩
        ────────────────────────────────────────────
        ⟨store(sub(o,path,τ), place a0), Σ⟩ →^ℓ ⟨(), Σ'⟩

    [Store-Sub-Relocate]
        r = place a0 with is-resource(type(a0,Σ)) and target(a0,Σ) = extent(of(a0,Σ),Σ),
            or r = temp o_src
        o_src = of(a0,Σ) in the place case
        ⟨relocate-in(o_src, o, path), Σ⟩ →^ℓ ⟨(), Σ'⟩                 -- rule.resauth.relocate-in
        ────────────────────────────────────────────
        ⟨store(sub(o,path,τ), r), Σ⟩ →^ℓ ⟨(), Σ'⟩

    [Store-Result-Value]      ⟨store(result, v), Σ⟩ →^ℓ ⟨v, Σ⟩
    [Store-Result-Place-Copy] ¬is-resource(type(a0,Σ));  ⟨read(a0),Σ⟩ →^ℓ ⟨v,Σ⟩     ⟨store(result, place a0), Σ⟩ →^ℓ ⟨v, Σ⟩
    [Store-Result-Place-Release]
        is-resource(type(a0,Σ)),  target(a0,Σ) = extent(o,Σ),  o = of(a0,Σ)
        ────────────────────────────────────────────
        ⟨store(result, place a0), Σ⟩ →^ℓ
            ⟨temp o, Σ[ holder := Σ.holder \ {o}, access-paths(a0).valid := false ]⟩
    [Store-Result-Temp]       ⟨store(result, temp o), Σ⟩ →^ℓ ⟨temp o, Σ⟩

`store` is the one operation `let` (`rule.init.let`), parameter binding
(`rule.fn.bind-param`), aggregate construction (`rule.agg.*-construct`),
closure capture (`rule.fn.closure`), `return`, and block/statement
results (`rule.control.*`) all reduce to (D-0019). A scalar value is
written; a non-resource place is copied; a resource place is
transferred (binding) or relocated (sub-range); a temporary is adopted
(binding) or relocated (sub-range); in result position a resource
binding is released into a temporary that the receiving construct will
store. Moving a resource *out of* a field of a live object is rejected
(`[Store-Binding-Place-Transfer-Sub]`): the composite would be left
with a dangling obligation; the whole container must be moved, or the
field reached through a reference.

`[Store-Sub-Value]` writes cells directly rather than through
`[Write]`: it is only ever invoked by `rule.agg.*-construct` while `o`
is under construction (`init(o) = uninitialized`, no access path to
`o` exists yet), which guarantees every sub-range is written exactly
once before the constructing rule sets `init(o) := valid`, so
`[Write]`'s initialization, mode, and clash premises have nothing to
check.

**Depends on:** rule.value-object.object-establish,
rule.value-object.binding-form, rule.value-object.read,
rule.value-object.write, rule.resauth.transfer,
rule.resauth.relocate-in, rule.agg.layout, rule.arith.represent,
rule.type.is-resource, D-0019
**Affects:** state.objects, state.bindings, state.holder,
state.access-paths, state.authority

---

### `rule.value-object.object-end`
**Status:** ACCEPTED

    [Object-End]
        alive(o, Σ)
        A_o = { a | of(a,Σ) = o }
        H_o = { a' | o ∈ Σ.access-paths(a').held-by }
        Dead = { a' ∈ H_o | Σ.access-paths(a').held-by = {o} }
        ────────────────────────────────────────────
        ⟨end-object(o), Σ⟩ →^ℓ
            ⟨(), Σ[ objects := Σ.objects \ {o},
                    init := Σ.init \ {o},
                    holder := Σ.holder \ {o},
                    storage := Σ.storage \ extent(o,Σ)   if storage-kind(o) = fresh,
                    access-paths(a).valid := false          for a ∈ A_o ∪ Dead,
                    access-paths(a').held-by := Σ.access-paths(a').held-by \ {o}   for a' ∈ H_o ]⟩

Ending an object removes it from `Σ.objects`, releases its own storage
(not reclaimed raw storage, `spec/20` §2), invalidates every path that
targets it, and releases every reference it held — invalidating those
whose only holder it was (D-0018). `[Object-End]` carries no
`disposition`: it is reached only through `rule.resauth.destroy`
(resources, which discharges authority and solitary access first) and
`rule.control.stmt`/`block-exit` (plain temporaries and plain
owned objects, for which nothing remains to check: a plain object has
no authority, and any path still targeting it is precisely what this
rule invalidates so that `inv.temporal-validity` is enforced at the
path's next use).

**Depends on:** inv.identity, inv.temporal-validity,
inv.resource-authority, D-0018, D-0019
**Affects:** state.objects, state.storage, state.init, state.holder,
state.access-paths

## Change Log

- 1.3.0 — `CHG-0057` (D-0049): `live-resource-at` asks whether the
  value at the target owns anything (`owns`), not only whether its type
  is a resource.

- 1.2.0 — `CHG-0042` (D-0033): `[Write-Stale]` excludes an assignment
  to a whole binding, which `spec/11` `[Assign-Reestablish]` governs.

- 1.1.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"):
  `[Write]`'s scope-membership test written against the
  `(TempScopeId × Frame)` entries `state.temp-scope-stack` actually
  holds; a citation of a rule label that does not exist
  (`[Definite-Assignment]`) replaced by the rule id; one `let` mention
  re-worded.
- 1.1.0 — `CHG-0001`: replaced a stale `let mut` cross-reference with
  prose stating the same fact (no separate mutability qualifier
  exists); no rule semantics changed.
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-02, B-04,
  B-05, B-07, B-11, B-15, B-24). Establishment allocates storage;
  `[Read]`/`[Write]` act on `target(a)` with `type(a)` and check
  `clash` at use; `[Write]` maintains `held-by`; `store` added as the
  single value-placing operation; `[Object-End]` releases storage and
  held references; `[Binding-Form]` sets every record field and the
  holder; all rules promoted to `ACCEPTED`.
- 0.5.0 and earlier — superseded; history in `spec/AUDIT.md`.
