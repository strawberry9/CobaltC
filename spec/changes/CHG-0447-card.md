# CHG-0447 — CARD.md, CobaltC on one page, generated

Status: implemented (2026-11-14); Update 188
Affects: `impl/tools/card.py` (new), `CARD.md` (new, generated), `impl/tools/gate.py` (`records`),
`README.md`, `showcase/SHOWCASE_INSTRUCTIONS.md`

## Why

Item 4 of the plan. A reader who knows C or Rust, or a model about to write CobaltC, needs the syntax, the
differences from those languages, the tools, the diagnostics and the standard library's names in one place,
and none of it can be allowed to fall behind.

## What changes

`card.py` writes `CARD.md` from the book's appendices A and C, `impl/tests/foreign_forms/expected.txt` (CHG-0445),
`cobc`'s help and `coby`'s usage, the diagnostic registry, and `cobdoc.py`'s items (each with its signature and
summary; removed names left out). `card.py --check` fails when `CARD.md` is not current, and the gate's
`records` unit runs it, with `impl/tests/foreign_forms/run.sh --compare`. The examples on the card are the book's,
which the book's checks run.
