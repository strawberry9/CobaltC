# CHG-0441 — A struct field of a 32- or 64-byte vector, and telling the C compiler a slice's length

Status: implemented (2026-11-13), D-0296; Update 184
Affects: `impl/cobc/src/lower/layout.rs`, its twin `impl/selfhost/back/layout.cb`; `spec/conformance.md`
(3.309.0); the guide's Performance chapter (`htmlguide/generator/content_perf.py`);
`stress/perfguide/slice_lengths/`

## Why

Found while a game-of-life program was written and tuned against C (`private/game-of-life`,
`cobc-bugs-found.txt`, BUG 1 and P3, and its README's "In short:"). A struct with a `simd<u64, 4>`,
`simd<u8, 32>`, `simd<u64, 8>` or `simd<u8, 64>` field passed `cobc --check` and ran under `coby`, but
`cobc`'s C failed its own layout assertion under GCC:

    _Static_assert(sizeof(struct s_P) == 128 && _Alignof(struct s_P) == 64, "layout of s_P");

`[Layout-Simd]` (`spec/16` §0) gives a vector the alignment of its size, and the assertion checks the C
against it. GCC gives a `vector_size(32)` or `vector_size(64)` type an alignment of 16 on a target
without AVX or AVX-512 (Clang gives the vector's size everywhere), so the C type was not the specified
one. Reproduced here with GCC 12.3; the report's was GCC 14.2. `-march=native` on an AVX machine did not
help, since 64 still needs AVX-512.

## What changes

**The vector typedefs say their alignment.** Both toolchains emit
`typedef T cbv_… __attribute__((vector_size(N), aligned(N)));` and the same for the unsigned twin
(`cbvu_…`), so the C layout is `[Layout-Simd]`'s under either compiler and any `-march`. The runtime
already allocates with the type's alignment (`cb_allocate(size, align)`), so a `Vec` or a `Box` of such
structs is aligned too. No rule changes.

**Conformance:** `conf.simd-wide-fields` (`impl/conformance/16-aggregates/simd_wide_fields_ok.cb`): two
`simd<u64, 8>` fields, a `simd<u8, 32>` after a `u8`, the struct passed by value and kept in a `Vec`,
its size and alignment printed (128 and 64; 64 and 32). It fails before this change under GCC, and passes
now under coby and under cobc with GCC and Clang, with and without `-march=native`.

**The guide: "Tell the C compiler a slice's length"** (Performance). A slice parameter's length is a
number the C compiler knows nothing about, so a loop whose bounds are constants keeps every index check.
One `assert(slice_len(s) >= n)` before the loop (an ordinary check, made once a call) lets it prove and
remove them. Measured on a 256 × 256 five-point blur over two slices (`stress/perfguide/slice_lengths`,
instructions a cell, GCC / Clang): no assert 31 / 38; `==` 16 / 28; `>=` 16 / 16;
`ref<array<u32, CELLS>, …>` parameters 17 / 16; C 12 / 4.9; `>=` with `wrapping_add` additions
14 / 5.1, Clang's the same time as the C. Clang proves `i < len` from `len >= C` and not from `len == C`,
so the guide says `>=`. A rule line and an example (`perf-lengths`) go with it.

## Not changed

The report's other observations, assessed:

- P1, `read_file` reads a file over 4096 bytes twice and copies it a byte at a time with `Vec::push`
  (`impl/std/fs.cb`): confirmed by reading the code; left for its own change (a `std` edit, `spec/21`'s
  listing).
- P2, start-up 0.2 ms above C and a larger executable: the runtime's size; not investigated.
- P4, a row-order loop over three slices 1.2–1.3 times C's instructions with two checks left on one line:
  not investigated here; the guide's last bullet names the index forms the C compiler proves best.
- `cobc` inserting the guard itself: prototyped by hand in `cobc`'s C (NOTES.md there): the loop's
  body twice, under `if (src.len >= 65536 && dst.len >= 65536)`, every check kept in both copies, ran as
  the hand-written assert does (GCC 17, Clang 16 instructions a cell). Proposed to the owner, not built.
