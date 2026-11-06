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
| `strings.cb` / `strings.rs` | 500,000 words drawn with `Rng`: built with `sprintf`, counted in a `HashMap<String, u32>`, sorted, their lengths summed |
| `records.cb` / `records.rs` | 2,000 records in a `Vec`, each updated 100,000 times through an exclusive reference |
| `tree.cb` / `tree.rs` | A binary search tree of `Box`es: 60,000 inserts of keys drawn with `Rng`, a recursive sum and the tree's end, ten times |
| `hashcount.cb` / `hashcount.rs` | 2,000,000 keys drawn with `Rng`, counted in a `HashMap<String, u32>` |

The last four were made longer on 2026-11-05 (each ran for under 0.1 s,
too short for a time to mean anything): their counts are not comparable
with earlier runs'.

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

Every workload here is a *decided* program (D-0284; `cobc -v` says
"lowered pathless and decided"): the static pass settles every
conflict between references before the run, so the executable keeps no
record of references and checks none at run time. What a `cobc` build
still does that Rust's does not is the checks the rules leave to the run
whatever the references (bounds, overflow, a `Box`'s or element's
object where the runtime may have placed one), and the costs of its own
code generation.

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
  size: most of the time goes to waiting for memory, which every build
  does alike, so the times are closer than the instruction counts. The
  sieves dominate `bench`.
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
  that is the shape it measures. A loop whose one use of a vector is such
  a push, and which calls nothing else, keeps the vector's pointer,
  length and capacity in locals (D-0309): stored to an element of
  `Vec<bool>`, a byte that C lets alias anything, the store would
  otherwise make the C compiler load the header again for each element.
  `mark`'s loop runs in a copy without its bounds checks where its bound
  is within the vector's length. What remains is `count`'s loop, which
  LLVM unrolls by two and GCC does not.
- **`records`** updates structs in a `Vec` through `step(&mut ps[i])`, and
  reads them in `energy`'s `foreach`. The `Vec` is confined: each element
  is handed to a function that only reads and writes its fields and
  keeps nothing, field reads form no path, and the loop over the
  reference parameter reads only fields. So each call costs the bounds
  check and the function's own work. Under GCC the loop also keeps the
  statement's location (one store before the call, which a caller makes
  for any callee) and an overflow flag it never reads: more instructions
  than Rust's, about Rust's time.
- **`tree`** inserts into a tree of `Box`es, sums it and ends it, ten
  times. Descending is the recursive call alone, a jump since D-0309's
  self tail call: a `match` statement's arms each get their own cleanup
  scope, so the descending arm, which makes no temporaries, has none.
  Building a node is an allocation and a store: `Box::new` is native,
  and `None`, a `Node` built of plain fields and `None`s, and
  `Some(Box::new(…))` are values that own nothing a fault could leave
  behind, so they get no run-time object. Since D-0298 a `Box` is one
  pointer and `Option<Box<Node>>` is the pointer alone, `None` being
  null, so a node is 24 bytes as in Rust. The tree's end is a loop of
  the runtime's own that follows each node type's plan of its `Box`es
  and frees each node at once. Three things the allocator does made the
  rest of the difference, none of them `cobc`'s code: a 24-byte node is
  a 32-byte chunk after glibc's 16-byte header, and the runtime's own
  blocks left the heap so that every second node crossed a cache line
  (the runtime now leaves it as a fresh process's, Rust's case); the
  walk's own stack, on the heap, was the one block too large for
  glibc's fast bins, and freeing it merged every node freed before it
  (the stack is now on the machine stack); and standard output's buffer
  was made at the first `printf`, after a tree was freed, with the same
  effect (it is now made before the program runs).
- **`strings`** builds words with `sprintf` into a `Vec<String>`, counts
  them in a `HashMap<String, u32>`, sorts them and sums their lengths. No
  push, map lookup or loop step checks a reference at run time. The
  `sprintf` is written in place by the compiler (D-0297: the literal's
  bytes and the number's digits, straight into the result, with no call
  to the runtime's formatter), and `Vec::sort` of text is the library's
  stable sort, as Rust's is: `cobc`'s code executes fewer instructions
  than Rust's. What remains of the time is chiefly the allocator: one
  allocation per word and per map key, freed at the end. Its locals end
  last bound first, as Rust's do (D-0008's order): ended the other way,
  the map's table, a large block, was freed after half a million small
  ones, and glibc merged them all first (a fifth of the run).
- **`hashcount`** counts `sprintf`-built keys in a `HashMap<String, u32>`.
  The map's operations are native code and, the program being decided,
  no lookup checks the map at run time; with the key written in place
  (D-0297; the runtime's general formatter was 41% of the cycles before),
  `cobc`'s code executes under half the instructions Rust's does and
  finishes first.
- GCC takes a function called only from `main`, as the program's `main`
  is, for code run once, and compiles its loops for size: a constant
  divisor becomes a `div`. `cobc` calls `main` through a volatile
  pointer so that it does not (CHG-0393); `hashcount` was 43% slower in
  cycles without it, `strings` 11%, `tree` 7%.

The virtual machine's speed moves between runs by 10% or more, Rust's
column included, on the same code. Compare columns within one run, and
the instruction counts across runs.

`impl/COBC-PLAN.md` §10 gives the argument for each check the compiler
leaves out, and `impl/STATUS.md` what each change measured at the time.
