# D-0062 — Sorting and searching

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0010, D-0041, D-0048, rule.stdlib.vec
Affects: rule.stdlib.sort (new), rule.stdlib.prelude

## Problem

`std` had no sort. `showcase/tier2/sort_vec.cb`, `tier3/shortest_paths.cb`
and `tier6/wordfreq.cb` each wrote one; the CSV statistics of the
2026-09-27 real-world testing kept insertion order for want of one; and
`list_dir` (D-0061) sorts its names in the implementation because a
program could not.

## Constraints

- No bound system (D-0010): a generic sort cannot ask `T` to compare
  itself.
- No new tokens; results the same in both tools; deterministic.
- Elements that are resources (`String`, `Vec`) sort without being
  copied, and with every check the language makes.

## Candidate mechanisms

1. **`Vec::sort_by(&mut v, less)`, `less` a `fn` value**
   (`fn(ref<T, shared>, ref<T, shared>) : bool`, true when its first
   argument goes first). **Selected.**
2. **`Vec::sort(&mut v)` for the key types** (integers, `bool`, `str`,
   `String`: `HashMap`'s, D-0041), checked at each call. **Selected.**
3. A three-way comparison (`fn(…) : i32`).
4. Stable (merge sort) — **selected** — or not (quicksort).
5. **`Vec::binary_search(&v, &key) : Result<usize, usize>`** and
   `binary_search_by(&v, &key, less)`: the first equal position, or
   where the key would go. **Selected.**
6. Sorting slices too.

## Selected design

Candidates 1, 2, 4 and 5.

- **The order.** `sort` uses `key_less`, an intrinsic private to `std`:
  integers by value, `false` before `true`, text by its bytes (a prefix
  first). `sort_by` uses the program's `less`.
- **Stable.** Equal elements keep their order, so output does not depend
  on how the sort is done.
- **The algorithm.** A bottom-up merge sort of the elements' *positions*
  (the elements read through shared references), then each element moved
  once into a new buffer in order (`take_raw`, as `Vec::clear` does). No
  element is copied or moved behind the runtime's back.
- **Search.** Ok(the first equal position) — a definite answer with
  duplicates — or Err(the insertion point), Rust's convention.
- `cobc` realizes `Vec::sort` of an integer or `bool` element type
  natively (spec/21 §0's latitude): the same checks on `v`, the values
  sorted in place (200,000 `u64`s: 7.0 s → 1.2 s).

## Rejected alternatives

- **3:** a `less` is the simpler contract, and what `sort` and `search`
  both need.
- **Unstable sorting:** results would depend on the algorithm.
- **6:** deferred: a slice's elements cannot be moved into a new buffer
  (it does not own them); an in-place stable sort is a separate piece of
  work (`slice_swap`, D-0048, is the tool it would use).

## Semantic rationale

Sorting is a permutation of the elements, each moved once; its
observable effects are the calls of `less` (on shared references while
the `Vec` is borrowed exclusively) and the final order.

## Usability

    Vec::sort(&mut names);
    Vec::sort_by(&mut people, by_age);        // fn by_age(ref<Person, shared> a, ref<Person, shared> b) : bool
    match (Vec::binary_search(&names, &who))
    {
        Ok(i) : printf("found at %v\n", i),
        Err(i) : printf("would go at %v\n", i),
    }

## Explainability

"`sort` puts a `Vec` in order, keeping equal elements as they were;
`binary_search` finds a key in a sorted one, or where it would go."

## Implementation-feasibility

About 150 lines of `std` CobaltC; `key_less` in the checker, `coby` and
`cobc` (`cb_bytes_less` for text); `cobc`'s native `Vec::sort` for
integers and `bool`s (`cb_sort_plain`).

## Compatibility impact

Extension.

## Prior-art status

- **Rust:** `slice::sort` (stable), `sort_by` with an `Ordering`,
  `binary_search` returning `Result<usize, usize>`.
- **C:** `qsort` (unstable, three-way), `bsearch`.
- **Go:** `sort.SliceStable` with a less function, `sort.Search`.

## Revisit conditions

- Sorting slices in place.
- `sort_by_key`, reversing, deduplicating.
