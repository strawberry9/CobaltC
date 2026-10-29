# D-0199 — String performance: native text searches, fused substrings, `unwrap`, and a cheaper runtime

Status: ACCEPTED (2026-10-09; the owner: "proceed with your recommendation. If you think B is justified, go ahead also", then "I need string performance significantly improved!")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0192, D-0193, D-0196
Affects: `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`; `spec/conformance.md`; `CHG-0227`

## Problem

After D-0196, compiled programs that work on text still ran 15 to 70 times slower than Python: csvstat 3.5 s, extsort
11.4 s and wordpar 5.1 s, against 0.2 s each (`stress/perf_oct9`). Profiling showed:

- the cost does not depend on how many values are alive;
- `String::find` and `String::view` took about 3 µs each on ten bytes, about 40 runtime calls apiece (a frame and a
  checked element access per byte position for `find`; four objects and two sends for `view`);
- the runtime's own calls allocated (a borrow of a range made six heap vectors);
- the element-access fast path was global, so one object over raw cells anywhere sent every element access in the
  program to the slow path.

The two options considered before were cheap reads of fields of `Vec` elements (A) and lazily made runtime objects (B).
A was not implemented: the shape occurs 4 times in all fifteen round-8 programs and never in the slow ones. B was
not where the time is: halving the objects `String::view` makes moved the programs by noise.

## Decisions (no rule changes; every check is made as before, in the same order)

1. **Native text searches.** `find`, `contains`, `starts_with` and `ends_with` of `String` and of `StringView` are
   native. A view's bytes are read through its binding, which cannot fail. A `String`'s bytes are read after the
   shared check of its bytes, which is the check that can fail.
2. **Native `String::view`.** The same checks in the same order: the bytes' shared check, then the bounds, then
   the character boundaries. The range is then borrowed from the `String`'s path and sent as the view's one
   reference.
3. **Literal views for pure parameters.** `StringView::of(L)` passed to a pure view parameter is a bare view of the
   literal's bytes, with no path and no object.
4. **Fused substrings.** `String::from_view(String::view(s, lo, hi))` and `String::append(d, String::view(s, lo, hi))`
   are each one call, with the checks of the calls they replace:
   - the bytes' check, the bounds and the boundaries;
   - for `append`: nothing for an empty view; otherwise the aliasing fault exactly when `d` is `s`, then `d`'s
     exclusive check.
   The view's path, which would end unheld with the statement, is not formed.
5. **Native `unwrap` and `expect`.** `Option::unwrap`/`expect` and `Result::unwrap`/`expect` are native for a value
   that is plain data or a reference (and, for `Result`, a plain error).
6. **Fewer objects per value holding references.**
   - A `let` of such a value takes over the temporary's object.
   - A temporary already holding its object is not copied into another.
   - A struct or enum part holding references moves into the aggregate (`cb_absorb_refs`) rather than being copied.
7. **The runtime allocates less.** A range borrow, a move, a copy and a send or receive of a value holding references
   use the runtime's reusable buffers.
8. **Page-local element checks.** The runtime counts objects over raw cells by page (`cb_reclaimed_pages`), and
   `cb_elem_access` takes the element's size. An element access goes to the slow path only when an object lies on
   the element's own pages.

## Results (cobc, gcc, best of three)

| | before | after | Python |
|---|---|---|---|
| csvstat | 3502 ms | 2608 ms | 223 ms |
| extsort | 11422 ms | 5790 ms | 169 ms |
| wordpar | 5102 ms | 4376 ms | 170 ms |

Single operations: `String::find` 3105 → 277 ns, `String::view` 3446 → 1026 ns, extsort's `key_of` 8413 → 1587 ns.

## Not decided here

The rest of the gap is spread across per-statement and per-block bookkeeping, and across the objects of values that
live in containers. The general step is the dual body of D-0188 extended from slices to `ref<String>` and
`StringView` parameters: a whole function body run with no runtime events when its arguments are disjoint.
