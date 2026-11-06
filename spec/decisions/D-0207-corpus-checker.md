# D-0207 — A machine check of the normative corpus

Status: ACCEPTED (2026-10-11; the owner: "Implement WI-11 as D-0207")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §13, §14, §18
Depends on: D-0206
Affects: `impl/tools/speccheck.py` (new), `impl/battery.sh` stage 1; the findings: `spec/19` 1.11.2,
`spec/21` 4.53.1, `spec/conformance.md` 3.201.0, `spec/02` 1.0.69 (CHG-0245)

## Problem

The specification's entities, rule labels, diagnostics, decision and change numbers, conformance rows and
edition are linked by name across thirty files, and nothing checked the links. The independent review of
edition 2026.101003 asked for a machine-checkable corpus (its evidence table, row 1;
`ChatGPT_review/ACTION-PLAN-2026-10-11.md` WI-11); D-0206's schema repair (CHG-0242) would have been found
by such a check, and so would the eight stale references below.

## Decision

`impl/tools/speccheck.py` (Python 3, standard library only, Windows-portable) reads `spec/` and reports,
with file and line:

- every entity heading has a `**Status:**` from the closed set, and an artifact's `Version:` is the version
  of its first Change Log entry;
- every `**Depends on:**` target resolves to an entity, a decision or a change record;
- every rule label `[Name]` cited in the artifacts, the conformance cases and the registries is defined
  by some artifact — as a CFN rule block, or in prose or a table cell (`` `[Contains]` — … ``);
- every `diag.*` named in `spec/` is in the diagnostics registry, and every registry entry is named by an
  artifact;
- `disposition:` and `outcome:` values are from `spec/01` §5's vocabularies, and a
  `⟦ … ⟧ discharge: trusted` side-condition lies under a `trusted-unchecked` rule;
- every `D-nnnn` and `CHG-nnnn` cited has a file, the numbers are contiguous, and a change record names its
  decision (a warning: the first records predate the convention);
- `spec/README.md`'s bold edition is its table's last row;
- every `conf.*` row's program is an inline fragment, an existing case file, or an `ex.*` of
  `spec/examples.md`;
- `spec/21` §0's trusted table is what `impl/tools/trusted_table.py` generates (D-0206).

History is a warning, not an error: a decision, a change record and an artifact's Change Log cite labels
and ids as they were when written, and that history is append-only. The built-in types (`type.bool`,
`type.ref`, `type.usize`, …) have no entity heading — `spec/06` §1 gives them in a table — and are accepted
as references to it; giving them headings is a later decision. Case files that no conformance row names
(314 of 831) are counted, not reported: the file suites run every file, and a row is written where a
derivation is worth stating.

The check runs first in `impl/battery.sh` stage 1 and takes under a second. `--warnings` prints the
history warnings too.

## Results

First run, before any fix (`stress/speccheck_oct11/first-run.txt`, local): 176 errors, of which all but ten
were the checker's own noise (examples carry the artifact's status; registry lines list several ids; a
row naming two files; the metalanguage's own grammar for the `discharge` tag; Change Logs) and were
refined away. The ten genuine findings, fixed by CHG-0245:

- `[Push-Ascii-Not-Ascii]` carried `disposition: fault`, not a vocabulary word; it is `checked`.
- `[Local-Offset]` carried `outcome: the platform's`, prose where a tag is required; it is
  `unspecified { the platform's offset o, −64800 ≤ o ≤ 64800 }`.
- `rule.conc.cancel` (`spec/19`) and `rule.stdlib.sync`'s supervised-task rules (`spec/21`) depended on
  `rule.fail.fault-contain`, which does not exist: `[Fault-Contain]` is a rule of `rule.fail.fault-unwind`.
- `rule.stdlib.crypto`'s digest rules depended on `rule.arith.wrap` and `rule.arith.bits`: the entities are
  `rule.arith.checked` (`[Wrapping]`) and `rule.arith.bitwise`.
- Three conformance derivations cited labels that were renamed long ago: `[Index-Array]`
  (`[Index-Checked]`), `[Deref]` (`[T-Deref]`), `[Hash-Kind]` (`[Hash]`).

After the fixes: 0 errors, 62 warnings (history). The warnings are left as they are.
