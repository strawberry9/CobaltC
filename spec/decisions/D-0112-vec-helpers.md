# D-0112 — `Vec` helpers the corpus keeps re-writing

Status: ACCEPTED (2026-09-30, the owner: "accept your recommendations, proceed with D-0112" — the findings report's F5, `private/findings.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 10), §9
Depends on: D-0082 (`Vec::clone`: plain `T`, checked at the call), D-0085 (`Vec::from_slice`, `truncate`), D-0089 (fn values), D-0090 (`eq` bound), D-0108 (`String` compared in place), rule.stdlib.vec
Affects: `spec/21` §0, `spec/conformance.md`, the guide §21, `impl/src/prelude.rs`, `impl/src/typecheck.rs` (two call-site messages)

## Problem

A scan of the repository's 288 programs (about 47 000 lines) found the
same loops around `Vec::push` and `Vec::swap` written over and over,
each saying less than a name would:

| Shape | Sites | What was pushed |
|---|---|---|
| fill: `foreach (k in 0..n) { Vec::push(&mut v, x); }` | 62 | `0` ×10, `false` ×8, `Vec::new()` ×4, `None` ×4, a byte, a struct literal, a computed value |
| append or map: `foreach (x in b) { Vec::push(&mut a, f(x)); }` | 35 | `x` moved ×8, `*x`, `f(x)`, `String::clone(k)`, `eval(a, env)?` |
| position: `foreach (i, x in &v) { if (p) { at = i; break; } }` | 22 | a field test, a `String` equality, a byte compare |
| 2-D: push `Vec::new()` per row, then fill each | 11 | (85 `Vec<Vec<…>>` types) |
| reverse: swap from both ends | 4 | |

D-0033 declined `Option`'s `map`/`and_then` as "not needed by any
program yet". That was the right test, and these shapes now pass it.

## Candidate mechanisms

1. **Leave them to loops.** Every program writes the same three lines,
   and a reader must recognise each loop's intent.
2. **Ordinary exported functions of `std`, written in CobaltC**, as
   D-0085 did. No rule changes. Element copying follows D-0082 exactly:
   a function that copies a `T` by reading it is rejected at the call
   for a resource `T` (`diag.type-mismatch`, the message naming the type
   and the alternative), and a `fn`-taking twin serves resources.
   Selected.
3. **Wait for a `clone` bound** (the report's F4) and add the copying
   functions under it. Ties a small library addition to an open design
   question, and D-0082/D-0085 already fixed the "plain `T`, checked at
   the call" convention.

## Selected design

    Vec::filled<T>(usize n, T x) : Vec<T>                       // n copies of x; T plain
    Vec::from_fn<T>(usize n, fn(usize) : T f) : Vec<T>          // f(0), …, f(n-1); any T
    Vec::append<T>(ref<Vec<T>, exclusive> a, Vec<T> b)          // b's elements moved onto a; b consumed
    Vec::extend_from<T>(ref<Vec<T>, exclusive> a, slice<T, shared> s)   // copied; T plain
    Vec::position<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>) : bool p) : Option<usize>
    Vec::index_of<T: eq>(ref<Vec<T>, shared> v, ref<T, shared> x) : Option<usize>
    Vec::contains<T: eq>(ref<Vec<T>, shared> v, ref<T, shared> x) : bool
    Vec::reverse<T>(ref<Vec<T>, exclusive> v)

- `filled(n, x)` reads `x` `n` times, so a resource `T` (`Vec<T>`,
  `Option<Box<T>>`, `String`) is rejected at the call with a message
  naming `from_fn`. A grid is one line:
  `Vec::from_fn(rows, [](usize r) : Vec<u8> { Vec::filled(cols, 0) })`.
  The closure takes the index so a row may depend on it.
- `append(a, b)` is the corpus's `foreach (x in b) { push(a, x) }`; the
  name follows `String::append`. `extend_from(a, s)` is the copying
  twin, over any array, `Vec` or slice through `&c[lo..hi]`, plain `T`
  only, like `Vec::from_slice`; a slice of `a` itself is the aliasing
  conflict it looks like (`[Slice-Form]`).
- `position(v, p)` takes a `fn` value, so a closure capturing the wanted
  key by borrow serves (D-0089); it is the search that works for user
  structs, since no user type satisfies `eq` (D-0090). `index_of` and
  `contains` take `x` by reference so a `String` key is not moved, and
  compare under `[Cmp-*]` (a `String` in place, D-0108).
- `reverse` is a loop of `Vec::swap`; nothing is destroyed.
- Not added, by the owner's acceptance of the recommendation: `map`
  (three lines in user code), `last`/`last_mut` (`v[$ - 1]` reads and
  writes the last element and faults when empty, D-0047; the guide's
  §16 shows it), `filter`, `retain`, `fold`, `any`, `all`.

## Compatibility impact

Additive. A program defining its own `Vec::filled`, `Vec::append`, …
keeps its own (D-0024). No rule, grammar or diagnostic changes; both
tools run the CobaltC bodies (cobc substitutes only `Vec::index_*` and
`Vec::len` natively).

## Revisit conditions

If user types ever satisfy `eq` (D-0090's revisit condition), `index_of`
and `contains` gain them with no change here.
