# CHG-0072 — `sync-exempt` for a mutex inside an aggregate

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen with D-0063)
Governed by: `CobaltC_Master_Instructions.md` §1, §17, §19, §21
Depends on: D-0018, D-0063
Affects: rule.conc.lock, `spec/04` `clash`

## Problem / motivation

`sync-exempt` exempted a lock path only from shared references whose
type is exactly `mutex<_>`. A struct with a `mutex` field shared between
threads — `Channel<T>` (D-0063), or any program's own — clashed: one
thread's write through its guard met the other thread's shared
reference to the struct, which overlaps the mutex's interior.

## Decision

A lock-derived path and a shared path that is not lock-derived never
clash, in either direction, whatever the shared path's type. Two
lock-derived paths are checked as any two paths are.

## What changed

- **`spec/19` 1.6.0:** `sync-exempt`'s definition and prose.
- **`spec/04` 1.3.3:** `clash`'s note.
- **`spec/conformance.md`:** the cases below.
- **Implementations:** `coby`'s `clash` decides lock-derivation by the
  guards' tokens and their descendants (it looked at whether the target
  object was a mutex), and a guard's token now records the locking
  reference and its ancestors as its own (`[Lock]`'s `base := a_m`);
  without that, locking through an exclusive reference — a destructor's
  `self` — clashed with that reference. `cbrt`'s `check_access` and
  `check_root_access` compare whether each path has a lock origin. The
  checker types `lock(e)` as `guard<τ>` (it was untyped, so arithmetic
  on a guarded value took a literal's default type).

## Compatibility classification

Extension: programs that faulted `diag.aliasing-conflict` now run;
none that ran changes.

## Conformance changes

**Added:** `conf.mutex-in-shared-struct`, `conf.lock-through-exclusive-ref`,
`conf.guard-interior-borrow-conflict`.

## Revisit conditions

None.
