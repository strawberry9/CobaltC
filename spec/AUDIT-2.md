# CobaltC Specification Audit 2

Status: supporting artifact (Master Instructions §24 — a work list that
resolves a current design problem: the specification's rules, as written,
do not derive the specification's own examples, and one closure of the
first audit introduced a soundness hole)
Audit date: 2026-09-20
Scope: every file under `spec/` after the closure pass recorded in
`spec/AUDIT-STATUS.md`, checked rule-by-rule against
`CobaltC_Master_Instructions.md` and against each other
Method: every finding was verified by locating the rule text and, where
a program is named, by attempting to derive that program step by step
under the rules as written. Findings marked **(regression)** were
introduced by the closure of `spec/AUDIT.md`; the rest predate it.

Severity uses `spec/AUDIT.md`'s scale: **Critical** = ordinary programs,
including this specification's own, are unsound or underivable;
**Major** = a master-instructions requirement is not met; **Minor** =
precision or hygiene.

## 1. Critical

### B-01 · Projections bypass the alias check; `inv.alias-validity` is violated **(regression)**
`[Field-Access]` (`spec/16` §1) forms an access path with no conflict
scan, and D-0015 exempts base/projection pairs from the invariant. After
`let r = &mut p.a;`, the statement `p.a = 5;` forms a fresh projection
from `p`'s still-valid binding and writes through it while `r` is live.
Before D-0015 this was over-conservatively rejected; now it is admitted.
**Acceptance:** every read and write is checked, at the access, against
every live non-ancestor access path with an overlapping target; a
conformance case shows `p.a = 5` rejected while `&mut p.a` is held.

### B-02 · Any borrow of a variable makes the variable unusable
Every binding is `exclusive` (`spec/05` `[Binding-Form]`) and `[Borrow]`
suspends its source unless both sides are shared, so after `let r = &v;`
every later use of `v` is `diag.stale-binding`. This contradicts
D-0004's own constraint ("passing read-only data to multiple
simultaneous readers") and the expected outcomes of
`conf.two-shared-borrows-ok`, `conf.shared-then-exclusive-rejected`,
`conf.destroy-while-borrowed-rejected`, `ex.borrow-conflict`,
`ex.destroy-while-borrowed`, `ex.bounds-check`.
**Acceptance:** a shared borrow leaves its source readable; an exclusive
borrow leaves its source unusable only until the borrow ends; the named
cases derive.

### B-03 · Reborrows are exempt from scope-exit invalidation **(regression)**
`[Block-Exit]` and `[Stmt-Exit]` (`spec/14` §1, §1a) subtract the
reactivated set `S` from `Stale`, so a reborrow stays valid after its
scope. `push(&mut v, 1); drop(v);` then fails with
`diag.destroy-while-aliased`.
**Acceptance:** a temporary borrow is invalid after the statement that
formed it; the fragment above derives as well-formed.

### B-04 · Only `[Borrow]` stamps `frame`/`temp-scope`/`base` **(regression)**
`[Binding-Form]`, `[Field-Access]`, `[Index-Checked]`, `[Reclaim]`,
`[Lock]` never set the fields D-0015/D-0016 rely on, so `[Block-Exit]`'s
sweep set is undefined for every binding and `related` never holds for a
projection.
**Acceptance:** every access-path-forming rule sets every field of
`AccessPathRecord`; no rule reads a field another rule may leave unset.

### B-05 · `[Read]`/`[Write]` act on the whole object, not the access path's target
`spec/05`: `[Read]` returns `value-at(storage, extent(o))`; `[Write]`
writes `extent(o)`, requires `v : type-of(o)`, and sets `init(o) :=
valid`. A field read yields the whole struct, a field write is ill-typed
and marks the whole object initialized. Every `v.len += 1` in `spec/21`
is underivable.
**Acceptance:** access paths carry the type of what they target; reads
and writes use `target(a)`; partial initialization is either modeled or
explicitly rejected.

### B-06 · Returning a resource does not work
`[Return]` never updates `state.holder`; trailing-expression returns
have no rule; `[Call]` sweeps the body frame so `fn make_vec() { let v
= ...; v }` destroys `v` before returning; `[Call]` and `[Return]` both
exit frame `f`. `ex.returned-resource`,
`conf.returned-resource-destroyed-at-caller-exit` do not derive.
**Acceptance:** one result-passing mechanism covers `return e` and a
trailing expression; a returned object's holder is the caller's binding
or, if unbound, it is destroyed at the end of the caller's statement.

### B-07 · Aggregate construction and `let` do not compose
`[Struct-Construct]` yields an identity `o`; `[Let-Value]` establishes a
second object and writes `o` into it; `[Let-Resource]` demands an access
path. `let v: Vec<i32> = Vec::new();` has no derivation.
**Acceptance:** expression results are classified (value, place,
temporary object) and one `store` operation, shared by `let`, parameter
binding, field initialization, and return, is defined for each class.

### B-08 · Closures cannot be called after their defining statement **(regression)**
Borrow-captured fields are aggregate-literal borrows, which D-0015
invalidates at statement exit. `ex.closure-capture`,
`conf.closure-borrow-capture` fail. The A-32 note that no example needs
this is false.
**Acceptance:** a reference stored in any object lives as long as that
object; closures derive.

## 2. Major

