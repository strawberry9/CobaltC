# Benchmark results

Written by `bench/run.sh` on 2026-09-25; rerun it rather than editing
this file. `bench/README.md` explains the workloads, the method and how to
read these numbers.

- **Machine:** Intel(R) Core(TM) i7 CPU 920 @ 2.67GHz, 4 cores, in a VMware virtual machine, Ubuntu 22.04.5 LTS, Linux 6.8.0-138-generic
- **C compilers for `cobc`:** `gcc`: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0; `clang`: Ubuntu clang version 14.0.0-1ubuntu1.1
- **Rust:** rustc 1.98.1 (48a229cea 2026-09-01)
- **Runs:** 9 per program. Times are the median, with the fastest and slowest in brackets.

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.574 s (2.425–2.627) | 3.684 s (3.659–3.697) | 1.836 s (1.720–1.897) | 2.254 s (2.188–2.324) |
| `sieve` | 2.034 s (1.985–2.071) | 3.064 s (3.040–3.113) | 1.605 s (1.525–1.622) | 1.645 s (1.541–1.749) |
| `sieve_fn` | 2.025 s (1.922–2.067) | 3.062 s (3.027–3.089) | 1.654 s (1.554–1.711) | 1.694 s (1.614–1.719) |
| `collatz` | 0.473 s (0.472–0.473) | 0.573 s (0.570–0.575) | 0.255 s (0.254–0.256) | 0.552 s (0.552–0.555) |
| `collatz_wrapping` | 0.425 s (0.421–0.426) | 0.239 s (0.238–0.265) | 0.254 s (0.254–0.256) | 0.255 s (0.254–0.255) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 5,884,308,872 | 7,065,558,052 | 3,395,717,211 | 3,463,754,336 |
| `sieve` | 4,723,209,804 | 5,650,981,039 | 2,200,806,599 | 2,138,019,861 |
| `sieve_fn` | 4,523,211,776 | 5,450,981,861 | 2,255,591,289 | 2,138,019,900 |
| `collatz` | 1,161,417,651 | 1,413,924,630 | 1,194,203,879 | 1,325,028,145 |
| `collatz_wrapping` | 798,943,984 | 1,063,840,040 | 1,194,203,927 | 1,194,204,401 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 45,861,079 | 57,784,085 | 1,257,100 | 56,831,086 |
| `sieve` | 7,403,513 | 7,912,445 | 182,472 | 6,974,024 |
| `sieve_fn` | 7,338,739 | 7,900,502 | 8,658,980 | 6,974,384 |
| `collatz` | 40,555,026 | 49,872,072 | 1,071,967 | 49,867,290 |
| `collatz_wrapping` | 48,102,709 | 1,066,661 | 1,071,955 | 1,072,011 |
