# D-0085 — `Vec::from_slice` and `Vec::truncate`

Status: ACCEPTED (2026-09-28, stress round under the owner's standing instruction to resolve frictions with the implementer's leanings; confirmed by the owner 2026-09-28)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 10), §9
Depends on: D-0047, D-0082, rule.stdlib.vec
Affects: `spec/21` §0

## Problem

Two operations every program that handles byte buffers or lists
reaches for were missing:

- copying a slice into a `Vec` of its own (a UTF-8 decoder copying an
  invalid sequence out of a buffer to report it, a Huffman file's
  header): only a loop of `Vec::push` did it;
- shortening a `Vec` (a test truncating a compressed file to see the
  decoder reject it): `String::truncate` existed, `Vec::truncate` did
  not, so programs popped in a loop.

## Candidate mechanisms

1. **Leave them to loops.** Every program writes the same four lines.
2. **Add `Vec::from_slice(s)` and `Vec::truncate(v, n)`** as ordinary
   `std` functions written in CobaltC. Selected.

## Selected design

- `Vec::from_slice<T>(slice<T, shared> s) : Vec<T>`: a new `Vec` of the
  slice's elements, copied in order. As for `Vec::clone` (D-0082), `T`
  must be plain: a resource element type is `diag.type-mismatch` at the
  call, whose message names the type.
- `Vec::truncate<T>(ref<Vec<T>, exclusive> v, usize n)`: the first `n`
  elements stay; the rest are destroyed, last first. `n` at or beyond
  the length leaves the `Vec` as it is.

  A first draft faulted `diag.index-out-of-bounds` beyond the length,
  as `String::truncate` does. The first real use (a trie's "keep the
  top five completions") had fewer than five hits and faulted: the
  call means "at most `n`", and every such caller would otherwise guard
  it with a `min`. `String::truncate` keeps its fault, since its `n` is a
  byte position that must also be a character boundary.

## Compatibility impact

Additive. A program defining its own `Vec::truncate` or
`Vec::from_slice` keeps its own (D-0024).

## Revisit conditions

None.
