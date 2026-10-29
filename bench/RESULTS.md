# Benchmark results

Written by `bench/run.sh` on 2026-10-08; rerun it rather than editing
this file. `bench/README.md` explains the workloads, the method and how to
read these numbers.

- **Machine:** Intel(R) Core(TM) i7 CPU 920 @ 2.67GHz, 4 cores, in a VMware virtual machine, Ubuntu 22.04.5 LTS, Linux 6.8.0-138-generic
- **C compilers for `cobc`:** `gcc`: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0; `clang`: Ubuntu clang version 14.0.0-1ubuntu1.1
- **Rust:** rustc 1.98.1 (48a229cea 2026-09-01)
- **Runs:** 9 per program. Times are the median, with the fastest and slowest in brackets.

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.425 s (2.378–2.483) | 3.387 s (3.376–3.411) | 1.972 s (1.933–2.038) | 2.403 s (2.345–2.434) |
| `sieve` | 1.987 s (1.903–2.072) | 2.831 s (2.778–2.846) | 1.768 s (1.655–1.825) | 1.868 s (1.771–1.915) |
| `sieve_fn` | 2.235 s (2.176–2.253) | 3.048 s (3.010–3.073) | 1.892 s (1.814–1.910) | 1.909 s (1.878–1.977) |
| `collatz` | 0.467 s (0.467–0.475) | 0.576 s (0.574–0.585) | 0.257 s (0.255–0.261) | 0.557 s (0.554–0.562) |
| `collatz_wrapping` | 0.434 s (0.431–0.440) | 0.244 s (0.242–0.248) | 0.258 s (0.255–0.265) | 0.257 s (0.256–0.262) |
| `strings` | 0.125 s (0.121–0.137) | 0.146 s (0.134–0.154) | 0.077 s (0.074–0.081) | 0.078 s (0.075–0.088) |
| `records` | 0.007 s (0.007–0.008) | 0.007 s (0.007–0.011) | 0.007 s (0.006–0.009) | 0.007 s (0.006–0.011) |
| `tree` | 0.064 s (0.061–0.074) | 0.064 s (0.059–0.069) | 0.029 s (0.027–0.035) | 0.029 s (0.027–0.035) |
| `hashcount` | 0.071 s (0.068–0.074) | 0.070 s (0.068–0.077) | 0.043 s (0.041–0.062) | 0.044 s (0.041–0.045) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 4,259,947,045 | 5,454,122,123 | 2,606,613,843 | 2,867,198,389 |
| `sieve` | 3,354,407,580 | 4,039,554,521 | 1,411,700,635 | 1,541,466,883 |
| `sieve_fn` | 4,843,533,799 | 5,428,661,048 | 2,200,807,273 | 2,138,020,442 |
| `collatz` | 1,148,338,185 | 1,413,841,813 | 1,194,204,921 | 1,325,028,162 |
| `collatz_wrapping` | 798,864,472 | 1,063,757,257 | 1,194,205,352 | 1,194,204,541 |
| `strings` | 413,602,515 | 426,589,585 | 242,954,860 | 243,739,197 |
| `records` | 23,394,005 | 17,414,680 | 15,394,083 | 17,405,471 |
| `tree` | 109,135,031 | 113,870,224 | 30,090,107 | 30,333,750 |
| `hashcount` | 321,481,928 | 322,673,309 | 216,384,894 | 216,790,266 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 46,511,599 | 57,667,924 | 1,267,324 | 56,904,613 |
| `sieve` | 7,122,092 | 7,790,686 | 183,566 | 6,978,170 |
| `sieve_fn` | 7,249,971 | 8,909,471 | 197,879 | 6,990,819 |
| `collatz` | 39,489,910 | 49,880,289 | 1,068,442 | 49,882,848 |
| `collatz_wrapping` | 48,124,921 | 1,072,496 | 1,069,348 | 1,069,348 |
| `strings` | 1,117,410 | 345,205 | 207,054 | 202,450 |
| `records` | 5,733 | 6,098 | 4,934 | 4,916 |
| `tree` | 1,055,517 | 1,031,138 | 759,826 | 750,974 |
| `hashcount` | 270,118 | 267,928 | 121,672 | 123,221 |
