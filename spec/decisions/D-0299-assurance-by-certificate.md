# D-0299 — Assurance without re-testing: the compiler's static decisions as a certificate, checked by the model, and the assurance case

Status: ACCEPTED IN PART (2026-10-31, under the standing autonomous mandate for the verification plan,
D-0261/D-0265/D-0270: "go ahead with the verification plan's remaining steps … autonomously"): step 1 of §Decision
implemented as `CHG-0396`, evidence only, no normative change. Steps 2–5 are PROPOSED for the owner's decision,
each with its cost and what it buys.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12 (the safe-program guarantee), §20 (adversarial review), §24
Depends on: D-0206 (the safe-client theorem; the flow analysis defines the language), D-0219 (the model),
D-0241 (the model checks what the text lets an implementation omit), D-0261–D-0265 (the verification plan),
D-0270 (the elision lemmas, the replay), D-0282–D-0284 (frame-held references; decided programs), D-0292 (light
references), D-0278/D-0296 (both toolchains in step)
Affects: `spec/ASSURANCE.md` (new, non-normative), `spec/02-schema.md` §5, `spec/README.md`; `impl/src/certificate.rs`
(new), `impl/src/typecheck.rs` (`tier_report`), `impl/src/main.rs`, `impl/cobc/src/main.rs` (`--certificate`);
`impl/model` (`cobmodel certify`, `Code::decided`, `Fault::tok`/`value`, `State::frame_held`);
`impl/tools/certify_model.py`, `impl/tools/assurance.py` (new); `impl/battery.sh`, `impl/tools/suites.sh`;
`impl/selfhost` (`cbc --certificate`, the same Update, D-0296); `README.md`, `impl/STATUS.md`, `impl/model/README.md`,
the guide's chapter P; `CHG-0396`

## Problem

The goal set on 2026-10-31: *provide users a higher level of assurance that the compiler and the runtime are
correctly enforcing the specification and the language rules, which does not require extensive and ongoing
testing.*

What the project had, sorted by what it is evidence about:

- **The rules.** `spec/23` argues preservation in prose for every rule, kept complete by `speccheck`; the Lean
  development proves it for the alias and temporal fragment (`reachable_invariants`, nine invariants, no
  `sorry`), proves the elision lemmas `[Standing-Local]` rests on, and replays the model's runs through the
  proven machine (D-0265, D-0270). This is proof, but about the text.
- **The tools.** Everything about `coby`, `cobc` and `cbrt` is testing: the conformance suites under both tools
  and both C compilers, differential random programs, trace equivalence, the verify modes (D-0261, a mode that
  turns every test into a test of the bookkeeping), the sanitizer sweeps, the fuzzer, Kani on the runtime's
  smallest tables, and the model's sample compared with the tools (D-0219). All of it is evidence about the
  programs that were run, and it has to be run again after every change: the battery takes hours, and the
  owner has asked for fewer of them.
- **The gap between them** is exactly where a user's trust is thinnest. Since D-0281 a *decided* program
  (`rule.control.frame-references`, D-0284) is compiled with no record of references at all, and a decided
  function in any program forms *light* references that no later access checks (D-0292): the compiler trusts a
  static argument and the run checks nothing. `proofs/README.md` says plainly that the static rules "are not
  mechanised here". The only check of the decided verdict was `cobmodel check --as-licensed` (D-0241), which
  checks the flow table's *proven* outcome, a different and weaker claim, over the sample. The decision
  itself was invisible: `cobc -v` printed one line, and nothing a user could keep or hand to a checker.

So the problem decomposes: (1) make the compiler's static decisions an artifact; (2) check that artifact with
something that did not make it; (3) say, in one place a user can read, what is proved, what is checked, what is
tested and what is trusted, so that "assurance" is a claim with evidence behind each part rather than a green
test run; (4) lay out the road by which the checked becomes the proved.

## Options

1. **More testing, better organised.** A larger corpus, more sanitizer runs, a longer differential campaign.
   Rejected as the answer to this goal: it is what the goal asks to move beyond, and its evidence is about
   the programs run.
2. **A verified compiler.** CompCert's road: the lowering proved correct in a proof assistant. Years for a
   team; `cobc` is 26,000 lines of Rust over a C back end, and the C compiler stays trusted anyway. Rejected
   as the next step; §5 of the assurance case keeps translation validation of the *decided* lowering as the
   furthest item.
