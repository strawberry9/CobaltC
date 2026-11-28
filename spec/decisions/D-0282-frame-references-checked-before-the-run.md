# D-0282 — Frame-held references: their conflicts are found before the run

Status: ACCEPTED (2026-10-25, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions"). Phase 2 of D-0281, first part.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0281 (the cost model: tier S and tier D), D-0007 (evaluation order), D-0011 and D-0088 (a result
borrows from its first reference argument), D-0018 (conflicts checked at use), D-0111 (last use ends a binding),
D-0135 (views of temporaries), D-0206 (the analysis defines the language)
Affects: `spec/14` §6 (`rule.control.frame-references`), `spec/05`, `spec/07`, `spec/08` (citations),
`spec/conformance.md` (38 rows static, 4 new dynamic rows), the shared front end (`impl/src/typecheck.rs`,
`impl/src/tier.rs`), the book's chapter 7, `CHG-0373`

## Problem

Until now the static pass refuted a conflict only when one function body showed it. Every conflict that passed
through a call (`String::append(&mut s, String::as_view(&s))`, a helper given `&mut x` and `&x`, a reference
passed on while a reborrow of it lives) was left to the run, and so were references the pass gave no fact (a
reference copied from another, one returned by a call). A conflict the program text decides was a run-time
fault, and the runtime had to keep the state to find it. D-0281 measured that state as two-thirds to
three-quarters of the instructions of the programs it measured, almost all of it for references a frame holds
(tier S).

D-0281's step 3 built the static rules a tier-S checker needs in the shared front end and tested them in
report mode, rejecting nothing: of the 42 tier-S fault cases in the conformance suite, 36 are decided by the
program text and 6 need a test of values (an index, a range, a value's state); over the corpus (the positive
conformance cases, `std`, the showcase, the examples and the self-hosted tools: 15,709 function bodies,
265,000 conflict checks) the rules rejected no correct program and left no check undecided.

## Decision

The rules are normative (`rule.control.frame-references`, `spec/14` §6) and on in both tools. A conflict among
frame-held references that the text decides is a compile-time error:

- **The call-site rule with access summaries.** Each function has, for each reference parameter, the field
  paths it may read and may write, computed over the whole program to a fixed point. A call is rejected when
  what the callee does through an argument meets a live reference of the caller, or meets another argument's
  referent. Because the rule uses what the callee does, not only the parameter's type, CobaltC keeps its
  use-time semantics (D-0018): passing `g` exclusively to a function that writes only `g.types` beside a live
  view of `g.prog` is accepted.
- **Inherited facts.** A reference formed from another reference, from a field or element of one, from a call on
  one, or from an `if`/`match` is rooted where its source is.
- **Liveness by last use.** A reference not mentioned after a point (nor in an enclosing loop it precedes) has
  ended there.
- **Three always-errors:** an element place found before a value that borrows its container exclusively
  (evaluation order, `diag.stale-binding`); a value moved out through a reference (`diag.move-while-aliased`);
  a view of a temporary used after its statement (`diag.destroy-while-aliased`).

Overlap that depends on values (two computed indexes, a computed range) is left to the run, as before. Tier D
(references kept in data) is untouched: the run checks it.

## Consequences

- 38 conformance cases that faulted at run time are rejected before the run; four new cases keep the run-time
  forms of the mechanisms some of them tested (unwinding after a fault, a `Vec`'s growth faulting, a dual
  body's entry test, overlapping slices), with the overlap made to depend on values.
- The diagnostic names the callee and what it does: "`set` writes `*r` through argument 1 while `s` still
  borrows `*r`".
- The rule is not modular in the callee's body: a change in a function's body can change what its callers may
  hold across a call to it. The whole program is checked together already; a summary is part of an exported
  function's interface under `--lib` when that is needed (not yet).
- Nothing at run time changes in this step: the runtime still tracks every reference, and still checks what
  the static pass proved. Removing tracking for tier S is the second part of phase 2, which needs the
  compiler to place destruction itself (D-0281 §18).
- The self-hosted toolchain is frozen (D-0278) and does not apply the rule until it is ported at the phase
  boundary; until then it accepts the 38 programs and faults at run time, as every implementation did before.

## Verification

- The conformance suites of both tools, with the 38 cases and five inline rows moved to static.
- A differential check against Update 136's `coby` over 11,434 hand-written programs (the conformance cases,
  the showcase, the examples, the book, the guide, and the stress programs written over the project's life):
  every program rejected now and accepted then was run under Update 136. The first run found six correct
  programs rejected (from the round-6 programs and the model's generated programs), by five causes, all fixed
  in the rule as stated: a fact that holds on some paths only, one reference passed twice, liveness by name
  rather than by scope, a `foreach` over a map, and a reference read out of a vector of references. The other
  103 new rejections all faulted at run time under Update 136.
- The 30,837 generated programs whose outcome the model (`impl/model`) gives as "runs without a fault" are all
  accepted.

## Alternatives considered

- **Borrow-time checking (Rust's rule).** Simpler and modular, but it rejects correct programs CobaltC accepts
  (twelve functions in `cbc` alone in step 2's run), and changes the language's model.
- **Signatures only (an exclusive parameter reaches its whole referent).** Modular, and rejected the same
  twelve functions.
- **Leaving them dynamic.** The runtime then has to keep tier-S state, which is the cost D-0281 removes.
