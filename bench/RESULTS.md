# Benchmark results

Written by `bench/run.sh` on 2026-11-23; rerun it rather than editing
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
| `bench` | 0.99 | 0.89 | 0.94 | 0.90 |
| `sieve` | 1.01 | 0.97 | 1.01 | 0.98 |
| `sieve_fn` | 1.04 | 1.07 | 1.01 | 1.08 |
| `collatz` | 0.87 | 0.80 | 0.83 | 0.80 |
| `collatz_wrapping` | 1.60 | 0.67 | 0.95 | 0.89 |
| `strings` | 0.44 | 0.41 | 0.44 | 0.40 |
| `records` | 1.01 | 1.18 | 0.97 | 1.00 |
| `tree` | 0.94 | 0.59 | 0.93 | 0.63 |
| `hashcount` | 0.46 | 0.28 | 0.45 | 0.27 |
| `hashmap` | 0.84 | 0.47 | 0.83 | 0.41 |

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.335 s (2.222–2.352) | 2.219 s (2.150–2.295) | 1.911 s (1.837–2.000) | 2.359 s (2.248–2.405) |
| `sieve` | 1.782 s (1.735–1.860) | 1.779 s (1.732–1.843) | 1.740 s (1.668–1.748) | 1.756 s (1.707–1.795) |
| `sieve_fn` | 1.942 s (1.867–1.989) | 1.883 s (1.830–1.911) | 1.802 s (1.737–1.834) | 1.869 s (1.805–1.940) |
| `collatz` | 0.479 s (0.478–0.482) | 0.461 s (0.457–0.462) | 0.255 s (0.254–0.257) | 0.553 s (0.549–0.555) |
| `collatz_wrapping` | 0.408 s (0.398–0.409) | 0.241 s (0.240–0.242) | 0.255 s (0.254–0.256) | 0.255 s (0.254–0.255) |
| `strings` | 0.216 s (0.214–0.217) | 0.214 s (0.213–0.215) | 0.485 s (0.484–0.486) | 0.486 s (0.483–0.488) |
| `records` | 0.506 s (0.506–0.508) | 0.486 s (0.480–0.488) | 0.494 s (0.493–0.498) | 0.500 s (0.499–0.502) |
| `tree` | 0.192 s (0.185–0.194) | 0.189 s (0.189–0.190) | 0.203 s (0.203–0.204) | 0.204 s (0.203–0.208) |
| `hashcount` | 0.165 s (0.161–0.166) | 0.162 s (0.159–0.162) | 0.363 s (0.361–0.370) | 0.361 s (0.360–0.368) |
| `hashmap` | 1.022 s (1.005–1.042) | 1.014 s (0.999–1.022) | 1.376 s (1.362–1.383) | 1.223 s (1.220–1.238) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2,553,954,587 | 2,572,504,146 | 2,606,613,727 | 2,867,199,000 |
| `sieve` | 1,493,090,170 | 1,510,639,829 | 1,411,701,352 | 1,541,466,878 |
| `sieve_fn` | 2,282,190,213 | 2,299,754,911 | 2,200,808,164 | 2,138,020,747 |
| `collatz` | 1,061,068,325 | 1,061,065,298 | 1,194,204,426 | 1,325,029,129 |
| `collatz_wrapping` | 796,810,718 | 1,063,675,966 | 1,194,204,598 | 1,194,204,843 |
| `strings` | 500,054,174 | 492,049,709 | 1,212,513,812 | 1,216,753,230 |
| `records` | 4,000,978,571 | 3,401,458,491 | 3,001,056,184 | 3,401,135,319 |
| `tree` | 139,819,493 | 150,614,454 | 235,797,962 | 238,235,534 |
| `hashcount` | 600,605,992 | 587,464,744 | 2,139,473,224 | 2,143,251,339 |
| `hashmap` | 1,092,519,574 | 959,947,740 | 2,539,420,483 | 2,342,409,910 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 48,837,486 | 49,014,374 | 1,262,071 | 56,834,997 |
| `sieve` | 7,244,813 | 7,344,114 | 184,935 | 6,957,373 |
| `sieve_fn` | 7,250,836 | 7,357,610 | 184,361 | 6,989,996 |
| `collatz` | 41,798,915 | 41,700,240 | 1,065,702 | 49,867,446 |
| `collatz_wrapping` | 42,027,717 | 1,064,854 | 1,065,847 | 1,065,832 |
| `strings` | 1,548,144 | 1,536,576 | 777,245 | 765,206 |
| `records` | 165,928 | 167,203 | 167,415 | 167,980 |
| `tree` | 7,814,328 | 7,752,362 | 7,674,873 | 7,594,344 |
| `hashcount` | 1,982,081 | 1,967,003 | 1,107,496 | 1,096,272 |
| `hashmap` | 8,470,365 | 8,477,983 | 2,619,848 | 2,661,541 |
