# CHG-0432 — A call dropped at once takes no object in a decided program

Status: implemented (2026-11-10), D-0315, D-0296; agenda item 3, Update 178
Affects: `impl/cobc/src/lower/intrinsics.rs` (`drop`), `impl/selfhost/back/gen.cb` (its twin). No rule changes. No
program's outcome changes.

## What was wrong

`drop(e)` of a temporary lowered `e` as a value: in a decided program a call's result got an object where it
landed, a statement scope and a frame for it, and was ended by `cb_consume`, the runtime's path. `std`'s
`overwrite<T>` is `drop(replace(r, v))`, so every `overwrite` of a decided program paid that: in
`stress/vsrust/lzss.cb`, `overwrite(&mut hit, best_match(…))` per position, 16% of the run in the runtime's object
entries (`new_obj`, `end_identity_in`, `cb_stmt_pop`, `drop_in_place_tail`). D-0315 step 4 had made a call's
argument temporaries and its discarded results objectless; this one was left out.

## What changes

In a decided body, a call passed to `drop` whose type's end frees memory alone, or has a destructor of its own, is
taken with no object and ended where it stands by its drop glue or `cb_drop_local` (the objectless local's end,
`objectless_drop`): nothing runs between the call and its end.

## Measured

`stress/vsrust/run.sh`, Update 177's `cobc` against this one with CHG-0430 and CHG-0431 (time against checked Rust,
gcc / clang): lzss 1.93 / 2.08 → 1.13 / 1.14 (gcc 4.35 G → 1.70 G instructions).
