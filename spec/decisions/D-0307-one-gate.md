# D-0307 — One gate: units with declared inputs, verdicts kept by the hash of their inputs, runs on a snapshot

Status: ACCEPTED (2026-11-02, the owner: every decision of `private/GATE-one-pipeline-proposal.md` as
recommended)
Kind: Decision (`spec/02-schema.md`); process and tooling
Supersedes: D-0212's "tested" record (`impl/tools/suites.sh`'s `TESTED`) and the staged battery as a separate
driver (`impl/battery.sh`, 2026-10-08); the per-proposal gate lists (D-0303's §9 was the last)
Affects: `spec/README.md` ("Evidence tiers"); `impl/tools/manifest.py` and `spec/MANIFEST.md`; `impl/gate.sh`,
`impl/tools/gate.py`; the corpus runners (`impl/tests/common/mod.rs`, `impl/cobc/tests/oracle/mod.rs`,
`spec_rows`, `cb_conformance_suite`, `compiled_spec_rows`, `compiled_suite`), the guide's and the book's checks

## Problem

On 2026-11-02 one run of D-0303's gates took 7 hours, a one-line repair meant another 7, and the battery (5 hours)
would have followed: about 12 hours of machine time per Update with the tree locked, 19 with one repair. Most of it
was work done again: `cargo test --workspace` repeated `suites.sh`; the conformance corpus was compiled five times
per run (the rows' files under GCC and Clang, the same files again as the file suite, a fifth time under
`--all-checks`); the battery repeated the gates under other names; and nothing kept a verdict, so a check whose
inputs had not changed (the Lean proofs, 19,911 parser mutants, the formatters' corpus) ran again in full. The
battery itself had grown from 2 hours (2026-10-08) to 5 (2026-10-30) as each decision added its check and none was
tied to what it reads. Of that run's four reds, one was a fault (found by an eight-second test); the others were
the harness (deleted files still in git's index, a stale scratch directory in a list, a hand-kept reference
measured on an older tree).

## Candidates

1. Keep the battery and the suites, and stop writing gate lists into proposals: removes the third layer only;
   the battery still repeats itself and keeps nothing.
2. Scope the battery by changed files (its classifier, 2026-10-08): coarse (any change to `impl/src` or `std`
   selects everything) and nothing kept between runs.
3. One driver of units with declared inputs, kept verdicts and per-case results, on a snapshot. Selected.

## Selected design

1. **One driver**, `impl/gate.sh` (`impl/tools/gate.py`), with two profiles. *gate*: what a commit and a push
   need. *release*: the gate and the slow units for the rarer classes of fault (the self-hosted fixed point, the
   whole-corpus formatter and parser comparisons, `--all-checks`, the debug profile, the `slow:` cases, the verify
   modes, the stress programs and sanitizers, the fuzzer, the external servers), run when the owner asks, not per
   Update. `impl/battery.sh` is `gate.sh release`; `impl/tools/suites.sh` is `gate.sh`.
2. **Units with declared inputs.** A unit is one check: a command, the tracked paths it reads, a limit. Its key is
   the hash of those paths' index entries, the versions of rustc, GCC and Clang and its own command;
   `README.md` and `spec/MANIFEST.md` are outputs and never inputs. A green verdict is kept under that key, and a
   unit whose key has a green verdict does not run. Units run one after another, at most three processes each.
3. **Per-case results.** The corpus runners, the guide's and the book's checks keep a result per case under the
   hash of the toolchain's sources and versions, the case's text, its support files and how it was run; only a
   result that met its check is kept. The compiled runners keep `cobc`'s run itself, so the rows' runner and the
   file suite share one compile per file per C compiler, and `coby`'s oracle run is kept the same way.
4. **A snapshot.** The gate runs on a clone of the repository at the commit (`--dirty`: with the working tree's
   changes), its build directory kept between runs; the working tree stays free to edit while it runs, and every
   verdict names the tree it was produced on.
5. **What the record says.** The evidence tier words stay: *reviewed* — no green gate for this tree; *tested* —
   every unit of the gate profile has a green verdict for this tree's inputs; *battery* — the release profile's
   too. `impl/tools/manifest.py` reads the kept verdicts; `spec/MANIFEST.md` lists each unit with its verdict.
6. **Rules.** No proposal lists its own gates; a proposal that adds a kind of check adds a unit. A push needs the
   gate green on the pushed commit. A red unit is repaired and run again; nothing whose inputs did not change runs
   again. The release profile runs on the owner's word (the battery's authorisation rule, carried over).

Declined (the proposal's §4): GCC alone in the gate (Clang has found C that GCC passed), `-O0` for the corpus (the
undefined-behaviour class shows at `-O2`).

## Compatibility impact

None on the language or `std`. The evidence tier of editions before 2026.110204 keeps its meaning as recorded.

## Revisit conditions

A fault found only by a release unit, twice: move that unit into the gate profile.
