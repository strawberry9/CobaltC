# CobaltC Semantics Overview

Status: non-normative artifact (like `IMPLEMENTATION-NOTES.md` and
`STABILITY.md`: it carries no rule, invariant, type or state entity). It
explains the semantics for an implementer or a reviewer who knows systems
languages such as Rust, C++ and C but not CobaltC. It restates; it does not
add. Where it and the normative text differ, the normative text wins
(`spec/README.md`, "What is normative").
Version: 1.2.0

## Purpose

The normative chapters are written as rules over an abstract machine state,
each rule complete but terse, and the reasoning that ties them together is
spread over the rules' prose, the decision records and the conformance
derivations. This file walks through that reasoning in one place, in the
order a newcomer needs it: the state, how references come and go, the one
conflict predicate, what is decided before a program runs and what while it
runs, ownership and destruction, threads, the trust boundary, the lemma that
licenses checking a reference once at a function's entry, and the guarantee
the whole is built to give. Every paragraph names the rules it explains, by
label (`[Borrow]`), by entity id (`rule.alias.borrow`) and by section
(`spec/08` §2), so that each statement can be checked against its source.
Nothing here is a rule: a summary that says less than its rule is
deliberate, and a reader who needs the exact premises reads the rule
(`spec/README.md`, "What is normative"; `term.specification`).

## 1. What the specification is and how to read it

