# CHG-0428 — The performance items open since Update 100, measured again and closed

Status: closed (2026-11-10), agenda item 9 (the owner's delegation of 2026-11-07: "I decide what counts as
acceptable, measure, fix what is engineering, record the rest as accepted with numbers"), Update 178
Affects: no implementation (the measurements); the fixes they led to are CHG-0427 and CHG-0430 to CHG-0432. No rule
changes.

## The items and their verdicts

Measured on Update 177's tools (rustc 1.98.1, gcc 12.3, clang 14.0; the machine idle; `perf stat` user-space
instructions; `bench/run.sh` 9 runs; `stress/perfguide/run.sh` 5).

1. **The tree benchmark's +13%** (2026-10-12): **fixed** by the work since. `bench/tree.cb` against Rust with overflow
   checks, 41 runs of task-clock: `cobc` gcc 247.2 ms, Clang 244.1 ms, Rust checked 239.7 ms (1.03, 1.02);
   instructions 1.09, 1.04. `bench/RESULTS.md`'s table has every workload's ratios; the one above 1.05 in time is
   `collatz_wrapping` under GCC (1.59, GCC keeps a branch; accepted by the owner on 2026-11-05).
2. **E3, the frozen-test call cost (2×)**: **fixed**. `stress/perfguide/e03_string_param.cb`, 1,000,000 calls: a
   `String` passed by value (the caller clones) 148 M instructions, a clone and a free per call, the same work as
   Rust's; by `ref<String, shared>`, by `StringView` and by one view made before the loop 4.28 M under GCC, 0.60 M
   under Clang: the call costs nothing.
3. **Instruction creep since Update 84**: **none**. Every one of the 68 perfguide rows that `results_oct5.txt` (the
   Update-84-era round) also has runs fewer instructions now; against the previous round every row is lower or
   equal but `e07_wrapping` a under GCC (2.24 G → 2.39 G, still below 2.58 G in October: GCC's choice of code
   for the same C, not an addition).
4. **Shared reads across threads did not scale** (round 9): **fixed** where it was engineering. In
   `stress/round9/scaling_owned.cb` (decided, migrated to D-0308's and D-0312's names) four threads were about 2× one,
   with the same instructions as one and 37% more CPU time: the runtime's entries under its lock. Two lowering gaps
   made them: a decided program's drop glue sent every value to the runtime's walk once a thread had been spawned,
   and a call compared where it stands (`column(r, …) == value`) took an object and a statement scope (CHG-0427).
   Now one thread 341–351 ms → 52–78 ms, four 15–39 ms: 3.5× one.
