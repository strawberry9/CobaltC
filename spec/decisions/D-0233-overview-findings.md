# D-0233 — What writing the semantics overview found in the text

Status: ACCEPTED (2026-10-14; under the owner's instruction to resolve the issues found, WI-19)
Kind: Decision (`spec/02-schema.md`); clarifications and one correction of the text to the decision it
realizes
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0008 (destruction), D-0018 (clash), D-0107 (`pending`), D-0146 (`unsafe extern`), D-0187
(`std::extensions`), D-0206 (the safe-program theorem), D-0213 (`[Thread-Affine]`)
Affects: `spec/00` 1.3.1, `spec/03` 1.6.1, `spec/04` 1.7.1, `spec/05` 1.8.1, `spec/08` 1.5.1, `spec/14` 1.26.1,
`spec/19` 1.15.1, `spec/20` 1.17.1, `spec/conformance.md`; `impl/model` (CHG-0279)

## Problem

`spec/OVERVIEW.md` (WI-19) restates every chapter's rules in prose, and each paragraph cites what it
restates. Writing it found eight places where two parts of the text said different things, or where one
said something the rules do not bear out.

## Decisions

1. **The frame's objects end last bound first.** `[Block-Exit]` ordered them by descending `origin`,
   which is when each object was established. D-0008 says reverse declaration order, and both
   implementations do that. The two differ after a move: in `Ticket a; Ticket b; Ticket c = a;`, `c`
   holds `a`'s object, whose origin is before `b`'s. The text gave "ticket 2, then ticket 1", and both
   tools give "ticket 1, then ticket 2". The decision stands; the rule now orders by descending
   `formed-at` of each object's holder, the binding path that holds it (`spec/14` `[Block-Exit]`).
   Without a move that is the same order as before. The model follows. Case:
   `conf.destruction-order-after-move`.
2. **One `clash`.** `spec/08` §1 restated `clash` without `¬pending` and `¬sync-exempt`, which
   `spec/04` §2, the definition, has; `spec/03`'s `inv.alias-validity` did not mention `pending` either.
   Both now say what `spec/04` says.
3. **A lock path is derived, not a root.** `[Lock]` gives the lock path the locking reference as its base,
   so that the reference and its ancestors are not its competitors. `term.access-path`, `state.holder`
   and `spec/04`'s `steps` called it a root. They now describe it as `[Lock]` forms it: its steps are the
   base's and then the mutex's `inner`.
4. **Addresses are observable, and still unspecified.** `spec/05` said the address chosen for an object
   is unobservable to a safe program, since no rule converts a `ref` to an address. `[Rawptr-Of]` is safe
   and does exactly that, and `==` on raw pointers compares addresses. The freedom stays `unspecified`,
   which is what it always meant: a program whose outcome depends on the address has more than one
   permitted outcome.
5. **The list of trusted rules is complete.** `rule.trust.unsafe` left out `[Rawptr-Read-Volatile]`,
   `[Rawptr-Write-Volatile]` and `[Extern-Decl]`, and said that an `unsafe` block is the only place a
   trusted claim is made. `[Extern-Decl]`'s claim is made by the `unsafe` before `extern` on the
   declaration (D-0146). Now both are said.
6. **`spec/19`'s purpose names `[Thread-Affine]`.** It said that cross-thread aliasing "needs no new
   check", which was true before D-0213.
7. **The theorem covers `std::extensions`.** `term.trusted-library-base` now includes the `unsafe` blocks
   of the implementation's own `std::extensions` bodies. A program that names an extension is covered on
   the implementation that provides it, and that implementation documents those blocks.

The overview also noted that `[Standing-Local]` has six hypotheses, while the plan for WI-19 said five. The
text is right and the plan was out of date. `impl/model/README.md` still said that closures awaited the
owner, although D-0221 settled the question; the README is corrected.

## What changed for programs

Nothing that a program can observe, except that the text now gives the destruction order after a move that
both implementations always gave.