**The artifacts.** The specification is the directory `spec/`. Its
normative part is the rules written in CFN (`spec/01`) inside chapters `03`
to `21`, the state definition in `spec/04`, the grammar in `spec/22`, the
two registries and the conformance suite; prose in those chapters explains,
and where prose and a rule differ, the rule wins (`spec/README.md`, "What
is normative"; `term.specification`). Each chapter has a Status line, a
Version, and a Change Log at its end, and is the *home* of the entities of
its area (`spec/02` §2, §5). The decision records under `spec/decisions/`
are rationale, not rules (`spec/README.md`). Files such as `README.md`,
`IMPLEMENTATION-NOTES.md`, `STABILITY.md`, `MANIFEST.md`, the audit files and
this one carry no rule (`spec/02` §6; the Status lines of those files).

| Chapter | What it holds |
|---|---|
| `00` | the vocabulary: `term.*` entities |
| `01` | CFN, the notation: judgment forms, tags, rule format |
| `02` | the schema: entity kinds, ids, lifecycle, versioning |
| `03` | the invariants: `inv.*`, each a proposition over the state |
| `04` | the abstract state `Σ`: `state.*` components and every predicate |
| `05`–`20` | the rules, one area each: values and objects, arithmetic, resources, aliasing, references, escape, initialization, types, expressions, control flow, functions, aggregates, modules, faults, threads, trust |
| `21` | the standard library `std`, with its normative bodies |
| `22` | the surface grammar, and its table from productions to rules |
| `conformance.md` | the conformance cases, `conf.*`, each with a derivation |
| `registry/` | the diagnostics (`diag.*`) and the feature proposals (`feat.*`) |

**Entity ids.** Every normative item is an *entity* with a kind, a
permanent id, a status, and `Depends on` / `Affects` links (`spec/02` §1).
The id's prefix names the kind (`spec/02` §2): `term.` for terms,
`inv.` for invariants, `state.` for state components, `rule.<area>.<name>`
for rules, `type.` for types, `diag.` for diagnostics, `conf.` for
conformance cases, `ex.` for examples, `feat.` for features, and `D-nnnn`
and `CHG-nnnn` for decisions and change records. An id is never reused for
another meaning, even after its entity is removed (`spec/02` §2). In the
chapters an entity is a heading such as ``### `rule.alias.borrow` `` with
its fields beneath.

**Rule labels.** Inside a rule entity, each inference rule has a label in
brackets: `rule.alias.borrow` holds `[Borrow]`, `[Borrow-Stale]`,
`[Borrow-Exceeds-Source]` and `[Borrow-Denied]` (`spec/08` §2). The entity
groups a construct's rules; the label names one of them, and conformance
derivations and diagnostics cite labels (`spec/registry/diagnostics.md`,
Purpose). A rule has premises above a line, a conclusion below it, and
optional side-conditions, each tagged with how it is discharged (`spec/01`
§4, §6). Its conclusion is one of six judgment forms (`spec/01` §2): a
static judgment `Γ ⊢ J`; a state assertion `Σ ⊨ P`; a small step
`⟨c, Σ⟩ →^ℓ ⟨c', Σ'⟩` taken by thread `ℓ`; a diagnostic `⟨c, Σ⟩ ↛ D`, meaning
the construct has no next step and produces diagnostic `D`; the
program-termination judgment `⟨program, Σ⟩ ↛↛ terminate(d, Σ')`, which no
rule uses as a premise; and the invariant lifecycle judgments.

**Tags.** Three closed vocabularies classify rules (`spec/01` §4–§5). A
*disposition* says how a rule treats a precondition the invariants rely on:
`rejected` (the program is ill-formed), `checked` (a run-time check whose
failure is `↛`), `fallible` (failure is an ordinary result value),
`trusted-unchecked` (the condition is assumed, and allowed only inside
`unsafe`), or `unsupported`; a rule with no such precondition carries none
(`spec/01` §5). A *discharge* tag on a side-condition says who establishes
it: `static`, `dynamic` or `trusted`. An *outcome* tag marks a rule whose
result the premises do not fix: `impl-defined` (one documented choice per
implementation) or `unspecified` (chosen per execution from a documented
set given by listing or by a condition, D-0206). The common reading
"`discharge: static (rule.control.flow-analysis; dynamic where unknown)`"
is the subject of §5 below.

**Decisions and change records.** A semantic change is made by a decision
record `D-nnnn` (Problem, Options, Decision, Results) and a change record
`CHG-nnnn`, then the chapter text with its Version and Change Log; an
accepted decision is never edited, and a later one supersedes it
(`spec/02` §6). The decisions cited in this file are where to read *why* a
rule is as it is; the rule is still the authority (`spec/README.md`). The
whole specification is identified by its *edition*, `YYYY.MMDDnn`; the
manifest records what an edition consists of and how much testing stands
behind it (the evidence tiers), and `STABILITY.md` says what an edition
promises about later ones (`spec/README.md`, "Edition"; D-0212; D-0220).

**Conformance rows.** Each row of `spec/conformance.md` is a `conf.*`
entity with a program (an inline fragment or a case file under
`impl/conformance/`), the required outcome, and a derivation through the
rules (`spec/conformance.md`, Purpose and Conventions). Outcomes are
written `→ v` for a value, `ok` for normal termination, `✗ diag.x (static)`
for a rejection before the program runs, and `✗ diag.x (dynamic)` for a
fault at run time. An implementation conforms only if every case holds,
among the other conditions of `term.conformance`. The diagnostics registry
gives each `diag.*` its phase, the rules that produce it, the invariant it
protects, and a repair (`spec/registry/diagnostics.md`, Purpose).

**Reading order.** `spec/README.md` gives an implementer's reading order:
the grammar (`spec/22`), the state (`spec/04`), typing (`spec/12`), then
evaluation, aliasing and resources, the flow analysis of `spec/14` §6, and
threads, trust and `std`. This overview follows the same dependency order
but groups by idea, and is meant to be read after the schema (`spec/02`)
and before the rules themselves (`spec/README.md`, "Reading order for an
implementer").

## 2. The abstract state

**`Σ` and its components.** Every run-time rule is a transition on one
abstract state `Σ`, whose components are listed in `spec/04` §1; each exists
because a named rule reads or writes it (`spec/04`, Purpose). Its index sets
— identities, addresses, threads, events, access-path tokens, frames,
statement scopes, value tokens — are opaque and pairwise disjoint token sets
(`spec/04`, Purpose). The *performer* of a step is a thread; `ℓ` in every
rule is the thread taking the step (`spec/04`, Purpose; `spec/01` §2.3). The
components fall into five groups, described below: objects and storage;
access paths; ownership (bindings, holders, frames, statement scopes);
resources (obligations and authority); and threads, locks and trust.

**Objects and cells.** `state.objects` maps an identity to an object
record: the event that established it (`origin`), the set of addresses it
occupies (`extent`), its type, whether its storage is `fresh` or
`reclaimed`, the statement scope it was made in, and its `anchor` address
(`state.objects`). Because the map is a partial function, two live objects
cannot share an identity (`inv.identity`, by construction, `spec/04` §3),
and no rule rewrites an origin, so `inv.origin-stability` also holds by
construction. `state.storage` maps an address — a *cell* — to `uninit` or a
datum of the cell alphabet that `spec/06` §7 fixes (`state.storage`,
`term.storage`). Cells can exist with no object over them: a raw allocation
is storage before any object claims it, which is why storage and object are
two components, not one (`state.storage`). An object of a zero-sized type
has an empty extent and an anchor of its own, so two live ones still have
different addresses (`[Object-Establish-Resource]`, `[Object-Establish-Plain]`,
D-0217). `state.init` gives each object `uninitialized` or `valid`; there is
no partially initialized state (`state.init`, D-0019).

**Objects are not values.** A value is an element of a type's represented
domain, independent of where it is stored; an object is an identity with
storage, a type and an initialization state, which holds a value
(`term.value`, `term.object`). Reading an object yields a copy of the value
over its target cells (`[Read]`, via `value-at`, `rule.arith.represent`);
writing stores a value's image there (`[Write]`). Every expression reduces
to one of three *results*: a value `v`, a place `place a` (an access path),
or a temporary object `temp o` that nothing owns yet (`spec/05`, Purpose;
`rule.expr.context`). Which sub-expressions are consumed as places — the
operand of `&`, the target of `=`, the base of a projection, a resource
argument, a `match` scrutinee — and which are read as values is fixed by
`spec/13` §1 (`rule.expr.context`, "Place positions").

**Access paths and tokens.** The central component is `state.access-paths`:
a map from an *access-path token* to a record with a target (a set of
addresses within the object), the object it is `of`, the type of what the
target holds, a mode (`shared` or `exclusive`), the thread it belongs to (or
`transit`), a `valid` flag, the event and frame and statement scope of its
formation, a `base` (the path it was derived from, none for a root), and
`held-by` (`state.access-paths`). A reference value *is* such a token:
`ref<τ, m>`'s represented domain is the tokens whose path has type `τ` and
mode `m`, and copying a reference copies the token, so every copy names the
same path (`type.ref`). A reference type names a pointee type and a mode and
nothing else (`type.ref`), so a reference's validity is not a static region
attached to its type but a fact of the state, `temporally-valid(a, Σ)` — the
token is in the map, its `valid` flag is set, and its object is alive
(`spec/04` §2; `term.temporal-validity`). The static analyses of §5 below
predict that fact where they can.

**Roots, derived paths and ancestry.** A *root* path has no base: the path
a binding owns (`[Binding-Form]`), the path `reclaim` forms over raw
storage (`[Reclaim]`), and the fresh paths `[Destroy-Composite]` forms into
sub-resources (`state.access-paths`; `rule.resauth.destroy-composite`). A
*derived* path records its base: a borrow (`[Borrow]`) or a projection
(`[Field-Access]`, `rule.agg.index`). `ancestors(a)` is `a` together with
its base, its base's base and so on, which is what `clash` excludes (§4)
(`spec/04` §2). A binding's root path is always `exclusive`: there is no
mutability qualifier on a binding, and restricting access is what a borrow
does (`[Binding-Form]`).

**`held-by` is derived.** `held-by(a)` is the set of live objects whose
stored contents contain the token `a`, anywhere in a field, element or
payload: in every state it equals `{ o | alive(o, Σ) ∧ a ∈
refs-in(contents(o, Σ)) }` (`state.access-paths`, D-0215). It is a set of
holders, not of copies: a struct holding one reference in two fields still
holds it after one field is overwritten (`[Write]`, its `kept` set). The
rules that write storage and end objects keep the field equal to that set —
`[Write]` adds holders for the references the new value contains and removes
them for the ones no longer present anywhere in the object, and
`[Object-End]` removes the ended object from every path it held
(`rule.value-object.write`, `rule.value-object.object-end`). Holders matter
because a reference lives as long as something holds it: a path stays valid
while its object lives and some holder of it lives or the statement that
formed it is still open (`inv.temporal-validity`, Preservation).

**Bindings, holders, frames and statement scopes.** `state.bindings` maps
a name in a frame to the root path that owns the bound object
(`state.bindings`, `term.binding`). `state.holder` maps each top-level
object that a binding, a reclaim path or a lock guard owns to that owning
root path; an object with no holder and fresh storage is a *temporary*
(`state.holder`; `temporary(o, Σ)`, `spec/04` §2; `term.temporary-object`).
`owned-by-frame(o, f, Σ)` reads the owning path's `frame`, so a shadowed
binding's object is still ended with its frame (`spec/04` §2).
`state.frame-stack` holds each thread's open blocks, and
`state.temp-scope-stack` its open statement scopes, each scope recording
the frame current when it was pushed so that statement exits and block
exits can be interleaved in order during an unwind (`state.frame-stack`,
`state.temp-scope-stack`, `rule.control.unwind`).

**Resources.** `state.destruction-obligations` is the set of resources
that must be destroyed exactly once before their storage ends; a resource
is identified by an object, or by an object and a path for a
resource-bearing part of a composite (`ResourceId`, `spec/04` Purpose and
`spec/07` §4; `state.destruction-obligations`). `state.authority` records
that a thread may destroy a resource once: an entry keyed by thread,
operation and resource, with a `consumed` flag; the only operation any rule
uses is `destroy` (`state.authority`, `term.authority`). How these move and
are consumed is §6.

**Threads, locks and trust.** `state.threads` records, for each thread,
its expression in progress, the handle object `spawn` made for it, and its
final result: `None` while it runs, a result once its body has reduced,
`failed(d)` once a fault was contained to it, or `taken` once `join` has
claimed the result (`state.threads`, D-0204, D-0215). `running(ℓ, Σ)` holds
while a thread has not finished its body (`spec/04` §2). `state.sync`
gives each mutex `unlocked`, `locked-by(ℓ)` or `poisoned(ℓ)`
(`state.sync`). `state.trust` marks values that came from outside the
program, which is how `inv.trust-transition` is stated (`state.trust`). Two
earlier components, one for regions and one for suspended paths, were
removed and their ids retired (`spec/04` §1, "Removed components"); the
second matters for §4, since D-0018 removed suspension of a borrowed path
altogether.

## 3. Forming and ending paths

**Five ways a path comes to exist.** Every path in the state was formed
by one of five rules (`spec/08` §3): `[Binding-Form]`, the root path of a
new binding; `[Borrow]`, a path derived from another with a mode no wider
than its base's; the projections `[Field-Access]` and `rule.agg.index`,
which narrow a path to a part; `[Reclaim]`, a root over raw storage under
`unsafe`; and `[Lock]`, the path into a mutex's interior. Other rules reuse
these: `[Thread-Lend]` is a sequence of `[Borrow]` steps, `[Ref-Form]` is
`&` and `&mut` over `[Borrow]`, and `[Destroy-Composite]` forms a fresh path
into each sub-resource it destroys (`spec/19` §1; `rule.ref.form`;
`rule.resauth.destroy-composite`). Every path-forming rule sets every field
of the record (`state.access-paths`).

**Bindings.** `[Binding-Form]` gives an alive object a new exclusive root
path, records it under the name in the current frame, and makes it the
object's holder (`rule.value-object.binding-form`). It is invoked only by
`store` (a declaration's initializer, a parameter), by `[Let-Uninit]`, and
by a transfer's destination, each of which guarantees the object is fresh, a
temporary, or about to lose its previous owner
(`rule.value-object.binding-form`). A name may be declared once in its
declaration region (`[Binding-Form-Redeclared]`, D-0131); an inner block may
shadow it.

**Borrows.** `[Borrow]` derives a new path from `a0` in mode `m`. Its
premises are that `a0` is valid, that an exclusive borrow comes only from
an exclusive path (`[Borrow-Exceeds-Source]`, static), and that the new
path would not clash with any other (`[Borrow-Denied]`); the new record has
`a0` as its base, the current thread, frame and statement scope, and no
holders yet (`rule.alias.borrow`). The formation-time clash check is an
early diagnostic: it reports `auto r2 = &mut x` while `auto r1 = &x` lives
at the borrow rather than at the first use, but soundness rests on the
use-time checks in `[Read]` and `[Write]` (`spec/08` §2–§3). Nothing is
suspended: `a0` and its ancestors stay valid, and what they may do while the
borrow lives is decided by `clash` at each of their uses (§4; D-0018).
`&e` and `&mut e` are `[Ref-Form]` over `[Borrow]`; `e` must be a place,
and a place rooted in a temporary may be borrowed only as a call's argument,
since the temporary ends at its statement's end (`[Ref-Form-Not-Place]`,
`[Ref-Form-Temporary]`, `[Ref-Form-Temporary-Argument]`,
`[Ref-Form-Temporary-Part-Argument]`).

**Reborrows and passing references on.** `*e` for a reference `e` is a
place whose path is the token `e` holds (`[Ref-Deref-Place]`); as the
operand of `&` or `&mut` it is borrowed again, so `&*r` is a path whose base
is the path `r` holds — a reborrow (`rule.ref.deref`). Passing an existing
reference to a function forms no new path: the parameter object holds the
same token, and the reference stays valid for whoever else holds it when
the parameter frame ends (`rule.fn.bind-param`, the `ref<τ,m>` row). The one
exception is an exclusive reference passed where a shared one is declared:
it is first reborrowed shared, through `[Borrow]` with its checks
(`rule.fn.bind-param`, D-0049).

**Projections exist for one access.** `[Field-Access]` narrows `a0` to the
field's cells, inheriting its mode, with `a0` as its base; it performs no
conflict check when formed, and it exists only for the one access made
through it — the `[Read]`, `[Write]` or `[Borrow]` that uses it, or the next
projection — which also ends it (`rule.agg.field-access`, D-0219;
`conf.projection-one-access`). So in `p.a = p.a + p.b` the exclusive
projection that is the target and the projections that are read never meet.
A field of a reference is reached through it (`[Field-Access-Auto-Deref]`),
and a field of a guard through its lock path (`[Guard-Auto-Deref]`). A slice
borrows a *range* of an array or `Vec`, so two slices conflict only where
their ranges meet (`[Slice-Form]`, D-0070). A slice of a `Vec` is a path of
the vector's own object whose last step is `range(i, j)`, and `overlap`
compares such steps (`meet`): a range meets an intersecting range, an index
inside it, the vector's own fields and any path to the whole vector
(`spec/04` §2, `overlap`, `meet`; D-0238). `v[k]` on a `Vec` gives the place
`*Vec::index_exclusive(&mut v, k)` gives where it is written and
`*Vec::index_shared(&v, k)` elsewhere, but it is not a call: its one check is
an element borrow whose last step is `index(k)`, so it lives beside a slice
of other elements (`[Index-Vec]`, D-0238).

**Arguments are pending until bound.** A path an argument forms is
*pending* from its formation until `store` binds it to its parameter, and
`clash` does not count a pending path; so `put(&mut x, read(&x) + 1)`
evaluates its second argument freely (`rule.fn.call`, "Argument borrows",
D-0107; `clash`, `spec/04` §2). Once bound, a parameter holds its path like
any other, and a conflict between two parameters is found where one of them
is used (`rule.fn.call`, D-0022). A path held by a *value* an argument
computes, such as the view `String::as_view(&s)` returns, was formed and
stored by that call and is not pending (`rule.fn.call`, D-0179). This
program is accepted, and faults at the first write: the second `&mut x` is
formed while the first is still pending, so `[Borrow]` passes, and once both
are bound the two parameters hold two exclusive paths borrowed from `x`'s
root path, neither an ancestor of the other:

```
import std;

fn two(ref<i32, exclusive> a, ref<i32, exclusive> b)
{
    *a = 1;
    *b = 2;
}

fn main()
{
    i32 x = 0;
    two(&mut x, &mut x);
    printf("%v\n", x);
}
```

The outcome is `diag.aliasing-conflict` at `*a = 1`, dynamic: inside
`two`, the flow analysis cannot see what the caller passed (`[Write-Conflict]`;
`rule.control.flow-analysis`, the row for a place rooted at `*q`).

**How a path ends.** A path stops being valid in exactly the ways
`inv.temporal-validity` lists under Invalidation, and `spec/10` §1 gathers
them: its object ends (`[Object-End]` invalidates every path *of* the
object); the last object holding it ends or overwrites it (`[Object-End]`'s
`Dead` set, `[Write]`'s `gone` set); the statement that formed it ends while
nothing holds it and it is not the statement's result (`[Stmt-Exit]`'s
`Unheld` set); its object is moved (`[Authority-Transfer]`,
`[Relocate-In]`, the raw moves); its guard is dropped (`[Guard-Drop]`); it
was a destructor's borrow (`[Run-Destructor]`); or it was a projection
whose access has been made (`[Field-Access]`). A derived path's validity is
its own once formed: it does not end because its base is used, only by one
of these events (`inv.temporal-validity`, Transformation).

**Statement exit.** At the end of every statement, `[Stmt-Exit]` puts the
result in result form, invalidates every path formed during the statement
that nothing holds and that is not the result, ends or destroys every
temporary made during it that is not the result, and re-stamps the result's
own temporary or unheld reference into the enclosing scope
(`rule.control.stmt`). That is what makes `Vec::push(&mut v, 1);
Vec::push(&mut v, 2);` well-formed — the first borrow ends at its `;` — and
what keeps `auto r = &v;` alive: the borrow is held by `r`'s object
(`spec/14` §1a).

**Block exit.** `[Block-Exit]` ends every object the block's frame owns,
last bound first, in reverse order of the bindings that hold them (an
object moved into a later binding ends with it, D-0233): a resource by
`destroy`, any other object by `end-object` (`rule.control.block`). The block's own result
is exempt, and so are the references it holds: an object ended here does
not invalidate a reference the result carries, so a function may return
the reference a parameter held (`[Block-Exit]`, `[Object-End]`, D-0219;
`conf.returned-reference-parameter`). Ending the owned objects is what
invalidates the frame's bindings and every reference they held
(`rule.value-object.object-end`).

**Last use.** A local binding of a type that holds a reference (and no
resource and no `fn` value) ends before its block does: after its last
mention, but deferred while it shares a statement with another binding that
may hold a reference and that is mentioned again, and only once the flow
facts root no other binding's reference in it (`[Ref-Binding-Last-Use]`,
D-0111, D-0221). The end is deferred, never early (`spec/14` §1, prose
after the rule). In the first program below, `r`'s last mention is the
assignment, so `&x` meets no live exclusive path; in the second, `s` shares
its declaring statement with `r`, which is mentioned again, so `s` still
lives at `*r = 5` and the write is refuted statically, and the third ends
`s` with its block instead:

```
i32 x = 1;
auto r = &mut x;
*r = *r + 1;
auto s = &x;               // accepted: r has ended
printf("%v\n", *s);
```

```
i32 x = 1;
auto r = &mut x;
auto s = &*r;
printf("%v\n", *s);
*r = 5;                    // diag.aliasing-conflict (static): s still lives
```

```
i32 x = 1;
auto r = &mut x;
{
    auto s = &*r;
    printf("%v\n", *s);
}
*r = 5;                    // accepted: s ended with its block
```

These fragments are bodies of `fn main()` in a program that begins with
`import std;` (the convention of `spec/conformance.md`); the second is
rejected because the flow facts hold `deriv(s, *r, shared)` at the write
(`rule.control.flow-analysis`, the row for a place rooted at `*q`).

## 4. `clash`, and why ancestors are excluded

**The predicate.** Every access and every borrow is judged by one
predicate, as `spec/04` §2 defines it:

    clash(a, m, Σ)  ≝  ∃ a'. temporally-valid(a',Σ) ∧ ¬pending(a',Σ) ∧ of(a',Σ) = of(a,Σ)
                          ∧ a' ∉ ancestors(a,Σ) ∧ overlap(a,a',Σ)
                          ∧ ¬permitted(m, mode(a',Σ)) ∧ ¬sync-exempt(a, m, a', Σ)

An access of mode `m` through `a` is denied when some *other* valid path to
the same object, not one of `a`'s ancestors, reaches overlapping cells with
a mode not jointly permitted with `m`. `permitted` holds only for two shared
modes (D-0004), so a shared access meets only exclusive competitors and an
exclusive one meets every competitor. A path's mode is fixed when it is
formed; `shared` permits reading and `exclusive` permits reading, writing
and, with authority, destroying (`spec/08` §1). The path a diagnostic names
is a `witness`: any `a'` satisfying the body (`spec/04` §2;
`diag.aliasing-conflict`).

**Where it is evaluated.** `inv.alias-validity` is the proposition that
every access of mode `m` through `a` satisfies `¬clash(a, m, Σ)` at the
moment of the access, and, for an exclusive `a`, that `a` is the accessing
thread's (`inv.alias-validity`; `spec/08` §1). Its guarantee rests on the
use-time checks: `[Read]` requires `¬clash(a, shared)` (`[Read-Conflict]`
otherwise), `[Write]` requires `¬clash(a, exclusive)` (`[Write-Conflict]`),
and every access — a dereference, a `match`, a parameter binding, every
intrinsic — reduces to these (`spec/08` §3). The std-only intrinsics that
write a place they are handed, `[Swap-Places]` beneath `swap`, `replace` and
`overwrite`, and `[Lend-Places]`, take `[Write]`'s premises for it; before
D-0235 they did not, and `swap` through a reference passed on whole changed
what a live shared reborrow of it reached (`rule.stdlib.swap`, D-0235). `[Borrow]` checks once more at
formation, and the projections not at all; those choices decide how early a
conflict is reported, not whether it is (`spec/08` §3). The earlier argument
that aliasing held by construction was found not to hold, and this one
replaced it (`spec/08` §3; D-0018).

**Why ancestors are excluded.** The exclusion is one-directional, and that
is the whole design of borrowing without suspension (D-0018). When an access
goes through a derived path `a`, `a`'s own ancestors are not competitors:
the child reaches its parent's cells by construction, and a parent cannot
be what forbids an access through its own child — otherwise no access
through a borrow of an exclusive path could ever pass. When the access goes
through the parent, the child is *not* among the parent's ancestors, so a
live exclusive child clashes with every use of its parent and a live shared
child clashes with the parent's writes. That is `[Borrow]`'s statement: "a
parent may read while a shared child lives, and may do nothing that
conflicts with an exclusive child until that child is invalid"
(`rule.alias.borrow`, prose). The same exclusion is what lets a lock path
coexist with the locking reference and the mutex's binding: `[Lock]` says
that the locking reference and its ancestors (the mutex's owning binding)
are ancestors of the lock path, "not competitors" (`[Lock]`, its comment on
the lock path's `base`).

**A path never clashes with itself.** `a ∈ ancestors(a)` (`spec/04` §2), so
`clash(a, ·)` never counts `a`. Since every copy of a reference value is the
same token (`type.ref`), two copies of one exclusive reference are, to
`clash`, one path. Within one thread the copies are used one after the
other, which is harmless; in two threads they would be two unsynchronized
writers no `clash` could see. `[Thread-Affine]` closes that gap by making an
exclusive path belong to one thread (`rule.alias.thread-affine`, D-0213;
§7 below).

**Overlap is by cells.** `overlap(a1, a2)` holds when the two targets share
a cell; for a zero-sized place, whose target is empty, it holds when the
two paths are on the same object and one's projection steps are a prefix of
the other's (`spec/04` §2, D-0217). So two exclusive borrows of *different*
fields of one struct coexist (`spec/08` §4), and this program is accepted:

```
import std;

struct Pair
{
    i32 a;
    i32 b;
}

fn main()
{
    Pair p = Pair { .a = 1, .b = 2 };
    auto ra = &mut p.a;
    auto rb = &mut p.b;
    *ra = *ra + 10;
    *rb = *rb + 20;
    printf("%v %v\n", p.a, p.b);
}
```

**The two other exclusions.** A *pending* path — a call argument's path not
yet bound to its parameter — is not counted (§3; D-0107). And `sync-exempt`
lets a lock-derived path and a shared path that is not lock-derived pass
each other: the shared path, to the mutex or to a struct around it, can
reach nothing inside the mutex, which only a lock reaches (`rule.conc.lock`,
`sync-exempt`, `CHG-0072`). Two lock-derived paths are checked as any two
paths are (`rule.conc.lock`, prose).

**What a shared reference reaches is read-only.** Because an exclusive
borrow comes only from an exclusive path (`[Borrow-Exceeds-Source]`), and an
exclusive reference *stored* in what a shared path reaches is read out as a
shared borrow of the stored path (`[Read-Ref-Through-Shared]`), a
`ref<τ, shared>` value is never a source of mutation at any depth
(`rule.alias.borrow`; `rule.alias.read-through-shared`, D-0218). A copy that
would carry an exclusive reference out through a shared path is rejected
(`[Read-Exclusive-Through-Shared]`), and a `fn` value read or called through
a shared path faults when a closure in it has an exclusive capture, or is a
`move` closure whose body writes a capture, which changes the closure's own
storage (`[Read-Fn-Through-Shared]`, dynamic, because a `fn` type does not
show its captures; D-0239). Such a closure is borrowed exclusively when called
(`[Closure-Call]`, D-0239); any other is borrowed shared, and inside the call
a borrow capture is its own stored path, not a shared borrow of it
(`rule.fn.closure`, D-0221, D-0239). The routes for mutation through a shared handle are a mutex,
whose lock path is exclusive whatever reference reached it, and an `Atomic`, whose operations
form no path (`spec/19` §2a)
(`rule.alias.read-through-shared`, prose). So `poke` below is rejected with
`diag.write-through-shared` (static, `[Write-Not-Exclusive]`), while `peek`
is accepted:

```
struct Hold
{
    ref<i32, exclusive> r;
}

fn peek(ref<Hold, shared> h) : i32
{
    *h.r + 1
}

fn poke(ref<Hold, shared> h)
{
    *h.r = 1;
}
```

## 5. The static/dynamic boundary

**Three ways to discharge a side-condition.** A rule's side-condition is
tagged `discharge: static` (proved before the rule applies; a program that
violates it is rejected), `dynamic` (checked as part of the step; failure is
a `↛` diagnostic), or `trusted` (assumed, and allowed only on a
`trusted-unchecked` rule inside `unsafe`) (`spec/01` §4). Most of the
safety-relevant side-conditions — a path's validity, `¬clash`, `solitary`,
authority, initialization — are tagged "`discharge: static
(rule.control.flow-analysis; dynamic where unknown)`": decided before the
run where one fixed analysis decides them, checked at run time otherwise
(`[Read]`, `[Write]`, `[Borrow]`, `[Destroy]`, `[Authority-Transfer]`).

**One analysis defines the accepted set.** `rule.control.flow-analysis`
(`spec/14` §6) discharges every side-condition written that way, so that the
set of programs rejected before they run is the same for every conforming
implementation. Its facts, lattice, control-flow graph and transfer
functions fix the accepted set; an implementation may run further analyses
to prove a check cannot fail and leave it out, but none may accept a
program this rule rejects or reject one it accepts (D-0206). Where two
static diagnostics apply, the first in source order is reported (D-0110).
The reference interpreter's `--check` is the executable form of the rule
(`rule.control.flow-analysis`; `spec/IMPLEMENTATION-NOTES.md` §4). A
conforming implementation may report additional warnings from stronger
analyses but may not change acceptance (`rule.control.flow-analysis`,
Scope).

**Facts and lattice.** For each function body the analysis tracks, per
binding `x`, `valid(x)` (its path is valid: not moved from, its object not
ended) and `init(x)` (its object is initialized); per reference-typed
binding `r`, `deriv(r, x.π, m)` (`r` holds a reference derived from `x`'s
object at the syntactic projection path `π` with mode `m`); and per binding,
`escaped(x)` (a reference to `x`'s object was stored where the analysis
does not follow it) (`rule.control.flow-analysis`, Facts). Each fact is `T`,
`F` or `?`, `?` being the join of the other two; the analysis is a forward
dataflow over the syntactic control-flow graph of `spec/14` §5, joined at
merge points and iterated to the least fixed point (Lattice). Projection
paths overlap when one is a prefix of the other, a literal index meeting
only itself and a non-literal one meeting every index; a slice's step is its
range (Facts, D-0070).

**Transfer functions.** The table in `spec/14` §6 says how each construct
changes the facts. The rows a reader meets first: a declaration sets
`valid(x)` and, with an initializer, `init(x)`; `auto r = &_m x.π` sets
`deriv(r, x.π, m)`, as does a call with exactly one reference parameter that
returns a reference (the D-0011 elided call); a resource-typed binding
consumed — as an argument, an initializer, a `return`, a `move` capture —
sets `valid(x) := F` and clears every `deriv` on `x`, as `drop(x)` does; a
borrow of `x` in any other position sets `escaped(x)`; block exit, and a
binding ended by `[Ref-Binding-Last-Use]`, clear the bindings' facts, and
clearing a binding's `deriv(r, x.π, m)` this way, or by reassigning or moving
`r`, *forgets* it: `escaped(x)` is set, because `r`'s value may have been
copied where the analysis does not look — a struct field, a `Vec`, a thread's
parameter — and the reference stays valid there after `r` ends
(Forgetting, D-0236); and
`spawn(…, r …)` with `r` a tracked reference is a borrow of `*r` in the
parameter's mode, held by the handle until `join` (Transfer functions,
D-0071, D-0213, D-0219, D-0221). Rows for closures without `move`, `match`
by reference, and results that hold a reference without being one complete
the table (D-0071, D-0088, `CHG-0100`).

**Discharge.** At each guarded side-condition the state there gives one of
three answers (Discharge). *Proven*: the condition holds statically and no
run-time check is made. *Refuted*: the program is ill-formed, and the rule's
own diagnostic is reported as static. *Unknown* — `?`, or a shape the table
does not cover — means `discharge: dynamic`: the run-time check is made and
its diagnostic applies if it fails. For `¬clash` at a place `x.π`, proven
means `escaped(x) = F` and no overlapping, non-permitted `deriv` fact is `T`;
refuted means some such `deriv` fact is `T`. A place rooted at `*q` for a
tracked reference `q` is never proven, because the caller may hold other
paths to the referent (D-0022), but is refuted by an overlapping `deriv` on
`*q`; a place rooted at a temporary is never decided either way. Value-range
conditions (`v2 ≠ 0`, `0 ≤ i < N`, range of a result) are decided only for
literals and literal-bound locals (Discharge, CHG-0092).

**Where unknown rejects.** Two side-conditions reject on `unknown` instead
of falling back to a check: definite assignment, because whether a binding
is initialized depends only on the function's own control flow, so `unknown`
means some path writes nothing (`rule.init.definite-assignment`,
`[Definite-Assignment-Rejected]`); and the use of a binding that may have
been moved or dropped, `valid(x) = ?` (D-0087). Both are listed as the
exceptions in `rule.control.flow-analysis`'s outcome paragraph.

**What the analysis does not see.** It is intraprocedural and syntactic: a
call is opaque except for its declared signature and the D-0011 elision,
and there is no value-range reasoning beyond literals (Scope). Two
consequences are recorded as trades, not gaps. Any borrow of `x` in a
position other than the two `deriv` rows sets `escaped(x)`, so after the
first `Vec::push(&mut nums, …)` no later access to `nums` can be proven; it is
still not refuted, so the program is accepted and checked at run time
(`rule.control.flow-analysis`, "A confirmed trade";
`conf.sequential-exclusive-borrows-ok`). And because an elided call sets
`deriv` as a visible borrow would, a reference taken by such a call is
treated as a borrow of its argument, so a later exclusive borrow of that
argument in the same function is refuted (`rule.temporal.elision`, "A
confirmed trade"; `conf.vec-ref-then-push-rejected`):

```
import std;

fn first(ref<Vec<i32>, shared> v) : ref<i32, shared>
{
    &(*v)[0]
}

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 7);
    auto r = first(&v);
    Vec::push(&mut v, 8);      // diag.aliasing-conflict (static)
    printf("%v\n", *r);
}
```

**`Vec` elements are not places of the analysis.** A borrow of an element,
`&_m v[k]`, or a reference from `Vec::index_shared` or `index_exclusive`,
sets a fact on the whole of `v`; an element access is not compared with
other element accesses statically, and `[Index-Vec]`'s run-time check
decides it, element by element (`rule.control.flow-analysis`, Facts, D-0221;
`[Index-Vec]`).

**Two separate static rules.** `rule.temporal.ref-escape` is a lexical rule,
not an instance of the analysis: a borrow whose referent's block is known
may not escape — be stored, directly or in an aggregate, into a binding of a
strictly enclosing block — nor be the function's result unless the
referent's root binding is a reference-typed parameter
(`[Ref-Escape-Rejected]`, `diag.reference-escapes-scope`). Where the
referent is not syntactically visible, the dynamic baseline of `spec/10` §1
enforces validity at use (`rule.temporal.ref-escape`, prose).
`rule.temporal.elision` lets a function with exactly one reference parameter
return a reference tied to it; a function with none or several may not
return one (`[Call-Elided-Lifetime]`, `[Call-Multi-Ref-Return-Rejected]`),
and explicit lifetime parameters were rejected as a permanent boundary
(`feat.explicit-lifetime-parameters`, `CHG-0022`).

**Reading a conformance outcome.** A row's `(static)` means the rule's
guard was refuted, or the rule is `rejected`; `(dynamic)` means the guard
was unknown and the run-time check failed (`spec/conformance.md`,
Conventions; `spec/registry/diagnostics.md`, Phase). The two programs below
show both phases of the same diagnostic. In the first, `deriv(r, x, shared)`
is `T` at the borrow `&mut x`, which `[Borrow-Denied]`'s guard therefore
refutes: `diag.aliasing-conflict (static)`. The second is the `two` example
of §3: inside `two` the places are rooted at `*a` and `*b`, never proven and,
with no `deriv` facts, not refuted, so `[Write-Conflict]` is checked and
faults: `diag.aliasing-conflict (dynamic)`.

```
i32 x = 1;
auto r = &x;
auto w = &mut x;           // refuted: r still holds a shared borrow of x
*w = 2;
printf("%v\n", *r);
```

**What a dynamic check does when it fails.** The step does not happen: the
rule's `↛` judgment applies instead (`spec/01` §2.4), and the fault unwinds
the thread, running destructors, and ends the program — or, in a spawned
thread that was lent no exclusive reference, ends only that thread
(`[Fault-Unwind]`, `[Fault-Contain]`, `rule.fail.fault-unwind`, D-0009,
D-0204). There is no catching (`spec/01` §2.5a; `spec/18` §1).

## 6. Moves, destruction and `solitary`

**Which types are resources.** `is-resource` is false for scalars, `str`,
references, raw pointers and `fn` values; an array is a resource iff its
element is; a struct or enum is one iff it is declared `resource` or holds a
resource field or payload; `handle`, `mutex` and `guard` always are
(`rule.type.is-resource`). A resource carries a destruction obligation and
single-holder destroy authority (`term.resource`, D-0003). A resource is
never copied: `[Read]` of a resource-typed place is rejected
(`[Read-Resource-Rejected]`, `diag.read-of-resource`), since a copy would
duplicate the authority `state.authority` tracks (`rule.value-object.read`,
prose). A plain value is copied wherever it is stored.

**`store` decides between copy and move.** Every construct that puts a
result somewhere — a declaration, a parameter, a field initializer, a
closure capture, `return`, a block's or statement's result — reduces to
`store` (`rule.value-object.store`, D-0019). A value is written; a place of
plain type is read and copied (`[Store-Binding-Place-Copy]`); a resource
place is *transferred* into a binding (`[Store-Binding-Place-Transfer]`) or
*relocated* into a part of an aggregate (`[Store-Sub-Relocate]`); a
temporary is adopted by a binding (`[Store-Binding-Temp]`); and in result
position a resource binding is released into a temporary that the receiving
construct will store (`[Store-Result-Place-Release]`). Moving a resource out
of a field of a live object is rejected (`[Store-Binding-Place-Transfer-Sub]`,
`diag.move-out-of-field`); the whole container is moved, or the field is
reached through a reference (`rule.value-object.store`, prose), and a field
of a temporary is taken by taking the temporary apart (`[Temp-Field-Move]`,
D-0103).

**Transfer.** `[Authority-Transfer]` moves ownership of a whole object from
one root path to another: the destination becomes the object's holder, the
source path is invalidated — which is what makes it a move — and, when the
destination was formed in another thread, the destroy authority is re-keyed
to that thread (`rule.resauth.transfer`). The object's identity, origin and
cells are untouched (`rule.resauth.transfer`, prose). Its premises are that
the source is valid and has authority (`[Authority-Transfer-Unauthorized]`
otherwise) and that no path other than the source and the destination is
valid on the object (`[Authority-Transfer-Aliased]`,
`diag.move-while-aliased`): those paths would otherwise outlive the binding
they were derived from with nothing to invalidate them, because the object
lives on (`rule.resauth.transfer`, prose; D-0018). After a move, the flow
analysis has `valid(x) = F`, so a later use is refuted
(`rule.control.flow-analysis`, the consumption row;
`[Binding-Lookup-Stale]`); an assignment of a whole new value re-establishes
the binding (`spec/11` `[Assign-Reestablish]`, D-0033).

```
import std;

resource struct Ticket
{
    i32 id;
}

fn consume(Ticket t)
{
}

fn main()
{
    Ticket t = Ticket { .id = 1 };
    consume(t);
    printf("%v\n", t.id);      // diag.stale-binding (static): t was moved
}
```

**`solitary`.** `solitary(a, Σ)` says that no valid path other than `a` is
on `a`'s object at all — not merely no overlapping or conflicting one
(`spec/04` §2). It is stronger than `¬clash`: an exclusive borrow of one
field of a struct does not clash with a write to another field, but it does
make the struct not solitary. `[Destroy]` and `[Authority-Transfer]` require
it (the latter modulo its destination), and `[Relocate-In]` and
`[Rawptr-Move-In]` require that every valid path on the moved object is its
holder (`spec/08` §4; `rule.resauth.relocate-in`; `rule.trust.rawptr`). The
flow analysis proves it when `escaped(x) = F` and no `deriv` on `x` is `T`,
and refutes it when some `deriv` on `x` is `T`
(`rule.control.flow-analysis`, Discharge).

**Destruction.** `[Destroy]` of a whole resource consumes the thread's
authority, removes the obligation, runs the type's destructor through a
fresh exclusive borrow (so the destructor can change the object but not
move it out), destroys every resource-bearing part innermost first, and
ends the object (`rule.resauth.destroy`, `[Run-Destructor]`,
`[Destroy-Composite]`, `[Object-End]`). The destructor's borrow succeeds
because the path is solitary, and is invalidated afterwards so that the
parts' destruction and the end find it solitary still (`rule.resauth.destroy`,
prose). Destroying while a reference lives is `diag.destroy-while-aliased`
(`[Destroy-Not-Solitary]`); a second `drop` through the same binding is
caught earlier as `diag.stale-binding` (`[Destroy-Stale]`), since the first
ended the object and so invalidated the binding's path (`rule.resauth.destroy`,
prose). `drop(x)` is the intrinsic form; the block and statement exits are
the automatic ones (`rule.resauth.destroy`, prose). In this program the
`drop` is refuted, since `r` is used afterwards:

```
Ticket a = Ticket { .id = 1 };
auto r = &a;
drop(a);                   // diag.destroy-while-aliased (static)
printf("%v\n", r.id);
```

**Destructors.** A type has a destructor iff it declares
`fn τ::drop(ref<τ, exclusive> self)` with no return type; any other
signature, a destructor on a plain type, and a direct call of one are
rejected (`spec/07` §1; `diag.bad-destructor-signature`,
`diag.destructor-on-plain-type`, D-0075; `diag.direct-destructor-call`). A
destructor is run only by `[Run-Destructor]`, so each resource's own cleanup
runs exactly once, when its authority is consumed (`spec/07` §1). The
built-in resources have built-in destructors: a handle's joins its thread
(`[Handle-Destructor]`), a mutex's destroys its interior, a guard's unlocks
(`[Guard-Drop]`) (`spec/07` §1; `spec/19`).

**Leak freedom is structural.** Every resource object is either owned —
destroyed when its owning frame exits (`[Block-Exit]`) — or a temporary,
destroyed at the end of the statement that made it unless it is that
statement's result, in which case the receiving construct owns it
(`[Stmt-Exit]`); every part is destroyed with its container
(`[Destroy-Composite]`). No obligation can outlive its scope, and there is
no leak diagnostic, because the state it would report is unreachable
(`spec/07` §5, D-0008). Raw storage is the exception, under trust (§8):
`inv.resource-authority` gives a reclaimed object's obligation to the
`unsafe` author who reclaimed it (`inv.resource-authority`, Proposition).

**Why block exit finds each object solitary.** When `[Block-Exit]` destroys
the frame's objects last bound first — by when each one's holder was formed,
not by the object's origin, which differs after a move (D-0233) — any other path to one of
them is a temporary path already ended at its statement's exit, a reference
held by an object of the same frame established later and so already ended
in the same fold, or a reference held by an outer object — which
`rule.temporal.ref-escape` rejects statically and `[Destroy-Not-Solitary]`
catches at run time (`rule.control.block`, prose).

**Overwriting a resource.** A write may not replace a value that still owns
a resource whose obligation is outstanding: that would strand the obligation
(`[Write]`'s `¬live-resource-at` premise; `[Write-Resource-Overwrite-Rejected]`,
`diag.overwrite-of-live-resource`). What counts is the value there now: a
`None` of an `Option<Box<T>>` owns nothing and may be overwritten (`owns`,
D-0049). A *quiet* type — one whose destruction only frees memory, such as
plain data, `String`, `Vec` of quiet elements and `Box` of a quiet type — is
destroyed by the write itself, after the write's other checks pass
(`[Write-Quiet-Replace]`, `quiet`, D-0194).

**Relocation.** Storing a resource into a part of an aggregate relocates it:
the cells are copied into the part, the obligations and authority are
re-keyed under the container's identity and path, the references it held are
now held by the container, and the source object ends (`[Relocate-In]`). The
inverse, used by `match` for a resource payload, promotes a part to a fresh
temporary (`[Relocate-Out]`). Neither rewrites an origin: relocation ends one
identity and continues the value under another (`inv.origin-stability`,
Transformation).

## 7. Threads

**Spawning.** `spawn(f, e1, …, en)` evaluates its arguments, lends the
exclusive references among them, makes a fresh thread with a fresh frame,
binds the arguments as parameters in that thread exactly as a call would —
resources move, with their authority re-keyed to the new thread — and
returns a handle, a temporary resource the caller stores (`[Spawn]`,
`rule.conc.spawn`). A closure passed to `spawn` must be a `move` closure: a
borrowing closure would hold references into the spawning frame
(`diag.spawn-borrow-closure`, static; `rule.conc.spawn`, prose). The
arguments are bound by the `[Spawn]` step itself, so a reference passed is
held by the new thread from the spawn until the thread finishes, whether or
not it has taken a step (`rule.conc.spawn`, prose).

**Interleaving.** Any live thread may take the next step, and each step is
atomic with respect to `Σ`: executions are sequentially consistent
interleavings of the threads' own steps (`[Thread-Step]`, `outcome:
unspecified`; `[Conc-Interleaving]`). Atomics are sequentially consistent
(`rule.conc.atomic`, D-0249); a weak memory model is not provided (`spec/22` §5). Cross-thread aliasing
uses the same `clash`: it is evaluated at every access in every thread, so
a conflicting access is diagnosed at whichever access comes second,
regardless of timing (`spec/19`, Purpose; `inv.concurrency-validity`).

**`[Thread-Affine]`: an exclusive path belongs to one thread.** Every rule
that accesses through a path or borrows from one has a further premise:
when the path is exclusive, it must be the operating thread's, or in
transit, or of a thread that has ended; otherwise the step is
`diag.aliasing-conflict` (`rule.alias.thread-affine`, `affine(a, ℓ, Σ)`,
D-0213). A step that passes with another thread's path takes the path over:
a path lent at a thread boundary is taken by the first thread to access
through it, and an ended thread's path by the next thread that does
(`[Thread-Affine]`, comment). This is what `clash` alone could not give
(§4): copies of one exclusive reference in two threads are one path. An
exclusive reference that reaches another thread other than by a lend — in a
struct the thread is lent a reference to, in a `Vec` moved to it, in a mutex
it locks — remains the forming thread's, and an access through it faults
while that thread runs (`rule.alias.thread-affine`, prose;
`conf.exclusive-ref-of-another-thread`). Shared paths are not restricted.

**`[Thread-Lend]`: how a reference crosses.** The exclusive references in
`spawn`'s arguments (however deep in a value, or in a moved argument's
contents) and among a `move` closure's captures, and those in the value
`Channel::send` carries, are each replaced by a path borrowed exclusively
from them, marked `transit` until a thread accesses through it
(`[Thread-Lend]`, D-0213; `[Send]`). The borrow is an ordinary `[Borrow]`
with its checks, so the sender's own copy clashes with the lent path while
it lives, and lending the same reference twice is a second exclusive
borrow — `diag.aliasing-conflict` (`rule.conc.spawn`, prose;
`conf.exclusive-ref-lent-twice`). What an argument reaches only through
storage it owns, such as a `Vec`'s elements, is not lent (`rule.conc.spawn`,
prose). The flow analysis treats `spawn(f, r)` as a borrow of `*r` held by
the handle until `join` (`rule.control.flow-analysis`, the `spawn` rows), so
a use by the spawner before the join is refuted, and after it is accepted:

```
import std;

fn bump(ref<i32, exclusive> a)
{
    *a = *a + 1;
}

fn main()
{
    i32 x = 0;
    ref<i32, exclusive> r = &mut x;
    auto h = spawn(bump, r);
    join(h);
    *r = *r + 10;              // accepted; before the join it is refuted
    printf("%v\n", x);
}
```

(`conf.exclusive-ref-used-by-spawner` is the refuted form.)

**Handles.** `join(h)` destroys the handle, waits for the thread, and gives
its result to the joiner's statement, re-keying a resource result's
authority to the joiner first (`[Join]`). Destroying a handle any other way
— `drop` or a block's exit — also waits, and discards the result
(`[Handle-Destructor]`). So a thread can never outlive the frame that owns
its handle, which is what lets references passed to a thread be treated like
references passed to any call (`rule.conc.join`, prose). Waiting is the
absence of a step, not a failure; a wait that can never end is `[Deadlock]`
(`rule.conc.join`, prose). When `main` returns normally, its block exit has
already joined every thread (`[Terminate-Ok]`, comment).

**`foreign-held`: containers and other threads' elements.** An element of a
`Vec`, a slot of a map or queue, or a `Box`'s value is a reclaimed object
over the container's buffer, so an exclusive borrow of the container does
not clash with a path to the element: their cells are disjoint (`spec/20`
§2, "A container's element and another thread", D-0214). When another
running thread holds an exclusive derived path to such an element (or one
lent and not yet taken), it may be writing the element while the
container's operation reads, moves or releases it; so `[Release]` and
`[Rawptr-Move-Out]` check `foreign-held` and fault with
`diag.aliasing-conflict` first (`foreign-held(o, ℓ, Σ)`, `spec/20` §2).

**Mutexes.** `Mutex::new(e)` makes a `mutex<τ>` holding `e` as its interior
(`[Mutex-New]`, `type.mutex`). `lock` takes a *shared* reference to the
mutex, which any number of threads may hold, and blocks until the mutex is
unlocked; it then marks it `locked-by` the thread and returns a guard
holding a new exclusive path into the interior (`[Lock]`). Locking a mutex
the thread already holds is a fault, not a deadlock (`[Lock-Reentrant]`,
`diag.mutex-reentrant-lock`). `*g`, `g.f` and `g[i]` reach the interior
through the guard (`[Guard-Deref]`, `[Guard-Auto-Deref]`, D-0064), and
dropping the guard unlocks the mutex and invalidates the path
(`[Guard-Drop]`). Since at most one lock path is live at a time and the
interior is reachable through no other path, accesses through it never race
(`rule.conc.lock`, prose); `sync-exempt` keeps shared paths to the mutex
from conflicting with the lock path (§4). This program prints `3`:

```
import std;

fn add(ref<mutex<i32>, shared> m, i32 n)
{
    auto g = lock(m);
    *g = *g + n;
}

fn main()
{
    auto m = Mutex::new(0);
    {
        auto h1 = spawn(add, &m, 1);
        auto h2 = spawn(add, &m, 2);
        join(h1);
        join(h2);
    }
    auto g = lock(&m);
    printf("%v\n", *g);
}
```

**Channels.** `Channel<T>` is a `std` resource written in CobaltC over a
mutex: a bounded queue of at most `cap` values, shared between threads by a
shared reference, with a closed flag (`rule.conc.channel`, D-0063;
`rule.stdlib.channel`). `send` first lends the value's exclusive references
(`[Thread-Lend]`), then moves the value in, or gives it back as `Err` when
the channel is closed, or blocks while it is full (`[Send]`). `recv` moves
the oldest value out and re-stamps its references into the receiver's
statement scope, returns `None` when the channel is empty and closed, or
blocks while it is empty and open (`[Recv]`, D-0219). A value sent is
received at most once and in order relative to the channel's other values;
which waiting thread proceeds is unspecified (`rule.conc.channel`, prose).
The main thread blocked on a channel while no spawned thread runs is
`diag.channel-deadlock` (`[Channel-Deadlock]`); a channel of zero capacity
is a fault (`[Channel-Zero-Capacity]`).

**The guarantee.** `rule.conc.guarantee` gathers what a program without
`unsafe` can rely on when it runs threads; it adds no rule but states what
an implementation must not depart from (`spec/19` §4). Every execution is a
sequentially consistent interleaving (`[Conc-Interleaving]`). Two accesses
of different threads to overlapping cells of one object, at least one a
write, not ordered by a join, by one mutex's lock and unlock, or by the send
and receive of one value, cannot both happen: the later is
`diag.aliasing-conflict`, or a stale diagnostic if the earlier ended the
object — there is no data race (`[Conc-No-Race]`). Every step of every
interleaving is defined: an access that is not permitted is a diagnostic,
never undefined behaviour (`[Conc-Defined]`). An implementation may leave a
check out only where it cannot fail in any interleaving
(`[Conc-Checks-Omitted]`). The reasons the rule gives for `[Conc-No-Race]`
are that a path's standing is local to its thread (§9), that a reference
passed to `spawn` is held from the spawn, and `[Thread-Affine]` with
`[Thread-Lend]` (`rule.conc.guarantee`, prose).

**Deadlock is diagnosed, not excluded.** A thread whose wait (on a join, a
lock or a channel) closes a cycle of waits faults with `diag.deadlock`; and
when every live thread waits on another thread, a mutex or a channel and
none can go on, a member of the cycle, or each thread at the end of a chain
of waits, faults instead (`[Deadlock]`, D-0204, D-0210). The fault is
contained like any other (below), so the outcome need not depend on timing
even where which member faults does (`rule.conc.guarantee`, prose;
`conf.lock-join-cycle`). Not decided is a wait on a channel while another
thread is in a call outside the program — a socket, input, the clock — which
is never a deadlock; a program bounds such a wait with
`Channel::recv_timeout_ms`, or a supervisor cancels it (`rule.conc.guarantee`,
"What is not decided").

**Supervision.** A fault in a spawned thread is *contained* to it when the
thread was lent no exclusive reference and has locked no mutex whose
interior may hold one: the thread unwinds, every mutex it held is poisoned,
the threads it started are cancelled, and its result is `failed(d)`, while
the program goes on (`[Fault-Contain]`, `contained(ℓ, Σ)`, D-0204). A thread
lent exclusive access to another's data is not contained, and its fault ends
the program, because what it was changing is not its own
(`rule.fail.fault-unwind`, prose); nor is one that took over an exclusive
path by `[Thread-Affine]` — from a `Vec` moved to it, or a value a joined
thread returned — since it then holds the same access without having been
lent it (`contained(ℓ, Σ)`, D-0237). `join` of a failed thread raises its
failure again in the joiner (`[Join-Failed]`); `std::try_join` waits as
`join` does and returns the failure as a value, and `is_finished` asks
without waiting (`[Try-Join]`, `[Is-Finished]`, `rule.stdlib.supervision`).
A later `lock` of a poisoned mutex faults (`[Lock-Poisoned]`,
`conf.mutex-poisoned`). `cancel` asks a thread to stop; it does so at its
next wait — a join, a lock, a channel operation, `sleep` or a socket's wait
— with `diag.thread-cancelled`, and a thread lent an exclusive reference is
never cancelled (`[Cancel]`, `[Cancelled-Wait]`, `rule.conc.cancel`,
D-0210). A supervisor loop:

```
import std;

fn work(i32 n) : i32
{
    assert(n != 2);
    n * 10
}

fn main()
{
    Vec<handle<i32>> hs = Vec::new();
    foreach (n in 1..4: i32)
    {
        Vec::push(&mut hs, spawn(work, n));
    }
    while (Vec::len(&hs) > 0)
    {
        auto h = Option::unwrap(Vec::pop(&mut hs));
        match (try_join(h))
        {
            Ok(v) : printf("ok %v\n", v),
            Err(f) : printf("failed %s\n", &ThreadFailure::diagnostic(&f)),
        }
    }
}
```

The thread given `2` fails its assertion, a fault contained to it; the
others' results are printed, and the program ends normally
(`[Fault-Contain]`, `[Try-Join]`, `rule.fail.assert`).

## 8. The trust boundary

**`unsafe` blocks.** A construct whose rule has a `discharge: trusted`
side-condition may appear only lexically inside an `unsafe { }` block;
elsewhere the program is ill-formed (`[Unsafe-Rejected]`,
`diag.trusted-outside-unsafe`). The trusted rules are `[Rawptr-Read]`,
`[Rawptr-Write]`, `[Rawptr-Move-In]`, `[Rawptr-Move-Out]`, `[Rawptr-Offset]`,
`[Reclaim]`, `[Release]`, `[Extern-Call]`, `[Deallocate]` and `[Copy-Raw]`
(`rule.trust.unsafe`); the volatile forms `[Rawptr-Read-Volatile]` and
`[Rawptr-Write-Volatile]` are trusted in the same way and also need `unsafe`
(D-0121). An `unsafe` block is otherwise an ordinary block
(`rule.control.block`, prose). Its text is the author's claim, and a claim
is not checked (`rule.trust.unsafe`, prose). The one trusted claim made
outside a block is an `extern` declaration's signature, which carries its
own `unsafe` (`[Extern-Decl]`, below).

**When a claim is false.** An execution that reaches a trusted step whose
side-condition does not hold is constrained by the rules up to that step and
by none after it (`[Trusted-Claim-False]`; `spec/01` §5, `trusted-unchecked`;
D-0096). This is how undefined behaviour enters the specification, and the
only way: it is not an `outcome`, and no set of results is admitted
(`spec/01` §5). Detecting a false claim is optional; an implementation that
does is encouraged to stop with a diagnostic (`rule.trust.unsafe`,
informative note).

**Raw pointers.** `rawptr<τ>` holds an address and has no access path, no
mode and no checks; it is plain and freely copied (`type.rawptr`). Obtaining
one from a reference is safe (`[Rawptr-Of]`); offsetting, reading and
writing through one are trusted (`[Rawptr-Offset]`, `[Rawptr-Read]`,
`[Rawptr-Write]`). The trusted conditions of a raw read or write say that no
live *derived* path — a borrow or a projection — of another thread reaches
the cells in exclusive mode (for a read) or of any object (for a write), and
that no other thread accesses them meanwhile; an object's own root path is
not mentioned, so writing a local's cells through `rawptr_of(&mut x)` while
`x` is bound is the ordinary buffer pattern, and `x`'s later reads see the
cells as written (`rule.trust.rawptr`, "Root paths and raw access"). What the
author asserts is that no reference currently reaching those cells is
bypassed (same paragraph).

**Reclaimed objects.** `reclaim<τ>(p)` makes the cells at `p` an object of
`storage-kind = reclaimed`, initialized, with a fresh destroy authority for a
resource type, and returns a root path to it; a reclaimed object already
there is re-attached instead, its authority passing to the reclaiming thread
(`[Reclaim]`, CHG-0030). From then on every access to it goes through
ordinary paths with every ordinary check, so two `reclaim` calls on the same
cells give two root paths that `clash` sees (`rule.trust.rawptr`,
"Reclaimed objects"). A reclaimed object is not a temporary and persists
until `[Release]`, `[Rawptr-Move-Out]` or `destroy` ends it (same paragraph;
`temporary(o, Σ)`). `[Rawptr-Move-In]` moves a whole object into raw storage
as a reclaimed object, and `[Rawptr-Move-Out]` moves one out into a
temporary; `[Release]` ends every reclaimed object over a range, after
checking `foreign-held` (§7); `[Copy-Raw]` copies cells and keeps a holder
for any reference it copies (D-0216, D-0219); `[Allocate]` and
`[Deallocate]` obtain and return raw storage (`rule.stdlib.prelude`). These
are what `std`'s containers are built from.

**Foreign functions.** An `extern` function is declared
`unsafe extern fn name(…) : τ;`, the `unsafe` recording that its signature is
a claim about code no tool checks (`[Extern-Decl]`, D-0146); its parameter
and result types must be FFI types — integers, floats, `bool`, `void`, raw
pointers, and plain structs of those passed by value (D-0252) — so every
image that crosses is a value of its type
(`rule.trust.extern-call`, `FfiType`; `[Extern-Non-Ffi-Type]`,
`diag.extern-non-ffi-type`). A call needs `unsafe` and asserts that the
callee honours the signature and touches only cells reachable from its
pointer arguments and its own allocations (`[Extern-Call]`); an `extern fn`
is never a `fn` value, whose call would need no `unsafe` (`[Trusted-Call]`).
Text crosses as a `CString` one way and `String::from_c` the other
(`rule.stdlib.ffi`). Its result is an
*unchecked claim* (`state.trust`), and a checked validator turns a claim into
an internally produced value (`[Trust-Transition]`; `inv.trust-transition`).
`extern "c";` names foreign code to link and changes no rule
(`rule.trust.extern-code`).

**The trusted library base.** The `unsafe` blocks in `std`'s normative
bodies are the *trusted library base*: each discharges the side-conditions
of the trusted rules it instantiates, and `spec/21` §0 lists every block with
the module, item, line, rules, and the fact that discharges them there
(`term.trusted-library-base`; `spec/21`, "What `std` asserts"). The table is
generated from the `// trusted:` comment each block carries, by
`impl/tools/trusted_table.py`, which refuses a block without one (`spec/21`,
"What `std` asserts"; D-0206). A row reads, for `Vec::push`: `[Rawptr-Write]`
and `[Rawptr-Move-In]` at index `len`, which after `grow` is below `cap`, so
the slot is inside the buffer `allocate` returned and no element object is
established there (`spec/21` §0, the trusted table). An implementation that
realizes a `std` item natively takes over that item's obligations
unchanged: its native body may reach only the trusted side-conditions the
listed body reaches, under the same facts (`term.trusted-library-base`).

**Assurance.** `spec/21` §0 also states, non-normatively, the four tiers a
claim about `std` rests on — core-language safety, the trusted library base,
functional conformance, security profiles — and a profile for each
security-relevant module, including what is and is not claimed about
constant time and that those modules are not independently audited
(`spec/21` §0, "Assurance", "Security profiles", "Not independently
audited"; D-0211). Tier 1 is the theorem of §10 below.

## 9. What `[Standing-Local]` lets an implementation elide, and why

**The question.** Every access re-evaluates `clash` (§4). An implementation
that compiles a function would rather check a reference parameter once, when
the function is entered, and then access through it unchecked. That is sound
only if nothing can make the path invalid or clashing behind the function's
back while it runs. Within one thread the function's own code is all that
runs, but another thread's steps interleave with it (`[Thread-Step]`).
`[Standing-Local]` is the lemma that says when those steps cannot matter
(`spec/08` §4, D-0188, D-0209).

**The statement.** Let `a` be a path held by thread `ℓ`. If the hypotheses
below hold, then after any step of any *other* thread, `a` is still valid and
still does not clash in its own mode — except for an instant, through a
projection the other thread forms for one access, and that access is the
one that faults (`[Standing-Local]`, `spec/08` §4). In plain words, the
hypotheses are:

- **H1** — `a` is not lock-derived. A lock-derived path's standing depends on
  `sync-exempt`, which another thread's `[Lock]` changes (`rule.conc.lock`,
  `lock-derived`).
- **H2** — `a` is not pending: it has been checked against the other
  arguments of its call (D-0107).
- **H3** — `a` is valid and does not clash in its own mode, now.
- **H4** — if `a` is a slice, no live object lies over the cells it covers
  (such as a `Vec`'s element given an object by `[Reclaim]`): an access
  through a slice also meets the objects over its range, which `clash(a, ·)`,
  being about `a`'s own object, does not consider.
- **H5** — `a`'s object is fresh storage, or `ℓ` is the only live thread.
- **H6** — `ℓ` itself forms no path from `a`, hands `a` on, or leaves the
  frame or scope that holds `a`, while the property is relied on.

**Why each other-thread step is harmless.** The lemma's proof sketch goes
through every kind of step another thread `ℓ'` can take (`spec/08` §4, the
case list). A lasting `[Borrow]` that would clash with `a` is refused,
because `[Borrow-Denied]` sees `a`. A projection is formed unchecked, but
from a path `ℓ'` holds, and it can conflict with `a` only if it is an
ancestor of `a`'s; it then exists only for its one access, which clashes with
`a` and faults (`[Read-Conflict]`, `[Write-Conflict]`). Reads and writes
through `ℓ'`'s own paths touch other objects' cells, or `a`'s under a path
whose borrow was refused. Ending or moving `a`'s object (`[Destroy]`,
`[Authority-Transfer]`, `[Relocate-Out]`, `[Store-Binding-Place-Transfer]`)
needs `solitary`, which `a` refutes; overwriting it needs an exclusive path
overlapping `a`, refused as above. `[Thread-Affine]` stops `ℓ'` accessing
through a copy of `a` or of an exclusive ancestor of `a` that is `ℓ`'s.
`[Lock]` and `[Guard-Drop]` change only lock state, irrelevant by H1.
`[Spawn]`'s arguments are formed by `[Borrow]`. Joins, thread ends,
handle destructors, unwinds, containment and cancellation end frames of
`ℓ'` or of threads it joins, never `ℓ`'s frame (H6). The trusted raw writes
and copies write no cell a live path reaches, by their side-conditions —
the trusted library base's claims, true in a conforming implementation
(`spec/08` §4; `spec/21` §0).

**Why H5.** The one step kind no `clash` or `solitary` premise covers is
`[Release]` or `[Deallocate]`: they end every reclaimed object over the
released cells and require no `solitary` (`spec/08` §4, the `[Release]`
case). An object over raw storage — a `Vec`'s element, a map's entry — is
ended by whoever holds an exclusive path to its *container* (a push that
grows the buffer, `pop`, `clear`, a rehash), and that path does not clash
with a path to the element, because the container's cells and the
element's are disjoint (§7, `foreign-held`). A fresh-storage object has no
such owner: it ends with its frame, in `ℓ`, or by `[Destroy]`, which
`solitary` refuses. And when `ℓ` is the only live thread as `a` arrives, no
other thread can hold or form such a path while `ℓ` stays in the frame,
since a thread `ℓ` spawns receives only what `ℓ` passes it (`spec/08` §4).
Since D-0214, `[Release]` itself faults when the path to the element is an
exclusive one of another thread (`foreign-held`), so H5 is needed only for
shared paths and for paths of `ℓ`'s own (`spec/08` §4, the `[Release]`
case; D-0214).

