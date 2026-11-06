# CHG-0310 — C strings, `NULL`, and trusted calls

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-16)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0252
Affects: `spec/20` §1 (1.18.0), `spec/21` §0, §3f (4.64.0), `spec/OVERVIEW.md`,
`spec/registry/diagnostics.md` (1.55.0), `spec/conformance.md`; `impl/std/ffi.cb`, `impl/std/text.cb`,
`impl/std/collections.cb`, `impl/std/std.cb`, `impl/src/prelude.rs`, `impl/src/typecheck.rs`,
`impl/src/interp/intrinsics.rs`, `impl/cobc/src/lower/intrinsics.rs`; seven conformance cases

## What changed

`std::ffi` (`CString`, `NulError`, `is_null`), `String::from_c`, `Vec::from_c_bytes`, the intrinsic
`null_rawptr<T>()` and `diag.null-pointer`. `[Trusted-Call]`: those two reading functions only inside `unsafe`;
neither they nor an `extern fn` as a `fn` value.

## What changed for programs

New names only; a program's own `CString`, `NulError`, `is_null` or `null_rawptr` shadows `std`'s. A program
that used an `extern fn` as a value was accepted before and failed when it ran; it is now rejected before it
runs.
