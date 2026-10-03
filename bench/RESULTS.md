# Benchmark results

Written by `bench/run.sh` on 2026-10-03; rerun it rather than editing
this file. `bench/README.md` explains the workloads, the method and how to
read these numbers.

- **Machine:** Intel(R) Core(TM) i7 CPU 920 @ 2.67GHz, 4 cores, in a VMware virtual machine, Ubuntu 22.04.5 LTS, Linux 6.8.0-138-generic
- **C compilers for `cobc`:** `gcc`: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0; `clang`: Ubuntu clang version 14.0.0-1ubuntu1.1
- **Rust:** rustc 1.98.1 (48a229cea 2026-09-01)
- **Runs:** 9 per program. Times are the median, with the fastest and slowest in brackets.

## Time

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 2.544 s (2.463–2.644) | 3.618 s (3.591–3.645) | 1.909 s (1.860–1.986) | 2.339 s (2.214–2.390) |
| `sieve` | 2.081 s (1.981–2.233) | 3.021 s (3.000–3.067) | 1.684 s (1.612–1.732) | 1.782 s (1.632–1.842) |
| `sieve_fn` | 2.095 s (2.018–2.252) | 3.024 s (2.989–3.051) | 1.680 s (1.604–1.788) | 1.777 s (1.662–1.830) |
| `collatz` | 0.463 s (0.463–0.463) | 0.571 s (0.569–0.574) | 0.254 s (0.254–0.256) | 0.553 s (0.550–0.555) |
| `collatz_wrapping` | 0.425 s (0.423–0.427) | 0.239 s (0.238–0.240) | 0.255 s (0.254–0.260) | 0.255 s (0.254–0.255) |
| `strings` | 0.128 s (0.124–0.130) | 0.133 s (0.132–0.136) | 0.072 s (0.071–0.073) | 0.072 s (0.072–0.073) |
| `records` | 0.007 s (0.007–0.008) | 0.006 s (0.006–0.007) | 0.006 s (0.006–0.006) | 0.006 s (0.006–0.006) |
| `tree` | 0.056 s (0.055–0.057) | 0.057 s (0.057–0.058) | 0.027 s (0.027–0.027) | 0.027 s (0.027–0.027) |
| `hashcount` | 0.066 s (0.065–0.067) | 0.066 s (0.066–0.081) | 0.041 s (0.041–0.054) | 0.041 s (0.041–0.047) |

## Instructions executed

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 5,771,124,180 | 7,065,320,093 | 3,395,716,381 | 3,463,754,149 |
| `sieve` | 4,865,602,667 | 5,650,751,497 | 2,200,806,191 | 2,138,019,432 |
| `sieve_fn` | 4,865,602,743 | 5,450,751,068 | 2,200,806,569 | 2,138,019,382 |
| `collatz` | 1,148,334,922 | 1,413,839,458 | 1,194,203,945 | 1,325,027,340 |
| `collatz_wrapping` | 798,861,123 | 1,063,754,865 | 1,194,203,389 | 1,194,203,537 |
| `strings` | 438,171,032 | 451,078,053 | 242,936,383 | 243,741,110 |
| `records` | 23,408,747 | 17,430,392 | 15,393,266 | 17,404,762 |
| `tree` | 106,070,736 | 110,027,870 | 30,089,030 | 30,333,720 |
| `hashcount` | 314,500,916 | 315,424,170 | 216,387,693 | 216,803,342 |

## Branch mispredictions

User space, counted by `perf`.

| Workload | `cobc` (gcc) | `cobc` (clang) | Rust | Rust, overflow checks |
|---|---|---|---|---|
| `bench` | 48,195,432 | 57,769,923 | 1,257,195 | 56,825,681 |
| `sieve` | 7,300,029 | 7,993,672 | 180,956 | 6,947,706 |
| `sieve_fn` | 7,373,500 | 8,999,645 | 165,625 | 6,998,238 |
| `collatz` | 39,466,505 | 49,865,389 | 1,066,416 | 49,865,536 |
| `collatz_wrapping` | 48,102,605 | 1,070,042 | 1,065,753 | 1,065,841 |
| `strings` | 1,147,738 | 344,004 | 202,971 | 203,745 |
| `records` | 5,567 | 6,173 | 4,891 | 4,891 |
| `tree` | 1,068,889 | 1,056,617 | 754,684 | 749,140 |
| `hashcount` | 270,585 | 281,141 | 122,418 | 122,660 |
