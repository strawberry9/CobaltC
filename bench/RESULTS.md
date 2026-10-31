# Benchmark results

Written by `bench/run.sh` on 2026-10-31; rerun it rather than editing
this file. `bench/README.md` explains the workloads, the method and how to
read these numbers.

- **Machine:** Intel(R) Core(TM) i7 CPU 920 @ 2.67GHz, 4 cores, in a VMware virtual machine, Ubuntu 22.04.5 LTS, Linux 6.8.0-138-generic
- **C compilers for `cobc`:** `gcc`: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0; `clang`: Ubuntu clang version 14.0.0-1ubuntu1.1
- **Rust:** rustc 1.98.1 (48a229cea 2026-09-01)
- **Runs:** 9 per program. Times are the median, with the fastest and slowest in brackets.

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.330 s (2.206–2.368) | 3.286 s (3.243–3.304) | 1.784 s (1.770–1.913) | 2.245 s (2.143–2.331) |
| `sieve` | 1.803 s (1.699–1.899) | 2.704 s (2.689–2.720) | 1.583 s (1.514–1.641) | 1.689 s (1.611–1.728) |
| `sieve_fn` | 2.060 s (2.007–2.089) | 2.907 s (2.860–2.933) | 1.669 s (1.623–1.749) | 1.769 s (1.728–1.848) |
| `collatz` | 0.490 s (0.484–0.493) | 0.581 s (0.578–0.585) | 0.258 s (0.255–0.260) | 0.558 s (0.553–0.559) |
| `collatz_wrapping` | 0.431 s (0.426–0.436) | 0.242 s (0.241–0.245) | 0.256 s (0.254–0.256) | 0.256 s (0.254–0.273) |
| `strings` | 0.079 s (0.079–0.080) | 0.083 s (0.081–0.089) | 0.070 s (0.070–0.071) | 0.071 s (0.070–0.071) |
| `records` | 0.007 s (0.007–0.008) | 0.007 s (0.007–0.007) | 0.006 s (0.006–0.007) | 0.006 s (0.006–0.006) |
| `tree` | 0.038 s (0.038–0.038) | 0.039 s (0.038–0.039) | 0.027 s (0.026–0.027) | 0.027 s (0.026–0.027) |
| `hashcount` | 0.027 s (0.027–0.028) | 0.035 s (0.035–0.036) | 0.042 s (0.041–0.053) | 0.042 s (0.041–0.042) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 4,259,941,237 | 5,454,116,843 | 2,606,613,006 | 2,867,197,903 |
| `sieve` | 3,354,407,835 | 4,039,555,960 | 1,411,700,075 | 1,541,466,361 |
| `sieve_fn` | 4,943,492,180 | 5,728,638,079 | 2,200,807,084 | 2,138,020,153 |
| `collatz` | 1,148,347,333 | 1,413,848,079 | 1,194,204,416 | 1,325,028,738 |
| `collatz_wrapping` | 798,873,715 | 1,063,763,475 | 1,194,204,002 | 1,194,203,933 |
| `strings` | 226,510,984 | 232,232,859 | 242,945,146 | 243,741,924 |
| `records` | 23,393,132 | 17,407,308 | 15,393,554 | 17,405,198 |
| `tree` | 72,072,899 | 76,705,044 | 30,089,163 | 30,334,140 |
| `hashcount` | 100,176,889 | 114,746,070 | 216,391,972 | 216,794,814 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 45,501,603 | 57,642,514 | 1,235,383 | 56,805,686 |
| `sieve` | 7,164,122 | 7,779,027 | 168,437 | 6,973,086 |
| `sieve_fn` | 7,245,313 | 7,755,717 | 169,862 | 6,982,179 |
| `collatz` | 41,853,225 | 49,865,711 | 1,065,773 | 49,867,597 |
| `collatz_wrapping` | 48,044,862 | 1,070,140 | 1,065,707 | 1,065,819 |
| `strings` | 332,322 | 335,532 | 205,044 | 202,087 |
| `records` | 5,516 | 6,076 | 4,907 | 4,899 |
| `tree` | 892,151 | 897,568 | 754,855 | 752,099 |
| `hashcount` | 209,703 | 217,936 | 122,393 | 122,435 |
