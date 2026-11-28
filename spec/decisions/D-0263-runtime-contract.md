# D-0263 — The runtime's contract: every entry point's rule steps and assumptions, in its header

Status: ACCEPTED (2026-10-17, the owner: "go ahead with the verification plan's remaining steps … autonomously")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §15, §20, §24
Depends on: D-0206, D-0207, D-0261, D-0262
Affects: `impl/cbrt/include/cbrt.h`; `spec/IMPLEMENTATION-NOTES.md` §6; `impl/tools/runtime_contract.py`;
`impl/battery.sh`; `CHG-0327`

## Problem

A compiled program is C that calls the runtime at the points where the rules take their checked steps:
about 120 entry points in `cbrt.h` for frames, objects, paths, reference data, raw storage, values in
flight, threads and the program's life. Which rule steps each one performs, and what it assumes its
caller (`cobc`'s lowering) has established, lived in comments of varying depth and in the lowering's
code. The verify mode (D-0261) checks the runtime's tables against themselves; nothing stated what each
call promises, which is what a refinement argument (the runtime refines `spec/04`'s state and `spec/05`–
`21`'s rules) and the trace-equivalence check (plan B3) need as their specification.

## Options

- **A. A hand-written table in the implementation notes.** Drifts from the header as entry points come
  and go.
- **B. The header is the contract; a tool renders and checks it.** Every declaration's comment names the
  rule steps it performs (`[Rule]` labels the spec defines) and what it assumes, or says `internal`; the
  tool writes the table into the notes and fails the battery when the table is stale, a declaration of a
  rule-performing group names no rule, or a label no artifact defines. **Selected.**
- **C. Contracts as assertions in the runtime.** Costs every program; the verify mode is where runtime
  assertions live, and it checks states, not promises.

## Decision

`impl/cbrt/include/cbrt.h` carries, on each declaration of the groups that perform rule steps (program,
frames and statement scopes, objects, access paths, reference data, raw storage, values in flight,
threads and mutexes), the rule labels of the steps it performs and the facts it assumes of the generated
C; the standard library's externs name the `std` rule they implement; pure helpers and bit operations
are their own groups. `impl/tools/runtime_contract.py` reads the declarations and their comments into
`spec/IMPLEMENTATION-NOTES.md` §6 and, with `--check` (battery stage 1), verifies the table is current,
every checked declaration names a rule or is marked internal, and every label named is defined by a spec
artifact. The generated programs include the header verbatim, so the contract travels with every
program's C.

## Results

193 declarations, 123 in the checked groups, every one with a rule or marked internal, no unknown label.
Writing the contract named, for the first time in one place, which calls check premises without taking
a step (`cb_read`, `cb_write`, the `_unminted` and `_check` borrows, `cb_frozen_ok`, `cb_direct_ok`),
which take the step (`cb_borrow`, `cb_bind`, `cb_take`, `cb_destroy`, the frame and scope pushes and
pops) and which are internal to the lowering's optimisations (the fast-path gates, dual-body dispatch,
objectless locals): the three kinds a trace-equivalence check must treat differently.

## Revisit conditions

The trace-equivalence check (plan B3), which would make each contract line a thing a test compares; an
entry point whose contract cannot be stated in a line, which is a sign it does two things.
