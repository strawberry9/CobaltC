# D-0192 — Performance close-out: resource elements, closure values, `from_fn`, the runtime lock, bare dual calls

Status: ACCEPTED (2026-10-09; the owner: "close of all open items that are performance related, and do this autonomously")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0188, D-0189, D-0190, D-0191
Affects: `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`; `spec/conformance.md` (3.185.0); `CHG-0220`

## Problem

The open performance items after D-0191:

1. `Vec` operations on elements that are not plain data (`String`, a
   resource with a destructor) took the prelude's bodies: about 1 µs an
   element for `swap`, `reverse`, `insert`, `remove`, `pop`, `truncate`,
   `clear`, the searches and `clone`.
2. A capturing closure held in a `fn` local, used as a predicate, kept the
   prelude's path (~1.2 µs an element; D-0191, "Not decided here").
3. `Vec::from_fn` with a closure literal pushed through the checked body
   (~35 ns an element).
4. The global runtime lock: after the last spawned thread ended, every check
   still took it; with two threads running, checks that only read the state
   serialized on it (two threads 450 ms against 185 ms sequential).
5. A D-0188 dual-body call always ran the dispatcher's disjointness test,
   even where the call site proves the arguments distinct.

## Decisions

1. **Movable elements.** An element type holding no reference and no `fn`
   value (a `String`, a resource of plain fields) is moved by its bytes:
   `cobc`'s native `swap`, `reverse`, `insert`, `remove`, `pop`,
   `truncate` and `clear` handle it, running destructors where the prelude's
   bodies run them (last first for `truncate` and `clear`), returning a
   removed value through the object protocol. `contains`, `index_of`,
   `binary_search` compare `String`s by their bytes; `clone` copies them.
   Any element with an object (a live borrow) sends the call to the
   prelude's body (`cb_live_in`), as before.
2. **Search keys** are pure direct: read, never kept. A place key is passed
   by address after its borrow's check, so a local used only as a key has no
   object.
3. **Capturing closure values.** A closure whose reference parameters are
   direct and never handed on, and which has no slice parameter, is marked
   pure when boxed (`cb_fn_mark_pure`); D-0191's call-time test then takes
   the plain loop for it.
4. **`from_fn` with a literal** runs as an inline loop that grows the vector
   as `push` would (the same doublings) and calls the closure in order.
5. **The runtime lock.** Once only the main thread runs again, locking stops
   (the next entry in the main thread switches it off). While several run,
   the lock is a reader-writer lock: checks that only read the state
   (`cb_read`, `cb_borrow_check`, `cb_borrow_unminted`, `cb_live_in`,
   `cb_paths_disjoint`, `cb_fn_pure`, a write to an initialized target) take
   it shared; a failing check trades the shared hold for the exclusive one
   before reporting. `cb_allocate` takes no runtime lock.
6. **Bare dual calls.** A dual-body function also gets a bare form (its fast
   half, its own element test for slices, slices not received). A call whose
   reference and slice arguments are each rooted at a distinct owned local,
   none mentioned by another argument, calls it directly after each
   borrow's check.

All of this is unobservable: the same checks are made in the same order, the
same faults and destructor output result (the conformance rows of 3.185.0).

## Results

| item | before | after |
|---|---|---|
| `Vec<String>` swap / reverse / remove (per op) | ~1.1 µs | 5–60 ns |
| `contains` over 10 `i64`, key a local | 482 ns | 16 ns |
| capturing closure value in `retain` | 1,237 ns/elem | 149 ns/elem |
| `from_fn` literal | 35 ns/elem | 3.8 ns/elem |
| single-threaded work after a join | 256 ms | 200 ms |
| two threads of runtime-heavy work | 450 ms | 200 ms (sequential 188) |
| D-0188 call, distinct locals | 1,544 ns | 391 ns |
| sole-slice function | 1,242 ns | 145 ns |

The last row is a defect fixed on the way: `used_as_value` took any bare
name in value position, including `std`'s locals (`total`, `len`), as
naming a function, so every function sharing a name with any local had no
pure direct slice parameters.

## Not decided here

Faster-than-sequential scaling for threads that both use the runtime heavily
needs per-thread runtime state: a redesign, not taken. `Option<String>` returns
(`pop`, ~900 ns) pay the general object protocol.
