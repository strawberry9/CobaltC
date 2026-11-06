# CHG-0319 — Library mode: functions C calls, `cobc --lib`, the `plain` bound

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §5, §9, §21
Depends on: D-0146, D-0204, D-0250, D-0252, D-0257
Affects: `spec/22` §3 (`extern-def`, `bound`), `spec/20` §3 (`[Extern-Entry]`), `spec/12` §1 (the bound
`plain`), `spec/21` (`Vec::from_raw`, `copy_to_raw`), `spec/IMPLEMENTATION-NOTES.md` (`cobc --lib`, the
header, the runtime at the boundary), `spec/conformance.md` (eight rows), the diagnostics registry (notes);
`impl/src/{ast,parser,typecheck}.rs`, `impl/src/interp/exec.rs`, `impl/cobc` (`--lib`, `lower/header.rs`,
`lower/program.rs`, `main.rs`), `impl/cbrt` (`cb_lib_*`), `impl/std/{collections,ffi}.cb`; the guide

## What changed

**`extern fn name(τ1 p1, …) : τr { … }`** (`spec/22` §3 `extern-def`) defines a function C calls by that
name, with the platform's C convention: `[Extern-Entry]` (`spec/20` §3). Its parameters and result are
FfiTypes (`diag.extern-non-ffi-type` otherwise), it is not generic and belongs to no type (syntax errors),
its name is not the runtime's (`cb_…`, `cobalt_…`) and no other entry point's (`diag.duplicate-item`; C has
one name space). Inside the program it is an ordinary function. Without a body, `extern fn` is still the
D-0146 error. A program that defines entry points and no `main` is a library: both tools check it,
`--unittest` runs its tests, `coby` has nothing to run (`unsupported`), and `cobc` builds it only with
`--lib`; a program with a `main` is built without it.

**`cobc --lib -o libx.a x.cb`** writes the program's objects (and those of its own C and assembly files) as
a static archive, copies the runtime `libcbrt.a` beside it (two libraries in one program link it once),
and writes `x.h`: each ffi-struct the entry points use as C declares it, with its D-0250 layout asserted,
the library's API (`cobalt_init`, `cobalt_shutdown`, `cobalt_last_fault`), and each entry point's prototype
with its `_try` form. The runtime starts on the first call, with none of a program's process-wide effects;
the first thread to enter is its main thread, every other C thread is registered on its first call and
released when it ends. A fault inside `name(…)` reports and ends the process, as in a program; inside
`name_try(…, out)` it unwinds the call's frames, keeps its report for `cobalt_last_fault`, and returns 1
with `*out` untouched. The library's output is written out as each call returns (a C caller that buffers
its own stdio flushes it, or switches buffering off, to keep the two in order).

**The bound `plain`** (`spec/12` §1): bytes that mean the same anywhere — a number, `bool`, a raw pointer, a
`simd`, or an array, struct or enum of those; no resource, reference, slice, handle, mutex, guard,
function value or `str`. It implies `clone`; no operator bound implies it.

**`std`:** `unsafe Vec::from_raw<T: plain>(rawptr<T> p, usize n) : Vec<T>` and `unsafe copy_to_raw<T: plain>
(slice<T, shared> s, rawptr<T> p)` (`[Trusted-Call]`s: the caller vouches for the memory), for buffers a C
caller owns.

## What changed for programs

A new item form, a new contextual word in a bound, two `std` functions. No existing program changes.

## Found by

D-0257 (library mode), 2026-10-17. Tested by `cobc/tests/library.rs` (a C program under GCC and Clang:
values, structs by value, buffers, strings, four C threads, a fault under both forms), the cross-built
library with a MinGW C program under Wine, and the eight rows.
