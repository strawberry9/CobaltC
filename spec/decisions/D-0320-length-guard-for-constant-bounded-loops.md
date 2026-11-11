# D-0320 — The compiler tells the C compiler a slice's length

Status: ACCEPTED (2026-11-13, the owner: "complete all this work using your recommendations", after the
recommendation "yes, as D-0320, built in Update 185"); implemented by `CHG-0443`
Kind: Decision (`spec/02-schema.md`); a property of the compilers' output, no rule changes
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0284 (decided programs), D-0296 (both toolchains in step), D-0309 (`version_loops`)
Affects: `impl/cobc/src/lower/` (`guards.rs`, `exprs.rs`, `stmts.rs`, `passes.rs`, `simd.rs`), their twins in
`impl/selfhost/back` (`gen.cb`, `passes.cb`, `state.cb`); the guide's Performance chapter

## 1. Problem

A slice parameter's length is a number the C compiler knows nothing about. A loop whose bounds are constants,
such as a grid's width and height or a row stride, therefore keeps every index check, and in a loop that reads
several neighbours of each cell the checks are half the work. The game-of-life report
(`private/game-of-life`, its README's "In short:") found that one `assert(slice_len(s) >= n)` before the loop
removes them, and asked whether the compiler could do it. Update 184 documented the assert (CHG-0441): a
five-point blur went from 31 to 16 instructions a cell under GCC and from 38 to 16 under Clang. But a programmer
had to know the trick, and had to read the generated C to find it.

## 2. Options

1. **Insert the assert.** A fault would come at the loop's start instead of at the first index out of range, and
   with another diagnostic: the program's observable behaviour (its output before the fault, the diagnostic, its
   location) would change. Rejected.
2. **Prove the checks in `cobc` and leave them out.** D-0309's `version_loops` does this for one shape (`x < n`,
   `v[x]` against a fixed vector length). Each further shape is a proof the compilers must get right, and a wrong
   one is a memory error. Kept for that shape; not extended.
3. **Emit the loop twice under a length guard, every check kept in both copies.** `if (s.len >= N && …)` picks
   the first copy, otherwise the second. The two copies are the same code, so whatever the guard says, the program
   does exactly what it did; the guard only gives the C compiler a fact from which it proves the first copy's
   checks and removes them. **Chosen.**

## 3. Decision

Both compilers, in a program built at a bootstrap level that allows D-0309's versioning, give a loop a length
guard when:

- its body indexes (`s[i]`, `simd_load<simd<T, N>>(s, i)`, `simd_store(s, i, x)` with `x` a load or a vector
  whose type the body writes) a slice in scope that the loop never writes, borrows or declares again; and
- the largest such index is known before the program runs: built with `+`, `*`, `-` of a constant, `/` and `%`
  of a constant, `&`, `>>` of a constant and `widen` from literals, `const` items, locals bound to such a value
  that the function never assigns (a `foreach` range's hidden end among them), and the variables of the loop and
  the loops inside it, each bounded by its loop's test (`x < B`, `x <= B`, `x + k < B`, the first term of an
  `&&`) when nothing but the loop's step writes it.

The guard is one test `(S).len >= N` per such slice, N one past its largest index, laid out as the likely branch
(`__builtin_expect`). The outermost loop of a nest that has a guard carries it; the loops inside are copied with
it. A loop D-0309 already versions keeps that copy alone, and a loop longer than 400 lines of C is emitted once. A vector access's
bounds check is written `len < n || i > len - n` (the same test as `i > len || len - i < n`, wrapping in neither),
which a C compiler proves from a lower bound of `len`.

## 4. Consequences

- No rule, diagnostic or behaviour changes. A program's C changes where a loop is guarded (in `std`, the
  crypto routines' fixed-size loops); compare_c and the fixed point see both compilers write the same.
- Measured (`stress/perfguide/slice_lengths`, CHG-0443): the blur without an assert runs as it did with one
  (GCC 17, Clang 16 instructions a cell); with `wrapping_add` additions it runs the same instructions as the C
  (GCC 12.0, Clang 4.9). The game-of-life program with its length asserts removed: GCC 180.2 K to 114.5 K
  instructions a generation (with them, 124.7 K), Clang 166.9 K to 126.0 K (with them, 109.5 K).
- Code: 10 of 89 showcase programs gain guards, 0.3% more C over all of them.
- A loop whose bounds are known only at run time is not guarded. The guide says what the guard needs.
