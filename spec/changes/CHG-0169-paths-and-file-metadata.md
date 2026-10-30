# CHG-0169 — Paths and file metadata

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0141)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0141
Affects: `spec/21` (4.7.0) §0, §2e′; `spec/conformance.md` (3.131.0); `impl/conformance/README.md`; the guide §21; `impl/std/sys.cb`, `impl/std/std.cb`, `impl/src/fileio.rs`, `impl/src/interp.rs`, `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`, `impl/cbrt/build.rs`, `impl/cobc/src/lower.rs`, the four conformance runners

## What changed

- **`spec/21` §2e′ `rule.stdlib.fs`:** `[Path-Join]`, `[Path-Parts]`, `[Path-Normalize]`, `[Path-Canonical]`, `[File-Info]`, `[Copy-File]`, `[Dir-All]`, `[Set-Current-Dir]`, `[Temp-Dir]`, `[Home-Dir]`; the prose on the platform's path grammar; the listing.
- **`spec/21` §0:** `FileInfo` among the types; the `std::sys` rows.
- **`std`:** the functions in `impl/std/sys.cb`, written in CobaltC over the new private primitive `path_op` and the private helpers `byte_op`, `byte_answer`, `bytes_ptr`, `code_error` (`impl/std/std.cb`), which the process and networking facilities share.
- **Both tools:** `path_op` in `impl/src/fileio.rs` over Rust's `std::path` and `std::fs`; `coby` routes the three primitives of its shape through one dispatch that releases the GIL while the call runs; `cbrt` exports `cb_path_op` (and `cb_proc_op`, `cb_net_op`, `cb_os_random`); `cobc` lowers the four to those names; `cbrt`'s source stamp covers the new shared files.
- **Conformance runners:** the `platform:` header, honoured by `cb_conformance_suite`, `spec_rows`, `compiled_suite` and `compiled_spec_rows`.
- **Rows:** `conf.path-join`, `conf.path-parts`, `conf.path-normalize`, `conf.file-info`, `conf.copy-file`, `conf.dir-all`, `conf.set-current-dir`, `conf.path-canonical`, `conf.temp-home-dir`.

## Compatibility classification

Additive: new names in `std`, shadowed by a program's own.
