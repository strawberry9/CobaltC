# D-0017 — Retroactive Derivation Records for Undocumented Major Mechanisms

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §11
Depends on: D-0003, D-0004, D-0008

Realizes the fix for `spec/AUDIT.md` A-12.

## Problem

Master Instructions §11 requires a derivation record for every major
semantic mechanism. Seven were introduced without one: `unsafe` scoping,
`rawptr`, `Reclaim`, `spawn`/`join`, `Mutex`/`Guard`, `type.ref` itself,
modules, and closure capture-mode inference. `spec/20-trust-
boundaries.md` and `spec/17-modules.md` acknowledge the records were
skipped "to preserve pace." This decision supplies the missing
derivation for each, in compact form (problem, candidates, selection,
rejected alternatives, invariant traceability — the substance Master
Instructions §11 requires; usability/explainability/feasibility
implications are recorded only where they add information beyond what
each mechanism's home artifact already states, to avoid restating
material already normative there per Master Instructions §14).

## `unsafe` scoping (`spec/20-trust-boundaries.md` §1)

**Semantic problem:** where in a program `discharge: trusted` may
appear at all — without a bound, every `trusted-unchecked` disposition
(§12) is an unaudited, unlocatable escape from every invariant it
touches. **Candidates:** (1) unrestricted — any expression may assert
`trusted`; rejected, defeats §19's audit/explainability goal outright.
(2) function-level — a whole function marked `unsafe`; rejected, makes
the asserted region larger than necessary, diluting what is actually
being manually verified. (3) block-scoped `unsafe { }`, an ordinary
block with the one restriction lifted. **Selected: (3)** — the smallest
unit already available (`spec/14-control-flow.md`'s block) that can
bound the assertion to exactly the statements it covers. **Rejected:**
(1), (2). **Invariant traceability:** governs every invariant a
`trusted-unchecked` rule touches (spatial, initialization, alias,
resource-authority) by bounding where its unchecked discharge may occur,
not by weakening any of them.

## `rawptr` (`spec/20-trust-boundaries.md` §2)

**Semantic problem:** §5's low-level story requires bare-address
manipulation (raw storage interaction, hardware-facing programming)
that `type.ref<τ,m>`'s checked discipline cannot express without
weakening it for every caller. **Candidates:** (1) extend `type.ref`
with an "unchecked mode"; rejected — this is exactly the single-
mechanism-weakened-to-cover-both-needs pattern Master Instructions §5
rules out ("coexist through explicitly derived mechanisms"). (2) a
wholly separate type, `type.rawptr<τ>`, carrying no `Σ.access-paths`
entry, no mode, no checks, freely copyable. **Selected: (2)** — keeps
the checked mechanism's guarantees absolute (never weakened by a rawptr
in the same type family) while still providing the escape. **Rejected:**
(1). **Invariant traceability:** deliberately opts a `rawptr` *out* of
`inv.spatial-validity`/`inv.alias-validity`/`inv.initialization-
validity`'s automatic enforcement; the `unsafe` mechanism above is what
re-admits accountability for that opt-out.

## `Reclaim` (`spec/20-trust-boundaries.md` §2)