**What it licenses.** An implementation may check such paths once, where
they arrive — for a function's reference arguments, at its entry: valid,
clash-free, not lock-derived, pairwise disjoint where one is exclusive,
pending ones checked against one another, a slice's range free of live
objects, and, unless the thread is alone, not over raw storage — and then
omit `[Read]` and `[Write]`'s use-time `clash` test on them while H6 holds
(`spec/08` §4, the paragraph after the case list). A program run this way
gives a result the rules admit, and every conflict involving such a path is
still diagnosed, at the other thread's access (same paragraph). This is one
instance of the general permission to leave out a check that cannot fail
in any interleaving (`[Conc-Checks-Omitted]`) and of the rule that further
analyses may discharge checks but not change acceptance (`spec/14` §6,
D-0206).

**What the implementations do with it.** The decision records describe
how `cobc` uses the lemma; they are rationale, not rules (`spec/README.md`,
"What is normative"). D-0188 introduced dual bodies: for suitable functions
`cobc` emits a body with every reference parameter accessed directly and a
body checked as before, and runs the direct one when a runtime test at entry
finds every incoming path valid, not lock-derived and pairwise free of
overlap where one is exclusive. D-0209 stated the lemma
with its hypotheses after writing out the step kinds found the `[Release]`
case: `cobc` had compiled some functions with no entry test and read freed
cells where `coby` reported `diag.stale-binding`; H5 was added, `cobc`'s
entry tests refuse a path over a reclaimed object while other threads are
live, and `cobc --all-checks` compiles every parameter access checked, for
differential runs (D-0209; `conf.direct-param-element-grown-by-other-thread`,
`conf.frozen-param-element-grown-by-other-thread`). D-0219's model then
found that the entry test had assumed the caller formed the borrow, which is
false for an existing reference passed on whole; the test now requires each
argument to be clash-free as it arrives, as H3 says (D-0219, "Tool defects
found"; `conf.reference-passed-on-while-reborrowed`).

## 10. The safe-program theorem, in plain words

**The statement.** A *safe program* is a safe client run on a conforming
implementation; for it, every reachable state of its execution satisfies
every proposition in `spec/03-invariants.md`, unconditionally
(`term.safe-program`, D-0206). The specification states it as the
definition of `term.safe-program` and argues it through the rules; the
decision records note that no mechanised proof has been made (D-0206,
Results; D-0209, "Not done").

**Hypothesis 1: the program is well-formed.** It is a set of items in one
root module, every item is well-typed, every static `rejected` rule is
satisfied — which includes every guard the flow analysis refutes — and it
has exactly one valid `main` (`rule.fn.program`; `term.program`). A
program that is not well-formed is rejected before it runs
(`term.conformance`).

**Hypothesis 2: the program is a safe client.** None of the program's own
modules contains an `unsafe { }` block (`term.safe-client`). `std`'s modules
are not the program's own: `std` is declared at the root of every program by
the implementation, and the `unsafe` blocks in its bodies belong to the
trusted library base (`term.safe-client`; `term.trusted-library-base`). A
safe client therefore makes no trusted claim of its own; since an `extern`
call needs `unsafe`, it calls no foreign code except through `std`
(`rule.trust.extern-call`, prose; `[Unsafe-Rejected]`).

**Hypothesis 3: the implementation conforms.** `term.conformance` has four
parts: for every program, its observable behaviour — the termination outcome
and the sequence of `extern` calls with their arguments — is one the rules
admit; it rejects exactly the statically ill-formed programs; every case in
`spec/conformance.md` holds; and its trusted library base makes only true
claims (`term.conformance`; `rule.fn.program`, "observable behavior"). The
last part includes the native realizations: an implementation that realizes
a `std` item natively may reach only the trusted side-conditions the listed
body reaches, under the same facts (`term.trusted-library-base`).

**Why that suffices.** Every trusted side-condition a safe client's
execution reaches lies in the trusted library base, and keeping those claims
true is the implementation's obligation, not the program's; so the
`[Trusted-Claim-False]` boundary is never reached, and every step of the
execution is constrained by the rules (`term.safe-program`;
`[Trusted-Claim-False]`; `rule.trust.unsafe`, prose). The rules in turn keep
each invariant: by construction (`inv.identity`, `inv.origin-stability`,
`spec/04` §3), by a check at every access (`inv.alias-validity`,
`inv.temporal-validity`, `inv.initialization-validity`, `spec/08` §3), by the
single-holder discipline and the scope sweeps (`inv.resource-authority`,
`spec/07` §5), by the threads' rules (`inv.concurrency-validity`,
`rule.conc.guarantee`), and by checked arithmetic and validation at the
trust boundary (`inv.arith.range-validity`, `inv.trust-transition`). A
static discharge removes a check only where it cannot fail (`spec/14` §6;
`[Conc-Checks-Omitted]`). A safe client that calls no `std` function whose
body holds an `unsafe` block reaches no trusted side-condition at all
(`term.safe-program`).

