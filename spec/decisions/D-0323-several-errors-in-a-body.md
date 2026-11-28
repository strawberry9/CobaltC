# D-0323 — Within a body, an error for each statement that stands on its own

Status: ACCEPTED (2026-11-14, the owner: "I want them all, write the plan so my agent can implement it in full
autonomously", then "implement the revised plan in full and push once completed", on
`private/EASE-PLAN-2-2026-11-14.md`, item 1 and its decisions Q1 to Q3); implemented by `CHG-0450`
Amended: 2026-11-14 by `CHG-0455` (Update 190): clauses 5 to 7, the blocks of a statement (the plan's
stage B, left out of Update 189 and built with these three rules)
Kind: Decision (`spec/02-schema.md`); the diagnostic registry's reporting order
Governed by: `CobaltC_Master_Instructions.md` §19
Depends on: D-0321 (one error per item, whose sentence this replaces for bodies), CHG-0130, D-0111 (a reference
ends after its last use), D-0296 (both toolchains in step)
Supersedes: D-0321 §3's "a phase reports at most one error for each item", for the phase of the bodies
Affects: `spec/registry/diagnostics.md` (Purpose, 1.69.0); `spec/conformance.md` (3.312.0); `impl/src/`
(`typecheck.rs`, `lib.rs`); `impl/selfhost/front/typecheck.cb`, `impl/selfhost/cbc/diags.cb`

## 1. Problem

D-0321 reports one error for each function, so a `main` with five errors still takes five runs. Four programs
written cold (`private/EASE-PLAN-2-2026-11-14.md` §1) had three to five each in `main`. D-0321 rejected "every
error the checker finds" because a later error may follow from an earlier one (a binding whose declaration
failed); the problem is to report more without ever reporting such a follow-on.

## 2. Options

1. **Every error.** Rejected by D-0321 §2, for the same reason.
2. **One error for each statement of a body that stands on its own.** The state is put back after a failed
   statement, so the later statements are judged as if it were absent; what its absence could change is named
   and the statements that depend on it are left unchecked. **Chosen.**
3. **Also inside nested blocks** (the blocks of an `if`, a loop or a `match` in statement position). A first
   form, recovering a nested block as the body's own is recovered, gave 20 single injections a second, false
   error: the hidden locals of a compound assignment's expansion, whose block was taken for the program's,
   and states a loop body's fixed-point walk had carried round from an undone statement. **Chosen**, with
   three rules that remove them (§3, 5 to 7).

## 3. Decision

Within the phase that checks bodies, a body reports the earliest error of each statement of its own block, and
of its tail, that stands on its own; a compound statement (`if`, `while`, `match`, a block) is one statement.
After a statement fails:

1. the body's state is put back to what it was before the statement;
2. a local the statement declares with a written type (`T x = e;`) is declared at `T`;
3. every other local it declares (`auto x = e;`, a destructuring's names), and every local it mentions that,
   before it, was not valid and initialised, had a value the static pass knew from a literal (`spec/14`'s
   value-range row), or has a type that may hold a reference, is *poisoned*;
4. after a failed `return`, `break`, `continue`, or call of `exit` or `fault`, the rest of the block is not
   checked.

Within a statement in statement position that holds blocks (an `if`, `while`, `for`, `foreach`, `match` or a
block), its blocks are checked the same way, with three differences:

5. a failed statement of such a block ends the block as a `return` would: the rest of it is not checked, and
   the point after it is unreachable, so nothing after it reaches a join (the end of the `if`, the loop's next
   round);
6. a block the front end made of one statement of the program (a compound assignment's
   `{ auto $vN = &mut p; *$vN = … }`, whose first statement declares a hidden name) is one statement;
7. the body's statement that holds a block with an error is then a failed statement itself (1 to 3 apply to
   it), its errors those of its blocks.

A block in value position and a closure's body are one statement, as before.

A statement that mentions a poisoned local is not checked, and is treated as a failed one for 2 and 3. Why that
is enough: with the state put back, a later statement can be judged differently from the program with the
failed statement fixed only through what that statement would have declared, given a value, given a literal
value, or done to a reference's borrows; each is a poisoned local, and nothing that mentions one is checked.
A body that recorded an error is not checked for its result type. The errors are keyed by their function and
place, so two instantiations of a generic body give one error for a statement. When any statement was not
checked, the report ends with the line `note: statements that depend on a failed statement were not checked`,
so a reader does not take the statements left out for correct.

Unchanged: the phases, the first diagnostic (the one D-0321 and CHG-0130 report), the limit of ten and the
count line, the exit status, what a conformance case requires.

## 4. Consequences

- An accepted program pays nothing: the second walk of a body, with a snapshot of its state before each
  statement, happens only for a body that failed.
- The measure, `impl/tools/error_mutants.py` (seed 189, 30 mutants for each of the 88 single-file showcases):
  2,550 mutants with one to three injected errors (an undeclared name, `true` for an integer literal, a
  doubled letter in a qualified function's name), each with its injections taken alone beside it. A
  diagnostic in a mutant that none of its parts reports is a follow-on. The first run found 11, all one
  shape (a failed `foreach` that would have written `best_r` left its literal fact, so a later
  `next[best_r]` was a static index out of bounds), which clause 3's literal fact removed. With the body's
  own statements alone (1 to 4): no follow-on, no injection alone giving two errors, and 1,348 of the 1,688
  mutants with two or three injections reporting every injection's errors. With nested blocks (5 to 7): no
  follow-on; 1,399 report every injection's errors, 118 one or none; one injection alone gives two errors,
  both true (it renamed a loop variable's declaration, so the name is undeclared at two uses).
- D-0321's syntax and item phases are unchanged.
