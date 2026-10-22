# CHG-0251 — `[Thread-Affine]`, `[Thread-Lend]`, `[Lend-Places]`; `spawn` and `Channel::send` lend exclusive references

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0213)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0213
Affects: `spec/08` 1.4.0, `spec/04` 1.6.0, `spec/19` 1.14.0, `spec/14` 1.24.0, `spec/21` 4.55.0,
`spec/registry/diagnostics.md` 1.51.0, `spec/conformance.md` 3.208.0; `impl/std/sync.cb` (`Channel::send`);
`impl/src/interp.rs`, `impl/src/typecheck.rs`, `impl/cbrt/src/lib.rs`, `impl/cbrt/include/cbrt.h`, `impl/cobc/src/lower.rs`;
`impl/tests/conformance.rs` (covers line); seven cases in `impl/conformance/19-concurrency/`

## What changed

- `spec/08`: `rule.alias.thread-affine` (`[Thread-Affine]`, checked, `diag.aliasing-conflict`); §1 grounds
  `inv.alias-validity` on it as well as on `clash`; `[Standing-Local]` lists it among the transition kinds.
- `spec/04`: `AccessPathRecord.thread : Thread | transit`, its meaning; `running(ℓ, Σ)`.
- `spec/19`: `[Thread-Lend]`; `[Spawn]` lends its arguments and captures; `[Send]` lends its value; the prose
  after `[Spawn]` and the reason `[Conc-No-Race]` holds.
- `spec/14`: two rows of the flow analysis's table for a reference binding passed to `spawn`.
- `spec/21`: `[Lend-Places]` (std-only intrinsic `lend_places`); `Channel::send`'s body calls it first.
- `spec/registry/diagnostics.md`: `diag.aliasing-conflict` names the two rules.
- `spec/conformance.md` §12: seven rows.

## What changed for programs

A program in which one exclusive reference was used by two threads at once, or by a thread and its spawner,
was accepted and raced; it now faults (statically where the flow analysis sees both steps). A thread's access
through an exclusive reference another running thread formed and did not lend — one stored in a struct the
thread was lent a reference to, in a `Vec` moved to it, in a `mutex` — now faults. Programs that pass a
reference to one thread at a time, send references through channels, or share shared references are unchanged.
