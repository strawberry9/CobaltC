# D-0147 — Local time: the machine's offset from UTC, and `DateTime::to_local`

Status: ACCEPTED (2026-10-04, the owner: "write up D-0147 for local time, the narrow version", then "use the current offset as fallback, accept and implement D-0147")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0061 (the clocks, `clock_read`), D-0134 (`std`'s naming conventions: the unit in the name), D-0136 (`std`'s submodules), D-0140 (`DateTime`, UTC only)
Affects: `spec/21` §0, §2j (4.12.0), `spec/conformance.md` (3.138.0), the guide §21 "Dates and times", `impl/std/time.cb`, `impl/std/std.cb` (one private declaration), `impl/src/fileio.rs`, `impl/src/interp.rs`, `impl/cbrt/src/lib.rs` and `include/cbrt.h`, `impl/cobc/src/lower.rs`; `CHG-0175`

## Problem

D-0140 gave `std::time` the proleptic Gregorian calendar in UTC and
nothing else: no time zones, no local time. That is right for what a
program records (logs, file names, timestamps compared across machines),
and wrong for what a program shows a person: "modified today at 18:42",
"next run at 02:00", a clock in a status line. Today such a program
either hard-codes an offset (`to_unix(&t) + 10 * 3600`), which is wrong
twice a year wherever clocks change, or runs `/bin/date` through
`std::process`, which is Unix only and starts a process to read a clock.
Every language a CobaltC programmer comes from has local time in its
standard library: C's `localtime` since the first standard, Rust's
`chrono::Local`, Go's `time.Now()` (local by default), Python's
`datetime.now()`.

The intent "show this moment in the machine's local time" is one very
common intent (§9), so a helper for it qualifies. The question is how
much of time zones to take on.

## Candidate mechanisms

1. **A time-zone database in `std`** (named zones, historical rules,
   both directions, a zoned `DateTime` type). The complete answer, and a
   large one: a body of data that changes several times a year, that
   both tools would have to carry and agree on byte for byte, and a
   second date type with its own arithmetic. Nothing in the showcases or
   the stress rounds has asked for a zone other than the machine's.
   Deferred (a revisit condition).
2. **The machine's current zone only, as the operating system reports
   it, as a function on the moment.** The C library's `localtime` model:
   the platform knows the zone (from `TZ`, the system's zone file, or
   the registry) and its daylight-saving rules; `std` asks it for the
   offset at a moment and shifts the `DateTime`. A few dozen lines of
   Rust shared by both tools. Covers nearly all of the demand. Selected.
3. **A zoned type** (`LocalDateTime`, or an offset field on `DateTime`).
   Would let `to_iso` print `+10:00` instead of `Z`, at the cost of a
   second type, or a field every existing `DateTime` literal would have
   to supply. Rejected: the owner prefers the fewest new tokens, and a
   program that prints an offset can ask for the offset and print it.
4. **Local time as the default** (`DateTime::now()` local, as in Go).
   Rejected: every stored timestamp would then depend on where the
   program ran, and D-0140 chose UTC for `now()` deliberately.

## Selected design

`spec/21` §2j, `rule.stdlib.time`, two additions:

- **`local_offset_seconds(i64 unix) : i64`**: the number of seconds the
  machine's local time is ahead of UTC at the moment `unix` seconds
  after 1970-01-01T00:00:00Z (negative when behind), daylight saving
  included, by the operating system's own rule for the zone the program
  runs in. The unit is in the name (D-0134). Rule block
  `[Local-Offset]`, disposition `trusted-unchecked`, outcome
  "the platform's": the spec cannot define the number, only its
  meaning and range (−18 h ..= +18 h, as the zone database's own
  bounds are). For a moment the platform has no rule for (before its
  records begin, or beyond its range), the offset at the current moment
  is used, so a program never gets 0 masquerading as UTC (the owner's
  choice over `Option<i64>` with `None` there, 2026-10-04).
