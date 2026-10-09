# D-0204 — Supervised structured tasks: CobaltC's concurrency model

Status: ACCEPTED (2026-10-10; the owner delegated the choice of model: "independently attempt to discover a
genuinely better concurrent model that best matches CobaltC", with the DNS-over-TLS/HTTP server as the flagship)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §11, §17
Depends on: D-0008, D-0009, D-0063, D-0202, D-0203
Affects: `spec/18` (`rule.fail.fault-unwind`), `spec/19`, `spec/21` §0 (prelude), `std::sync`, both tools

## Problem

D-0202 and D-0203 made threads run in parallel and stated what they guarantee. What a program does with threads is
still the 2026-09 design: spawn, join, a mutex, a channel. Concurrency is now meant to be a main selling point, and
the flagship is a server that holds many connections at once. Measured against that, four things are missing:

1. **A bug in one connection ends the server.** A checked fault anywhere ends the program (D-0009). That is right
   for a single computation. It is wrong for a server, where one handler's bug takes every other client down.
2. **Nothing can be stopped.** A thread waiting on a socket or a channel can't be told to give up: no timeouts on
   a whole task, no shutdown, no "first answer wins".
3. **A deadlock hangs** unless it is the main thread waiting alone on a channel.
4. **A thread is an OS thread.** A server with ten thousand idle keep-alive connections needs ten thousand of them.

## Candidate models

1. **Go: lightweight threads, blocking I/O, channels, `select`.** Cheap tasks without colouring functions. A panic
   still ends the program unless recovered with an exception-like `recover`, and goroutines are unstructured.
2. **async/await (Rust, C#, Python).** Cheap tasks, but every function is one colour or the other, a second
   calling convention, and new keywords. Against §9 and the owner's preference for few new tokens.
3. **Erlang/OTP: isolated processes, "let it crash", supervisors.** The model servers are built on, because a
   fault is contained to one process. Its isolation is paid for by copying every message, since processes share
   nothing.
4. **Structured concurrency (Trio, Kotlin, Swift).** A task cannot outlive its scope; failure and cancellation
   follow the scope tree.
5. **Supervised structured tasks** (this decision): 1's cheap blocking tasks, 3's containment, 4's structure. Each
   is fitted to what CobaltC already has.

## Why 5 fits CobaltC and nothing else does

- **CobaltC detects a bug at the access, before it does damage.** A checked fault means no invalid step was taken.
  That is the property Erlang gets from isolation, so CobaltC can contain a fault to the thread that hit it without
  copying, as long as no other code sees state the thread may have left half-changed. Two rules ensure that:
  - **A mutex the failed thread held is poisoned.** Locking it again faults.
  - **A thread lent an exclusive reference to another thread's data is not contained.** Its fault ends the program,
    as today, because the data it was changing belongs to someone still running.

  So D-0009's guarantee holds: no code continues past a failed check in a state that check protected.
- **Threads are already structured.** A handle joins its thread when the handle ends (`[Handle-Destructor]`), so
  a thread never outlives its scope. Failure and cancellation can follow that tree with no new construct.
- **Moving data into a thread already gives it the data** (D-0202 migration). Message passing costs no copy.
- **Blocking stays ordinary.** No colours and no new keywords: a task that waits parks, and the language is
  unchanged.

## Decision

- **Containment (`spec/18`).** A checked fault in a spawned thread ends that thread:
  - it unwinds as before, destructors running;
  - it cancels and joins its own threads first;
  - a mutex it held is poisoned;
  - the thread's result becomes its failure.

  The main thread's fault, or a fault in a thread lent an exclusive reference to another thread's data, still ends
  the program at once (`[Fault-Unwind]`).
- **Propagation.** `join(h)` of a failed thread re-raises the failure in the joiner, and so does a handle's
  destructor, unless its thread is already unwinding. So a program that never asks still fails as a whole, one
  scope at a time.
- **`try_join(h) : Result<R, ThreadFailure>`** (prelude): the failure as a value, never re-raised. Its `text` is
  the report the program would have printed. This is how a server supervises its handlers.
- **`cancel(&h)`** (prelude): the thread's current or next wait faults with `diag.thread-cancelled` (contained):
  - the waits are a join, a lock, a channel, sleeping, and a socket;
  - joining a cancelled thread reports that diagnostic as the failure;
  - a thread lent an exclusive reference is not cancelled: it finishes its work rather than stop halfway.
- **Poisoning.** Locking a poisoned mutex is `diag.mutex-poisoned`, whose report names the failure that poisoned
  it.
- **Deadlock (`diag.deadlock`).**
  - A thread whose wait would close a cycle of lock and join waits faults at once.
  - When every thread waits and none waits on a socket, a timer or input, every one of them faults.

  `diag.channel-deadlock` stays as the name for the main thread waiting alone on a channel. All of these faults
  are contained like any other.
- **Tasks stay OS threads, reused (`cbrt`).** `spawn` hands its body to an OS thread that finished one (D-0202
  phase 4) or starts one with a reserved, uncommitted stack. M:N scheduling was measured for and not adopted (see
  Results): the flagship's load fits in OS threads, and an M:N runtime would have to move a task's C stack between
  OS threads, which generated code holding thread-local addresses forbids. No rule depends on the choice -- a task
  is a thread, as `spec/19` defines one -- so it can be revisited when a measurement asks for it.
- **Data parallelism.** `std::collections::slice_parts(s, k)` splits an exclusive slice into `k` disjoint runs,
  each an exclusive slice, to give to `k` threads. Disjointness is the checker's ordinary ranged-path rule, so no
  new proof is needed. (A closure is called through an exclusive borrow of itself, `spec/15`, so one closure
  cannot be shared by several threads; a parallel `map` takes a `fn` item.)
- **Schedule exploration (both tools).** `coby --explore N prog.cb` and `cobc --explore N prog.cb` run the
  program under N seeded orders of its threads -- one thread runs at a time, the next chosen by the seed -- group
  the runs by outcome, and name a seed for each, which `--schedule-seed S` replays exactly. They exit 1 when runs
  differ. `coby` may switch at every step; a compiled program switches where threads interact (a `spawn`, a
  wait, a mutex released, a thread's end) and now and then between two runtime checks. A thread asleep or in a
  socket or input call leaves the schedule until the call returns.

## Rejected

- **async/await**: function colouring and new keywords for what parking does without either.
- **Containing every fault, including a thread lent exclusive data**: the lender would go on with half-changed
  data.
- **Unstructured, detached tasks**: a task outliving its spawner breaks what lets references be passed to threads
  at all.
- **`select` as syntax**: spawn, channel and cancel compose "first of several" without it.

## Compatibility impact

- A spawned thread's fault no longer ends the program the moment it happens. It ends it when the failure reaches
  the main thread through `join` or a handle's end, unless something on the way uses `try_join`. Until then other
  threads keep running.
- A program that waits forever on a thread that faulted is now told so (`diag.deadlock`, or the failure itself
  through `join`). Before, it ended at the fault.
- Programs that do not fault are unchanged.

## Results (2026-10-10)

- **Flagship load** (`stress/d0204/flagship`: a DNS-over-TCP server, one supervised thread per connection,
  `try_join` + `is_finished`; a client holding N connections open, two queries on each):

  | N | connect all | 2N round trips | server peak RSS | failures |
  |---|---|---|---|---|
  | 1,000 | 0.12 s | 0.64 s | 30 MB | 0 |
  | 5,000 | 0.57 s | 3.4 s | 127 MB | 0 |
  | 10,000 | 4.6 s | 12.0 s | 248 MB | 0 |

  About 25 kB per idle connection. That is the case for M:N gone: ten thousand open connections per process is
  beyond what a DNS-over-TLS resolver serves from one host, and the cost of a thread was not where the time went.
- **What the load test found and fixed:**
  - Heap ids past 127 set the bit `cbrt` uses to mark held paths, so a program with more than 127 threads alive
    faulted with a false `diag.aliasing-conflict` (present in Updates 91 and 92). Heaps are now capped at 128,
    and threads past that share the least-used heap, locked as a shared heap (`conf.many-threads-owning-frames`).
  - `TcpListener::bind` asked for the standard library's listen backlog of 128: a burst of more clients lost
    connections and their retries came a second later. It asks for 4096 (300 connections: 1.06 s → 54 ms).
  - Per-connection runtime work took the exclusive lock (`spawn`'s ids and counts, temporary paths, ranged
    borrows, reclaimed cells, socket waits marking the book): these are now atomics or local entries. Round
    trips for 3,000 connections: 2.8 s → 1.6 s.
- **Containment** holds under unwinds 200 calls deep with owned threads, files and vectors; 200 threads with a
  third failing; nested failures; cancellation of every kind of wait.

## Results after Update 93 (2026-10-10, CHG-0234)

- **The same load, after the second round:**

  | N | connect all | 2N round trips | per round trip |
  |---|---|---|---|
  | 1,000 | 0.12 s | 0.26 s | 0.13 ms |
  | 3,000 | 0.32 s | 0.85 s | 0.14 ms |
  | 10,000 | 2.1 s | 3.2 s | 0.16 ms |

  Per-connection cost no longer grows with the number of connections. The causes, each found by profiling:
  - every socket call viewed the literal `""`, and reclaiming a `str`'s bytes made every thread meet in the
    runtime on its address -- a `str`'s view now has no path (`type.str`: no object's cells);
  - an absent id (0) was locked as heap 0, which every thread shares;
  - 128 heaps shared by 10,000 threads -- now 256;
  - `cb_rebind` (a binding given a new value) stopped every thread, thousands of times per TLS handshake.
  On Linux a socket wait blocks in `ppoll` until ready, its deadline, or a `cancel`'s signal (`SIGURG`, blocked
  outside `ppoll`, so none is lost): an idle connection costs no wake-ups.
- **DNS over TLS** (`stress/d0204/flagship/dot_srv.cb`, `dot_cli.cb`): 1,000 and 3,000 TLS 1.3 connections at
  once, with every 50th client tripping a handler bug, garbage instead of handshakes, and silent clients. Every
  count exact: all good queries answered, every bug contained and logged, garbage refused, silent clients
  cancelled mid-handshake at shutdown (or refused by the handshake's own timeout). A handshake costs the server
  about 20 ms of CPU, almost all of it the checked X25519 and Ed25519 arithmetic: a matter for `std::crypto`'s
  speed, not for the model.
- **`cobc --explore`** over 138 thread programs: one outcome each, except one whose result the spec leaves to
  timing (a thread's borrow that conflicts only if the thread has not yet finished) -- both outcomes found. It
  also found the timing-dependent location of `[Deadlock]` (b), now one outcome.
- **Lent through a channel:** a thread that received `&mut v` through a channel was contained by both tools;
  locking a mutex whose interior may hold an exclusive reference now lends.

