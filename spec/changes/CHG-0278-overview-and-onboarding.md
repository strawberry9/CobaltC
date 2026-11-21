# CHG-0278 — The semantics overview, and the onboarding documents brought up to date

Kind: Change record (`spec/02-schema.md` §2); non-normative
Status: ACCEPTED (2026-10-14; the 2026-10-12 plan's WI-19 and WI-20, Q14–Q16 as recommended)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0233 (what writing the overview found)
Affects: new `spec/OVERVIEW.md`; `spec/README.md`, `spec/IMPLEMENTATION-NOTES.md`, `README.md`,
`impl/STATUS.md`, new `impl/CHANGELOG.md`, `impl/COBC-PLAN.md`, `impl/model/README.md`,
`showcase/README.md`; `impl/tools/speccheck.py`; the guide's introduction; `WHERE-TO-NOW.md` (removed)

## What changed

- **`spec/OVERVIEW.md`** (WI-19, Q14) is the semantics in prose for an implementer or a reviewer who knows
  systems languages. It covers:
  - how to read the specification;
  - the abstract state, and how paths are formed and end;
  - `clash`, and why ancestors are excluded;
  - the static/dynamic boundary;
  - moves, destruction and `solitary`;
  - threads, and the trust boundary;
  - what `[Standing-Local]` lets an implementation elide;
  - the safe-program theorem's hypotheses, and what the executable model covers.

  It is non-normative, and every paragraph cites the rules it restates. `speccheck.py` now reads it:
  every label, decision, change record, diagnostic and entity id it cites must exist. `spec/README.md`'s
  reading order places it after the schema, and the guide's introduction points to it.
- **`impl/STATUS.md`** (WI-20, Q16) is now a current-state page: the workspace, what each tool does, its
  options and limits, the platforms, and how to build and test. The dated log it was moves unchanged to
  `impl/CHANGELOG.md`, with a one-line index of every D-record at its top; references to the log's
  sections now point there.
- **`WHERE-TO-NOW.md`** (Q15), a local file outside the repository, is removed. Its §2, the plan for `cobc`
  on Windows, is kept with WI-14's working files, since WI-14 is on hold.
- **`spec/IMPLEMENTATION-NOTES.md`**:
  - Its Purpose no longer says that the implementations are still to be authorised.
  - A new §5 records the choice each tool made for every implementation-defined point, with the place in
    the source.
  - §2 and §3 follow D-0233.
  - Recording the choices found that both tools returned the address 1 from `dangling<T>()`, which is not
    `T`-aligned for a `T` wider than a byte. Both now return `alignof<T>()`. A safe program cannot turn a
    raw pointer into a number, so the change is visible only across a C call.
- **`README.md`** names `impl/CHANGELOG.md`, `spec/OVERVIEW.md`, `spec/STABILITY.md`, `impl/model/`,
  `impl/fuzz/` and `impl/conformance/run.py`. Editor support (WI-10) has no directory yet.

## What changed for programs

Nothing, apart from `dangling<T>()`'s address, which only C code can see.
