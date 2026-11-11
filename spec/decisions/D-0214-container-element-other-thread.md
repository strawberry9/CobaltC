# D-0214 — A container may not end an element another thread holds an exclusive path to

Status: ACCEPTED (2026-10-11; the owner: "action the plan in full autonomously and using the recommendations as written" — `ChatGPT_review/ACTION-PLAN-2026-10-11b.md` WI-2, Q2)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §6, §12, §17 (aliasing and concurrent access, external interfaces), §20
Depends on: D-0017, D-0019, D-0206, D-0209, D-0213
Affects: `spec/20` 1.14.0, `spec/08` 1.4.1, `spec/21` (the trusted table, regenerated), `spec/conformance.md` 3.209.0
(CHG-0252); `impl/std/collections.cb`, `impl/std/memory.cb` (trusted rationales); `impl/src/interp.rs`,
`impl/cbrt/src/lib.rs`; three cases

## Problem

The second independent review (F-03) found the trusted rationale for `Vec::pop` false: "no path reaches [the
last slot], since an index borrow would have clashed with the &mut v argument". A vector's element is a
reclaimed object of its own (`spec/20` §2), so an exclusive borrow of the vector does not clash with a path to
the element. A program that keeps `&mut v[2]` where the flow analysis does not follow it (in another vector, in
a struct) and then pops: `pop` raw-reads and releases the slot under the live path. In one thread this is
sound — the path is not used while `pop` runs and is stale afterwards (`spec/08` §4, H5; both tools fault
`diag.stale-binding` at its next use) — but `[Rawptr-Read]`'s trusted condition ("no live derived path …
in exclusive mode") was false when the read ran. With the path in another thread, the read and that thread's
writes race: `stress/review2_oct11/f03d.cb`.

## Options

1. **Check another thread's exclusive paths at `[Release]` and `[Rawptr-Move-Out]`** (the plan's
   recommendation): fault `diag.aliasing-conflict` when another running thread holds an exclusive path to an
   object being ended; keep the one-thread behaviour (stale at next use).
2. **Fault for any live derived path**, the same thread's included: stricter, but it changes the documented
   one-thread outcome and costs a check per element operation in every program.

## Decision

Option 1.

1. `spec/20` §2: `foreign-held(o, ℓ, Σ)` — a valid, held, exclusive derived path to `o` whose thread is another
   running one, or in transit (D-0213). `[Release]` checks it for every object it ends; `[Rawptr-Move-Out]`
   checks it for the object it moves out of. A path of the operating thread's own ends with the object.
2. `[Rawptr-Move-Out]`'s old premise — every valid path on the object invalid or absent, else
   `diag.move-while-aliased` — was checked by neither tool (both ended the object and left the path stale);
   it is replaced by the above, so the rule says what both do.
3. `[Rawptr-Read]`'s trusted condition speaks of another thread's exclusive paths: one of the reading thread's
   own is not used while the read runs.
4. The trusted rationales of `Vec::pop`, `replace`, `take_raw`, `grow`/`reserve`, the priority queue's moves and
   `Box::into_inner` cite the check (another thread) and H5 (this thread); the table is regenerated. Every `std`
   body that raw-reads an element ends it, or releases its buffer, in the same operation before the value read
   is used, so the check made at the release stands in front of the read.
5. With D-0213, an exclusive path reaches another thread only lent, so a path's thread decides the case.

## Results

- `cbrt`: `foreign_held`/`check_not_foreign` in `release_cells`, `release_plain_cells` (so `cb_release`,
  `cb_deallocate`, `cb_reallocate_plain`), `cb_raw_move_out` and the identity `cb_raw_move_in` replaces; only
  while another thread runs. `coby`: the same at `release`, `deallocate`, a resource's raw move-out and raw move-in.
  A `Box`'s teardown needs nothing new: destroying its value requires `solitary`.
- `stress/review2_oct11/f03d.cb`'s shape is now `conf.vec-pop-element-held-by-other-thread`: `main`'s `pop` faults,
  in both tools, at the `pop` (`--explore 40`: one outcome). The one-thread shapes are pinned as
  `conf.vec-pop-stored-element-ref` and `conf.vec-pop-stored-shared-element-ref` (stale at the next use).
- `conf.direct-param-element-grown-by-other-thread` and `conf.frozen-param-element-grown-by-other-thread`
  (D-0209) are unchanged: their element paths are shared.
- Re-timing (`stress/perfguide`, `bench`, instructions by `perf`, GCC and Clang) against the Update-100 counts: every perfguide example within 2% (E3 b +2%, eight instructions per call; E2 a and b +1.8%; the rest under 1%), `bench` within 1.1% (`strings` +0.9%, `hashcount` +1.1%, the rest unchanged); the checks cost a load of `cb_live_threads` where a program runs one thread.
