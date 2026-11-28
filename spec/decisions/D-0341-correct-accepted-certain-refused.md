# D-0341 — Correct programs are accepted, certain faults refused: the improvements D-0340 left

Status: §1 is the owner's rule (2026-11-25); §2 decided by the implementer under it (the owner: "improvements to the
compiler are always authorised if they assist in allowing incorrect programs to be determined at compile time; only
language and syntax changes require owner approval; correctness trumps speed always, although speed is very
important also"). Implemented by `CHG-0484`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0111 (`[Ref-Binding-Last-Use]`), D-0288, D-0336 §2.7, D-0339 §2.5, D-0340
Revisits: D-0340 §3 (418, 392, 409's `auto` `if`/`match`, dead branches)
Affects: `spec/14`, `spec/18`, `spec/12`, `spec/10`

## 1. The rule (owner)

1. A program that is correct is accepted at compile time.
2. A program certain to be incorrect is rejected at compile time (D-0340, `[Certain-Fault-Refused]`, per path).
3. Where there is doubt, the compiler is improved so that the doubt is decided at compile time; only where no
   improvement is viable is it left to the run. Such improvements need no approval; language and syntax changes do.
   Correctness before speed; speed matters too.

## 2. Decision

1. **Assigning a reference is no use of its loan (418).** A `ref` binding whose next mention in a block is a whole
   reassignment (`x = e`, `e` not naming `x`), with nothing between that may leave the block, and no other binding's
   facts rooted in it, holds no loan from its last read before the assignment: the checker drops its facts there and
   the run empties its slot (`Facts.releases`, `cb_store_ref(&slot, 0)`). `spec/14`'s last use is the last statement
   that *reads* the binding. The cursor over a `Vec` (`cur = &steps[i]` after writing `steps[i]`) is accepted by both
   builds.
2. **A holder whose element is read later (392).** A holder of references in data that may hold a binding's path
   when the binding is written, and an element of which is read later (`h[i]`, `h.f[i]`), faults on that path
   whatever it holds (no element: out of bounds; the element: the write through it): refused at the check. A holder
   only measured later (`Vec::len(&h)`, after a `pop`) may hold nothing: the run's.
3. **Open type slots across branches (409).** A branch or arm whose type leaves a slot open (`Ok(true)`: the error
   type) and another that fixes it (`Err("…")`) give the `if` or `match` the type with the slot filled
   (`merge_open`); a slot left open everywhere stays `diag.type-annotation-needed` / `diag.cannot-infer-type-parameter`.
4. **A branch never taken holds no fault (correct programs accepted).** A branch whose condition's operands decide it
   (`const_bool`: literals, known integers, `!`, `&&`, `||`, comparisons) the other way is checked as unreachable: no
   fault found there refuses the program (`if (!(1 <= 1)) { 1 / (1 - 1); }` is accepted).
5. **The lowering's scaling.** `elem_ref_lets` judges each element reference on the statements that name it or its
   vector (`rest_naming`), as `quiet_locals` does since CHG-0483: linear in a block of thousands of such `let`s.
6. **Messages.** The `Option<R>` overwrite says "it is not `None` on every path to here".
7. **Correct programs refused, accepted (rule 1).** A view's slice passed to a call refers to what the view views
   (396); an arm binder used only in an `if` condition ends after it (417); a copy whose references are all shared
   views is a shared fact on its field (415); a literal beside an operand whose type a generic body does not yet
   know takes the context's type (437); `&K` of a constant array is a `foreach` source (445); a literal `*` width
   is a `usize` (400); a module file may start with `#!` (370).
8. **The standard library.** `reason_phrase` names every RFC 9110 status (427); the HTTP server sends `Date` (436).

## 3. Left

- 369: an array literal borrowed as a generic argument (`Vec::from_slice(&[1, 2, 3])` for a `Vec<u8>`) is not
  given the element type the declaration fixes.
- The lowering's copies of 100,000-deep types (`check_scales_with_nesting_and_references.case` still times those
  through the static pass alone).
- 411's message (the lowering's, not the type error of `Option::unwrap(&a)` alone).
- Priorities 3–5 of reports 350–451.
