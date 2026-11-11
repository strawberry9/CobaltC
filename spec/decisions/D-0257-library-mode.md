# D-0257 — Library mode: CobaltC functions that C calls

Status: ACCEPTED (2026-10-17, the owner: "go with your recommendations and fold the corrections into the
records"; implemented as `CHG-0319`)
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0146, D-0202, D-0204, D-0250, D-0252, D-0255
Affects: `spec/15`, `spec/20` (the boundary in the other direction), `spec/22` (`extern fn` with a body),
`spec/21` (`Vec::from_raw`, `copy_to_raw`), `spec/IMPLEMENTATION-NOTES.md`; `impl/cobc` (`--lib`, entry
stubs, a generated header), `impl/cbrt` (start-up on first call, foreign threads); the guide. The full text
is `private/D-0257-library-mode-proposal.md`.

## Problem

CobaltC calls C, but C cannot call CobaltC: `cobc` builds only whole programs. So CobaltC code cannot be one
checked component inside an existing C or C++ system, which is how a safe language most often enters systems
code, and the route by which it would later reach kernels with C keeping the parts it cannot.

## Options

Marking an entry point: `extern fn` with a body (chosen); `export extern fn` (`export` already means visible
to other modules); an attribute (CobaltC has none). A fault inside a call: report and abort by default with
`--contain-faults` as the opt-in (chosen); always contained; abort only. Output: a static library of the
program's objects with the runtime as its own archive linked once, plus a generated header (chosen); the
runtime merged in (two libraries in one program would carry two runtimes); a shared library (later: a
runtime loaded twice needs its own decision).

## Decision

An `extern fn` with a body is a function C calls by that name with the platform's C convention; its
parameters and result are FfiTypes (`spec/20` §3), it is not generic, and its name must not collide with
the runtime's `cb_` symbols. Every dynamic check stays inside the library; nothing tracked crosses outward.
`cobc --lib -o libx.a x.cb` writes the program's objects, copies `libcbrt.a` beside them, and writes `x.h`
with the prototypes, the ffi-structs (D-0250 layouts, `_Static_assert`s) and the link line; a library with a
`main` is refused. The runtime starts on the first call (or `cobalt_init()`; `cobalt_shutdown()` flushes and
frees it) with no process-wide side effects (no `SIGPIPE` or console change). A C thread is registered on
its first call, its state released by a thread-exit hook, and counts as outside the runtime between calls
for `[Deadlock]`. A fault reports and aborts; with `--contain-faults` each entry point has a `_try` form,
`int name_try(args…, Ret *result)`, returning a nonzero fault code and leaving `*result` alone, the message
readable with `cobalt_last_fault`. `std` gains `unsafe Vec::from_raw<T>(rawptr<T>, usize)` and
`unsafe copy_to_raw<T>(slice<T, shared>, rawptr<T>)`. Callbacks from C are not in this decision.

## Results

`cobc/tests/library.rs`: a C program under GCC and Clang calls a library with integers, floats, structs by
value (nested), buffers through `Vec::from_raw` and `copy_to_raw`, a C string through `String::from_c`,
`printf` from inside, a fault contained by a `_try` form (status 1, the result untouched, the report from
`cobalt_last_fault`) and the library used again after it, the plain form's fault ending the process, and
four C threads calling in at once; the same library cross-built for Windows and called from a MinGW C
program with four Windows threads under Wine. As implemented, the `_try` forms are always generated beside
the plain ones, so no `--contain-faults` option exists: a host chooses per call. The library's output is
written as each call returns; a C caller keeps its own stdio in order by flushing it (or switching
buffering off) before calling in. The bound `plain` was needed for `Vec::from_raw`/`copy_to_raw` to admit
only bytes that mean the same anywhere. Eight conformance rows (`CHG-0319`). The showcase library
`showcase/tier14/digest` (SHA-256, HMAC, CRC-32, base64 for C, with its `Makefile`, C demo and `unittest`
blocks) found the header naming every `_try` result parameter `out`, clashing with an entry's own (now
`result`), and `CHG-0321`.

## Revisit conditions

A shared-library output; callbacks (`extern fn` values); several CobaltC libraries whose entry points share a
name.
