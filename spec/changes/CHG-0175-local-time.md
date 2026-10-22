# CHG-0175 — Local time: `local_offset_seconds`, `DateTime::to_local`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0147)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0147
Affects: `spec/21` (4.12.0) §0, §2j; `spec/conformance.md` (3.138.0); the guide §21; `impl/std/time.cb`, `impl/std/std.cb`, `impl/src/fileio.rs`, `impl/src/interp.rs`, `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`, `impl/cobc/src/lower.rs`

## What changed

- **`spec/21` §2j `rule.stdlib.time`:** `[Local-Offset]` (`local_offset_seconds(i64 unix) : i64`, the platform's offset from UTC at that moment, daylight saving included, −18 h ..= +18 h; for a moment the system has no rule for, the offset now) and `[To-Local]` (`DateTime::to_local`, `from_unix(unix(t) + offset)`, checked; `diag.invalid-datetime` on overflow); the prose no longer says there are no time zones, and says a local value is zone-less, printed by its fields, never by `to_iso`.
- **`spec/21` §0:** the submodule table and the `std::time` table gain the two functions.
- **`std` (`impl/std/time.cb`):** `local_offset_seconds` over a new std-private primitive `tz_offset(i64 unix) : i64` (`impl/std/std.cb`), realized in `impl/src/fileio.rs` for both tools: `localtime_r` on Unix (the offset computed from the broken-down fields), `GetTimeZoneInformationForYear` and `SystemTimeToTzSpecificLocalTime` on Windows; `DateTime::to_local` in CobaltC.
- **Rows:** `conf.local-offset-range`, `conf.to-local-consistent`.

## Compatibility classification

Additive: two exported functions in `std::time`, shadowed by a program's own.
