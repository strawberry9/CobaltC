# CobaltC Preservation Lemmas

Status: proof artifact (non-normative: it proves, rule by rule, what `spec/03` states and `spec/04`–`21`'s rules must keep; D-0262)
Version: 1.0.1
Conforms to: `spec/02-schema.md` §5 (a proof artifact, kept by `impl/tools/speccheck.py`)
Governed by: `CobaltC_Master_Instructions.md` §6, §15, §20

## Purpose

`spec/03` states each invariant and names, coarsely, the rules that establish, preserve and invalidate
it. This artifact makes the preservation claim per rule: for every invariant whose proposition is a
property of a state `Σ`, and for every rule `R` with `⟨e, Σ⟩ →R ⟨r, Σ'⟩`, `inv(Σ) ⇒ inv(Σ')`. The proof is
by cases on `R`. A rule that writes no component the invariant ranges over preserves it trivially — the
*frame case*, which covers 622 of the 675 rule blocks. For each rule that does write one, a row below names
the premise or the construction that does the work. The set of writing rules is not kept by hand:
`speccheck` reads every rule block of `spec/05`–`21`, finds the components it assigns (`Σ[ c := … ]`,
`c(…) := …`, `c.f := …`), and reports as an error any writer of an invariant's components that its table
does not list, and as a warning any listed rule that writes none of them. A rule change that touches a
component therefore fails `speccheck` until its lemma is updated.

For the invariants whose proposition quantifies over an access rather than a state (`inv.temporal-validity`,
`inv.alias-validity`, `inv.concurrency-validity`, `inv.spatial-validity`, `inv.initialization-validity`,
`inv.trust-transition`), preservation means two things, and each section states both: the *auxiliary
state lemma* the access premise relies on (that a valid path's object is alive, that `held-by` is the set
it is defined to be, that a locked mutex has its guard), proved per writing rule; and the *access lemma*,
that every rule performing such an access has the proposition's premise, listed by rule. `inv.identity`,
`inv.origin-stability` and `inv.resource-authority` are state properties outright.

What this artifact is not: a mechanised proof. Each row is an argument a reader checks against the rule's
text; the model checker (`impl/model`, D-0219) exercises the alias and thread fragment, and the verify
modes (D-0261) re-derive the tools' bookkeeping from their states. The mechanised fragment is a separate
track (`private/verification-plan-2026-10-17.md` A3).

## 0. Establishment: `Σ_0`

`[Program]` (`spec/15` §7) builds `Σ_0` with `items`, one thread `ℓ_0` with empty frame and scope stacks,
one object per `static` item (storage-kind `static`, holding its first value, `init` valid, never ended,
D-0258) and every other component empty. Every invariant below holds of `Σ_0`: there is no path, so every
path-quantified proposition is vacuous; the static objects have fresh identities and origins and their
extents do not overlap (`[Object-Establish-Plain]` places each in fresh storage, `spec/17` §1a′); no
resource obligation, authority, lock or trust entry exists (a `static` is an atomic, a plain value).

## 1. `inv.identity`

**Components:** objects

**State lemma.** `Σ.objects` is a partial function (`spec/04` §3), so two live objects never share an
identity. Preservation is that no rule gives an existing identity a second record.

| Rule | Writes | Preserved because |
|---|---|---|
| `[Object-Establish-Resource]`, `[Object-Establish-Plain]` | `objects(o) := …` | premise `o ∉ dom(Σ.objects)`: the identity is fresh |
| `[Object-End]` | `objects := Σ.objects \ {o}` | removal only; the identity is never reused, since establishment requires it absent and nothing re-adds a removed one |
| `[Stmt-Exit]` | `objects(o).temp-scope := t_parent` | re-stamps a field of an existing record; `dom(objects)` unchanged |
| `[Join]` | `objects(o).temp-scope := current-scope` | as `[Stmt-Exit]` |
| `[Program]` | `objects := one per static item` | the static objects of `Σ_0`, each a distinct fresh identity |
| `[Rawptr-Write]` | `objects(o') := …` for a fresh `o'`, or none | either the existing reclaimed object over those cells (`reclaimed-at`) is reused, no new record, or `o'` is fresh |
| `[Rawptr-Move-In]` | `objects(o') := …`, then `end-object(o_src)` | `o'` fresh; the end is `[Object-End]` |
| `[Reclaim]` | `objects(o) := …` or re-attachment | `reclaimed-at` reuses the one record over exactly those cells; otherwise a fresh identity |

## 2. `inv.origin-stability`

**Components:** objects

**State lemma.** No rule assigns `origin` to an existing record. Every writer of `objects` either creates a
record with `origin: this-event` (`[Object-Establish-*]`, `[Rawptr-Write]`, `[Rawptr-Move-In]`,
`[Reclaim]`, `[Program]`), removes a record (`[Object-End]`), or assigns `temp-scope` only (`[Stmt-Exit]`,
`[Join]`). Relocation (`[Relocate-In]`, `[Relocate-Out]`, `[Rawptr-Move-In]`, `[Rawptr-Move-Out]`) ends one
identity and establishes another; it never rewrites an origin.

| Rule | Writes | Preserved because |
|---|---|---|
| `[Object-Establish-Resource]`, `[Object-Establish-Plain]`, `[Program]` | new records | `origin` set once at creation |
| `[Object-End]` | removal | nothing to preserve for an ended object |
| `[Stmt-Exit]`, `[Join]` | `temp-scope` | `origin` untouched |
| `[Rawptr-Write]`, `[Rawptr-Move-In]`, `[Reclaim]` | new records, or reuse | a reused record (`reclaimed-at`) is unchanged; a new one has its own origin |

## 3. `inv.spatial-validity`

**Components:** access-paths, objects

**Auxiliary state lemma (S).** For every valid path `a` with `alive(of(a,Σ),Σ)`: `target(a,Σ) ⊆
extent(of(a,Σ),Σ)`. Targets never change after formation, extents never change for an object's life, so
(S) reduces to: every path-forming rule sets `target` within the extent of `of`.

| Rule | Writes | Preserved because |
|---|---|---|
| `[Binding-Form]`, `[Temp-Root]`, `[Reclaim]`, `[Destroy-Composite]` (`a_i`) | a root path | `target := extent(o)` (a root), or `sub-range(o, p_i)` ⊆ `extent(o)` (`[Destroy-Composite]`, `rule.agg.layout`) |
| `[Borrow]` | a path with `target(a0)` | the same target as its base, which satisfies (S) |
| `[Field-Access]`, `[Index-Checked]` | `sub-range(of(a0), path(a0)·f)`, `·n` | a sub-range of its base's target by `rule.agg.layout`; the index bound `0 ≤ n < N` is the premise (the one dynamic check) |
| `[Lock]` | `sub-range(o, inner)` | the mutex's interior, inside its extent |
| `[Write]`, `[Store-Sub-Value]`, `[Store-Result-Place-Release]`, `[Authority-Transfer]`, `[Run-Destructor]`, `[Relocate-In]`, `[Relocate-Out]`, `[Stmt-Exit]`, `[Thread-Affine]`, `[Thread-Lend]`, `[Join]`, `[Guard-Drop]`, `[Recv]` | `valid`, `held-by`, `temp-scope` or `thread` of existing paths | no `target` or `of` is written |
| `[Object-End]` | `valid := false` for every path of `o`; `objects \ {o}` | a path of an ended object is invalid, so (S) is vacuous for it |
| `[Object-Establish-Resource]`, `[Object-Establish-Plain]`, `[Rawptr-Write]`, `[Rawptr-Move-In]`, `[Program]` | new objects | no path targets a new object yet |

**Access lemma.** `[Read]` and `[Write]` reach `target(a,Σ)` only; with (S) and `alive` (a premise of both)
the addresses are inside a live object, which is `addr-in(a, of(a,Σ), Σ)`. Raw storage (`[Rawptr-Read]`,
`[Rawptr-Write]`, `[Copy-Raw]`) is outside the invariant by `rule.trust.rawptr` (asserted).

## 4. `inv.temporal-validity`

**Components:** access-paths, objects, holder, temp-scope-stack

**Auxiliary state lemma (T1).** `valid(a,Σ) ⇒ alive(of(a,Σ),Σ)`: a valid path's object is alive.
**Auxiliary state lemma (T2).** `held-by(a,Σ) = { o | alive(o,Σ) ∧ a ∈ refs-in(contents(o,Σ)) }`, the
definition `spec/04` §1 gives, kept as a field.

| Rule | Writes | Preserved because |
|---|---|---|
| `[Binding-Form]`, `[Borrow]`, `[Field-Access]`, `[Temp-Root]`, `[Index-Checked]`, `[Lock]`, `[Reclaim]`, `[Destroy-Composite]` | a new path, `valid: true`, `held-by: ∅` | (T1): formed from a live object (`alive(o)` or `temporally-valid(a0)` premise, which implies `alive(of(a0))` by (T1)); (T2): a fresh token is in no object's contents, so `∅` is right; `[Lock]`'s `a_g.held-by := {o_g}` agrees with the guard's contents (D-0240) |
| `[Object-End]` | `valid := false` for `A_o ∪ Dead`; `held-by \ {o}` for `H_o`; `objects \ {o}` | (T1): every path of `o` is made invalid as `o` ends; (T2): `o` leaves every `held-by` it was in; `Dead` (paths held only by `o`, outside the block result's references `E`) are invalidated, which (T1) allows and `rule.control` requires |
| `[Write]` | `held-by` for `gone` and `refs-in(v)`; `valid := false` for `gone` held by `o` alone with no open scope | (T2): `gone` = the references `o` no longer holds anywhere (`kept` counts every copy, D-0215), `refs-in(v)` the ones now stored; (T1) untouched |
| `[Store-Sub-Value]` | `held-by ∪ {o}` for `refs-in(v)` | (T2): the value is now in `o`'s contents |
| `[Store-Result-Place-Release]` | `holder \ {o}`; `access-paths(a0).valid := false` | the owner path of a resource being moved out ends; (T1) allows invalidation |
| `[Authority-Transfer]` | `access-paths(a1).valid := false`; `holder(o) := a2` | the source path ends on transfer; `a2` was formed by `[Binding-Form]` on a live `o` |
| `[Run-Destructor]` | `access-paths(a_d).valid := false` | the destructor's borrow ends with the call |
| `[Relocate-In]` | `held-by: (… \ {o_src}) ∪ {o}`; `holder \ {o_src}`; then `end-object(o_src)` | (T2): the references `o_src` held are now in `o`'s cells (its storage was copied); (T1) by `[Object-End]` |
| `[Relocate-Out]` | `held-by: (… \ {o}) ∪ {o_new}` for references in the moved sub-range; `storage(sub-range) := uninit` | (T2): those references leave `o` (its cells are now `uninit`) and enter `o_new` |
| `[Stmt-Exit]` | `valid := false` for `Unheld`; `temp-scope` re-stamped for the result | `Unheld` are paths of this scope held by nothing and not owners: ending them keeps (T1) (invalidation is always allowed) and is what `rule.control.stmt` means; the result's paths outlive the scope by the re-stamp |
| `[Thread-Affine]`, `[Thread-Lend]` | `thread` | neither `valid` nor `held-by` |
| `[Join]`, `[Recv]` | `temp-scope` of the result's paths | as `[Stmt-Exit]` |
| `[Guard-Drop]` | `access-paths(a_g).valid := false` | the lock path ends with its guard |
| `[Object-Establish-Resource]`, `[Object-Establish-Plain]`, `[Rawptr-Write]`, `[Rawptr-Move-In]`, `[Program]` | new objects; `[Rawptr-Write]` updates `held-by` for `refs-in(v)` as `[Write]` | a new object holds no reference yet (its cells are `uninit`), so (T2) is unchanged; (T2) for the reclaimed holder `o'` of `[Rawptr-Write]` as for `[Write]` |
| `[Store-Binding-Temp]` | `holder(o) := a` (adoption) | the binding path was formed on a live `o` |
| `[Stmt-Enter]`, `[Spawn]` | `temp-scope-stack` pushed (a new scope; a new thread's empty stack) | no path's `valid` or `held-by` changes; a pushed scope is open, so a path stamped with it is held by its statement |

**Access lemma.** `temporally-valid(a,Σ)` (`spec/04` §2: `valid(a) ∧ alive(of(a))`, the latter implied
by (T1)) is a premise of `[Binding-Lookup]`, `[Read]`, `[Write]`, `[Borrow]`, `[Field-Access]`,
`[Index-Checked]`, `[Authority-Transfer]`, `[Destroy]`, `[Relocate-In]` (its valid-path premise),
`[Atomic-Op]`, `[Match]`, `[Join]` (its handle place), and of every `spec/21` operation through `[Read]`
and `[Write]`. The failure forms are `[Read-Stale]`, `[Write-Stale]`, `[Binding-Lookup-Stale]`
(`diag.stale-binding`) and the static `rule.temporal.ref-escape` (`diag.reference-escapes-scope`).

## 5. `inv.initialization-validity`

**Components:** init, storage

**Auxiliary state lemma (I).** `init(o,Σ) = valid ⇒` the cells of `extent(o,Σ)` hold a datum that
represents a value of `type-of(o,Σ)` (no `uninit` cell), and `init(o,Σ) = uninitialized ⇒` no rule has
read `o`. D-0019 admits no partial state.

| Rule | Writes | Preserved because |
|---|---|---|
| `[Object-Establish-Resource]`, `[Object-Establish-Plain]` | `init(o) := uninitialized`; `storage(x) := uninit` | a new object is uninitialized with `uninit` cells: (I)'s first part is vacuous |
| `[Write]` | `storage := S'`; `init(o) := valid` | `init-ok`: a whole-object write from any state, or a projection write into a `valid` object, so after it every cell of `o` holds a datum (`represent(type(a), v)` over the target; the rest were already data) |
| `[Store-Sub-Value]` | `storage(sub-range) := represent(τ, v)` | a field of an object under construction; its `[*-Construct]` rule sets `init(o) := valid` only after every field (`[Struct-Construct]`: `i = 1..n`; `[Array-Construct]`; `[Enum-Construct]`: payload and discriminant) |
| `[Struct-Construct]`, `[Array-Construct]`, `[Enum-Construct]` | `init(o) := valid` | every field, element, payload and the discriminant has been stored first, so every cell holds a datum |
| `[Object-End]` | `init \ {o}`; `storage \ extent(o)` | removal |
| `[Relocate-In]` | `storage(sub-range(o,path)) := storage(extent(o_src))` | `o_src` is `valid` (premise), so its cells are data, copied cell for cell into a `valid` `o` |
| `[Relocate-Out]` | `init(o_new) := valid`; `storage(extent(o_new)) := …`; `storage(sub-range(o,path)) := uninit` | `o_new`'s cells are the data of a `valid` sub-range; the source sub-range becomes `uninit` inside a still-`valid` `o` — the one exception to "no `uninit` cell in a valid object", admitted because the moved-from sub-range is a resource whose obligation moved with it, and no rule reads it: `[Match]`'s consume destroys what is left, `[Store-Binding-Place-Transfer]` ends the binding (`spec/05` §Store) |
| `[Spawn]` | `init(o_h) := valid`; `storage(extent(o_h)) := represent(handle, ℓ')` | the handle's cells are written in the same step |
| `[Mutex-New]` | `init(o) := valid` after `store(sub(o, inner))` | the interior is stored first |
| `[Rawptr-Write]`, `[Rawptr-Move-In]`, `[Reclaim]`, `[Rawptr-Move-Out]` | `init(o') := valid` over raw cells | `discharge: trusted`: the side-condition asserts the cells hold a valid `τ` (`[Reclaim]`, `[Rawptr-Move-Out]`) or they were just written (`[Rawptr-Write]`, `[Rawptr-Move-In]`); `[Rawptr-Move-Out]` leaves its source `uninit` and ends its object |
| `[Atomic-Op]` | `storage(atomic-cell(o)) := represent(τ, new)` | a datum replaces a datum |
| `[Copy-Raw]`, `[Extern-Call]`, `[Allocate]`, `[Deallocate]`, `[Str-Ptr]`, `[Read]` (`spec/21` §2b), `[Arg-Bytes]` | raw storage | outside every live fresh-storage object by their side-conditions (`[Allocate]`: fresh cells; `[Deallocate]`: `release` first); (I) concerns objects only |

**Access lemma.** `init-state(of(a,Σ),Σ) = valid` is a premise of `[Read]` (`[Read-Uninitialized]`) and of
`[Match]`; `[Write]`'s `init-ok` admits `uninitialized` only for a whole-object write. Statically,
`rule.init.definite-assignment` rejects on `unknown`.

## 6. `inv.resource-authority`

**Components:** authority, destruction-obligations, holder, objects, frame-stack, temp-scope-stack

**State lemma (R).** For every `r ∈ Σ.destruction-obligations`, exactly one of: `r`'s holder's frame is
open (`holder(r)`'s `frame ∈ Σ.frame-stack`); `r` is a temporary of an open statement scope
(`r ∉ dom(holder)`, `objects(r).temp-scope` on `Σ.temp-scope-stack`); `r = (o, path)` with `o` live; `r`
is a reclaimed object; `r ∈ objs-in(Σ.threads(ℓ').value)` for a finished thread `ℓ'` whose result is not
`taken` and whose handle is an obligation (`CHG-0330`: the handle's `[Join]` or `[Handle-Destructor]` claims
or discards the result). And `authority(ℓ, destroy, r, Σ)` holds for exactly one performer `ℓ` while `r` is an
obligation, unconsumed.

| Rule | Writes | Preserved because |
|---|---|---|
| `[Object-Establish-Resource]` | `destruction-obligations ∪ {o}`; `authority(ℓ, destroy, o) := {consumed: false}` | `o` is a temporary of the current scope (`temp-scope := current-scope`), one performer, the establishing thread |
| `[Object-Establish-Plain]`, `[Program]` (its static objects) | a new plain object | no obligation and no authority: (R) ranges over obligations only |
| `[Rawptr-Write]` | a reclaimed holder `o'` for the references written | `o'` is a plain reclaimed object (`¬is-resource(τ)`): no obligation; it holds the references' `held-by`, not an authority |
| `[Store-Binding-Temp]`, `[Binding-Form]` | `holder(o) := a` | the temporary becomes owned by a binding of the current frame, which is open: the first case replaces the second |
| `[Store-Result-Place-Release]` | `holder \ {o}` | the owned resource becomes a temporary of the current statement (the store's scope), the second case |
| `[Authority-Transfer]` | `holder(o) := a2`; across threads, `consumed := true` for `ℓ` and a fresh token for `ℓ2` | the obligation moves from one open frame's binding to another's (`a2`'s frame; for `[Spawn]`, the new thread's parameter frame), and the performer is re-keyed in the same step, so one unconsumed token remains |
| `[Destroy]` | `consumed := true`; `destruction-obligations \ {o}` | the obligation is discharged by its authority holder (`authority(ℓ, destroy, o)` premise) exactly once: the token is consumed in the step that removes the obligation |
| `[Destroy-Composite]` | `consumed := true`, `\ {(o, p_i)}` for each sub-resource | each sub-obligation of `o` is discharged once, longest path first, before `o` ends |
| `[Relocate-In]` | obligations and authority re-keyed from `o_src` (and its sub-resources) to `(o, path…)`; `holder \ {o_src}`; `end-object(o_src)` | the third case (a sub-resource of a live `o`) replaces whatever case `o_src` was in; tokens move with the obligations, the `(ℓ, destroy, o_src…)` entries removed in the same step |
| `[Relocate-Out]` | obligations and authority re-keyed from `(o, path…)` to `o_new` (and `(o_new, q)`) | the second case (a temporary) replaces the third; one token per obligation moves |
| `[Object-End]` | `holder \ {o}`; `objects \ {o}` | an ended object has no obligation: `[Destroy]` removed it first (`[Object-End]` is reached through `[Destroy]` for a resource), or `o` was plain |
| `[Thread-Body-Done]` | `threads(ℓ').value := r_b`; the parameter frame's `[Block-Exit]` (`block'(f', r_b)`) | the frame's owned objects other than the result's are destroyed or ended before it pops; the result's objects lose their bindings and pass from the first case to the fifth: `ℓ'`'s handle is an obligation (its destructor blocks until `value` is set, so it has not run) |
| `[Join]`, `[Handle-Destructor]` | `authority` of the result's objects re-keyed from `ℓ'` to the joiner; `threads(ℓ').value := taken` | the worker's token is consumed and the joiner's created in one step; the result's objects pass from the fifth case to the second (`[Join]`: temporaries of the joiner's scope, `temp-scope` re-stamped) or are destroyed with the handle (`[Handle-Destructor]`, the result not taken) |
| `[Reclaim]`, `[Rawptr-Move-In]`, `[Rawptr-Move-Out]` | a reclaimed object's obligation and authority, created or re-keyed to the reclaiming thread (`CHG-0030`) | the fourth case; `discharge: trusted` asserts the obligation is tracked nowhere else |
| `[Block-Enter]`, `[Block-Exit]`, `[Call]`, `[Match]`, `[Spawn]`, `[Program]` | `frame-stack` | a frame pushed opens nothing's holder yet; `[Block-Exit]` destroys or ends every object the frame owns (`Owned`, last bound first) before popping it, so no obligation's holder frame closes while the obligation stands; a frame's bindings are dropped with it |
| `[Stmt-Enter]`, `[Stmt-Exit]`, `[Write]`, `[Program]` | `temp-scope-stack` | `[Stmt-Exit]` destroys every temporary of the scope (`Temps`) not in the result before popping it, and re-stamps the result's objects to the parent scope; `[Write]` reads the stack only (its `Unheld` condition) |
| `[Fault-Contain]` | (via `unwind-to`) | the unwind runs `[Block-Exit]`/`[Stmt-Exit]` for the thread's frames: every obligation they own is discharged before the thread stops |

**Access lemma.** `authority(ℓ, destroy, o, Σ)` is a premise of `[Destroy]` and `[Authority-Transfer]`;
`solitary(a, Σ)` of both. `[Write]`'s `¬live-resource-at` keeps an owned resource from being overwritten
without destruction. The structural leak-freedom is `spec/07` §5.

## 7. `inv.alias-validity`

**Components:** access-paths, sync

**Access lemma.** The proposition (`spec/03`, D-0018) is `¬clash(a, m, Σ)` at every access of mode `m`
through `a`. It is a premise of `[Read]` (`shared`), `[Write]` (`exclusive`), `[Borrow]` (`m`), `[Match]`
(`shared`), `[Atomic-Op]` (`shared`, ancestors excluded), `[Thread-Lend]`'s borrows, and every `spec/21`
operation through them. The failure forms are `[Read-Conflict]`, `[Write-Conflict]`, `[Borrow]`'s own
(`diag.aliasing-conflict`), `[Write-Not-Exclusive]` (`diag.write-through-shared`), and `[Destroy]`'s
`solitary` (`diag.destroy-while-aliased`, `diag.move-while-aliased`).

**Auxiliary state lemma (A).** `clash` is well defined: `ancestors(a, Σ)` is finite and acyclic, because
`base` is set once at formation to a path that already exists (`[Borrow]`: `a0`; `[Field-Access]`,
`[Index-Checked]`: `a0`; `[Lock]`: `a_m`; roots: `None`) and is never rewritten; and `pending(a, Σ)` (a
call argument's path from formation until `[Call]` binds it, D-0107, or a path formed for one access not
yet made, D-0242) ends when `[Call]`'s `store(binding(p_i))` runs or the access is made.

| Rule | Writes | Preserved because |
|---|---|---|
| `[Binding-Form]`, `[Temp-Root]`, `[Reclaim]`, `[Destroy-Composite]` | a root, `base: None` | a root has no ancestors |
| `[Borrow]`, `[Field-Access]`, `[Index-Checked]`, `[Lock]` | `base: a0` (`a_m`) | the base exists and was formed earlier (`formed-at` strictly earlier), so the chain is finite |
| `[Write]`, `[Store-Sub-Value]`, `[Store-Result-Place-Release]`, `[Object-End]`, `[Authority-Transfer]`, `[Run-Destructor]`, `[Relocate-In]`, `[Relocate-Out]`, `[Stmt-Exit]`, `[Join]`, `[Guard-Drop]`, `[Recv]` | `valid`, `held-by`, `temp-scope` | `base` and `mode` untouched; an invalidated path no longer counts in `clash` (`temporally-valid` is its first conjunct) |
| `[Thread-Affine]`, `[Thread-Lend]` | `thread` | `clash` is thread-independent; `[Thread-Affine]` is the separate premise that an exclusive path is used by its thread |
| `[Mutex-New]`, `[Lock]`, `[Guard-Drop]`, `[Fault-Contain]`, `[Atomic-Notify]` | `sync` | `sync-exempt` reads `lock-derived`, which is a property of ancestors (a `[Lock]`-formed path among them), not of `sync` |

Two coexisting conflicting paths are not a violation of the state (D-0107, D-0261): a call's arguments are
formed, pending, and then held in its parameters; the first access through either is the clash. What is
preserved is that no access ever passes a `clash`.

## 8. `inv.concurrency-validity`

**Components:** sync, threads, access-paths

**Auxiliary state lemma (C).** `sync(o,Σ) = locked-by(ℓ) ⇔` a valid lock path of thread `ℓ` on `o` exists
(formed by `[Lock]`, its guard alive); `poisoned(ℓ)` only after `[Fault-Contain]` of `ℓ` while it held the
lock; and `threads(ℓ).value` passes `None → r_b | failed(d) → taken` once.

| Rule | Writes | Preserved because |
|---|---|---|
| `[Mutex-New]` | `sync(o) := unlocked` | a new mutex, no lock path |
| `[Lock]` | `sync(o) := locked-by(ℓ)`; the lock path `a_g` | both sides of (C) set in one step; the premise `sync-state = unlocked` (blocking otherwise) keeps one holder |
| `[Guard-Drop]` | `sync(o) := unlocked` (unless poisoned); `a_g.valid := false` | both sides cleared in one step; a poisoned mutex stays poisoned (D-0215) |
| `[Fault-Contain]` | `sync(o) := poisoned(ℓ)` for `ℓ`'s locked mutexes; `threads(ℓ).value := failed(d)` | the contained thread's guards end in its unwind; the mutexes it held are marked, never unlocked, and `[Lock-Poisoned]` refuses them |
| `[Atomic-Notify]` | `sync(o) := wake-count(n)` | an atomic's count, read by `[Atomic-Wait]`; never a lock state |
| `[Spawn]`, `[Thread-Body-Done]`, `[Thread-Step]`, `[Join]`, `[Handle-Destructor]`, `[Program]` | `threads` | `value` is set once by `[Thread-Body-Done]` or `[Fault-Contain]` and claimed once (`taken`) by `[Join]` or `[Handle-Destructor]`, whose premises require it set; `[Thread-Step]` writes `expr` only |
| `[Thread-Affine]`, `[Thread-Lend]` | `access-paths(a).thread` | a lent exclusive path is `transit` until one thread takes it (`[Thread-Affine]` sets `thread := ℓ` on the first access, refusing a second running owner) |
| `[Binding-Form]`, `[Borrow]`, `[Field-Access]`, `[Temp-Root]`, `[Index-Checked]`, `[Reclaim]`, `[Destroy-Composite]`, `[Lock]` | a new path with `thread: ℓ` | the forming thread owns it; a lock path's `thread` is its locking thread and is never lent (`[Thread-Lend]`, D-0240) |
| `[Write]`, `[Store-Sub-Value]`, `[Store-Result-Place-Release]`, `[Object-End]`, `[Authority-Transfer]`, `[Run-Destructor]`, `[Relocate-In]`, `[Relocate-Out]`, `[Stmt-Exit]`, `[Join]`, `[Guard-Drop]`, `[Recv]` | `valid`, `held-by`, `temp-scope` | no `thread` field changes; `[Guard-Drop]` ends the lock path as (C) requires |

**Access lemma.** Across threads the proposition is discharged by `clash` at every access, which is
thread-independent, so an unsynchronized conflicting pair faults at the second access whatever the
interleaving (`spec/03`); a lock-derived path and a shared path to the mutex are `sync-exempt` because
`[Lock]` blocks while another thread holds the lock; an exclusive path is used by its own thread
(`[Thread-Affine]`). `rule.conc.guarantee` (`spec/19` §4) states the consequence: no data race.

## 9. `inv.trust-transition`

**Components:** trust, storage

| Rule | Writes | Preserved because |
|---|---|---|
| `[Extern-Call]` | `trust(claim) := unchecked-claim`; `storage` outside every live object | the result is marked; a validator (`[Trust-Transition]`) is the only rule that marks `transitioned`; the storage it may write is no object's |
| `[Read]` (`spec/21` §2b), `[Arg-Bytes]` | `storage` outside live objects | instances of `[Extern-Call]` |
| `[Object-Establish-Resource]`, `[Object-Establish-Plain]`, `[Write]`, `[Store-Sub-Value]`, `[Object-End]`, `[Relocate-In]`, `[Relocate-Out]`, `[Enum-Construct]`, `[Atomic-Op]`, `[Allocate]`, `[Deallocate]`, `[Str-Ptr]` | `storage` | writes a datum the program computed (`origin-of-value = internal`), copies cells of a live object, or makes or frees `uninit` cells: no claim is produced or transitioned |
| `[Rawptr-Write]`, `[Copy-Raw]` | raw `storage` | `discharge: trusted` inside `unsafe`: the use of what they write is under `rule.trust.unsafe`, the proposition's third disjunct |

**Access lemma.** `[Extern-Non-Ffi-Type]` confines what crosses; a claim is consumed only by a checked
rule (an FFI scalar) or under `discharge: trusted` (`rule.trust.unsafe`, rejected outside `unsafe`).

## 10. `inv.arith.range-validity`, `inv.string.utf8-validity`, `inv.str.utf8-validity`

These are not preserved but re-established: `inv.arith.range-validity` is checked fresh by every default
operation (`spec/06`: the rule yields a represented value or faults, never a value outside the domain), so
no state carries it. `inv.str.utf8-validity` holds of every `str` value by the grammar (`[Str-Literal]`)
and no rule writes into a `str`. `inv.string.utf8-validity` holds of every initialized `String` object:
the two constructors establish it (`from_utf8`'s validator, `from_str`'s copy of a `str`), and no exported
operation writes a byte of a `String` except through `std`'s own bodies, whose `unsafe` blocks are listed
with their claims in `spec/21` §0 (D-0206) — the writers of `storage` in §5's table reach a `String`'s
bytes only through those bodies.

## Change Log

- 1.0.1 — `CHG-0330` (D-0265): (R)'s fifth case and the `[Thread-Body-Done]` row.
- 1.0.0 — `CHG-0326` (D-0262): first version, over the 53 state-writing rule blocks of `spec/05`–`21`.
