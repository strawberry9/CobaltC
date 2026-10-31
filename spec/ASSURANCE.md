# CobaltC Assurance Case

Status: non-normative artifact (like `STABILITY.md`: it carries no rule,
invariant, type or state entity). It says what evidence the project keeps
that the rules, the compiler and the runtime do what the specification
says, what kind of evidence each piece is, what each establishes and what
it leaves trusted. Decided by D-0299; kept current by
`impl/tools/assurance.py --check`, which fails when a file, a theorem, a
tool option or a record this page names no longer exists.
Governed by: `CobaltC_Master_Instructions.md` §12 (the safe-program
guarantee), §20 (completeness and adversarial review)

## 1. The claim

A *safe program* — a program with no `unsafe { }` block of its own, run on
a conforming implementation — never reaches a state that breaks an
invariant of `03-invariants.md`: every access reaches a live object through
a valid path, no two live paths conflict, every resource has one authority,
no arithmetic result leaves its type's range, nothing crosses the trust
boundary unvalidated (`term.safe-program`, D-0206; `OVERVIEW.md` §10 puts
the theorem and its three hypotheses in plain words). A reader who runs
`cobc prog.cb` and trusts that claim is trusting a chain of six things,
listed in §3. This page says, for each, what the project has done to
deserve that trust and what it has not.

The question this page answers is the one a user cannot answer by running
the test suites: *when the compiler leaves a check out, how do I know it
could not have failed?* Testing shows the programs that were run; the
evidence below is ranked by how far it reaches beyond them.

## 2. Kinds of evidence

From strongest to weakest. Each row of §3 names one of these words.

