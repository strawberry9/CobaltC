# CHG-0012 — Fifth End-to-End Program: Element Reference Held Across a Reallocation

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: inv.temporal-validity, inv.alias-validity, rule.control.flow-analysis, rule.temporal.ref-escape, rule.trust.rawptr, rule.value-object.read, rule.value-object.object-end, rule.resauth.destroy, rule.fail.fault-unwind
Affects: ex.e2e-vec-realloc-stale-ref, conf.e2e-vec-realloc-stale-ref

## Problem / motivation

`CHG-0011`'s "Revisit conditions" named four programs the four
end-to-end derivations did not yet cover. The first: `Vec::pop`/
`Vec::push` on a reallocated buffer while an element reference
(`Vec::index_shared`) is still bound, expected to fault
`diag.stale-binding` dynamically at the reference's next use, via
`[Release]` → `[Object-End]` → the read's `[Read-Stale]` path — as
distinct from `conf.vec-ref-then-push-rejected`, which already covers
the *same-function* shape and is rejected *statically* before the
program ever runs.

## What was derived

`ex.e2e-vec-realloc-stale-ref`: a function `hold_across_grow` receives
its `Vec<i32>` by reference parameter, takes a shared element
reference through `&*v` (a reborrow of the parameter, not a borrow of
a plain local), pushes four more elements through the same parameter
— the last one reallocates — and dereferences the held reference.
Derived end to end in `spec/conformance.md`
`conf.e2e-vec-realloc-stale-ref`.

The key fact the derivation turns on: `rule.control.flow-analysis`'s
`deriv`/`escaped` facts, and `rule.temporal.ref-escape`'s
`referent-block`, are both defined only over a *plain* binding's own
`x.π` field/index projections. A place rooted at `*v` for a
reference-typed parameter `v` is neither shape, so nothing in either
mechanism names `v`'s referent at all — the case is not "refuted" the
way `conf.vec-ref-then-push-rejected`'s same-function `deriv(r, v.ε,
shared)` is; it is simply outside what either mechanism tracks, and
every guard that would otherwise have caught it resolves `unknown` →
`discharge: dynamic` per `rule.control.flow-analysis`'s own outcome
policy. Separately, `Vec::push(v, …)` passes `v`'s existing reference
*value* to `push`'s own parameter (`rule.fn.bind-param`'s `ref<τ,m>`
row: "no new borrow"), so no `[Borrow]` — and hence no `¬clash` check
of any kind, static or dynamic — is ever performed against `v`'s
referent at these call sites; and even where a clash check *is*
performed (inside `push`'s own body, against the Vec struct object
`o_v`), it could never witness the element reference regardless,
because `clash` requires the same `of` and the element reference's
`of` is the reclaimed element object, never `o_v` (`spec/04` §2). The
only mechanism that catches the hazard is temporal validity: `[Release]`
(run during `grow`'s `[Deallocate]`) ends the reclaimed element object,
and the later `*r` fails `[Read]`'s `alive(of(a,Σ),Σ)` premise —
`[Read-Stale]` → `diag.stale-binding`, discharged dynamically because
that premise, too, is a shape `rule.control.flow-analysis`'s discharge
table does not cover for a path reached by dereferencing a reference.

## Rule changes

None. Every rule needed — `[Ref-Deref-Place]`, `[Borrow]`, `[Reclaim]`,
`rule.fn.bind-param`, `[Allocate]`/`[Copy-Raw]`/`[Deallocate]`/
`[Release]` (as restated by `CHG-0011` E-02), `[Rawptr-Write]`,
`[Read]`/`[Read-Stale]`, `[Object-End]`, `[Destroy]`, `[Fault-Unwind]`
— already states what the derivation needed; this is the first
end-to-end pass (of five, across `spec/AUDIT.md`, `spec/AUDIT-2.md`,
`CHG-0009`, `CHG-0010`, `CHG-0011`) that found no gap. Recorded anyway,
per `CobaltC_Master_Instructions.md` §21, because it adds new required
behavior to two normative artifacts (`spec/examples.md`,
`spec/conformance.md`) even though no rule's meaning changed.

## Affected invariants

None restated. The derivation exercises `inv.temporal-validity` (the
reclaimed element's staleness) and `inv.alias-validity` (why `clash`
never engages here) exactly as already stated.

## Dependency impact

None. No existing entity's `Depends on`/`Affects` changes; the two new
entities depend on the rules they derive, listed above.

## Compatibility classification

Purely additive. No previously well-formed program changes meaning or
acceptance.

## Migration implications

None.

## Example changes

`ex.e2e-vec-realloc-stale-ref` added (`spec/examples.md` 3.6.0).

## Conformance changes

`conf.e2e-vec-realloc-stale-ref` added (`spec/conformance.md` 3.7.0);
no existing row changed.

## Future implementation implications

A conforming implementation's dynamic check on dereferencing a
reference must reach the *current* liveness of whatever object the
reference's value denotes at the moment of the access, not merely the
reference binding's own liveness — the two are tracked separately here
(`temporally-valid` vs. `alive(of(a,Σ),Σ)`) and only the dynamic path
enforces the latter for a reference reached through a parameter.

## Prior-art status

Not applicable (a Change record).

## Revisit conditions

`CHG-0011`'s remaining three: a `mutex<Vec<i32>>` guard drop, a
closure capturing an `Rc<Vec<i32>>` by move, and a fault inside a
spawned thread while `main` holds a guard. None is covered yet.
