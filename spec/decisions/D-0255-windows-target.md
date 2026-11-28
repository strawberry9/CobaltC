# D-0255 — `cobc` for Windows: `--target x86_64-pc-windows-gnu`, and `cobc.exe`

Status: ACCEPTED (2026-10-16, the owner: "stick with D-0255 and MinGW, install the packages and implement it";
x86-64 only: "lets stick with x64-64 for the moment")
Kind: Decision (`spec/02-schema.md`)
Supersedes: D-0234's WI-14 in its Windows half (`cobc` on Windows, declined for the 2026 series on 2026-10-15)
Depends on: D-0092, D-0146, D-0250, D-0251, D-0254
Affects: `spec/IMPLEMENTATION-NOTES.md` §5; `impl/cobc` (`target.rs`, `main.rs`, `link.rs`, `build.rs`,
`Cargo.toml`, `lower/functions.rs`, `lower/closures.rs`, `lower/intrinsics.rs`), `impl/cbrt` (`link_info.rs`,
`stamp.rs`, `build.rs`, `lib.rs`), `impl/src/fileio.rs`, `impl/conformance/run.py`; README; `CHG-0313`

## Problem

`cobc` compiled for x86-64 Linux only; the workspace type-checked for Windows, but nothing produced a Windows
program, and `cobc` did not run there.

## Options

The C compiler for Windows: MinGW-w64 GCC (chosen), Clang targeting MSVC (later, needs the Windows SDK and
an LLVM that no longer runs on Windows 7), or `cl.exe` (cannot compile the GNU C `cobc` writes). Also
discussed and set aside: a bundled compiler (TinyCC lacks vector types, overflow builtins and thread-local
storage; a GCC can be packaged with a release instead), `zig cc` (needs Windows 10; usable as a cross
compiler with `--cc`), a code generator inside `cobc` (a rewrite), and an independent implementation from
the specification (a project for validating the specification, not the route to Windows). AArch64 and other
architectures: later.

## Decision

`cobc --target TRIPLE`, `x86_64-unknown-linux-gnu` or `x86_64-pc-windows-gnu`, default the host's. The target
decides the runtime and where it is, the system libraries, the executable's name (`.exe`), the default C
compiler (`x86_64-w64-mingw32-gcc` from Linux, `gcc` on Windows) and whether `-rpath` is passed. A foreign
target's program runs under `COBALTC_RUNNER` (`wine64`) for `--run` and `--explore`. The Windows runtime is
built with Rust 1.77.2. `cobc.exe` is the same code built for `x86_64-pc-windows-gnu`; on Windows `cobc` does
not link the runtime crate, and its link knowledge and source stamp come from files shared with it
(`cbrt/src/link_info.rs`, `cbrt/stamp.rs`). Both targets have the same address width, byte order and layouts,
so no rule of the specification depends on which.

## Results

What running the conformance suite under Wine found, each fixed:

- **Arguments.** On Windows `argv` is in the ANSI code page. The runtime now reads the UTF-16 command line,
  as `coby` does. `conf.arg-invalid-utf8` is now `platform: unix`: no Windows argument holds the byte 0xFF.
- **Binary streams.** The C library's standard streams were in text mode, so C code a program links wrote
  `\r\n`. They are now binary, as the runtime's own are.
- **`copy_file`'s count.** Rust's `std::fs::copy` reports it from progress calls that Wine does not make. It
  is now taken from the copy's size.
- **Thread argument blocks** were only 16-aligned (`calloc`). This was true on Linux too, where an
  `align(64)` argument was aligned only by luck. They are now allocated with their struct's alignment.
- **Over-aligned parameters.** MinGW-w64's GCC does not align the copy the Windows x64 convention passes for a
  struct parameter aligned beyond 16 bytes, nor the frame of a function that calls `setjmp`. Such a parameter
  is now copied into an aligned local at entry, and a thread's call is made from a function of its own.

Two cases (300 threads) are killed by the OOM killer under Wine 6, which makes each thread's whole reserved
64 MB stack resident; a C program creating the same threads is killed the same way. They are for the check on
real Windows. The Windows runtime needs Rust 1.77.2: a later standard library imports
`bcryptprimitives.dll`, which neither Windows 7 nor Wine 6 has.

## Revisit conditions

Other architectures (AArch64 first); the MSVC target through Clang; a release archive bundling a MinGW-w64 GCC.
