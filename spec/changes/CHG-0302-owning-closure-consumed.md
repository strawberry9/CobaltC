# CHG-0302 — The tools' flow analysis consumes a closure that owns a resource

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; found by D-0246's extension to closures owning a resource)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `rule.type.is-resource` (a closure whose captures include a resource is one by derivation),
`rule.control.flow-analysis` (the consumption row), `spec/15` `[Fn-Value-Read]`
Affects: `impl/src/typecheck.rs` (`closure_owning`, `check_store_operand_inner`) — both tools; one conformance
case

## What changed

A binding initialized by a `move` closure literal that captured a resource is resource-typed by derivation;
the static pass now consumes it where it is passed or stored by value, as the consumption row says.

## What changed for programs

`call_val(mk); mk();` for such an `mk` is rejected before the program runs (`diag.stale-binding`, static); it
faulted at the call (`[Fn-Value-Empty-Call]`). A `fn` value the analysis did not see formed is still checked
at run time (`conf.fn-value-emptied-call`).
