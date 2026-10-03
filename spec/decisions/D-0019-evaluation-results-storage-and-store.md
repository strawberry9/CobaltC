# D-0019 — Evaluation Results, Storage Allocation, Representation, and the Store Operation

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §5, §6, §9, §11, §13, §16,
§17 (Initialization; Function Interfaces), §20
Depends on: inv.initialization-validity, inv.resource-authority,
inv.spatial-validity, term.object, term.value, term.storage, D-0003,
D-0006, D-0007, D-0008, D-0016, D-0018
Affects: state.objects, state.storage, state.holder, state.frame-stack,
state.temp-scope-stack, state.threads, rule.value-object.object-establish,
rule.value-object.object-end, rule.value-object.store,
rule.value-object.read, rule.value-object.write, rule.init.let,
rule.fn.call, rule.fn.return, rule.fn.bind-param, rule.agg.struct-construct,
rule.agg.array-construct, rule.agg.enum-construct,
rule.resauth.relocate-in, rule.resauth.relocate-out,
rule.control.stmt, rule.control.block, rule.conc.spawn

Realizes the fix for `spec/AUDIT-2.md` B-05, B-06, B-07, B-11, B-12,
B-13, B-18.

## Problem

The rules were written in three incompatible dialects about what an
expression produces and where its result lives:

1. `[Struct-Construct]` yields an object identity; `[Let-Value]`
   establishes a *second* object and writes that identity into it;
   `[Let-Resource]` demands an access path; nothing yields one for a
   call or a literal. `let v: Vec<i32> = Vec::new();` has no derivation
   (B-07).
2. `[Read]`/`[Write]` read and write `extent(o)` and type-check against
   the whole object's type, so a field access is either ill-typed or
   reads the whole struct (B-05).
3. `[Return]` and a trailing block expression are handled differently
   (the latter not at all), `[Call]` and `[Return]` both exit the
   parameter frame, and a returned resource's holder is never updated
   (B-06).
4. Every `establish-object` requires `addrs ⊆ dom(Σ.storage)` but no
   rule ever extends `Σ.storage`; `represent`/`value-at` are undefined
   (B-11).
5. `Performer` is undefined (B-12); the frame and statement-scope stacks
   are global while threads interleave (B-13); no entry point,
   initial state, or observable outcome is defined (B-18).

## Constraints

- §16: a false claim must not silently become a trusted fact along the
  chain — a resource moved into a field must leave exactly one
  authority, and a returned resource must have exactly one holder.
- §17 (Initialization): storage must never be read as a value it does
  not hold.
- §9: one store operation for every position a value can be put
  (`let`, parameter, field, element, payload, return), not one per
  syntactic form.
- §13: representation freedom must be tagged `outcome`, never implied.
- §20: an implementer must be able to determine every one of these
  decisions from the rules alone.

## Candidate mechanisms

### Result classification

1. **Everything is a value; aggregates are copied by value and
   authority is transferred by a separate mechanism.** Requires a value
   to carry authority "in flight", a second authority representation
   alongside `state.authority`. Rejected (§9, §16).
2. **Three result kinds: value, place, temporary object.** A scalar
   expression yields a *value* `v`; a name, field, index, or `*r`
   yields a *place* (an access path `a`); an aggregate literal, a
   call returning an aggregate or resource, `reclaim`, `spawn`, and
   `Field-Transfer-Out` yield a *temporary object* `o` — a live object
   with its own storage and authority but no holder yet. One `store`
   operation is defined for each kind against each slot kind.
   **Selected.**

### Store semantics

- **Top-level slot** (a `let`/parameter binding): a value is written
  into a freshly established object; a non-resource place is read and
  the value written into a fresh object (copy); a resource place is
  transferred (the new binding targets the same object, the source
  path is invalidated, `holder` moves); a temporary object is *adopted*
  (the binding targets it, `holder` set).
- **Sub-range slot** (a field, element, or payload of an object being
  constructed): a value is written; a non-resource place is read and
  written; a resource place or any temporary object is *relocated*
  (`[Relocate-In]`): its storage contents are copied into the sub-range,
  its top-level and composite authorities are re-keyed under
  `(o, path…)`, and the source object ends.
