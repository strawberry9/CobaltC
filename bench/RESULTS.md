# Benchmark results

Written by `bench/run.sh` on 2026-11-20; rerun it rather than editing
this file. `bench/README.md` explains the workloads, the method and how to
read these numbers.

- **Machine:** Intel(R) Core(TM) i7 CPU 920 @ 2.67GHz, 4 cores, in a VMware virtual machine, Ubuntu 22.04.5 LTS, Linux 6.8.0-138-generic
- **C compilers for `cobc`:** `gcc`: gcc (Ubuntu 12.3.0-1ubuntu1~22.04.3) 12.3.0; `clang`: Ubuntu clang version 14.0.0-1ubuntu1.1
- **Rust:** rustc 1.98.1 (48a229cea 2026-09-01)
- **Runs:** 9 per program. Times are the median, with the fastest and slowest in brackets.

## Against Rust with overflow checks

D-0309: a decided program (every workload here is one) is to be as fast as Rust
with comparable runtime checks, or faster. Each `cobc` build divided by
"Rust, overflow checks": 1.00 or less meets the goal.

| Workload | `cobc` (gcc), time | `cobc` (gcc), instructions | `cobc` (clang), time | `cobc` (clang), instructions |
|---|---|---|---|---|
| `bench` | 0.97 | 0.89 | 1.00 | 0.96 |
| `sieve` | 1.00 | 0.97 | 0.99 | 0.87 |
| `sieve_fn` | 1.03 | 1.07 | 1.01 | 1.11 |
| `collatz` | 0.86 | 0.80 | 1.03 | 1.07 |
| `collatz_wrapping` | 1.60 | 0.67 | 0.95 | 0.89 |
| `strings` | 0.43 | 0.41 | 0.43 | 0.40 |
| `records` | 1.03 | 1.18 | 0.97 | 1.00 |
| `tree` | 0.85 | 0.58 | 0.88 | 0.63 |
| `hashcount` | 0.45 | 0.28 | 0.44 | 0.27 |
| `hashmap` | 0.80 | 0.43 | 0.79 | 0.38 |

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.521 s (2.491–2.546) | 2.596 s (2.575–2.627) | 2.179 s (2.147–2.210) | 2.591 s (2.556–2.621) |
| `sieve` | 2.031 s (1.994–2.072) | 2.008 s (1.961–2.037) | 1.912 s (1.873–1.928) | 2.033 s (1.998–2.048) |
| `sieve_fn` | 2.160 s (2.072–2.188) | 2.122 s (2.063–2.161) | 2.020 s (1.904–2.049) | 2.102 s (2.028–2.149) |
| `collatz` | 0.477 s (0.476–0.479) | 0.574 s (0.571–0.577) | 0.258 s (0.255–0.262) | 0.557 s (0.556–0.562) |
| `collatz_wrapping` | 0.410 s (0.404–0.411) | 0.243 s (0.242–0.246) | 0.258 s (0.256–0.259) | 0.257 s (0.256–0.259) |
| `strings` | 0.220 s (0.217–0.223) | 0.220 s (0.217–0.223) | 0.517 s (0.510–0.525) | 0.511 s (0.503–0.526) |
| `records` | 0.517 s (0.513–0.524) | 0.489 s (0.488–0.491) | 0.498 s (0.496–0.505) | 0.502 s (0.498–0.506) |
| `tree` | 0.206 s (0.197–0.213) | 0.213 s (0.208–0.222) | 0.234 s (0.231–0.250) | 0.243 s (0.223–0.252) |
| `hashcount` | 0.167 s (0.165–0.171) | 0.163 s (0.159–0.168) | 0.376 s (0.369–0.381) | 0.371 s (0.368–0.380) |
| `hashmap` | 1.088 s (1.068–1.125) | 1.074 s (1.061–1.082) | 1.553 s (1.535–1.567) | 1.367 s (1.344–1.396) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2,553,927,077 | 2,757,633,119 | 2,606,614,502 | 2,867,198,954 |
| `sieve` | 1,493,062,521 | 1,343,073,653 | 1,411,701,185 | 1,541,467,112 |
| `sieve_fn` | 2,282,160,039 | 2,374,771,475 | 2,200,808,402 | 2,138,021,057 |
| `collatz` | 1,061,062,728 | 1,413,756,373 | 1,194,204,860 | 1,325,028,908 |
| `collatz_wrapping` | 796,797,199 | 1,063,670,449 | 1,194,205,266 | 1,194,204,795 |
| `strings` | 498,825,755 | 491,829,769 | 1,212,528,771 | 1,216,755,999 |
| `records` | 4,000,942,832 | 3,401,436,720 | 3,001,056,384 | 3,401,136,009 |
| `tree` | 139,220,327 | 150,610,313 | 235,797,610 | 238,236,070 |
| `hashcount` | 598,889,765 | 587,734,799 | 2,139,246,836 | 2,143,291,144 |
| `hashmap` | 1,091,876,115 | 959,878,280 | 2,539,432,778 | 2,342,430,263 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 48,877,814 | 57,802,197 | 1,267,846 | 56,949,159 |
| `sieve` | 7,259,843 | 7,863,840 | 171,504 | 6,979,681 |
| `sieve_fn` | 7,529,680 | 7,835,358 | 187,910 | 7,010,438 |
| `collatz` | 41,219,817 | 49,875,988 | 1,068,006 | 49,884,884 |
| `collatz_wrapping` | 42,028,497 | 1,068,177 | 1,068,982 | 1,066,714 |
| `strings` | 1,528,447 | 1,590,023 | 779,745 | 1,280,299 |
| `records` | 170,926 | 172,421 | 174,953 | 173,167 |
| `tree` | 7,841,013 | 7,872,125 | 7,703,897 | 7,642,083 |
| `hashcount` | 1,978,077 | 1,987,616 | 1,093,667 | 1,101,975 |
| `hashmap` | 8,514,785 | 8,531,966 | 2,643,857 | 2,687,865 |
