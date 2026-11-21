# D-0234 — Editor support, `cobc` on Windows and macOS, and the guide's chapter pages and newcomer chapters: declined for the 2026 edition series

Status: ACCEPTED (2026-10-15; the owner: "record WI-10, WI-14, WI-16, WI-17 as declined for this edition series so
that no document implies they are pending")
Superseded in part: WI-14's Windows half by D-0255 (2026-10-16, the owner's decision): `cobc` compiles for
x86-64 Windows with MinGW-w64. The macOS half and the other items stand.
Kind: Decision (`spec/02-schema.md`); a scope decision about the tools and the guide, not about the language
Governed by: `CobaltC_Master_Instructions.md` §21, §22
Depends on: D-0220 (the stability policy, which names editions `YYYY.MMDDnn`)
Affects: `impl/STATUS.md` (Limits, Known gaps); the guide's introduction (`htmlguide/generator/content_intro.py`);
`spec/02` §5 (1.0.87) (CHG-0281)

## Problem

The forward plan of 2026-10-12 (`private/AGENT-INSTRUCTIONS-2026-10-12.md`, outside the repository) listed twenty
work items. Sixteen were carried out as Updates 103 to 107 (D-0219 to D-0233, CHG-0258 to CHG-0280). Four were
put on hold by the owner on 2026-10-14 while the rest went on:

- **WI-10** editor support: a TextMate grammar, a minimal VS Code extension with a `--check` task and problem
  matcher, a vim syntax file, a check-on-save script; later a diagnostics-only language server.
- **WI-14** `cobc` on Windows (clang and MinGW GCC; MSVC out of scope) and a best-effort macOS run, with an
  acceptance script the owner would run.
- **WI-16** the guide as chapter pages beside the single page, with shared navigation and a client-side search.
- **WI-17** four newcomer chapters for the guide: Getting Started, Common Mistakes, a Cookbook, a Glossary.

"On hold" left them pending in `impl/STATUS.md`'s Known gaps, the guide's introduction and the plan's own records,
with no decision anyone could cite.

## Decision

The four items are **declined for the 2026 edition series**: every edition numbered `2026.MMDDnn` (D-0220,
`spec/STABILITY.md`). Within this series:

1. `cobc` builds programs on x86_64 Linux with GCC or Clang. The workspace keeps type-checking for Windows and
   building with Rust 1.77.2 (`coby` and `cobfmt` run there), and `cbrt` keeps its per-platform link knowledge,
   but compiled programs are not built or run on Windows or macOS, and no acceptance script is prepared.
2. The project ships no editor grammar, extension, syntax file or language server. `coby --check` and
   `cobc --check` with the human-form diagnostic (D-0228: `file:line:column`, a caret, `--explain`) are the
   editor integration: any editor that can run a command and read that form has what it needs.
3. The guide stays one generated page (`htmlguide/index.html`, published as the book's appendix), with its
   introduction, tour, reference sections and the std reference (CHG-0276). No chapter pages are generated.
4. No Getting Started, Common Mistakes, Cookbook or Glossary chapter is written. The introduction's "Running a
   program", "Testing a program" and "Reading a diagnostic", the tour, and `README.md`'s build table are where a
   newcomer starts.

Documents describe these four as decided absences for this series, citing this record, not as work pending.
The plan's own text is unchanged, as a record of what was proposed.

## Why

The project's goal for this series is a stable specification, a dependable standard library and reliable tools
(the plan's opening). The four items add convenience around the tools rather than evidence about the language;
none bears on the safe-program guarantee, on `spec/21`'s assurance tiers or on conformance. Each is also open-
ended in a way the finished items were not: editor support invites publishing and maintenance across editors;
a second compilation platform doubles the compiled test matrix the battery already spends two hours on; chapter
pages and newcomer chapters are writing that is never finished. Declaring them declined for the series lets the
remaining documents say one thing, and lets a candidate edition (D-0220) be judged on what the series set out to
deliver.

## Revisit

A later edition series (`2027.…` or whatever follows) may take any of the four up again, as its own D-record,
starting from the material kept: the Windows plan for `cobc` (the former `WHERE-TO-NOW.md` §2, kept with the
plan's working files), the item descriptions in the plan, and the human-form diagnostic a problem matcher would
parse. Within the 2026 series the decision is reopened only by the owner.

## Alternatives considered

- **Keep them "on hold".** Declined: it is what led here; a reader cannot tell a paused item from an abandoned
  one, and `impl/STATUS.md` would go on naming work nobody is doing.
- **Do them now.** Declined by the owner on 2026-10-14 and again on 2026-10-15, for the reasons above.
- **Decline them for good.** Not needed: the series is the natural boundary, and the next series will have its
  own plan.
