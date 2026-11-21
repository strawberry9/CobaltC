# CHG-0443 — The length guard of a constant-bounded loop (D-0320)

Status: implemented (2026-11-13), D-0320, D-0296; Update 185
Affects: `impl/cobc/src/lower/` (`guards.rs` new, `exprs.rs` `lower_while`, `stmts.rs`, `passes.rs`
`version_loops`, `uses.rs` `NameScan`, `simd.rs`, `lower.rs`, `functions.rs`, `closures.rs`); their twins in
`impl/selfhost/back` (`gen.cb`, `passes.cb`, `state.cb`) and `impl/selfhost/cbc/main.cb`; `spec/conformance.md` (3.310.0); the guide's
Performance chapter ("Tell the C compiler a slice's length"); `stress/perfguide/slice_lengths`

## Why

D-0320. Update 184 told programmers to write `assert(slice_len(s) >= n)` before a loop whose bounds are
constants, so that the C compiler could remove its index checks (CHG-0441). The owner accepted the
recommendation that the compiler do it instead, without changing what any program does.

## What changes

**`lower_while`** asks `loop_guard` (`guards.rs`; `loop_guard` in `gen.cb`) for the loop's guard unless an
enclosing loop already has one. `GuardScan` walks the loop nest: each loop's variable gets an upper bound from
its test (`x < B`, `x <= B`, `B > x`, `B >= x`, `x + k < B`, the first term of `&&`) when nothing but the step writes
it; a `let` in the nest gets the upper bound of its initializer when the rest of its block never writes, borrows
or declares it again; a name outside the nest has a value when `Fx::const_locals` holds one. That map is filled
as each `let` is lowered: a value known before the run (literals, `const` items as zero-argument calls, `+ - * /
%` of them; a `foreach` range's hidden `$hN` end through `$each_range`) bound to a name the function never assigns,
and any other binding of the name takes it out. Upper bounds go through `+`, `*`, `-`, `/` and `%` of a
constant, `&`, `>>` of a constant and `widen`. Each index into a candidate slice (in scope, never written,
borrowed or declared again in the loop) records one past its upper bound; `simd_load<simd<T, N>>(s, i)` records
`i + N`, and `simd_store(s, i, x)` the lanes of a load or of a vector whose `let` writes its type. The guard is
`(S).len >= N && …`, in the order the slices are first indexed; a loop D-0309 versions is left to D-0309, and a
loop of more than `GUARD_LINES` (400) lines of C is emitted once.

**`version_loops`** handles the new mark (`/*@grd TEST*/` before the loop's `for`) as it does D-0309's, except
that the copy is the loop as it is: `if (__builtin_expect(TEST, 1)) { copy, labels renamed } else { loop }`.
The likely-branch hint cost nothing under Clang and saved GCC a cycle in a loop the programmer had also asserted.

**A vector access's bounds check** is written `if (s.len < n || i > s.len - n)` in both compilers, in place of
`if (i > s.len || s.len - i < n)`: the same test (each faults exactly when `i + n > s.len`, neither wraps), which
Clang proves from `s.len >= N` where it could not prove the other (game of life, 142.4 K to 126.0 K
instructions a generation under Clang).

**Two differences between the compilers, found comparing their C on the game-of-life program** (neither is
D-0320's; both were in Update 184's pair). cbc wrote a vector access's per-lane checks on a direct slice without
the `if (live) { … }` that tests the slice's flag once for all the lanes, so its C was not cobc's
(`back/gen.cb`); and `cbc --emit-c` reported a rejected program in one line (`rejection_text`), where cobc and
cbc's own driver give the located form (`cbc/main.cb`). The showcases exercise neither.

**Conformance:** `conf.length-guard-long-slices` (in range: the same result) and `conf.length-guard-short-slice`
(`dst` too short: the rows before the bad index printed, the fault at that access), both under `coby` and
`cobc` with GCC and Clang.

## Measured

`stress/perfguide/slice_lengths` (instructions a cell, GCC / Clang): the blur with no assert 31 / 38 before,
17 / 16 now; with a `>=` assert 17 / 16; with an `==` assert 17 / 28; with `wrapping_add` additions 12.0 / 4.9,
the C's 12.0 / 4.9. The game of life (`private/game-of-life/life_bench.cb`, instructions a generation, GCC /
Clang): with its asserts removed 180.2 K / 166.9 K before, 114.5 K / 126.0 K now; with them 124.7 K / 109.5 K,
unchanged. Over the 89 showcase programs, 10 gain guards (the crypto routines' fixed-size loops in `std`, and
`records`), 0.3% more C.

## Not changed

No rule, diagnostic or behaviour. A loop whose bounds are known only at run time keeps its checks unless the C
compiler proves them from the loop itself.
