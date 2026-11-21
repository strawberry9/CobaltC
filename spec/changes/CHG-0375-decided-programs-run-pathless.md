# CHG-0375 — A program whose conflicts are all decided before the run keeps no record of references

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-26)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0284, D-0283, D-0282, D-0281
Affects: `spec/14` (1.32.0, `rule.control.frame-references`: element writes, the hidden reference of a compound
assignment, evaluation order through the whole value and a call's later arguments, *Decided programs*),
`spec/conformance.md` (3.275.0); `impl/src/typecheck.rs`, `impl/src/tier.rs`, `impl/src/interp.rs`,
`impl/src/lib.rs`; `impl/cobc/src/main.rs`, `lower.rs`, `lower/program.rs`, `lower/vec_calls.rs`,
`report.rs`; `impl/cbrt/include/cbrt.h`; six conformance cases; the guide's chapter P

## What changed

A program none of whose conflicts `rule.control.frame-references` or `rule.control.flow-analysis` leaves to the
run is *decided*, and may run with no record of references, with the same outcome. `cobc` builds every decided
program that way (`#define CB_PATHLESS 1` at the head of the generated C; every token 0, the runtime's path
entries constants or no-ops). `--all-checks`, `--lib` and `COBALTC_PATHLESS=0` keep the record. Under `-v`,
`cobc` says "lowered pathless", or "kept paths" with the file and line of the first conflict left to the run;
`--report-tracking` says the same before its listing.

The static pass now also treats these as the rule always meant:

- a write to a `Vec`'s element (`v[k] = e`) is an element access, against a live reference to another element;
- the hidden reference of `[Compound-Assign]` lives until the assignment ends, also when its target contains a
  call (`w[f()] += w[0]`);
- evaluation order looks through the whole assigned value: struct and array literals, branches, blocks and
  `let` (`v[0] = P { .x = grow(&mut v) }` is refuted, `diag.stale-binding`);
- a later argument that moves or assigns what an earlier reference argument reaches, or reassigns a reference
  an earlier argument read, leaves the call to the run (the program is not decided);
- a borrow given to a type parameter is a reference argument of the call, with the borrow's mode
  (`String::append(&mut s, &s)` is refuted, `diag.aliasing-conflict`);
- a reference to a reclaimed object in the program's own code leaves the program to the run.

`cobc`'s dual-body dispatcher treats a reference argument as a fresh borrow only when no later argument contains
a call, a block or an assignment; before, `set(&mut v[0], grow(&mut v))` skipped the callee's checks and wrote
into freed storage.

## What changed for programs

A decided program's outcome is unchanged and it runs faster (no path tables). A program of the forms above
faults where the specification says, under both tools, instead of writing through a stale reference
(`cobc`) or (for the refuted form) is rejected before the run.

## Conformance

New: `conf.later-argument-moves-element-dynamic`, `conf.later-argument-replaces-vec-dynamic`,
`conf.compound-call-target-reads-same-element-dynamic`, `conf.vec-element-write-beside-held-element-dynamic`,
`conf.argument-reassigns-view-then-destroys-dynamic`, `conf.assign-value-grows-container-rejected`. `conf.string-append-self-conflict` is now refuted before the run.
