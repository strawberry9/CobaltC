# D-0267 — `std::stats`: descriptive statistics over a slice of numbers

Status: ACCEPTED (2026-10-18, the owner: "accept the recommendations, do it after the push")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §9, §15, §20
Depends on: D-0090 (the `number` bound), D-0136 (`std`'s submodules), D-0181 (`[Sort-Float]`), D-0187, D-0223 (the frozen map admits a module for a subject `std` does not cover), D-0229, D-0230 (documentation and examples in `std`)
Affects: `spec/21` §0, §2t (4.69.0); `impl/std/stats.cb`, `impl/std/std.cb`, `impl/src/prelude.rs`; `CHG-0335`

## Problem

`std` had no statistics. A program with a run of timings, sizes or scores wrote its own mean and
standard deviation, and its own median by sorting a copy, every time, and each got the population
and sample forms, the percentile convention and the empty case wrong in its own way. "The sum, the
mean, the spread and the middle of these numbers" is one intent that almost every program with
numbers has, which is the admission test of Master Instructions §9, so the owner asked for a module.

## Options

1. **The element type.** (a) `slice<f64, shared>` only, every caller converting first. (b) Generic
   over `T: number`, each value converted with `to_float<f64>`, the results `f64`; `sum` alone in `T`,
   checked, so an integer sum that does not fit faults as `+` does. Counts and nanosecond timings are
   integers far more often than floats, and (a) would make every such caller copy into a `Vec<f64>`.
2. **Empty input.** (a) A fault. (b) `Option<f64>`, `None` for an empty slice and for a sample
   statistic of fewer than two values; `sum` of nothing is 0 and needs no `Option`. An empty data set
   is ordinary input, not a bug, which is the language's own test for a result against a fault (D-0009).
3. **Population or sample.** (a) One name and a `bool` parameter. (b) Two names: `variance` and
   `stddev` divide by the count, `sample_variance` and `sample_stddev` by one less. A `bool` reads
   badly at the call and both forms are common.
4. **Median and percentiles.** (a) The nearest rank. (b) A sorted copy with linear interpolation
   between the two values a percentile falls between (R's type 7, the default of spreadsheets and
   NumPy), and `median` the mean of the two middle values when the count is even. (b) is what a reader
   checks a result against.
5. **Numerics.** (a) One pass (Welford's update). (b) Two passes, the mean first and then the squared
   deviations, every sum left to right. (b) is the textbook definition, accurate enough for a
   descriptive statistic, and gives one result on every implementation, so a conformance case can
   state the exact output.
6. **Scope.** A first tier of `sum`, `mean`, `variance`, `stddev`, `sample_variance`,
   `sample_stddev`, `median`, `percentile`, `minimum`, `maximum` and `describe`; a second tier (mode,
   covariance, correlation, a linear fit, histograms) when a program needs it.

## Decision

(b) in each. `std::stats` (`impl/std/stats.cb`), declared in `std.cb` and re-exported by `import std;`
as every submodule is (D-0136, D-0187). Every function takes a `slice<T, shared>` with `T: number`;
`sum` gives a `T`, the others an `Option<f64>`; `percentile(data, p)` takes `p` from 0 to 100 and
faults (`diag.assert-failed`) outside it, as `clamp` does for bounds in the wrong order; `minimum`
and `maximum` pass over a NaN (a NaN only when every value is one); `describe(data)` gives a `Summary`
of the count, minimum, maximum, mean, population standard deviation and median. Written in CobaltC
over `sqrt` (§3d) and `Vec::sort` (§1b, `[Sort-Float]`: a NaN sorts after every number); no `unsafe`,
so `spec/21` §0's trusted table is unchanged.

## Results

`spec/21` §2t `rule.stdlib.stats` with `[Stats-Sum]`, `[Stats-Mean]`, `[Stats-Variance]`,
`[Stats-Median]`, `[Stats-Percentile]`, `[Stats-Extremes]` and `[Stats-Describe]`, and the source
transcribed; §0's module map, item table and signature table. `CHG-0335`. Cases `conf.stats-sum`,
`conf.stats-moments`, `conf.stats-order` and `conf.stats-describe` (`stats_ok.cb`), whose output is
byte-identical under `coby` and `cobc` with GCC and with Clang. Five documented `unittest` blocks,
shown as the reference's examples. The guide's §21 and the book's chapter 10 each gain a passage.
Additive: a program's own `sum` or `mean` still names its own item beside `import std;`
(`[Resolve-Unqualified]` clause 1; `--warn-shadow` notes the shadowing).

## Revisit conditions

The second tier when a program needs it. A `Summary` field for the sample standard deviation, or a
`percentiles(data, &ps)` that sorts once for several, if asked for.
