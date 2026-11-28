# CHG-0393 — Box-tree teardown, dispatch on vouched tokens, and quiet writes over a variant that owns nothing

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-31)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0188, D-0194, D-0209, D-0219, D-0281, D-0294
Affects: `impl/cobc/src/lower/vec_calls.rs`, `impl/cobc/src/lower/user_calls.rs`, `impl/cobc/src/lower/exprs.rs`,
`impl/cobc/src/lower/functions.rs`, `impl/cbrt/src/lib.rs`; `impl/selfhost/back/{bodies,gen}.cb` in the same
Update (D-0278 as resumed by D-0296)

## What changed

The bench of 2026-10-31 had `bench/tree` at 125 M instructions against Rust's 30 M. Its profile named four causes
in the compiler and the runtime, and the fourth's fix uncovered a fifth; none is a rule change.

1. **The direct twin called on vouched tokens** (`lower/vec_calls.rs`: `zero_token_call`, `direct_dispatched`;
   `lower/user_calls.rs`). A call whose every direct reference argument is `((cb_ref){ addr, 0 })` -- a path the
   caller vouched for, which `cb_direct_ok` passes whatever else holds (D-0209) -- now calls `NAME__dir` itself,
   as `direct_local_call` already did for a borrow of the caller's own local. The dispatcher's `toks[]` array,
   its address taken, had made every such call a frame with a stack-protector canary under GCC's defaults. The
   tail call stays a call (`-fno-optimize-sibling-calls`, `conf.tail-recursion-stack-exhausted`).
2. **A Box-linked tree destroyed by a short walk** (`cbrt`: `TypeInfo::tree`, `tree_parts`, worked out once in
   `cb_init` as the greatest set of types whose destruction frees Boxes and nothing else -- no destructor,
   handle, guard, `fn` value or vector anywhere inside; `tree_drop`, `tree_visit`, `tree_parts_drop`,
   `tree_free`; `HeapSlots::all_empty`). While this thread is alone on the runtime, `drop_in_place_tail` hands
   such a value to it: each Box over quiet cells is queued and its referent destroyed in turn, in constant stack;
   a Box with an object over its cells goes to `box_drop` as before, and a cell's release is `free` alone when no
   reference slot is held in any heap and no object lies over the cells, else `cb_deallocate`'s. The general walk
   had cost about 700 instructions a node (five descriptor lookups, a slot scan per Box); this one about 130
   including `free`.
3. **A quiet write skipped where the old value owns nothing** (`lower/exprs.rs`: `quiet_write_pathless`). The
   pathless `cb_write_quiet(0, …)` (D-0194, D-0294) over an enum that is not an owner is guarded inline by the
   old value's tag: a variant with no payload or a plain one owns nothing, so the runtime is not asked. `n.left =
   Some(…)` over a `None` makes no call.
4. **The all-zero token test inline** (`lower/functions.rs`: `lower_fn_direct`'s dispatcher and `__arrive` twin,
   `lower_fn_frozen`'s direct tokens). Tokens all 0 pass `cb_direct_ok`; the dispatchers test that before calling
   it, so a reference passed on with no record (`Rng::below`'s to `Rng::next_u64__arrive`) costs no runtime call.
5. **`main` called through a volatile pointer** (`lower/program.rs`, `selfhost/back/gen.cb`: `cb_program`). Item 4
   let GCC see `Rng::below`'s dispatcher reduce to its direct body and inline it into `main`'s loop in
   `bench/hashcount`, which then ran 43% more cycles on 21% fewer instructions: called only from `cb_program`,
   itself run once from `main`, the program's `main` is taken by GCC as executed once, and its blocks -- loops
   included -- are compiled for size, so `% 5000` and the formatter's `/ 10` became `div` instructions (40-90
   cycles each on the bench machine). Called through `void (*volatile cb_main_fn)(void)`, `main` is an ordinary
   function to GCC. Cycles under GCC, the same C otherwise: `hashcount` -37%, `strings` -11%, `tree` -7%,
   `bench` -2.5%, `sieve`/`sieve_fn`/`records` under 1% less, `collatz` +2.7% with the same instructions (code
   placement); under Clang no change beyond noise.

Found on the way and left as decided: a tail call must stay a call (`conf.tail-recursion-stack-exhausted`), so
the recursion's frames (about 14 M cycles here) are the language's; and the node's size, which is D-0298.

## Verification

- `stress/tree_gap/td1.cb` (tree shapes, a million-node chain, arrays and vectors of Boxes, a Box of a Box, a
  type with a destructor behind a Box, Strings and references behind Boxes, a tracked program), `td2.cb` (a chain
  destroyed while another thread runs) and `td3.cb` (quiet writes over `Some`, payload variants, through a
  direct parameter and into locals): `coby`'s output and `cobc`'s under GCC and Clang, with and without
  `--all-checks`, the same bytes; valgrind on `td1` and `td3` under GCC, no invalid free and no definite leak.
- `cargo test --release -p cobc` for `runtime_bookkeeping`, `tracking`, `differential`, `driver`, `extern_code`,
  `library`, `output_order`, `read_input`, `returned_view_inlined` and the binaries; `cargo test -p cbrt`;
  `cargo check --workspace --tests` for Linux and `x86_64-pc-windows-msvc`; `cargo +1.77.2 check`; the Windows
  runtime rebuilt.
- Before Update 154: `cbc` writes `cobc`'s C byte for byte on every tracked program, in the default form and
  under `--all-checks` (`compare_c.sh`); the undecided sweep (8,854 programs decided before) has every one still
  lowered decided, the 28 the static pass does not decide as before and one static rejection; the release
  suites (`stress/suites/runs/u154`).
- Bench (`bench/run.sh`, GCC, this change alone): `tree` 125.2 M → 75.4 M instructions, 143 M → 114 M cycles,
  0.068 s → 0.051 s against Rust's 0.026 s; every other row unchanged in instructions.
