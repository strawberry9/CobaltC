# CHG-0429 — The lowering keeps the objectless analysis's answers: `cbc` writes its own C in half the time

Status: implemented (2026-11-10), D-0296; on the owner's report that `cbc --emit-c` of its own source took 2–3 times
`cobc`'s time for the same C, Update 178
Affects: `impl/cobc/src/lower/analysis_strings.rs` (`objectless_type_in`, `decided_plain`, `names_closure`),
`impl/cobc/src/lower.rs` and `lower/program.rs` (`ol_memo`, `ol_low`, `dp_memo`); their twins in
`impl/selfhost/back/natives.cb` and `state.cb`. No rule changes. No program's C changes.

## What was wrong

The lowering asks, many times per function, whether a type's value can be held with no runtime object
(`objectless_type`, D-0200, D-0294, D-0315) and whether it is plain data in a decided program (`decided_plain`).
Neither kept an answer. At each part of a struct or an enum the first walked the part's whole type with
`decided_plain` and then answered it by walking it again, so a type's cost grew with the depth and the sharing of the
types under it. On small programs this is invisible. `cbc`'s own source is a decided program whose syntax trees
(`Expr`, `Stmt`, `Type` and the declarations holding them) are deep, wide and shared: there the two walks were half of
`cbc`'s cycles (`natives::objectless_type_in` 52% inclusive in a call-graph profile), and each step paid for a copy
of the declaration it read (`layout::enum_decl` 23%, `struct_decl`), where `cobc` shares it. `cobc` paid the same
walks at Rust's lower cost per step.

## What changes

Both answers are kept, by type (and, for the first, whether a noted value is asked for and whether the program is
decided). `decided_plain` has no assumptions, so every answer holds. `objectless_type_in` answers a type met again
on its own walk "yes" (D-0294's coinductive reading), so an answer made while an enclosing type was still being
answered may hold only there: it keeps a "no" (made with every type in progress taken as yes, it is no under fewer
of them too) and a "yes" that leaned on no type in progress outside its own walk (`ol_low`, the lowest such depth
met). A type that names a closure is answered afresh each time, since the closures' table grows as the lowering
goes. The answers are therefore the ones the walks gave: `cobc` with the memo turned off and on (a temporary switch)
wrote the same C, byte for byte, for 121 programs (the showcases, `bench`, `stress/vsrust`, round 9) and for `cbc`'s
own source.

## Measured

`--emit-c --c-name stage.c impl/selfhost/cbc/main.cb` (120 MB of C), idle machine, wall time:
`cbc` 321 s → 154 s, `cobc` 141 s → 116 s; `cbc` 2.3 → 1.33 times `cobc`'s time.
The speed test's ten programs (`private/speedtest`, 5 runs each, the sum of the medians): `cbc` 15.16 s → 14.60 s,
`cobc` 13.79 s → 13.58 s (1.10 → 1.07 times), the C of both the same.
