# CHG-0388 — The token-kind tests inline: a decided function's references cost no call in any program

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-29)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0293, D-0292, D-0281
Affects: `spec/IMPLEMENTATION-NOTES.md` (the runtime contract); `impl/cbrt/include/cbrt.h`, `impl/cbrt/src/lib.rs`,
`impl/cobc/src/lower/passes.rs`

## What changed

In the runtime header (`impl/cbrt/include/cbrt.h`, outside `CB_PATHLESS`):

1. `CB_LIGHT` (bit 53) and `CB_ROOT` (bit 54), the token kinds of D-0292; `extern uint8_t *cb_marks[256]` (per
   heap, one byte per object slot, nonzero while the object has paths on it); `extern uint8_t cb_inline_off`;
   `enum { cb_lf = 0 }` (a decided body shadows it with 1); `cb_vouched(t)`: `!cb_inline_off`, `CB_LIGHT` set, and
   `CB_ROOT` clear or the object's mark zero.
2. `static inline` forms behind variadic macros of the entries' names: `cb_borrow_check`, `cb_borrow_unminted`,
   `cb_borrow_range_unminted` and `cb_read` call the runtime only for a token that is not vouched; `cb_borrow`
   and `cb_borrow_unstamped` return `t & ~CB_ROOT` for a vouched token when `cb_lf` is 1, else call; `cb_write`
   through a light reference when `cb_lf` is 1 is `diag.overwrite-of-live-resource` for a resource target and
   nothing otherwise, else calls.

In the runtime (`impl/cbrt/src/lib.rs`):

3. `cb_marks`, `cb_inline_off` (`#[no_mangle]` statics), `MARK_LEN`; `ensure_mark(id)` after every
   `objs.reserve` (grows the heap's array to cover the id, copying the old marks and leaking the old array: a
   reader on another thread sharing the heap may still test a byte of it); `set_mark(id, on)`: a relaxed byte
   store. `mint_in` marks the object a path is minted on; `retire_token` unmarks it when its list empties;
   `destroy`'s removal and `migrate`'s re-insertion clear and carry the mark. `cb_init` sets `cb_inline_off`
   when `COBALTC_RECORD` is set.

In `cobc` (`impl/cobc/src/lower/passes.rs`):

4. `resolve_scopes_in` writes `enum { cb_lf = 1 };` where it meets a `G` marker (a decided body's frame, D-0292),
   whether or not the frame is emitted. It is written by the resolver, not emitted as a statement by the
   lowering, because the resolver counts any line with a `cb_` identifier as a use of the open frame: the first
   form made every light frame real (`cb_frame_push_fn` and `cb_frame_pop` around every decided call) and cost
   the micro program `d` 9% before the cause was found.

`spec/IMPLEMENTATION-NOTES.md`'s runtime contract is regenerated (the new inline entries and variables).

## Verification

- `cargo test -p cbrt`; `cargo +1.77.2 check --workspace --tests`; the Windows runtime rebuilt.
- The 167 dynamic conformance cases through `stress/d0281/phase3/gate3.sh` (`coby`, light under
  `COBALTC_RECORD=check`, `COBALTC_RECORD=full`) under GCC and under Clang: the 78 that keep paths agree three
  ways, the 89 that are decided are the pathless gate's and the compiled suites'.
- The three-way differential over the 996 programs that keep paths (`kept146.list`), GCC.
- The release suites (`stress/suites/runs/u148`).
- Measurements in `CHG-0388`'s changelog entry and `stress/d0281/phase3/BENCH.md` (`runs/step4`).