| Kind | What it is | What it establishes |
|---|---|---|
| **proof** | A theorem checked by a proof assistant (`proofs/`, Lean 4, `lake build` with no `sorry`) | The statement, for every program and every run within the statement's scope; nothing is re-run |
| **exhaustive** | Every interleaving of a given program enumerated by the executable model (`impl/model`), written from the rules alone | For that program, the whole set of outcomes the rules allow; over a corpus, that the tools' outcomes lie in it |
| **certificate** | A record the compiler writes of what it decided about one program (`--certificate`), checked by something that did not make the decision | For that program, that each elision the compiler made is one the checker's reading of the rules also admits |
| **independent derivation** | A second implementation of the same rules, written from the text and sharing no code, compared with the first | Where the two agree, the agreement is evidence about the text; where they disagree, one of them, or the text, is wrong |
| **checked table** | A correspondence between the text and the code kept by a tool that fails when they drift (`impl/tools/speccheck.py`, `impl/tools/proofcover.py`, `impl/tools/runtime_contract.py`, `impl/tools/trusted_table.py`, this page's checker) | That the argument in the text covers every rule, entry point or block the code has, and names each; not that the argument is right |
| **tested** | Conformance cases, differential runs, verify modes, sanitizers, fuzzing, the model's sample under the tools | The programs and schedules that were run, as run |
| **trusted** | Not checked by the project | Nothing; named so that the reader knows what rests on it |

A *port* — the same algorithm written again in another language, as the
toolchain in CobaltC (`impl/selfhost`) is of the Rust one — is compared byte
for byte and is strong evidence that the port is faithful, but it is not
an independent derivation: both can be wrong together.

## 3. The obligations

Each obligation names what must hold, the evidence, its kind, where the
reader finds it, and what remains. The record numbers are the decision
records (`decisions/`) that introduced each piece.

### O1. The rules preserve the invariants

*What must hold.* Every rule of `05`–`21`, taking a state that satisfies
`03`'s invariants to one that does.

| Evidence | Kind | Where |
|---|---|---|
| The preservation lemmas: for each invariant and each rule that writes a component it ranges over, why it survives; `impl/tools/speccheck.py` reports a writing rule with no row (D-0262) | checked table | `23-preservation.md`, `impl/tools/speccheck.py` |
| The mechanised fragment: the abstract machine of the alias and temporal rules stated in Lean, and every reachable state proved to satisfy nine invariants (`reachable_invariants`), with its corollaries (`one_lock_path`, `bound_parameter_clashes`, `sub_obligation_owned`) and an executable form proved to take only admitted steps (`exec_sound`) (D-0265, D-0270) | proof | `proofs/Cobalt/Preservation.lean`, `proofs/Cobalt/Corollaries.lean`, `proofs/Cobalt/Exec.lean`, `proofs/README.md` |
| Which rule rows of `23` the fragment has as steps and which it leaves outside, generated and checked (D-0270) | checked table | `proofs/COVERAGE.md`, `impl/tools/proofcover.py` |
| The model's runs replayed through the proven machine: every action the model takes is one the machine admits, or the replay stops where the model leaves the fragment (D-0270) | exhaustive + proof | `proofs/Replay.lean`, `impl/tools/replay_model.py` |

*What remains.* The rules outside the fragment — channels, atomics, cells
and addresses, `[Match]` as a whole, foreign calls — are argued in prose
only. The fragment reads two rules more loosely than the text
(`proofs/README.md`, "two readings"). Nothing here is about the tools.

### O2. The static rules accept exactly the language's programs

*What must hold.* `coby --check` and `cobc --check` reject a program if and
only if `rule.control.flow-analysis`, `rule.control.frame-references` and
the other static rules reject it, with the diagnostic the registry names
(`14-control-flow.md` §6; `term.conformance`).

| Evidence | Kind | Where |
|---|---|---|
| The reference checker is the executable form of the rule by definition (D-0206): a third implementation is measured against it on any program | — | `IMPLEMENTATION-NOTES.md` §4 |
| The model's statics, written from the text for the fragment, compared with the tools' verdict on the sample and on enumerated programs (D-0219, D-0282) | independent derivation | `impl/model/src/statics.rs`, `impl/model/src/frames.rs`, `cobmodel compare` |
| The static conformance rows, each a program with the rejection the text requires, under both tools | tested | `conformance.md`, `impl/conformance/` |
| The front end written in CobaltC, compared with the Rust one on every tracked program and on mutated copies | port | `impl/selfhost/compare_parser.sh` |

*What remains.* There is no mechanised statement of the static rules: the
flow analysis and the frame-reference rule exist as text, as two
implementations and as the model's third, for the fragment.

### O3. A static discharge removes only checks that cannot fail

*What must hold.* Where the compiler leaves a check out — an access the
flow table *proves*, a function the frame-reference rule *decides* so that
no record of its frame-held references is kept (D-0284), a reference made
*light* (D-0292), a parameter accessed directly under `[Standing-Local]`
(D-0188, D-0209) — no run of the program could have failed that check.
This is the one place where the tools trust a static argument and the run
checks nothing, and it is where the independent reviews found their
soundness holes.

| Evidence | Kind | Where |
|---|---|---|
| The elision lemmas: a standing path keeps its validity, object, projection and mode across any step of another thread, and is a `clash` witness against every conflicting access (`standing_step`, `standing_run`, `conflict_diagnosed`); an object no valid path reaches is reached by no other thread (`unreached_stays`) (D-0270) | proof | `proofs/Cobalt/Elision.lean`, `08-alias-validity.md` §4 |
| The certificate: `cobc --certificate FILE prog.cb` writes, for every body the static pass checked, `decided` or each conflict left to the run, the checks made when a call is made, and the program's verdict (D-0299) | certificate | `impl/src/certificate.rs`, `impl/STATUS.md` |
| The certificate checked against every interleaving: `cobmodel certify FILE prog.cb` runs a fragment program with the certified bodies kept unrecorded, as the compiler keeps them, and reports an alias or temporal fault at a frame-held path there as `model.decided-but-false`; `impl/tools/certify_model.py` does this for every program of the model's sample (D-0299) | certificate + exhaustive | `impl/model/src/explore.rs`, `impl/tools/certify_model.py` |
| What the flow table proves checked against every interleaving: an access the table proves clash-free that `clash` refutes all the same is `model.proven-but-false` (D-0241) | exhaustive | `cobmodel check --as-licensed` |
| The cost cases: conformance programs whose compiled form must keep no record of references, and the undecided sweep over the real programs (D-0281, D-0291) | tested | `conformance.md` (`decided: yes`), `impl/cobc/tests/tracking.rs` |
| Every elided form against its checked form: `cobc --all-checks`, `COBALTC_RECORD=full` and `COBALTC_RECORD=check` builds compared with the default on the suites, the showcases and the stress programs (D-0209, D-0292) | tested | `impl/battery.sh`, `impl/tools/suites.sh` |

*What remains.* The discharge table's *proven* outcome and the *decided*
verdict are not mechanised: that the static facts imply the dynamic
premise is argued in `14-control-flow.md` §6 and `OVERVIEW.md` §5, checked
by the model for the fragment, and tested beyond it. Outside the fragment a
certificate is a record the reader can audit, not one a checker verifies.
§5 is the road from here.

### O4. The lowering preserves the program's meaning

*What must hold.* The C `cobc` writes, compiled by a C compiler against the
runtime, has every outcome the rules admit the program and no other: the
same output, the same fault at the same line, the same destructor order,
the same exit status.

| Evidence | Kind | Where |
|---|---|---|
| The conformance suites under `cobc` with GCC and with Clang, byte for byte against `coby`, with and without `--all-checks` | tested | `impl/cobc/tests/compiled_suite.rs`, `impl/cobc/tests/compiled_spec_rows.rs` |
| Random well-typed programs run by `coby` and compiled by `cobc` with each C compiler, all three agreeing on status and both streams | tested | `impl/cobc/tests/differential.rs` |
| Trace equivalence: the destructors run, the fault, the exit and the end, as the interpreter and the compiled program report them under `COBALTC_TRACE_EVENTS=1` (D-0264) | tested | `impl/tools/trace_compare.py` |
| The sanitizer sweeps and the fuzzer under ASan and UBSan: no undefined behaviour of the generated C's own | tested | `impl/fuzz/`, `impl/battery.sh` |
| The compiler written in CobaltC writes the same C byte for byte, and compiles itself to a fixed point (D-0276, D-0296) | port | `impl/selfhost/compare_c.sh`, `impl/selfhost/fixpoint.sh` |

*What remains.* No translation validation: nothing checks one compiled
program's C against the rules except by running it.

### O5. The runtime refines the rules

*What must hold.* `cbrt`'s tables represent the abstract state `Σ` of
`04-abstract-state.md`, and each entry point the generated C calls takes
exactly the rule steps its contract names, over real addresses.

| Evidence | Kind | Where |
|---|---|---|
| The contract: every `cb_*` entry point's comment names the rule steps it performs and what it assumes; the table is generated into the implementation notes and a checking group's entry with no rule label fails (D-0263) | checked table | `impl/cbrt/include/cbrt.h`, `IMPLEMENTATION-NOTES.md` §6, `impl/tools/runtime_contract.py` |
| The verify mode: at every Nth statement's end the runtime re-derives, from its whole state, that its path and object tables agree with each other; the interpreter re-derives its holder index from a scan of every live object (D-0261); both suites run under it | tested | `impl/cbrt/src/lib.rs` (`verify_state`), `impl/src/interp/verify.rs` |
| Bounded proofs of the smallest tables: ids stable until removed, sets exact, places filled before read, the path fold streamed as built, overlap symmetric (Kani, `cargo kani -p cbrt`) | proof (bounded) | `impl/cbrt/src/lib.rs` (`kani_proofs`) |
| ThreadSanitizer on the instrumented runtime and Miri on its unit tests, run in the verification campaign that introduced the verify modes (two races found and fixed); not battery steps | tested | D-0261 |
| The same hand-written tests and suites with the compiler written in CobaltC as the compiler under test | port | `impl/selfhost/build.sh` |

*What remains.* No refinement proof. The replay of O1 reads the model's
traces, not the runtime's: a user's compiled program is not yet run
through the proven machine.

### O6. The trusted library base makes only true claims

*What must hold.* Every `unsafe { }` block in `std` claims a side-condition
of a trusted rule, and the claim is true (`term.trusted-library-base`,
D-0206): a safe client reaches no false claim.

| Evidence | Kind | Where |
|---|---|---|
| The trusted table: every `unsafe` block covered by a `// trusted:` comment naming its rule and why its side-conditions hold, generated into `21` §0 and checked (D-0206) | checked table | `impl/tools/trusted_table.py`, `21-standard-library-semantics.md` §0 |
| The safe-client traces: conformance rows that derive, step by step, which trusted side-conditions a safe program reaches and the fact in the table that discharges each (D-0206) | checked table | `conformance.md` |
| The parsers of the security-relevant modules fuzzed under the sanitizers, and the cryptography checked against its standards' vectors and against OpenSSL | tested | `impl/fuzz/` |

*What remains.* Each comment is an argument, not a proof; the modules are
not independently audited (`21` §0 says so).

### O7. What the project does not check

The C compiler, the operating system and the hardware are **trusted** in
both tools. A program with an `unsafe` block of its own that makes a false
claim voids the guarantee from that step on (`[Trusted-Claim-False]`).

## 4. What a user can check about their own program

None of these needs the project's test suites; each is a command on the
user's program, and says what it does and does not establish.

| Command | Establishes | Does not establish |
|---|---|---|
| `coby --check prog.cb`, `cobc --check prog.cb` | The program is accepted, or rejected with the language's diagnostic, exactly as `14` §6 defines (O2) | Anything about the run |
| `cobc --certificate cert.txt prog.cb` | The record of every static decision the compiler acted on: which bodies keep no record of references, which conflicts the run checks, where a call is checked as it is made, and why paths were kept if they were (O3) | That the decisions are right: the record is for audit, and for the checkers below |
| `cobmodel certify cert.txt prog.cb` | For a program in the fragment `impl/model` covers: over every interleaving, no alias or temporal fault at a path the certificate says is unrecorded, and the static verdicts agree (O3) | Anything outside the fragment (the command says where the model stops) |
| `cobc --report-tracking prog.cb` | Which lines of the compiled program still call the runtime's tracking, loops first | That the remaining checks pass |
| `cobc --all-checks prog.cb` | A build with every check kept, to compare with the default build's behaviour (O3, O4) | The comparison itself, beyond the runs made |
| `COBALTC_VERIFY=1 coby prog.cb`, `COBALTC_VERIFY=1 ./prog` | On that run, the tools' own bookkeeping agreed with their whole state at every check (O5) | Other runs; the rules themselves |
| `cobc --explore N prog.cb`, `coby --explore N prog.cb` | The distinct outcomes of N schedules of a threaded program, each with a seed that replays it | That there is no other outcome (the model does that, for the fragment) |
| `cd proofs && lake build`; `python3 impl/tools/proofcover.py --check` | The theorems of O1 and O3 check, with no `sorry`, and cover the rows they claim | Anything about the tools |

## 5. From checked to proved: the road (D-0299)

The weakest link a user depends on is O3: the compiler's static decisions,
checked by the model for a fragment and tested beyond it. The road moves it
to *certificate* for the whole language and to *proof* for the fragment,
and then does the same for O5.

1. **Done.** The certificate as a record of the decision (`--certificate`);
   the model's check of it against every interleaving (`cobmodel certify`,
   in the battery over the sample); this page.
