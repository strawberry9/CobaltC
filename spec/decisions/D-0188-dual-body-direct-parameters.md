# D-0188 — Dual bodies: reference arguments checked once at entry

Status: ACCEPTED (2026-10-08, the owner: "proceed with D-0188/CHG-0216, port the patch rather than copying the stale work tree, add the assertion/code-size cap, clarify the threading assumption in the spec", on `private/direct-params-proposal.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0018 (use-time checking), D-0019 (threads), D-0107 (borrows pending until bound)
Affects: `spec/08` §4 (1.2.0); `spec/19` Purpose (1.9.0); `spec/conformance.md` (3.178.0); `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`; `CHG-0216`

## Problem

`cobc` reads and writes a reference parameter unchecked ("direct") only
when it is the function's sole reference, or when every reference is
shared: then no argument can clash with another. The commonest shape in
real code is neither: one exclusive reference to the state being built
and shared ones to the input (`Sha256::update(ref<Sha256, exclusive>,
slice<u8, shared>)`, a writer and its tokens, an output `String` and a
view). Every element access in such a body goes through the runtime's
`clash` test. `std` was restructured by hand around it (helpers taking
the state by value, 5–40× each); a program has no such recourse short of
restructuring.

## Candidate mechanisms

1. **Two bodies and a test at entry.** Selected. For a function whose
   reference parameters are one or more exclusive and any shared, all to
   plain data, each used directly (never handed on, stored or
   reborrowed), and whose other parameters are plain values, `cobc`
   emits `NAME__fast` (every reference direct), `NAME__slow` (as
   before) and `NAME`, which asks the runtime once
   (`cb_paths_disjoint`) whether every incoming path is valid, over a
   live object, not lock-derived, and pairwise free of overlap where one
   is exclusive, and runs the fast body if so. Nothing in the body can
   change that answer (it holds no other reference, and a direct
   parameter forms nothing), and nothing in another thread can either
   (`spec/08` §4), so every access the fast body makes would have passed
   its check; where the test fails the slow body runs and faults where
   and as it always did.
2. **Caller-side proof** (call `NAME__fast` directly where the call
   site sees the arguments cannot alias). Not now: a later refinement of
   1, saving one runtime call per call.
3. **Decline.** `std` keeps its hand-made helpers; programs pay per
   access.
4. **A source annotation** (`noalias`-like). No: a new token, and an
   unchecked claim in safe code.

## Guards (the owner's additions)

- **The in-flight assertion.** A slice argument arrives as reference
  data in flight; the dispatcher reads its token without receiving it
  (`cb_peek_datum_tok(k, n)`). The runtime asserts that exactly the
  callee's `n` slice arguments are in flight, each one reference: any
  other shape would pair a parameter with another's path and is a
  compiler error, reported as a runtime panic rather than answered.
- **Objects inside a slice.** A slice of a `Vec` covers cells that the
  elements' own objects may lie over, and a path to such an element is
  met when a slice element is read, not when the slice is formed. The
  dispatcher also requires `cb_live_in` to find no live object over each
  slice argument's cells (found 2026-10-08 while testing native `Vec`
  bodies; `spec/08` §4 says so).
- **The code-size cap.** A function whose fast body is longer than
  `DUAL_LINES` (600 lines of C, about the 99th percentile of the
  functions `cobc` emits for `std`-heavy programs) is emitted once,
  checked.

## The threading assumption

The soundness argument's one premise beyond the function's own body is
that another thread cannot change an incoming path's standing. It
follows from the existing rules and is now stated in `spec/08` §4 ("A
path's standing changes only in its own thread") with a pointer from
`spec/19`. Ending a path from outside needs `solitary`; a lasting
conflicting path is formed by `[Borrow]`, which faults in the forming
thread; an unchecked projection that conflicts must come from an
ancestor the other thread holds and lives only for that thread's
access, which faults, so the fast body's accesses pass in the
interleaving that orders them first. Lock-derived paths are excluded
(`sync-exempt` changes with another thread's `[Lock]`), and the runtime
test refuses them.

## Consequences

- No rule, message or program changes; `coby` is unchanged.
- `--report-tracking` still lists the slow body's tracking calls (the
  fast body has none).
- `spec/08` 1.2.0, `spec/19` 1.9.0, `spec/conformance.md` 3.178.0: four
  cases of the argument shapes the entry test must tell apart.
- `cobc`'s `tracking` test `state_and_input_gets_a_dual_body_within_the_cap`.
