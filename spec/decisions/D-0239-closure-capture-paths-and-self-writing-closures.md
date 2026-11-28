# D-0239 — A closure's capture paths; a `move` closure that writes a capture is borrowed exclusively

Status: ACCEPTED (2026-10-15; delegated by the owner, as D-0235)
Kind: Decision (`spec/02-schema.md`); a clarification (capture paths) and a soundness repair (self-writing
closures, `spec/STABILITY.md` §2)
Governed by: `CobaltC_Master_Instructions.md` §8, §17, §20, §21
Depends on: D-0017 (capture modes), D-0218 (what a shared reference reaches is read-only), D-0221
(`[Closure-Call]` borrows shared)
Affects: `spec/15` §6 (`[Closure-Call]`, prose), `spec/08` §2 (`[Read-Fn-Through-Shared]`),
`spec/OVERVIEW.md` §4; `coby` (`closures.rs`, `calls.rs`, `refs.rs`); `cobc` (`closures.rs`, `exprs.rs`,
`calls.rs`); `cbrt` (`fn_exclusive_capture`, `cb_fn_shared`); `spec/conformance.md` (CHG-0288)

## Problem

The fourth review (F-04) found that `[Closure-Call]` as D-0221 wrote it — `self` a shared borrow, each free
`x_i` replaced by `*self.f_i` or `self.f_i` — composed with D-0218's `[Read-Ref-Through-Shared]` and
`[Write-Not-Exclusive]` rejects every closure that writes a capture, which
`conf.closure-called-while-shared-borrow-lives` and both tools accept. Checking the move-capture half of that
on 2026-10-15 found a real hole the review had not: a `move` closure whose body writes its captured value

    i32 n0 = 0;
    fn() : i32 c = move [n0]() : i32 { n0 = n0 + 1; n0 };
    printf("%v\n", call_it(&c));      // call_it(ref<fn() : i32, shared> f) { (*f)() }

ran under both tools, changing the closure's own state through a shared reference (and, called while
`auto s = &c` lived, changing what `s` reaches). D-0218 promises a shared reference is never a source of
mutation at any depth; `[Read-Fn-Through-Shared]` covered only closures holding exclusive *references*.

## Options

(i) state the capture path for borrow captures (the stored reference used as stored), and borrow a `move`
closure that writes a capture exclusively, extending `[Read-Fn-Through-Shared]` to it; (ii) reject `move`
closures that write a capture; (iii) keep D-0221's shared borrow for all closures and add an exemption for
move-capture writes. (ii) removes counters and accumulators people write; (iii) makes the hole the rule.

## Decision

(i). In `[Closure-Call]`, `writes-self(c)` — a `move` closure whose body writes a capture — makes `self` an
exclusive borrow of the place the closure is called through; any other closure is borrowed shared, as D-0221
said. Inside the call a borrow capture is the reference stored in its field, used as stored (mode `m_i`), not
`[Read-Ref-Through-Shared]`'s shared borrow of it; a move capture is the closure's field through `self`.
`[Read-Fn-Through-Shared]` also faults on a self-writing closure. Both tools record the fact per closure
(`coby`: `ClosureInfo.writes_self`; `cobc`: the same, marking the closure's box `closure = 3`), check an
exclusive access of the place a self-writing closure is called through, and fault, through a shared
reference, with `diag.borrow-exceeds-source` and the detail "changes a value it captured by `move`".

## Rationale

The capture-path sentence is a clarification: it is what both tools did and what D-0221 meant. The exclusive
borrow is a soundness repair: the invariant is D-0218's; the unsafe step is a call that writes the closure's
own storage while a shared reference reaches it; the newly faulted programs are exactly those calls. A
self-writing closure called from its own binding with no other live reference to it, the usual counter,
behaves as before.

## Results

- `spec/15` §6, `spec/08` §2, `spec/OVERVIEW.md` §4.
- Cost: a loop calling a closure through a `fn` field 20 million times, +0.38% / +0.30% user-space
  instructions (GCC / Clang) against Update 108's `cobc`; every `bench/` workload unchanged. The first form
  cost +21%: the test named `cb_fnbox`, which `resolve_scopes` did not know, so the loop kept statement and
  frame records it had dropped; the type is now listed as recording nothing.
- Cases: `conf.move-closure-writing-capture` (ok), `conf.move-closure-writing-capture-while-shared`,
  `conf.move-closure-writing-capture-through-shared`; `conf.closure-called-while-shared-borrow-lives` stands.
