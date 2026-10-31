# CHG-0394 — A `Box` is one pointer (D-0298 step 1)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-31)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0298, D-0045, D-0206
Affects: `impl/std/memory.cb`; `spec/21` §3a (the text, the quoted definition, the trusted-claims table);
`spec/conformance.md` (`conf.safe-client-box-trace`); `impl/cobc/src/lower/closures.rs`; `impl/cbrt/src/lib.rs`
(`box_drop`, `tree_visit`); `impl/src/interp/destroy.rs` (`destroy_box_value`); `impl/selfhost/back/fresh3.cb` and
`impl/selfhost/front/prelude.cb` (regenerated) in the same Update

## What changed

1. `Box<T>` is `{ rawptr<T> ptr; }`. `Box::into_inner` moves the value out, releases and deallocates the cells
   and sets `ptr` to `null_rawptr<T>()`; `Box::drop` returns at once for a null `ptr` and otherwise reclaims,
   destroys and deallocates as before. The trusted claims of `Box::get`, `Box::into_inner` and `Box::drop` say so
   (`impl/tools/trusted_table.py` regenerated the table); the trace of `conf.safe-client-box-trace` follows the
   new order of `[Release]`, `[Deallocate]` and the null.
2. `cobc`'s native `Box::new` writes the pointer alone; `cbrt`'s `box_drop` and the tree walk of `CHG-0393` read
   null as "taken" where they read `full` before; `coby`'s native Box destruction likewise.
3. `sizeof<Box<T>>()` is 8 on a 64-bit target (16 before); `sizeof<Option<Box<T>>>()` 16 (24 before).

## Verification

- `stress/tree_gap/td4.cb` (`into_inner` of an integer, of a `Box` of a `Box`, of a type with a destructor, of a
  zero-sized type, of a Box taken from an `Option` and from a `Vec`; the sizes): `coby`'s output and `cobc`'s
  under GCC and Clang, with and without `--all-checks`, the same bytes; valgrind under GCC, no invalid free and no
  definite leak. `td1`–`td3` of `CHG-0393` again, the same.
- `cobc --unittest --unittest-std` (the standard library's own tests, `Box`'s among them); the quick `cobc` and
  `cbrt` test targets of `CHG-0393`; `cobfmt --check` on `memory.cb`; `speccheck.py`; `trusted_table.py --check`.
  The release suites are owed with `CHG-0393`'s.
- Bench (`bench/tree`, GCC, `perf` over 15 runs): 73.9 M instructions, 95.6 M cycles, 42.2 ms of CPU against Rust's
  30.1 M, 61.4 M and 26.3 ms; the heap 2.5 MB against 3.5 MB for the same 60,000 nodes.
