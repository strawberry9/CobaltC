# CobaltC Specification Schema

Status: normative artifact
Version: 1.0.138
Governed by: `CobaltC_Master_Instructions.md` §13, §14, §15, §18, §21, §22

## Purpose

This document defines the structural format shared by every normative
artifact in the CobaltC Specification: what an "entity" is, how it is
identified, what status it can hold, what fields each kind of entity
must carry, how dependencies between entities are recorded so the
dependency graph required by §14/§21 can be mechanically derived rather
than hand-maintained, and where each kind of entity lives on disk.

This document defines structure, not content. It introduces no CobaltC
semantics and no new invariants.

## 1. Entity model

Every normative or design-record item in the specification is an
**entity**. Every entity has:

- **Kind** — one of the kinds in §2 below.
- **Id** — a stable identifier, namespaced by kind (§2), never reused
  for a different meaning once published (Master Instructions §13, §21).
- **Status** — one value from the lifecycle in §3.
- **Defined-in** — the artifact file that is authoritative for this
  entity (its home; see §5).
- **Depends on / Affects** — explicit links to other entity ids (§4),
  the raw material for the machine-readable dependency graph required
  by §14 and §21.
- Kind-specific required fields, given per kind in §2.

An entity's Id and Kind never change. Its Status, its kind-specific
fields, and its Depends-on/Affects links may change over the entity's
life, subject to the immutability rules in §6.

## 2. Kinds, id namespaces, and required fields

| Kind | Id prefix | Example | Home (§5) |
|---|---|---|---|
| Term | `term.<name>` | `term.object` | `spec/00-terminology.md` |
| Decision | `D-XXXX` | `D-0001` | `spec/decisions/D-XXXX-<slug>.md` |
| Invariant | `inv.<name>` | `inv.spatial-validity` | `spec/03-invariants.md` |
| Rule | `rule.<area>.<name>` | `rule.arith.checked` | topic artifact for `<area>` |
| Type | `type.<name>` | `type.i32` | topic artifact for `<area>` (`spec/06`, `spec/09`, `spec/12`, `spec/16`, ...). A family of types may be defined as rows of one table in its home (the numeric types, `spec/06` §1) rather than each as a headed block; each row's id is a full entity id, citable in `Depends on` lines, and its Status is that of the table's section |
| State component | `state.<name>` | `state.objects` | `spec/04-abstract-state.md` |
| Diagnostic | `diag.<name>` | `diag.unbound-name` | `spec/registry/diagnostics.md` |
| Example | `ex.<name>` | `ex.checked-arithmetic` | `spec/examples.md` |
| Conformance case | `conf.<name>` | `conf.i32-add-overflow-static` | `spec/conformance.md` |
| Feature | `feat.<name>` | `feat.closures` | `spec/registry/features.md` |
| Change record | `CHG-XXXX` | `CHG-0001` | `spec/changes/CHG-XXXX-<slug>.md` (directory created with the first record; required for any normative change from version 1.0.0 on, since every entity is now `ACCEPTED`) |