3. **A certifying compiler: the decisions written out, an independent checker.** The static pass writes what
   it decided for each body — the elisions a user's binary rests on — and a checker written from the rules
   alone verifies the certificate. For the fragment the model covers, the checker exists: `cobmodel`'s
   machine explores every interleaving, and its statics are an independent derivation of `spec/14` §6. The
   certificate is a per-program artifact, checked at compile time; nothing is re-run. This is proof-carrying
   code's shape, with the model as the first checker and a verified checker as the destination. **Selected**
   for step 1, with the road in steps 2–5.
4. **Mechanise the static rules first.** State `spec/14` §6 over the fragment's programs in Lean and prove the
   discharge sound against the proven machine. The right destination (step 3 below), months of work, and it
   would still say nothing about what `cobc` actually decided for a given program without the certificate of
   option 3 to connect the theorem to the binary.
5. **Replay the runtime's runs through the proven machine.** A per-run certificate of `cbrt`; the format and
   the replay exist for the model's traces (D-0270). Valuable (step 4 below), but it is about one run, and
   about the dynamic checks that are made; the decided program, which makes none, is what the user most needs
   assurance about.

## Decision

**Step 1 (done, `CHG-0396`).**

1. **The certificate.** `cobc --certificate FILE prog.cb` and `coby --certificate FILE (--check prog.cb |
   prog.cb …)` write, once the program is checked (and, for `cobc`, lowered), a text record: the tool, the
   program, the program's verdict (`decided`: no record of references; `paths kept: WHY`; `rejected: DIAG`;
   or, for a static-only run, how many bodies the pass decided), and for every body the static pass checked —
   the program's own first, then `std`'s — `decided` or `left`, each conflict left to the run (every one, not
   the first), each check made when a call is made (D-0283) or of a value, each binding kept in data (tier D)
   or of unknown origin, with its file and line. The static pass appends raw records as it checks each body
   (`typecheck::tier_report`, which D-0281's measurements already read; `Rep::lefts` now keeps every reason)
   and `certificate::finish` gives them their final form. The format is documented in `impl/src/certificate.rs`
   and is version 1. A decided body is one with no conflict left to the run, which is the unit of elision
   (D-0292: such a body's frame-held references are light in any program); the program's verdict is the
   lowering's `pathless` result, so it includes the compiler's own reasons (a type that keeps references in
   data, a library, a point check in the C).
2. **The model's check.** `cobmodel certify [--witness] CERT FILE` reads the certificate, requires the static
   verdicts to agree (a program the certificate calls rejected is one the model's statics reject with the
   same diagnostic, and the other way round), and explores every interleaving of the program with each
   certified body marked decided (`Code::decided`; a closure's body is decided with the function it is written
   in; `verdict: decided` marks every body). A fault of a reference's record — `diag.aliasing-conflict`,
   `diag.stale-binding`, `diag.destroy-while-aliased`, `diag.move-while-aliased` — at a step of a decided body
   is the outcome `model.decided-but-false` (exit status 1), **when the access is through a frame-held path**:
   the path, up through its projections, is a binding's own in the current call, or the reference a binding of
   reference type holds, one dereference away (`State::frame_held`). A reference reached through data (a
   field, an element, an `Option`'s payload), a slice's or a guard's path, and a parameter a thread was lent
   are recorded in every function (D-0292), so a fault there is the rules' own; and a fault that is a check of
   a value's own state (`[Fn-Value-Empty-Call]`, a handle already joined) is one a pathless program still
   makes (`Fault::value`). With `verdict: decided` every path counts: the program keeps no record at all.
   `impl/tools/certify_model.py DIR --check` runs `cobc --certificate` (through `--emit-c`, so nothing is
   compiled) and `cobmodel certify` over a directory; battery stage 1 and `suites.sh` run it over the sample
   after `--as-licensed`. Three unit tests in `impl/model/src/explore.rs` pin the three cases (a false claim
   refuted; an access through data in a decided body left to the run; a value-state check not counted).
3. **The assurance case.** `spec/ASSURANCE.md`, a non-normative artifact beside `STABILITY.md`: the claim; the
   seven kinds of evidence, strongest first (proof, exhaustive, certificate, independent derivation, checked
   table, tested, trusted), with what each establishes; the six obligations a user's trust decomposes into
   (the rules preserve the invariants; the static rules accept exactly the language's programs; a static
   discharge removes only checks that cannot fail; the lowering preserves meaning; the runtime refines the
   rules; the trusted library base makes only true claims) and what is not checked at all, each with its
   evidence, kind, place and what remains; what a user can check about their own program without the test
   suites; the road; how to read an edition's tier. `impl/tools/assurance.py --check` (battery stage 1) fails
   when a file, record, option, variable, subcommand or name the page cites no longer exists.
4. **Both toolchains.** `cbc --certificate` writes the same certificate (`impl/selfhost`, D-0296: a change to
   the static pass's outputs lands in both in the same Update).

**Steps 2–5 (proposed; the assurance case §5 states them for the reader).**

2. **A checker of the certificate for the whole language** — the certificate grows to carry the static pass's
   fixed point (the facts at every program point), and a checker written from `spec/14` §6's tables alone
   verifies each transfer row and each discharge locally, with no fixed-point iteration of its own; checking a
   solution is simpler than finding one. Cost: the checker is a second reading of the two rules over the
   resolved program (`--dump-folded` gives the canonical text), of the order of the model's statics for the
   fragment but over the whole language — weeks, in Python or Rust, outside the Rust workspace's build. Buys:
   O3 of the assurance case moves from *tested* to *certificate* for every program, and every disagreement is
   a finding about one of the two readings or the text, as the model's were.
3. **The static rules mechanised for the fragment** — the fragment's programs in Lean, the facts as an
   abstraction of the machine's state, each transfer row a lemma, and the theorems that a *proven* access
   passes `¬clash` in every reachable state and that a decided body's frame-held paths meet no conflicting
   path in any run; the checker of step 2 proved against it for the fragment, its unproved rows listed as
   `proofs/COVERAGE.md` lists the fragment's gaps. Cost: months; the development would be of the size of the
   present one. Buys: O3 at *proof* for the fragment, which is the part of the language where every soundness
   hole so far was found, and a checker whose trusted core shrinks with each row proved.
4. **The runtime's runs replayed through the proven machine** — `cbrt` writes a run's steps in the replay's
   action format under a tracing variable, as `cobmodel trace` does, so a user's compiled program built with
   every check is replayed through `proofs`' executable machine: every step admitted, the invariants kept.
   Cost: a session for the format (identities mapped as the machine allocates them, the stops where the
   runtime leaves the fragment) and a battery step over the conformance cases. Buys: O5 gains a per-run
   certificate where it has a table and a verify mode.
5. **Translation validation of the decided lowering** — the property the lowering asserts of itself (a decided
   program's C calls no tracking entry point, `pathless`) stated as a property of the C that a reader of the C
   can check with a tool, and extended to the shape of each access. Cost: unknown; the furthest item.

## Results

- The sample (`impl/model/sample`, 315 programs) under `certify_model.py`: every program certified or rejected by
  both; of the user-written bodies the static pass checked, most are decided and a third of the programs are
  decided whole. The run takes about three minutes.
- The first run reported three programs not certified, each investigated and none a defect of the tools:
  `hand-o02` (a `fn` value called after its closure was moved out: a value-state check a pathless program still
  makes; `cobc` faults there), and `three-p001018`/`three-p001019` (`bump_hold(Hold h)` decided, its access
  `*h.r` through a reference kept in a parameter's field: recorded in every function by D-0292, and `cobc`
  does fault there under more schedules than `--explore 8` tries). The check was refined as item 2 says:
  what the certificate's *decided* promises is unrecorded frame-held paths, not every access in the body, and
  the model now states that distinction (`frame_held`). This is itself a result: the meaning of *decided* at
  the level of an access is now written down in an executable form that a reader can compare with D-0292's
  prose.
- A forged certificate (a program's `left` bodies rewritten as `decided`) is refuted by `cobmodel certify`
  with `model.decided-but-false` at the line of the conflict; the unit tests keep this.
- `cobc --certificate` on showcases: the verdict is the one `-v` reports (`decided` where it says "lowered pathless
  and decided"), and the record lists every body behind it.

## Revisit conditions

- A `model.decided-but-false` in the battery: a decided body with an unrecorded path the run can find in
  conflict — a soundness finding against `rule.control.frame-references`, its implementation, or the model's
  reading of D-0292; settled as D-0219's findings were, with a case.
- A disagreement of the static verdicts in `certify`: the model's statics and the tools' differ on a program
  — a finding about the text or one implementation.
- When step 2's checker exists, the certificate's version advances to carry the fixed point, and this record
  gains the checker's results; when step 3's development exists, `spec/ASSURANCE.md` O3 moves to *proof* for
  the fragment and `proofs/README.md`'s sentence about the static rules changes.
- If the owner declines steps 2–5, the assurance case's §5 is rewritten to say so, and O3's "what remains"
  stands as the project's position.
