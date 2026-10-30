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
| `bench` | 2.716 s (2.663–2.738) | 3.541 s (3.523–3.594) | 2.236 s (2.217–2.262) | 2.655 s (2.618–2.693) |
| `sieve` | 2.253 s (2.216–2.291) | 2.959 s (2.942–2.977) | 1.961 s (1.947–1.984) | 2.084 s (2.039–2.100) |
| `sieve_fn` | 2.487 s (2.445–2.547) | 3.164 s (3.136–3.193) | 2.098 s (2.065–2.117) | 2.190 s (2.165–2.220) |
| `collatz` | 0.481 s (0.480–0.485) | 0.587 s (0.584–0.589) | 0.257 s (0.256–0.262) | 0.559 s (0.557–0.562) |
| `collatz_wrapping` | 0.407 s (0.403–0.414) | 0.244 s (0.243–0.246) | 0.258 s (0.257–0.262) | 0.258 s (0.257–0.261) |
| `strings` | 0.093 s (0.082–0.105) | 0.095 s (0.088–0.101) | 0.079 s (0.074–0.092) | 0.082 s (0.074–0.088) |
| `records` | 0.007 s (0.007–0.015) | 0.008 s (0.007–0.012) | 0.007 s (0.007–0.015) | 0.007 s (0.007–0.010) |
| `tree` | 0.068 s (0.061–0.076) | 0.071 s (0.061–0.075) | 0.029 s (0.027–0.035) | 0.032 s (0.027–0.034) |
| `hashcount` | 0.031 s (0.030–0.033) | 0.039 s (0.037–0.043) | 0.044 s (0.042–0.046) | 0.043 s (0.042–0.045) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 4,259,918,990 | 5,454,115,975 | 2,606,613,171 | 2,867,197,736 |
| `sieve` | 3,354,405,602 | 4,039,555,818 | 1,411,700,200 | 1,541,465,621 |
| `sieve_fn` | 4,943,468,235 | 5,728,637,192 | 2,200,806,780 | 2,138,019,795 |
| `collatz` | 1,148,345,343 | 1,413,848,040 | 1,194,203,989 | 1,325,027,779 |
| `collatz_wrapping` | 799,871,593 | 1,063,763,425 | 1,194,203,647 | 1,194,204,204 |
| `strings` | 235,800,359 | 239,430,457 | 242,938,919 | 243,731,826 |
| `records` | 23,388,963 | 17,406,221 | 15,392,751 | 17,404,822 |
| `tree` | 125,216,581 | 120,217,742 | 30,088,612 | 30,333,545 |
| `hashcount` | 120,320,835 | 128,942,223 | 216,406,301 | 216,777,163 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 48,438,779 | 57,686,034 | 1,271,703 | 56,865,933 |
| `sieve` | 7,222,209 | 7,810,993 | 195,472 | 7,066,159 |
| `sieve_fn` | 7,224,099 | 7,826,653 | 195,985 | 7,018,548 |
| `collatz` | 41,524,327 | 49,890,827 | 1,069,656 | 49,888,522 |
| `collatz_wrapping` | 41,503,780 | 1,073,843 | 1,068,319 | 1,069,275 |
| `strings` | 331,965 | 337,759 | 205,209 | 204,656 |
| `records` | 5,702 | 6,049 | 5,009 | 4,992 |
| `tree` | 1,053,478 | 1,022,251 | 754,874 | 756,843 |
| `hashcount` | 211,387 | 220,251 | 123,217 | 122,787 |
