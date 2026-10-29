# D-0076 — Ordering text, and shortening a `String`

Status: ACCEPTED (2026-09-28, owner-delegated: "fix the frictions using your leanings")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8 (item 7), §9
Depends on: D-0053, D-0062, D-0073
Affects: `spec/21` §0

## Problem

Four programs (a word count, a directory tree, a map-reduce, a command
line tool) wrote the same byte-by-byte comparison to order `String`s for
`Vec::sort_by`. `<` on text is rejected, and its message named nothing.
A Huffman coder needed to drop the last character of a `String` and
could not: there is no way to shorten one short of `clear`.

## Candidate mechanisms

1. **`String::less(&a, &b)` and `StringView::less(a, b)`** (byte order),
   plus `<` on text given a message naming them. Selected. It's a
   library addition, and `String::less` has exactly the signature
   `Vec::sort_by` takes.
2. **`<`, `<=`, `>`, `>=` defined on text,** as `==` is on a view
   (D-0053). It's more convenient, but it makes the operators do a
   library call on one more kind of operand. Kept as a revisit
   condition.
3. **`String::truncate(&mut s, n)`,** keeping the first `n` bytes, with
   `n` checked to be a character boundary. Selected. `String::pop`
   would need a character type the language does not have.

## Selected design

1 and 3.

## Compatibility impact

Extension.

## Revisit conditions

- Ordering operators on text, if `less` proves as common as `==`.
