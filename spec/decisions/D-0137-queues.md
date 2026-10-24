# D-0137 — `Queue<T>` and `PriorityQueue<T>` in `std::collections`

Status: ACCEPTED (2026-10-04, the owner: "write the proposal with PriorityQueue and Queue and then implement")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0041 (hash tables, key types), D-0062 (sorting, `key_less`), D-0110 (struct and enum keys), D-0134 (`std`'s naming conventions), D-0136 (`std::collections`)
Affects: `spec/21` §0 and §2i (new), `spec/conformance.md`, the guide §21, `impl/std/collections.cb`, `impl/src/typecheck.rs`, `impl/cobc/src/lower.rs`

## Problem

Two collections were written by hand again and again in the programs
we have (showcase/, stress/round5, stress/round6, bench/):

- **A priority queue.** dijkstra (round 5), routes/heap and checkout
  (round 6) each write a binary heap: a `Vec`, a comparison, sift-up and
  sift-down. Shortest paths, schedulers, event simulations and "the k
  best" all need one, and sift-down is easy to get subtly wrong.
- **A first-in first-out queue.** Five programs keep a `Vec` and a
  `head` index that only grows, so the `Vec` never shrinks; three take
  from the front with `Vec::remove(&mut v, 0)`, which moves every other
  element; three write their own `Ring` or `Queue`.

Each names one very common intent and is clearer than what programs
write without it, not only shorter (§9's admission test).

## Candidate mechanisms

1. **Leave it.** The hand-written forms work; each program pays for
   them in lines and in bugs.
2. **Rust's shapes and names** — `BinaryHeap` (largest first, ordered by
   a trait) and `VecDeque`. The names say the mechanism, not the intent;
   CobaltC has no `Ord` trait to order by; largest-first would reverse
   `sort`'s order for the same function.
3. **`PriorityQueue<T>` and `Queue<T>`, written in CobaltC in
   `std::collections`.** Selected; the owner chose the names.
4. **An ordered map (a balanced tree) as well.** The tree programs in
   the corpus write trees as the point of the exercise; `HashMap` and a
   sort of its keys serve ordered listing. Not adopted now.

## Selected design

`spec/21` §2i gives both, written in CobaltC:

- **`Queue<T>`**: `new`, `len`, `push_back`, `pop_front`, `push_front`,
  `pop_back`, `front`, `back` (each an `Option<ref<T, shared>>`, `None`
  when empty, as `HashMap::get`), `clear`. Two `Vec`s used as stacks, the
  front part and the back part, so every step is a `Vec`'s; taking from
  an empty part moves elements across from the other: all of them while
  the queue is taken from at one end only, half once it has been taken
  from at both, so every operation is constant time amortized. Elements
  left are destroyed front to back. The name says the common intent;
  `push_back` and `pop_front` against `push_front` and `pop_back` say
  which end at every call. Rejected: `Deque` (jargon, misread as
  "dequeue"); a `Vec<Option<T>>` ring (a tag per slot). A ring buffer
  over the raw-memory intrinsics, as `Vec` is, was the first
  implementation and was replaced for speed (below).
- **`PriorityQueue<T>`**: `new` (the key types' own order, as
  `Vec::sort`; another `T` is `[Priority-Queue-Not-Key]`,
  `diag.type-mismatch`, also at a generic function's instantiation),
  `new_by(less)` (as `sort_by`), `len`, `push`, `pop`, `peek`, `clear`.
  `pop` gives the least element first, so `new` and `sort` agree; a
  largest-first queue passes a "greater" `less`.
- **Equal elements leave first in, first out.** Each element carries an
  arrival number, compared only when `less` calls two elements equal. A
  plain binary heap's order among equals depends on its shape: fixed by
  the body, but not something a program could predict. An event
  simulation's simultaneous events run in scheduling order. Cost: one
  `u64` per element.

## Resolved during implementation

- **Speed under `cobc`.** `cobc` keeps a runtime record for every
  reference a program forms and every `Vec::swap`, and those records,
  not the heap's arithmetic, decided the cost. The first version (one
  `Vec` of (value, arrival number) pairs, `Vec::swap` at each level, two
  calls of `less` per comparison) popped 40,000 `u64`s in about 1.2 s
  (2.0 s with `new_by`), slower than a heap a program writes over
  `Vec<u64>` (0.65 s). The body now:
  - moves elements by `copy_raw` into a slot that holds nothing and
    `release` of the slot left, through the buffer's pointer, so a move
    forms no reference;
  - pops by moving the hole down to a leaf with one comparison per
    level, then the last element into it and up (seldom far);
  - keeps the arrival numbers in a second `Vec`, read through its
    pointer, so a comparison of two elements forms one reference to
    each (none for the key types' order, compared in place by
    `key_less_at`), not one to each pair and one to each value in it;
  - decides with one comparison: the earlier arrival comes out first
    unless the later goes before it, which is the order's definition
    when `less` is a strict weak order;
  - holds one shared reference to the heap per loop, not one per
    comparison.

  The same 40,000 now pop in about 0.23 s (0.57 s with `new_by`); a
  `String` queue went from 0.62 s to 0.51 s for push and pop together.
  A struct key (`(dist, node)`) is about 10% slower than with one `Vec`
  of pairs compared as one key; the two-`Vec` layout was kept because it
  is faster for every other case measured.
- **`key_less_at` in a branch not taken.** The comparison chooses between
  `key_less_at` and `less` at run time, so `cobc` lowers both for every
  element type, including those with no key order (a reference, an
  `Rc`), which a queue made by `new_by` may hold. `cobc` lowers
  `key_less_at` of such a type to the evaluator's dynamic verdict
  (`diag.type-mismatch`), as it lowers `[Match]` on a non-enum; the
  front end rejects every call that reaches the key order
  (`[Priority-Queue-Not-Key]`). Case `priority_queue_by_any_type_ok.cb`.
- **`Queue`'s speed under `cobc`.** The ring buffer reached each slot
  through `reclaim`, `release` and raw writes, each with runtime
  records, while `cobc` realizes `Vec`'s `push`, `pop` and indexing
  natively (D-0023): 200,000 `push_back` and then `pop_front` took 100 ms
  and 140 ms, where a program's own `Vec` with a growing head index took
  42 ms and 66 ms. Two `Vec`s as stacks turn every step into a native
  one; the elements crossing from one part to the other move by
  `copy_raw` and one `release` for the vacated range; and `cobc`
  realizes `Queue::front`, `back`, `pop_front` and `pop_back` natively
  for element types as its native `HashMap::get` and `Vec::pop` allow, as
  §0 permits. The same run now takes 44 ms and 74 ms, and a `front`
  costs what a `HashMap::get` does. A native `push_back` was tried and
  measured slower than the body's call of the native `Vec::push`, so the
  pushes stay as written.
- **Destruction order.** A priority queue destroys its elements in the
  order its `Vec` holds them, which the body determines; it is
  observable for elements with destructors, so it is specified as the
  body's, as every `std` body is.

## Compatibility impact

Additive: two new names in `std`, which a program's own items of those
names shadow (`[Resolve-Unqualified]`). A program that imports `std`
and another module exporting `Queue` or `PriorityQueue` and uses the
name unqualified now meets `[Resolve-Ambiguous]`; no program in the
corpus does.

## Revisit conditions

- `foreach` over a `Queue` front to back, or `q[i]`, if programs ask:
  each is a language change (`rule.control.foreach`, `[Index-Vec]`).
- `_mut` twins (`front_mut`), `clone`, `reserve`, `peek_mut`,
  `into_sorted`, if programs ask.
- An ordered map, when programs need one other than as an exercise in
  writing one.
- `Vec::swap` realized natively by `cobc`, which would speed every
  heap a program writes itself.
