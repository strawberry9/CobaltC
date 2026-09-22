# CobaltC Trust Boundaries and Low-Level Interoperability

Status: normative artifact
Version: 1.9.0
Conforms to: `spec/02-schema.md` (Kind: Type `type.rawptr`; Kind: Rule,
`rule.trust.*`)
Governed by: `CobaltC_Master_Instructions.md` §5, §17 (External
Interfaces), §23
Realizes: D-0017 (`unsafe`, `rawptr`, `Reclaim`), D-0019

## 1. `unsafe` blocks

### `rule.trust.unsafe`
**Status:** ACCEPTED

    [Unsafe-Rejected]   disposition: rejected
        a construct whose rule carries a `discharge: trusted` side-condition occurs
        lexically outside every `unsafe { }` block
        ────────────────────────────────────────────
        ill-formed; diag.trusted-outside-unsafe

`unsafe { … }` is an ordinary block (`spec/14` §1) inside which the
`trusted-unchecked` rules of this artifact may appear. It is the
*only* place any `discharge: trusted` side-condition may occur; the
block's text is the record of what the author asserts. The trusted
rules are: `[Rawptr-Read]`, `[Rawptr-Write]`, `[Rawptr-Move-In]`,
`[Rawptr-Move-Out]`, `[Rawptr-Offset]`, `[Reclaim]`, `[Release]`,
`[Extern-Call]`, `[Deallocate]` (`spec/21` §0), `[Copy-Raw]`.

**Depends on:** D-0017

## 2. Raw pointers

### `type.rawptr`
**Status:** ACCEPTED

`rawptr<τ>`: represented domain `Address` (`spec/06` §1). A raw pointer
has no `Σ.access-paths` entry, no mode, no `clash`/`addr-in`/validity
check; `is-resource = false`; freely copyable; obtaining one is safe,
using one to reach storage is trusted.

