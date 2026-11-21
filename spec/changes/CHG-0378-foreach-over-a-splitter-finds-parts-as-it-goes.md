# CHG-0378 — A `foreach` over a text splitter finds the parts as it goes

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-27)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0287, D-0281, D-0042, D-0128, D-0181
Affects: `spec/14-control-flow.md` 1.33.0 (`rule.control.foreach-parts`), `spec/conformance.md` 3.276.0;
`impl/std/text.cb` (`Parts`), `impl/src/parser.rs`, `impl/src/modres.rs`, `impl/src/each.rs`,
`impl/src/typecheck.rs`

## What changed

A consuming `foreach` whose collection is written as a call of `StringView::split`, `StringView::lines`,
`StringView::split_whitespace`, `StringView::chars` or the `String` function of the same name walks the text
(`[Foreach-Parts]`). The parser marks the consumed form over a call; name resolution maps a call that resolves to
one of the eight splitters, with no type arguments, to the matching constructor of `std::Parts`, a `std`-private
cursor (a view of the text, where the next part starts, the part last found, how many parts it has given, whether
one is left); `each.rs` walks a `std::Parts` by value: the bound is `Parts::bound($d)`, and each step is
`{ $d = Parts::next($d); StringView::sub($d.v, $d.lo, $d.hi) }`, so the cursor is never borrowed and a decided
program keeps it as plain data, with no runtime object. The static pass holds a `Parts` as it holds a view
(frame-held, no data holding references), so each part inherits the text's facts.

## What changed for programs

Nothing a program can observe: the parts, their number and order are the splitter's. Such a loop no longer makes
its program tier D: a word count over a 13.7 MB text (400 000 lines, `foreach` over `String::lines` and
`StringView::split_whitespace`) runs in 0.47 s and 42 MB instead of 9.80 s and 160 MB (`cobc`, GCC), the same
output. A program that binds the `Vec` first, borrows the loop (`&f(…)`) or indexes the result keeps the `Vec`.

## Conformance

`conf.foreach-parts` (positive, both tools; decided under `cobc`).
