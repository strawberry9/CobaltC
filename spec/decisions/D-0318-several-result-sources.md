# D-0318 — A `from` clause may name several parameters

Status: ACCEPTED (2026-11-12, the owner, on the agent's proposal: "go ahead and do it", after asking whether
`from a, b, c, d` would be allowed too); implemented by `CHG-0437`
Kind: Decision (`spec/02-schema.md`); one production widened, three rules restated over a set
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0011 (single-parameter elision), D-0018 (use-time checking), D-0296 (both toolchains in step),
D-0312 (the `from` clause), CHG-0022 (`feat.explicit-lifetime-parameters` `REJECTED`)
Affects: `spec/22` §3 (`result-source`), `spec/10` §3 (`rule.temporal.elision`: `[Call-Elided-Lifetime]`,
`[Result-Source-Form]`, `[Result-Source-Probe-Only]`), `spec/registry/features.md` (`feat.result-source-clause`),
`spec/registry/diagnostics.md`, `spec/conformance.md`, both front ends

## 1. Problem

A function whose reference result may be any of several of its arguments cannot be written. D-0312 recorded it:
"A result from either of two inputs, the guide's `longest(a, b)`, is still not expressible." Its clause names one
source, and `[Result-Source-Probe-Only]` forbids keeping any other reference parameter, so

    fn longer(ref<String, shared> a, ref<String, shared> b) : ref<String, shared>
    {
        if (StringView::len(a) >= StringView::len(b)) { a } else { b }
    }

is refused with or without `from a`. A program returns which one (a `bool`, an index) and borrows again at the
call. The shape is common: the larger of two by reference, a lookup with a fallback default, the first non-empty
of two views.

What the result's validity depends on is not unknown: it is *any of* `a` and `b`, decided by the run. The checker
needs no flow to use that: the call borrows both arguments for as long as the result lives, and the result lives
no longer than the shorter-lived of the two.

## 2. Candidates

1. **Nothing**: compose single-source helpers (CHG-0022's (c)). It leaves the shape refused for a limit of the
   notation, not of the analysis.
2. **Infer the sources from the body.** The answer is in the body, but a signature would no longer say what a
   call borrows: an edit inside a function would change which of its callers are accepted. The rest of
   `rule.temporal.*` reads only the signature at a call (CHG-0022's reason against (b)).
3. **Named lifetime parameters** (`feat.explicit-lifetime-parameters`): `REJECTED` (CHG-0022), and more than the
   shape needs (§5).
4. **`from` names a set of parameters: `from a, b, …`.** **Selected.** No new word, no lifetime; each rule that
   reads the clause reads "the parameters it names" where it read "the parameter".

The spelling: `from a, b` adds no token (a comma cannot otherwise follow the result type). `from a | b` reads as
an operator on values; `from a or b` adds a word the language has nowhere else. Neither says more.

## 3. Decision

### 3.1 Surface

    result-source ::= 'from' identifier (',' identifier)*

`from a, b, c, d` names four. A list of one is D-0312's clause, unchanged.

### 3.2 The declaration (`[Result-Source-Form]`, `[Result-Source-Probe-Only]`)

- Each name is a parameter of reference type (a `ref`, a slice or a `StringView`), named once, and the result's
  type may hold a reference: else `diag.result-source-invalid`. Naming every reference parameter is allowed.
- Every parameter the clause does not name whose type may hold a reference is only *probed*, as before; the
  named ones may be kept. `diag.result-source-kept` names the clause.
- Naming a parameter the body never returns is allowed: it is a promise the callers keep, sound and only more
  restrictive than needed.

### 3.3 The call (`[Call-Elided-Lifetime]`)

- **Borrowing.** The call is a borrow of every argument the clause names, each in the result's mode: a binding
  initialized or assigned by it has a `deriv` fact for each (`rule.control.flow-analysis`), so while the result
  is live none of those arguments may be moved, written or borrowed in conflict, whichever one the run gives.
- **Escape.** For `rule.temporal.ref-escape` the call's referent-block is the innermost of the named arguments'
  referent-blocks that are known: every named argument is visible at the call, so their blocks nest, and the
  result is valid no longer than the innermost.
- A result that holds references without being one (an `Option<ref<T, _>>`) borrows the named arguments, where
  without a clause it borrows the first reference argument (D-0088).

### 3.4 What does not change

D-0018's use-time checks: every use of the result is checked as any reference's. Without a clause a function
returning a reference still takes exactly one reference parameter (`[Call-Multi-Ref-Return-Rejected]`).
`StringView::split`, `HashMap::get` and `HashMap::get_mut` keep their one-name clauses. No program accepted before
changes meaning; programs refused only for the shape become expressible.

## 4. Cost

Static only: both front ends' parsers, dumps, `rule.temporal.elision` checks and decided-program analyses (D-0296),
and the one place each back end reads the clause (whether a view argument is only probed). No runtime change, and
the lowering is unchanged for every program with at most one name in its clauses. The diagnostics keep
their codes; their messages name the whole clause.

## 5. What it does not express

One set of sources for the whole result, as one Rust lifetime shared by several parameters. Not: parts of a result
from different sources (a pair whose `.0` is `a`'s and `.1` is `b`'s); ordering between inputs (Rust's
`'a: 'b`); storing one argument into another (`push(&mut v, x)` keeping `x`, which `[Result-Source-Probe-Only]`'s
"can receive" still refuses). Those are the cases named lifetimes are for, and CHG-0022's rejection of them stands.

## Revisit conditions

A program that needs a result's parts to come from different sources. The expected answer is a clause per part,
not lifetime names.
