# Benchmark results

Written by `bench/run.sh` on 2026-11-05; rerun it rather than editing
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
| `sieve` | 1.02 | 0.97 | 1.01 | 0.87 |
| `sieve_fn` | 1.02 | 1.07 | 1.01 | 1.11 |
| `collatz` | 0.87 | 0.80 | 1.04 | 1.07 |
| `collatz_wrapping` | 1.59 | 0.67 | 0.95 | 0.89 |
| `strings` | 0.98 | 0.85 | 0.98 | 0.85 |
| `records` | 1.03 | 1.18 | 0.97 | 1.00 |
| `tree` | 0.97 | 1.07 | 0.95 | 1.05 |
| `hashcount` | 0.58 | 0.41 | 0.64 | 0.43 |

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.573 s (2.532–2.647) | 2.648 s (2.632–2.670) | 2.227 s (2.203–2.255) | 2.645 s (2.601–2.669) |
| `sieve` | 2.092 s (2.058–2.178) | 2.078 s (2.050–2.099) | 1.961 s (1.934–1.985) | 2.059 s (2.024–2.095) |
| `sieve_fn` | 2.207 s (2.182–2.247) | 2.187 s (2.165–2.218) | 2.074 s (2.049–2.107) | 2.171 s (2.144–2.193) |
| `collatz` | 0.485 s (0.483–0.494) | 0.580 s (0.576–0.589) | 0.259 s (0.257–0.262) | 0.558 s (0.556–0.566) |
| `collatz_wrapping` | 0.413 s (0.402–0.417) | 0.245 s (0.243–0.248) | 0.257 s (0.255–0.260) | 0.259 s (0.257–0.265) |
| `strings` | 0.516 s (0.501–0.528) | 0.519 s (0.512–0.531) | 0.523 s (0.494–0.531) | 0.529 s (0.521–0.543) |
| `records` | 0.520 s (0.518–0.525) | 0.493 s (0.485–0.494) | 0.501 s (0.497–0.503) | 0.506 s (0.505–0.510) |
| `tree` | 0.236 s (0.226–0.260) | 0.231 s (0.226–0.264) | 0.239 s (0.232–0.259) | 0.243 s (0.229–0.255) |
| `hashcount` | 0.221 s (0.217–0.226) | 0.241 s (0.232–0.248) | 0.376 s (0.373–0.391) | 0.378 s (0.372–0.386) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2,553,988,255 | 2,757,717,043 | 2,606,613,687 | 2,867,198,139 |
| `sieve` | 1,493,123,637 | 1,343,157,795 | 1,411,700,552 | 1,541,466,257 |
| `sieve_fn` | 2,282,221,261 | 2,374,855,217 | 2,200,806,652 | 2,138,019,911 |
| `collatz` | 1,061,123,793 | 1,413,842,158 | 1,194,204,248 | 1,325,027,883 |
| `collatz_wrapping` | 796,865,739 | 1,063,757,519 | 1,194,203,883 | 1,194,204,406 |
| `strings` | 1,028,330,949 | 1,033,380,103 | 1,212,476,080 | 1,216,780,571 |
| `records` | 4,001,003,413 | 3,401,519,606 | 3,001,055,393 | 3,401,135,361 |
| `tree` | 255,305,679 | 249,928,622 | 235,796,952 | 238,235,541 |
| `hashcount` | 878,551,584 | 931,607,525 | 2,139,312,384 | 2,143,428,012 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 47,385,506 | 57,732,931 | 1,257,202 | 56,852,960 |
| `sieve` | 7,256,752 | 7,934,523 | 226,406 | 6,978,555 |
| `sieve_fn` | 7,391,443 | 7,934,013 | 192,694 | 7,012,960 |
| `collatz` | 41,621,175 | 49,878,920 | 1,070,051 | 49,883,765 |
| `collatz_wrapping` | 42,030,028 | 1,069,722 | 1,069,783 | 1,068,632 |
| `strings` | 1,019,126 | 1,043,682 | 786,282 | 782,094 |
| `records` | 174,620 | 175,655 | 174,045 | 173,912 |
| `tree` | 8,224,441 | 8,235,104 | 7,722,737 | 7,639,034 |
| `hashcount` | 1,937,347 | 1,956,285 | 1,096,593 | 1,108,922 |
