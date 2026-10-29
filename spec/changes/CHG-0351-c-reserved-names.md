# CHG-0351 — Names C keeps, given to C

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: none
Affects: `spec/conformance.md` (one row); `impl/cobc/src/lower.rs` (`san`, `C_RESERVED`)

## What changed

No rule changed. `cobc` writes a local, a function or a type into C with a prefix (`l_x`, `f_main`,
`s_Point`), but a struct's field under its own name, so a field named as one of C's keywords (`int`,
`float`, `char`, `long`, `switch`, `union`, …) or as a macro the generated C's headers or the C compilers
define (`NULL`, `INFINITY`, `M_PI`, and `linux` and `unix`, which GCC and Clang predefine in GNU mode) made
the C compiler refuse the program. These are ordinary CobaltC names. `san`, through which every name
`cobc` writes passes, now gives such a name one more `_` (`int` is `int_`), as it does that name followed
by `_`s (`int_` is `int__`), so that two names never meet. A function C calls keeps its own name: its
symbol is an assembler label. Found by `cbparse` (CHG-0350), whose tokens had fields `int` and `float`.
The new row `conf.c-reserved-field-names` covers them in a literal, a read, a compound assignment, a
destructuring and a variant's payload.

## What changed for programs

Programs that `cobc` could not compile now compile and run as under `coby`.
