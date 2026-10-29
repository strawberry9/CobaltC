# CHG-0387 — Light references: a decided function's references carry no record, in any program

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-29)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0292, D-0281, D-0283, D-0284, D-0290, D-0291
Affects: `spec/14-control-flow.md` 1.41.0 (§6 the cost contract); `spec/conformance.md` 3.283.0;
`spec/IMPLEMENTATION-NOTES.md` (the runtime contract); `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`,
`impl/cobc/src/lower/*.rs`, `impl/cobc/src/main.rs`, `impl/cobc/src/report.rs`, `impl/src/typecheck.rs`,
`impl/src/interp.rs`, `impl/src/lib.rs`

## What changed

In the runtime (`impl/cbrt/src/lib.rs`, `cbrt.h`):

1. **Token kinds.** `GEN_MASK` is 21 bits; bit 53 is `LIGHT`, bit 54 `ROOT`. `HeapArena::get`/`get_mut`/`remove`
   resolve no light token. `make_root` gives `cb_bind`, `cb_rebind`, `cb_reclaim`, `establish_reclaimed`,
   `cb_fn_box` and `cb_fn_copy` a light root (`LIGHT | ROOT | obj`), or a recorded root path under
   `COBALTC_RECORD=full`. `Obj.root` holds it; `cb_rebind` takes a binding to own its object only while the
   object is at the binding's own address (a later binding of the same object hands the same bits out again).
2. **`view(r, tok)`** reads a token's path whatever its kind: a light root is the object's own path (no
   projection, exclusive, no ancestors); a light reference is the whole object where a check needs a record (more
   clashes than its place has, never fewer); a path id is its valid record. Every entry that read `paths.get`
   for a token it is given reads `view` (`path_target`, `check_access_with`, `borrow_in`, `borrow_range_in`,
   `cb_read`, `cb_frozen_ok`, `cb_paths_disjoint`, `access_clashes`, `cb_direct_ok`, `cb_rebind`).
3. **Recording frames.** `TState.rec` per frame; `cb_frame_push` inherits, `cb_frame_push_fn` pushes a light
   frame, `cb_frame_push_rec` a recording one; `frame_rec()` is true in every frame under `COBALTC_RECORD=full`.
4. **Light borrows.** `cb_borrow`, `cb_borrow_unstamped` and `cb_call_self` from a light base in a light frame:
   `light_borrow` (a root: valid, holders checked with the explicit projection when the object has any; a light
   reference: alive) gives `LIGHT | obj`. `cb_borrow_check`, `cb_borrow_unminted` through a light reference in a
   light frame check nothing. `cb_read`, `cb_write` (`diag.overwrite-of-live-resource` for a resource target),
   `cb_write_quiet`, `cb_valid` (the object alive) through a light reference do no more; `cb_frozen_ok`,
   `cb_paths_disjoint`, `cb_paths_arrive`, `cb_direct_ok` take one as vouched for, as token 0; `cb_frame_hold`,
   `cb_frame_unhold`, `cb_call_done`, `cb_result_ref` of one do nothing; `cb_send_ref` carries one as it is.
