# CHG-0437 — A `from` clause may name several parameters (D-0318)

Status: implemented (2026-11-12), D-0318, D-0296
Affects: `spec/22` §3 (`result-source`), `spec/10` §3 (`rule.temporal.elision`), `spec/registry/features.md`
(`feat.result-source-clause`, `feat.explicit-lifetime-parameters`), `spec/registry/diagnostics.md`
(`diag.lifetime-elision-ambiguous`, `diag.result-source-invalid`, `diag.result-source-kept`), `spec/conformance.md`
(five rows); `impl/src` (`ast.rs`, `parser.rs`, `astdump.rs`, `derive.rs`, `views.rs`, `tier.rs`, `typecheck.rs`);
`impl/cobc/src/lower/analysis_refs.rs`; their twins in `impl/selfhost/front` (`ast.cb`, `parser.cb`, `dump.cb`,
`derive.cb`, `tier.cb`, `typecheck.cb`) and `impl/selfhost/back/strings4.cb`; the guide and the book

## What changes

**The clause.** `fn f(…) : τ from a, b, …` names the parameters the result's references may come from. A
declaration's `source` is a list in both front ends (empty without a clause); the dumps print the names after
`from`, separated by spaces.

**The declaration.** `[Result-Source-Form]` checks each name in order: a name given twice, a name that is no
parameter, and a parameter of no reference type are each `diag.result-source-invalid`, then a result holding no
reference. `[Result-Source-Probe-Only]` probes every reference parameter the clause leaves out; the walk itself
is unchanged, and a call of a function whose clause does not name an argument's parameter probes that argument,
as one naming another did.

**The call.** The source positions are the named parameters', in order (else the one reference parameter).
`referent_of` takes the innermost of the named arguments' known referent-blocks; `deriv_facts_of` gives one fact
per named argument that is a visible borrow, so a binding of the result holds all of them. A result that holds
references without being one (`contained_ref_facts`) borrows the named arguments, where it took the first
reference argument whatever the clause said; every `std` clause names its first reference parameter, so no
`std` call's facts change.

**The back ends.** `pure_view_in` (cobc) and its twin (cbc) treat a view argument as probed when the clause does
not name its parameter. The decided-program analysis (`tier`) reaches inside every named argument.

**Messages.** `diag.lifetime-elision-ambiguous` now suggests the clause (`name the ones it may come from with
\`from\` (\`from p1, p2\`)`); `diag.result-source-kept` names the whole clause and its sources ("comes from `a`
and `b` alone"); `diag.result-source-invalid` names the offending name. Codes are unchanged.

## Measured

The five new cases give the same diagnostics and output under `coby`, under `cobc` with GCC and Clang, and under
`cbc`. A clause of one name lowers and checks as before: every `std` clause has one name, and no case outside the
new ones changes outcome.
