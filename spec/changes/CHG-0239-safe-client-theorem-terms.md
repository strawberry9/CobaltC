# CHG-0239 — `term.safe-client`, `term.trusted-library-base`, the safe-program theorem, implementation-independence, `term.portable-program`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0206 (1), (2))
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0206
Affects: `spec/00` 1.3.0, `spec/conformance.md` 3.200.0 (`conf.impl-defined-width-observed`)

## What changed

- `term.safe-client`: a well-formed program none of whose own modules contains an `unsafe { }` block.
- `term.trusted-library-base`: the `unsafe` blocks in `std`'s normative bodies, listed in `spec/21` §0
  (CHG-0240); a native realization keeps the same obligations.
- `term.conformance`: a conforming implementation's trusted library base makes only true claims.
- `term.safe-program`: the theorem — a safe client on a conforming implementation satisfies every
  proposition of `spec/03` in every reachable `Σ`, unconditionally (option (b) of D-0206).
- `term.implementation-dependent-program`: "runs the same on every conforming implementation" replaced by
  "the same rules, parameterised by documented choices, admitting declared variation; differs only where a
  rule says so"; `term.portable-program` added.
- `conf.impl-defined-width-observed`: prints `sizeof<usize>()`, stated as a function of `AddrWidth`.

## What did not change

No rule, diagnostic or program outcome. A program with no `unsafe` block of its own was and is a safe
program; the text now says why its execution's trusted steps do not weaken the guarantee.
