# CHG-0299 — The tools' flow analysis keeps a copied closure's facts

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; found by D-0246's enumeration)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `rule.control.flow-analysis` (the closure row; the consumption row covers resource-typed bindings
only), `spec/15` `[Fn-Value-Read]`, D-0206
Affects: `impl/src/typecheck.rs` (`closure_copied`, `check_store_operand`) — both tools; one conformance case

## What changed

The static pass dropped the `deriv` facts of any binding holding borrows without being a reference when it
was stored elsewhere (an argument, an initializer): right for a handle, which moves, but a closure that owns
no resource is copied (`[Fn-Value-Read]`), and the original keeps its borrows. It now keeps the facts of a
binding initialized by a closure literal that owns no resource.

## What changed for programs

`auto c = [x]() { x = x + 1; }; call_val(c); printf("%v\n", x);` is rejected before it runs
(`diag.aliasing-conflict`, static), as the transfer table already had it; both tools faulted at the read
(`conf.closure-passed-by-value-keeps-borrow`). The text accepts no program it rejected; the tools now reject
none it accepts.
