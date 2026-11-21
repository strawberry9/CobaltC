# D-0334 — What Update 204 left that could be closed

Status: ACCEPTED (2026-11-21). Decided for the owner, who was away and had asked that no report stay open: after
Update 204 the items of `CHG-0476`'s "What is left" that could be closed were closed in one more push. Implemented by
`CHG-0477`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0333 (the reports Update 203 left open), D-0332 (the reports), D-0116 (`clone`), D-0204, D-0257
Affects: `spec/16` §4a, `spec/17`, `spec/conformance.md`, `spec/decisions/D-0333`

## 1. Decisions

1. **A tree is cloned with a loop too (022).** A struct with two or more links to itself, or owning something on both
   sides of its link, and an enum whose variant with fields links to it, has a derived `clone` that recurses for the
   first 1,024 levels and goes on with a loop over a stack of its own, making the same calls in the same order. A type
   that links to itself only through another type still recurses.
2. **An array length may use the layout of any laid-out type (013).** `sizeof` and `alignof` of any type the program
   lays out, in an array length, directly or through a `const`: the lengths are computed once the program's items are
   known, with the constant evaluator, and the program is parsed again with their values. `sizeof` of a type
   parameter (one length serves every instance) and a length that depends on the size of a type holding it are
   refused, each with a message that says so.
3. **The program `--run` starts says why it was not started (037).** When the forked child cannot tie itself to
   `cobc`, set its processor limit or execute the program, it writes the reason to standard error before exiting 127.
4. **The Windows behaviour is tested on Windows (032, 037).** `impl/tests/windows/cleanup.py`, a step of the bootstrap
   workflow's Windows job: the processor limit ends the program, a console break ends `cobc` and the program and
   removes the directories, killing `cobc` ends the program (the job object). It passed on GitHub's runner.
5. **Left open for review (001).** The runtime's locking stays on in a program with entry points for the whole run, and
   `--lib` keeps the race of a second thread arriving while the first runs unlocked; the owner asked for it to be
   reviewed later.

## 2. Consequences

- Cloning a full tree of 1,023 nodes 3,000 times: a struct tree −11% (GCC) and +34% (Clang) instructions, an enum tree
  +7% and +9%.
- A program with array lengths that need a type's layout is parsed and its items built several more times; a program
  without them is unchanged.

## 3. Verification

See `CHG-0477`.