**What the theorem does not say.** It does not say a safe program runs
without faults: each invariant's *Failure behavior* is a diagnostic, and a
violated check is a fault that ends the program or the thread
(`spec/03`, each entry's Failure behavior; `rule.fail.fault-unwind`). It does
not exclude deadlock, which is diagnosed (`[Deadlock]`), or running out of
stack, which is a fault (`[Call-Stack-Exhausted]`, D-0077). It does not say a
program behaves the same everywhere: an implementation-independent program
is governed by the same rules on every conforming implementation,
parameterised by that implementation's documented choices and admitting the
per-run variation the rules declare, and only a *portable* program's
observable behaviour is the same under every choice and variation
(`term.implementation-dependent-program`, `term.portable-program`). And it
says nothing about a program whose own `unsafe` blocks make a false claim:
for such an execution every behaviour from that step on is admitted
(`term.conformance`; `[Trusted-Claim-False]`).

## 11. What the executable model covers, and what it does not

**What it is.** `impl/model` (`cobmodel`) is a separate program written from
the specification alone, sharing no code with `coby`, `cobc` or their
runtime, that runs a fragment of the language by the rules and lists every
outcome they allow a program: its static verdict and, for an accepted
program, every result of every interleaving of its threads (D-0219;
`impl/model/README.md`). Programs are generated, run by the model, and run
by `coby` and by `cobc --explore` under two C compilers; every outcome a
tool shows must be in the model's set (D-0219, Decision). It was chosen over
a mechanised proof and over further review (D-0219, Options).

**The fragment.** References to `i32`, a struct holding an exclusive
reference and shared references to it, `Vec<i32>` and vectors of exclusive
references, mutexes and guards, channels of exclusive references, and
handles; declarations, assignments, blocks, `if`, `match` on an `Option`,
`printf`; calls of a fixed set of helper functions, `Vec`'s operations,
`Mutex::new` and `lock`, the channel operations, `spawn` and `join`
(`impl/model/README.md`, "What it covers"). The static half implements
typing for the fragment with D-0218's shared-path rules,
`rule.control.flow-analysis` as tabulated, `rule.temporal.ref-escape` and
`[Ref-Binding-Last-Use]`, the first finding in source order being the
outcome (D-0110). The dynamic half implements `Σ` as `spec/04` gives it,
`held-by` derived from storage, `[Read]`, `[Write]`, `[Borrow]`, `clash` with
`sync-exempt`, `[Thread-Affine]`, `[Thread-Lend]`, statement and block exits,
calls with pending arguments, the effects of `Vec`'s bodies including
`[Release]` with `foreign-held`, `[Lock]`, `[Guard-Drop]` and poisoning,
channels, `[Deadlock]`, `[Channel-Deadlock]`, contained faults and
`[Join-Failed]`, and every interleaving, explored depth first with
duplicate states merged (D-0219, Decision; `impl/model/README.md`).

**What the text licenses.** `cobmodel check --as-licensed` also models what
the discharge table lets an implementation omit: each statement's accesses
the table proves clash-free are marked, and one that `clash` refutes all the
same ends in the outcome `model.proven-but-false`. Over the sample it reports
none; with the transfer table as it stood before D-0236 it reports eleven
programs, which is how the review's F-01 shows in the model (D-0241, D-0236).
Battery stage 1 runs it after the comparison (`impl/model/README.md`).

**What it leaves out.** Closures, slices, a handle stored in a container,
arithmetic beyond `+`, text, loops,
generics and raw pointers are not modelled (`impl/model/README.md`, "What it
covers"). Since raw pointers are not modelled, the trusted library base
appears only as the effects of `Vec`'s bodies (`impl/model/README.md`,
"Dynamic rules"). Programs are
enumerated up to a small number of statements, plus random longer ones, so
agreement is evidence over that set, not a proof (D-0219, Results). Where
the text did not decide something, the model took the reading both tools
take, said so in its source, and D-0219 listed it as a finding; D-0221 and
D-0222 settled those findings, and the model follows the text on each
(`impl/model/README.md`; D-0219, Findings).

**What it found.** Six places where the text was ambiguous or had a gap and
both tools already did the evident intent, each repaired as a clarification
with a case (D-0219, Decision, bin (c)): a projection path had no end
(`[Field-Access]`, `conf.projection-one-access`); a returned reference
parameter was invalidated at block exit (`[Block-Exit]`,
`conf.returned-reference-parameter`); the last-use end was missing from the
flow table (`[Ref-Binding-Last-Use]`); a received value was exposed to the
sender's statement exit (`[Recv]`); a vector of references lost them when it
grew (`[Copy-Raw]`); and `*v[k] = e` for a vector of exclusive references
(`[Index-Vec]`) (D-0219, Findings 1–6). Four more needed the owner's
decision: a closure call borrows the closure shared (`[Closure-Call]`), the
deferred last-use end, `Vec` elements not being places of the flow analysis
(D-0221), and a shared reference to a container not freezing its elements,
repaired so that writing an element takes the container exclusively
(D-0222). It also found tool defects, among them the `cobc` arrival test of
§9 (D-0219, "Tool defects found").

**How it is used.** `impl/model/sample` holds hand-written cases, every
program that ever disagreed, and a selection of the enumeration; battery
stage 1 and `impl/tools/suites.sh` compare it with both tools, and a clean
comparison lets an edition's evidence tier say *modelled* (`spec/README.md`,
"Evidence tiers"; D-0219, Results; `impl/model/README.md`, "The sample and
the battery"). A model outcome no tool showed is advisory, since the tools try
a few schedules and the model all of them (`impl/model/README.md`,
Commands).

## 12. Where to go next

The rules themselves, in the order `spec/README.md` gives
(`spec/README.md`, "Reading order for an implementer"). The worked
derivations in `spec/conformance.md`, which take a fragment through the
rules step by step (`spec/conformance.md`, Purpose), and the end-to-end
examples of `spec/examples.md`. The diagnostics registry, for what each
diagnostic means and which rule produces it (`spec/registry/diagnostics.md`),
and the feature registry for what was proposed and decided
(`spec/registry/features.md`). `spec/IMPLEMENTATION-NOTES.md` for every
choice an implementation must make and document, every point of permitted
nondeterminism, and every trusted condition (`spec/README.md`, "What you
must decide and document"). The guide (`htmlguide/`) for the language as a
programmer uses it. And the decision records, newest first, for why a rule
reads as it does (`spec/02` §6).

## Change Log

- 1.2.0 — `CHG-0306` (D-0249): atomics are sequentially consistent (§ on interleaving). `CHG-0309`, `CHG-0310` (D-0252): structs by value across `extern`, an `extern fn` is never a value, C strings.
- 1.1.0 — `CHG-0290`: the fourth review's corrections (F-09: `[Block-Exit]`'s order after D-0233; the intrinsics' reduction to `[Write]`); D-0235–D-0241 explained (§3, §4, §5, §7, §11).
- 1.0.0 — `CHG-0278`: first version (WI-19).

