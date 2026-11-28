# D-0284 — A program whose conflicts are all decided before the run keeps no record of references

Status: ACCEPTED (2026-10-26, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and on 2026-10-26 asked for all
of D-0281 to be completed without further prompting). D-0281 phase 2, step 4c.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0281 (tier S and tier D), D-0282 (`rule.control.frame-references`), D-0283 (point checks at the
call), D-0279 (no untracked build), D-0007 (evaluation order), D-0035 (`[Compound-Assign]`), D-0188 (dual bodies)
Affects: `spec/14` §6 (1.32.0), `spec/conformance.md` (3.275.0), the shared front end (`impl/src/typecheck.rs`,
`impl/src/tier.rs`, `impl/src/interp.rs`), `cobc` (`lower.rs`, `lower/program.rs`, `lower/vec_calls.rs`,
`main.rs`), `cbrt` (`cbrt.h`), `CHG-0375`

## Problem

D-0282 and D-0283 made the conflicts of frame-held references (tier S) a matter for the static pass: refuted
before the run when the text decides them, checked at the call when only values do. What they did not yet
change is the run. `cobc` still gave every reference a token and recorded every borrow, read and statement's
temporaries in the runtime's path tables, and step 1 of D-0281 measured that bookkeeping as most of the cost of
a text-processing program (csvstat: statement scopes 36%, element borrows 28%, borrow checks 7%, reads 5%).

A program where the static pass leaves nothing to the run gains nothing from that record: no access it makes
can be found in conflict. The question is whether the language says so, how a program qualifies, how the claim
is tested, and whether the untracked form is the default.

## Options

- **A. Decided programs run pathless, by default.** The rule names the programs none of whose conflicts are left
  to the run (*decided*), and lets an implementation run them with no record of references. `cobc` does it for
  every decided program; `--all-checks` (and `COBALTC_PATHLESS=0`) keep the record.
- **B. The same, opt-in.** `cobc --pathless`. No program changes behaviour unless asked.
- **C. Per function, now.** Lower tier-S functions untracked inside tracked programs (the §15 mark). The larger
  gain, but it needs the boundary design first (D-0281 §15, step 5).

## Decision

A, with the soundness repairs the gate found folded in.

`rule.control.frame-references` gains a paragraph, **Decided programs**: a program none of whose conflicts this
rule or `rule.control.flow-analysis` leaves to the run is decided, and may run with no record of references,
with the same outcome. Not decided: a function with a conflict left to the run (a may-overlap, a fact `?`, a
binding of unknown origin, a point check at a call), a type that keeps references in data, a closure that
borrows what it captures or captures a reference, a reference to a reclaimed object in the program's own code, a
spawn.

Reasons:

- It is what D-0281 promised: a frame-held reference costs nothing the program did not ask for. Making it opt-in
  (B) would leave the default build paying for checks that cannot fail, which is the criticism D-0281 exists to
  answer.
- It is not D-0279's "untracked build". D-0279 rejected a build that turns checks off; a decided program has no
  check to turn off. The outcome of every decided program is the one the tracked run gives (Verification).
- The bounds, overflow, stack and position checks stay, and so does every check of a value: a resource
  overwrite reads the target's state, not a path, and still faults; a `fn` value's state is still tested.
- Per-function lowering (C) is the next step, not this one; whole programs first give a gate with no boundary.

