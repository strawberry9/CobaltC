# Benchmark results

Written by `bench/run.sh` on 2026-10-30; rerun it rather than editing
this file. `bench/README.md` explains the workloads, the method and how to
read these numbers.

- **Machine:** Intel(R) Core(TM) i7 CPU 920 @ 2.67GHz, 4 cores, in a VMware virtual machine, Ubuntu 22.04.5 LTS, Linux 6.8.0-138-generic
- **C compilers for `cobc`:** `gcc`: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0; `clang`: Ubuntu clang version 14.0.0-1ubuntu1.1
- **Rust:** rustc 1.98.1 (48a229cea 2026-09-01)
- **Runs:** 9 per program. Times are the median, with the fastest and slowest in brackets.

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.690 s (2.508–2.832) | 3.531 s (3.444–3.560) | 2.266 s (2.014–2.372) | 2.415 s (2.411–2.494) |
| `sieve` | 2.023 s (2.021–2.026) | 2.775 s (2.760–2.785) | 1.755 s (1.752–1.762) | 1.850 s (1.846–1.898) |
| `sieve_fn` | 2.254 s (2.252–2.304) | 2.996 s (2.989–3.022) | 1.869 s (1.867–1.879) | 1.957 s (1.953–1.959) |
| `collatz` | 0.479 s (0.478–0.481) | 0.577 s (0.575–0.580) | 0.253 s (0.253–0.254) | 0.549 s (0.547–0.556) |
| `collatz_wrapping` | 0.408 s (0.405–0.410) | 0.242 s (0.241–0.244) | 0.255 s (0.255–0.257) | 0.254 s (0.251–0.256) |
| `strings` | 0.104 s (0.102–0.106) | 0.118 s (0.115–0.119) | 0.073 s (0.073–0.073) | 0.073 s (0.073–0.074) |
| `records` | 0.007 s (0.007–0.007) | 0.007 s (0.007–0.007) | 0.007 s (0.006–0.007) | 0.007 s (0.006–0.007) |
| `tree` | 0.060 s (0.060–0.061) | 0.060 s (0.059–0.060) | 0.027 s (0.027–0.028) | 0.027 s (0.027–0.027) |
| `hashcount` | 0.047 s (0.047–0.048) | 0.054 s (0.053–0.054) | 0.041 s (0.041–0.042) | 0.041 s (0.041–0.042) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 4,259,935,115 | 5,454,130,928 | 2,606,612,818 | 2,867,197,510 |
| `sieve` | 3,354,413,402 | 4,039,562,939 | 1,411,700,250 | 1,541,466,311 |
| `sieve_fn` | 4,943,483,584 | 5,728,641,187 | 2,200,806,654 | 2,138,019,578 |
| `collatz` | 1,148,349,694 | 1,413,854,380 | 1,194,203,939 | 1,325,027,487 |
| `collatz_wrapping` | 799,875,936 | 1,063,769,693 | 1,194,203,453 | 1,194,203,944 |
| `strings` | 282,466,805 | 297,325,454 | 242,943,969 | 243,728,851 |
| `records` | 23,399,007 | 17,414,108 | 15,392,967 | 17,404,784 |
| `tree` | 125,223,926 | 120,224,928 | 30,088,616 | 30,333,310 |
| `hashcount` | 210,153,426 | 219,091,402 | 216,396,040 | 216,800,026 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 46,840,823 | 57,665,497 | 1,259,913 | 56,836,419 |
| `sieve` | 7,174,104 | 7,779,881 | 160,174 | 6,976,012 |
| `sieve_fn` | 7,261,051 | 8,832,985 | 173,188 | 7,006,956 |
| `collatz` | 41,799,134 | 49,864,538 | 1,065,403 | 49,869,242 |
| `collatz_wrapping` | 41,800,024 | 1,066,423 | 1,065,761 | 1,065,734 |
| `strings` | 1,201,947 | 416,184 | 202,740 | 203,685 |
| `records` | 5,631 | 6,083 | 4,935 | 4,888 |
| `tree` | 1,026,796 | 1,018,860 | 754,863 | 751,747 |
| `hashcount` | 256,255 | 218,093 | 122,251 | 122,376 |