2. **A checker of the certificate for the whole language.** The certificate
   grows to carry the static pass's fixed point — the facts at every program
   point — so that a checker verifies each transfer row and each discharge
   locally, against the tables of `14` §6 as written, with no fixed-point
   iteration, no loop handling and no liveness computation of its own:
   checking a solution is simpler than finding one. Written from the text,
   sharing no code with the static pass, run by the compiler on request. A program
   whose certificate the checker refuses is one the two readings of the
   rules disagree on, which is a finding about one of them.
3. **The static rules mechanised for the fragment.** The fragment's programs
   (statements, the syntactic CFG) stated in Lean; the facts of `14` §6 as
   an abstraction of the machine's state; each transfer row a lemma that the
   abstraction is kept; the theorem that an access the table *proves*
   passes `¬clash` in every reachable state, and that a *decided* body's
   frame-held paths meet no conflicting path in any run. The checker of
   step 2 is then proved against this development for the fragment, or
   extracted from it, and its unproved rows are listed as `proofs/COVERAGE.md`
   lists the fragment's gaps.
4. **The runtime's runs replayed.** `cbrt` writes a run's steps in the
   replay's action format, as the model does, so that a user's compiled
   program, built with every check, is replayed through the proven machine:
   every step it took was one the rules admit, and the invariants held
   throughout. A per-run certificate of O5, where today there is a table and
   a verify mode.
5. **Translation validation of the decided lowering** (O4), the furthest
   item: a decided program's C has no path to a runtime check, which the
   lowering already asserts of itself (`pathless`), stated as a property of
   the C that a reader of the C can check.

What each step costs, and what it buys, is in D-0299.

## 6. Reading an edition's evidence tier

An edition's row in `README.md` carries a word — *reviewed*, *tested*,
*battery*, *explored to N*, *modelled* — that says what was run on it
before it was published. Those words are about the project's corpus and
are evidence of the *tested* and *exhaustive* kinds. The certificate of §4
is about one program, the user's, and is independent of editions: it is
what a user can hold in hand when the corpus is not theirs.

## Change Log

- 1.0.0 — D-0299 (`CHG-0396`): the page.
