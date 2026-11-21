# D-0328 — The ref `rust-reference` is not published

Status: ACCEPTED (2026-11-17, the owner: "also remove the rust-reference tag from upstream (on github) as it is
not needed"); implemented by `CHG-0467`
Kind: Decision (`spec/02-schema.md`); the repository, not the language
Governed by: `CobaltC_Master_Instructions.md` §9
Depends on: D-0326 (the toolchain without Rust; its item 8)
Affects: `README.md`; `impl/STATUS.md`; `spec/ASSURANCE.md`; D-0326 (a note); and, as its consequence, the
texts and the tests' records that pointed at the Rust tools (`CHG-0467`). No rule of the language changes.

## 1. Problem

D-0326 item 8 took the Rust sources out of `main` and kept the last tree that has them as one ref,
`rust-reference`, "the historical oracle for a suspected miscompile". The repository is published as a single
commit that each update replaces, so the tag was the only way a reader had to that tree, and six texts sent
the reader to it.

Since the cut-over the tree has not been used. What the Rust tools were kept for is done by the tools as they
are: the compiler writes itself again from the seed, by GCC and by Clang, on every update and on every push
(`bootstrap`); it is run against itself on random programs (`differential`); the model reads the alias and
temporal rules a second time (`model`); the recorded runs hold every case's output. A suspected miscompile is
settled against the specification and those, not against a compiler that stopped at Update 193.

## 2. Decision

The tag `rust-reference` is removed from the published repository (done 2026-11-17). No ref of the published
repository holds the Rust tools.

D-0326 item 8 stands in its first sentence (the Rust sources left `main`) and not in its second: the tree is not
kept as a published ref, and nothing in the project relies on it.

## 3. Consequences

- The texts that named the ref say what was done with the Rust tools and where it is recorded (`CHG-0457`,
  `CHG-0458`, `CHG-0459`), not where the tools can be had.
- The comparisons with the Rust tools (the parse, the C in every form, the reports, the 674 test functions'
  port) stand as recorded. They cannot be run again from the published repository; none was a standing check
  after D-0326.
- What pointed a reader at the Rust tools goes with the ref (the owner, the same day): the comments that name
  a Rust file, the map of the Rust tests to their new homes and its check, the dormant options that took the
  interpreter's path (`CHG-0467`).
- `spec/ASSURANCE.md`'s two rows of kind `port` point at the record of the last comparison.
- The Rust comparison programs of `bench/` are not affected (D-0326 item 11).

## Revisit conditions

- A defect whose cause can only be found by running the Rust tools: the tree is Update 193's, and a copy of it
  would have to be had from outside the published repository.
