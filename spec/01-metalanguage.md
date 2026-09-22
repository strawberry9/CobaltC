# CFN — CobaltC Formal Notation

Status: normative artifact (notation; ACCEPTED)
Version: 1.0.0
Selected by: `spec/decisions/D-0001-formal-semantic-metalanguage.md`
Governed by: `CobaltC_Master_Instructions.md` §13
Note: not an entity registry under `spec/02-schema.md` — it defines
notation, not Kind-bearing entities; entities in other artifacts write
rules using this notation.

## Purpose

CFN is the formal semantic metalanguage in which all normative CobaltC
rules (invariant registry, value/object semantics, arithmetic, type
system, etc.) must eventually be expressed, per Master Instructions §13.
This document defines CFN's notation. It defines *how rules are written*,
not any CobaltC-specific rule — no rule in this document is normative
about CobaltC itself.

CFN presupposes the terms defined in `spec/00-terminology.md` but does
not presuppose any mechanism (ownership, borrowing, GC, regions,
capabilities, or a specific concurrency model), per Master Instructions
§7.

## 1. Abstract machine state `Σ`

`Σ` denotes the abstract semantic state: a structured, componentized
value threaded through the runtime-semantics judgments. Its components
(bindings, objects, storage, regions, temporal-validity facts, authority,
access relationships, initialization state, synchronization state, trust
state — cf. Master Instructions §16) are **not** fixed by this document.
They are defined in the forthcoming Abstract Semantic State artifact, per
the design order in Master Instructions §23. Until then, `Σ` is treated
as an opaque structured value supporting:

- **Component projection**, written `Σ.C`, for a named component `C`
  (e.g., `Σ.Objects`, `Σ.Authority`);
- **Disjoint composition**, written `Σ1 ⊎ Σ2`, defined component-wise:
  a component `C` is *partitionable* if the Abstract Semantic State
  artifact declares it so (resource- or storage-like components are
  expected to be partitionable); for a partitionable component,
  `(Σ1 ⊎ Σ2).C = Σ1.C ⊎ Σ2.C` requires `Σ1.C` and `Σ2.C` to be disjoint;
  for a non-partitionable component (e.g., a shared static type
  environment), `(Σ1 ⊎ Σ2).C` requires `Σ1.C = Σ2.C` and that common
  value is shared, not summed. `Σ1 ⊎ Σ2` is undefined (the composition
  fails) if any partitionable component is non-disjoint.

Disjoint composition is what gives the separating conjunction (§4) its
meaning: `Σ = Σ1 ⊎ Σ2` states that `Σ` can be split into two
resource-disjoint parts.

## 2. Judgment forms

CFN has five base judgment forms. Every normative rule in every future
artifact must be an instance of one of these.

### 2.1 Static judgment

    Γ ⊢ J

`Γ` is a static context (a finite mapping from names to static
information such as types or module visibility — fully defined by the
type-system and module artifacts when written). `J` is a static
judgment, e.g. `e : τ` (typing) or a well-formedness judgment. States a
fact decidable from program text and `Γ` alone, without execution
(`term.static-semantics`).

### 2.2 State assertion

    Σ ⊨ P

States that abstract state `Σ` satisfies assertion `P` (§3). This is the
vehicle for invariant propositions (`term.invariant`) and for
preconditions/postconditions attached to rules.

### 2.3 Small-step transition

    ⟨c, Σ⟩ →^ℓ ⟨c', Σ'⟩

Construct `c` in state `Σ` steps to construct `c'` in state `Σ'`. The
optional label `ℓ` identifies the thread or event performing the step,
and is required whenever concurrency rules are in play; `ℓ` may be
omitted (written simply `→`) for single-threaded rules. This is the base
form of `term.runtime-semantics`.

**Derived big-step notation** (sugar, never primitive):

    ⟨c, Σ⟩ ⇓ ⟨v, Σ'⟩   ≝   ⟨c, Σ⟩ →* ⟨v, Σ'⟩
        where →* is the reflexive-transitive closure of →,
        and no step in the sequence is labeled with a thread
        other than the one evaluating c.

`⇓` may be used in examples and in rules where interleaving is
demonstrably irrelevant, but must always be justified by, and reducible
to, the underlying `→*` sequence.

### 2.4 Diagnostic (failure) judgment

    ⟨c, Σ⟩ ↛ D

Construct `c` in state `Σ` has no valid next step and instead produces
diagnostic `D` (`term.diagnostic`, identified per Master Instructions
§19). This is distinct from a construct having *no further steps because
it is already a final value* — reaching a value is success, not a
`↛` outcome. Every rule with a `checked` or `fallible` disposition (§5)
must define, or reference, the `↛` instance(s) it can produce.

