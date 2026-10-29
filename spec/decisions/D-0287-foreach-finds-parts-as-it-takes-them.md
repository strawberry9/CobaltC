# D-0287 — A `foreach` over a text splitter finds the parts as it takes them

Status: ACCEPTED (2026-10-27, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 phase 1, the lazy splitters (§6, decision 4 of §16).
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0042 (`foreach`), D-0281 (tier S), D-0282 (frame references decided statically), D-0284 (decided
programs), D-0128, D-0181 (the splitters)
Affects: `spec/14-control-flow.md` (`rule.control.foreach-parts`, `rule.control.frame-references`),
`spec/conformance.md`; `impl/std/text.cb`, `impl/src/parser.rs`, `modres.rs`, `each.rs`, `typecheck.rs`; `CHG-0378`, `CHG-0379`

## Problem

The commonest text idiom, `foreach (l in StringView::lines(v))`, and its kin over `split`, `split_whitespace` and
`chars`, made the whole program tier D. The splitter returns a `Vec<StringView>`; the loop consumes it through a
`Drain`, and an element taken out of a `Vec` of views is a reference read out of data, which the static pass
cannot follow (D-0281 §7). Such a program kept every record of references for its whole run: a word count over a
13 MB text took 9.8 s, where the loop it means is a scan.

## Options

- **A. An iterator type.** The splitters return a lazy `Lines`/`Split`/… that `foreach` walks, with a `collect`
  for the programs that want the `Vec` (D-0281 §6's sketch). Every program that keeps the `Vec`, takes its
  length or indexes it would change, and the iterator, a struct holding a view, is itself an aggregate holding a
  reference.
- **B. The loop form, not the function.** The splitters keep their contract. A `foreach` that consumes one of
  their calls directly walks the text instead: the loop holds a `std`-private cursor, a view of the same text,
  and finds each part when it takes it. The parts, their order, and every program's outcome are the `Vec`'s; only
  the `Vec` is gone. A program that binds the `Vec` first keeps it.
- **C. A cheaper `Vec` of views.** Make a `Vec<StringView>` tier S when its elements are read in a `foreach`
  only. A rule about data holding references, the hardest kind (D-0281 §7), for one idiom.

## Decision

B. `foreach (x in f(a…))` and `foreach (k, x in f(a…))`, where `f` is `StringView::split`, `StringView::lines`,
`StringView::split_whitespace`, `StringView::chars` or their `String` forms, visit the parts `f(a…)` would hold,
in its order, each found when the loop reaches it (`rule.control.foreach-parts`). The loop holds a
`std::Parts`: the view, where the next part starts, the part last found, how many parts it has given and whether
one is left. The loop passes it by value, never borrowing it, and the static pass holds it as it holds a view, so each part is a view of the argument's text, tier S, as the body's
other views are.

The rule changes nothing a program can observe. The text is borrowed shared for the whole loop, so it cannot
change between parts; the parts are views of it, valid as long as it is, inside or after the loop. Leaving the
loop early leaves nothing to destroy. A `Vec` that is not made cannot fail to be allocated.

## Repairs found with it (CHG-0379)

Making these loops tier S put their views where the static pass had not followed views before, and three holes in
D-0284's static pass showed, each letting a decided program read a `String` that had ended:

1. **A block's end.** A reference copied into a binding declared outside a block (`keep = v;`, `v` a view of a
   `String` the block declares) was not checked when the block ended: the escape check sees only a borrow written
   in place. Now, when a block ends, a binding declared outside it that surely holds a reference to what the block
   ends, and is used later, is `diag.destroy-while-aliased` (static, D-0282); one that may hold one keeps the
   program's tracking, and the run decides.
2. **A loop's body.** The facts a copy of a reference makes (`keep = l;`) were made only on the static pass's
   last pass over a loop's body, so they never reached the loop's head or its exit: a reference kept from one
   iteration to the next, or past the loop, was taken to hold nothing. They are now made on every pass.
3. **A literal argument.** A call whose first reference argument is static data (a literal where a view is
   expected) gave a result of unknown origin, which kept tracking; its result is static data too.

## Consequences

- No program changes meaning. The splitters' contracts are unchanged; `Vec::len` and indexing of their results
  work as before, at tier D's price.
- The lazy loop is tier S: the word count above runs in 0.47 s (42 MB) instead of 9.8 s (160 MB), the same output.
- The repairs make some programs that were decided keep their tracking (they now fault where `coby` faults), and
  reject statically the ones that surely read an ended `String` (`coby` faults on each at run time).
- `coby` runs the same loops; `cbc` (frozen, D-0278) keeps the eager form until it is ported.

## Verification

See `CHG-0378` and `CHG-0379`.
