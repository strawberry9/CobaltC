# D-0208 — The review's recommended tests, audited and filled

Status: ACCEPTED (2026-10-11; the owner: "Implement WI-12 as D-0208")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §19, §20
Depends on: D-0204, D-0206
Affects: `spec/conformance.md` 3.202.0 (CHG-0246), `impl/conformance/` (five case files), `coby`
(`impl/src/interp.rs`: two fixes), `spec/02` 1.0.70

## Problem

The independent review of edition 2026.101003 (§3.2, §3.5, §3.6; `ChatGPT_review/ACTION-PLAN-2026-10-11.md`
WI-12) recommended twenty specific tests of ownership, arithmetic and modules. Most were already
conformance cases under other names; a few were not, and nothing recorded which was which.

## Decision

Every recommended test is mapped to a `conf.*` row, in `stress/coverage_oct11/audit.md` (local: the
mapping is working material, the rows are the record). Thirteen were covered. Seven rows are added
(CHG-0246), each run under `coby`, `cobc --cc gcc` and `cobc --cc clang`:

- `conf.zero-sized-types`: `struct Z {}` and `array<i32, 0>` have size 0 and alignment 1; a `Vec<Z>`
  counts its elements; a reference to a `Z` is formed and read through.
- `conf.early-return-destructured`: a resource aggregate taken apart by destructuring and left by an
  early `return` carrying one part: the other is destroyed exactly once, at the return.
- `conf.reclaim-twice-same-object`: two `reclaim`s of one address re-attach one object; two shared
  borrows of it coexist (the positive form of `conf.reclaim-two-paths-clash`).
- `conf.try-join-resource-result`: a resource result through `try_join`, and through a generic function
  of the program's own joining a `handle<R>` parameter.
- `conf.shift-width-edges`: shifts by width − 1 (signed `1 << 31` is the minimum, `-8 >> 31` is `−1`,
  `1: u64 << 63`) and a computed shift by the width, `diag.shift-amount-out-of-range` at run time.
- `conf.signed-zero`: `−0.0 == 0.0`, not below it, printed with its sign, `1.0 / −0.0 < 0.0`.
- `conf.if-branch-type-mismatch`: `if (c) { big } else { 2: i32 }` with `big : i64` is rejected.

## Results

`conf.try-join-resource-result` found that `try_join` of a resource result faulted `diag.read-of-resource`
in `coby` while `cobc` ran it. Two causes, both fixed in `impl/src/interp.rs`: `spawn` stored a `void`
placeholder as the handle's result type, so a generic callee (`try_join<R>(handle<R>)`) inferred `R = void`
and the joined resource was read as a value; and the interpreter's call-site inference bound any
argument-less type name, so even a non-generic `fn get(handle<Noisy> h)` bound `Noisy := void`. The
handle now carries the thread's declared result type, and only a name that is not a declared struct or
enum is bound. `[Join]`'s own result still takes its type from the thread's status, as before. Every other
probe agreed between the tools on its first run.
