# D-0202 — The runtime's lock under threads

Status: ACCEPTED (2026-10-09; the owner chose option 2, per-thread runtime state); phases 1 to 3 implemented
2026-10-10 (`CHG-0230`); phase 4 (`CHG-0231`) the same day
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §17
Depends on: D-0192 (lock work so far), D-0200 (fewer runtime calls)
Affects: `impl/cbrt/src/lib.rs` (the runtime's state and locking); no rule of the specification

## Problem

A compiled program's checks run in `cbrt`, and `cbrt` keeps every object and path in one table guarded by one lock.
While more than one thread runs, every runtime call takes that lock. D-0192 already did three things:

- it made it a reader-writer lock, read-only checks taking the shared side;
- it switched locking off entirely once the last spawned thread has ended;
- it made allocation lock-free.

What is left is contention. Measured on wordpar (round 8: a reading thread and a summing thread through a
`Channel`), about 28% of all CPU samples are spent acquiring and releasing the lock. Two threads doing
runtime-heavy work run about as fast as one, never faster. D-0200 removed most runtime calls from single-threaded
hot paths, which shrinks this, but every call that remains in threaded code still takes the lock.

For comparison: CPython has a global interpreter lock too, so Python's threads also do not run CPU-bound work in
parallel. The Python versions of the round-8 programs are single-threaded.

## Candidate mechanisms

1. **Keep one lock; keep removing runtime calls** (recommended for now). Every call compile-time discharge proves
   unnecessary is one lock acquisition fewer, in threaded code as elsewhere: D-0200's channel natives halved theirs.
   No risk to the runtime's design. Threads still do not speed up runtime-heavy work, as in Python.
2. **Per-thread runtime state.** Each thread keeps its own table of the objects and paths it alone can reach, with
   no lock. Only values that cross threads move to a shared table under the lock: spawn arguments, channel and
   mutex contents, join results. Most work in a thread touches only its own values, so contention would mostly
   disappear and threads would scale.

   This is a redesign of `cbrt`'s core: every object and path operation must know which table holds its operand,
   and a value's records move when it crosses threads. It would take weeks, a phased plan and its own full battery
   at each phase. `coby` is unaffected (its checks are its own).
3. **A sharded table** (the lock split by object id). Less contention per call, but a check that spans objects (a
   path's ancestors, a borrow across a struct's parts) needs several shards, in a fixed order to avoid deadlock.
   The gain is limited: every call still takes a lock.

## Recommendation

Option 1 now, with option 2 recorded as the path if thread scaling becomes a goal, for instance for the DNS-over-TLS
flagship serving many clients. The cost of option 2 is only worth paying for a concrete threaded workload that needs
it.

## Plan (accepted option 2)

Per-thread heaps. Each running thread keeps the records of the objects and paths it makes, and the reference slots
and reclaimed cells it registers, in a heap of its own with its own lock; ids carry their heap's index. The shared
places where threads meet are handled one by one:

- spawn arguments migrate to the new thread's heap;
- join results migrate to the joining thread's;
- a finished thread's leftovers merge into a shared heap;
- channel values already get a new object on receipt;
- a reference into another thread's data locks that thread's heap only while it is used.

Phases, each checked by parity, the concurrency cases, sanitizers and a full battery before the next:

1. a pure refactor (one heap);
2. per-thread heaps still under the global lock;
3. heap locks replace the global lock;
4. tuning.

`stress/d0202/DESIGN.md` has the details.

## Result (phases 1 to 3, 2026-10-10)

Implemented as `CHG-0230`.

**What was built.**
- Heaps, one per running thread.
- A spin lock per heap. Local entries take only the heaps they work in, which they reckon from ids and per-heap page
  buckets. Each entry checks that its work reaches no further before changing anything; if it would, it enters
  exclusively.
- A separate small lock for threads, mutex owners and channel counts.
- Migration of a spawn's argument and a join's result into the receiving heap.

**Still exclusive.**
- Entries that may run a destructor (a resource temporary or local ended).
- Entries that store or forget reference slots.
- The mutex guard's paths.

The heaps of finished threads are reused once empty, rather than merged into heap 0.

**Figures** (this machine, 4 cores; Update 90 → now):

| program | Update 90 | now |
|---|---|---|
| `stress/d0202/scale/scale.cb`, the same word count in each of W threads, W = 1 / 2 / 4 | 0.15 / 0.34 / 0.84 s | 0.16 / 0.20 / 0.30 s |
| wordpar, 1 / 4 workers | 0.61 / 0.85 s | 0.55 / 0.58 s |
| csvstat / extsort (one thread) | 241 / 645 ms | 241 / 644 ms |

Four separate one-thread processes of `scale.cb` take 0.236 s together, which is what this machine allows; four threads
in one process take 0.30 s. Before, more threads were slower than one.

**Lessons from tuning.**
- The paths taken only while threads run are kept out of line and `#[cold]`. Inlined, they grew the hot code
  enough to raise instruction-cache misses by a third on one-thread programs.
- `cbrt` is built as one codegen unit. With sixteen, its hot functions were spread across the text, and
  csvstat's instruction-cache misses tripled.
- While threads run, the global count of reclaimed entries is held at a constant. The generated code then goes on
  to the page buckets, which are exact and spread by page. Kept entry by entry, the count's one cache line moved
  between cores at every element borrow. A per-heap share, taken at a heap's first entry, was slower still: the
  runtime's early exits stopped firing.

## Result (phase 4, 2026-10-10)

Implemented as `CHG-0231`. Measured on three workloads written to hit what phase 3 left exclusive. Each runs W
threads doing the same work, so flat times are perfect scaling:

| workload, W = 1 / 2 / 4 | Update 90 | after phase 3 | after phase 4 |
|---|---|---|---|
| one shared mutex, 200,000 locks per thread | 0.41 / 2.93 / 10.3 s | 0.58 / 3.82 / 17.3 s | 0.09 / 0.16 / 0.62 s |
| structs of references, 300,000 per thread | 0.85 / 4.40 / 9.21 s | 1.39 / 4.37 / 16.3 s | 1.71 / 2.05 / 3.00 s |
| helpers owning Strings, per line | 0.10 / 0.14 / 0.20 s | 0.11 / 0.14 / 0.20 s | 0.11 / 0.15 / 0.22 s |

What made the difference:
- **Planned local entries.** Frame and scope ends, reference slots and `lock` no longer stop every thread.
- **Hashed page buckets.** All threads' stacks shared their buckets.
- **Brief spinning** before a wait sleeps.
- **Guard-free lock blocks in `cobc`.**
- **Reused OS threads.** 20,000 spawns and joins went from 2.97 s to 1.89 s; what remains is the kernel's wake-ups.

One thread with workers stored in structs of references pays for the planning: 0.85 → 1.71 s. A program that never
spawns takes none of these paths.

## Compatibility impact

None for option 1. Option 2 would change no rule or diagnostic, only the runtime's internals and its speed.