- **Result position** (`return e`, a block's trailing expression, a
  statement's value): a value stays a value; a non-resource place is
  read; a resource place becomes a temporary object (`holder` cleared,
  the path invalidated); a temporary stays a temporary.

The alternative — a single "move" primitive that always relocates —
was rejected because a top-level `let` of a resource has no reason to
copy storage (D-0003's transfer already moves authority without moving
bytes).

### Storage and representation

`[Object-Establish-*]` allocates: it chooses `addrs` fresh (disjoint
from `dom(Σ.storage)`), of size `sizeof(τ)`, aligned to `alignof(τ)`,
`outcome: unspecified { any such range }` — addresses of checked
objects are unobservable to a safe program, so this freedom is safe.
`ObjectRecord` gains `storage-kind ∈ {fresh, reclaimed}`;
`[Object-End]` releases `fresh` storage and leaves `reclaimed` storage
to its raw allocation. `Σ.storage` cells hold abstract contents;
`represent_τ`/`value-at_τ` are defined per type in `spec/06` §7 with
byte order `outcome: impl-defined { little-endian, big-endian }` and
enum discriminant width `outcome: impl-defined { 1, 2, 4, 8 }`; every
other encoding fact is determined.

### Threads and performers

`Performer ≝ Thread`; `state.frame-stack` and
`state.temp-scope-stack` become per-thread maps; every rule that reads
`top(...)` reads the current thread's stack. `current-thread` is the
label `ℓ` of the step (`spec/01` §2.3).

### Entry and termination

A program's items are declared in `Σ_0`; execution is the evaluation
of `main()` in thread `ℓ_0`. The observable outcome is
`terminate(ok, Σ)` when `main` returns, or `terminate(d, Σ)` when any
rule yields `↛ d` (`spec/18` §1). An implementation reports `d` and
distinguishes the two outcomes to its environment; how is not a
language concern.

## Selected design

Stated normatively in: `spec/04` (record shapes, per-thread stacks,
`Performer`), `spec/05` (`[Object-Establish-*]` allocation,
`[Object-End]` release, `[Read]`/`[Write]` over `target(a)` and
`type(a)`, `rule.value-object.store`), `spec/06` §7 (representation),
`spec/07` (`[Relocate-In]`/`[Relocate-Out]` replacing
`[Field-Transfer-In]`/`-Out]`), `spec/11` (`[Let]` as `store`),
`spec/13` §1 (result kinds, contexts), `spec/14` (statement/block result
exemption, temporaries destroyed at statement end), `spec/15` (`[Call]`
exits only the parameter frame; `[Return]` folds only frames pushed
inside the body; `main`), `spec/16` (construction yields a temporary),
`spec/18` (termination outcome), `spec/19` (`spawn` yields a temporary
handle; per-thread stacks).

**Partial initialization** is rejected rather than modeled: a write
through a projection requires `init(o) = valid`; an object becomes
`valid` only by a whole-object write or by construction. `partial(Step)`
and `partially-destroyed(Step)` remain unreachable (`spec/11` §2), now
by decision rather than by omission.

## Rejected alternatives

Values-carry-authority (1); relocate-always move; modeled partial
initialization (would need per-path init state for a capability no
example requires, §24).

## Semantic rationale

Classifying results makes every "where does this go" question a
lookup in one table. Temporaries are the minimal way to let an
aggregate or resource exist between the rule that creates it and the
rule that stores it without inventing a value-level authority carrier;
destroying unstored temporaries at statement end keeps D-0008's
leak-freedom structural for `make();` as much as for `let v = make();`.

## Usability implications

None visible beyond the rules now agreeing with the examples. A struct
must be initialized as a whole (`x = S{...}`), not field by field from
an uninitialized state.

## Explainability implications

A diagnostic can say "temporary destroyed at end of statement",
"relocated into `p.a`", or "adopted by `x`", each an event in `Σ`.

## Implementation-feasibility implications

Temporaries are the ordinary notion of an unnamed rvalue with a
destructor; relocation is a memcpy plus bookkeeping; the store table is
finite. Nothing here requires analysis beyond `stat.flow-analysis`.

## Compatibility impact

Extends D-0016 (holder tracking) to every top-level object, resource or
not, so that plain locals also end at frame exit. Revises the `spec/`
artifacts listed above, all `PROVISIONAL`, in place.

## Prior-art status

Value/place/temporary is the ordinary rvalue/lvalue/prvalue
distinction, rediscovered from the need to give `[Struct-Construct]`'s
result a home; retained on its merits.

## Invariant traceability

`inv.initialization-validity` (reads over `target(a)` at `init = valid`;
partial init rejected); `inv.resource-authority` (exactly one holder
after every store; temporaries destroyed at statement end);
`inv.spatial-validity` (allocation is aligned and disjoint; `target ⊆
extent` by construction of projections).

## Revisit conditions

Revisit partial initialization if a construct needs field-by-field
construction of a large aggregate; revisit temporaries if expression-
level parallelism is ever introduced.
