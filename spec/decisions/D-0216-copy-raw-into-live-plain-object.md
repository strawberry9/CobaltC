# D-0216 — `copy_raw` into the cells of a live plain object

Status: ACCEPTED (2026-10-11; the owner: "action the plan in full autonomously and using the recommendations as written" — `ChatGPT_review/ACTION-PLAN-2026-10-11b.md` WI-5, Q3)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12, §17 (external interfaces), §19
Depends on: D-0017, D-0206
Affects: `spec/20` 1.15.0, `spec/conformance.md` 3.210.0 (CHG-0254); `impl/conformance/README.md` (an author's note);
`impl/conformance/06-arithmetic/ref_byte_image_width_ok.cb` (its header)

## Problem

The second independent review (F-04): `conf.ref-byte-image-width` copies a reference's bytes with `copy_raw`
into a live `array<u8, 16>`, and its derivation justified the trusted step by `[Rawptr-Write]`'s condition,
but `[Copy-Raw]` required that no live object overlap the destination at all. A positive case whose trusted
step breaks the rule's own premise makes a claim the rules do not support.

## Options

1. Let `[Copy-Raw]` write into a live object under `[Rawptr-Write]`'s conditions — the FFI-buffer pattern a
   byte array filled from a foreign source, or from another value's bytes, already relies on.
2. Keep the rule and rewrite the case to copy into `allocate`d storage; the pattern then has no rule.

## Decision

Option 1. `[Copy-Raw]`'s side condition: every live object overlapping the destination is of a plain type
holding no reference, lies in the range or holds it, and is reached by no live derived path and by no other
thread meanwhile; no reclaimed object and no object of a resource type overlaps it. A reference-free plain
type is required because a copied reference's cells are not a reference value the destination's type could
hold (`[Repr-Ref]`'s cells are a token, not bytes the type system can name): writing them into a `ref` field
would forge a path. "Root paths and raw access" names `copy_raw` beside `rawptr_of`. The row's derivation now
cites this condition. `impl/conformance/README.md` gains a note: a positive `unsafe` case cites, for every
trusted step, the condition it satisfies, and satisfies it.

## Results

No tool changes: neither checks `[Copy-Raw]`'s trusted condition, and the case's output is unchanged. The
trusted rationales that cite `[Copy-Raw]` (`Vec::grow`, `reserve`, `Vec::append`'s copy, the priority queue)
copy into fresh ranges or slots that hold no element, which the new condition admits as before.
