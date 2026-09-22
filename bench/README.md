# Benchmarks: `cobc` against Rust

Two compute-heavy workloads, the sieve in two shapes, each written in
CobaltC and in Rust the same way, line for line:

| File | Workload |
|---|---|
| `sieve.cb` / `sieve.rs` | Sieve of Eratosthenes over a `Vec<bool>`: the number of primes below 100,000,000 |
| `sieve_fn.cb` / `sieve_fn.rs` | The same sieve, its work in functions that take the vector by reference |
| `collatz.cb` / `collatz.rs` | The longest Collatz chain for starting values below 1,000,000 |
| `collatz_wrapping.cb` / `collatz_wrapping.rs` | The same, its chain arithmetic written with `wrapping_add` and `wrapping_mul`: CobaltC's own way to ask for unchecked arithmetic, to measure what the checks cost |
| `bench.cb` / `bench.rs` | Both, one after the other |

**The results are in [`RESULTS.md`](RESULTS.md)**, which `run.sh`
writes; it is the one place they are kept, and other documents refer
to it rather than copying its numbers.

## Running

Build the compiler and its runtime first, then run the script:

    cd impl && cargo build --release -p cbrt -p cobc && cd ..
    bench/run.sh

`run.sh` builds each workload four ways:
- with `cobc` (default `-O2`), once per C compiler: GCC and Clang
  (`cobc --cc`), or those `COBC_CCS` names;
- with `rustc -O`;
- with `rustc -O -C overflow-checks=on`.

It checks that every build prints the same answer, runs each program
`RUNS` times (default 9), and reports the median wall-clock time with
the range, fastest to slowest. It prints the results and rewrites
`RESULTS.md` with them, headed by the date, the machine, and the
compilers' versions. Executables go to `bench/build/`, which git
ignores.

Where the machine allows it, `run.sh` also counts, with `perf`, the
user-space instructions each program executes and its branch
mispredictions. Unlike time, these barely change from run to run. The
counts need:
- `perf` for the running kernel (`linux-tools-generic`, or any
  `/usr/lib/linux-tools/*/perf`, which `run.sh` finds by itself);
- `kernel.perf_event_paranoid` at 2 or lower;
- working hardware counters. In a VMware VM that means *Virtualize CPU
  performance counters* in the VM's processor settings, changed with
  the VM powered off.

Without them, `run.sh` says so and `RESULTS.md` records times only.

## Reading the results

**How much to trust a difference.** Times vary between runs, and the
ranges in `RESULTS.md` show by how much. `collatz` works only in
registers and barely varies. Each sieve run makes the operating system
supply 100 MB of fresh memory, which varies much more, especially in a
virtual machine. A difference smaller than the ranges is not
meaningful. Instruction and misprediction counts vary by a few parts
in a billion: they show how much work each build's code does, while
time also depends on how fast the processor gets through it.

A compiled CobaltC program performs every check the specification
discharges at run time, except those the compiler proves cannot fail.
It checks every arithmetic operation for overflow (spec/06), so Rust
with overflow checks is the like-for-like comparison.

- **`collatz`** is plain arithmetic on locals, and `cobc`'s code
  executes about as many instructions as Rust's. What separates the
  builds is one branch, `if (x % 2 == 0)`, whose outcome is close to
  random: compare the branch mispredictions. A back end that computes
  both sides and picks one without branching avoids them. LLVM (in
  `rustc` and in Clang) does, but only without overflow checks: with
  them, `3 * x + 1` must fault only when x is odd, so the branch stays.
  GCC keeps the branch even without checks. So `collatz_wrapping` is
  fast in Rust and in `cobc` with Clang, and not in `cobc` with GCC;
  and with checks, every build keeps the branch.
- **Making checked code branch-free.** `cobc` could compute both sides
  of such an `if`, overflow results included, pick one without a
  branch, and fault only if the chosen side overflowed: correct, since
  the fault still happens exactly when the rule says. Tried by hand on
  `cobc`'s output, it helps under Clang but not under GCC, and Clang is
  slower than GCC on the sieves, so `cobc` does not do it yet.
  `impl/STATUS.md` records the measurements.
- **The sieves** are limited by memory more than by arithmetic at this
  size. `cobc`'s code executes more than twice as many instructions as
  Rust's, but much of the time goes to waiting for memory, which every
  build does alike, so the times are closer than the instruction
  counts. The sieves dominate `bench`.
- **`sieve`** uses a local `Vec` only through `Vec::push`, `Vec::len`
  and element access dereferenced at once. The compiler proves nothing
  else can hold a path to it (a *confined* `Vec`), so its accesses
  compile to plain C with bounds and overflow checks, not calls into the
  runtime's alias and validity checks.
- **`sieve_fn`** passes the vector to functions by reference. Each
  function uses its parameter only as a confined vector would be used,
  so it is compiled a second time, for confined arguments, with no
  checks through that parameter.

`impl/COBC-PLAN.md` §10 gives the argument for each check the compiler
leaves out, and `impl/STATUS.md` what each change measured at the time.
