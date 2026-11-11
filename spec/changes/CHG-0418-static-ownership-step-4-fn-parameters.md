# CHG-0418 — The static ownership plan, step 4's first part: the unwinders fold frames and tables together; a `fn` parameter the body only calls has no runtime object

Status: implemented (2026-11-08), D-0315
Affects: `spec/IMPLEMENTATION-NOTES.md` (the runtime contract's table); `spec/conformance.md` 3.300.0 (two rows);
`cbrt` (`lib.rs`: `TState::notes`, `cb_notes_push`, `fold_frames_and_notes` in `fault_impl`, `cb_exit`,
`contained_fault` and `lib_contained_fault`; `cbrt.h`: `cb_reallocate_plain_fast`'s labels); `cobc` (`lower/`:
`frames` — `bind_param`, `push_drop_under_root`, `end_diverged`, `notes_finish`; `functions`, `closures`, `syntax` —
`callee_only`, `calls_last_named`; `stack_checks` — `drop_unused_notes`; `passes`) and its twins in `cbc`
(`back/`: `gen`, `state`, `vec_scan`, `stack_checks`, `passes`, `header`); `impl/tools/validate_c.py` (the two new
entry points classified: `cb_set_decided` with the program's start, `cb_reallocate_plain_fast` with the allocators). No
rule changes.

## What changes

1. **The tables carry their depth.** A function's table of noted locals (D-0269's `cb_nt`) is registered with the
   frame stack's depth at the push. Every unwinder — `[Fault-Unwind]` in `fault_impl`, `cb_exit`'s, `[Fault-Contain]`'s
   and a library `_try`'s — folds the frames and the tables together, innermost first: the entries above a table's
   depth (its function's frames and its callees'), then the table's live entries, last bound first
   (`fold_frames_and_notes`). Before, only a contained fault read the tables, after all the frames; a library's `_try`
   left them unread, and with them the objectless locals of the bodies it unwound (memory only: nothing observed it).
2. **A `fn` parameter the body only calls** (every mention of it a call's callee: `f(x)`; not in a function that calls
   itself, whose self tail call would rebind it) has no object in a decided program. It has an entry in the body's
   table, which the body then registers on every call (`cb_nt_on = 1`: a closure's captures may have a destructor a
   fault must run, and could observe), and its end at every return: `if (f && closure) cb_drop_value(&f, t);` (a `fn`
   item owns nothing). Bound before anything in the body, it ends after the body's frame (`[Block-Exit]`): its drop is
   placed below the root frame on the lowering's cleanup stack, and the table's pop below it
   (`push_drop_under_root`); `drop_unused_notes` keeps such a table where no thread is ever spawned.
3. **`cbc`'s reserved-word test** compares only the words of a name's length (it scanned the whole list for each name,
   8,500 instructions a call); the C it writes is unchanged.

Found by the gate: `Vec::push(&mut v, x)` of an objectless local took the general push, about six instructions a
push more than the checked-once push a local with an object takes (perfguide's `e02_reserve`, +33%); it takes the
checked-once push with token 0 now (`lower_push_checked_once`, `vec3.cb`), and `e02_reserve` runs 11% fewer
instructions than before D-0315. Found on the way: the first form put the parameter's drop above the root frame, so a closure ended before the callee's
own local (`[end first][end twice 4]` for `[end twice 4][end first]`, `conf.fn-parameter-closure-ends`); and an
argument `reinterpret<i64>(x)` of `spawn` keeps a program's paths ("a thread is lent a reference no fact describes"),
which the case avoids and a later change should look at.

## Measured

Instructions of `cbc --emit-c` over `cobc`'s (both of this change): `complex_lisp` 1.18, `complex_typeinf` 1.43,
`large_60` 1.46, `large_200` 1.56. Wall time (`timeit.py`, five runs): `complex_typeinf` 1.69, `large_60` 1.63,
`large_200` 1.85; the others within 1.45. On the largest programs `cbc` runs at fewer instructions a cycle (0.93
against 1.08 on `large_200`, with 1.75 times the first-level cache misses): what remains is spread over its own text
and allocation work, the runtime's bookkeeping about 6% of its cycles.

## Conformance

`conf.fn-parameter-closure-ends`, `conf.fn-parameter-closure-fault-unwinds`: alike under `coby`, `cobc` (gcc, clang)
and the sanitizers; `cbc` writes `cobc`'s C for both.
