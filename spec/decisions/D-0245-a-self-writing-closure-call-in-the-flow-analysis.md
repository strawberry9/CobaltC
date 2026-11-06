# D-0245 — A self-writing closure's call is an exclusive access in the flow analysis

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); `spec/14` §6 and `spec/15` §6, both tools
Governed by: `CobaltC_Master_Instructions.md` §20 (adversarial review)
Depends on: D-0239 (writes-self), D-0206 (the analysis defines the language)
Affects: `spec/14` §6 ("A closure call"), `spec/15` §6; `impl/src/typecheck.rs` (both tools) (CHG-0297)

## Problem

D-0239 made a `move` closure that writes a capture borrow its own place exclusively for a call. The flow
analysis's discharge table applies to that borrow like any other: in `auto s = &c; c();` the fact
`deriv(s, c, shared) = T` refutes `¬clash(a_c, exclusive)`, so the text rejected the program statically. Both
tools faulted at run time instead (their static pass treated every closure call as a read), and D-0239's case
expected the dynamic fault. The model (D-0246) showed the disagreement.

## Options

(i) The tools follow the text: a call of a binding initialized by a self-writing `move` closure literal is an
exclusive access in the static pass; (ii) the text says the analysis takes every closure call as a shared
access. (ii) would make the text weaker than its own rule for a case the analysis can see; (i) only turns a
fault that happens on every path into a rejection.

## Decision

(i). Where the callee is not a binding the analysis saw formed from a literal (a `fn` value, `(*f)(…)`), the
mode is not known: the call is refuted only as a shared access would be, and proven only as an exclusive one
would be. The message: "`s` still borrows `c`, so it cannot be called here: the call changes what it
captured".

## Results

`conf.move-closure-writing-capture-while-shared` now expects the static diagnostic; both tools give it.
