# CobaltC Temporal Validity

Status: normative artifact
Version: 1.7.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.temporal.*`)
Governed by: `CobaltC_Master_Instructions.md` §17 (Temporal Validity),
§23
Realizes: D-0005 (two-layer checking), D-0011 (elision), D-0018, D-0312 (the
`from` clause)

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
  (`rule.control.block`); a local reference, slice or `StringView`
  binding's, after its last use (`[Ref-Binding-Last-Use]`, D-0111).
- A static's object, and an addressed constant's (`spec/17` §1a′, §1a,
  D-0258), never ends: a path to it is invalidated by nothing, and a
  reference to it escapes nothing (§2's `referent-block` is the program).

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
- A closure without `move` is such an aggregate: `[Closure-Form-Borrow]`
  (`spec/15`) stores `&_{m_i} x_i` for each capture `x_i` in a field, so
  each capture of non-reference type is a borrow expression with
  `referent-block = decl-block(x_i)`, escaping wherever the closure's
  value does (`CHG-0113`).

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

## 3. Interprocedural refinement: elision and the result's sources (D-0011, D-0312, D-0318, D-0319)

### `rule.temporal.elision`
**Status:** ACCEPTED

    [Call-Elided-Lifetime]
        v_f has signature (…, ref<τ,m>, …) -> τ_r, where τ_r is a reference
            type or may hold a reference (D-0088, D-0319), and its source
            parameters are at positions K: the parameters its `from` clause
            names (D-0312, D-0318), else its one reference-typed parameter
        for each k in K, the call's k-th argument expression is e_k, and
            b_k is referent-block(e_k) (if e_k is itself `&_m e'`) or
            decl-block(x) (if e_k is a binding x of reference type
            initialized by a visible borrow), else unknown
        ────────────────────────────────────────────
        for rule.temporal.ref-escape, the call expression is treated as a
        borrow expression whose referent-block is the innermost of the
        known b_k (they nest: every e_k is visible at the call), else
        unknown; for rule.control.flow-analysis, as a borrow of each e_k

    [Call-Multi-Ref-Return-Rejected]   disposition: rejected
        v_f has no `from` clause, and either
            v_f's return type is a reference type and v_f has 0 or ≥ 2
            reference-typed parameters, or
            v_f's return type may hold a reference without being one and
            v_f has ≥ 2 reference-typed parameters (D-0319)
        ────────────────────────────────────────────
        ill-formed; diag.lifetime-elision-ambiguous

    [Result-Source-Form]   disposition: rejected
        v_f is declared `fn f(…) : τ from p_1, …, p_n`
        some p_i is not a parameter of v_f whose declared type is a
            reference type, or p_i = p_j for some j < i, or τ may hold
            no reference
        ────────────────────────────────────────────
        ill-formed; diag.result-source-invalid

    [Result-Source-Probe-Only]   disposition: rejected
        v_f is declared `fn f(…) : τ from p_1, …, p_n`
        q ∉ {p_1, …, p_n} is a parameter of v_f whose type (in the
            instantiation checked; a type parameter counts) may hold a
            reference
        an occurrence of q in v_f's body is not probed
        ────────────────────────────────────────────
        ill-formed; diag.result-source-kept

Here a *reference type* is a `ref`, a slice or a `StringView` (D-0047,
D-0053), and a type *may hold a reference* when it is one or holds one
in a field, an element, a payload, a type argument or a closure's
captures (`Vec<StringView>`, `Option<ref<T, shared>>`).

**The `from` clause** (D-0312, D-0318). `fn f(…) : τ from p` names the
parameter the result's references come from, and `from p, q, …` the
parameters they may come from, any of them, as the run decides. It is
the one way to return a reference from a function with several
reference parameters: `[Call-Elided-Lifetime]` uses the named
parameters where it would use the one, so the call borrows each named
argument while the result lives, and the result lives no longer than
the innermost of them. So is it for a result that holds references
without being one (an `Option<ref<T, shared>>`, a `Vec<StringView>`, a
struct with a reference field): with several reference parameters it
names its sources too (D-0319), and its call borrows them. Every
parameter the clause does not name whose
type may hold a reference (a type parameter counts, in every
instantiation) may only be *probed*, read for a value that holds no
reference of it:

- An occurrence of q is *derived* by the smallest expression around it
  when that expression is `(e)`, `e.f`, `e[i]`, `*e`, a borrow `&e` or
  `&e[lo .. hi]` of a place under it, or a call taking it as an argument
  whose result's type may hold a reference; and the derived expression is
  derived in turn. An occurrence is *probed* when the first expression
  around it that is not a derivation is a *probe*: the operand of an
  operator; a field, element or referent read whose type holds no
  reference; or an argument of a call of a named function whose result's
  type (in the type arguments the call writes, where it writes them:
  `StringView::parse<usize>(q)`) holds no reference, or whose own `from`
  clause does not name that parameter, and none of whose parameters
  other than the argument's own can receive one (the one that may
  receive references is given them, and does not keep q: `Vec::push(q,
  i)`; CHG-0439).
