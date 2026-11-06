# CHG-0281 — Editor support, `cobc` on other platforms, chapter pages and newcomer chapters: declined for the 2026 edition series

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; under D-0234)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0234
Affects: `spec/02` §5 (1.0.87); `impl/STATUS.md` (Limits, Known gaps); the guide's introduction
(`htmlguide/generator/content_intro.py`); `impl/CHANGELOG.md`

## What changed

- **D-0234** is recorded: the four work items of the 2026-10-12 plan that the owner had put on hold (editor
  support, `cobc` on Windows and macOS, the guide as chapter pages, the Getting Started, Common Mistakes,
  Cookbook and Glossary chapters) are declined for the 2026 edition series.
- `impl/STATUS.md`: the compiler's Limits say that compiled programs are built on x86_64 Linux only in this
  series (D-0234); the Known gaps no longer list the four items as not started or on hold. The gap that stays
  is the one that is still a gap: the built-in items have no `///` documentation.
- The guide's introduction: the sentence on Windows says `cobc` builds programs on Linux only in this edition
  series, rather than that it "does not yet" follow `coby` there.
- `spec/02` §5's "in use" ranges: D-0234, CHG-0281.
- `impl/CHANGELOG.md`: the decisions index gains D-0233 and D-0234; a section for this record.

No rule, no `std` item and no conformance row changed; the specification's normative text is as at
edition 2026.101402.

## What changed for programs

Nothing.
