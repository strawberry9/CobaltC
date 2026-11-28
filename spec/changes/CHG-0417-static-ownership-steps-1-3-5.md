# CHG-0417 — The static ownership plan, steps 1–3 and 5: a decided program's memory-only values pass, end and are destructured with no runtime object

Status: implemented (2026-11-08), D-0315
Affects: `spec/conformance.md` 3.300.0 (nine rows); `cobc` (`lower/`: `analysis_strings`, `calls`, `closures`,
`equality`, `exprs`, `flights`, `frames`, `fresh`, `functions`, `intrinsics`, `matching`, `native_map`, `native_vec`,
`passes`, `places`, `program`, `stmts`, `syntax`, `user_calls`, `values`; `report.rs`, `main.rs`) and its twins in `cbc`
(`back/`: `analyses`, `flights`, `fresh3`, `gen`, `header`, `native_map`, `natives`, `native_vec`, `passes`, `state`,
`strings4`; `cbc/driver.cb`, `cbc/report.cb`), in step (D-0296); `cbrt` (`lib.rs`: `cb_set_decided`, a destructor on
a value with no object; `cbrt.h`: the decided constants, `cb_reallocate_plain`'s fast path inline). No rule changes.

## What changes

Every rule below holds in a decided program only (`Gen::decided`); a program that keeps paths is lowered as before.

1. **Statement temporaries** (step 2). A borrowed call result of a type whose end frees memory alone
   (`objectless_type`) is a C local of its statement with token 0: dropped by C where the statement ends or a jump
   leaves it (`Fx::stmt_temp_drop`, the drops above the statement's scope, last made first), under a flag per
   temporary (`cb_tl[j]`, the function's, zeroed at entry) since a short-circuit operand may pass it by, and noted in
   the body's table (D-0269) for a contained fault.
2. **Consumers arm their producers** (steps 1, 2). A binding, an argument, a payload, a field, an assignment into a
   quiet place, `Box::new`, a `match`'s scrutinee, a destructure and a discarded statement (`f();`, `_ = e;`, `f()?;`)
   arm the expression they take (`arm_inert`; `raw_arm` for a `match`, `if`, block, `?` and struct literal, whose sink
   is then raw). A call and a `match` take their arming as their lowering begins (their arguments' and scrutinee's
   own lowering arms other producers). `?`'s binders are objectless locals. A consumer that needs an object gives
   one where the value lands (`materialize`).
3. **The candidate walk** (step 3). It follows a place by its whole path (`place_chain`; `field_entry_ok`); takes a
   `match` arm binding an enum's whole value (`*whole`), `drop(x)` (a move), `$each_drain(e, n)` (a move) and
   `$each_len`/`$each_take`/`$each_take_key` of a holder (a borrow); walks a destructure as a `let` (its binders by
   `fields`, the binder names, not `names`, the struct's; keyed by position, `destructure_key`). `void` parts, slices
   and views are plain; a `resource struct` with no destructor of its own, a `std` struct, `std::Drain` and
   `std::HashDrain` (destructors that free memory alone) are judged by their parts. A pinned plain local and a
   reference binding have root token 0 and no object; `drop` of either ends nothing.
4. **Frames and the runtime** (steps 4's frame half, 5). A function whose frame nothing binds into pushes none
   (`elide_empty_frames`). `cbrt.h` under `CB_DECIDED`: a reference's flight, receipt and re-stamp are constants;
   `cb_init` first tells the runtime the program is decided (`cb_set_decided`), and a destructor run on a value with
   no object then gets token 0 with no object made for it. `Vec::drop` of plain elements takes the plain loop.
5. **Drop glue** (step 5). `cb_dg_<type>(void *p, uint32_t d)`, one per memory-only `String`, `Vec`, `Box`, `HashMap`,
   `HashSet`, struct or enum with no destructor of its own, in the thunks: a struct's fields last first, an enum's
   payload, a `Vec`'s elements then its buffer, a `Box`'s referent then its block; `cb_drop_local` past depth 1000 and
   for any part without glue. An objectless value's drop and a quiet write's end of the old value (base token 0) call
   it when `cb_reclaimed_any()` is false, `cb_drop_local`/`cb_write_quiet` otherwise.
6. **`HashMap::insert`'s native text** gives a `String` key no object in a decided program (none arrives there), and
   frees a key already present in place.
7. **`cb_reallocate_plain`** is `realloc` inline (`cbrt.h`) where its arguments grow a block, no object lies over raw
   cells and no thread was ever started: the runtime function's own path there.
8. **`--report-tracking`** on a decided program lists, per source line, the frames, statement scopes and objects its C
   still writes (`report::bookkeeping`, `cbc`'s `bookkeeping_names`). `COBC_DEBUG_SCOPES` names the first line that
   keeps a frame or scope.

Found by the work and fixed before it landed: `a && f(&g())` dropped a temporary the short circuit had not made (the
`cb_tl` flags); a live bit in the notes table broke the programs that never spawn (`drop_unused_notes` removes the
table); `cbc` keyed a `u32` map by reference; `stmt_temp_drop` looked only at the cleanup stack's top; `drop(k)` of a
pinned plain local destroyed token 0 (`cbc` built by the step faulted in `HashMap::swap_remove`;
`conf.drop-of-plain-local`); the walk scoped a destructure's field names, not its binders.

## Measured

Instructions (`perf stat -e instructions:u`, `--emit-c`; `cbc` built by the same step's `cobc`):

| program | `cobc` | `cbc` before | `cbc` after |
|---|---|---|---|
| `complex_expr` | 1.08 G | 3.70 G | 0.76 G |
| `complex_lisp` | 9.13 G | 54.7 G | 11.3 G |
| `large_60` | 3.41 G | 25.0 G | 5.36 G |

Wall time, `cbc`/`cobc` (the speed test's `timeit.py`, five runs, both compilers of this change): `small_hello` 0.31,
`small_fib` 0.30, `medium_stack` 0.32, `complex_expr` 0.57, `large_20` 1.14, `complex_dp` 1.43, `complex_lisp` 1.44,
`complex_typeinf` 1.67, `large_60` 1.69, `large_200` 1.86 (before: 1.16 to 5.92). The runtime is 13% of `cbc`'s cycles
on `complex_lisp` (71% before). Written in `cbc`'s own C: `cb_new` 14,893 → 1,071, statement scopes 29,785 → 2,041,
block frames 15,292 → 792, function frames 2,277 → 373. `--report-tracking` lists nothing for any program of the
speed test.

## Conformance

`conf.drop-of-plain-local`, `conf.destructure-memory-only`, `conf.stmt-temp-in-short-circuit`,
`conf.stmt-temps-end-descending`, `conf.match-whole-arm-memory-only`, `conf.foreach-by-value-memory-only`,
`conf.propagate-memory-only-payloads`, `conf.contained-fault-frees-statement-temporaries`, `conf.map-insert-text-keys`.
Each runs alike under `coby`, `cobc` (gcc and clang) and the address, undefined-behaviour and leak sanitizers, with
the eight programs of the speed test; `cbc` built by each step writes that step's `cobc`'s C byte for byte.
