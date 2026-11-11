# Benchmark results

Written by `bench/run.sh` on 2026-11-14; rerun it rather than editing
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
| `bench` | 0.95 | 0.89 | 0.99 | 0.96 |
| `sieve` | 1.03 | 0.97 | 1.01 | 0.87 |
| `sieve_fn` | 1.00 | 1.07 | 0.98 | 1.11 |
| `collatz` | 0.85 | 0.80 | 1.04 | 1.07 |
| `collatz_wrapping` | 1.59 | 0.67 | 0.94 | 0.89 |
| `strings` | 0.91 | 0.69 | 0.90 | 0.68 |
| `records` | 1.02 | 1.18 | 0.97 | 1.00 |
| `tree` | 1.05 | 1.09 | 1.04 | 1.11 |
| `hashcount` | 0.59 | 0.40 | 0.59 | 0.40 |

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.138 s (2.050–2.214) | 2.224 s (2.159–2.267) | 1.821 s (1.735–1.901) | 2.243 s (2.038–2.355) |
| `sieve` | 1.681 s (1.561–1.735) | 1.646 s (1.507–1.780) | 1.472 s (1.355–1.662) | 1.637 s (1.516–1.742) |
| `sieve_fn` | 1.749 s (1.703–1.844) | 1.705 s (1.580–1.888) | 1.620 s (1.554–1.741) | 1.747 s (1.665–1.807) |
| `collatz` | 0.473 s (0.473–0.475) | 0.577 s (0.576–0.582) | 0.255 s (0.254–0.257) | 0.554 s (0.552–0.555) |
| `collatz_wrapping` | 0.404 s (0.392–0.407) | 0.239 s (0.238–0.241) | 0.253 s (0.252–0.263) | 0.254 s (0.252–0.256) |
| `strings` | 0.437 s (0.431–0.440) | 0.433 s (0.430–0.443) | 0.485 s (0.477–0.490) | 0.481 s (0.476–0.488) |
| `records` | 0.513 s (0.511–0.517) | 0.486 s (0.481–0.489) | 0.495 s (0.493–0.497) | 0.502 s (0.500–0.503) |
| `tree` | 0.216 s (0.209–0.226) | 0.213 s (0.211–0.238) | 0.205 s (0.202–0.231) | 0.205 s (0.203–0.229) |
| `hashcount` | 0.214 s (0.212–0.216) | 0.213 s (0.211–0.218) | 0.362 s (0.357–0.373) | 0.364 s (0.360–0.373) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2,553,978,877 | 2,757,707,629 | 2,606,605,414 | 2,867,190,794 |
| `sieve` | 1,493,114,326 | 1,343,148,191 | 1,411,691,945 | 1,541,457,890 |
| `sieve_fn` | 2,282,211,320 | 2,374,844,252 | 2,200,799,105 | 2,138,011,990 |
| `collatz` | 1,061,114,962 | 1,413,833,445 | 1,194,196,353 | 1,325,020,290 |
| `collatz_wrapping` | 796,856,997 | 1,063,748,947 | 1,194,196,523 | 1,194,196,715 |
| `strings` | 836,004,997 | 825,947,804 | 1,212,534,312 | 1,216,797,433 |
| `records` | 4,000,994,649 | 3,401,510,370 | 3,001,047,229 | 3,401,126,764 |
| `tree` | 259,498,951 | 263,419,407 | 235,789,016 | 238,226,955 |
| `hashcount` | 846,632,324 | 846,742,754 | 2,139,301,111 | 2,143,099,509 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 49,017,066 | 57,720,857 | 1,257,400 | 56,834,745 |
| `sieve` | 7,232,638 | 7,853,487 | 176,327 | 6,947,538 |
| `sieve_fn` | 7,525,659 | 7,899,544 | 180,743 | 6,979,669 |
| `collatz` | 41,123,539 | 49,867,085 | 1,065,718 | 49,869,323 |
| `collatz_wrapping` | 41,767,602 | 1,069,634 | 1,066,335 | 1,065,493 |
| `strings` | 1,101,285 | 1,089,813 | 780,105 | 771,196 |
| `records` | 168,661 | 168,825 | 168,077 | 168,935 |
| `tree` | 7,651,782 | 7,627,755 | 7,682,301 | 7,598,816 |
| `hashcount` | 1,894,551 | 1,953,667 | 1,100,016 | 1,092,748 |