### `rule.trust.rawptr`
**Status:** ACCEPTED

    addrs(p, τ) ≝ { p, …, p + sizeof(τ) − 1 }
    reclaimed-at(addrs, τ, Σ) ≝ the unique o with alive(o,Σ), storage-kind(o) = reclaimed,
                                  extent(o,Σ) = addrs, type-of(o,Σ) = τ, if it exists

    [Rawptr-Of]                                                 -- safe
        r : ref<τ, m> (a value a)
        ────────────────────────────────────────────
        ⟨rawptr_of(r), Σ⟩ → ⟨min(target(a,Σ)) : rawptr<τ>, Σ⟩

    [Rawptr-Offset]   disposition: trusted-unchecked
        p : rawptr<τ>,  n : isize
        ────────────────────────────────────────────
        ⟨p + n, Σ⟩ → ⟨p + n · sizeof(τ), Σ⟩
        side-conditions: ⟦ the result lies within, or one past the end of, one allocation
                            (rule.stdlib.prelude) or one object's extent ⟧ discharge: trusted

    [Rawptr-Read]   disposition: trusted-unchecked         -- *p in value position, ¬is-resource(τ)
        p : rawptr<τ>,  ¬is-resource(τ),  v = value-at(τ, Σ.storage, addrs(p,τ))
        ────────────────────────────────────────────
        ⟨*p, Σ⟩ → ⟨v, Σ⟩
        side-conditions: ⟦ addrs(p,τ) ⊆ dom(Σ.storage) and hold an image of represent(τ, ·);
                            no live derived path (base ≠ None) of any object reaches them
                            in exclusive mode; no other thread writes them concurrently ⟧ discharge: trusted

    [Rawptr-Write]   disposition: trusted-unchecked        -- *p = e, ¬is-resource(τ)
        p : rawptr<τ>,  ¬is-resource(τ),  ⟨e, Σ⟩ →* ⟨v, Σ1⟩
        ────────────────────────────────────────────
        ⟨*p = e, Σ⟩ → ⟨(), Σ1[ storage(addrs(p,τ)) := represent(τ, v),
                                if refs-in(v) ≠ ∅: o' = reclaimed-at(addrs(p,τ), τ, Σ1) if it exists, else a fresh identity
                                    with objects(o') := { extent: addrs(p,τ), type: τ, storage-kind: reclaimed,
                                    temp-scope: current-scope }, init(o') := valid      -- CHG-0031
                                held-by updated for refs-in(v) and the old contents as in rule.value-object.write,
                                with o' (else reclaimed-at(addrs(p,τ)), if it exists) as the holder ]⟩
        side-conditions: ⟦ addrs(p,τ) ⊆ dom(Σ.storage); no live derived path (base ≠ None) of any
                            object reaches them; no other thread accesses them concurrently ⟧ discharge: trusted

    [Rawptr-Move-In]   disposition: trusted-unchecked      -- *p = e, is-resource(τ): moves the resource into raw storage
        p : rawptr<τ>,  is-resource(τ),  ⟨e, Σ⟩ →* ⟨r, Σ1⟩ in place position (place a_src or temp o_src)
        o_src the source object (whole object; a projection source is diag.move-out-of-field)
        every valid path on o_src is its holder (else diag.move-while-aliased)
        o' fresh identity
        ────────────────────────────────────────────
        ⟨*p = e, Σ⟩ → ⟨(), Σ2⟩
        where Σ2 = Σ1 with: objects(o') := { origin: this-event, extent: addrs(p,τ), type: τ,
                                              storage-kind: reclaimed, temp-scope: current-scope };
                             storage(addrs(p,τ)) := Σ1.storage(extent(o_src));  init(o') := valid;
                             obligations, authority, and held-by re-keyed from o_src to o' exactly as
                               rule.resauth.relocate-in re-keys to (o, path), but to a top-level key o';
                             then end-object(o_src)
        side-conditions: ⟦ addrs(p,τ) ⊆ dom(Σ.storage) and are not part of any live object ⟧ discharge: trusted

    [Rawptr-Move-Out]   disposition: trusted-unchecked     -- *p in value position, is-resource(τ)
        p : rawptr<τ>,  is-resource(τ)
        o' = reclaimed-at(addrs(p,τ), τ, Σ) if it exists, else the object [Reclaim] would establish over
            addrs(p,τ) (fresh identity, storage-kind reclaimed, init valid, fresh authority and obligation)
            -- lazy re-attachment, exactly as [Reclaim]: the cells may have been copied here by copy_raw
        every valid path on o' is invalid or absent (else diag.move-while-aliased)
        ⟨establish(τ), Σ⟩ →^ℓ ⟨o_new, Σ1⟩ ignoring its authority effects
        ────────────────────────────────────────────
        ⟨*p, Σ⟩ → ⟨temp o_new, Σ2⟩
        where Σ2 = Σ1 with storage(extent(o_new)) := Σ.storage(addrs(p,τ)); init(o_new) := valid;
              obligations/authority/held-by re-keyed from o' to o_new, the authority from (ℓ0, destroy, o') to
                (ℓ, destroy, o_new) whichever thread ℓ0 held it (CHG-0030); storage(addrs(p,τ)) := uninit;
              then end-object(o') (its storage is reclaimed: not released)
        side-conditions: ⟦ addrs(p,τ) hold a valid τ that no live fresh-storage object claims and, if no reclaimed
                            object exists there, whose destroy obligation is not tracked anywhere else ⟧ discharge: trusted

    [Reclaim]   disposition: trusted-unchecked             -- a place expression (spec/13 §1)
        p : rawptr<τ>;  o = reclaimed-at(addrs(p,τ), τ, Σ) if it exists, else a fresh identity with
            objects(o) := { …, extent: addrs(p,τ), type: τ, storage-kind: reclaimed, temp-scope: current-scope },
            init(o) := valid, and (if is-resource(τ)) authority(ℓ, destroy, o) := {consumed: false},
            destruction-obligations ∪= {o}
        if o was re-attached and is-resource(τ): authority(ℓ, destroy, o) := authority(ℓ0, destroy, o) and the
            (ℓ0, destroy, o) entry is removed, ℓ0 the thread holding it   -- the reclaiming thread takes it (CHG-0030)
        a ∉ dom(Σ.access-paths)
        ────────────────────────────────────────────
        ⟨reclaim<τ>(p), Σ⟩ →^ℓ ⟨place a, Σ'[ access-paths(a) := root record for o (mode exclusive, base None,
                                                                            held-by ∅, frame current-frame, temp-scope current-scope) ]⟩
        side-conditions: ⟦ addrs(p,τ) hold a valid τ that no live fresh-storage object claims (a reclaimed
                            object over exactly these cells is re-attached, above) and, if is-resource(τ),
                            whose destroy obligation is not tracked anywhere else ⟧ discharge: trusted

    [Release]   disposition: trusted-unchecked
        p : rawptr<u8>,  n : usize
        R = { o | storage-kind(o) = reclaimed ∧ extent(o) ⊆ [p, p+n) }
        ────────────────────────────────────────────
        ⟨release(p, n), Σ⟩ → ⟨(), Σ with end-object(o) for every o ∈ R, their obligations removed unfulfilled ⟩
        side-conditions: ⟦ every outstanding obligation of an o ∈ R has been discharged, or its cells have been copied
                            (copy_raw) to storage where a later [Reclaim]/[Rawptr-Move-Out] re-establishes it ⟧ discharge: trusted

    [Copy-Raw]   disposition: trusted-unchecked
        dst, src : rawptr<u8>,  n : usize
        ────────────────────────────────────────────
        ⟨copy_raw(dst, src, n), Σ⟩ → ⟨(), Σ[ storage(dst + i) := Σ.storage(src + i) for 0 ≤ i < n ]⟩
        side-conditions: ⟦ both ranges ⊆ dom(Σ.storage); no live object overlaps [dst, dst+n) ⟧ discharge: trusted

**Root paths and raw access.** The trusted conditions of `[Rawptr-Read]`
and `[Rawptr-Write]` speak of *derived* paths (borrows and projections,
`base ≠ None`), not of an object's own root path: reading or writing a
local's cells through `rawptr_of(&x)`/`rawptr_of(&mut x)` while `x`'s
binding exists is the ordinary FFI-buffer pattern, and `x`'s later
reads simply see the cells as written. What the author asserts is that
no *reference* currently reaching those cells is being bypassed.

**Reclaimed objects.** Storage reached through a raw pointer holds a
value CobaltC can reason about only once it is *reclaimed*: an object
of `storage-kind = reclaimed` is established over the cells, with
`init = valid` and, for a resource type, a fresh destroy authority —
the asserted claim (`inv.trust-transition`'s "explicit trust"
transition). Re-attaching an existing reclaimed object takes its
destroy authority for the reclaiming thread, `consumed` flag and all
(`CHG-0030`): a collection's elements, reached only below a raw
pointer, are destroyed by whichever thread the collection has moved
to, which is the author the trusted side-condition already holds
responsible. A reclaimed object persists until `[Release]`,
`[Rawptr-Move-Out]`, or `destroy`; it is not a temporary
(`spec/04` §2 `temporary`), and it has no holder unless bound. Every
access to it thereafter goes through ordinary access paths with every
ordinary check: two references obtained by two `reclaim` calls to the
same cells are two root paths on the same object, so `clash` sees
them. A raw access that bypasses `reclaim` entirely (`[Rawptr-Read]`,
`[Rawptr-Write]`) must find no such object still alive at all —
`CHG-0019` tightened both to say so, closing the one way a raw write
could otherwise have silently changed a reclaimed object's value out
from under a live reference to it without going through this
paragraph's "ordinary check" at all. `[Rawptr-Move-In]` moves a whole object into raw storage as a
reclaimed object, preserving its identity's obligations under a new
identity (no forgetting, no double authority); `[Rawptr-Move-Out]`
is its inverse. A `rawptr` obtained from a reference by `rawptr_of`
and passed to `extern` code is the FFI escape hatch; what the foreign
code does is covered by `[Extern-Call]`'s assertion.

**Depends on:** inv.spatial-validity, inv.initialization-validity,
inv.alias-validity, inv.trust-transition, inv.resource-authority,
D-0017, D-0019, rule.value-object.object-establish,
rule.value-object.object-end, rule.resauth.relocate-in,
rule.arith.represent
**Affects:** state.objects, state.storage, state.access-paths,
state.authority, state.destruction-obligations

## 3. `extern` functions

### `rule.trust.extern-call`
**Status:** ACCEPTED

    extern fn name(τ1 p1, …, τn pn) : τr;        -- spec/22 §3; every τi, τr ∈ FfiType
    FfiType ::= integer types | f32 | f64 | bool | void | rawptr<τ>

    [Extern-Call]   disposition: trusted-unchecked
        v1 : τ1, …, vn : τn evaluated left to right (values; FfiTypes are never resources or places)
        ────────────────────────────────────────────
        ⟨name(v1,…,vn), Σ⟩ → ⟨claim : τr, Σ[ trust(claim) := unchecked-claim,
                                             storage := any Σ' ⊇ Σ.storage on cells not part of any live
                                             fresh-storage object or reclaimed object ]⟩
        side-conditions: ⟦ the callee honors the FfiType signature; it reads/writes only cells reachable from
                            the rawptr arguments and its own allocations; the returned image is a valid τr ⟧
                          discharge: trusted

    [Extern-Non-Ffi-Type]   disposition: rejected
        some τi or τr ∉ FfiType
        ────────────────────────────────────────────
        ill-formed; diag.extern-non-ffi-type

    [Trust-Transition]
        claim : τ with Σ.trust(claim) = unchecked-claim;  validate is a CobaltC function (τ) -> Result<τ', E>
        ⟨validate(claim), Σ⟩ →* ⟨Ok(v), Σ'⟩
        ────────────────────────────────────────────
        v is internally produced (origin-of-value(v) = internal); this is the transition
        `external representation → unchecked claim → validation → established value`

Only types whose every cell image is a valid value cross the boundary,
so an extern result *is* a value of its declared type — but a value
whose relationship to anything else (that an `i32` is a valid index,
that a `rawptr` points at `n` valid bytes) is a claim. Every
safety-relevant use of such a value is either already checked by the
consuming rule (`[Index-Checked]`, arithmetic) or is itself trusted
(`[Reclaim]`, `[Rawptr-Read]`). `[Trust-Transition]` is the pattern
libraries use to turn a claim into a typed fact under a checked
validator (`spec/21` §2 `String::from_utf8`); `state.trust` records
provenance for diagnostics and for `inv.trust-transition`'s statement,
not as an additional runtime gate. An `extern` call needs `unsafe`
(§1). Calling convention, name mangling, and linking are outside this
specification (Master Instructions §1); a conforming implementation
documents them.

**Depends on:** inv.trust-transition, term.trust-boundary, D-0017,
rule.trust.unsafe
**Affects:** state.trust, state.storage

### `rule.trust.extern-code`
**Status:** ACCEPTED

    extern "c";                                  -- spec/22 §3 extern-code; c a str-literal

`extern "c";` names foreign code — a source file, a compiled object, or
a library — that the program's `extern fn` declarations may be found
in. It may appear wherever an item may. It declares no name: it adds
nothing to any module, takes no visibility, and changes no name
resolution, typing or evaluation rule. Any number may appear, in any
modules; declaring the same code twice is not an error.

Which strings an implementation accepts, how it resolves them (a
relative path, for example, against the declaring file's directory, as
`[Module-File]` resolves a module's), and how it links them are
implementation-defined and must be documented, as calling convention
and linking are (`rule.trust.extern-call`). An implementation that
does not link foreign code accepts every `extern "c";` and runs the
program otherwise unchanged; an `extern fn` it then cannot call is its
documented limit, as any unavailable `extern fn` is.

**Depends on:** rule.trust.extern-call, D-0030

## Change Log

- 1.9.0 — `CHG-0039` (D-0030): `rule.trust.extern-code`, the `extern "c";`
  item naming foreign code to link, its meaning implementation-defined.
- 1.8.0 — `CHG-0031` (owner-directed): `[Rawptr-Write]` of a value that
  holds references re-attaches or establishes a reclaimed object over
  the written cells as their holder, as `[Rawptr-Move-In]` already did
  for resources. Before, a plain `Vec` element holding a reference was
  held by nothing once `push` returned, so `[Stmt-Exit]` invalidated it
  and a `Vec<ref<…>>` could not be used.
- 1.7.0 — `CHG-0030` (owner-directed): `[Reclaim]`, when it re-attaches
  an existing reclaimed object, and `[Rawptr-Move-Out]` re-key the
  object's destroy authority to the performing thread, keeping its
  `consumed` flag. Before, a re-attached object kept the authority of
  the thread that first moved it into raw storage, so a `Vec` of
  resources moved to another thread (or returned through `join`)
  faulted `diag.no-destroy-authority` when that thread dropped it.
- 1.6.0 — `CHG-0019`: `[Rawptr-Read]`/`[Rawptr-Write]`'s trusted
  conditions no longer exempt the target cells' own `reclaimed-at(addrs)`
  object from the "no live derived path reaches them" requirement —
  the exemption was never actually needed by any correct call site
  (every legitimate write targets either never-reclaimed or, after
  this record's `Vec::pop` fix, always-released cells) and its
  presence is what let a stale-but-still-referenced plain element
  survive a `pop`+`push` reuse of its slot undetected
  (`spec/AUDIT-STATUS.md` finding F-05). Now symmetric with
  `[Rawptr-Move-In]`'s already-strict "not part of any live object"
  condition.
- 1.5.0 — `CHG-0011`: `[Rawptr-Move-Out]` re-attaches lazily like
  `[Reclaim]` — after `Vec::grow` copied a buffer and released its
  reclaimed elements, `Vec::pop` of a resource element had no rule;
  `[Release]`'s trusted condition restated as the obligation
  hand-over `grow` actually performs (the 1.4.0 wording was false for
  it). Found deriving `ex.e2e-vec-nested-realloc`.
- 1.4.0 — `CHG-0010`: `[Rawptr-Read]`/`[Rawptr-Write]`'s trusted
  conditions restated over derived paths; the 1.3.0 wording ("no live
  path … conflicts") was false whenever the cells belonged to a local
  whose own binding path existed, which is every raw access to a local
  (`conf.rawptr-deref-inside-unsafe-ok`).
- 1.3.0 — `CHG-0009`: `[Reclaim]`'s trusted side-condition now says
  what its own premise already implements — the cells may be claimed
  by a *reclaimed* object (which is re-attached), only not by a live
  fresh-storage object; the 1.2.0 wording ("no other object") made the
  rule's re-attachment premise unreachable without a false assertion.
  Non-normative in the same pass: `[Reclaim]`'s subject written as
  the intrinsic is spelled (`reclaim<τ>(p)`, `spec/21` §0).
- 1.2.0 — `CHG-0007`: `FfiType`'s `unit` alternative re-spelled `void`
  to match its real surface token (`spec/22` 2.6.0) — the other
  alternatives here (`f32`, `f64`, `bool`, `rawptr<τ>`) were already
  literal tokens; `unit` was the odd one out. `FfiType`'s membership
  (which types may cross `extern`) is unchanged.
- 1.1.0 — `CHG-0001`: `extern fn` signature illustration re-spelled to
  `spec/22` 2.0.0 syntax; no rule semantics changed.
- 1.0.0 — Rewritten per D-0019 (`spec/AUDIT-2.md` B-05, B-11, B-17,
  B-20): reclaimed objects defined as persistent, re-attachable
  objects; `[Rawptr-Move-In]`/`-Out]` move resources across the raw
  boundary without forgetting or duplicating authority; `[Rawptr-Of]`,
  `[Release]`, `[Copy-Raw]` added for `spec/21`; `FfiType` restricts
  what crosses `extern`; `[Trust-Transition]` stated as the library
  pattern it is. All entities `ACCEPTED`.
- 0.2.0 and earlier — superseded.
