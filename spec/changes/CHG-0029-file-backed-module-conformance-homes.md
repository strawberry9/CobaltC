# CHG-0029 — Conformance and Example Homes for File-Backed Modules

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-22)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §20, §21
Depends on: CHG-0026, D-0021, rule.module.file, term.conformance
Affects: conf.module-file-ok, conf.module-file-import-ok, conf.module-file-type-ok, conf.module-file-private-rejected, conf.module-file-nested-ok, conf.module-file-missing-rejected, conf.module-file-cycle-rejected, conf.module-file-duplicate-rejected, conf.module-file-main-is-ordinary-ok, conf.unbound-qualified-name-static, ex.module-file

## Problem / motivation

`CHG-0026` named nine `conf.module-file-*` cases but placed them only
in the file-based suite, reasoning that a `spec/conformance.md` row is
a single fragment and a file-backed case is two files. The audit that
followed found this leaves the ids without the home `spec/02` §2
fixes for `conf.*`, outside `term.conformance` ("every case in
`spec/conformance.md`"), and without the `ex.*` "canonical valid use"
example Master Instructions §19 expects. Separately, the reference
interpreter reported a qualified unbound name dynamically while the
registry says `[Resolve-Unbound]` is static, and `spec/IMPLEMENTATION-
NOTES.md` §1 lacked the two impl-defined rows `spec/22` 2.10.0 added.

## Decision

Give the cases their home by extending the conformance table's
conventions, not by weakening `term.conformance`: a fragment that is a
path to a whole-program file under `impl/conformance/` is that
program, run as `coby <file>` runs it.

## What changed

**`spec/conformance.md` 3.14.0**: new §14 with the convention, the
nine `conf.module-file-*` rows, and `conf.unbound-qualified-name-
static` (the `CHG-0027`/`CHG-0028` rows share the section).
**`spec/examples.md` 3.12.0**: `ex.module-file`. **`spec/
IMPLEMENTATION-NOTES.md`** (non-normative): the two rows; a stray
"`spec/17` §..." placeholder fixed. **`spec/02-schema.md` 1.0.3**,
**`spec/04-abstract-state.md` 1.3.1** (non-normative hygiene found by
the same audit): id ranges, table-defined type ids, "four" → "five".

## Affected entities

The nine `conf.module-file-*` cases (now homed; outcomes as
`CHG-0026` stated), `conf.unbound-qualified-name-static` (new),
`ex.module-file` (new). `term.conformance` is unchanged in text and,
with the convention, unchanged in meaning: every case is still a row
of `spec/conformance.md`.

## Previous semantics

Not applicable — no rule's meaning is involved.

## New semantics

Not applicable. The one normative-adjacent clarification: a qualified
unbound path is rejected statically, which `[Resolve-Unbound]`'s
registry entry (Phase: static) already stated; the interpreter is
brought into line and `conf.unbound-qualified-name-static` pins it.

## Affected invariants

None.

## Dependency impact

None.

## Compatibility classification

None for programs. For the conformance table's readers and runners: a
new fragment form (a `.cb` path) in §14 only.

## Migration implications

None.

## Example changes

`ex.module-file` added.

## Conformance changes

As above. `impl/tests/spec_rows.rs` runs a path-fragment row through
the same entry point `coby` uses (`run_program_phased_located` with
the file's own path), so the row's outcome and phase are checked
mechanically like every other row.

## Future implementation implications

None beyond the runner change. An implementation that already passes
the file-based suite passes §14.

## Prior-art status

Not applicable.

## Revisit conditions

If a second family of multi-file cases appears, generalize the
convention (a fixture directory per row) rather than adding a second
one.
