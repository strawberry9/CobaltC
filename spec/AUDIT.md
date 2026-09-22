# CobaltC Specification Audit

Status: supporting artifact (Master Instructions §24 — created because it
resolves a current design problem: the correctness and completeness of
the specification as it stands)
Audit date: 2026-09-20
Scope: every file under `spec/` (41 artifacts, ~8,000 lines), checked
against `CobaltC_Master_Instructions.md`
Method: each finding below was verified directly against the rule text
(by grep/read), not taken from changelogs or from `spec/22` §5's
self-audit. Where a finding contradicts a claim the specification makes
about itself, the contradicting claim is named.

## How to use this document

This is a work list for the design AI, governed by the master
instructions. Each finding has an id (`A-nn`), a severity, the files and
rules involved, what is wrong, and an **acceptance criterion** — a
concrete condition that, once true, closes the finding. Work the
findings in the order given in §6. Constraints that apply to every fix:

- Master Instructions §1 still holds: fix the *specification*; write no
  implementation.
- `spec/02-schema.md` §6 still holds: an `ACCEPTED` Decision is never
  edited — a fix that changes a decision needs a new `D-XXXX` that
  supersedes it, with a pointer added to the old one. Artifacts that are
  still `PROVISIONAL` may be revised in place, with a version bump and
  changelog entry, as has been the practice.
- Do not fix an example or conformance case by weakening a rule to
  match it, unless the rule is what the finding says is wrong.
- After the Critical findings are closed, **regenerate**
  `spec/examples.md` and `spec/conformance.md` from the corrected rules
  rather than patching individual entries (see A-30).

Severity: **Critical** = ordinary programs (including the spec's own
examples) are ill-formed or unsound under the rules as written.
**Major** = a master-instructions requirement is not met. **Systematic**
= internal contradictions between artifacts. **Minor** = precision or
hygiene.

## 1. Overall verdict

The specification genuinely follows the constitution's *process*:
mechanism-neutral derivation (§7), decision records with real rejected
alternatives (§11/§22), invariant-first ordering (§23), and no leakage
into implementation (§1 — fully respected; the master instructions file
is byte-for-byte unchanged). Two "by construction" results
(`inv.identity`, `inv.origin-stability`, `spec/04` §3) are real and
correctly argued. The change-log trail is candid.

