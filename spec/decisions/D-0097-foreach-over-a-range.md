# D-0097 — `foreach` over an integer range

Status: ACCEPTED (2026-09-29, owner-proposed: "would be nice if I could do `foreach(i, v in 1..10)`"; the recommended options chosen: "proceed with your recommendations"; then "make two plain-number bounds default to usize")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0035 (`for`), D-0037 (literal expressions), D-0042 (`foreach`), D-0047 (`..`)
Affects: rule.control.foreach-range (new), spec/22 `foreach-expr`, the guide

## Problem

Counting through integers took the whole counted loop:
`for (usize i = 0; i < n; i += 1) { … }`. Both D-0042 and D-0047 left
ranges in `foreach` as a revisit condition, and `..` has been a token
since D-0047.

## Constraints

- `..` keeps the one meaning it has in a slice.
- No new token; nothing new underneath (`foreach` is already a `for`).
- No new implicit conversion (D-0006).
- The same values and output in `coby` and `cobc`.

## Candidate mechanisms

Bounds: **half-open `lo .. hi`** (selected), inclusive `lo .. hi`, or
half-open plus `..=` for inclusive. Where a range may appear:
**only in a `foreach`** (selected), or everywhere, as a first-class
`std` `Range<T>` value. Second name: **the position, `usize`**
(selected, as for a `Vec`), or none.

## Selected design

- `foreach (x in lo .. hi)` visits `lo`, `lo + 1`, …, `hi - 1`;
  `foreach (k, x in lo .. hi)` adds the position `k : usize` from 0.
  Three names are rejected.
- The bounds have one integer type, `x`'s. A bound that is a literal
  expression takes the other's type, as across `<`; two literal
  expressions are `usize`, so `foreach (i in 0..3)` can index
  (`a[i]`). A negative bound then needs a type: `-3..0: i32`. Bounds of two types, or not integers, are
  `diag.type-mismatch`.
- `lo` and `hi` are evaluated once, `lo` first. When `hi ≤ lo` the body
  never runs.
- `x` is a copy: assigning to it does not change the loop.
- A range is not a value, is not borrowed, and has no `$`.
- **Not added:** inclusive ranges, steps, reverse ranges.

## Rejected alternatives

- **Inclusive `lo .. hi`:** reads like English `1..10`, but gives `..`
  two meanings (a slice's is half-open), makes `0 .. n` run `n + 1`
  times, and `x .. MAX` would need its step guarded against overflow.
- **`..=` for inclusive:** a new token for what `lo .. hi + 1` says, and
  `hi + 1` is checked.
- **A first-class `Range<T>`:** a type, construction, and a way to loop
  over a user-visible value (D-0042 rejected an iterator protocol). No
  program has asked to store or pass a range.
- **Two literal expressions default to `i32`** (as `auto x = 1;`):
  first implemented; `foreach (i in 0 .. 3)` then could not index
  `a[i]` without `0 .. 3: usize`. Changed before release, on the owner's
  word: "make two plain-number bounds default to usize".
- **A step or reverse range:** `for (i32 v = 10; v > 0; v -= 1)` says it.

## Semantic rationale

`[Foreach-Range]` is an equivalence with a `for` over hidden bindings
(spec/14). The loop variable stops at `hi`, a value of the type, so the
step never leaves the type. A literal bound is typed by the other bound
exactly as `[Literal-Type-From-Context]` types it across `<`. Reading a
literal expression after the other bound is unobservable: it has no
effect and every fault of one is static.

## Usability

    foreach (v in 1 .. 10)
    {
        printf("%d ", v);              // 1 2 … 9
    }
    foreach (i in 0 .. Vec::len(&names))
    {
        …
    }
    foreach (i in 0 .. 3)
    {
        printf("%s ", names[i]);       // i : usize
    }
    foreach (i, v in 100 .. 103)
    {
        printf("%d:%d ", i, v);        // 0:100 1:101 2:102
    }

## Explainability

"`foreach (x in lo .. hi)` counts from `lo` up to, not including, `hi`,
as `&a[lo .. hi]` takes elements `lo` up to `hi`."

## Implementation-feasibility

The parser writes the loop (`parse_foreach`'s `foreach_range`); one
`$each_range` intrinsic in `src/each.rs`, shared by the checker, `coby`
and `cobc`, checks the bounds are integers and gives a literal bound the
other's type. No change to either back end.

## Compatibility impact

Additive: `foreach (x in e .. e)` did not parse before.

## Prior-art status

Rust `for i in 0..n` (half-open, `..=` inclusive); D `foreach (i; 0 .. n)`
(half-open); Swift `0..<n` / `0...n`; Python `range(lo, hi)`.

## Invariant traceability

None new.

## Revisit conditions

- A program that needs a range as a value, or a step.
- An inclusive range if half-open proves error-prone in practice.
