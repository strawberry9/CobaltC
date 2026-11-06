# CHG-0230 — Per-thread heaps in the runtime

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; D-0202)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0202
Affects: `spec/conformance.md` (3.195.0); `impl/cbrt/src/lib.rs`, `impl/Cargo.toml`

## What changed

- **Heaps.** `cbrt` keeps the records of objects and paths, the reference slots and the reclaimed cells in heaps,
  one per running thread, plus heap 0 (the main thread's):
  - an id carries its heap in its top 8 bits (24 generation bits remain);
  - a path lives in its object's heap;
  - a spawned thread claims a free, empty heap, or a new one. Past 127 threads alive at once, the newest share
    the least-used heap, which is then locked even where a thread's own heap needs no lock (CHG-0233; it said 255
    here, but ids from heaps past 127 collided with the held-path mark);
  - each heap keeps its own pools of reused vectors.
- **Locks.**
  - Each heap has a spin lock on its own cache line.
  - A *local* entry takes only the heaps its work is in, in index order: its operand's, the heaps whose page
    buckets name the cells it works on, and its own thread's.
  - An *exclusive* entry takes the runtime lock, then every heap's lock, also in index order.
  - The threads, mutex owners and channel counts moved out of the runtime state, under a small lock of their own
    (`Book`). Channel and mutex natives (`cb_lock_bare`, `cb_unlock_signal`, `cb_event_op`) no longer stop every
    thread. A mutex is claimed within the same hold of that lock that finds it free.
- **Local entries.** Each one reckons its heaps, takes them, and checks that its work reaches no further. If it
  would, it enters exclusively before changing anything. Local entries:
  - `cb_new`, `cb_bind`, `cb_send`, `cb_recv`, `cb_move_to`, `cb_end_moved_out`, `cb_forget_obj`;
  - `cb_borrow`, `cb_read`, `cb_write`, `cb_borrow_check`, `cb_borrow_unminted`, `cb_paths_disjoint`;
  - `cb_send_ref`, `cb_recv_ref`, `cb_result_ref`;
  - the element entries (`cb_elem_borrow`, `_here`, `_datum`, `cb_elem_access_slow`);
  - releasing cells (`cb_release`, `cb_release_plain`, `cb_deallocate`, `cb_reallocate_plain`,
    `cb_vec_drop_plain`);
  - `cb_live_in`, `cb_live_at`;
  - a statement scope's end whose temporaries run no code.

  `cb_frame_push`, and the end of a frame or scope that holds nothing, take no lock at all.
- **Page buckets per heap.** Each heap counts, per page bucket, its reclaimed entries and the pages it holds
  reference slots on. They are kept only while threads run, and rebuilt from the tables when locking turns on.
- **Migration.** An object received from another heap moves into the receiving thread's heap under a new id: a
  spawn's argument (`cb_recv`) and a join's result (`cb_join`). It must be detached, have no path, not lie over raw
  cells, and hold no reference.
- **The global reclaimed counts while threads run.** `cb_reclaimed_entries` is held at a constant (`THREADS_ENTRIES`),
  so the generated code goes on to the page buckets, which stay exact. The runtime's own early exits ask
  `quiet_cells` (no entry, or none on the cells' pages). `go_solo` counts the entries again.
- **Layout.** The paths taken only while threads run are out of line and `#[cold]`. `impl/Cargo.toml` builds `cbrt`
  as one codegen unit, so its hot paths sit together.
- **Checks.**
  - A debug build asserts that no record of a heap the entry does not hold is touched.
  - Every exclusive entry inside a local one is reported as a runtime bug rather than left to deadlock.
  - `COBALTC_RT_STATS` prints the local, exclusive and migrated counts.
- **Conformance:** `conf.heaps-meet`.

## What did not change

No rule, diagnostic or observable behavior. `coby` is unchanged. A program with one thread takes no lock, as
before.