**Semantic problem:** raw-storage-backed types (`Vec<T>`) need to bring
a region of raw storage back into checked `Σ.objects`/`Σ.authority`
bookkeeping to be destroyed through the ordinary mechanism, rather than
inventing a parallel, rawptr-specific destruction path. **Candidates:**
(1) a rawptr-specific `destroy_raw` operation, bypassing checked
bookkeeping entirely; rejected — duplicates `rule.resauth.destroy`'s
logic for no reason, against Master Instructions §9. (2) `Reclaim`:
promote raw storage to a fresh checked object (`trusted-unchecked` on
the promotion's own validity claim), then use the ordinary destroy path
unchanged. **Selected: (2)**. **Rejected:** (1). **Invariant
traceability:** the promotion step is exactly `inv.trust-transition`'s
"external representation → unchecked claim → validation/proof/explicit
trust → established semantic value" diagram, instantiated for raw
storage rather than FFI/network data — the same pattern
`inv.string.utf8-validity` (`spec/21`) independently confirms
generalizes.

## `spawn`/`join` (`spec/19-concurrency.md` §1)

**Semantic problem:** thread creation and result-retrieval need
authority bookkeeping (a handle's "destroy" is joining it) without a
new resource-kind-specific mechanism. **Candidates:** (1) a dedicated
thread-authority relation, independent of `state.authority`; rejected —
duplicates D-0003's whole apparatus for one resource kind. (2) reuse
D-0003's single-holder authority mechanism directly, treating the join
handle as an ordinary resource (per D-0016's correction, `spec/AUDIT.md`
A-24). **Selected: (2)**. **Rejected:** (1). **Invariant
traceability:** `inv.resource-authority` (the handle), transitively
`inv.concurrency-validity` (its interaction with `synchronized(...)`,
`spec/08` §1's baseline).

## `Mutex`/`Guard` (`spec/19-concurrency.md` §2)

**Semantic problem:** `spec/08`'s static shared/exclusive discipline is
too restrictive for state genuinely mutated from more than one thread at
different, unpredictable times — a legitimate pattern D-0004 explicitly
left open pending Concurrency. **Candidates:** (1) a new, independent
synchronization primitive with its own destruction discipline; rejected
— would need to redefine RAII-style scope-end release from scratch. (2)
`Mutex<T>`/`Guard<T>` as ordinary resource types (`is-resource = true`),
reusing D-0008's automatic scope-end destruction for lock release and
D-0004's mode vocabulary for the guard's own access path. **Selected:
(2)**. **Rejected:** (1). **Invariant traceability:** extends
`synchronized(...)` (`inv.concurrency-validity`) beyond D-0004's
unsynchronized baseline for exactly the mutex-guarded case, per
`spec/19` §2's own text.

## `type.ref` itself (`spec/09-identity-origin-extent.md` §1)

**Semantic problem:** up to `spec/08`, identity/origin/extent existed
only as internal `Σ` bookkeeping — no CobaltC program could hold, pass,
compare, or store such a fact as a value, which §17 (Identity, Origin,
Spatial Validity) requires for ordinary programs (e.g. a function
returning access to a field). **Candidates:** (1) no value-level
encoding — every access stays purely syntactic (a bare access path,
never a storable value); rejected, makes references-as-values
(returned, stored in a struct, compared) impossible, failing
composability (§8) for a routine pattern. (2) `type.ref<τ,m>`: an
access-path token reinterpreted as a value, formed by `Borrow` (no new
check), dereferenced by delegation to `Read`/`Write` (no new check).
**Selected: (2)** — the minimum value-level encoding, deriving no new
safety mechanism, only a representation for one already-checked fact.
**Rejected:** (1). **Invariant traceability:** `inv.alias-validity`
(formation delegates entirely to `Borrow`), `inv.identity`
(`===_ref`, §3).

## Modules (`spec/17-modules.md`)

**Semantic problem:** program organization (naming, visibility) without
introducing a new semantic invariant — `spec/17`'s own Purpose section
already argues, correctly, that no invariant in the registry names a
module-level concern. **Candidates:** (1) a capability/authority-scoped
visibility mechanism (visibility as a form of authority, reusing
`state.authority`); rejected — no invariant motivates it, and it would
duplicate ordinary declaration-site visibility for no identified
requirement (Master Instructions §9). (2) purely static, structural
visibility (`pub` vs. private, checked against declaration site) and
qualified-name resolution as a `state.bindings` extension. **Selected:
(2)** — the minimum needed for name resolution and visibility, nothing
more, matching `spec/17`'s explicit "no deeper invariant surface to
over-derive" framing. **Rejected:** (1). **Invariant traceability:**
none directly (by design — recorded, not assumed, per `spec/17`'s own
Purpose section, itself a form of derivation this record makes
official).

## Closure capture-mode inference (`spec/15-function-semantics.md` §6)

**Semantic problem:** a borrow-capturing closure's per-variable mode
(`shared` vs. `exclusive`) must be decided by *something* for
`rule.alias.borrow` to form the capturing field's reference correctly.
**Candidates:** (1) require the programmer to annotate each captured
variable's mode explicitly in the closure literal; rejected — adds
syntax cost (§10) for a fact almost always mechanically derivable from
the body, and no example in this specification needs the annotation
escape hatch. (2) a static, syntactic scan of the closure body: `mi =
exclusive` iff the body contains a `Write` whose target's root access
path is the captured variable, `shared` otherwise (`spec/15` §6, added
alongside D-0016/D-0015's `related`/`root` machinery, resolving
`spec/AUDIT.md` A-22's "analysis... never defined" finding).
**Selected: (2)**, decidable from the body's own AST, the same class of
fact `stat.flow-analysis` (`spec/14-control-flow.md` §6) covers
generally. **Rejected:** (1). **Invariant traceability:**
`inv.alias-validity` (the resulting mode gates `Borrow`'s own check,
unchanged), `inv.resource-authority` (a move-captured resource field's
obligation, `spec/16` §1/§4, unaffected by this choice).

## Compatibility impact

Purely additive documentation; no rule's normative behavior changes.
Each mechanism above remains exactly as defined in its home artifact.

## Revisit conditions

None beyond each mechanism's own home artifact's existing Revisit
conditions (D-0003, D-0004, D-0008, D-0011).