5. **`cb_hold_ref(slot, tok, addr, ty, mode, file, line)`** replaces `cb_store_ref` where a reference value is
   stored (a binding's object for a stored parameter, `Fx::store`, a spawn's block, a thread's reference result,
   `Vec::extend_from`'s native): a light reference becomes a holder — `mint_in` with `proj_of_addr(obj, addr,
   ty)`, occurrence 1, no scope — a recorded one is stored as before. `cb_store_ref` of a light token aborts
   (a lowering hole). Slots of slices, views and guards keep `cb_store_ref`: their tokens are paths.
6. **`cb_rec_ref(tok, addr, ty, mode, kind, file, line)`**: a light reference arriving in a recording frame gets
   a path with its place's projection, held by the frame (`kind` 1: a parameter) or stamped into the statement
   (0: a result); a path or 0 is returned as it is.
7. **Places given.** `cb_borrow_range(base, cont, cty, …)` and `cb_borrow_range_unminted` take the container's
   place and type: through a light reference the slice's path is `proj_of_addr(obj, cont, cty) ++ Range`;
   `cb_destroy_via(tok, addr, ty, …)` faults `diag.move-out-of-field` unless the place is the whole object;
   `cb_lock(tok, m, ty, …)` makes the guard's path from the mutex's place.
8. **`proj_of_addr`** walks the owner's `TypeInfo` from the object's address to the place: fields by offset
   (zero-sized fields only for a zero-sized referent), array elements by size, an enum's payload by the tag read
   from memory, until the place's type is met; the deepest place found otherwise (under `COBALTC_RECORD=check`
   that aborts).
9. **Switches and counters.** `COBALTC_RECORD=full|check` read at `cb_init`; `COBALTC_RT_STATS` prints the roots
   handed out light and recorded, light references, paths minted (holders at stores, recorded on arrival),
   marked-owner checks, projections from addresses.
10. `verify_state` is unchanged: light tokens are in no table. The planner (`enter_planned`) needs no change: a
    light token's heap bits are its object's.

In the front end (`impl/src`): `Items::point_check_fns`, the bodies (`tier::body_key`) a point check was recorded
in (`tier_report::func`), beside `tracking_needed`.

In `cobc`:

11. `Fx::tracked` = the program is a library or `--all-checks`, or the body's key is in `tracking_needed` or
    `point_check_fns`; closures take their host's. `push_frame_fn` emits the function frame marker `R`
    (recording: always emitted, `cb_frame_push_rec`) or `G` (`cb_frame_push_fn`, elided where unused, as `F`);
    block frames keep `F` and inherit.
12. In a recording body: a reference parameter is recorded on arrival (`p.tok = cb_rec_ref(p.tok, p.p, TY,
    MODE, 1, …)`), whether its uses go through its own token (an unstored parameter, a confined vector, a `move`
    closure's `self`, through whose token its captures are read) or through its binding's slot (then the holder
    stored there is the record); a call's reference result (`finish_call`, the element borrow of `Vec::index_*`,
    a joined thread's reference result) is recorded after `cb_recv_ref`. `COBALTC_RECORD=check` over the
    dynamic cases and the tracked corpus found the `move` closures' `self` (read through its own token after the
    slot took the holder) before it was covered.
13. `cb_hold_ref` at the five stores of reference values; `cb_borrow_range` sites give the container (`(p).bytes`
    for a `String`'s view, the sliced place, `s->bytes` in `String::view`'s native, the `Vec` in
    `Vec::index_exclusive`'s element check; `NULL, 0` where the base is a slot's path); `cb_destroy_via` and
    `cb_lock` their places.
14. `resolve_scopes` knows `cb_hold_ref` (no scope effect) and `cb_rec_ref` (a statement's); `--report-tracking`
    counts both; `-v` says "kept paths: …; lowered with holders (D-0292)".
15. `spec/IMPLEMENTATION-NOTES.md`'s runtime contract regenerated (`cb_frame_push_fn`, `cb_frame_push_rec`,
    `cb_hold_ref`, `cb_rec_ref`, the changed signatures).

## Verification

- `cargo test -p cbrt`; the 167 dynamic conformance cases under GCC and Clang with `COBALTC_RECORD=check`
  (nothing aborted; every outcome as expected); the three-way differential over the 996 programs that keep paths
  (`stress/d0281/phase3/gate3.sh`: `coby`, light under check, full): the only differences are thread programs
  whose model headers admit both outcomes, re-run to show both modes give both.
- Two conformance rows added (`spec/conformance.md` 3.283.0), run under `coby`, GCC and Clang.
- The release suites (`stress/suites/runs/u147`).
- Measurements in `CHG-0387`'s changelog entry and `stress/d0281/phase3/BENCH.md`.