### B-09 · `while` nests a frame per iteration
`spec/14` §4 unfolds `while c {e}` into `if c {e ; while c {e}}`, and
`if` wraps its branch in a block, so every iteration's frame stays open
until the loop ends.
**Acceptance:** loop iterations exit their frame before the next begins.

### B-10 · The static/dynamic boundary is still not language-defined
`stat.flow-analysis` (`spec/14` §6) names an analysis without an
abstract domain, join, or loop treatment, and no rule states whether a
provable violation is a static rejection or a runtime fault.
**Acceptance:** the analysis is defined as a lattice-valued forward
analysis with stated transfer functions and join; each guarded rule
states its static outcome for `proven`, `refuted`, and `unknown`.

### B-11 · Storage is never allocated; `represent`/`value-at` undefined
Every `establish-object` requires `addrs ⊆ dom(Σ.storage)`, but no
rule extends `Σ.storage`; `[Struct-Construct]` passes layout offsets as
addresses. Representation of values in storage is undefined.
**Acceptance:** establishment allocates fresh aligned storage; object
end releases it; `represent_τ`/`value-at_τ` are defined for every type
with the implementation-chosen degrees of freedom tagged `outcome`.

### B-12 · `Performer` is undefined
`current-performer` keys every authority token, but `Performer` is
opaque. **Acceptance:** `Performer ≝ Thread`, stated in `spec/04`.

### B-13 · One global frame stack under threads
`state.frame-stack`/`state.temp-scope-stack` are single lists;
`[Spawn]` pushes frames in another thread. **Acceptance:** both are
per-thread.

### B-14 · `diag.no-destroy-authority` is unreachable through a binding
`[Object-End]` invalidates the binding, so a second `drop(v)` yields
`diag.stale-binding`. `ex.double-destroy`, `conf.double-destroy-
rejected`, `conf.transfer-invalidates-source` cite the wrong diagnostic.
**Acceptance:** the suite states the diagnostic the rules produce.

### B-15 · Exclusive access can be derived from a shared reference
`[Borrow]` never requires `m ≤ mode(a0)`. **Acceptance:** an exclusive
borrow requires an exclusive-capable source, with its own diagnostic.

### B-16 · Reference identity ignores the target
`[Ref-Identity-Eq]` compares `.of` only, so `&p.a ===_ref &p.b`.
**Acceptance:** identity compares object and target.

### B-17 · Prose rules remain (§13)
`[Bind-Param-Value]`, `[Closure-Call]`, `[Destroy-Composite]`,
`[Match]`, `[Enum-Destroy]`, `[Field-Transfer-In]`'s holder update.
**Acceptance:** each restated over `Σ`.

### B-18 · No program entry point or termination outcome is defined
Nothing says where execution starts, what `Σ_0` is, or what a
conforming implementation's observable outcome is.
**Acceptance:** `main`, `Σ_0`, and the observable outcome of
`terminate(d, Σ)` defined.

### B-19 · Schema debt (carried from A-14/A-15)
No rule or type carries a Status; 20 dependency lines fail the schema's
grep; no promotion pass. **Acceptance:** every rule/type entity has an
id header and Status; the grep prints only ids; a promotion pass moves
the design to `ACCEPTED`.

### B-20 · The surface grammar is not sufficient for an implementer
No lexical rules for whitespace/comments; `drop`, `propagate`,
`wrapping_add`, `narrow`, `widen`, `Option`, `AllocError`, `Utf8Error`
are used but never declared; no `main`.
**Acceptance:** `spec/22` is complete for every construct with
semantics; library items are declared in `spec/21`.

## 3. Minor

- B-21 Stale text: `rule.resauth.leak` (origin-keyed), `spec/12` §3
  ("no type is a resource"), `spec/15` §1 (closures under
  investigation), `spec/22` §3 ("never inferred"), `spec/12` §5 grammar.
- B-22 `>>` has no rule; `if` without `else` has no rule.
- B-23 D-0015 contradicts itself (projections carry `base` vs.
  `[Field-Access]` unchanged).
- B-24 `[Object-Establish-Resource]` tagged `checked` with only static
  side-conditions.
- B-25 `spec/06` §1 says `AddrWidth` is not an `outcome` instance while
  §7 tags it as one.
- B-26 Every decision converges on the same design as one well-known
  prior-art language; candidate sets in D-0006–D-0009 are thin. Recorded
  for the human owner; not treated as a defect by this audit.

## 4. Closure-log accuracy

Of the 29 findings `spec/AUDIT-STATUS.md` marked CLOSED, 17 are closed
and 12 are overstated: A-01 through A-07, A-09, A-11, A-22, A-28, A-30.
The pattern: decision records and prose were written, the rules those
records name were not all updated, and the regenerated suite was not
derived against the rules.

## 5. Order of work

1. B-01, B-02, B-03, B-04, B-08, B-15, B-16 together — one problem,
   access-path lifetime and alias checking: new decision D-0018,
   superseding D-0015.
2. B-05, B-06, B-07, B-11, B-12, B-13 together — evaluation results,
   storage, and the store operation: new decision D-0019.
3. B-09, B-18, B-22 — loops as primitives, program entry.
4. B-10 — `stat.flow-analysis` defined.
5. B-17, B-19, B-20, B-21, B-23, B-24, B-25 — restatement, schema,
   grammar, hygiene.
6. Regenerate `spec/examples.md`/`spec/conformance.md` **with a written
   derivation per case**, and record closure in `spec/AUDIT-STATUS.md`.
