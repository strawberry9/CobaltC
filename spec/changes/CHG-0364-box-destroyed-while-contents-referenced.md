# CHG-0364 — `coby` checks `[Destroy-Not-Solitary]` when it destroys a `Box`'s value natively

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-21)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0111, D-0221 (`[Ref-Binding-Last-Use]`), `spec/20` (`[Reclaim]`)
Affects: `spec/conformance.md` (one row); `impl/src/interp/destroy.rs` (`destroy_box_value`)

## What changed

No rule changed. `Box::drop` destroys the value with `drop(reclaim<T>(ptr))`. `[Reclaim]` is a place
expression, so that `drop` is `[Destroy]` of a place and needs `solitary`: a reference `Box::get` gave
that is still live makes it fault `diag.destroy-while-aliased`. `cbrt` checked this. `coby`'s native Box
teardown (a Box field, payload or element destroyed with its holder) destroyed the value without the
check, so the reference went stale and faulted `diag.stale-binding` only if it was used again, and a program
that never used it again ran to the end. `coby` now checks it as the body does. The new row
`conf.box-destroyed-while-contents-referenced` has the reference's end deferred (D-0221) past a call that
destroys the Box's holder.

## What changed for programs

Under `coby`, such a program now faults where it faulted under `cobc`.

## Found by

Porting the code generator to CobaltC (D-0274): the compiled port faulted in its `match` lowering, where a
reference to a boxed type name was still live (deferred: its declaring statement mentions a reference
parameter used again) when the scrutinee was moved into a temporary and destroyed; `coby` ran the same
code to the end. 2026-10-21.
