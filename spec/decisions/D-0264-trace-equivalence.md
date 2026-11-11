# D-0264 — Trace equivalence: the two tools' destructor and outcome events compared

Status: ACCEPTED (2026-10-17, the owner: "go ahead with the verification plan's remaining steps … autonomously")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §15, §20
Depends on: D-0008, D-0188, D-0189, D-0200, D-0209, D-0210, D-0261, D-0263
Affects: `impl/src/interp.rs`, `impl/src/interp/destroy.rs`, `impl/src/lib.rs`, `impl/cbrt/src/lib.rs`,
`impl/tools/trace_compare.py`, `impl/battery.sh`, `README.md`; `CHG-0328`

## Problem

The compiler must match the interpreter byte for byte on every program's output and diagnostic, and it
does, on 1,029 cases. But the compiler also elides: checks it proves cannot fail, objects it proves no
check will ask about (direct and frozen parameters, objectless locals, confined vectors — D-0188, D-0189,
D-0200, D-0209, D-0210). Each elision is a proof made at a site and never checked afterwards: the
no-elision build (`--all-checks`) compares results, and a wrongly elided step that leaves the output alone
is invisible. What an elision must not change is the sequence of rule steps the program takes between
outputs — above all the order in which destructors run and where the program faults, which are the steps
whose timing the elided bookkeeping governs. The plan toward proofs (B3) asked for that sequence to be
compared.

## Options

- **A. Compare every rule step.** The interpreter can print its steps; the compiled program takes most of
  its steps inside the C, with no runtime call, and the elided ones take none at all. Modulo elision, a
  full trace is not comparable.
- **B. Compare the events both tools take as runtime steps and no elision may remove: every user
  destructor run, the program's fault, an `exit`, the end.** Cheap, exact, and precisely the steps whose
  order the elisions could disturb. **Selected**, as a first version.
- **C. Compare lock and unlock events too.** Deferred: locks live in `std` bodies the compiler lowers
  natively (`cb_lock_bare`), and multi-threaded traces interleave.

## Decision

`COBALTC_TRACE_EVENTS=1` makes both tools write one `EV` line on standard error per event: `EV drop T`
when a user-written destructor (`T::drop`, not `std`'s) begins, `EV fault diag.x` when the program's
fault is raised, `EV exit n` for `exit` (and then no `end`), `EV end n` for a normal end. `impl/tools/trace_compare.py` runs
every single-threaded conformance case under both tools with the variable set and compares the
sequences, a type named by its last segment; the battery runs it in stage 1. A difference is a defect of
the compiler's lowering (an elision that changed when something ends) or of one tool's rules.

## Results

Over the 899 single-threaded conformance cases (one skipped as unsupported by the interpreter), 898
agree on every event and none differ: 702 events compared, under GCC. Getting there taught the
comparison three things about where an event belongs, each an instrumentation point moved, not a
difference in the rules: the interpreter's fault event must be written at the fault's first raise (the
runtime writes it before its unwind runs the destructors), its destructor event at both places a user
destructor is called (a value inside raw memory, an `Rc`'s or a channel's, takes the second), and the
overwrite fault (`[Write-Resource-Overwrite-Rejected]`) is raised before the pending temporary it ends.
A fault the checker folds from a literal (`dynamic` by its row, raised by the compiled program) is written
by the interpreter at its run's end. A local type's name (`Tracker$4` and `Tracker_4`) compares equal.
The runtime's Miri smoke test (`cbrt/tests/miri_smoke.rs`, plan B4) drives a statement's steps through the
tables under Miri with no undefined behaviour reported; the network wake-up and the stack-floor probe step
aside under `cfg!(miri)`.

Threaded cases (2026-10-18). A case that spawns is compared as a multiset: the interleaving is not the
rules' to fix, but what runs is — the same destructors the same number of times, the same fault, the same
end. All 128 cases of `spec/19` agree. The one difference the extension found was the interpreter's: it
wrote the fault event for a thread's contained fault (`[Fault-Contain]`, D-0204), which is the thread's
result and not the program's fault, and the once-per-process rule then hid the program's own fault behind
it; the event is now written only for a fault that is not contained, as the runtime's `fault` does.
Over every case, one more: an `exit` from a spawned thread, where the interpreter wrote `EV exit` and then
`EV end` for the run it ended, and the runtime `EV exit` alone (its process ends from the thread). An
`exit` now writes its own event and no `end` in both tools: `[Terminate-Exit]` is not a normal end.

## Revisit conditions

Lock and channel events (C above), with per-thread sequences keyed by spawn order so that threaded
cases compare; a count of checks performed, compared under `--all-checks` against the interpreter's.
