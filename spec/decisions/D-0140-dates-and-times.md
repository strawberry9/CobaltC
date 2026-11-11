# D-0140 — `std::time`: UTC dates and times, and a millisecond wall clock

Status: ACCEPTED (2026-10-04, the owner, through `private/std-deployable-plan.md` §3 and §10.1: "Dates are UTC only, `DateTime` with `i64` year, `to_unix` faults on invalid fields, `from_iso` strict plus bare date, `unix_ms` added")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0061 (the clocks), D-0117 (`text` of an error), D-0134 (`std`'s naming conventions), D-0136 (`std`'s submodules)
Affects: `spec/21` §0, §2e′ `rule.stdlib.env`, §2j (new) (4.6.0), `spec/registry/diagnostics.md` (`diag.invalid-datetime`), `spec/conformance.md` (3.130.0), the guide §21, `impl/std/time.cb` (new), `impl/std/std.cb`, `impl/src/fileio.rs`

## Problem

`std` had the wall clock as one number, `unix_seconds()`. A deployed
program writes dates: in log lines, file names, HTTP headers, reports,
and reads them back from configuration and data. Turning seconds into a
year, month and day is a known algorithm that every program would
otherwise copy, and getting leap years, negative seconds or the
400-year rule wrong is easy. Log lines that need the milliseconds had
no clock that gave them.

## Candidate mechanisms

1. **Leave it to programs.** Each copies a `civil_from_days` and its
   bugs.
2. **A full date-time library** with time zones, a `Duration`, a format
   language. Time zones need the system's zone database, which differs
   per platform and changes by law; a format language is a second
   `printf`. Out of scope (the plan's out-of-scope list).
3. **A UTC calendar, `DateTime`, in a new `std::time`, written in
   CobaltC, with `to_iso`/`from_iso` for the one interchange format
   everyone reads, and `unix_ms()`.** Selected.

## Selected design

`spec/21` §2j, `rule.stdlib.time`:

- `DateTime { year : i64, month, day, hour, minute, second : u8 }`, all
  fields exported, the proleptic Gregorian calendar in UTC; year 0 is
  1 BC (ISO 8601's astronomical numbering). `year` is an `i64` so that
  `DateTime::from_unix` is total: every `i64` second names a moment.
- `DateTime::from_unix(i64) : DateTime` (H. Hinnant's
  `civil_from_days`), `DateTime::now()`, `DateTime::new(…) :
  Option<DateTime>` (`None` for fields that name no moment),
  `DateTime::to_unix(&t) : i64`, `DateTime::weekday(&t) : Weekday`,
  `Weekday::text(&w) : str`, `DateTime::to_iso(&t) : String`,
  `DateTime::from_iso(StringView) : Result<DateTime, ParseError>`,
  `DateTime::eq`, `DateTime::less` (fits `Vec::sort_by`), `unix_ms()`.
- **`to_unix` of a `DateTime` that names no moment is a fault**,
  `diag.invalid-datetime` (`[Invalid-DateTime]`, disposition
  `checked`): such a value can only be made by a struct literal or by
  changing a field, which is misuse, as an index out of bounds is. The
  same fault for a valid date too far from 1970 for an `i64` of seconds
  (beyond about ±292 billion years), which only a hand-built `DateTime`
  can be. `from_unix` and `to_unix` are inverses over every `i64`.
- **`weekday` never faults for a valid `DateTime`:** the Gregorian
  calendar repeats every 400 years, a whole number of weeks, so the year
  is reduced into 2000 ..= 2399 first.
- **`to_iso`** writes `YYYY-MM-DDTHH:MM:SSZ`; a year outside 0 ..= 9999
  is written with its sign and at least four digits (`-0001`,
  `+10000`), ISO 8601's expanded form.
- **`from_iso` accepts exactly what `to_iso` writes, and a bare date**
  (`YYYY-MM-DD`, read as its midnight): a year of four digits, or a sign
  and four or more; no lower-case `z`, no offset, no fractional
  seconds, no second 60. Errors reuse `ParseError`: `Empty`,
  `Invalid(i)` at the first byte that does not fit (the length when the
  text stops short), `OutOfRange` for fields that name no moment (a
  month 13, `2024-02-30`, `:60`) or a year beyond `i64`. Written by hand
  over the view's bytes: no `split`, nothing allocated.
- **`eq` and `less` compare the fields** in order of significance, which
  for every valid `DateTime` is the order of `to_unix` (the plan said "by
  `to_unix` order"); comparing fields gives the same order without a
  fault for an invalid value or the arithmetic.
- **`unix_ms()`** is a new clock, `clock_read(2)`: milliseconds since
  1970-01-01T00:00:00Z, rounded down (towards the past).
- `monotonic_ns`, `unix_seconds` and `sleep_ms` stay in `std::sys`;
  `std::time` is the calendar and `unix_ms`. Elapsed time is measured
  with `monotonic_ns`, never with the wall clock, which can be changed
  while a program runs (the guide says so).
- `Weekday` is an enum `Monday` … `Sunday` (ISO 8601's week, Monday
  first) with `Weekday::text`.

Not adopted: `is_leap_year` and `days_in_month` helpers (each one line
over `DateTime::new`, and not a common intent of their own); adding
days or seconds to a `DateTime` (`from_unix(to_unix(&t) + n)` says it);
local time; a format language.

## Compatibility impact

Additive: new names `DateTime`, `Weekday`, `unix_ms` in `std` (a new
submodule `std::time`, re-exported by `std`'s root). A program's own
items of those names shadow them (`[Resolve-Unqualified]`). The private
`clock_read` gains a code.

## Revisit conditions

- Local time and zones, if programs need them and a portable zone
  database becomes available to both tools.
- A `Duration` type, adding and subtracting time, if programs measured
  to write `from_unix(to_unix(&t) + n)` often find it unclear.
- Milliseconds (or finer) in `DateTime` and in ISO text, if log or
  protocol work asks for them.
