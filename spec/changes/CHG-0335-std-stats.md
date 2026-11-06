# CHG-0335 — `std::stats`: sum, mean, variance, standard deviation, median, percentiles, extremes and a summary over a slice of numbers

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18, D-0267: the owner's acceptance)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0267; `spec/21` §1b `[Sort-Float]`, §3d `sqrt`; `spec/06` `[Int-To-Float]`, `[Float-To-Float]`
Affects: `spec/21` §0, §2t (4.69.0); `spec/conformance.md` (3.245.0); `spec/02` §5 (1.0.116); `impl/std/stats.cb`, `impl/std/std.cb`, `impl/src/prelude.rs`; `htmlguide` §21; `book/` chapter 10

## What changed

- **The rule.** `spec/21` §2t `rule.stdlib.stats`: `sum<T: number>(slice<T, shared>) : T` adds in `T`
  left to right, checked (`[Stats-Sum]`); `mean`, `variance`, `stddev`, `sample_variance`,
  `sample_stddev`, `median`, `minimum` and `maximum` take the same slice and give an `Option<f64>`,
  `None` for an empty slice and, for the two sample forms, for fewer than two values
  (`[Stats-Mean]`, `[Stats-Variance]`, `[Stats-Median]`, `[Stats-Extremes]`); `percentile(data, p)`
  interpolates between the two sorted values position `(n - 1) · p / 100` falls between and faults
  (`diag.assert-failed`) for `p` outside 0 to 100 (`[Stats-Percentile]`); `describe(data)` gives a
  `Summary` (`count`, `minimum`, `maximum`, `mean`, `stddev`, `median`) (`[Stats-Describe]`). The
  variance is two-pass; every sum goes left to right; a NaN among the values makes `mean` and the
  variances a NaN, sorts last for `median` and `percentile`, and is passed over by `minimum` and
  `maximum`.
- **The module.** `impl/std/stats.cb`, declared `export module stats "stats.cb";` in `std.cb` and
  re-exported by `export import std::stats;`, embedded by `impl/src/prelude.rs`; §0's module map, item
  table and signature table list it. Written in CobaltC over `sqrt` and `Vec::sort`; no `unsafe`.
- **Verification.** `conf.stats-sum`, `conf.stats-moments`, `conf.stats-order`, `conf.stats-describe`
  (`impl/conformance/21-standard-library-semantics/stats_ok.cb`), one output under both tools and both
  C compilers; five documented `unittest` blocks in the module (`--unittest-std`), which the reference
  shows as examples and `gen.py --check` runs.

## What changed for programs

Additive. A program that declares its own `sum`, `mean`, `median`, `minimum`, `maximum`, `describe` or
`Summary` beside `import std;` keeps its own: `[Resolve-Unqualified]` finds the program's items first,
and `--warn-shadow` names the shadowing. A program that wrote these functions itself can delete them.
