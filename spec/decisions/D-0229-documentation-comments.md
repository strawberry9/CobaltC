# D-0229 — `///` before an item is its documentation

Status: ACCEPTED (2026-10-14; Q9 of the 2026-10-12 plan, recommendation accepted by the owner)
Kind: Decision (`spec/02-schema.md`); a convention, with no language change
Governed by: `CobaltC_Master_Instructions.md` §21
Affects: `spec/22` §1 (2.49.0) (CHG-0273); `impl/tools/cobdoc.py`; `impl/cobfmt/tests/cli.rs`

## Decision

A run of `//` comments that each begin `///`, immediately before an item with no blank line between, is that
item's documentation. It is written in Markdown, and its first sentence is the item's summary.

- **Lexically nothing new.** `///` is a line comment. The grammar does not change, and no program's meaning
  depends on a comment. `cobfmt` keeps comments where they are; a test now pins that a `///` block stays
  directly before its item.
- **`impl/tools/cobdoc.py`** follows a program's module tree from its root file (by default `std`'s) and
  lists every exported item: its module, kind, signature, documentation, summary, file and line, as JSON.
  `--missing` lists the items that have no documentation yet. The std reference (WI-18) will be generated
  from this output.
- A `///` comment on a `unittest` block right after an item makes that test the item's example (D-0230).

## Alternatives considered

- **No convention, with the std reference written by hand in the guide's generator.** Declined: its
  signatures would be copied and could go stale.
- **A doc-comment token in the grammar (`/** … */`, or attributes).** Declined: a convention costs no new
  token.

## State at acceptance

`std` has 670 exported items, and none has a `///` comment yet. WI-18 writes them; this record does not
require them.
