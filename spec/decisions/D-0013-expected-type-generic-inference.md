# D-0013 — Expected-Type-Directed Generic Call Inference

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §11
Depends on: D-0012 (extends, does not revise — see `spec/02-schema.md`
§6: `D-0012` is `ACCEPTED` and is not edited in place)

## Problem

Asked directly what `let nums = Vec::new(); push(&mut nums, 10); ...`
infers `nums`'s element type as. Tracing it against D-0012 §3c
(argument-directed generic call inference) found it **cannot** be
inferred: `new`'s signature is `fn new<T>() -> Vec<T>` — `T` appears
only in the return type, exactly `[Generic-Call-Uninferable]`'s
rejection case, since `new` takes no arguments at all to read `T` off
of. The example given in the previous response was, as written, ill-
formed under the specification's own rules — an error, not a
simplification. (Separately, `Vec::new` had never actually been given a
formal signature anywhere in `spec/21-standard-library-semantics.md` —
fixed alongside this decision, not a consequence of it.)

## Constraints

Same as D-0012: local, syntax-directed only; no cross-statement flow
analysis; must not reopen full global inference (§9).

## Candidate mechanisms

1. **Leave it rejected** — require explicit instantiation
  (`Vec::<i32>::new()`) whenever no argument determines a type
  parameter. Sound, consistent with D-0012 as written, but rejects the
  single most common generic-constructor idiom in the language
  (`Vec::new()` assigned to an annotated binding, or returned from a
  function) for no invariant-preservation reason — exactly the
  unjustified-burden problem D-0012 was created to fix, recurring in
  the one place D-0012 didn't yet reach.
2. **Expected-type-directed inference for the residual case**: when
   argument-direction (D-0012 §3c) leaves some type parameters
   undetermined, and a statically-known expected type is available at
   the call's syntactic position (a `let` annotation, an enclosing
   parameter/return type — the *same* sources D-0012 §3b already reads
   for literals), structurally match the function's return-type schema
   against that expected type to solve for the remaining parameters.
   **Selected.**

## Selected design

    [Generic-Call-Expected-Type]
        name(e1,...,em), type params T1..Tn
        T_args ⊆ {T1..Tn} determined by [Generic-Call-Inferred] (D-0012 §3c)
        T_rest = {T1..Tn} \ T_args, non-empty
        an expected type τ_expected available (same sources as D-0012 §3b)
        return-type schema τr, with T_rest substituted by T_args's results,
            structurally matches τ_expected at the same type-constructor
            shape (e.g. Vec<T> against Vec<i32>, same arity, same
            constructor) — a single top-level shape match, not recursive
            unification into arbitrarily nested positions
        ────────────────────────────────────────────
        remaining T_rest solved from the matching positions in τ_expected

    [Generic-Call-Uninferable]  (spec/15 §5, revised: now the residual
                                    case after BOTH D-0012 §3c and this
                                    decision are tried, not just §3c)
        some Ti determined by neither
        ────────────────────────────────────────────
        ill-formed; diag.cannot-infer-type-parameter

`let nums: Vec<i32> = Vec::new();` now infers `T := i32` for `new`
(expected type `Vec<i32>` matches return schema `Vec<T>` at the `Vec`
constructor, position-for-position). `let nums = Vec::new();` (no
annotation at all) is **still** rejected — there is no expected type
anywhere for this rule to read, and D-0012 deliberately does not look
at the later `push` calls to retroactively determine it. This is the
correct, honest boundary of "local": the annotation moves from every
element push (what the flawed first attempt implied) to exactly one
place, the binding's own declaration — not to zero places.

## Rejected alternatives

Leaving the residual case rejected (1) — recognized as an inconsistency
with D-0012's own stated purpose, not a stable position.

## Semantic rationale

This is D-0012 §3b's checking principle (context supplies a type,
propagate it inward) applied to one more syntactic position (a generic
call's return type) that D-0012's first version didn't cover. It adds
no new inference *direction* beyond what D-0012 already established —
checking against a locally-visible expected type — only a new place
that principle is recognized, keeping the mechanism's total shape
unchanged.

## Usability / Explainability / Implementation-feasibility

Usability: `Vec::new()` now needs exactly one annotation, at the
binding it's assigned to, matching ordinary idiomatic use in every
language with comparable generics. Explainability: the inferred type is
still traceable to one specific, nearby annotation — never a distant
or aggregated one. Implementation-feasibility: a single top-level
shape match (`Vec<_>` against `Vec<i32>`) is not full unification;
no substitution into arbitrarily nested compound types is required
since CobaltC's generic constructors are not themselves parametrized by
other generic constructors in this specification.

## Compatibility impact

Revises `[Generic-Call-Uninferable]`'s premise in
`spec/15-function-semantics.md` §5 (still `PROVISIONAL`, in-place);
defines `Vec::new` for the first time in
`spec/21-standard-library-semantics.md` §2 (also still `PROVISIONAL`).
D-0012 itself is unedited, per `spec/02-schema.md` §6.

## Prior-art status

Return-type-directed / expected-type generic inference for constructor
calls is well-established elsewhere; adopted here as the minimal
extension of D-0012's own already-accepted principle, not for
familiarity.

## Invariant traceability

None directly — surface ergonomics only, same as D-0012.

## Revisit conditions

Revisit if a case is found where neither argument- nor expected-type-
direction suffices and is demonstrated to be a real, recurring burden —
the expected resolution would be a further-scoped extension, not a
reversal.