It is **not** what `spec/22` §5 claims — "a complete, formally precise
specification with one narrow open item." The aliasing/lifetime
mechanics have at least four defects serious enough that the
specification's own example program (`push(&mut nums, 10); push(&mut
nums, 20);`) is rejected by its own rules; the most common statement
form (`let x = e;`) has no semantics; §12 is violated pervasively; and
the examples/conformance suite contradicts the rules in more than
thirty places. `spec/22` §5 must be rewritten once the findings below
are addressed (A-31).

## 2. Critical — semantic defects

### A-01 · Access paths are never invalidated at scope exit
**Files:** `spec/14-control-flow.md` §1 (`[Block-Exit]`),
`spec/04-abstract-state.md` (`state.access-paths`),
`spec/07-resource-authority.md` (`[Destroy-Not-Solitary]`).
**Verified:** no rule in `spec/14` or `spec/15` ever sets an access
path's `valid := false` at frame exit. `Block-Exit` restricts
`bindings` and reactivates suspensions only.
**Consequence:** every binding, borrow, and projection formed in a frame
stays live forever. `{ let r = &v; } drop(v);` then hits
`[Destroy-Not-Solitary]` — `ex.destroy-while-borrowed`'s own "corrected"
form fails. A reference parameter's projections (`v.len` inside a
callee) outlive the call and block the caller's later destroy.
**Acceptance:** `[Block-Exit]` invalidates every access path whose
`formed-at` event lies within frame `f` (bindings, borrows, and
projections alike), before its destruction sweep; `inv.temporal-
validity`'s registry entry records this as an invalidation event;
`conf.destroy-while-borrowed-rejected` gains a companion case showing
destroy *succeeding* after the borrow's block ends.

### A-02 · No temporary lifetime: argument-position borrows suspend the source until the enclosing block ends
**Files:** `spec/08-alias-validity.md` §2 (`[Borrow]`, `[Borrow-
Reactivate]`), `spec/15-function-semantics.md` §2–3.
**Verified:** `[Borrow]` suspends the source whenever the pair would
conflict; `[Borrow-Reactivate]` is invoked only by `Block-Exit`. The
word "temporary" appears nowhere in `spec/08`, `13`, `14`, `15`.
**Consequence:** `push(&mut nums, 10); push(&mut nums, 20);` — the
second `&mut nums` finds `nums`'s path suspended → `temporally-valid`
fails → rejected. Every sequence of two exclusive-borrow calls on the
same object in one block is ill-formed.
**Acceptance:** a defined temporary scope for a borrow formed directly
as a call argument (or other expression-level position), ending when
the enclosing full expression/statement completes; `[Borrow-Reactivate]`
fires at that point as well as at `Block-Exit`; a conformance case with
two consecutive `push(&mut v, …)` calls is well-formed.

### A-03 · `[Field-Access]` bypasses `Borrow`, so the "`inv.alias-validity` by construction" claim is false
**Files:** `spec/16-aggregates.md` §1 (`[Field-Access]`),
`spec/08-alias-validity.md` §2 (the claim), `spec/03-invariants.md`
(`inv.alias-validity`'s pairwise proposition).
**Verified:** `spec/16` §1 states field access "does **not** go through
`Borrow` — it inherits `a0`'s mode" and does not suspend `a0`. A
projection from an exclusive base yields two live exclusive paths on the
same `o`; the invariant as written (`∀ a1,a2 … ⇒ permitted`) has no
projection exception, and `permitted(exclusive, exclusive) = false`.
**Acceptance:** either (a) `inv.alias-validity`'s proposition is
revised to treat a projection and its base as one access (with the
formal condition stated — e.g. paths related by a base/projection chain
are excluded from the pairwise quantification, and conflicts between
*disjoint* projections are defined), or (b) `[Field-Access]` goes
through the same suspend/reactivate discipline as `[Borrow]`. Whichever
is chosen, `spec/08` §2's "by construction" paragraph is rewritten to
name every access-path-introducing rule, not just `Borrow`.

### A-04 · `[Struct-Construct]` never registers destroy-authority or an obligation for a declared-resource struct
**Files:** `spec/16-aggregates.md` §1 (`[Struct-Construct]`),
`spec/05-value-object-semantics.md` (`[Object-Establish-Resource]`),
`spec/21-standard-library-semantics.md` (`Vec`, `Rc`, `Mutex`).
**Verified:** `[Struct-Construct]` creates `objects(o)` directly and
performs `Field-Transfer-In` only for resource-bearing *fields*. Nothing
invokes `[Object-Establish-Resource]` for a struct whose `is-resource =
true` is *declared* with no resource fields — which is every library
resource type (`Vec<T>`, `Rc<T>`, `Mutex<T>`).
**Consequence:** such a value has no `authority(p, destroy, o)` and no
obligation: `drop(v)` fails with `diag.no-destroy-authority`, and
`Block-Exit` never destroys it (leak).
**Acceptance:** `[Struct-Construct]`/`[Array-Construct]` (and enum
construction) are defined as *specializations* of
`[Object-Establish-Resource]`/`-Plain` — one establishment rule family,
with the resource/plain split decided by `is-resource(τ)` exactly as in
`spec/05` — so a declared-resource struct receives authority and an
obligation at construction. `conf.leak-free-by-construction` is
re-verified against the corrected rule.

### A-05 · `[Block-Exit]` sweeps by *origin frame*, so moved and returned resources are never destroyed
**Files:** `spec/14-control-flow.md` §1 (`O = { o … origin was
established in frame f }`), `spec/07-resource-authority.md`
(`[Authority-Transfer]`), `spec/15-function-semantics.md` §4
(`[Return]`), D-0008.
**Verified:** the sweep criterion is `Σ.objects(o).origin was
established in frame f`; `inv.origin-stability` forbids changing origin;
`transfer` does not touch it.
**Consequence:** a `Vec` returned from a function was established in the
callee's (now popped) frame; the caller's `Block-Exit` never includes it.
Same for `let w = v` across blocks. D-0008's "leak-freedom is
structurally impossible" is false for exactly the cases ownership
transfer exists to support.
**Acceptance:** the sweep set is keyed on the *current holder of
destroy-authority* (which frame's binding currently holds
`authority(p, destroy, o)`), not on origin; `[Return]`'s "becomes the
caller's obligation" is stated as an actual `Σ` update; a conformance
case returns a `Vec` from a function and verifies it is destroyed at the
caller's block exit.

### A-06 · `let x = e;` has no semantic rule
**Files:** `spec/05-value-object-semantics.md` (`[Binding-Form]` binds a
name to an *existing* object), `spec/13`, `spec/14` (no `let` rule),
`spec/22` §2 (`let-decl` grammar only).
**Verified:** grep for a `let` rule across `spec/05`, `13`, `14` finds
none.
**Consequence:** unspecified whether initialization is establish-then-
`Write` (which bit-copies a resource — the very bug `[Read-Resource-
Rejected]` exists to prevent) or `transfer`. Also, `let-decl` requires
`'=' expr`, so the uninitialized `let x: i32;` used by definite-
assignment examples is ungrammatical (see A-27).
**Acceptance:** a `[Let]` rule: fresh object establishment
(`[Object-Establish-*]`), then initialization via `transfer` when
`is-resource(τ)` (invalidating the source, as for by-value parameters)
and via `Write` of the read value otherwise; a `let x: τ;` form (no
initializer) is either added to the grammar with `init := uninitialized`
or removed from every example/conformance case.

## 3. Major — master-instructions compliance

### A-07 · §12 violated: static-vs-dynamic disposition is implementation-defined
**Files:** pervasive — `spec/07` (`[Authority-Transfer]`, `[Destroy]`),
`spec/08` (`[Borrow]`), `spec/10`, `spec/11`, `spec/16`
(`[Index-Out-Of-Bounds]`), others.
**Verified:** the recurring pattern "`discharge: static` (discharge:
dynamic where flow analysis cannot resolve it)" with no definition of
*which* programs an analysis must resolve.
**Why it matters:** §12: "Whether a claimed CobaltC guarantee applies
must be language-defined, not implementation-defined." §20: two
conforming implementations would reject different programs at compile
time.
**Acceptance:** for each such rule, a language-defined boundary: name
the exact static analysis (e.g. "lexical containment as defined in
`spec/14` §2"; "definite assignment over the syntactic CFG of `spec/14`
§3") and state that everything that analysis does not prove is
*dynamic*, never rejected — so the set of statically rejected programs
is the same for every implementation.

### A-08 · §12: the "valid safe CobaltC program" category is never defined
**Verified:** no term, no definition anywhere (`spec/00`, `spec/20`).
**Acceptance:** a `term.safe-program` entry (presumably: a well-formed
program containing no `unsafe` block and no `extern` call), and the
§12 guarantee restated normatively against it.

### A-09 · §13: key rules are prose, and CFN was not extended for constructs it now needs
**Files:** `spec/15` §4 (`[Return]`), `spec/14` §5 (`[Break]`,
`[Continue]`), `spec/18` §1 (`[Fault-Unwind]`, `↛↛`), `spec/10` §3
(`[Call-Elided-Lifetime]`'s conclusion), `spec/14` §1 (`Block-Exit`'s
fold), `spec/19` (`[Thread-Step]` "interleaves freely").
**Verified:** premises/conclusions such as "F = frames from current block
up to …, innermost first" and "`result`'s escape-checking is exactly
`borrow(a_k, m')`"; `↛↛` is used but absent from `spec/01` §2; `()` is
used but is not a defined type (see A-13).
**Acceptance:** each listed rule restated over `Σ` (frame-stack,
access-paths, obligations) with no prose in premise or conclusion; CFN
§2 gains a program-termination judgment; `spec/01` version bumped.

### A-10 · §13: the `outcome:` taxonomy is never used
**Verified:** zero rules carry `outcome: impl-defined` or `outcome:
unspecified`. Thread interleaving (`spec/19` §1), struct layout
(`spec/16` §1 "no requirement yet forces one"), and `AddrWidth` are
exactly the permitted nondeterminism §13 requires to be expressed.
**Acceptance:** those three (at minimum) carry `outcome:` tags with an
enumerated or documented set; `spec/01` §5's taxonomy is cross-checked
against every place the spec says "unspecified", "implementation-
defined", or "freely".

### A-11 · §20: byte-level representation, `sizeof`, and layout are undefined — fatal for the §5 low-level story
**Files:** `spec/05` (`represent(v)`, `value-at(…)`), `spec/20` §2
(`sizeof(τ)` in `[Rawptr-Deref]`/`[Reclaim]`), `spec/16` §1
(`layout(struct)`), `spec/21` (`deallocate(v.ptr, v.cap)` with mismatched
pointer types).
**Verified:** none of `represent`, `value-at`, `sizeof` is defined
anywhere.
**Why it matters:** `reclaim(p, τ)` reinterprets raw bytes as `τ`;
`extern fn` passes typed values; two implementations would disagree on
every such operation.
**Acceptance:** a representation artifact (or section of `spec/06`/
`spec/16`) defining `sizeof` and byte layout for every scalar, `ref`,
`rawptr`, struct, array, and enum — or, for each, an explicit
`outcome: impl-defined { … }` with the documented degrees of freedom and
a rule that `Reclaim`/FFI are only well-defined when the two sides agree
on that documented choice.

### A-12 · §11: major mechanisms without derivation records
**Verified:** no `D-XXXX` for `unsafe` scoping, `rawptr`, `Reclaim`,
`spawn`/`join`, `Mutex`/`Guard`, `type.ref` itself, modules, or closure
capture-mode inference. `spec/20` and `spec/17` say the records were
skipped "to preserve pace."
**Acceptance:** derivation records (§11 fields) for each, or an explicit
`D-XXXX` stating which of these are *minor* and why they fall below
§11's "major" threshold.

### A-13 · §20: the unit type, unary minus, and four comparison operators are undefined
**Verified:** `()` appears in `[If-False]`/`[While]`/`push` but no
`type.unit`; `unop` is in the grammar but `-e` has no rule (examples use
`-1: i32`, which is also not a literal per `int-literal ::= digit+`);
only `<` and `==` have rules, though `<= > >= !=` are in the precedence
table.
**Acceptance:** `type.unit` with `is-resource = false`; `[Neg]` for
signed types (with the `min(τ)` overflow case, `disposition: checked`);
the remaining comparisons defined by reduction to `<`/`==` or given
rules.

### A-14 · §15/§18/§21: the lifecycle machinery has never been exercised
**Verified:**
- 9 of 11 `spec/03` entries still say "Enforcement strategy: deferred
  to X" where X now exists; `inv.concurrency-validity`'s preservation/
  transformation/weakening/invalidation fields still say "deferred".
- No `CHG-XXXX` record exists; `spec/changes/` does not exist.
- Rules (`rule.*`) and types (`type.*`) carry **no Status field**,
  though `spec/02` §1 requires one on every entity.
- Nearly every entity is `PROVISIONAL`. By §18 ("only `ACCEPTED`
  features belong to the intended language design") almost none of the
  language is officially in the language.
**Acceptance:** every `spec/03` entry back-filled from the artifact
that resolved it; every rule/type given a Status; a promotion pass that
moves entities to `ACCEPTED` (with the review §00's own conventions
promised) or records why not; the first `CHG` record written for the
first post-promotion normative change.

### A-15 · §21: the dependency graph is not mechanically extractable
**Verified:** `Depends on:` values mix entity ids (`inv.…`, `D-0008`),
file paths (`spec/16` §1), and section refs (`§1 (\`Block-Exit\`)`).
**Acceptance:** `spec/02` §4 tightened to "entity ids only"; every
`Depends on:`/`Affects:` line normalized; a one-line grep demonstrating
extraction added to `spec/02` §4 as the conformance check for the
convention itself.

### A-16 · §19: diagnostic entries omit required fields
**File:** `spec/registry/diagnostics.md`.
**Verified:** "provenance of conflicting facts" is present in ~4 of 23
entries; "relevant source locations" was collapsed to one blanket note.
**Acceptance:** every entry carries all §19 fields explicitly.

## 4. Systematic — examples and conformance contradict the rules

### A-17 · Bare `let v = Vec::new();` — 18 occurrences, all ill-formed per D-0013
**Files:** `spec/examples.md` (12), `spec/conformance.md` (6, including
`conf.leak-free-by-construction`, `conf.double-destroy-rejected`,
`conf.transfer-invalidates-source`).
**Verified:** `conf.generic-call-uninferable-rejected` in the same file
states this exact form is rejected.

### A-18 · `let mut` — 8 occurrences; `spec/05` states it does not exist
**Files:** `spec/examples.md` (5), `spec/conformance.md` (3).

### A-19 · Payload-less enum variants are constructed/matched with payloads
**Files:** `spec/examples.md` (`ex.exhaustive-match`: `enum Sign { Pos,
Neg, Zero }` matched as `Pos(_)`), `spec/16` §3 (`[Enum-Construct]`
requires `v : τi`), `spec/22` §2 (construction grammar demands
`'(' expr ')'`; `enum-decl` makes the payload optional).
**Acceptance:** payload-less variants defined end to end (construction
without parens, patterns without a binder), or removed from the
declaration grammar.

### A-20 · `Vec::new`, `Rc::new`, `Rc::clone` use `::` on a type; no such mechanism exists
**Files:** `spec/21` throughout, `spec/17` (only module paths),
`spec/22` §3 (`path ::= identifier ('::' identifier)*` resolves against
modules).
**Acceptance:** either an associated-function/namespace-on-type rule, or
the library rewritten as free functions (`vec_new<T>()`), consistently.

### A-21 · `Vec`'s `index(v: ref<Vec<T>,m>) -> ref<T,m>` needs mode polymorphism the generics system cannot express
**Files:** `spec/21` §2, `spec/12` §1 (type parameters range over
`Type`; `Mode` is not a `Type`).
**Acceptance:** either two functions (`index_shared`/`index_exclusive`),
or a mode-polymorphism decision record.

### A-22 · Closures cannot be called
**Files:** `spec/15` §6 (`[Closure-Call]`), `spec/15` §2 (`[Call]`
requires `v_f : type.fn<…>`).
**Verified:** a closure's anonymous struct type is not a `type.fn`;
nothing types a closure in call position or lets a `type.fn` parameter
accept one. Capture mode ("exclusive if body writes through `xi`") names
an analysis that is never defined.
**Acceptance:** a callable-type judgment covering both, and the capture-
mode analysis stated (or captures made explicit in the literal).

### A-23 · `Vec::new`'s body is undefined
**File:** `spec/21` §2 — `allocate-zero-capacity-marker` is not a
defined operation.

### A-24 · `Spawn`/`Join` treat `h` as both a value and a resource id without establishing an object
**File:** `spec/19` §1 — `authority(current-performer, join, h)` keyed
on a value; no `Object-Establish-Resource` for the handle.
**Acceptance:** the handle established as an object like any other
resource (this is the same defect class as A-04).

### A-25 · `Ref-Form` lists no `disposition` though `Borrow` can fail
**File:** `spec/09` §2 — `&_m a0` delegates to `[Borrow]`, which has a
`[Borrow-Denied]` case; the delegating rule should say so.

### A-26 · Examples use `-1: i32` (see A-13) and `x = 1: i32` inside `if` blocks written as `{x = 1: i32;}` — fine — but `read(x)` appears as surface syntax though `read` is not a surface operation (`spec/22`)
**Files:** `spec/examples.md` (`ex.definite-assignment`),
`spec/conformance.md` §4.

### A-27 · `let x: i32;` is ungrammatical (see A-06)
**Files:** `spec/examples.md` (2), `spec/conformance.md` (2).

### A-28 · `conf.generic-struct-monomorphize-resource` and `ex.generic-box` depend on D-0014, which was added after them — verify they now hold end to end once A-04 is fixed (a `Box{value: Vec::new()}` also needs the inner `Vec` to receive authority).

### A-29 · `spec/13` §1a's access-path-preserving list omits `match` scrutinee-by-reference and `spawn` arguments
Minor, but the list is normative ("every other position is value
position"), so an omission changes meaning.

### A-30 · Regenerate, don't patch
Once A-01–A-06 are closed, regenerate `spec/examples.md` and
`spec/conformance.md` from the corrected rules. Every fragment must be
grammatical per `spec/22` and must trace to a rule that exists. Add the
conformance cases named in the Acceptance criteria above.

### A-31 · Rewrite `spec/22` §5
The closing audit claims one narrow open item. After this audit it must
list the findings here that remain open, and must stop describing
`inv.alias-validity` and leak-freedom as holding "by construction"
until A-03/A-05 are closed.

## 5. What is sound and should be preserved

- The overall architecture: terminology → CFN → schema → invariants →
  abstract state → mechanisms → syntax, with every rule tagged by
  disposition. Keep it.
- D-0001 through D-0014 as *records*: real alternatives, real
  rationale. Extend by supersession, do not rewrite.
- `spec/04` §3's two by-construction results (`inv.identity`,
  `inv.origin-stability`) — correct as stated.
- The trust-boundary model (`spec/20` §3, `spec/21` §3): external data
  as a claim until validated is exactly §17's requirement, and the reuse
  for UTF-8 validity is a good sign the pattern generalizes.
- The honesty of the change logs. Keep recording what was found and
  why, including this audit's findings as they close.

## 6. Suggested order of work

1. **A-01, A-02, A-03** together — they are one problem: access-path
   lifetime. Decide temporary scope, scope-exit invalidation, and the
   projection/base relationship in a single new decision record
   (`D-0015`), then update `spec/08`, `spec/14`, `spec/16`, `spec/03`.
2. **A-04, A-05, A-24** together — one establishment rule family, sweep
   keyed on current holder (`D-0016`).
3. **A-06, A-13** — `let`, `()`, unary minus, comparisons.
4. **A-07** — the language-defined static boundary (this is a
   constitutional-priority item: §12 is among the highest-ranked
   requirements).
5. **A-11, A-10, A-09** — representation, `outcome:` tags, prose rules.
6. **A-19 → A-23, A-20, A-21, A-22** — make `spec/21` expressible in the
   language it claims to demonstrate.
7. **A-30, A-31** — regenerate the suites, rewrite the closing audit.
8. **A-14, A-15, A-16, A-12, A-08** — lifecycle, graph, diagnostics,
   missing derivation records, `term.safe-program`.

Closing all of §2 and A-07 is the bar for "an independent implementer
can determine the semantics mechanically" (Master Instructions §26).
Everything else in this list is required for the specification to be
consistent with its own schema and its own claims.