An id, once assigned to a Kind, is permanent for that Kind even if the
entity is later `REMOVED` (§3) — removal is a status, not a deletion of
the id from the historical record (Master Instructions §21: "do not
silently change the meaning of an existing normative entity").

### Required fields by kind

Fields below are in addition to Kind/Id/Status/Defined-in/Depends-on/Affects.

- **Term** — Definition; Notes (optional).
  (Established convention: `spec/00-terminology.md`.)

- **Invariant** (Master Instructions §15) — subjects; formal proposition
  (in CFN, `spec/01-metalanguage.md`); establishment; preservation;
  transformation; weakening; invalidation; consumption/restoration where
  applicable; checking; reliance points; enforcement strategy; failure
  behavior.

- **Rule** — a CFN rule (`spec/01-metalanguage.md` §6) carrying a
  `disposition` tag where it has an invariant-reliant precondition and
  none otherwise (`spec/01` §5), and an `outcome` tag where its result
  is not determined by its premises (D-0206). Depends-on
  must list every `term.*`/`inv.*`/`type.*`/`state.*` it references.

- **Diagnostic** (Master Instructions §19) — semantic phase; violated
  rule; violated invariant; required fact; observed fact; provenance of
  conflicting facts; relevant source-location shape; repair category;
  related examples.

- **Example** (Master Instructions §19) — category (one of: canonical
  valid use, common mistake, boundary case, invalid semantic state,
  corrected form, feature interaction); the illustrated rule(s)/
  invariant(s) via Depends-on; body. Examples illustrate; they carry no
  normative force of their own (§19).

- **Conformance case** — the program fragment or scenario; the expected
  judgment outcome(s) (in terms of the rules/diagnostics that must or
  must not apply); Depends-on the rule(s)/invariant(s) under test.

- **Feature** (Master Instructions §18) — motivating capability;
  semantic problem; examples demonstrating need; affected invariants;
  required semantic state and transitions; existing mechanisms
  considered; alternative mechanisms; library/inference/generation/
  tooling alternatives; smallest viable mechanism; syntax cost;
  conceptual cost; interaction cost; specification cost; future
  implementation cost; diagnostic cost; runtime cost; learning cost;
  compatibility burden; safety implications; composability implications;
  prior-art influence; invariant traceability (via Depends-on); decision
  rationale.

- **Decision** — unifies Master Instructions §11 (derivation record for
  a major semantic mechanism) and §22 (design-decision record); the two
  field lists overlap enough that maintaining them as separate kinds
  would duplicate structure without semantic benefit (Master
  Instructions §9, §14: prefer one authoritative source). A Decision
  records: problem/semantic problem; constraints; required facts (where
  applicable); relevant invariants (via Depends-on); invalid states to
  prevent (where applicable); minimum required information / necessary
  abstract state (where applicable); valid/invalid transitions (where
  applicable); candidate designs / materially distinct candidate
  mechanisms; selected design; rejected alternatives; semantic
  rationale; usability implications; explainability implications;
  implementation-feasibility implications; compatibility impact;
  prior-art status; invariant traceability (via Depends-on); revisit
  conditions.
  (`D-0001` is the first conforming instance.)

- **Change record** (Master Instructions §21) — affected entities (via
  Affects); previous semantics; new semantics; affected invariants (via
  Depends-on); dependency impact; compatibility classification;
  migration implications; example changes; conformance changes; future
  implementation implications.

## 3. Status lifecycle

All entity kinds share one closed status lifecycle — the set Master
Instructions §18 defines for features, generalized here to every kind so
the specification system has one lifecycle model rather than one per
kind (Master Instructions §9 applied reflexively to the specification's
own structure):

    ABSENT | UNDER_INVESTIGATION | PROVISIONAL | ACCEPTED | DEFERRED | REJECTED | REMOVED

Not every kind exercises every value in practice — e.g. a Term
typically only moves `PROVISIONAL → ACCEPTED`, while a Feature is
expected to use the full range — but no kind is restricted from any
value by this schema. Only `ACCEPTED` entities are part of the intended
CobaltC design (Master Instructions §18).

## 4. Dependency links and the dependency graph

Every entity file records its links using exactly two field labels, so
the graph required by Master Instructions §14/§21 can be extracted
mechanically by scanning for these labels rather than maintained as a
separate hand-written artifact:

    Depends on: <id>, <id>, ...
    Affects: <id>, <id>, ...

- **Depends on** — entities this one presupposes or requires to be
  meaningful (e.g. a Rule depends on the terms and invariants it
  references).
- **Affects** — entities this one changes, constrains, or motivates,
  used primarily by Decision and Change-record kinds (e.g. a Decision
  affects the invariants or terms its selected design touches).

Both fields are optional per entity (omit when empty) but must use
exactly this label text when present, so the graph can be built by a
simple mechanical scan — no separate dependency file is maintained by
hand (Master Instructions §14: "prefer one authoritative source").

**Tightened (`spec/AUDIT.md` A-15):** a `Depends on:`/`Affects:` value
must be a comma-separated list of entity ids ONLY (`term.<name>`,
`D-XXXX`, `inv.<name>`, `rule.<area>.<name>`, `type.<name>`,
`state.<name>`, `diag.<name>`, `ex.<name>`, `conf.<name>`,
`feat.<name>`, `CHG-XXXX`) — never a bare file path, a `§`-numbered
section reference, or a bracket-quoted rule name standing in for an id
that was never actually assigned. Where a referenced rule has not yet
been given a formal `rule.<area>.<name>` id (a real, separate gap —
`spec/AUDIT.md` A-14), the correct fix is to assign it one, not to cite
its file/section instead. A field label is written either bold
(`**Depends on:**`, the form every entity in `spec/00`–`spec/21` and the
registries uses) or plain (`Depends on:`, the form the decision and
change records use), and its value may wrap onto following lines; a
continuation line is any line that does not start a new field, heading,
list item, table row, or blank line. Extraction is mechanical:

    perl -0777 -ne 'while (/^\*{0,2}(?:Depends on|Affects):\*{0,2}((?:[^\n]*)(?:\n(?![ \t]*(?:\*\*|#|-|\||$|[A-Z][A-Za-z ]*:))[^\n]*)*)/mg) { print "$1\n" }' spec/**/*.md \
        | grep -oE '\b(term|D|inv|rule|type|state|diag|ex|conf|feat|CHG)[.-][A-Za-z0-9_.-]+'

Every id this prints is a graph edge's endpoint; anything a `Depends
on:`/`Affects:` value contains that the second pattern does *not* match
is, by this section, a non-conforming line. (The 1.0.0 form of this
command matched only plain, single-line labels, so it silently skipped
every bold or wrapped value — the check recorded as clean in
`spec/AUDIT-STATUS.md` A-15 had inspected 57 of the 696 ids the corpus
actually carries. Decision records that are `ACCEPTED`/`REMOVED` and
therefore immutable (§6) keep their historical, occasionally
non-conforming values; the normative artifacts are clean under this
command.)

## 5. Artifact organization

Core numbered artifacts, reflecting the §23 design order and created
only as each becomes needed (Master Instructions §24):

    spec/00-terminology.md      -- Term entities
    spec/01-metalanguage.md     -- CFN notation (not an entity registry)
    spec/02-schema.md           -- this document
    spec/03-invariants.md       -- Invariant entities
    spec/04-abstract-state.md   -- State-component entities
    spec/05..22                 -- value/object, arithmetic, type system,
                                    etc., per the §23 order; each such
                                    artifact is the home for its area's
                                    Rule/Type entities (id prefixes
                                    `rule.<area>.*`, `type.<name>`)
    spec/23-preservation.md     -- the preservation lemmas (a proof artifact,
                                    non-normative; D-0262): per rule, why each
                                    invariant of spec/03 survives it
    spec/examples.md            -- Example entities (single file; would
                                    split into spec/examples/ only if
                                    volume later warrants it)
    spec/conformance.md         -- Conformance-case entities (same note)

Non-numbered, growing directories, created on first actual use rather
than pre-created speculatively:

    spec/decisions/D-XXXX-<slug>.md   -- one file per Decision (in use:
                                          D-0001 through D-0275)
    spec/registry/features.md         -- Feature entities (in use)
    spec/registry/diagnostics.md      -- Diagnostic entities (in use)
    spec/changes/CHG-XXXX-<slug>.md   -- one file per Change record (in use:
                                          CHG-0001 through CHG-0367)

A directory or file is created at the point a real entity needs it, not
in advance (Master Instructions §24).

## 6. Immutability and versioning

- While an entity's Status is `PROVISIONAL` (or earlier:
  `ABSENT`/`UNDER_INVESTIGATION`), its fields may be edited in place in
  its home artifact, and the artifact's own Change Log records that a
  revision occurred, without needing a separate Change record.
- Once an entity's Status is `ACCEPTED`, altering its normative meaning
  requires a Change record (§2, Change record kind) and a version bump
  of the containing artifact (Master Instructions §21: "do not silently
  change the meaning of an existing normative entity"). Non-normative
  fixes (typos, formatting) do not require a Change record but should
  still be noted in the artifact's Change Log.
- A Decision entity, once `ACCEPTED`, is never edited. A later decision
  that revisits it is a new Decision id that supersedes the old one
  (stated explicitly in the new Decision's body and via `Affects:`); the
  superseded Decision's file is kept as-is, its Status updated to
  `REMOVED` or `DEFERRED` as appropriate. History is preserved, never
  overwritten.
- Every artifact that is itself a registry (terminology, invariants,
  abstract state, feature registry, diagnostic registry) carries a
  single document Version and Change Log, independent of the
  per-entity Status values it contains, per the pattern already
  established in `spec/00-terminology.md` and `spec/01-metalanguage.md`.

## Change Log

- 1.0.138 — Non-normative (`CHG-0367`): ranges up to D-0275 and CHG-0367.
- 1.0.137 — Non-normative (`CHG-0365`, `CHG-0366`): ranges up to D-0275 and CHG-0366.
- 1.0.136 — Non-normative (`CHG-0364`): ranges up to D-0274 and CHG-0364.
- 1.0.135 — Non-normative (`CHG-0363`): ranges up to D-0274 and CHG-0363.
- 1.0.134 — Non-normative (`CHG-0362`): ranges up to D-0274 and CHG-0362.
- 1.0.133 — Non-normative (`CHG-0361`): ranges up to D-0273 and CHG-0361.
- 1.0.132 — Non-normative (`CHG-0360`): ranges up to D-0273 and CHG-0360.
- 1.0.131 — Non-normative (`CHG-0359`): ranges up to D-0272 and CHG-0359.
- 1.0.130 — Non-normative (`CHG-0354`): ranges up to D-0272 and CHG-0354.
- 1.0.129 — Non-normative (`CHG-0353`): ranges up to D-0272 and CHG-0353.
- 1.0.128 — Non-normative (`CHG-0352`): ranges up to D-0272 and CHG-0352.
- 1.0.127 — Non-normative (`CHG-0349`, `CHG-0350`, `CHG-0351`): ranges up to D-0272 and CHG-0351.
- 1.0.126 — Non-normative (`CHG-0347`, `CHG-0348`, D-0272): ranges up to D-0272 and CHG-0348.
- 1.0.125 — Non-normative (`CHG-0345`, `CHG-0346`): ranges up to D-0271 and CHG-0346.
- 1.0.124 — Non-normative (`CHG-0344`, D-0271): ranges up to D-0271 and CHG-0344.
- 1.0.123 — Non-normative (`CHG-0343`, D-0270): ranges up to D-0270 and CHG-0343.
- 1.0.122 — Non-normative (`CHG-0342`): ranges up to CHG-0342.
- 1.0.121 — Non-normative (`CHG-0341`, D-0269): ranges up to D-0269 and CHG-0341.
- 1.0.120 — Non-normative (`CHG-0340`): ranges up to CHG-0340.
- 1.0.119 — Non-normative (`CHG-0338`, `CHG-0339`): ranges up to CHG-0339.
- 1.0.118 — Non-normative (`CHG-0337`): ranges up to CHG-0337.
- 1.0.117 — Non-normative (`CHG-0336`, D-0268): ranges up to D-0268 and CHG-0336.
- 1.0.116 — Non-normative (`CHG-0335`, D-0267): ranges up to D-0267 and CHG-0335.
- 1.0.115 — Non-normative (`CHG-0334`, D-0266): ranges up to D-0266 and CHG-0334.
- 1.0.114 — Non-normative (`CHG-0333`): ranges up to CHG-0333.
- 1.0.113 — Non-normative (`CHG-0332`, D-0261): ranges up to CHG-0332.
- 1.0.112 — Non-normative (`CHG-0331`, D-0261): ranges up to CHG-0331.
- 1.0.111 — Non-normative (`CHG-0330`, D-0265): ranges up to CHG-0330.
- 1.0.110 — Non-normative (`CHG-0329`, D-0265): ranges up to D-0265 and CHG-0329.
- 1.0.109 — Non-normative (`CHG-0328`, D-0264): ranges up to D-0264 and CHG-0328.
- 1.0.108 — Non-normative (`CHG-0327`, D-0263): ranges up to D-0263 and CHG-0327.
- 1.0.107 — Non-normative (`CHG-0326`, D-0262): §5 lists `spec/23-preservation.md`; ranges up to D-0262 and CHG-0326.
- 1.0.106 — Non-normative (`CHG-0325`, D-0261): §5's "in use" ranges brought up to D-0261 and CHG-0325.
- 1.0.105 — Non-normative (`CHG-0324`, D-0260): §5's "in use" ranges brought up to D-0260 and CHG-0324.
- 1.0.104 — Non-normative (`CHG-0321`): §5's "in use" range brought up to CHG-0321.
- 1.0.103 — Non-normative (`CHG-0320`): §5's "in use" range brought up to CHG-0320.
- 1.0.102 — Non-normative (`CHG-0319`): §5's "in use" range brought up to CHG-0319.
- 1.0.101 — Non-normative (`CHG-0318`): §5's "in use" range brought up to CHG-0318.
- 1.0.100 — Non-normative (`CHG-0317`): §5's "in use" range brought up to CHG-0317.
- 1.0.99 — Non-normative (`CHG-0316`): §5's "in use" range brought up to CHG-0316.
- 1.0.98 — Non-normative (`CHG-0315`): §5's "in use" range brought up to CHG-0315.
- 1.0.97 — Non-normative (`CHG-0314`): §5's "in use" range brought up to CHG-0314.
- 1.0.96 — Non-normative (`CHG-0313`): §5's "in use" ranges brought up to D-0255 and CHG-0313.
- 1.0.95 — Non-normative (`CHG-0312`): §5's "in use" ranges brought up to D-0254 and CHG-0312.
- 1.0.94 — Non-normative (`CHG-0311`): §5's "in use" ranges brought up to D-0253 and CHG-0311.
- 1.0.93 — Non-normative (`CHG-0309`, `CHG-0310`): §5's "in use" ranges brought up to D-0252 and CHG-0310.
- 1.0.92 — Non-normative (`CHG-0306`–`CHG-0308`): §5's "in use" ranges brought up to D-0251 and CHG-0308.
- 1.0.91 — Non-normative (`CHG-0292`–`CHG-0305`): §5's "in use" ranges brought up to D-0248 and CHG-0305.
- 1.0.90 — Non-normative (`CHG-0284`–`CHG-0291`): §5's "in use" ranges brought up to D-0241 and CHG-0291.
- 1.0.89 — Non-normative (`CHG-0283`): §5's "in use" range brought up to CHG-0283.
- 1.0.88 — Non-normative (`CHG-0282`): §5's "in use" range brought up to CHG-0282.
- 1.0.87 — Non-normative (`CHG-0281`): §5's "in use" ranges brought up to D-0234 and CHG-0281.
- 1.0.86 — Non-normative (`CHG-0280`): §5's "in use" ranges brought up to D-0233 and CHG-0280.
- 1.0.85 — Non-normative (`CHG-0279`): §5's "in use" range brought up to CHG-0279.
- 1.0.84 — Non-normative (`CHG-0278`): §5's "in use" range brought up to CHG-0278.
- 1.0.83 — Non-normative (`CHG-0277`): §5's "in use" range brought up to CHG-0277.
- 1.0.82 — Non-normative (`CHG-0276`): §5's "in use" range brought up to CHG-0276.
- 1.0.81 — Non-normative (`CHG-0275`): §5's "in use" range brought up to CHG-0275.
- 1.0.80 — Non-normative (D-0228 to D-0231, `CHG-0270` to `CHG-0274`): §5's "in use"
  ranges brought up to D-0231 and CHG-0274.
- 1.0.79 — Non-normative (D-0223 to D-0227, `CHG-0262` to `CHG-0269`): §5's "in use"
  ranges brought up to D-0227 and CHG-0269.
- 1.0.78 — Non-normative (D-0221, D-0222, `CHG-0260`, `CHG-0261`): §5's "in use"
  ranges brought up to D-0222 and CHG-0261.
- 1.0.77 — Non-normative (D-0219, D-0220, `CHG-0258`, `CHG-0259`): §5's "in use"
  ranges brought up to D-0220 and CHG-0259.
- 1.0.76 — Non-normative (D-0218, `CHG-0257`): §5's "in use" ranges
  brought up to D-0218 and CHG-0257.
- 1.0.75 — Non-normative (D-0213 to D-0217, `CHG-0251` to `CHG-0256`): §5's "in use"
  ranges brought up to D-0217 and CHG-0256.
- 1.0.74 — Non-normative (D-0212, `CHG-0250`): §5's "in use" ranges
  brought up to D-0212 and CHG-0250.
- 1.0.73 — Non-normative (D-0211, `CHG-0249`): §5's "in use" ranges
  brought up to D-0211 and CHG-0249.
- 1.0.72 — Non-normative (D-0210, `CHG-0248`): §5's "in use" ranges
  brought up to D-0210 and CHG-0248.
- 1.0.71 — Non-normative (D-0209, `CHG-0247`): §5's "in use" ranges
  brought up to D-0209 and CHG-0247.
- 1.0.70 — Non-normative (D-0208, `CHG-0246`): §5's "in use" ranges
  brought up to D-0208 and CHG-0246.
- 1.0.69 — Non-normative (D-0207, `CHG-0245`): §5's "in use" ranges
  brought up to D-0207 and CHG-0245.
- 1.0.68 — `CHG-0242` (D-0206): §2's Rule entry carries a `disposition`
  only where `spec/01` §5 gives it one; §5's "in use" ranges brought up
  to D-0206 and CHG-0244.
- 1.0.67 — Non-normative (D-0086 through D-0091, `CHG-0101` through
  `CHG-0106`): §5's "in use" ranges brought up to D-0091 and CHG-0106.
- 1.0.66 — Non-normative (D-0084, D-0085, `CHG-0100`): §5's "in use"
  ranges brought up to D-0085 and CHG-0100.
- 1.0.65 — Non-normative (D-0078 through D-0083, `CHG-0092` through
  `CHG-0099`): §5's "in use" ranges brought up to D-0083 and CHG-0099.
- 1.0.64 — Non-normative (D-0077, `CHG-0091`): §5's "in use" ranges
  brought up to D-0077 and CHG-0091.
- 1.0.63 — Non-normative (D-0076, `CHG-0090`): §5's "in use" ranges
  brought up to D-0076 and CHG-0090.
- 1.0.62 — Non-normative (`CHG-0089`): §5's "in use" ranges brought up
  to CHG-0089.
- 1.0.61 — Non-normative (D-0075, `CHG-0088`): §5's "in use" ranges
  brought up to D-0075 and CHG-0088.
- 1.0.60 — Non-normative (D-0074, `CHG-0087`): §5's "in use" ranges
  brought up to D-0074 and CHG-0087.
- 1.0.59 — Non-normative (D-0073, `CHG-0086`): §5's "in use" ranges
  brought up to D-0073 and CHG-0086.
- 1.0.58 — Non-normative (D-0072, `CHG-0085`): §5's "in use" ranges
  brought up to D-0072 and CHG-0085.
- 1.0.57 — Non-normative (`CHG-0084`): §5's "in use" ranges brought up
  to CHG-0084.
- 1.0.56 — Non-normative (D-0071, `CHG-0083`): §5's "in use" ranges
  brought up to D-0071 and CHG-0083.
- 1.0.55 — Non-normative (`CHG-0082`): §5's "in use" ranges brought up
  to CHG-0082.
- 1.0.54 — Non-normative (`CHG-0081`): §5's "in use" ranges brought up
  to CHG-0081.
- 1.0.53 — Non-normative (D-0070, `CHG-0080`): §5's "in use" ranges
  brought up to D-0070 and CHG-0080.
- 1.0.52 — Non-normative (D-0069, `CHG-0079`): §5's "in use" ranges
  brought up to D-0069 and CHG-0079.
- 1.0.51 — Non-normative (D-0068, `CHG-0078`): §5's "in use" ranges
  brought up to D-0068 and CHG-0078.
- 1.0.50 — Non-normative (D-0067, `CHG-0077`): §5's "in use" ranges
  brought up to D-0067 and CHG-0077.
- 1.0.49 — Non-normative (D-0066, `CHG-0076`): §5's "in use" ranges
  brought up to D-0066 and CHG-0076.
- 1.0.48 — Non-normative (D-0065, `CHG-0075`): §5's "in use" ranges
  brought up to D-0065 and CHG-0075.
- 1.0.47 — Non-normative (D-0064, `CHG-0074`): §5's "in use" ranges
  brought up to D-0064 and CHG-0074.
- 1.0.46 — Non-normative (D-0063, `CHG-0072`, `CHG-0073`): §5's "in use"
  ranges brought up to D-0063 and CHG-0073.
- 1.0.45 — Non-normative (`CHG-0071`): §5's "in use" ranges brought up
  to CHG-0071.
- 1.0.44 — Non-normative (D-0062, `CHG-0070`): §5's "in use" ranges
  brought up to D-0062 and CHG-0070.
- 1.0.43 — Non-normative (D-0061, `CHG-0069`): §5's "in use" ranges
  brought up to D-0061 and CHG-0069.
- 1.0.42 — Non-normative (D-0058–D-0060, `CHG-0066`–`CHG-0068`): §5's
  "in use" ranges brought up to D-0060 and CHG-0068.
- 1.0.41 — Non-normative (D-0057, `CHG-0065`): §5's "in use" ranges
  brought up to D-0057 and CHG-0065.
- 1.0.40 — Non-normative (D-0056, `CHG-0064`): §5's "in use" ranges
  brought up to D-0056 and CHG-0064.
- 1.0.39 — Non-normative (D-0055, `CHG-0063`): §5's "in use" ranges
  brought up to D-0055 and CHG-0063.
- 1.0.38 — Non-normative (D-0054, `CHG-0062`): §5's "in use" ranges
  brought up to D-0054 and CHG-0062.
- 1.0.37 — Non-normative (D-0053, `CHG-0061`): §5's "in use" ranges
  brought up to D-0053 and CHG-0061.
- 1.0.36 — Non-normative (D-0052, `CHG-0060`): §5's "in use" ranges
  brought up to D-0052 and CHG-0060.
- 1.0.35 — Non-normative (D-0051, `CHG-0059`): §5's "in use" ranges
  brought up to D-0051 and CHG-0059.
- 1.0.34 — Non-normative (D-0050, `CHG-0058`): §5's "in use" ranges
  brought up to D-0050 and CHG-0058.
- 1.0.33 — Non-normative (D-0049, `CHG-0057`): §5's "in use" ranges
  brought up to D-0049 and CHG-0057.
- 1.0.32 — Non-normative (D-0048, `CHG-0056`): §5's "in use" ranges
  brought up to D-0048 and CHG-0056.

- 1.0.31 — Non-normative (D-0047, `CHG-0055`): §5's "in use" ranges
  brought up to D-0047 and CHG-0055.

- 1.0.30 — Non-normative (D-0046, `CHG-0054`): §5's "in use" ranges
  brought up to D-0046 and CHG-0054.

- 1.0.29 — Non-normative (D-0045, `CHG-0053`): §5's "in use" ranges
  brought up to D-0045 and CHG-0053.

- 1.0.28 — Non-normative (D-0044, `CHG-0052`): §5's "in use" ranges
  brought up to D-0044 and CHG-0052.

- 1.0.27 — Non-normative (D-0043, `CHG-0051`): §5's "in use" ranges
  brought up to D-0043 and CHG-0051.

- 1.0.26 — Non-normative (D-0042, `CHG-0050`): §5's "in use" ranges
  brought up to D-0042 and CHG-0050.

- 1.0.25 — Non-normative (D-0041, `CHG-0049`): §5's "in use" ranges
  brought up to D-0041 and CHG-0049.

- 1.0.24 — Non-normative (D-0040, `CHG-0048`): §5's "in use" ranges
  brought up to D-0040 and CHG-0048.

- 1.0.23 — Non-normative (D-0039, `CHG-0047`): §5's "in use" ranges
  brought up to D-0039 and CHG-0047.

- 1.0.22 — Non-normative (D-0038, `CHG-0046`): §5's "in use" ranges
  brought up to D-0038 and CHG-0046.

- 1.0.21 — Non-normative (D-0037, `CHG-0045`): §5's "in use" ranges
  brought up to D-0037 and CHG-0045.

- 1.0.20 — Non-normative (D-0035, D-0036, `CHG-0044`): §5's "in use"
  ranges brought up to D-0036 and CHG-0044.

- 1.0.19 — Non-normative (D-0034, `CHG-0043`): §5's "in use" ranges
  brought up to D-0034 and CHG-0043.

- 1.0.18 — Non-normative (D-0033, `CHG-0042`): §5's "in use" ranges
  brought up to D-0033 and CHG-0042.

- 1.0.17 — Non-normative (D-0032, `CHG-0041`): §5's "in use" ranges
  brought up to D-0032 and CHG-0041.

- 1.0.16 — Non-normative (D-0031, `CHG-0040`): §5's "in use" ranges
  brought up to D-0031 and CHG-0040.

- 1.0.15 — Non-normative (D-0030, `CHG-0039`): §5's "in use" ranges
  brought up to D-0030 and CHG-0039.

- 1.0.14 — Non-normative (D-0029, `CHG-0038`): §5's "in use" ranges
  brought up to D-0029 and CHG-0038.

- 1.0.13 — Non-normative (D-0028, `CHG-0037`): §5's "in use" ranges
  brought up to D-0028 and CHG-0037.

- 1.0.12 — Non-normative (D-0027, `CHG-0036`): §5's "in use" ranges
  brought up to D-0027 and CHG-0036.

- 1.0.11 — Non-normative (D-0026, `CHG-0035`): §5's "in use" ranges
  brought up to D-0026 and CHG-0035.

- 1.0.10 — Non-normative (D-0025, `CHG-0034`): §5's "in use" ranges
  brought up to D-0025 and CHG-0034.

- 1.0.9 — Non-normative (D-0024, `CHG-0033`): §5's "in use" ranges
  brought up to D-0024 and CHG-0033.
- 1.0.8 — Non-normative (`CHG-0032`): §5's "in use" range brought up
  to CHG-0032.
- 1.0.7 — Non-normative (D-0023): §5's "in use" range brought up to
  D-0023.
- 1.0.6 — Non-normative (`CHG-0031`): §5's "in use" range brought up
  to CHG-0031.
- 1.0.5 — Non-normative (D-0022): §5's "in use" range brought up to
  D-0022.
- 1.0.4 — Non-normative (`CHG-0030`): §5's "in use" range brought up
  to CHG-0030.
- 1.0.3 — Non-normative (consistency pass, 2026-09-22): §5's "in use"
  ranges brought up to D-0021 and CHG-0029; §2's Type row states that
  table-defined type ids (`type.i8` … `type.f64`, `spec/06` §1) are
  full entity ids, which `spec/06`/`spec/16` `Depends on` lines have
  cited all along.
- 1.0.2 — Non-normative (`CHG-0025`): §5's "in use" ranges brought up
  to date (D-0020, CHG-0025); they had stopped at D-0019/CHG-0019.
- 1.0.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"):
  §4's extraction command replaced by one that handles the bold and
  wrapped label forms the corpus actually uses (the 1.0.0 command
  matched almost nothing); §5's "none yet" for change records
  corrected (eight existed); §2's example ids now name entities that
  exist.
- 1.0.0 — Promoted to `ACCEPTED`; §2/§5 updated for D-0018/D-0019 and the `spec/README.md` entry point. The §4 grep is the conformance check for dependency lines; `spec/AUDIT-STATUS.md` records its last clean run.

- 0.3.0 — §4 tightened to entity-ids-only for `Depends on:`/`Affects:`
  (`spec/AUDIT.md` A-15), with a one-line grep as the conformance check
  for the convention itself. Existing non-conforming lines fixed where
  found across the corpus in the same pass; a residual cleanup item is
  tracked in `spec/AUDIT-STATUS.md` for any missed elsewhere. Still
  `PROVISIONAL`; in-place revision.
- 0.2.0 — Updated §2/§5 "Home" locations from "(future)" placeholders to
  actual paths now that every entity kind has a populated home
  (`spec/03`, `spec/04`, `spec/examples.md`, `spec/conformance.md`,
  `spec/registry/diagnostics.md`, `spec/registry/features.md`). Examples
  and conformance cases were given single files rather than the
  originally-sketched directories, since current volume doesn't warrant
  the split — noted as a deliberate simplification, not a deviation.
  Still `PROVISIONAL`; in-place revision.
- 0.1.0 — Initial definition. Establishes the entity model, id
  namespaces, the shared status lifecycle, the Depends-on/Affects
  dependency-graph convention, artifact organization, and immutability
  rules.
