# D-0281 — A cost model for references: which are free, which are checked, and what a check costs

Status: ACCEPTED IN DIRECTION (2026-10-25: the owner handed the proposal to the assistant "to manage autonomously,
with authorisation given to make all necessary decisions to push CobaltC in the direction it needs to go in order
for it to be considered a serious systems programming language"). Adopted: the review's verdict in §12, and the
order measurement (`cobc --cost`), invariant map and per-operation cost contract, rule text, prototypes. The tier
rules are not yet normative; each phase takes its own change records. D-0278 (the Rust toolchain leads) and D-0279
(no untracked build) were decided first. Drafted PROPOSED 2026-10-25 at the owner's request ("put all options on
the table … so that sentence does not become the main criticism of CobaltC").
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md`
Depends on: D-0011 (single-parameter elision), D-0015 (frame-exit invalidation), D-0018 (access-path families,
use-time checking, container-bound reference lifetime), D-0111 (last use ends a binding), D-0135 (views of
temporaries), D-0189–D-0191 (slices), D-0199/D-0200 (compile-time discharge), D-0202 (per-thread heaps), D-0205
(frozen parameters), D-0218 (shared reaches read-only), D-0243–D-0245 (closures), D-0259 (`Arc`), D-0276 (the
working bound of ten times `cobc`), D-0278 (the Rust toolchain leads), D-0279 (no untracked build), D-0280 (deferred)
Affects, if accepted: `spec/04`, `spec/08`, `spec/10`, `spec/14` §6, `spec/16`, `spec/21` (`StringView::split` and
its kin), `spec/22` (one keyword), `spec/conformance.md` (a new kind of row), the guide's references chapter and
performance chapter, the book, `coby`, `cobc`, `cbc`, `cbrt`

## 0. The recommendation in one paragraph

Make the cost of a reference a property of the language, not of the compiler. Two tiers, defined by rules a
programmer can apply by reading the source: **tier S** (frame-held references: locals, parameters, results,
temporaries and binders, rooted in storage the function can name) costs nothing at run time, and a conforming
compiler is obliged to emit no bookkeeping for it; **tier D** (a reference kept inside data: a struct field, an
enum payload, a container element, a closure that outlives its frame, a handle's box) is spelled in the source,
because an aggregate that holds a reference must be declared `tracked` and a container of references shows the
reference in its type, and it costs a bounded, documented amount. Pin the model with *cost cases* in the
conformance suite (programs whose compiled form must call no tracking), rebuild the runtime for tier D around
counts kept in the owner rather than a table keyed by address, and make the standard library's collections of
views lazy so that the common text idioms are tier S. The semantics of faults do not change; what changes is that
the programmer can see, and the specification promises, where the cost is. Sections 4 and 5 give the alternatives
this was chosen over, including the stricter one (no stored references at all) that the measurements in §8 say the
showcase would not even notice.

## 1. The problem, precisely

The sentence: *a programmer cannot look at a view copy and know whether it costs three instructions or three
thousand.* It is true today, and the numbers behind it are:

| what | instructions |
|---|---|
| copying a two-word struct | 2–3 |
| copying a `StringView` into a local (Update 134) | 2,689 |
| the same after D-0280's prototype (not shipped) | 1,070 |
| `csvstat`'s remaining tracking after D-0199/D-0200 | almost none |
| `cbc` against `cobc` on the same input | 15 to 45 times slower; D-0276's bound is 10 |

Why it is true:

1. **The rules fix the outcome, not the cost.** `spec/08` and `spec/10` say when an access faults; `spec/14` §6
   says an implementation *may* prove a check away and keep less state. Nothing says it *must*, for any form.
   Discharge (D-0199, D-0200, D-0209, D-0218, D-0205, the objectless locals) has been treated as optimisation: each
   round adds forms the compiler happens to prove, and the set is documented nowhere a programmer would look,
   differs between `cobc` and `cbc` at times, and moves with every update.
2. **The runtime is shaped like the interpreter's state.** An object table keyed by address, a statement scope
   pushed and popped per statement, an object per view, holders as lists. This is the right shape for `coby`
   and for discovering the semantics; it is why the cost of a dynamic check is thousands of instructions rather
   than the handful it needs to be. Swift's dynamic exclusivity check is a few loads and compares; Vale's
   generational check is a load and a compare.
3. **The cost depends on what the compiler can see.** Passing a view through one more function, or putting it in
   a struct, moves it from "free" to "tracked" with no change in the text that formed it.

What a systems programmer requires, and what this proposal must deliver:

- **Visible.** The cost of an operation is readable from the source, by rule, without running `--report-tracking`.
- **Stable.** The same program has the same cost class under every conforming compiler and every edition.
- **Bounded.** Where a check exists, its worst case is small and stated, with no hidden allocation.
- **Checkable.** A test can assert that a program is in the free tier, and the conformance suite does so.
- **Honest.** Where the language keeps a dynamic check, it says why and what it buys.

## 2. What the semantics fix, and what is free to change

Fixed (observable, pinned by 225 dynamic and 529 static rows in `spec/conformance.md`): which access faults and
with which diagnostic; that the fault is at that access; that a binding's last use ends it (D-0111); that paths are
fine-grained (a shared reference to element 0 of a `Vec` does not stop a write to element 1,
`conf.vec-element-access-beside-held-element-ref`); that frame exit invalidates (D-0015); that a value's size is
what `spec/16` says.

Free: where a token lives; whether a reference carries anything beside its address; whether a check is performed or
proven; the number of words in a slice (`spec/16` §3a is implementation-defined); the standard library's choice of
eager or lazy collections of views, within its stated contracts.

Also free, if the owner says so, because nobody uses CobaltC yet: the granularity of conflict detection for data
reached through containers (§4, O8), and the set of places a reference may be stored (§4, O5). Both are breaking
changes; both are on the table at the owner's request; §6 says which this proposal takes and which it leaves.

## 3. How other languages answer the same question

Not only Rust. The question is old and every answer is a different point on three axes: *who decides* the cost
(rules, compiler, programmer), *what happens when a check cannot be discharged* (dynamic check, compile error,
restructure), and *where the dynamic state lives* (none, inline in values, a side table).

| language | mechanism | cost visible in source? | what it gives up |
|---|---|---|---|
| Rust | lifetimes, borrow checker; `RefCell`/`Rc` for the dynamic cases | yes: the dynamic case is a different type | rejects correct programs; annotation burden in data structures |
| Swift | exclusivity enforcement: static for locals and value types, dynamic per stored property of classes and globals; runtime overhead reported small | partly: classes are dynamic, structs static | whole-array exclusivity (no element granularity); ARC elsewhere |
| Hylo (Val) | mutable value semantics: references are second-class (parameter conventions `let`/`inout`/`sink`/`set`, projections via subscripts), never stored | yes: there is nothing to track | no reference inside data at all; collections of views are iterators |
| C# | `Span<T>` and `ref struct`: stack-only types, scoped-ref rules (C# 11); cannot be boxed, stored in a class, or captured | yes: the type says stack-only | same restriction as Hylo for those types; the rest is GC |
| Vale | generational references: each allocation has a generation, each reference carries one; a dereference compares; regions elide the compare | no, but the cost is one compare everywhere | detects use-after-free only; aliasing with mutation is allowed |
| Cyclone | regions with lexical lifetimes, dynamic regions when needed | partly | needed annotations in practice; the project ended |
| Ada / SPARK | accessibility levels: the standard states which checks are static and which dynamic (anonymous access types); SPARK proves them | yes: by rule | dynamic accessibility checks in the anonymous cases |
| Austral | linear types with regions, no runtime | yes | strict; no implicit anything |
| Pony | reference capabilities (`iso`, `val`, `ref`, `box`, `tag`), static | yes: in the type | a capability vocabulary to learn; GC per actor |
| Cone | permissions in the type, including run-time ones (`rc`, `lock`) | yes: the permission is written | a vocabulary of permissions |
| Lobster | compile-time reference counting with ownership inference; counts where inference fails | no | the same "which cases pay" question, answered by a compiler report |
| Nim | `var` views (experimental) with scoping rules; ORC otherwise | partly | cycle collector |
| D | `@safe` with DIP1000 `scope` inference | partly | GC by default |
| Zig | no checks; `ReleaseSafe` keeps bounds checks only | yes: nothing | no temporal safety |
| Go | GC, bounds checks, race detector only in tests | yes | GC; aliasing bugs are the programmer's |
| Checked C, SoftBound/CETS | bounds and temporal metadata beside pointers (fat or disjoint) | no | about 2× slowdown; C compatibility was the goal |
| CHERI, ARM MTE | capabilities or memory tags in hardware | no | hardware |
| C++ standard | complexity guarantees for containers and algorithms | yes: in the standard | precedent that a standard may require a cost, not only an outcome |
| Scheme | proper tail calls required by the report | yes | precedent again |

Three things to take from the table. First, the languages whose cost is visible all put it **in the type or in the
rules**, never in the compiler's cleverness. Second, the cheap dynamic schemes keep their state **inline** (a
generation, a count, an access record), never in a table keyed by address. Third, two languages (Hylo, C#) solve
the hard case by **forbidding stored references** and have users; one (Swift) solves it by **a dynamic check per
owner** and has very many users. CobaltC can take either, or both in layers.

## 4. The options

Each option: the rule, what the programmer sees, the cost profile, what breaks, the size of the work, and a verdict.
They are not all exclusive; §6 combines.

### O1. Publish the cost model (documentation only)

A chapter listing the forms the compilers discharge today, with `--report-tracking` as the oracle.
*Sees:* a list to memorise. *Cost:* unchanged. *Breaks:* nothing. *Work:* a week.
*Verdict:* necessary but not sufficient. The list is a snapshot of two compilers; it drifts, it differs between
them, and it does not stop "put it in a struct" from silently costing a thousandfold. Reject as the answer; keep
as part of every other option.

### O2. A normative two-tier cost model, without new syntax

`spec/14` §6 gains obligations. **Tier S:** a reference held by a frame (local, parameter, result, temporary,
`match`/`foreach` binder) whose root the function can name (a local, a parameter, a result, a field or element
reached from one of those); a conforming compiler emits no bookkeeping for it; conflicts among tier-S references
are decided at compile time (the function sees every access to the root, and a call site sees the roots of the
references it passes: the argument-disjointness rule that P4 §5 item 2 wanted to prove is a static rule here). The
one dynamic residue in tier S is a point check with no state, like a bounds check: `f(&mut v[i], &v[j])` compiles
to `i != j` at the call. **Tier D:** everything else: a reference inside a struct, an enum, a container, a closure
that outlives its frame, a handle's box. Tier D costs are stated per operation (form, copy, end, write, destroy) and
bounded.
*Sees:* a rule, applied by reading. *Cost:* S is zero by obligation; D is bounded. *Breaks:* nothing in programs;
it obliges both compilers to discharge all of tier S, which they nearly do for locals and not yet for binders and
parameters passed on. *Work:* the rule text; a completeness audit of the static pass against it; cost cases.
*Verdict:* the core. Accept. Its weakness is that tier D is invisible at the use site: `StringView h = p.text;`
is tier D because `p` is a struct holding a view, and nothing on that line says so. O3 and O4 fix that.

### O3. Tier D visible in types: `tracked` aggregates

A struct or enum with a field of reference type (transitively) must be declared `tracked struct` / `tracked enum`;
otherwise `diag.untracked-reference-field` at the declaration, with the repair text naming the two ways out:
declare it tracked, or store an index or an owned value. A container of references (`Vec<StringView>`,
`Option<ref<T>>`, `HashMap<K, StringView>`) is tier D by its type, which already shows the reference. A closure
that captures a reference and is stored is a value of a tracked type. So the rule the programmer applies is one
sentence: *a value is tracked iff its type names a reference, and a user type that does is spelled `tracked`.*
*Sees:* the cost at the declaration and at every use of the type's name. *Cost:* as O2. *Breaks:* a handful of
declarations (§8: none in the showcase, six in `std`, about eight in the self-hosted tools) gain a keyword; that is
the whole migration. *Work:* a keyword in both front ends, a check in both type checkers, `cobfmt`, the guide.
*Verdict:* accept with O2. It is the cheapest honest way to make the cost visible, and it is additive in
practice: existing programs add `tracked` where the compiler tells them to.

### O4. Static-only by default, `checked { … }` regions for the rest

Instead of marking types, mark sites: outside a `checked` block the compiler must decide every access statically
and rejects what it cannot (`diag.unproven-access`); inside, the dynamic model applies.
*Sees:* a block boundary. *Cost:* zero outside, dynamic inside. *Breaks:* every program that stores a reference,
until its uses are wrapped. *Work:* as O3 plus the error path.
*Verdict:* reject in favour of O3. Cost belongs to data, not to code regions: the same struct used in two places
should cost the same, and a `checked` block around a call tells the reader nothing about which value paid.
Ada's `pragma Suppress` and Swift's `@exclusivity(unchecked)` are the opposite direction (turning checks off) and
the evidence against region-scoped checking is that nobody can review it.

### O5. Second-class references: no stored references at all (Hylo, C#)

References (`ref<T>`, `StringView`, slices) may be held only by frames: parameters, locals, results (projections
under D-0011), temporaries, binders. No field, element, payload or captured-and-stored closure may hold one.
`StringView::split` and its kin return iterators, not `Vec<StringView>`. A parser keeps `ref<String, shared>
text` as a parameter of every function rather than as a field, or holds offsets. Long-lived links are indices,
handles (`Rc`, `Arc`) or owned values.
*Sees:* nothing to see; there is no tier D for references. *Cost:* zero; the runtime keeps no reference state at
all except in handles. Every conflict is decided at compile time by the rules of O2's tier S, which are then the
whole model, and the two compilers lose the object table, the scopes, the frames and half of `cbrt`.
*Breaks:* `Vec<StringView>` and `Vec<ref<…>>` at 122 sites in `std`, the showcase and the self-hosted tools
(mostly `split` results, which become `foreach` over an iterator and often get *faster*); the six `std` types and
about eight `cbc` types that hold views (§8); the `Option<ref<T>>` *results* of `HashMap::get` stay, being
projections. `cbc`'s `Fx`/`Frame`/`Facts` hold views into its tables and would hold indices.
*Work:* the largest option: the rule, the static pass made complete and the dynamic model removed from both
compilers and the runtime, `std` reshaped, the self-hosted compiler reshaped, then the fixed point again.
*Verdict:* the purest answer and the one with the best precedent for a *systems* language (C#'s `Span` is exactly
this and is used everywhere performance matters). I do not recommend it first, for two reasons: it removes a
capability the language has advertised (views in data) before we know how much real code wants it, and O2+O3
reach the same place for the programs that do not use it while keeping a priced door open for those that do.
If, after O3 ships, the count of `tracked` declarations in real code stays near zero, O5 is the natural next
edition: remove the door.

### O6. Owner-typed views (regions without the vocabulary)

A stored view names its owner's binding: `struct Parser { StringView of text; }` where `text` is a parameter
of the enclosing function or a field of the same struct holding an owned `String`. Validity is then a scoping
rule (the struct cannot outlive what it names), checked statically, as Cyclone's regions or Rust's lifetimes in
their simplest form.
*Verdict:* reject. The self-referential form (a struct holding a `String` and a view into it) is unsound under
moves and Rust forbids it for that reason; the parameter form is O5 with extra words; the general form is
Cyclone, whose authors reported that the annotations grew until users gave up. CobaltC's distinguishing promise
is no lifetime vocabulary; this option breaks it for the one case O3 prices and O5 forbids.

### O7. Generational references and no exclusivity (Vale, Go)

Keep temporal validity only: an owner carries a generation, a reference carries the one it was formed under, a
use compares; a write beside a live reference is allowed. Aliasing-with-mutation stops being a fault.
*Verdict:* reject. It discards the half of the model that makes CobaltC's single-threaded programs
data-race-free by construction and that D-0218, D-0235 and a hundred conformance rows define; 225 dynamic rows
would change meaning. It would also not make the cost visible, only uniform. Generations remain a useful
*implementation* of validity inside tier D (O8) and nothing more.

### O8. Rebuild the dynamic tier: counts in the owner, no table

For tier D only: each owner that a tracked reference can point into (a `String`, a `Vec`, a `HashMap`, a box of
`Rc`/`Arc`, a tracked struct's own storage) keeps a small header: a count of live shared holders and an exclusive
flag; forming a reference increments, its end decrements, a write or destroy of the owner checks the count. This
is `RefCell`'s and Swift's mechanism, with CobaltC's diagnostics. Cost per operation: an increment, a decrement, a
compare: tens of instructions at worst, often optimised away by the C compiler when the owner is local.
`destroy-while-aliased` falls out (count non-zero at destroy); so does the LTO bug class that CHG-0372 fixed,
since nothing is keyed by an address that a compiler may reuse. Threads: owners are confined (D-0202) or under a
guard (D-0259), so plain counters suffice.
*What it changes:* granularity. A count per owner does not know *which* element or field a holder points at, so a
write to element 1 beside a held reference to element 0 would fault, as it does in Swift. Three ways to keep the
fine-grained rule: (a) keep today's path table for tier D only (semantics identical; tier D keeps today's cost,
which after O2/O3 is paid only by code that asked for it); (b) counts per owner plus static disjointness for
fields of frame-held aggregates (Swift's rule), and accept whole-container exclusivity, listing the rows that
change (`conf.vec-element-access-beside-held-element-ref` and its kin); (c) counts per owner for validity and
destroy, the path table only for conflicts. I recommend starting at (a) and deciding (b) on measurements of tier
D in real code, since the choice is semantic and the owner's.
*Breaks:* nothing under (a); the element-granularity rows under (b). *Work:* the runtime's core and both
compilers' lowering of tier D; a long project, but one that *replaces* P4 §5 items 1 and 6 rather than adding to
them. *Verdict:* accept as the third phase, path (a) first.

### O9. An unchecked release build (D-0279)

Keep the model, strip the checks from release binaries, as Zig's `ReleaseFast` or C#'s unsafe. *Verdict:* this
solves a different problem (the cost of the toolchain's own binaries) and is already before the owner as
D-0279. It does not answer the sentence: the checked build is the language, and its cost must be predictable.
After O2/O3/O8, the case for an unchecked build weakens, because tier S is already unchecked and tier D is cheap;
Swift ships exclusivity enforcement on in release for this reason. Leave D-0279 as it is; revisit after O8.

### O10. Hardware (CHERI, MTE)

*Verdict:* not now; note that O8's counts are what a capability system would make free.

## 5. Comparison

| option | cost visible | stable across compilers | semantics kept | breaks existing code | work | `cbc` after (estimate) |
|---|---|---|---|---|---|---|
| O1 docs | partly | no | yes | none | small | unchanged |
| O2 rules | by rule | yes | yes | none | medium | tier-S code at `cobc` speed; `cbc` itself limited by its tier-D use |
| O3 `tracked` | yes, in types | yes | yes | ~14 declarations gain a keyword | small on top of O2 | as O2 |
| O4 `checked {}` | at sites | yes | yes | every stored reference's uses | medium | as O2 |
| O5 second-class | nothing to see | yes | reduced (no stored references) | 122 sites + ~14 types | largest | all code at `cobc` speed; runtime nearly gone |
| O6 owner-typed | yes | yes | yes | all stored views | large | as O2 |
| O7 generational | uniform, not visible | yes | no (exclusivity dropped) | 225 rows change | large | fast, wrong language |
| O8 counts | n/a (tier D's price) | yes | (a) yes, (b) coarser containers | (a) none | large | tier D at tens of instructions |
| O9 unchecked release | n/a | n/a | not in release | none | small | release at C speed, unchecked |

The `cbc` column assumes `cbc`'s own code moves its tables to indices (it holds views in `Fx`, `Frame`, `Facts`);
the estimate is from `cbc`'s profile (tracking entry points and scopes together above 40% inclusive) and should
be measured with the probe in §9 before anyone relies on it.

## 6. The recommendation, in phases

**Phase 1, the model (O2 + O3 + O1).** Write tier S and tier D into `spec/14` §6 (obligations) with the tier-S
discipline in `spec/08` and `spec/10`; add `tracked` to `spec/22` and `spec/16`; add *cost cases* to
`spec/conformance.md` (§9); audit both compilers' static passes for completeness over tier S and make them
complete (binders and passed-on parameters are the known gaps, which is what D-0280's measurements said); make
`StringView::split`, `lines`, `chars` and their kin lazy (an iterator type, `foreach`-able; a `collect` for the
programs that want the `Vec`). Edition with the breaking change listed: `tracked` required on aggregates with
reference fields. The guide's references chapter gets the one-sentence rule; the performance chapter becomes a
statement of the cost model rather than advice.

**Phase 2, tier S costs nothing in fact.** Both compilers emit no scope, frame, object or path operation in a
function whose values are all tier S, by construction and checked by the cost cases; the per-statement scopes go
from every function to tier-D functions only. This is where `csvstat`, `wordpar`, `extsort` and the showcase land
at the cost of the equivalent C, and it is measurable per program with `--report-tracking` becoming a conformance
oracle rather than a diagnostic.

**Phase 3, tier D is cheap (O8, path a then a decision on b).** Counts in owners for validity and destroy; the path
table kept for conflicts among tier-D holders only; then measure how much tier D real programs keep and decide the
granularity question on that evidence.

**Phase 4, decide O5 on evidence.** If after a few months of writing programs under phase 1 the `tracked`
declarations in real code are rare and the lazy iterators cover the idioms, remove the door in an edition and
delete the dynamic model from the compilers. If they are common, tier D has earned its place and stays priced.

**Sequencing with D-0278.** Phases 1 to 3 change both compilers and the runtime. That is the strongest argument
yet for deciding D-0278 (one toolchain) first: every phase is half the work with one compiler. The P4 plan's §5
items 1 (views) and 6 (scopes) are subsumed by phases 2 and 3; item 2 (parameter disjointness) becomes a rule in
phase 1; items 3 and 4 stand.

## 7. The tier-S discipline, concretely

What a programmer needs to know, and what the rule text must say:

- **Held by a frame:** a reference bound to a local, a parameter, a `match` or `foreach` binder, a temporary, or
  returned as a result, is tier S. Copying it, passing it, returning it (D-0011) and ending it cost nothing.
- **Rooted where the function can see:** its root is a local, a parameter, a call's result, or a field or element
  reached from one of those by projection or index. A reference reached through a handle (`Rc::get`,
  `Arc::get`, a raw pointer) is rooted in the box, which is tier D's.
- **Conflicts are decided at compile time:** the function sees every access to a root it owns; a call site sees
  the roots of what it passes and applies disjointness (two exclusive paths with the same root and possibly
  overlapping projections are an error; two indices into one container are a point check, `i != j`, at the call).
  A tier-S program is accepted or rejected before it runs, except for point checks.
- **Leaving the frame makes it tier D:** storing the reference in a field, an element, a payload or a stored
  closure requires the destination's type to say so (`tracked`, or a container type naming a reference), and from
  then on the value pays tier D's prices: a count or a table operation at form, copy, end, write and destroy,
  each bounded and listed in `spec/14` §6.
- **The standard library is tier S unless its type says otherwise:** iterators of views are frame-held;
  `Vec<StringView>` is tier D and says so in its name.

Open questions inside this discipline, to settle in the rule text: results of functions with two or more
reference parameters (today D-0011 covers one; the rule must name the root or require an owned result);
closures capturing references that are called but not stored (tier S, as D-0243 treats them, if the capture
list is visible); `shared` references passed on to functions that store them (the callee's parameter type must
then be tracked, which makes the call site's intent visible: a function that keeps your view says so in its
signature, as C#'s `scoped` does in the negative).

## 8. What the corpus says today (measured 2026-10-25)

| corpus | aggregates | with a reference field | which |
|---|---|---|---|
| showcase (14 tiers) | 96 | 0 | — |
| `std` | 146 | 6 | `StringView` itself, `JsonReader.text`, `TomlReader.text`, `BitReader`, `OnceRun.once`, `PriorityQueue` (comparator) |
| self-hosted tools | 560 | ~8 | `Decl`, `GlobalDecl`, `Facts`, `Frame`, `Fx`, `Rewrite` (views into source and name tables) |
| examples | 8 | 0 | — |

Sites naming `Vec<StringView>` or `Vec<ref<…>>` across the same corpus: 122, the majority `StringView::split`
results walked by `foreach`. That is the single idiom that puts most views into tier D today, and it is the one a
lazy iterator removes without the programmer doing anything. The showcase, the language's public face, would
compile unchanged under O3 and nearly unchanged under O5.

## 9. Cost cases and the measurement to do first

**Cost cases.** A new kind of conformance row: `cost.<name>`, a program and the assertion that a conforming
native implementation's output for it calls no tracking entry point (`--report-tracking` reports none), or, for
tier D rows, that the runtime's counts at exit are bounded (`COBALTC_RT_STATS`). `coby` is exempt as an
interpreter; the rows bind compilers. The first rows are the showcase programs and the three text benchmarks.

**The first measurement, before any rule text:** a `cobc --cost` report (a day's work on `--report-tracking`)
that classifies every reference-forming site in a program as tier S or tier D under §7's rules, and the count of
runtime operations each class causes at run time. Run it over the corpus. It answers the two questions the
decision depends on: how much of `cbc`'s and the showcase's tracking is tier S that the compilers merely fail to
discharge today (phase 2's gain), and how much is genuine tier D (phase 3's, and the evidence for phase 4).

## 10. Risks

- **Rule design replaces compiler cleverness, and must be complete.** Tier S is a syntactic class; a compiler
  that cannot discharge a tier-S form is non-conforming, not slow. The risk moves to writing the rules so that
  the class is decidable and the static pass is sound and complete over it. The existing static pass and the 529
  static rows are most of the way; the audit is the work.
- **Two compilers and a runtime.** Every phase is done twice until D-0278 is decided.
- **Library churn.** Lazy `split` changes a contract programs rely on (`Vec::len` of the parts, random access);
  a `collect` keeps them, at tier D's visible price.
- **Diagnostics under counts (phase 3).** Today's fault names the access and asks the programmer to end "the
  conflicting reference" without naming it; counts can do the same. Naming the other reference would need the
  table, which `-g` can keep.
- **The self-hosted compiler's own style.** `cbc` keeps views in its tables; moving to indices is a rewrite of
  parts of it, and also the demonstration that the discipline is livable for a large program.
- **A second criticism waiting behind the first:** "faults at run time are not safety". Tier S answers it for
  the programs that stay in tier S (they are decided statically); tier D remains a priced, visible choice, which
  is the same position as `RefCell`.

## 11. Questions for the owner

1. Phase 1 as drafted (O2 + O3, `tracked` on aggregates, lazy collections of views, cost cases)?
2. Should the `cobc --cost` measurement of §9 run first, with the rule text waiting on its numbers? (My
   recommendation: yes; it is a day and it sizes everything else.)
3. D-0278 before this? (My recommendation: decide it now; one toolchain halves phases 1–3.)
4. Is O5 (no stored references) to be kept as the stated destination if `tracked` turns out rare, or ruled out
   so that tier D is permanent?
5. Granularity (O8 b): is whole-container exclusivity acceptable in principle, or is element granularity a
   promise to keep whatever it costs tier D?

## 12. Review (an external review, 2026-10-25) and responses

The review approves the measurement, endorses O2 and O3 in direction, and withholds acceptance of the tier
definitions and O8's representation until four risks are resolved. Responses, and what the next revision changes:

| review point | response | change to the draft |
|---|---|---|
| Tier S is not precise enough: naming a root is not deciding every aliasing relation; callees, multiple reference parameters, returned references, derived references, closures, handles, foreign code and threads each need an explicit answer | Agreed in substance. CobaltC has no separate compilation or function pointers, but the conclusion holds: the classification must be **modular**, decidable from signatures (a callee that keeps a reference has a `tracked` parameter type; D-0011 roots a result), with disjointness established inductively at call sites. This is a theorem for the mechanised fragment (D-0265), not a residue of §7 | §7's open questions move to the definition; a soundness statement is added; the eight cases get one rule each |
| `tracked` shows participation, not cost: construct, copy, move, read a field, end, destroy, store each need a stated contract | Agreed. The contract is **properties** (may allocate? constant time? global lookup? metadata bound?), not instruction counts. Under phase 3 path (a) tier D keeps a table, so its promise is "amortised, lookup, may allocate" until counts replace it; say so | a per-operation contract table for each tier-D representation, in `spec/14` §6 |
| O8: owner counts conflate owner lifetime, path conflicts and container invalidation; map invariants to metadata first; keep fine-grained semantics; do not claim "tens of instructions" | Agreed, with one simplification: CobaltC has **no separate container-invalidation invariant** (a reallocation is a write to the container and faults while an element reference is live), so the map has two rows. Counts settle owner lifetime exactly and path conflicts not at all. Path (a) first; the estimate stays an estimate until measured | an invariant-to-metadata map (§ new); O8's cost figure marked "to measure" |
| Cost cases must test the language's contract, not a function name; three layers (semantic, representation/cost, performance); `coby` stays the semantic reference | Agreed. The specification states the property; each implementation supplies its oracle (`cobc`: no tracking call and no runtime allocation in the emitted C); layer C is not conformance | §9 restated as properties with per-implementation oracles |
| Hidden risk: deciding tier-S conflicts statically rejects programs that today run and fault only on some paths; needs an explicit fallback and a diagnostic naming both accesses; "emit no bookkeeping" must be defined | Agreed; this is Swift's trade and acceptable only with the fallback. **Fallback: `tracked` on a binding** (a local or parameter opts into tier D at its visible price). Wording: tier S means **no reference state at run time**, with bounded point checks allowed, not "costs nothing" | `tracked` bindings added to O3; the diagnostic specified; §0 and §7 reworded |
| Lazy iterators are a library optimisation, not a prerequisite | Half agreed: not a prerequisite for the *model*, but for the *outcome*: without them the commonest text idiom is tier D and the showcase is visible-but-slow. Separate CHG, same phase | lazy `split` and kin become their own change record in phase 1 |
| D-0278 need not precede the design work | Agreed for the measurement and the rule text; not for the prototypes, which are done twice until it is decided | sequencing: measure → invariant map and rules → D-0278 → prototypes |
| Keep O5 alive; corpus counts show current use, not need; test on real code | Agreed. Experiment: convert `JsonReader` and `TomlReader` (the `std` types that hold a view) and one `cbc` table to no stored references; record speed and API complexity | added to §9 as the second measurement |
| Granularity: preserve element-level semantics initially | Agreed; it was path (a) already | none |
| Sixth question: a stable cost contract without promising every tier-S operation is free of all run-time validation | Yes, and the draft must say it: zero *reference state* and zero *lifetime bookkeeping* are the promise; bounds and point checks remain, bounded and stated | §0 and §6 |

What the review did not weigh: keeping the path table beside owner counts means two runtimes until one wins, and
that engineering cost is the strongest reason for deciding D-0278 before phase 2. Verdict adopted: approve the
measurement; endorse O2 and O3 in direction; write the invariant map and the per-operation cost contract before
any normative text; keep O5 alive and test it on the readers.
