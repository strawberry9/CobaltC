# CHG-0427 — A decided program's drops ask of each buffer whether an object lies over it, and a call compared where it stands is the statement's own temporary

Status: implemented (2026-11-10), D-0315, D-0296; agenda item 3 (decided programs equal or beat checked Rust,
the owner's goal of 2026-11-04), Update 178
Affects: `impl/cbrt/src/lib.rs` (`cb_cells_held`), `impl/cbrt/include/cbrt.h` (`cb_cells_busy`;
`cb_drop_plain_buf` asks it), `impl/cobc/src/lower/equality.rs` (`drop_glue`), `frames.rs` (`objectless_drop`),
`exprs.rs` (`quiet_write_pathless`, the quiet write through a reference), `calls.rs`
(`lower_borrowed_operand_as`), `passes.rs`; their twins in `impl/selfhost/back/gen.cb` and `passes.cb`;
`header.cb` regenerated; `impl/tools/validate_c.py`. No rule changes. No program's outcome changes.

## What was wrong

1. **One object over raw cells anywhere sent every drop to the runtime's general path.** In a decided program a
   value with no object ends by its drop glue (`cb_dg_…`, D-0315 step 5), each call guarded by
   `if (cb_reclaimed_any()) cb_drop_local(…); else cb_dg_…(…);`: the runtime's walk wherever an object might lie over
   the cells the value frees. The test is global. One live `Rc` (its box is an object over raw cells) turns it on for
   the rest of the run, and a program that has spawned a thread holds the count at a constant (D-0202), so it is on
   from the first spawn. Every struct, `String`, `Vec` and `Box` the program ends then took `cb_drop_local`, its
   lock and its type-directed walk: CHG-0422 found it costing 21% of `cbc`'s cycles with `Rc` handles, and left it
   as a follow-up.
2. **A call's result compared where it stands had an object.** `column(r, &f.column[0..$]) == f.value` (a `String`
   or a key type, read in place by `cb_bytes_cmp` or `cb_eq_…`) lowered its operand as a value: in a decided program
   `take_result` gave it an object where it landed, with a statement scope to end it (`cb_new`, `cb_move_to`,
   `cb_stmt_push`, `cb_stmt_pop` and the runtime's end of the object), per comparison. D-0315 step 4 had made a call
   borrowed in part (`make().f`, `&make()`) and a discarded result the statement's own temporary; this operand was
   left out.

## What changes

1. **The glue asks of each buffer it frees.** A `String`'s glue ends it by `cb_drop_plain_buf`; a `Vec`'s and a
   `Box`'s first ask `cb_cells_busy(buffer, bytes)` and, where it answers yes, end that one value by
   `cb_drop_local`. `cb_cells_busy` is the page buckets' test (`cb_reclaimed_near`, D-0202) and, where a bucket is
   not empty, the runtime's exact one (`cb_cells_held`: an entry, live or not, over the cells; `release_cells`'s
   test). A struct's or an enum's glue only ends its parts, as the runtime's walk does (it never looks for an object
   over a struct's own cells). The calls lose their global guard: `cb_dg_…(&x, 0u)`. The value's end is the same
   (`[Destroy-Composite]`, `[Deallocate]`): the runtime's path for a buffer with an object over it, the same frees
   for any other. `cb_drop_plain_buf` asks `cb_cells_busy` too, not the buckets alone.
2. **A compared call is the statement's own.** In a decided body, a call compared where it stands whose type's end
   frees memory alone, or has a destructor of its own (noted), is taken with no object and dropped where its
   statement ends (`stmt_temp_drop`, its entry in the function's table so that a contained fault ends it too); where
   no statement of its own ends it, it gets an object as before.

Both in `cobc` and `cbc` (D-0296): `compare_c.sh` gives the same C from both on every tracked file.

## Measured

(Update 177's `cobc` against this one, idle machine, `perf stat`, user-space instructions and task-clock.)

- A decided program holding 100 `Rc<Decl>` that makes and drops a `Decl { String, Vec<i64> }` 3,000,000 times
  (`stress/update178/rc2.cb`): gcc 5.81 G → 1.71 G instructions, 946 → 252 ms; clang 5.84 G → 1.73 G, 907 → 244 ms.
- `stress/round9/scaling_owned.cb` (decided; each thread aggregates its own rows, its `matches` compares
  `column(…) == value`), the timed part over 5 runs: one thread 341–351 ms → 58–78 (gcc), 52–60 (clang); two
  189–225 → 28–41; four 151–219 → 15–39. Four threads were 2× one; now 3.5×.
- The price: where nothing lies over raw cells, a `Box`'s or a `Vec`'s glue now reads the count itself, once per
  buffer, where the call read it once per value. `bench/tree.cb` (a tree of `Box`es): gcc's instructions unchanged
  (259.5 M), Clang's 246.9 M → 263.4 M, its time within noise (254.5 → 257.2 ms, 31 runs). Reading the count once at
  the outermost call and passing the answer down in the depth argument cost more on both (267.3 M, 274.8 M) and was
  not kept.
