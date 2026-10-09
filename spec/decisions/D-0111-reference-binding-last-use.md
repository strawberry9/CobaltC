# D-0111 — A local reference binding ends after its last use

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 proposal P6, option A)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §12
Depends on: D-0008 (scope-end destruction), D-0018 (a path lives while something holds it), D-0019
Affects: spec/14 `rule.control.block`, spec/10 §1

## Problem

A reference binding held its path until its block ended, so

    ref<Vec<u8>, shared> b = String::as_bytes(&cur);
    … read b …
    String::clear(&mut cur);        // diag.aliasing-conflict: `b` still borrows `cur`

was rejected although `b` was never used again. The fix was an extra
`{ }` around the reads. It was the friction the round-6 programs met
most often, and the first one a programmer coming from Rust or C++
meets.

## Candidate mechanisms

- **A: a local reference binding stops holding its path after its last
  use.** Selected.
- **B: keep lexical holding** and teach the `{ }` idiom.

## Selected design

- A binding declared by a `let` of a block, of type `ref<…>`,
  `slice<…>` or `StringView`, ends after the last statement of that
  block that mentions it, as it would at the block's exit
  (`[Ref-Binding-Last-Use]`): its object ends, and so does its hold on
  the path it refers through. A use inside a loop, a branch or a nested
  block is a use by the enclosing statement, so a binding used in a
  loop lives through the whole loop.
- It ends at its block's exit, as before, when:
  - its block's trailing expression mentions it;
  - a closure in its block captures it (the closure may use it later);
  - its name is declared twice in the block;
  - another binding holds a reference through it or to it; or
  - a binding that may hold a reference (a `Vec<StringView>`, a struct
    with a reference field …) shares a statement with it and is used
    later (it may hold a reference derived from it).
- Resources are unaffected: nothing is destroyed earlier.

## Compatibility impact

Programs rejected only because a reference binding lived to its
block's end are accepted. A program that was accepted is accepted,
with the same output: ending a plain reference object has no
observable effect but the path it no longer holds.

## Revisit conditions

Bindings of other types that hold references (`Option<ref<…>>`, a
struct of references) could follow if programs ask for it.
