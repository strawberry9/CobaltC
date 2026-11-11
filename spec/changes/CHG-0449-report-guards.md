# CHG-0449 — `cobc --report-guards`: the length guard of each loop, or why there is none

Status: implemented (2026-11-14), D-0296; Update 188
Affects: `impl/cobc/src/lower/guards.rs` (`GuardScan::loose`, `note_loose`, `unbounded`, `loop_guard`'s
note), `impl/cobc/src/lower/exprs.rs` (`lower_while`), `impl/cobc/src/lower/program.rs` (`guard_note`),
`impl/cobc/src/lower.rs` (`Gen::report_guards`, `guard_notes`, `Generated::guard_notes`, `Fx::pending_note`),
`impl/cobc/src/report.rs` (`guards`), `impl/cobc/src/main.rs`; their twins `impl/selfhost/back/gen.cb`
(`gs_note_loose`, `gs_unbounded`, `loop_guard`, `generate_program`), `impl/selfhost/back/state.cb`
(`GuardNote`, `guard_note`), `impl/selfhost/cbc/report.cb` (`guards`), `impl/selfhost/cbc/driver.cb`; the
guide's Performance chapter

## Why

Item 6 of the plan. D-0320's guard is found by `cobc` from the loop's bounds; whether a loop got one, and the
count it asks for, could be learnt only by reading the C (the game-of-life program's `at < WORDS - 2` was found
that way).

## What changes

`--report-guards` compiles nothing. For each loop of the program's own files whose scan met a candidate slice, it
prints the file and line, then the guard as `slice_len(s) >= N` tests, each index into a candidate slice that has
no known bound with the first name in it that has none, and whether the guard is not used (a loop D-0309
versions, or one over 400 lines of C). The notes are collected while the program is lowered (the C does not
change) and sorted by file and line. What the C compiler then drops is not known to cobc; the help text says to
look at the executable for that.
