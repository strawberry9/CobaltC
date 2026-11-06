# D-0226 — The Unicode scope of `std`

Status: ACCEPTED (2026-10-13; the owner's answer to Q7 of the 2026-10-12 plan, "record the doctrine")
Kind: Doctrine (`spec/02-schema.md`); no code
Governed by: `CobaltC_Master_Instructions.md` §9 (the std helper admission test)
Depends on: D-0128 (`String::chars`), D-0181 (code points), D-0134
Affects: `spec/21` §2h (4.60.0), the guide's "Which text type?" (CHG-0269)

## Problem

What `std` promises about Unicode was nowhere written: whether a case mapping, a normalization or a grapheme
iterator was missing or absent by design.

## Decision

Text in `std` is UTF-8 bytes. Lengths, indices and ranges count bytes; equality and order compare bytes (code
point order for UTF-8); `String::chars` and `StringView::code_point` give code points. `std` carries no Unicode
tables: case mapping and folding are ASCII only, there is no normalization, no grapheme segmentation, and no
character class beyond ASCII. A program that needs them brings its own tables, or uses `std::extensions` where
an implementation offers them.

## Rationale

The tables are large, change with each Unicode version, and are needed by few programs; the byte model is
what every other `std` text function already assumes.

## Revisit

A `std::text::unicode` module, if a real program shows the need and the admission test (§9) is met.
