# CHG-0395 — `Option<Box<T>>` is one pointer: `[Layout-Enum-Niche]` (D-0298 step 2)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-31)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0298, D-0250, `CHG-0394`
Affects: `spec/16` §1 (`[Layout-Enum-Niche]`, 1.32.0), `spec/06` §7 (`[Repr-Enum]`, 1.21.0),
`spec/IMPLEMENTATION-NOTES.md` §4, `spec/conformance.md` (3.286.0); `impl/cobc/src/lower/layout.rs` (`niche_box`,
`tag_of`, `tag_is`, `tag_set`, the enum's C type, size and descriptor), `impl/cobc/src/lower/{values,matching,exprs,
intrinsics,native_misc,native_vec,native_map,native_io}.rs`; `impl/cbrt/src/lib.rs` (`TypeInfo::niche`, `enum_tag`);
`impl/src/interp/{layout,extern_calls,memory}.rs` (`EnumLayout::niche`); `impl/conformance/16-aggregates/
option_box_one_pointer_ok.cb`, `impl/conformance/06-arithmetic/option_box_null_bytes_ok.cb`; `htmlguide/generator/
content_b.py`; `impl/selfhost/back/{layout,gen,native_misc,native_vec,native_map,native_io}.cb`
in the same Update (D-0278 as resumed by D-0296)

## What changed

1. **The rule.** `[Layout-Enum-Niche]` (`spec/16` §1): an enum of exactly two variants, one with no payload and
   one whose payload is a `Box<T>`, has no discriminant; it is the `Box` alone, at offset 0, and the no-payload
   variant is the null pointer, which `Box::new` never gives. `[Repr-Enum]` (`spec/06` §7) names its image: the
   `Box`'s own, or the null address. `[Layout-Enum]` does not apply to such an enum; every other enum --
   `Option<Option<Box<T>>>` and `Option<ref<T, m>>` included -- is laid out as before.
2. **`cobc`.** `Gen::niche_box` recognises the shape; the enum's C type is `struct E { union { struct Box vK; } u; }`
   (the payload's path `.u.vK` unchanged), its size the `Box`'s, and its descriptor marked for the runtime (`elem` =
   the Box variant's index + 1, `count` = the other's + 1). Every read and write of a discriminant the generated C
   makes goes through `Gen::tag_of`, `tag_is` and `tag_set`, which read the pointer for a niche enum and write
   null for its no-payload variant (making it the Box variant is the payload's store alone): enum construction
   (`values.rs`), `match` tests at every level of a nested pattern (`matching.rs`), the quiet-write guard of
   `CHG-0393` (`exprs.rs`), printing a variant's name (`intrinsics.rs`), and the native bodies that make or test an
   `Option<T>` or `Result<T, E>` of a type a caller chooses (`Option::unwrap`/`expect`, `Vec::pop`, `Queue::pop`,
   `HashMap::insert`, `PriorityQueue::pop`, `Channel::send`/`recv`). Natives whose enum's payload can never be a
   `Box` (`Option<usize>`, `Option<ref<…>>`, the I/O results) write `.tag` as before.
3. **`cbrt`.** `TypeInfo::niche` from the descriptor's marks; `enum_tag` reads a value's variant index as its
   layout keeps it, at the seven places the runtime read a discriminant (destruction, `owns`, the reference and
   `fn` walks, `Proj::Payload`'s address, the tree teardown of `CHG-0393`).
4. **`coby`.** `EnumLayout::niche` (`layout.rs`): `sizeof`/`alignof` are the `Box`'s, `dw` 0 and `payload_off` 0;
   `encode`/`decode` and a raw path into the payload (`memory.rs`) read the variant from the pointer.
5. Two conformance rows: `conf.option-box-one-pointer` (the sizes; a tree, a user enum of the shape in either order,
   a `Vec` and a channel of such values, `pop`, `swap`, `replace`, printing) and `conf.option-box-null-bytes` (the
   image through a `rawptr`: `None` is null, `Some(b)` is `b`'s address). The guide's layout paragraph (§B) notes
   the exception.

## Verification

- The two new cases and `stress/tree_gap/td1.cb`–`td4.cb`: `coby`'s output and `cobc`'s under GCC and Clang, with
  and without `--all-checks`, the same bytes; valgrind on `td1`, `td3`, `td4` under GCC, no invalid free and no
  definite leak.
- `cobc --unittest --unittest-std`; `cargo test --release -p cobc` for the quick targets (`runtime_bookkeeping`,
  `tracking`, `differential`, `driver`, `extern_code`, `library`, `output_order`, `read_input`,
  `returned_view_inlined`, the binaries); `cargo test -p cbrt`; the `conf_coverage` test; `cargo check --workspace
  --tests` for Linux and `x86_64-pc-windows-msvc`; `cargo +1.77.2 check`; the Windows runtime rebuilt;
  `speccheck.py`; `cobfmt --check` on the new cases; the guide regenerated. The release suites and the undecided
  sweep are owed with `CHG-0393`'s (the owner: cheap checks only).
- Bench (`bench/tree`, `perf` over 15 runs, the same session): GCC 71.9 M instructions, 90.0 M cycles, 38.1 ms of
  CPU; Clang 76.7 M, 84.8 M, 36.0 ms; Rust 30.1 M, 57.5 M, 24.5 ms. The node is 24 bytes (`_Static_assert` in
  the C) and the heap 1.56 MB for 60,000 nodes against 3.47 MB this morning. From 2.40× Rust's CPU time at the
  start of the day to 1.55× (GCC) and 1.47× (Clang).
