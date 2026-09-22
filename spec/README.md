# CobaltC Specification — Reading Guide

This directory is the normative definition of CobaltC (Master
Instructions §13). Every artifact is at version 1.0.0 or later (each
carries its own Version and Change Log; `changes/` records every
normative change since 1.0.0); every entity `ACCEPTED`. This file is a
map, not a normative artifact.

## What is normative

Rules written in CFN (`01-metalanguage.md`) inside `03`–`21`, the
state definition in `04`, the grammar in `22`, the registries, and
the conformance suite. Prose explains; where prose and a rule differ,
the rule wins. Decision records (`decisions/`) are rationale, not
rules.

## Reading order for an implementer

1. `22-surface-syntax.md` — the grammar and the correspondence table
   from each production to its rule.
2. `04-abstract-state.md` — `Σ`, every component, every predicate.
3. `12-type-system.md` — static typing; then `06` §7 for
   representation and the implementation-defined parameters you must
   choose and document.
4. `13` → `05` → `14` → `15` — evaluation: results, `store`, statement
   and block exit, calls and `main`.
5. `08`, `07`, `16`, `09`, `10`, `11` — aliasing, resources and
   destructors, aggregates, references, escape checking, `let`.
6. `14` §6 — `rule.control.flow-analysis`: the exact static/dynamic
   boundary; your implementation must reject exactly the refuted
   instances.
7. `17`, `18`, `19`, `20`, `21` — modules, faults, threads, raw
   pointers and FFI, the standard library module `std` and its three library types.
8. `conformance.md` — every case with its derivation; `registry/
   diagnostics.md` — every diagnostic you must be able to report.

## What you must decide and document

Listed in `22` §5: `AddrWidth`, byte order, enum discriminant width,
struct padding, `dangling<T>()`, `fn` value encoding, mutex cells,
the reporting form of termination outcomes, calling convention/ABI.
`IMPLEMENTATION-NOTES.md` (non-normative) is the consolidated form of
this list, alongside every point of permitted nondeterminism and every
`unsafe` trust condition, each with its governing rule.

## What the language does not provide

`22` §5's "deliberate absences" and "restrictions". Do not add them
in an implementation; propose them through a `D-XXXX` decision.

## Provenance

`AUDIT.md` (first audit), `AUDIT-2.md` (second audit, rules-level),
`AUDIT-STATUS.md` (closure of both, and the 2026-09-20 consistency
pass recorded as `CHG-0009`, and the per-case derivation of the
conformance suite recorded as `CHG-0010`, and the end-to-end program
derivations recorded as `CHG-0011`). D-0015 was superseded by D-0018;
D-0018 and D-0019 are the decisions behind the 1.0.0 rewrite.
