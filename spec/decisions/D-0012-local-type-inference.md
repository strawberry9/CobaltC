# D-0012 — Local Type Inference (Literals, `let`, Generic Call Arguments)

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §11, §12
Depends on: D-0006, D-0010, `spec/12-type-system.md` §5

## Problem

Three places in the specification left an annotation *syntactically*
optional without ever stating what determines the type when it is
omitted: an integer literal's suffix (`spec/06` §6), a `let` binding's
type (`spec/12` §5's static typing judgment, left abstract), and a
generic function call's type arguments (D-0010: "explicit instantiation
only," no mechanism given for when the arguments already determine
them). Surfaced by writing example programs and being asked directly
why the surface syntax reads as more annotation-heavy than the grammar
requires. Master Instructions §8 places human comprehensibility (7th)
and conceptual economy (6th) above syntactic convenience (13th) but
*below* invariant preservation, safety, and formal precision — an
annotation burden with no corresponding invariant benefit is exactly
the kind of cost this priority order says not to pay.

## Constraints

- §9: prefer the smallest sufficient mechanism. Full, global,
  constraint-propagating (Hindley-Milner-style) inference — where a
  variable's type can be determined by looking at uses arbitrarily far
  from its declaration — is a large mechanism with real complexity
  cost (principal-type computation, deferred solving). Nothing in this
  specification has demonstrated a need for it.
- D-0006: no implicit conversion. Whatever inference is added must
  still produce one concrete, fixed type per expression — inference
  is about *finding* that type, never about silently coercing between
  two different ones.
- Must not weaken D-0010's core rationale (keep `is-resource` and
  instantiation unambiguous, static, per-declaration) — inference must
  still resolve to a single, statically-fixed monomorphization per call
  site, just without requiring it be spelled out when redundant.

## Candidate mechanisms

1. **Global, constraint-based inference** (infer a `let` binding's type
   from *any* of its uses anywhere in scope, Hindley-Milner-style).
   Rejected: large mechanism, unforced by any concrete requirement;
   works against §9. Not excluded forever — if a genuine need surfaces,
   it would need its own derivation, not a quiet expansion of this one.
2. **No inference; explicit annotation always required** (the status
   quo this decision replaces). Rejected: exactly the unjustified
   burden the problem statement identifies — redundant annotation with
   no invariant-preservation benefit.
3. **Local, syntax-directed inference**: three narrow, independently
   simple sub-rules, each determined entirely by information already
   available at the same syntactic point — no cross-statement
   propagation, no global solving. **Selected.**

## Selected design

Three sub-rules, together "local type inference":

- **3a. `let` type synthesis** (`spec/12-type-system.md` §5): `let x =
  e;` with no `: τ` infers `x`'s type as `e`'s own synthesized type
  (`Γ ⊢ e : τ`, already computed bottom-up by existing rules regardless
  of `let`). Direction: expression → binding.
- **3b. Literal checking against an expected type, with defaulting**
  (`spec/06-arithmetic.md` §6): an integer literal with no suffix takes
  its type from an immediately available expected-type source — a
  `let` type annotation, an enclosing function parameter/return type, or
  (within one arithmetic/comparison expression) the other operand's
  already-determined type. If no such source exists at that point, the
  literal **defaults to `i32`** — a fixed, predictable, always-available
  choice, not a silent guess among several; a later use requiring a
  different type is then an ordinary type error (D-0006: no implicit
  conversion bails it out), not a special inference failure. Direction:
  context → literal.
- **3c. Generic call argument-directed unification** (`spec/15-function-
  semantics.md` §5, revises D-0010's explicit-only stance for this one
  case): for `name(e1,...,en)` where `name`'s declared parameter `pi`
  has type exactly `Ti` for one of the function's type parameters, and
  `ei` synthesizes type `σi`, set `Ti := σi`. If `Ti` appears in more
  than one parameter position, all corresponding `σi` must be
  identical (equality, not unification-with-substitution — no
  compound-type deconstruction is needed for this rule, since it only
  ever matches a *bare* type parameter against a fully-synthesized
  concrete argument type). A type parameter that appears in **no**
  parameter position (only in the return type) **cannot** be inferred
  this way — explicit instantiation remains required for it,
  unchanged from D-0010. Direction: arguments → type parameters.

`spec/16-aggregates.md` §5's existing "construction against a known
expected type" rule (added while resolving the example-program review)
is recognized here as a fourth instance of the same *checking* direction
as 3b, not a separate mechanism — this decision retroactively supplies
its general justification.

## Rejected alternatives

Global constraint-based inference (1, not excluded forever, just
unforced); no inference at all (2, the prior state).

## Semantic rationale

All three sub-rules share one shape: the type is already, unambiguously
determined by something the compiler is looking at *at the same point*
— nothing requires searching forward or backward through a scope, or
solving a system of constraints. This is what keeps the mechanism small
(§9) while removing exactly the annotation burden that had no
invariant-preservation justification (§8's priority order).

## Usability / Explainability / Implementation-feasibility

Usability: `let x = 5;`, `identity(5)`, `Ok(a/b)` all now typecheck
without annotation, matching how this specification's own examples had
already been informally written. Explainability: every inferred type
has a single, locally-visible reason (D-0010's rejected
alternative was "not explicit" — this decision's inferred cases are
each still traceable to one specific nearby syntactic fact, never a
distant one). Implementation-feasibility: none of the three sub-rules
requires unification with substitution into compound types, backtracking,
or deferred constraint solving — each is a direct lookup against
already-computed information.

## Compatibility impact

Narrows D-0010's "no inference" claim to apply only to generic type
parameters that appear in no parameter position — recorded as a
refinement of D-0010, not a reversal of its core (monomorphization,
no trait/bound system) rationale. `spec/06` §6, `spec/12` §5, `spec/15`
§5 are revised accordingly (all still `PROVISIONAL`; in-place
revisions).

## Prior-art status

Bidirectional (checking + synthesis) local type inference and
argument-directed generic instantiation are both well-established,
independently well-understood techniques; adopted here because they are
the minimal mechanism satisfying §9 given the concrete annotation-burden
problem identified, not for familiarity.

## Invariant traceability

None directly — this decision is about surface ergonomics, not safety;
it does not change what is safe, only what must be written to say it.
D-0006 (no implicit conversion) bounds it: inference always resolves to
one fixed type, never silently reconciles two different ones.

## Revisit conditions

Revisit toward global inference only if a concrete case is found where
local, syntax-directed information is insufficient and the resulting
annotation burden is demonstrated to be a real problem, not merely
possible in principle.
