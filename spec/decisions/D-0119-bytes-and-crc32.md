# D-0119 — Integers as bytes in a fixed order, and CRC-32

Status: ACCEPTED (2026-09-30: F9 of the findings report; the byte functions decided with D-0118's delegated decisions — "F9's byte functions go in as D-0119" — and `crc32` by the owner: "also add to std, F9's crc32, using the CRC-32/IEEE and documenting the algorithm completely in the spec")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §5, §8 (item 10), §9 ("Library helpers")
Depends on: D-0090 (`integer` bound), D-0047 (slices), rule.arith.convert (`narrow_wrapping`), rule.arith.bitwise, D-0118 (a `bitstruct` crosses into bytes through these)
Affects: `spec/21` §0 and §2g (3.47.0), `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`

## Problem

Every file format, protocol and checksum turns integers into bytes in
a fixed order and back. The corpus wrote it by hand in a dozen
programs — `put_u32`/`get_u32` pairs of shifts and `narrow<u8>`,
`widen` and `<<` — each for one width, each a place to get a shift
wrong; and five programs implemented CRC-32.

## Candidate mechanisms

1. **Leave it to programs.** Each keeps its own, per width.
2. **Generic `std` functions over the `integer` bound**, and one
   `crc32`. Selected: "this integer as bytes, in this order" is one
   intent whose name says the order, which the hand-written loop does
   not until its shift is read (§9's test).

## Selected design

    Vec::push_le<T: integer>(ref<Vec<u8>, exclusive> out, T x)
    Vec::push_be<T: integer>(ref<Vec<u8>, exclusive> out, T x)
    read_le<T: integer>(slice<u8, shared> s, usize at) : T
    read_be<T: integer>(slice<u8, shared> s, usize at) : T
    crc32(slice<u8, shared> data) : u32

- The bytes are the value's two's-complement image, `sizeof<T>()` of
  them, least (`le`) or most (`be`) significant first (`[Bytes-LE]`,
  `[Bytes-BE]`); signed types round-trip; `u8` is one byte. Fewer than
  `sizeof<T>()` bytes from `at` is `diag.index-out-of-bounds`.
- `crc32` is CRC-32/IEEE, written out bit by bit in `spec/21`
  `[CRC32]` with its parameters and check values, so that both tools
  and any other implementation compute exactly one function; `std`'s
  body is that definition.
- All written in CobaltC: `sizeof<T>()`, shifts, `narrow_wrapping`
  (which the `integer` bound admits in both directions), `|`, `^`.

## Compatibility impact

Additive; a program's own `crc32` or `Vec::push_le` shadows it (D-0024).

## Revisit conditions

Other checksums (CRC-16, Adler-32, CRC-32C) as their own decisions if
programs need them; a table-driven `crc32` in either tool is an
implementation choice the rule allows.
