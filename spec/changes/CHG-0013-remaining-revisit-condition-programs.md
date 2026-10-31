# CHG-0013 — Remaining Three Revisit-Condition Programs

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: inv.resource-authority, inv.concurrency-validity, rule.conc.lock, rule.conc.spawn, rule.conc.join, rule.resauth.destroy-composite, rule.fn.closure, rule.fail.fault-unwind
Affects: ex.e2e-mutex-vec-resource-interior, ex.e2e-closure-move-rc, ex.e2e-thread-fault-abandons-guard, conf.e2e-mutex-vec-resource-interior, conf.e2e-closure-move-rc, conf.e2e-thread-fault-abandons-guard

## Problem / motivation

`CHG-0011`'s "Revisit conditions" named four programs; `CHG-0012`
closed the first (a stale element reference across a `Vec`
reallocation). This record closes the remaining three: a
`mutex<Vec<i32>>` exercised through its guard, a `move` closure
capturing an `Rc<Vec<i32>>` and later dropped, and a fault inside a
spawned thread while `main` holds an unrelated guard.

## What was derived

- `ex.e2e-mutex-vec-resource-interior` /
  `conf.e2e-mutex-vec-resource-interior`: `Mutex::new` moves a resource-typed `inner`
  (`rule.resauth.relocate-in` tracks `(o_m, inner)` as an obligation,
  unlike the existing `ex.e2e-threads-mutex` example's plain `i32`
  interior, whose derivation explicitly notes "composite of `inner`
  (none)"); pushing through the guard exercises `[Guard-Deref]` on a
  resource-typed interior and `[Sizeof-Mutex]`/`[Repr-Mutex]`; the
  mutex's automatic destruction at block exit runs
  `rule.resauth.destroy-composite` on `inner`, reaching `Vec::drop`
  through a temporary exclusive root exactly as
  `conf.e2e-rc-resource-payload` already does for `RcBox`'s `value` field.
- `ex.e2e-closure-move-rc` / `conf.e2e-closure-move-rc`: a `move`
  closure's captured `Rc<Vec<i32>>` is a struct field like any other
  (`[Closure-Form-Move]`); calling the closure twice only forms
  transient shared sub-borrows of that field, consuming nothing;
  `drop`ping the closure runs `destroy-composite` on the captured
  field, reaching `Rc::drop` exactly as `conf.e2e-rc-resource-payload`
  already derives for a last drop.
- `ex.e2e-thread-fault-abandons-guard` /
  `conf.e2e-thread-fault-abandons-guard`: a spawned thread whose body always faults
  (`n / 0`), while `main` holds a guard on an unrelated mutex.
  `[Fault-Unwind]` unwinds only the faulting thread; `main`'s guard,
  mutex, and unjoined handle are abandoned mid-flight, exactly as
  `spec/18` §1's prose already states ("other threads take no further
  steps... their resources are abandoned"). Derived and confirmed: no
  design gap. `rule.fn.program`'s observable behavior is only the
  termination outcome and the `extern`-call sequence; since none of
  `main`'s remaining steps in this program perform an `extern` call or
  produce a value read again, every interleaving `[Thread-Step]`
  admits reaches the identical observable outcome
  (`terminate(diag.div-by-zero, Σ')`) — the abandoned guard/mutex/
  handle affect only unobserved internal state.

## Rule changes

None. All three derivations go through under the rules as they stand
after `CHG-0011`/`CHG-0012`. This is the second and third consecutive
pass (after `CHG-0012`) to close a `CHG-0011` revisit condition without
finding a gap — the `Vec::grow`/reclaim machinery, `destroy-composite`,
and `[Fault-Unwind]`'s per-thread scoping were all completed by earlier
passes precisely so that these interactions would compose correctly.

## Affected invariants

None restated.

## Dependency impact

None on existing entities.

## Compatibility classification

Purely additive.

## Migration implications

None.

## Example changes

Three examples added (`spec/examples.md` 3.7.0).

## Conformance changes

Three rows added (`spec/conformance.md` 3.8.0); no existing row
changed.

## Future implementation implications

A conforming implementation's thread-fault handling must not attempt
to unwind or run destructors for any thread other than the one that
faulted, even when doing so would look like an obvious leak fix —
`spec/18` §1 is explicit that this is by design (a second unwind could
itself fault).

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

`CHG-0011`'s four named revisit-condition programs are now all closed
(`CHG-0012`, this record).
