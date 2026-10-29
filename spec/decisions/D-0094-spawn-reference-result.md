# D-0094 — A spawned thread's result may be a bare reference

Status: ACCEPTED (2026-09-29, owner: "accept both gap recommendations, D-0093 and D-0094")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0031 (threads), D-0071 (handle deriv facts), D-0088
Affects: cobc only (the specification already says this works), conformance

## Problem

`[T-Spawn]` (`spec/12` §5) types `spawn(e_f, …) : handle<τr>` with no
restriction on `τr`, and `spec/19`'s borrow rules make the handle
carry the arguments' borrows, so a reference result is sound and coby
runs it. cobc refused it (`unsupported: a thread whose result is a
reference`) — while running the *same reference inside a struct*
correctly, which is the same capability. Either cobc implements the
bare case, or the specification restricts `τr` — and a restriction
that forbids the bare reference but allows the wrapped one is
indefensible in the spec's own terms.

## Candidate mechanisms

1. **Restrict `[T-Spawn]` to non-reference `τr`.** Must then also
   forbid the aggregate-with-reference result (it is the same
   capability), removing something that works and is useful; or stay
   arbitrary. Rejected.
2. **Implement the bare case in cobc.** The struct path showed the
   representation and borrow bookkeeping already exist. Selected.

## Selected design

No specification change. cobc lowers a bare reference result in its
stored form — the pointer in the thread block's first field, the
token as a slot record there — mirroring how a reference *argument*
travels the other way. The worker receives the call's returned
reference as any ref-returning call's is received, stores it before
the arguments' slots are forgotten (when a function returns its own
reference parameter, the result token IS an argument token, and a
gap in holding would retire it — the worker thread has no scope to
keep an unheld path alive), and `join` rekeys the slot to the
joiner's storage, re-stamps the token into the joining statement's
scope (a same-thread send/receive pair), and forgets the transient
slot.

## Compatibility impact

None: previously-refused programs now run, identically to coby.
Checked shapes: shared and exclusive results, a function returning
its own reference parameter, a `String` reference across threads, the
static rejection when the source is written while the handle borrows
it, all under gcc, clang, and ASan+UBSan.

## Revisit conditions

None.
