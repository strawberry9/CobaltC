# D-0011 — Single-Parameter Lifetime Elision

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §11, §12, §17
Depends on: inv.temporal-validity, D-0005, `spec/15-function-semantics.md`

## Problem

D-0005 left interprocedural reference-escape checking to the dynamic
`alive(o,Σ)` fallback and recorded `feat.static-lifetime-tracking`,
`UNDER_INVESTIGATION`, pending Function Semantics existing to state
signatures over (`spec/15`, now written). A full lifetime-polymorphism
system (multiple named lifetime parameters, subtyping/variance among
them) is a large, independently-hard design problem. The dominant real
case — a function taking one reference parameter and returning a
reference derived from it (an accessor) — does not need that much
machinery.

## Constraints

- §12: prefer static rejection wherever the fact is knowable without
  execution; the dynamic baseline (D-0005 §1) remains sound regardless,
  so any static addition only needs to be *sound*, not complete.
- §9: smallest sufficient mechanism — do not design full lifetime
  polymorphism if a narrower rule covers the common case.
- Must compose with `spec/08`'s `Borrow`/suspend-reactivate mechanism
  rather than duplicating it.

## Candidate mechanisms

1. **Full lifetime polymorphism** (named lifetime parameters on
   function signatures, subtyping/variance, multiple independent
   parameters). Most general; large design surface with no forcing
   requirement yet (no construct defined through `spec/21` needs more
   than the single-parameter case). Deferred — recorded as a narrower
   feature, not designed now (see Selected design).
2. **No static improvement; dynamic baseline only.** Sound, but leaves
   the dominant accessor-function pattern unable to get a compile-time
   diagnostic for a caller-side escape, unlike the equivalent
   direct-borrow case D-0005 §2 already covers. Rejected as
   unnecessarily weak given option 3 is available at low cost.
3. **Single-parameter elision**: when a function has *exactly one*
   reference-typed parameter and returns a reference type, a call to it
   is treated, for temporal-validity purposes, exactly as if the caller
   had called `Borrow` directly on the argument passed for that
   parameter — reusing `spec/08`'s existing suspend/reactivate and
   `spec/10` §2's lexical-escape check verbatim, no new state or
   checking logic. Functions with zero or more than one reference-typed
   parameter that attempt to return a reference type are rejected
   outright (no elision applies; nothing to infer from). **Selected.**

## Selected design

- **Elision applies** when a function's parameter list contains
  exactly one `ref<τ,m>`-typed parameter and its return type is
  `ref<τ',m'>`. A call `f(...)` is checked as if the caller had written
  `borrow(a_k, m')` directly on the argument `a_k` passed for that
  parameter — `[Call-Elided-Lifetime]`, `spec/10-temporal-validity.md`
  §3.
- **Elision does not apply** (return-of-reference is rejected outright)
  when the parameter count of reference type is 0 or ≥2 —
  `[Call-Multi-Ref-Return-Rejected]`. This is deliberately conservative:
  no attempt is made to guess which parameter a multi-parameter
  function's returned reference derives from.
- **Full lifetime polymorphism** (the general, multi-parameter,
  annotated case) is *not* designed here. `feat.static-lifetime-
  tracking` is updated to `ACCEPTED` for the elision scope just
  described; a new, narrower feature, `feat.explicit-lifetime-
  parameters`, is opened `UNDER_INVESTIGATION` for the rejected
  multi-parameter case (`spec/registry/features.md`).

## Rejected alternatives

Full lifetime polymorphism now (1, deferred to a narrower future
feature); dynamic-baseline-only (2).

## Semantic rationale

Reusing `Borrow`'s existing suspend/reactivate mechanism rather than
inventing call-boundary-specific state means this decision adds no new
`Σ` component and no new invariant — it is a *checking* rule about when
an already-defined operation's effect applies, exactly the pattern
D-0005 itself used for the lexical case. The conservative multi-
parameter rejection keeps the mechanism sound without needing to solve
the harder disambiguation problem now.

## Usability / Explainability / Implementation-feasibility

Usability: the overwhelmingly common accessor-function shape (one
reference in, one reference out) gets a compile-time diagnostic instead
of relying on the dynamic fallback; multi-parameter reference-returning
functions are rejected rather than silently falling back to the dynamic
check, which is a real (if narrow) reduction in what earlier artifacts
would have accepted — recorded honestly as a new restriction, not hidden.
Explainability: `diag.lifetime-elision-ambiguous` names exactly why
(more than one, or no, reference parameter to elide from).
Implementation-feasibility: reduces to an existing, already-implemented
check (`Borrow`); no new analysis required.

## Compatibility impact

`spec/10-temporal-validity.md` gains §3; no earlier artifact's rules
change meaning, since nothing before this decision could express a
reference-returning function at all without going through the (now
stricter, previously unrestricted) multi-parameter case — this is a new
restriction on a previously-unconstrained corner, not a behavior change
to any existing accepted program shape.

## Prior-art status

Single-parameter elision is independently the minimal generalization of
D-0005's lexical rule across one call boundary; that similar elision
rules exist elsewhere is consistent with, not the basis for, this
derivation.

## Invariant traceability

Extends `inv.temporal-validity`'s static coverage
(`spec/03-invariants.md`); traces to D-0005.

## Revisit conditions

Revisit `feat.explicit-lifetime-parameters` if a concrete
multi-reference-parameter accessor pattern is needed — the expected
resolution is named lifetime parameters on the signature, not a
reversal of this decision's conservative default.
