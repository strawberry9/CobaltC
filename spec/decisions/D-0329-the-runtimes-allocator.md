# D-0329 — The runtime allocates small blocks itself

Status: ACCEPTED (2026-11-18). Decided by the implementer: the owner asked for the runtime's C, and the C the
compiler emits around it, to be as fast and as small as they can be made, and left the decisions to the
implementer ("I leave the decisions for you to make"); implemented by `CHG-0468`
Kind: Decision (`spec/02-schema.md`); the implementation, not the language
Depends on: D-0326 (the runtime in C), D-0315 (the generated C's inline paths), D-0202 (heaps and threads)
Affects: `impl/cbrt/src/rt_alloc.c` (new), `rt_refs.c`, `rt_objs.c`, `rt_init.c`, `rt_threads.c`;
`impl/cbrt/include/cbrt.h`; `impl/back/gen.cb` (`glue_free`), `impl/back/header.cb`; `impl/cbrt/SOURCES`,
`tools/exports.txt`. No rule of the language changes.

## 1. Problem

`[Allocate]` and `[Deallocate]` went to the C library's `malloc` and `free`. On the programs that allocate,
that was the largest single cost the runtime environment had in the default form: glibc's allocator took 20.5%
of the instructions the compiler itself executes (1.8 million allocations compiling `tinyos.cb`), and 15 to 30%
of the time of the `tree`, `hashcount` and `strings` workloads.

The C library's allocator cannot know a block's size when it is freed, so each block carries a header and each
free reads it; and it serves every thread from shared state, so each call pays for that too. The language gives
the runtime more: `[Deallocate]`'s side-condition (spec/21 §0) is that `(p, n, align)` are exactly what one
`allocate` returned, so the size is in hand at every free.

## 2. Decision

1. **Small blocks are the runtime's own.** A block of 1 to 1024 bytes at an alignment `malloc` gives (16) is
   one of 64 classes 16 bytes apart, named by its size: `(size + 15) >> 4`. It has no header. A heap is one
   list of free blocks per class; blocks are carved from 64 KB slabs taken from `malloc`, a batch at a time in
   address order; a freed block goes on its class's list. A slab is never returned to `malloc`.
2. **Everything else stays with the C library**: a block over 1024 bytes, an over-aligned block (recorded by
   address, as before), the runtime's own tables, closure boxes, and the sort's scratch buffers.
3. **One thread: in line.** While no thread was ever started, the replay trace does not record allocations, no
   over-aligned block exists and no sanitizer is on (`cb_alloc_gate` is 0), the generated C pops and pushes the
   lone heap's lists itself (`cbrt.h`: `cb_allocate_at_once`, `cb_deallocate_at_once`,
   `cb_deallocate_plain_at_once`, `cb_block_free`, `cb_reallocate_plain_fast`): an allocation is a load, a
   test and two stores.
4. **Threads: a heap each.** From the first spawn, and in a library from its start (any thread may call one),
   every thread allocates from its own heap through one thread-local pointer, out of line. The first thread
   to ask keeps the lone heap. A heap whose thread ended goes whole to the next thread. A list that grows past
   32 KB of blocks (at least 8) moves to a central list under a lock, and an empty list is filled from there
   before a slab is carved: one thread freeing what another allocates does not grow without bound.
5. **Under a sanitizer every block is `malloc`'s**, so that the sanitizer sees each block: when the runtime
   is built under one (it knows at compile time), and when the program is (its C says so before `main`, a
   constructor in `cbrt.h` under the sanitizers' macros, and the runtime then takes every block from
   `malloc`). The release profile's `runtime` unit and `cobc --cc` with a sanitizing compiler check what
   they checked.
6. **`realloc` for plain cells** (`cb_reallocate_plain`): two sizes of one class are the same block; two large
   sizes are `realloc`; otherwise a block of the new class, the bytes copied, the old block freed.

## 3. Consequences

- Measured on this machine (GCC, instructions and cycles, Update 197 -> Update 198, `CHG-0468`): the compiler
  compiling `tinyos.cb` -22% and -17%; `hashcount` -28% and -22%; `strings` -21% and -28%; `tree` -47% and
  -16%. Peak memory of the allocator's own test: 3.4 MB where glibc used 2.7 MB.
- **A wrong size at `deallocate` is now a real fault of trusted code**, where the C library forgave it. The
  specification already made it a false trusted claim; `std` passes the size it allocated everywhere (the
  whole suite and the recorded programs pass). User code under `unsafe` must do the same.
- **A block from `allocate` is no longer one `free` accepts**, and a `malloc` block is not one `deallocate`
  accepts unless it is over 1024 bytes. Foreign code must not free a CobaltC block or hand CobaltC a block to
  deallocate. (The specification never said they were the C library's.)
- The generated C and the header had called `free` and `realloc` on program blocks directly (D-0315's fast
  paths, `tree_drop`): each such place now goes through the allocator.
- Memory freed in one class is reused only by that class, and slabs are not returned: a program that frees
  many small blocks of one size and then needs another size keeps both. The classes are fine-grained (16
  bytes), so what a class holds is what the program once used at that size.
- Valgrind sees slabs, not blocks, in a normal build; the sanitizer build sees blocks.
- A library's threads (not made by the runtime) get a heap at registration, returned by their exit hook; the
  first registered thread's heap stays for the life of the process.

## 4. Alternatives considered

- **Keep `malloc`.** Simplest, and what a C program does; costs the numbers above.
- **A header-carrying allocator** (measured as an experiment, preloaded): -17% instructions on the compiler
  but `tree` slower in cycles (+14%): the header makes a 24-byte node 48 bytes. The size at free makes the
  header unnecessary.
- **Pages per class with reclamation** (a mimalloc-like design): returns memory across classes; more code and
  a page lookup at each free. Not needed for the measured gain; can replace the lists later without touching
  the interface.

## 5. Verification

`CHG-0468`, "Checked".
