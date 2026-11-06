# CHG-0018 — `spec/IMPLEMENTATION-NOTES.md` Added

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §20, §24
Depends on: term.safe-program, rule.trust.unsafe

## Problem / motivation

Write the Master
Instructions §20 implementation-feasibility analysis in its final
form, consolidating every `outcome: impl-defined` choice, every
`outcome: unspecified` point, and every `discharge: trusted` condition
with its rule label, as what the human owner needs to authorize a
reference implementation.

## What was added

`spec/IMPLEMENTATION-NOTES.md` (non-normative, per `spec/02-schema.md`
§6 — carries no rule/invariant/type/state entity and needs no version-
bump discipline of its own beyond its own Change Log): three tables
(implementation-defined choices, permitted nondeterminism, trusted
conditions) drawing on `spec/22` §5's existing lists and a fresh sweep
of every `impl-defined`/`unspecified`/`discharge: trusted` occurrence
in `spec/01`–`spec/21`, plus a closing assessment of what nine
verification passes have and have not found, and a recommendation
(advisory only) that F-05 (`spec/AUDIT-STATUS.md`, unresolved) be
settled before a reference implementation is authorized, since it
concerns whether a specific documented trust condition is actually
sound rather than merely an open ambiguity.

`spec/README.md` gains one cross-reference to the new file from its
existing "What you must decide and document" section.

## Rule changes

None — this record adds a non-normative artifact and one cross-
reference; no rule, invariant, type, or state entity's meaning
changed.

## Affected invariants

None.

## Dependency impact

None on existing entities.

## Compatibility classification

Purely additive, non-normative.

## Migration implications

None.

## Example changes

None.

## Conformance changes

None.

## Future implementation implications

This file's entire content is future-implementation guidance by
design; no further note needed here beyond directing a reader to it.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

Update `spec/IMPLEMENTATION-NOTES.md` whenever a future pass adds,
removes, or resolves an `impl-defined`/`unspecified`/`trusted` point
(starting with F-05, whenever it is resolved) — it is a derived
summary, not a second source of truth, and should never be edited to
say something the normative artifacts do not already say.
