# D-0177 — `read_le`/`read_be` in `std::collections`, `crc32` in `std::crypto`

Status: ACCEPTED (2026-10-05, the owner: "option 1, include it in Update 69")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0119 (integers as bytes, `crc32`), D-0136 (`std`'s submodules), D-0151 (`std::crypto`), D-0176 (each item with the items it works with)
Affects: `spec/21` §0, §2g, §2m (4.41.0), `spec/conformance.md` (3.169.0), the guide §21 and its subjects table, `impl/std/` (`text.cb`, `collections.cb`, `crypto.cb`); `CHG-0205`

## Problem

`read_le`, `read_be` and `crc32` were in `std::text`, which holds text:
`String`, `StringView`, parsing and formatting, and the encodings that
turn bytes into text and back (`to_hex`, `base64_encode`). None of the
three touches text. `read_le`/`read_be` decode integers from a byte
slice, and their inverses, `Vec::push_le`/`push_be`, were already in
`std::collections`; `crc32` checksums bytes. The placement broke the rule
D-0176 wrote into `spec/21` §0: each item lives in the submodule whose
existing items it works with.

## Candidate mechanisms

1. **`read_le`/`read_be` to `std::collections`, `crc32` to `std::crypto`.**
   Selected. The readers sit beside their writers, and slices are already
   there (`slice_swap`); `crc32` joins the other checksums of bytes
   (`sha256`, `blake2b`, `Hasher`), marked plainly as not cryptographic.
2. **A new `std::bytes` holding all three.** Not adopted: a submodule for
   three functions, and `push_le`/`push_be` cannot follow them
   (`[Assoc-Fn-Foreign-Type]`), so writers and readers would still be
   apart.
3. **Only the readers move; `crc32` stays in `std::text`.** Not adopted:
   leaves one misplaced item.
4. **No change.** Not adopted.

## Selected design

`read_le` and `read_be` are declared in `std::collections`, `crc32` in
`std::crypto`. `spec/21` §2g says where each lives; §2m says that `crc32`
detects accidental corruption only, since anyone can make data with a
chosen CRC, and points to a hash or a MAC for tampering. No item's
signature or meaning changes.

## Compatibility impact

Additive for every program that imports `std` (all of the repository's
do). A program that imports `std::text` alone and calls one of the three
names `std::collections` or `std::crypto` instead.

## Revisit conditions

- A larger set of byte helpers (bit readers, varints) might justify a
  `std::bytes` after all; the three would then move with them.