- **`DateTime::to_local(ref<DateTime, shared> t) : DateTime`**: the
  same moment as `t` (a UTC `DateTime`) written in local time:
  `from_unix(to_unix(t) + local_offset_seconds(to_unix(t)))`, the
  addition checked; overflow (a moment within 18 hours of an `i64`
  limit) is `diag.invalid-datetime`, as `to_unix`'s own overflow is.
  Pure CobaltC. Rule block `[To-Local]`.

What the result is: a `DateTime` like any other, zone-less. The guide
says what that means: `to_iso` writes `Z` and is for UTC values only; a
local value is printed with its own fields, and a program that wants
the offset shown asks `local_offset_seconds` and formats
`+%02d:%02d` itself. `to_unix` of a local value is not the moment (it
is the moment plus the offset), so a program keeps the UTC value for
arithmetic and converts for display at the last step. Logs stay in
UTC; the guide repeats D-0140's rule.

Not provided, each a revisit condition: the other direction (local
fields to a moment; C's `mktime`, which must resolve an hour that is
missing or repeated at a clock change), named zones, a zoned type, the
zone's name or abbreviation, a daylight-saving flag, `to_iso` with an
offset, `DateTime::now_local()` (`to_local(&now())` says the same;
shorter but not clearer).

**Realization.** Rust's standard library has no local-time function, so
the offset is native, like the clocks: a std-private primitive
`tz_offset(i64 unix) : i64` in `impl/std/std.cb`, realized in
`impl/src/fileio.rs` beside `clock_read` and shared by both tools
(`cb_tz_offset` in `cbrt`, lowered by `cobc`). Unix: `localtime_r` on
the moment and the offset computed from the broken-down fields with
D-0140's own `days_from_civil` arithmetic in Rust (so no reliance on the
non-standard `tm_gmtoff`); the C library reads `TZ` and the zone file,
so a program's zone can be set the way every Unix program's is. Windows:
`GetTimeZoneInformationForYear` and `SystemTimeToTzSpecificLocalTime`
on the moment (present since Vista, so Windows 7 has them, and Rust
1.77.2 needs nothing new). The platform's answer is the same under
`coby` and `cobc` because the code is the same; it differs between
machines by design. No GIL concern: the call does not block.

**Conformance** (one case, `datetime_local_ok.cb`, rows
`conf.local-offset-range`, `conf.to-local-consistent`): the offset for
`unix_seconds()` is a multiple of 60 within ±18 hours and the same asked
twice; `to_local` of a fixed `DateTime` equals `from_unix(to_unix +
local_offset_seconds)` field for field; `to_local` of two moments a
second apart differ by one second; moments of years -10000 and 9999
convert (the fallback path on a platform without records there). The
overflow fault is not a case: whether `i64::MAX` overflows depends on the
sign of the machine's offset, and a case has one expectation. The actual
offset is never printed: it is the machine's. The suites run with
whatever `TZ` the machine has; nothing in the case depends on it.

**Guide.** A paragraph "Local time" in §21 "Dates and times": the two
functions, one example that converts a fixed UTC moment and prints its
fields with the offset formatted by hand (the output depends on the
machine, so the example is shown, not checked by `gen.py --check`, as
the clock examples are), and the three rules: store UTC, convert for
display last, never `to_iso` a local value.

## Compatibility impact

Additive: two exported functions in `std::time`, shadowed by a program's
own items of those names. No new types, variants or diagnostics
(`diag.invalid-datetime` is D-0140's). Programs that computed an offset
by hand keep working.

## Revisit conditions

- The other direction (`DateTime::from_local`), when a program needs to
  read a local time a person typed; it must then decide the missing and
  repeated hours, which is a decision of its own.
- Named zones and a zone database, if a program has to show a time in a
  zone other than the machine's (a calendar, a meeting across offices).
- A zoned type or `to_iso` with an offset, if local values turn out to
  be stored or exchanged rather than only shown.
- `Option<i64>` from `local_offset_seconds` instead of the current
  offset, if a program is found that must tell a moment the system has
  no rule for from one it has.
