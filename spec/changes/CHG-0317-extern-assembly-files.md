# CHG-0317 — Assembly files as foreign code

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §5, §21
Depends on: D-0030, D-0146, D-0258 (part C)
Affects: `spec/conformance.md` (one row); `impl/cobc/src/link.rs`; the guide (§20 "Your own C code")

## What changed

No rule changed. `rule.trust.extern-code` leaves which strings `extern "…";` accepts to the implementation,
which must document them. `cobc` accepted `.c`, `.o`, `.a` and `.so` files and library names; it now also
accepts assembly files, `.S` (run through the C preprocessor, so `#ifdef _WIN32` chooses the platform's
calling convention in one file) and `.s` (plain), each assembled by the C compiler as a `.c` file is compiled,
with the same options. Their functions are declared and called as any `unsafe extern fn`. This is the route
for instructions C cannot express: port I/O, `cpuid`, `rdtsc`, descriptor-table loads, context switches.
`coby` accepts the declaration and refuses the first call into it, as for C code ("which only cobc links").
New row `conf.extern-code-asm-file`; the guide's table of foreign code gains the row.

## What changed for programs

A program may name an assembly file where it could name a C file. Nothing else changes.

## Found by

D-0258 (the low-level pieces), part C, 2026-10-17.