- A parameter *can receive* a reference when its type holds an exclusive
  reference or slice to a type that may hold one; a raw pointer to one,
  by value or behind an exclusive reference; a `mutex`, a guard, a
  `Channel` or an `AtomicPtr` whose content may hold one, behind any
  reference; or a `fn` value or a closure, which may be called with it.
- Anything else keeps q: a binding's initializer, either side of an
  assignment, a `return` or a body's result, a part of an aggregate
  literal, the scrutinee of a `match` or a `foreach`, a capture, a
  `spawn`'s argument, an argument of a call through a `fn` value. A
  kept occurrence is `[Result-Source-Probe-Only]`.

So no reference of q reaches the result, or anything the caller keeps,
and the caller may use q's argument as soon as the call returns. The
rule is a syntactic check of one body, in each instantiation, with no
lifetime to infer; it is deliberately conservative (a body that keeps
q in a local and then only probes the local is rejected, and so is a
generic body keeping a `T`, which may be a view). Without the
clause a function returning a reference takes exactly one reference
parameter, and one returning a value that holds references at most one
(D-0319). `StringView::split(StringView v, StringView sep)
: Vec<StringView> from v` is the standard library's use: the
separator is any text, and the parts are views of `v` alone. A clause
names each parameter once, and may name every reference parameter:
`fn longer(ref<String, shared> a, ref<String, shared> b) :
ref<String, shared> from a, b` returns either, and its caller can
change neither argument while the result is used.

Inside such a function the returned reference is an ordinary path
derived (through the parameter's reference) from the caller's object;
D-0018's use-time checking already makes every later use sound. The
elision rule adds only the *static* escape diagnostic at the call
site, by naming which argument the result's validity depends on. It
no longer suspends anything (`spec/AUDIT-2.md`: D-0015's suspend
layering is gone). Functions with zero reference parameters may not return a
reference, and functions with several only with a `from` clause:
`feat.explicit-lifetime-parameters` (`spec/registry/features.md`),
lifetime parameters relating several inputs to a result, stays
`REJECTED` (`CHG-0022`); the clause names one set of sources for the
whole result and needs no lifetime (`feat.result-source-clause`,
D-0312, D-0318). A result whose parts come from different sources (a
pair of references, one from each argument) is not expressible; a
caller needing that shape composes single-source helpers. A slice (`slice<τ, m>`, `spec/16` §3a, D-0047) is a borrow
here as a reference is: it counts as a reference parameter and as a
reference result, and a slice of a local may not escape its scope.

**A confirmed trade, not a gap.** Because elision also
feeds `rule.control.flow-analysis`'s `deriv` fact (a call that returns
a reference from its one reference parameter, or from those its `from`
clause names, sets `deriv(r, x.π, m)` for each
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

**Depends on:** D-0011, D-0018, D-0088, D-0312, D-0318, D-0319, rule.temporal.ref-escape, rule.fn.call

## Change Log

- 1.7.0 — `CHG-0439` (D-0319): `[Call-Multi-Ref-Return-Rejected]` applies
  to a result that holds references without being one, from two or more
  reference parameters; `[Call-Elided-Lifetime]` reads such a result's
  clause (it took the first reference argument, whatever the clause
  named). `[Result-Source-Probe-Only]`'s probe reads a call's written
  type arguments, and the parameter that may receive references does
  not keep the argument given it. Restrictive for a holder result with
  no clause; additive otherwise.
- 1.6.0 — `CHG-0437` (D-0318): a `from` clause may name several
  parameters: `[Call-Elided-Lifetime]` borrows each named argument and
  takes the innermost referent-block, `[Result-Source-Form]` rejects a
  name given twice, `[Result-Source-Probe-Only]` probes the parameters
  it does not name. Additive.
- 1.5.0 — `CHG-0413` (D-0312): the `from` clause names a result's source
  parameter: `[Call-Elided-Lifetime]` uses it, `[Call-Multi-Ref-Return-Rejected]`
  does not apply, `[Result-Source-Form]` and `[Result-Source-Probe-Only]`.
- 1.4.0 — `CHG-0320` (D-0258): §1 a static's object never ends.
- 1.3.0 — `CHG-0131` (D-0111): a local reference binding's object ends
  after its last use.
- 1.2.0 — `CHG-0113`: `rule.temporal.ref-escape` states that a closure
  without `move` stores a borrow of each capture (clarification; the
  rule already covered it through `[Closure-Form-Borrow]`).
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
