# D-0261 — Verify modes: the tools re-derive what their checks rely on

Status: ACCEPTED (2026-10-17, the owner: "proceed", on the plan `private/verification-plan-2026-10-17.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §12, §17, §20
Depends on: D-0004, D-0018, D-0107, D-0202, D-0206, D-0219
Affects: `spec/IMPLEMENTATION-NOTES.md` §5; `impl/src/interp/verify.rs`, `impl/src/interp/access.rs`,
`impl/src/interp/exec.rs`, `impl/cbrt/src/lib.rs`, `impl/battery.sh`, `README.md`; `CHG-0325`

## Problem

The invariants of `spec/03` are stated as rules and checked at every access by both tools, but the checks
decide from bookkeeping the tools keep incrementally: `coby`'s index of which objects hold references to
which, `cbrt`'s per-object token lists and ancestor lists. A defect in that bookkeeping is a check that
never fires, and nothing tested the bookkeeping itself against the state it summarises. The plan toward
proofs (A2 and B2 of `private/verification-plan-2026-10-17.md`) asked for a mode in which each tool
re-derives, from its whole state and independently, what its checks assume, so that every existing test
program becomes a test of the checks' inputs and a failure is a concrete counterexample.

## Options

- **A. Assertions in the normal build.** Free to run but paid by every program; the whole-state scan is
  linear in the state per check, so it cannot be the default.
- **B. A mode (`COBALTC_VERIFY`), selected by the environment, with a stride.** The same binaries; the
  battery runs the suites under it; a long program takes every Nth check. **Selected.**
- **C. A separate build (a cargo feature).** Two binaries to keep; the stamp would differ; no gain over B.

## Decision

`COBALTC_VERIFY=N` (1 for every time; 0 or unset is off), and in either tool no more often than its state's
size allows — once per 64 live objects' (entries') worth of events — so a check, which scans the state,
costs a bounded factor however large the program: at every event, the interpreter took over a hundred
times a heavy case's time (`x509_refusals_ok`, 6.5 s alone), and no stride short of the state's size helped. `coby`: at every Nth alias check, the distinct
references to the object that the holder index yields must equal those a scan of every live object and
of raw storage's side table yields; at every Nth statement's end, the index lists every holder, every live
guard's mutex is locked and every locked mutex has its guard (`inv.concurrency-validity`), and no live
object is the value of two bindings (`inv.resource-authority`). `cbrt`: at every Nth statement's end, every
valid path's object exists and lists it, every listed token names a path of that object, the exclusive
list is exactly the non-root exclusive ones, and every ancestors list is closed. A violation panics naming
the invariant and the state; it is the tool's defect, never the program's.

What is deliberately not checked as a property of the state: `inv.alias-validity` itself. It is a property
of accesses (D-0018; the proposition in `spec/03` quantifies over an access being performed), and a state
between statements may legitimately hold two conflicting paths — a call's arguments, pending until bound
(D-0107) and held in its parameters until one is used (`copy_in(&mut x, &x.b[0..4])`, `swap(&mut a,
&mut a)`). A first version checked the state pairwise and refuted exactly those programs, which is how the
distinction was learned; the check now re-derives the per-access predicate's inputs instead. Likewise
`coby`'s frame ownership lists and `cbrt`'s frame holds may name an object or path that has ended (a
destructured source, an element released by another thread's `grow`, an elided slot), which both tools'
sweeps expect; those are not violations.

## Results

Under `coby`, the whole suite (1,029 cases, 400 s against about 150 s plain), the showcases and `std`'s
134 tests run with no violation once the two over-strong checks above were withdrawn; under `cbrt`, the
suite compiled with GCC (615 s against about 330 s) and `std`'s tests likewise; of the showcases under
both modes, one (`token_gate`, Argon2 under the interpreter) exceeds the runner's 900 s limit.
No defect in either tool's bookkeeping was found by this round; the mode is in the battery (stage 1,
`VERIFY_STRIDE`) so that every future program runs it. The remaining steps of the plan — the written
meta-theory per rule, the runtime's contract table, trace equivalence between the tools, Miri and
ThreadSanitizer on the runtime (Miri needs a nightly component installed), and the mechanised fragment —
are each their own work. ThreadSanitizer through a `clang` wrapper (`stress/verify/tsan`, under `setarch
-R` as ASan needs on this machine) reports as races the runtime's hand-offs to a spawned thread, whose
synchronisation is Rust's futex-based and invisible to it: a meaningful run needs the runtime itself
instrumented (a nightly `-Zsanitizer=thread` build of `cbrt` and its `std`), which is that follow-up's
first step.

**ThreadSanitizer on the instrumented runtime (2026-10-18).** `cbrt` and Rust's `std` built with
`-Zsanitizer=thread` (`-Zbuild-std`, a separate target directory), the C side by `clang -fsanitize=thread`,
both served by Rust's own sanitizer runtime (clang 14's lacks `__tsan_memset`, which instrumented Rust
calls), and every `expect: ok` conformance case that spawns (105 of them, `stress/verify/tsan/run_tsan.sh`)
run under it. Two data races in the runtime, both real and both fixed:

1. The per-heap tables' count (`Heaps<T>`, D-0202): a spawn, under the runtime lock, pushed a new heap's
   place while other threads read the vector's length lock-free on their own heaps' fast paths. The
   places never move (the vector was pre-sized), so the race was on the length alone; it is now an atomic
   count over a fixed slice of uninitialised places, published with release and read with acquire. (The
   first form of that fix, a slice of `Option<T>`, was itself a race the next run found: iterating the
   places read each one's discriminant, which Rust keeps in a niche inside the table, a word another
   thread's own-heap fast path writes. A place is now reached by reference and never read until its
   count says it is filled.)
2. A fault raised outside every entry: a cancelled socket wait (`cb_net_op`, D-0204) faults from
   `outside` the runtime, holding nothing, and `fault_lock` took the exclusive lock only for a thread
   inside a local or read-only hold. The contained unwind that followed ended the thread's objects,
   scanning every heap's reference slots with no lock at all (`holds_heap` reads "no local hold" as "the
   exclusive hold"), while any other thread, including a second cancelled one, ran. Two cancelled threads
   unwinding at once were the report (`thread_cancel_socket_ok`). `fault_lock` now enters exclusively
   for a thread that holds nothing.

After the fixes the sweep is clean, as are the 18 showcases and the 34 stress programs that spawn. The
sweep is not in the battery (it needs the nightly toolchain and an instrumented build); it is the round's
tool, rerun by hand when the runtime's locking changes.

**Kani (2026-10-18).** The plan's bounded model checking, tried: `cargo kani -p cbrt` runs three
harnesses at the end of `cbrt/src/lib.rs` (`#[cfg(kani)]`, nothing in a normal build) over the runtime's
smallest tables — an arena's ids name their entry until removed and a reused slot gets a new generation;
a heap set holds exactly what was added and visits it in order; the per-heap places are read only once
filled and `len` counts them — each proved for every input within its bound (the places over a heap
count of 8 under Kani, 256 otherwise), with Rust's undefined behaviour checked on the way, in under a
minute. A fourth, over the reference-slot map (three slots across a page boundary: insert, replace,
scan in order, remove, the emptied page dropped and reused clean), does not finish: the map is
hashbrown-backed, and the checker cannot unroll its probing and hashing within an hour; it waits for a
Kani with hash-map stubs. Kani is a dev tool (`cargo install kani-verifier && cargo kani setup`), not a
dependency, and not in the battery; the harnesses are where the next table's proof goes.

## Revisit conditions

A check that the per-access predicate's two implementations agree (`coby`'s `clash` against a second,
independent derivation); the lock check in `cbrt` (its locks are keyed by address, its lock paths by
projection, so the pairing needs type information); a cheaper scan when the mode is wanted at stride 1 on
large programs.
