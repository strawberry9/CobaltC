# CHG-0430 — A decided program's small functions are inlined under GCC as under Clang

Status: implemented (2026-11-10), D-0309, D-0296; agenda item 3 (decided programs equal or beat checked Rust),
Update 178
Affects: `impl/cobc/src/lower/stack_checks.rs` (`inline_small`), `lower/program.rs`; their twins in
`impl/selfhost/back/stack_checks.cb` and `gen.cb` (`parts`); `impl/cbrt/include/cbrt.h` (`CB_SMALL`). No rule
changes. No program's outcome changes.

## What was wrong

GCC at `-O2` leaves a call to a small static function where Clang inlines it. A decided program's helpers are
small and called in its hot loops: `stress/vsrust/mini_interpreter.cb`'s `pop` (`Option::ok_or(Vec::pop(s),
Underflow(pc))`), `arith`, and `std`'s `Option::ok_or` at the program's types, each a call returning an enum by
value per instruction interpreted. Under GCC the program ran 3.36 times checked Rust's time (24.5 G instructions,
Rust 8.2 G); under Clang 0.82. The agenda's note of 2026-11-07 measured the remedy by hand: marking those three
`always_inline` halved the instructions.

## What changes

After `prune_stack_checks`, in a decided program, every function `localize` made `static` (the program's own and
`std`'s instantiated at its types) whose definition carries no attribute, whose body is at most 32 lines past its
`#line` directives, and which has no loop (`for (`, `while (`, `do {`), no self tail call (`goto self_tail;`), no
`setjmp`, `alloca` or `va_start` and no `cb_stack_check();` left, is defined, and declared, `CB_SMALL`: `cbrt.h`
makes that `static inline __attribute__((always_inline))` under GCC and `static`, as before, under Clang, which
inlines such functions itself (forced, it ran more instructions: `bench/tree.cb` +6.7%). A function D-0309 leaves a check in is on a cycle of calls, has its
address taken or is an entry, so none of those is forced inline; nothing recursive is. Inlining changes no program's
behaviour: the C is the same function's, and the stack the chain uses is no more than the frames `prune_stack_checks`
summed.

## Measured

`stress/vsrust/run.sh` (time against checked Rust, gcc / clang), Update 177's `cobc` against this one with
CHG-0431 and CHG-0432: mini_interpreter 3.36 / 0.82 → 0.90 / 0.83 (gcc 24.5 G → 11.9 G instructions).
