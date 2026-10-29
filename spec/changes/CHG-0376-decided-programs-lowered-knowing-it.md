# CHG-0376 — A decided program is lowered knowing it is decided

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-26)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0285, D-0284, D-0281
Affects: `impl/cobc/src/main.rs`, `lower.rs`, `lower/layout.rs`, `lower/passes.rs`, `lower/functions.rs`,
`lower/closures.rs`, `lower/program.rs`; `impl/cbrt/include/cbrt.h`; `impl/cobc/tests/driver.rs`; the guide's
chapter P; no change to the specification's rules

## What changed

`cobc` lowers a program that its first lowering finds decided (D-0284) a second time, knowing it. In that
lowering a value that holds a reference inside it (a view, an `Option<ref<T>>`) is plain data: it has no runtime
object, its copies copy no holds, and it is passed and returned as its bytes alone. The C starts
`#define CB_PATHLESS 1` and `#define CB_DECIDED 1`; `cbrt.h` makes the datum flights, a frozen parameter's test
and a call's token end constants, and the statement scopes that would hold nothing are left out. When the
second lowering cannot lower the program or finds it not decided, the first lowering's C is used. `-v` says
"lowered pathless and decided"; `COBALTC_DECIDED=0` keeps the first lowering.

## What changed for programs

Nothing a program can observe: a decided program ends as before, faster (csvstat 95 → 73 ms, extsort 484 → 431
ms, GCC, best of five).

## Conformance

No row changes. A driver test checks the second lowering's C and outcome under both C compilers.
