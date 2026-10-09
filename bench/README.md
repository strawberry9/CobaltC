# Benchmarks: `cobc` against Rust

Compute-heavy workloads and workloads shaped like real programs, each
written in CobaltC and in Rust the same way, line for line:

| File | Workload |
|---|---|
| `sieve.cb` / `sieve.rs` | Sieve of Eratosthenes over a `Vec<bool>`: the number of primes below 100,000,000 |
| `sieve_fn.cb` / `sieve_fn.rs` | The same sieve, its work in functions that take the vector by reference |
| `collatz.cb` / `collatz.rs` | The longest Collatz chain for starting values below 1,000,000 |
| `collatz_wrapping.cb` / `collatz_wrapping.rs` | The same, its chain arithmetic written with `wrapping_add` and `wrapping_mul`: CobaltC's own way to ask for unchecked arithmetic, to measure what the checks cost |
| `bench.cb` / `bench.rs` | Both, one after the other |
| `strings.cb` / `strings.rs` | 100,000 words drawn with `Rng`: built with `sprintf`, counted in a `HashMap<String, u32>`, sorted, their lengths summed |
| `records.cb` / `records.rs` | 2,000 records in a `Vec`, each updated 500 times through an exclusive reference |
| `tree.cb` / `tree.rs` | A binary search tree of `Box`es: 60,000 inserts of keys drawn with `Rng`, then a recursive sum |
| `hashcount.cb` / `hashcount.rs` | 200,000 keys drawn with `Rng`, counted in a `HashMap<String, u32>` |

The programs that need random numbers draw them with `std`'s `Rng`;
`rng.rs` is the Rust twins' copy of it, written the same way, so every
build draws the same sequence and prints the same answer.

The first five are compute kernels; the last four are the shapes real
programs have — strings, maps, references, recursive structures — where
the runtime's checks of references would do most of the work if the
compiler could not prove most of them away (below).

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
- **Why GCC keeps that branch.** This comes from GCC, not from `cobc`:
  `cobc` writes the same C for both compilers, and the same loop written
  directly in C behaves the same under `gcc -O2` (tried with GCC 11.4,
  2026-10-03). On x86, GCC's if-conversion judges computing both `x / 2`
  and `3 * x + 1` and picking one with a conditional move too costly, so
  it jumps on the low bit instead, and about one jump in three goes the
  way the processor did not guess. Rewriting the C did not change that:
  a `?:` expression, both values computed before choosing, and
  `__builtin_expect_with_probability(…, 0.5)` all compile to the same
  jump. Raising GCC's limit, `--param
  max-rtl-if-conversion-unpredictable-cost=30`, gives the conditional
  move and Clang's misprediction count. `cobc` does not pass that
  parameter. It changes the choice for every branch in a program, and a
  conditional move is slower than a branch the processor predicts well.
  It is also an internal GCC setting that a later GCC may change or
  remove. A program that needs the branch-free loop under GCC can be
  built with `--cc clang`.
- **Making checked code branch-free.** `cobc` could compute both sides
  of such an `if`, overflow results included, pick one without a
  branch, and fault only if the chosen side overflowed: correct, since
  the fault still happens exactly when the rule says. Tried by hand on
  `cobc`'s output, it helps under Clang but not under GCC, and Clang is
  slower than GCC on the sieves, so `cobc` does not do it yet.
  `impl/STATUS.md` records the measurements.
- **`collatz_wrapping` under a newer Clang.** Clang 23 executes about
  12% more instructions here than Clang 14 (2026-10-07, on the i7 920),
  from the same C. Tuning for generic x86-64, it computes `3 * x + 1` as
  `lea` then `inc`, where Clang 14 used one three-operand `lea` (slow on
  Intel cores since Sandy Bridge): one instruction more per step. Rust
  uses LLVM too, and its loop is the same instruction for instruction,
  so the two builds now match on both counts. The time rises less, about
  6% in cycles, and not because of the extra instruction: built for
  this processor (`-march=nehalem`), Clang 23 keeps the single `lea` and
  is slower still. Its loop copies `x` to another register before
  testing the low bit, a copy that processors since Ivy Bridge make for
  free and the i7 920 does not, which likely lengthens each step. It is
  the C compiler's choice for this processor, not something `cobc` can
  change.
- **The sieves** are limited by memory more than by arithmetic at this
  size. `cobc`'s code executes more than twice as many instructions as
  Rust's, but much of the time goes to waiting for memory, which every
  build does alike, so the times are closer than the instruction
  counts. The sieves dominate `bench`.
- **`sieve`** makes its `Vec<bool>` with `Vec::filled` and then uses it
  only through `Vec::len` and indexing (`is_prime[m] = false`,
  `if (is_prime[k])`). The compiler proves nothing else can hold a path
  to it (a *confined* `Vec`), so its accesses compile to plain C with
  bounds and overflow checks, not calls into the runtime's alias and
  validity checks. `Vec::filled` of a plain element type is native: one
  allocation, filled at once. (Until 2026-10-08 the sieves built the
  vector with a `push` loop, because `Vec::filled` was then compiled from
  its CobaltC source, a `clone` per element, and doubled the run time;
  natively it is now about 9% faster than the loop. The Rust programs use
  `vec![true; n]` to match.)
- **`sieve_fn`** passes the vector to functions by reference. Each
  function uses its parameter only as a confined vector would be used,
  so it is compiled a second time, for confined arguments, with no
  checks through that parameter.
  Its `fill` still pushes, element by element, through the reference:
  that is the shape it measures.
- **`records`** updates structs in a `Vec` through `step(&mut ps[i])`, and
  reads them in `energy`'s `foreach`. The `Vec` is confined: each element
  is handed to a function that only reads and writes its fields and
  keeps nothing, field reads form no path, and the loop over the
  reference parameter reads only fields. So each call costs the bounds
  check and the function's own work.
- **`tree`** inserts into a tree of `Box`es and sums it. Descending is the
  recursive call alone: a `match` statement's arms each get their own
  cleanup scope, so the descending arm, which makes no temporaries, has
  none. Building a node is an allocation and a store: `Box::new` is
  native, and `None`, a `Node` built of plain fields and `None`s, and
  `Some(Box::new(…))` are values that own nothing a fault could leave
  behind, so they get no run-time object. What remains is a check per
  level that nothing else holds a path into the node, made inline while
  the runtime tracks no object over heap memory, the teardown of the
  tree at the end, and the allocator.
- **`strings`** builds words with `sprintf` into a `Vec<String>`, counts
  them in a `HashMap<String, u32>`, sorts them and sums their lengths. A
  `foreach` over `&words` whose body hands each `String` only to functions
  that read it and keep nothing (`String::len`, `String::clone`) reaches
  the element directly, with no run-time path: the loop holds the `Vec`
  shared throughout, so no read through it can fail. What remains is a
  run-time check on `words` or `counts` per push, per map lookup and per
  loop step (a `Vec<String>` that is also sorted is not one the compiler
  can prove confined), and the formatting, sorting, allocations and
  copies Rust makes too.
- **`hashcount`** counts `sprintf`-built keys in a `HashMap<String, u32>`.
  The map's operations are native code; each lookup still checks the
  map at run time, and the rest is the formatting, hashing and
  allocation Rust does as well.

`impl/COBC-PLAN.md` §10 gives the argument for each check the compiler
leaves out, and `impl/STATUS.md` what each change measured at the time.
