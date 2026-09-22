# CobaltC Specification Schema

Status: normative artifact
Version: 1.0.41
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

- **Rule** — a CFN rule (`spec/01-metalanguage.md` §6) with a
  `disposition` tag and, where applicable, an `outcome` tag. Depends-on
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
    spec/examples.md            -- Example entities (single file; would
                                    split into spec/examples/ only if
                                    volume later warrants it)
    spec/conformance.md         -- Conformance-case entities (same note)

Non-numbered, growing directories, created on first actual use rather
than pre-created speculatively:

    spec/decisions/D-XXXX-<slug>.md   -- one file per Decision (in use:
                                          D-0001 through D-0057)
    spec/registry/features.md         -- Feature entities (in use)
    spec/registry/diagnostics.md      -- Diagnostic entities (in use)
    spec/changes/CHG-XXXX-<slug>.md   -- one file per Change record (in use:
                                          CHG-0001 through CHG-0065)

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
