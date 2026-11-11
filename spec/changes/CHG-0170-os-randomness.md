# CHG-0170 — `os_random_bytes`, `os_random_u64`, `Rng::from_os`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0142)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0142
Affects: `spec/21` (4.8.0) §0; `spec/registry/diagnostics.md` (1.44.0); `spec/conformance.md` (3.132.0); the guide §21; `impl/std/random.cb`, `impl/std/std.cb`, `impl/src/osrand.rs` (new), `impl/src/interp.rs`, `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`, `impl/cbrt/build.rs`, `impl/cobc/src/lower.rs`

## What changed

- **`spec/21` §0:** `[Os-Random]` and `[Rng-From-Os]` beside `[Rng]`; a row in the `std::random` table; the submodule table lists the two functions.
- **`spec/registry/diagnostics.md`:** `diag.entropy-unavailable`.
- **`std` (`impl/std/random.cb`):** the three functions, written in CobaltC over the std-private `os_random` (`impl/std/std.cb`).
- **Both tools:** `os_random` in `impl/src/osrand.rs`; `coby` dispatches it, `cbrt` exports `cb_os_random`, `cobc` lowers to it; `bcrypt` in `cbrt::LINK_LIBS` on Windows.
- **Rows:** `conf.os-random-u64`, `conf.os-random-bytes`, `conf.os-random-empty`, `conf.rng-from-os`.

## Compatibility classification

Additive: three new names in `std`, shadowed by a program's own.
