# D-0220 — A stability policy: marks per part, kinds of change, a deprecation window

Status: ACCEPTED (2026-10-12; the owner handed over `private/AGENT-INSTRUCTIONS-2026-10-12.md` with its recommended
answers standing, Q2: "a stability policy … core and std each carry a stability mark")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §21 (change management), §26 (success criteria)
Depends on: D-0212 (edition manifest, evidence tiers)
Affects: `spec/STABILITY.md` (new, non-normative), `spec/README.md` (a "Stability" paragraph; the register keeps
the ten most recent editions), `spec/EDITIONS-HISTORY.md` (new: the older rows, unchanged), `NAMING-POLICY.md`
(a claim inherits its edition's marks), the guide's introduction (CHG-0259)

## Problem

The project published 61 editions in sixteen days, three of them breaking. Nothing said what "stable" would
mean, what a reader of one edition could rely on in the next, or when that would begin. The language and the
standard library change at different rates: since D-0206 the language has taken only repairs, while the
library's module map moved four times in nine days. A single word for both would either hold the library back
or overstate the language. A conformance claim (`NAMING-POLICY.md`) named an edition but nothing about whether
that edition would last.

## Options

A. **Two marks, one edition series.** The language (*core*) and the library (*std*) each carry a mark —
*moving*, *candidate*, *stable* — in every edition row. A candidate period (thirty days) admits only
clarifications and soundness repairs; a stable part takes breaking changes only through a deprecation window.
B. **Two edition series**, one for the language and one for the library. Cleaner in principle, but every tool,
the manifest and every conformance claim would then name two numbers, and a program depends on both anyway.
C. **One mark for the whole specification.** Simplest, but the library would hold the language back.

## Decision

A, as `spec/STABILITY.md` states it:

- **Marks.** *moving*: any change through the process. *candidate*: offered as stable; for thirty days only
  clarifications and soundness repairs, any other change withdraws the candidacy. *stable*: the candidate period
  passed without withdrawal.
- **Kinds of change**, named in each row as the register already does: *clarification* (no accepted program
  rejected, no outcome changed), *soundness repair* (rejects or faults only programs that could reach unsafe
  execution, with a decision record that shows the unsafe step), *additive*, *breaking (source)*.
- **Breaking a stable part** needs a decision record and a deprecation window: the old form keeps its meaning
  for at least two stable editions and ninety days, both implementations accept both forms, and the guide shows
  the new one.
- **Conditions for a candidate**: a green battery on the edition; the model sample (D-0219) agreeing with both
  implementations; no open priority-1 soundness finding; the last independent review's priority-1 findings
  repaired and checked; for *std* also a module map declared fixed, a conformance case for every exported item,
  and a full fuzzing budget for every security-relevant parser.
- **Where the marks go**: at the end of each register row from this decision on, after the evidence tier
  (`Stability: core moving, std moving`), so that the tools that read the register (`impl/tools/speccheck.py`,
  `impl/tools/manifest.py`) read it as before. A row without marks is read as both parts moving.
- **The register** in `spec/README.md` keeps the ten most recent editions; the earlier rows move, unchanged, to
  `spec/EDITIONS-HISTORY.md`.

## Rationale

The marks separate what changes at different rates. The candidate period gives a review and the model a fixed
target before anything is promised. Naming soundness repairs as their own kind keeps the safe-program guarantee
above stability (Master Instructions §8: preservation of invariants first) while making each such repair
checkable by a reader. Two numbers per claim (option B) buys little: a program is written against both parts
at once.

## Results

- `spec/STABILITY.md` 1.0.0; `spec/README.md`'s Stability paragraph; `spec/EDITIONS-HISTORY.md` with the 51
  earlier rows; `NAMING-POLICY.md`'s marks paragraph; the guide's introduction says the parts are moving and
  what stable will mean.
- The marks of this edition: core moving, std moving.
