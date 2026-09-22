# CobaltC Temporal Validity

Status: normative artifact
Version: 1.1.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.temporal.*`)
Governed by: `CobaltC_Master_Instructions.md` §17 (Temporal Validity),
§23
Realizes: D-0005 (two-layer checking), D-0011 (elision), D-0018

## 1. Dynamic baseline (already in force)

`inv.temporal-validity` is fully enforced by rules defined elsewhere;
this artifact adds static refinements only.

- Every access through a path checks `temporally-valid(a, Σ)`
  (`rule.value-object.read`/`write`, `rule.alias.borrow`,
  `rule.resauth.*`).
- A path is invalidated by: its object ending
  (`rule.value-object.object-end`); the last object holding it ending
  or overwriting it (`object-end`, `rule.value-object.write`); the end
  of the statement that formed it, if nothing holds it and it is not
  the statement's result (`rule.control.stmt`); transfer or
  relocation of its object (`rule.resauth.transfer`/`relocate-in`).
- A binding's object ends when its frame exits
  (`rule.control.block`).

## 2. Static refinement: lexical escape

### `rule.temporal.ref-escape`
**Status:** ACCEPTED

Definitions over program text (all decidable without execution):
- `decl-block(x)`: the block (or function body) whose statement list
  declares binding `x` (a parameter's `decl-block` is its function's
  body).
- `referent-block(e)` for a borrow expression `e = &_m e'`: if `e'`'s
  place root is a binding `x` of non-reference type, `decl-block(x)`;
  if `e'`'s place root is `*r` or a projection through a reference-
  typed binding `r`, `referent-block(e) = referent-block(the
  expression that initialized r)` when that is a syntactically visible
  borrow in the same function, else `unknown`.
- A reference-valued expression `e` **escapes** to block `B` if its
  value is stored by a local declaration or assignment into a binding whose
  `decl-block` is `B`, stored into a field of an aggregate whose own
  value escapes to `B`, or is the result of the function body (then
  `B = caller`, treated as enclosing every block of the function).

    [Ref-Escape-Rejected]   disposition: rejected
        e = &_m e' with referent-block(e) = B_r ≠ unknown
        e escapes to B with B strictly lexically enclosing B_r, or B = caller
            and the referent's root binding is not a reference-typed parameter
        ────────────────────────────────────────────
        ill-formed; diag.reference-escapes-scope

Where `referent-block` is `unknown` (the reference flows through a
non-visible path — a call other than the elided case in §3, a value
read out of an aggregate, a `match` binding), no static rule applies
and §1's dynamic baseline is the enforcement (`disposition: checked`
at the use). This boundary is language-defined: it is exactly the
syntactic visibility stated above, not implementation quality
(Master Instructions §12; `spec/AUDIT-2.md` B-10 for the general
mechanism).

**Depends on:** inv.temporal-validity, type.ref, D-0005,
rule.control.flow-analysis

## 3. Interprocedural refinement: single-parameter elision (D-0011)

### `rule.temporal.elision`
**Status:** ACCEPTED

    [Call-Elided-Lifetime]
        v_f has signature (…, ref<τ,m>, …) -> ref<τ',m'> with exactly one
            reference-typed parameter, at position k
        the call's k-th argument expression is e_k
        ────────────────────────────────────────────
        for rule.temporal.ref-escape, the call expression is treated as a
        borrow expression whose referent-block is referent-block(e_k)
        (if e_k is itself `&_m e'`) or decl-block(x) (if e_k is a binding x
        of reference type initialized by a visible borrow), else unknown

    [Call-Multi-Ref-Return-Rejected]   disposition: rejected
        v_f's return type is ref<_,_>
        v_f has 0 or ≥ 2 reference-typed parameters
        ────────────────────────────────────────────
        ill-formed; diag.lifetime-elision-ambiguous

Inside such a function the returned reference is an ordinary path
derived (through the parameter's reference) from the caller's object;
D-0018's use-time checking already makes every later use sound. The
elision rule adds only the *static* escape diagnostic at the call
site, by naming which argument the result's validity depends on. It
no longer suspends anything (`spec/AUDIT-2.md`: D-0015's suspend
layering is gone). Functions with zero or several reference parameters may not return a
reference: `feat.explicit-lifetime-parameters`
(`spec/registry/features.md`) considered adding a mechanism for this
general case and was `REJECTED` (`CHG-0022`) — a deliberate permanent
boundary, not a placeholder pending a future mechanism. A caller
needing this shape composes single-reference-parameter helpers
instead. A slice (`slice<τ, m>`, `spec/16` §3a, D-0047) is a borrow
here as a reference is: it counts as a reference parameter and as a
reference result, and a slice of a local may not escape its scope.

**A confirmed trade, not a gap.** Because elision also
feeds `rule.control.flow-analysis`'s `deriv` fact (a call with exactly
one reference parameter and a reference return sets `deriv(r, x.π, m)`
the same way a direct borrow would), a call site like
`Vec::index_shared` is treated as *if* the reference had been formed by
a visible `&`/`&mut` of the argument — so a later exclusive borrow of
the same argument in the same function is refuted statically
(`conf.vec-ref-then-push-rejected`), even though the analogous access
through a reclaimed element and a raw buffer release is, dynamically,
merely a `diag.stale-binding` fault (`conf.e2e-vec-realloc-stale-ref`)
rather than a hard aliasing violation the moment the second borrow
forms. This is intended: `rule.control.flow-analysis`'s own outcome
policy states the front line is deliberately conservative (`discharge:
dynamic where unknown`, never the reverse), and rejecting the
same-function shape early is strictly safer for the programmer than
waiting for a dynamic fault that a differently-shaped call (through a
reference parameter, where `deriv` cannot be tracked at all —
`spec/14` §6) would not catch until first use. No sharper static rule
is proposed: narrowing the rejection to "only when a realloc could
actually occur" would require value-range reasoning
(`rule.control.flow-analysis`'s own stated scope excludes it beyond
literals) for no safety gain, at real cost to the conceptual economy
`rule.control.flow-analysis` already keeps (Master Instructions §9).

**Depends on:** D-0011, D-0018, rule.temporal.ref-escape, rule.fn.call

## Change Log

- 1.1.0 — `CHG-0055` (D-0047): `rule.temporal.elision` counts slices
  as references.

- 1.0.3 — Non-normative (`CHG-0022` §"Hygiene"): `rule.temporal.elision`'s
  closing note updated — `feat.explicit-lifetime-parameters` is now
  `REJECTED`, not merely recorded/undesigned; no rule changed.
- 1.0.2 — Non-normative (`CHG-0016` §"Hygiene"): §3 gains a paragraph
  confirming that `conf.vec-ref-then-push-rejected`'s static rejection
  of the same-function shape, ahead of the dynamic `diag.stale-binding`
  a differently-shaped access gets, is an intended conservative trade;
  no rule semantics changed.
- 1.0.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"): one
  reference to the retired `let` keyword in §2's definition of
  "escapes" re-worded (`CHG-0001` missed this file).
- 1.0.0 — Rewritten per D-0018 (`spec/AUDIT-2.md` B-03, B-10):
  `[Ref-Escape-Rejected]` stated over defined syntactic functions
  (`decl-block`, `referent-block`, escapes); `[Call-Elided-Lifetime]`
  reduced to its static role. All entities `ACCEPTED`.
- 0.3.0 and earlier — superseded.