**Soundness repairs (the gate's findings).** The differential gate and the compiled conformance rows found seven forms where the static pass left a
conflict to the run without recording it, so a pathless build would have run them unchecked. Each now either is
refuted, as the rule already said, or keeps the program's record (it is not decided):

1. a write to a `Vec`'s element was not an element access: `v[k] = e` beside a live reference to `v[j]`;
2. the hidden reference of `[Compound-Assign]` with a call in its target (`w[f()] += w[0]`) was treated as
   ended, because its uses carry the target's position;
3. the evaluation-order rule did not look inside a struct literal, an array literal, a branch or a `let` in the
   assigned value (`v[0] = P { .x = grow(&mut v) }`): now refuted, `diag.stale-binding`, when the value takes the
   indexed container or a place around it exclusively (`h.v[0] = side(&mut h.other)` stays accepted);
4. a later argument that moves what an earlier reference argument reaches (`set(&mut v[0], grow(&mut v))`);
5. a later argument that reassigns a reference an earlier argument read, then destroys its referent.
6. a borrow given to a type parameter (`String::append(&mut s, &s)`, `T` of `append<T>`) was not a reference
   argument of the call: now one, with the borrow's mode, and the pair is refuted;
7. a reference to a reclaimed object (`&mut reclaim<T>(p)` in the program's own `unsafe` code) is checked
   against the run's record of raw storage, which the static pass does not follow: such a program is not
   decided.

Form 4 was also a defect of the tracked build: `cobc`'s dual-body dispatcher (D-0188, H5) skips the callee's
checks on a single-threaded run when its one reference argument is a fresh borrow, and a later argument made
that borrow stale before the call. The tracked build wrote into freed storage. A reference argument is now
fresh only when no later argument contains a call, a block or an assignment.

## Consequences

- `cobc` lowers a decided program pathless: every token is 0, and `cbrt.h` under `CB_PATHLESS` makes the path
  entries constants or no-ops, so the C compiler deletes them and what was computed for them. The generated C
  starts `#define CB_PATHLESS 1`; `-v` says "lowered pathless", or "kept paths" and why, naming the file and
  line of the first conflict left to the run.
- `coby` is unchanged: it keeps the record, so it remains the oracle the gate compares against.
- Libraries (`--lib`) keep the record: C callers are not checked by the pass.
- Conformance: six new rows and one now refuted before the run (`spec/conformance.md` 3.275.0).
- The self-hosted toolchain, frozen at Update 135 (D-0278), stays tracked until it is ported.

## Verification

A differential gate (`stress/d0281/pathless/gate/gate.sh`): each program `cobc` finds decided is built pathless and
run, and its exit status, standard output and standard error must equal `coby`'s (tracked). Where `coby` cannot run
a program (foreign code) or is too slow, the pathless build is compared with the tracked `cobc` build instead
(`recheck.sh`).

- 19,835 hand-written and probe programs (conformance, showcase, the book's and the guide's examples, every stress
  directory, the soundness and verification probes): 9,692 decided; 9,488 the same, 204 different. Of those, 187
  that `coby` could not run (foreign code) or ran past the time limit were rebuilt tracked with `cobc`: 127 the
  same, 60 different only in the times they print. Of the other 17, all but the two below differ only in printed
  times (GCC; run before repairs 4–7).
- Under Clang, the decided programs again: 8,855 of them still decided after repairs 4–5 (837 now keep their
  paths): 8,649 the same, 205 different; 187 against tracked `cobc`: 128 the same, 59 only in printed times. Of
  the other 18, all but four differ only in printed times: the two below, a program that exhausts the stack (same
  fault and status; pathless frames are smaller, so it names another frame), and a benchmark of random input. One
  program fails to build both ways (Clang's limit of 256 nested brackets).
- New static rejections, against Update 138, over the same 19,835 programs: two. One faults at the run under
  Update 138 (`diag.stale-binding`); the other, `showcase/tier3/tinyos.cb`, was a false rejection by repair 3's
  first form (it compared the element's place by its root alone) and is accepted since the repair names the
  indexed container.
- 11,315 generated programs of the model whose outcome is a dynamic aliasing, staleness or destruction fault:
  11,269 not decided, 46 decided, 0 differences.
- The compiled conformance suite pathless under GCC and Clang: 410 of the 1,109 cases are decided; the compiled
  suite and the compiled `spec/conformance.md` rows pass under both compilers. The rows found repairs 6 and 7.

Two programs still differ, neither a hole in this decision: a probe whose two zero-sized elements `v[0]` and `v[1]`
the tracked runtimes report as overlapping (the spec says they do not, `spec/04` `overlap`; the pathless outcome is
the correct one, and the runtimes' false fault is recorded for repair), and a probe whose unsafe block moves a `Vec`
out through a raw pointer and leaves it in place, which both builds then free twice (undefined behaviour of the
program's own unsafe code).

Measured (best of five, GCC): csvstat 217 → 102 ms (942 → 405 million instructions, 2.3×), extsort 670 → 476 ms
(2.80 → 2.05 billion, 1.37×). Objects, frames and statement scopes remain; removing them is the next step.
