# CHG-0231 — Threads: the runtime's remaining stops, mutexes, thread reuse

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; D-0202 phase 4)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0202, CHG-0230
Affects: `spec/conformance.md` 3.196.0; `impl/cbrt/src/lib.rs`, `impl/cobc/src/lower.rs`

## What changed

- **Planned local entries (`cbrt`).** An entry whose work may cascade reckons, under a local hold and before
  changing anything, every heap that work will touch (`Plan`, `enter_planned`). It widens its hold until the hold
  covers them, or enters exclusively where the work would run code of the program. Each plan step mirrors the
  function it plans for:
  - `drop_occ`, `retire_token` (with an ephemeral element object ending);
  - `forget_slots`, `end_identity`, `destroy_obj`;
  - a guard's release, a plain vector's buffer.

  Local now:
  - the end of a frame or statement scope;
  - the reference-slot entries (`cb_store_ref`, `cb_load_ref`, `cb_copy_datum`, `cb_send_datum`,
    `cb_recv_datum`);
  - moves and ends of values holding references;
  - `cb_lock`: its wait holds only the book's lock.

  Only destructors of the program, boxes, handles and `fn` values still stop every thread.
- **Waiting.** `wait_until` asks a few times, briefly, before sleeping: most waits for a mutex are for another
  thread's few steps.
- **Page buckets.** A heap's page buckets are indexed by a hash of the page. Every thread's stack sits at the same
  offset of an equally sized region, so by low bits all stacks shared their buckets, and every slot operation on a
  stack reckoned every heap.
- **The address tables** consult only the thread's own heap when a local hold has locked it alone (`sole_heap`).
- **Thread reuse.** An OS thread that finishes a job waits for the next `spawn` (at most 64 wait). A `spawn` no
  longer creates a thread and its stack each time.
- **Bare guards (`cobc`).** `auto g = lock(m);` whose block uses `g` only as `*g` or `g.f…`, read or written, plain
  data, leaves the block only at its end, and does not name what `m` names, is lowered without a guard object:
  `cb_lock_bare`, direct access to the interior, `cb_unlock_bare` as the block ends (`bare_guard_let`,
  `lower_bare_guard`).
- **Nested local entries** rely on the debug build's per-access assertions instead of comparing heap sets. Two
  reckonings of a heap set can differ while other threads change their page buckets.
- **Conformance:** `conf.mutex-bare-guard`, `conf.mutex-bare-guard-reentrant`, `conf.spawn-threads-reused`.

## Not done, and why

The heaps of finished threads are not merged into heap 0. A heap is reused by the next thread once it is empty,
and objects left in one are reached by their ids as before. Merging would move records for no gain measured.

## What did not change

No rule, diagnostic or observable behavior. A program with one thread takes no lock, as before.
