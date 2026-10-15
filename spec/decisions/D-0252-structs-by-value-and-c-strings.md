# D-0252 — Structs by value across `extern`, and C strings

Status: ACCEPTED (2026-10-16, the owner: "go with the recommendations and implement it", on
`private/D-0252-c-interop-proposal.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (std admission), §17 (External Interfaces)
Depends on: D-0017, D-0030, D-0146, D-0250, D-0251, D-0136
Affects: `spec/20` §1 `[Trusted-Call]`, §3 `FfiType` (1.18.0), `spec/21` §0 and §3f (4.64.0), `spec/16` (1.30.1),
`spec/OVERVIEW.md`, `spec/registry/diagnostics.md` (1.55.0), `spec/conformance.md`; `impl/src/typecheck.rs`,
`impl/src/ffi.rs`, `impl/src/interp/intrinsics.rs`, `impl/cobc/src/lower/intrinsics.rs`, `impl/std/ffi.cb`,
`impl/std/text.cb`, `impl/std/collections.cb`, `impl/std/std.cb`, `impl/src/prelude.rs`; `CHG-0309`, `CHG-0310`

## Problem

A C function that takes or returns a struct needed a C wrapper that split it into scalars, although D-0250 had
made the bytes agree. Text for C needed a NUL byte after it, and C's `char *` results a scan and a copy, which
every program wrote for itself; and nothing could make or test for C's `NULL`.

## Options

Part 1, structs: (A) plain structs of FFI types by value; (B) the same behind a marker; (C) no change.
Part 2, text to C: (A) a `CString` resource in `std::ffi`; (B) a `Vec<u8>` with a NUL; (C) a recipe.
Reading C's text: (R-A) `unsafe fn` in the language; (R-B) `std` functions on the trusted-call list; (R-C) a
recipe. Further choices: coby refuses struct-by-value calls or classifies structs itself; `Vec::from_c_bytes`
for text that is not UTF-8; `null_rawptr`/`is_null` and a null fault; the module name; interior NUL as `Err`.

Selected: every recommendation — 1A, coby refuses, 2A, R-B, `Vec::from_c_bytes` included,
`null_rawptr`/`is_null` with `diag.null-pointer`, `std::ffi`, `Result<_, NulError>`.

## Decision

- `FfiType` gains `ffi-struct`: a struct, not a `bitstruct` or a resource, with at least one field, each an
  FfiType other than `void`, an `ffi-struct`, or `array<τ, N>` (N > 0) of one. Enums, empty structs and arrays
  as parameters or results stay out. `diag.extern-non-ffi-type`'s message names the field path to the first
  part that does not cross.
- `std::ffi`: `CString` (`new`, `from_bytes`, `ptr`, `len`, `as_bytes`), `NulError`, `is_null<T>`.
  `String::from_c` (in `std::text`) and `Vec::from_c_bytes` (in `std::collections`, where
  `[Assoc-Fn-Foreign-Type]` puts them) copy C's text; a null pointer faults with `diag.null-pointer`.
- `null_rawptr<T>()`: a safe intrinsic, address 0.
- `[Trusted-Call]` (`spec/20` §1): the two reading functions are called only inside `unsafe`, as a dereference
  is, and neither they nor an `extern fn` are ever `fn` values, whose calls need no `unsafe`.

Two departures from the draft's text, neither a change of choice: `is_null` is an ordinary generic function of
`std::ffi` (`p == null_rawptr<T>()`) rather than a second intrinsic, and `CString` is a plain struct holding a
`Vec`, a resource through its field as `String` is.

## Results

`cobc` needed no change for structs by value: it already declares the callee with the struct's C type, so the
C compiler applies the platform's convention. `coby` refuses a struct argument or result before calling
(`unsupported`), as it refuses raw pointers. Making the trusted-call rule found an older fault: an `extern fn`
used as a `fn` value was accepted, then failed when run (`diag.unbound-name` in `coby`, an internal error in
`cobc`); it is now rejected (`conf.extern-fn-value`). Thirteen cases: struct by value through C code and
libc's `div` (`cobc` only, GCC and Clang), five rejections, and seven for `std::ffi`, among them a libc case
(`strlen`, `strchr`, `getenv`, `puts`).

## Revisit conditions

C-shaped enums, if ever added, join FfiType. If `unsafe fn` is accepted (R-A), the two trusted calls become
such functions and the list goes.
