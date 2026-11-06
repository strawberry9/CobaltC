# D-0300 — Steps 2–5 of D-0299: the certificate read again, the static rules proved for a core, the runtime's runs replayed, the decided lowering validated in its C; and the two holes they found

Status: ACCEPTED (2026-10-31, the owner: "proceed with steps 2-5 of D-0299"). The soundness repairs are
`CHG-0397` (normative: `spec/14` 1.43.0, `spec/conformance.md` 3.287.0); the evidence is `CHG-0398`
(non-normative: tools, the runtime's trace, the Lean development, the assurance case).
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12 (the safe-program guarantee), §20 (adversarial review), §24
Depends on: D-0299 (the certificate; steps 2–5 as proposed), D-0206 (the flow analysis defines the language),
D-0219 (the model), D-0241, D-0265, D-0270 (the mechanised fragment, the replay), D-0282–D-0284 (frame-held
references; decided programs), D-0291 (handing over), D-0292 (light references), D-0296 (both toolchains in step)
Affects: `spec/14-control-flow.md` §6 (1.43.0), `spec/conformance.md` (3.287.0), `spec/ASSURANCE.md` (1.1.0);
`impl/src/typecheck.rs` (`set_deriv`, `set_deriv_tri`, `hand_over_referent`, `check_while`, `Flow::truncated`);
`impl/selfhost/front/typecheck.cb` (the same, D-0296); `impl/cbrt/src/replay.rs` (new), `impl/cbrt/src/lib.rs`
(the trace's hooks); `impl/tools/factcheck.py`, `impl/tools/validate_c.py`, `impl/tools/replay_runtime.py`
(new); `proofs/` (`Cobalt/Statics.lean` new; `dead` in `Step.lean`, `Preservation.lean`, `Elision.lean`,
`Exec.lean`, `Replay.lean`; `COVERAGE.md`, `README.md`, `lakefile.lean`); `impl/battery.sh`,
`impl/tools/suites.sh`; `impl/conformance/14-control-flow/ref_copy_*_rejected.cb`; `CHG-0397`, `CHG-0398`

## Problem

D-0299 made the compiler's static decisions an artifact (the certificate) and checked it with the model for
the fragment the model covers, and proposed four further steps, each with its cost: a checker of the
certificate for the whole language (2), the static rules mechanised for the fragment (3), the runtime's runs
replayed through the proven machine (4), translation validation of the decided lowering (5). The owner asked
for all four. Each is large as D-0299 stated it (weeks to months for 2 and 3); the question is what each first
increment is, so that it is real evidence and says plainly where it stops.

## Decision

Each step is done as a first increment that runs, is checked by the battery, and names its limits in
`spec/ASSURANCE.md`.

**Step 2 — the certificate read again** (`impl/tools/factcheck.py`). A second reading of
`rule.control.flow-analysis` and `rule.control.frame-references`, written from `spec/14` §6, over the
program's canonical syntax (`coby --dump-ast`), sharing no code with the static pass and keeping its own names
for bindings. For each body the certificate calls decided it derives the body's facts itself and examines
every access, borrow, call (the call-site rule with access summaries to a fixed point; `std`'s `{ε}`) and
block end; a body the certificate calls decided where this reading leaves a conflict to the run, or the rules
refute one, is `factcheck.decided-but-left` / `-refuted`. It reads a core of the language and reports any
other body `unchecked` with the first construct outside it. One departure from D-0299's design: the checker
re-derives the facts rather than reading a fixed point from the certificate. Reading them would have meant a
new certificate version in both toolchains, and the first finding below is a fixed point the static pass
stopped short of: a checker that took the pass's facts on trust would have checked against the wrong ones.
The certificate's format is unchanged (version 1).

**Step 3 — the static rules proved for a core** (`proofs/Cobalt/Statics.lean`). The facts `deriv(r, x.π, m)` as
an abstraction of the machine's state (`Describes`: every valid, non-pending path on an owner's object other
than its root is a reference binding's, and a fact says so with its projection and mode); the discharge's
proven row as `proven_no_clash` (an access no fact meets in a mode it does not permit passes `¬clash`); one
lemma per transfer row of the core (a read, a write, a reference's end, a borrow of an owner and of its field,
and weakening for a merge or a loop's post-fixed point); and `run_sound` over a run of the core's commands.
`[Ref-Binding-Last-Use]` needs `[Object-End]`'s `Dead` reading, which the fragment did not take (D-0270 listed
it as the next milestone): it is now the fragment's step `dead`, performed by the path's own thread, with no
live holder, not pending and not its owner's root, and all nine invariants, the elision lemmas and
`exec_sound` hold over it. The replay takes a trace's `inv` as that step where its premises hold.

**Step 4 — the runtime's runs replayed** (`impl/cbrt/src/replay.rs`, `impl/tools/replay_runtime.py`). Under
`COBALTC_REPLAY_TRACE` a program built with every check (`--all-checks`, `COBALTC_PATHLESS=0`) and recorded in
full (`COBALTC_RECORD=full`) writes its run in the replay's action format, and `proofs`' `replay` checks and
performs each step. A fault of the alias and temporal rules is written `flt DIAG` before the action the
runtime refused, and the machine must refuse it too (`faulted as the rules say`; `MISSED` otherwise). The
runtime's identities are the trace's; a borrow is written pending until something holds it (the runtime
cannot tell a call's argument from any other borrow when it forms it), a frame's hold or a parameter is a
holder object (`bpa`), a slot's reference is the write that stores it, and the trace opens a statement scope
of its own around an object the runtime establishes outside every statement (`cobc` makes a whole-statement
call with none). Threads, mutexes, raw storage, vectors' elements, closures, slices and destructors end the
trace where the run reaches them (`out`); this increment covers one thread.

**Step 5 — the decided lowering validated in its C** (`impl/tools/validate_c.py`). On the C `cobc` writes, after
the C preprocessor (with GCC's and with Clang's), against a classification of every entry point `cbrt.h`
declares (an unclassified one is an error): the verdict and the C agree (`CB_PATHLESS`); a decided program's
own C names no entry point that forms, records or reads an access path; every body the certificate leaves to
the run records its references (`cb_frame_push_rec`) or is a dispatcher to its own variants. With `--deep` it
compiles the C and attributes each check that survives optimisation in a decided program to the source line
it was written for. The validation is of the record of references, not of each access's meaning.

**The repairs** (`CHG-0397`). The second reading's first test found two programs the static pass decided and
the rules do not, and `cobmodel certify` refuted both certificates (`model.decided-but-false`):

1. `r1 = r2; r2 = &x;` in a loop. A copy `w = q` of a reference holds a fact through `q`'s referent (`*q`); an
   assignment to `q` cleared every fact rooted at `*q` (the row for `q` assigned) without handing it over, so
   `w`'s fact said nothing, and on the second pass `w` reached `x` unseen. Handing over (D-0291) now applies when
   a reference binding is assigned, as when it ends, each fact handed over `?`: what the old referent was lent
   to or copied into may have let it go already (a thread that finished), so the run decides the access, and
   no access the text leaves to the run is refuted. (Handed over as `T`, the first version of the repair
   refuted four programs of the model's sample, `three-p000078` among them, that the model accepts.)
   `spec/14` §6 says so, and both static passes do it.
2. A chain `r1 = r2; …; r19 = r20;` carries a fact one copy a round; the loop's iteration stopped after 16
   rounds whether or not the head had settled, and the body was walked from a head that was not a fixed point.
   It now iterates until the head stops moving: after 16 rounds the head is joined with what the body gives it
   (it only grows) and the facts' paths are cut at eight steps (a `?` on the prefix, which overlaps every
   place under it), so it stops at a post-fixed point; the text's "least fixed point" is what it computes
   where no path grows. A loop that has not settled after 4,096 rounds is an internal error, never an
   acceptance. A path handed over is cut at eight steps likewise (without the cut, a walk down a structure in
   a loop doubled its facts each round).

Both programs, compiled by the earlier `cobc`, ran with no record of references and wrote `x` while `r1`
reached it; `coby` faulted as the rules say. Both are conformance cases now
(`conf.ref-copy-source-reassigned-in-loop`, `conf.ref-copy-chain-longer-than-loop-rounds`). No program the
text accepted is rejected by the repairs.

## Results

`CHG-0398` §Verification has every count. In short: the second reading agrees with the certificate on all 372
bodies it can read (531 more are outside its core); the decided C of 206 programs validates, as do the 118
undecided; the runtime's runs of the sample replay with no refusal and no miss, 52 whole and 2 faulting as the
rules say, most of the rest stopping at threads; the Lean development builds with no `sorry`, and taking
`Dead` as a step cut the model replay's gaps from 219 to 64. The two holes are repaired with no change to any
program's static verdict (1,570 compared) and none to the decided set (the undecided sweep: 8,825 of 8,854, as
before).

## Revisit conditions

- A `factcheck.decided-but-left` (or `-refuted`): one of the two readings, or the text, is wrong; settled as the
  two holes above were, with a case.
- A `validate_c` finding: the lowering wrote C its own verdict does not license.
- A `REFUSED` or `MISSED` in the runtime's replay: a step of the runtime the rules do not admit, or a fault the
  rules do not make; or a gap of the trace's encoding, settled by repairing the encoding.
- The cores grow: the second reading's (closures, threads, `match`, reference data), the Lean development's
  (reborrows, copies, calls, threads), the runtime trace's (threads first); each growth is recorded here and in
  `spec/ASSURANCE.md`.
