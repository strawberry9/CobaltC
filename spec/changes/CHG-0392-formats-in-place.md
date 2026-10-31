# CHG-0392 — Formats written in place, text sorted by the runtime, and `--run`'s directory removed

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-30)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0297, D-0296, D-0062, D-0038
Affects: `impl/cobc/src/lower/intrinsics.rs`, `impl/cobc/src/lower/native_vec.rs`, `impl/cobc/src/lower/passes.rs`,
`impl/cobc/src/main.rs`, `impl/selfhost/back/gen.cb`, `impl/selfhost/back/native_vec.cb`,
`impl/selfhost/back/passes.cb`, `impl/selfhost/back/header.cb`, `impl/selfhost/cbc/driver.cb`,
`impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`, `spec/IMPLEMENTATION-NOTES.md`,
`impl/conformance/21-standard-library-semantics/format_in_place_ok.cb`, `spec/conformance.md`, `bench/`

## What changed

1. **`$fmt` of a literal format written in place** (D-0297 item 1). `cobc` (`lower/intrinsics.rs`: `FmtArg`,
   `FmtOp`, `fmt_plan`, `Fx::fmt_in_place`) and `cbc` (`back/gen.cb`: the same names) record, beside each
   argument's `cb_fmt_arg` initialiser, how the in-place form would take it; when the format is a string literal
   and `fmt_plan` accepts every specifier, the text is written by straight-line C instead of `cb_format`: the
   literal's own bytes (`memcpy` from its array at an offset, `%%` as the second `%`), `cb_fmt_i64`/`cb_fmt_u64`
   for `%v %d %i %u`, `cb_fmt_hex` for `%x %X` (the value's bits in its width, `(uint64_t)(uintN_t)`),
   `cb_fmt_pad` for a width (spaces before; `-` spaces after; `0` zeros after a `-`), and text's bytes; into
   `uint8_t fpN[bound]` when no text argument is present and the bound is at most 256, else into
   `cb_fmt_space(bound + the texts' lengths)`. A format with no specifier is its literal, and the empty format
   empty. `cbrt.h` gains the four inline renderers and `cb_fmt_space`; `resolve_scopes`' `FREE` and
   `elide_implied_checks`' `PURE` (both compilers) list them, as they record nothing and change no path.
2. **`cb_format`'s state** (D-0297 item 2): `FORMATTED`, `FMT_CACHE` and `FMT_ARGS` become one thread-local
   `FmtState`; the cache is searched most recent first. `cb_fmt_space` has its own thread-local buffer.
3. **`Vec::sort` of text** (D-0297 item 3): the native body for `String` and `str` elements (`native_vec_sort_keys`
   in both compilers) reads `v`, ends the elements' objects and releases their cells (`cb_vec_drop_plain`,
   `cb_release`), sorts in place with `cb_sort_text` (new in `cbrt`: `slice::sort_by` on the elements' bytes), and
   writes `v` — the order of the integer sort's body; the merge in C and its two buffers are gone.
4. **`--run`'s directory** (D-0297 item 4): `cobc` (`main.rs`, `drop_scratch`) and `cbc` (`driver.cb`, the same)
   remove it on every early return after it is made but the C compiler's failure.
5. New case `conf.format-in-place` (`spec/conformance.md` 3.285.0); `spec/IMPLEMENTATION-NOTES.md` §6 regenerated
   (`impl/tools/runtime_contract.py`).

## Verification

- `impl/selfhost/compare_c.sh` over every tracked `.cb`: 1,164 same, 607 skipped; the conformance suite with
  `--all-checks`: 755 same; the showcases and controls with `-g`, `--no-line-directives` and `LEVEL=10`: 92 same
  each; `fixpoint.sh`: stage 2 = stage 1 = `cobc`'s C (135,985,767 bytes).
- `conf.format-in-place` under `coby` and `cobc` (GCC and Clang): the same bytes; the conformance coverage test.
- `stress/d0281/objectless/undecided_sweep.sh` over `decided.list`: 8,825 decided, 28 never decided and one
  rejected statically, as before.
- `impl/tools/runtime_contract.py --check` (the table regenerated; `cb_point_check`'s contract now names `[Read]`
  and `[Write]`, which it had not since D-0283), `speccheck.py`, `cargo +1.77.2 check --workspace --tests`, the
  Windows runtime rebuilt.
- Bench (`bench/run.sh`, `bench/RESULTS.md`, GCC): `hashcount` 210 M → 120 M instructions (Rust 216 M), 0.031 s
  against Rust's 0.044 s; `strings` 282 M → 236 M (Rust 243 M); every other row unchanged in instructions. `cbc
  --emit-c anagrams.cb` 5.21 G → 5.11 G instructions.
- The release suites (`stress/suites/runs/u152`): every stage passed, 315 programs modelled.
