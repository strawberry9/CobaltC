# D-0206 — Review repairs: the safe-client theorem, implementation-independence, outcome sets, dispositions, the reference byte view, and what the flow analysis defines

Status: ACCEPTED (2026-10-11; the owner: "Implement WI-1 through WI-5 and WI-9 as D-0206", wording (b) for
the theorem chosen by the owner)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §12, §13, §18
Depends on: D-0017, D-0096, D-0110, D-0187, D-0188, D-0203
Affects: `spec/00` 1.3.0 (CHG-0239), `spec/20` 1.13.0 and `spec/21` 4.53.0 (CHG-0240), `spec/01` 1.2.0
(CHG-0241), `spec/02` 1.0.68 (CHG-0242), `spec/06` 1.18.0 (CHG-0243), `spec/14` 1.23.0 (CHG-0244),
`spec/IMPLEMENTATION-NOTES.md`, `spec/conformance.md` 3.200.0, `impl/std` (comments only),
`impl/tools/trusted_table.py`, `coby` (two fixes), the guide

## Problem

An independent review of edition 2026.101003 (`ChatGPT_review/`, 2026-10-10) found five places where the
normative text contradicts itself or its own examples, and one where a policy the text relies on is not
stated. Each was re-checked against edition 2026.101007 and found still present:

1. `term.safe-program` said a program with no `unsafe { }` block "never reaches a `discharge: trusted`
   side-condition", while `std`'s normative bodies hold 93 `unsafe` blocks (`Vec::push` writes through a
   raw pointer) that every ordinary program reaches. Under one reading the guarantee was false; under the
   other, almost no program was safe.
2. `term.implementation-dependent-program` said every program not naming `std::extensions` "runs the
   same on every conforming implementation", while `[Sizeof-Addr]`, `[Repr-Byte-Order]`, `[Repr-Enum]`
   and `[Thread-Step]` let two conforming implementations, or two runs, differ observably.
3. `spec/01` §5 required an `outcome` tag's set to be "a documented finite set", while `[Thread-Step]`,
   `[Allocate]`, `[Object-Establish-*]`, `[Os-Random]`, `[Repr-Fn]` and `[Repr-Handle]` give open sets
   by a condition; and §2 counted five base judgment forms where §2.5a makes six.
4. `spec/02` §2 required every Rule to carry a `disposition` tag, while `spec/01` §5 says a rule with no
   invariant-reliant precondition carries none.
5. `spec/06` §7's byte view gave `bytes(ref(a))` as a whole `AddrWidth/8`-byte image and `bytes(cont)` as
   one byte, cell by cell — read literally, `2·AddrWidth/8 − 1` bytes for one reference.
6. `rule.control.flow-analysis` fixes the accepted set but did not say that an implementation's further
   analyses may only discharge checks, nor where diagnostic precedence is stated, nor what the executable
   reference for the accepted set is.

## Options considered for (1)

- (a) "A safe client cannot violate the registered invariants unless a claim of the trusted library base
  is false." Shortest; names the hypothesis and leaves it with the program.
- (b) "The guarantee is unconditional for a safe client over a conforming implementation; the trusted
  library base is part of that implementation's conformance obligation." Puts the burden where
  `spec/21` §0 already put the bodies: an implementation may realize them differently provided every
  conformance case holds.

## Decision

1. **The safe-client theorem, option (b).** `term.safe-client`: a well-formed program none of whose own
   modules holds an `unsafe` block (`std`'s modules are not the program's own). `term.trusted-library-base`:
   the `unsafe` blocks in `std`'s normative bodies, each discharging the trusted rules it instantiates, every
   one listed in `spec/21` §0 with the fact that discharges it there; a native realization of an item takes
   over its obligations unchanged. `term.conformance` now also requires that the implementation's trusted
   library base makes only true claims. `term.safe-program` is the theorem: a safe client on a conforming
   implementation has every reachable `Σ` satisfying `spec/03`, unconditionally, because the only trusted
   claims its execution reaches are the implementation's to keep. The listing is generated from
   `// trusted:` comments in `impl/std` by `impl/tools/trusted_table.py`, which refuses an uncommented
   block (93 blocks, 71 functions); the conformance rows `conf.safe-client-*-trace` walk `Vec`, `String`,
   `Box`, `Rc` and `Channel` through their trusted steps.
2. **Implementation-independent, not identical.** Every program not naming `std::extensions` is governed
   by the same rules, parameterised by the implementation's documented choices (`outcome: impl-defined`)
   and admitting the per-run variation the rules declare (`outcome: unspecified`); it may differ only where
   a rule says so. `term.portable-program` names the programs whose observable behavior is the same under
   every choice and variation. No portable profile beyond the term: the suite runs on one width, and a
   profile nobody tests is a claim nobody checks. `conf.impl-defined-width-observed` is a row whose
   expected output is a function of `AddrWidth`.
3. **Outcome sets.** The braces of an `outcome` tag hold a documented set given by listing or by a
   deciding condition, a listing preferred where one exists; the tag's two freedoms, which step and which
   value, are named; the count of base judgment forms is six.
4. **Dispositions.** The schema's Rule entry carries a `disposition` where `spec/01` §5 gives it one and
   none otherwise.
5. **The byte view** is defined over a range of cells by absolute position; a reference's `ref`/`cont`
   group contributes one address image, equal to the `rawptr` image of the same address.
   `conf.ref-byte-image-width` copies a reference's cells and compares them with the bytes of
   `rawptr_of` on the same object.
6. **The flow analysis defines the language.** A further analysis may discharge checks (`discharge:
   static`, `[Conc-Checks-Omitted]`, `spec/08` §4) but may not accept a program the rule rejects or reject
   one it accepts; diagnostic precedence is D-0110's, cited there; `coby --check` is the executable
   reference checker (`spec/IMPLEMENTATION-NOTES.md` §4).

All six are clarifications: no accepted program is rejected, no outcome changes.

## Results

`conf.ref-byte-image-width` found two `coby` divergences, both fixed: a reference's byte image was all
zeros (the arena kept the reference aside and encoded nothing), where `[Repr-Ref]` makes it the referent's
address image; and `rawptr_of(r)` on a reference *binding* took the address of the binding's own cells
instead of reading the reference (`[Rawptr-Of]`: `r` is a value), so `rawptr_of(r) == rawptr_of(&x)` was
false. The interpreter now gives an object one stable address, shared by `rawptr_of` and by every
reference image to it. `cobc` was right on both.

Not actioned from the review, by this decision: a mechanised proof of the kernel or of `spec/08` §4's
property, a wait-for-graph model checker, a second conformance target, shrinking `std`, and a
lifetime-expressiveness experiment (`ChatGPT_review/ACTION-PLAN-2026-10-11.md` says why). The review's
remaining work items (differential elision testing, supervised-task cases, the corpus checker, the
assurance tiers and security profiles, the edition manifest) are later decisions.
