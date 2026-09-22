# CHG-0021 — Corpus Example for the `write` Extern Surface

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: rule.trust.extern-call, rule.trust.rawptr, rule.stdlib.prelude, rule.agg.array-construct, CHG-0020
Affects: examples.md, conformance.md

## Problem / motivation

`CHG-0020` added `write(rawptr<u8> buf, usize len) : isize` to the
prelude but deliberately added no example or conformance case of its
own ("a future example ... may now route an observable value through
`write` ... none is required by this decision itself"). The human
owner asked for a worked example demonstrating the new surface, formalized
as a corpus entry rather than left as illustrative-only prose.

## Decision

Add one `canonical valid use` example and its derivation, in the same
form as `ex.unsafe-required`/`conf.rawptr-deref-inside-unsafe-ok` (the
existing trust-boundary corpus entries): build a byte buffer with an
array literal (no string/char literal exists per `spec/22` §1), take a
`rawptr<u8>` to it via the safe `[Rawptr-Of]` + `reinterpret_ptr` path,
and call `write` inside `unsafe { }`. No rule change is needed or
proposed — this exercises `[Extern-Call]` exactly as already specified.

## What changed

**`spec/examples.md` 3.10.0**: `ex.extern-write` added (a `send`
function wrapping `write` in `unsafe { }`, called with a 6-byte
`array<u8,6>` literal spelling "hello\n").

**`spec/conformance.md` 3.12.0**: `conf.extern-write-observed` added,
deriving `ex.extern-write` step by step: `[Array-Construct]` →
`[Rawptr-Of]` (safe) → `reinterpret_ptr` (safe) → `[Extern-Call]`
(trusted-unchecked, discharged by the surrounding `unsafe` block).
Outcome recorded as `ok; → claim : isize` with
`Σ.trust(claim) = unchecked-claim` — the specification does not pin a
concrete numeric value for a real `extern` call's result (that is the
whole point of `feat.minimal-io-extern-surface`: the value is
observable by a real running program, not derivable by the formalism),
so the conformance row states the typed, trust-tagged claim `[Extern-
Call]`'s own conclusion produces, not a fabricated number.

## Rule changes

None.

## Affected invariants

None restated. `inv.trust-transition` already governs `claim`'s
`unchecked-claim` tag; this is a new instance exercising it, not a
change to it.

## Dependency impact

`spec/examples.md`/`spec/conformance.md` each gain one new entity;
`feat.minimal-io-extern-surface` and `rule.trust.extern-call` gain no
new `Depends on` citations (examples/conformance cases are consumers,
not depended upon).

## Compatibility classification

Purely additive. No existing example, conformance row, or rule
changes.

## Migration implications

None.

## Example changes

`ex.extern-write` added (`spec/examples.md` 3.10.0).

## Conformance changes

`conf.extern-write-observed` added (`spec/conformance.md` 3.12.0).

## Future implementation implications

None beyond what `CHG-0020` already stated (a reference interpreter
must bind `write` to a real OS primitive). This record adds no new
obligation — it only confirms, by derivation, that the existing rules
already produce a well-formed, non-faulting outcome for the example
shown.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

None outstanding.