### 2.5a Program-termination judgment

    ⟨program, Σ⟩ ↛↛ terminate(d, Σ')

The whole program halts in state `Σ'` following diagnostic `d` (or `d =
ok` for ordinary termination with no fault). Distinct from a single
expression's `↛` (§2.4): `↛↛` is a judgment about the entire program,
never a value any other judgment form produces or consumes as a
premise — no rule in this specification's normative artifacts may use
`↛↛` as a premise (this is the formal statement of D-0009's "no
catching," `spec/18-error-failure-semantics.md` §1). Added per
`spec/AUDIT.md` A-09: `spec/18` §1's `[Fault-Unwind]` used `↛↛` before
this document defined it.

### 2.5 Invariant lifecycle judgments

Named relations over `Σ`, used so that Invariant Registry entries
(future artifact, per §15) can cite lifecycle events by name instead of
restating them:

    establish(I, Σ) = Σ'      -- invariant I becomes true, yielding Σ'
    Σ ⊨ preserved(I)          -- I remains true unchanged in Σ
    invalidate(I, Σ) = Σ'     -- I becomes false (or ceases to apply), yielding Σ'

`I` ranges over invariant identities as they will be assigned in the
Invariant Registry (`term.invariant`). These judgments do not yet define
any concrete invariant; they define the *shape* an invariant's lifecycle
must be expressed in.

## 3. Assertion language

Grammar for `P` in `Σ ⊨ P` and for preconditions/postconditions/typing
predicates generally:

    P ::= true | false
        | e1 = e2 | e1 ≠ e2 | e1 < e2 | e1 ≤ e2 | e1 > e2 | e1 ≥ e2
        | e ∈ S | e ∉ S | S1 ⊆ S2
        | P1 ∧ P2 | P1 ∨ P2 | ¬P | P1 ⇒ P2 | P1 ⇔ P2
        | ∀x:D. P | ∃x:D. P
        | P1 ∗ P2
        | Σ.C(k)                      -- lookup of key k in component C
        | p(e1, ..., en)              -- a named predicate

`e` ranges over terms built from variables, component lookups, and
functions/predicates introduced by later artifacts (arithmetic
expressions, identity comparisons, etc.). `S`, `D` range over sets or
domains, similarly introduced later.

**Named predicates** (`p(...)`) are how domain-specific vocabulary enters
CFN without changing this grammar: e.g. a future artifact may introduce
`alive(o, Σ)`, `authority(a, op, r, Σ)`, or `valid(r, span, Σ)` as named
predicates defined in terms of the grammar above. This document does not
define any named predicate; it only fixes that they exist as ordinary
first-order predicates over `Σ`'s components.

**Separating conjunction:**

    Σ ⊨ P1 ∗ P2   ⟺   ∃ Σ1, Σ2. Σ = Σ1 ⊎ Σ2 ∧ Σ1 ⊨ P1 ∧ Σ2 ⊨ P2

`P1 ∗ P2` holds when `Σ` can be split into two resource-disjoint parts
separately satisfying `P1` and `P2`. This is the mechanism for stating
"I own this resource and, disjointly, something else holds" without
enumerating the rest of `Σ`, and is the intended vehicle for resource
authority (`term.authority`), aliasing validity (`term.alias`), and
spatial validity (`term.extent`) propositions in later artifacts.

## 4. Proof obligations

A rule may impose a side-condition that must be discharged rather than
assumed. Side-conditions are written after the rule, tagged with how
they are discharged:

    side-conditions:
        ⟦ φ ⟧ discharge: static
        ⟦ ψ ⟧ discharge: dynamic
        ⟦ χ ⟧ discharge: trusted

- `discharge: static` — the static judgment (§2.1) must prove `φ` before
  the rule applies; violating programs are rejected at that phase.
- `discharge: dynamic` — a runtime check for `ψ` is performed as part of
  taking the step; failure produces a `↛` diagnostic judgment (§2.4)
  rather than proceeding.
- `discharge: trusted` — `χ` is assumed without a language-enforced
  check; only permitted on a rule whose disposition (§5) is
  `trusted-unchecked`.

## 5. Rule tags

Every normative rule with an invariant-reliant precondition must carry a
**disposition** tag, closed and identical to Master Instructions §12
(see the qualification after the tag list below for rules with no such
precondition):

    disposition: rejected | checked | fallible | trusted-unchecked | unsupported

- `rejected` — the construct is statically invalid whenever the relevant
  precondition cannot be established; no runtime rule applies.
- `checked` — the rule includes a `discharge: dynamic` side-condition
  whose failure yields `↛`.
- `fallible` — the rule's conclusion is itself a result type carrying
  success or an explicit failure value (not a `↛` diagnostic) — used
  when failure is an ordinary, handleable outcome rather than a semantic
  fault.
- `trusted-unchecked` — the rule relies on a `discharge: trusted`
  side-condition; using it is only valid from a construct the language
  marks as explicitly trusted (defined by later artifacts).
- `unsupported` — no rule is defined; the construct is not part of
  CobaltC's normative semantics at the current specification version.

`disposition` classifies how a rule handles an **invariant-reliant
precondition** — one beyond ordinary static well-typedness, which every
construct presupposes regardless of disposition and is not itself an
instance of the taxonomy. A rule with no such precondition (e.g. a
totally-defined operation on already well-typed operands, such as a
widening conversion) carries no `disposition` tag at all — there is
nothing for the tag to classify. Do not force such a rule into
`rejected` or `checked` merely to satisfy a "every rule must be tagged"
reading; the tag exists to make invariant handling explicit, not to be
present unconditionally.

A rule whose conclusion's value is not uniquely determined by its
premises must additionally carry an **outcome** tag, closed:

    outcome: deterministic
    outcome: impl-defined { v1, v2, ... }   -- implementation commits to one, documented
    outcome: unspecified  { v1, v2, ... }   -- chosen freely per execution, all safe

`outcome` is omitted when the conclusion is fully determined by the
premises (the default, and the common case). There is no fourth outcome
value for unbounded or undeclared variation; any construct whose result
cannot be bounded to a documented finite set does not get an `outcome`
tag — it is instead not yet `ACCEPTED` (Master Instructions §18) until
one can be defined.

## 6. Rule format

    [Rule-Name]   disposition: <tag>   outcome: <tag, optional>
        premise-1    premise-2    ...
        ────────────────────────────────
        conclusion
        side-conditions:
            ⟦ ... ⟧ discharge: ...

Premises and conclusion are judgments from §2. A rule with no premises
(a fact) omits the line and premises.

## 7. Illustrative example (non-normative)

The following is a toy rule to demonstrate notation only. It uses
`term.binding` informally and does **not** define any real CobaltC
semantics — binding lookup semantics will be defined normatively by a
later artifact.

    [Binding-Lookup]   disposition: checked
        x ∈ dom(Σ.Bindings)
        ────────────────────────────────
        ⟨x, Σ⟩ → ⟨Σ.Bindings(x), Σ⟩
        side-conditions:
            ⟦ x ∈ dom(Σ.Bindings) ⟧ discharge: dynamic

    [Binding-Lookup-Fail]   disposition: checked
        x ∉ dom(Σ.Bindings)
        ────────────────────────────────
        ⟨x, Σ⟩ ↛ D-unbound-name

This shows the pattern later artifacts must follow: a premise gates
success, a paired rule (or the absence of any applicable rule) accounts
for failure, and the disposition/side-condition tags make the safety
handling explicit rather than implicit.

## Change Log

- 1.0.0 — Promoted to `ACCEPTED` unchanged in content (`spec/AUDIT-2.md` B-19). Every judgment form, the assertion grammar, `discharge`, `disposition`, and `outcome` are now exercised by `spec/03`–`spec/21`; `outcome: impl-defined`/`unspecified` appear in `spec/05`, `spec/06` §7, `spec/16` §1, `spec/19` §1, `spec/21` §0. §1's "Until then, Σ is opaque" is historical: `spec/04` fixes Σ.

- 0.2.0 — Added §2.5a, the program-termination judgment `↛↛`
  (`spec/AUDIT.md` A-09): `spec/18-error-failure-semantics.md` §1's
  `[Fault-Unwind]` used it before this document defined it.
- 0.1.1 — Clarified that `disposition` classifies invariant-reliant
  preconditions specifically; a rule with no such precondition (a
  totally-defined operation on well-typed operands) carries no
  `disposition` tag. Discovered while drafting arithmetic semantics
  (`spec/06-arithmetic.md`), where several operations (widening
  conversion, wrapping arithmetic, same-type comparison after static
  type-checking) have no dynamic or additional-static failure mode.
- 0.1.0 — Initial definition, selected per D-0001. Establishes judgment
  forms, assertion grammar with separating conjunction, proof-obligation
  notation, and the closed disposition/outcome taxonomies. No CobaltC
  semantics defined yet.
