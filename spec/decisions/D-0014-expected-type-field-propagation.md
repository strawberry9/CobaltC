# D-0014 — Expected-Type Propagation Into Struct/Enum Literal Fields

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §11
Depends on: D-0012, D-0013 (extends, does not revise — both `ACCEPTED`,
neither edited, per `spec/02-schema.md` §6)

## Problem

Found while re-verifying the conformance suite for staleness (not by
new design work): `conf.generic-struct-monomorphize-resource`
(`spec/conformance.md` §8) exercises `let c: Box<Vec<i32>> =
Box{value: Vec::new()};` — for this to typecheck, the field `value`'s
expected type (`Vec<i32>`, from `Box<Vec<i32>>`'s own, already-known
instantiation) must propagate into the `Vec::new()` sub-expression so
D-0013 can infer `new`'s `T`. Neither D-0012 §3b's nor D-0013's stated
expected-type source list (`let` annotation, enclosing parameter/return
type, other operand in one arithmetic expression) includes "a struct or
enum literal's field position" — so, as written, this conformance case
was itself unverifiable against the rules, the same class of problem
D-0013 was created to fix for `Vec::new()` at top level.

## Constraints

Same as D-0012/D-0013: local, syntax-directed only, no cross-statement
propagation; must not reintroduce global inference.

## Candidate mechanisms

1. **Do not propagate; require explicit instantiation in every field**
   (`Box{value: Vec::<i32>::new()}`). Sound, consistent with D-0012/
   D-0013 as written, but reintroduces exactly the unforced burden
   those decisions exist to remove — worse here, since the enclosing
   `Box<Vec<i32>>` *already* states the field's type in full; requiring
   it a second time at the field literal is pure redundancy, not a case
   where the type is genuinely ambiguous.
2. **Propagate the enclosing construction's (already-known) field type
   as the expected type for each field's initializer expression.**
   **Selected.**

## Selected design

    [Field-Expected-Type]
        struct/enum literal Name<σ1,...,σn>{ f1: e1, ..., fk: ek }
            OR Name{...} where Name's type is itself already the
            expected type at this literal's own position (D-0012 §3b/
            D-0013 applied one level up — e.g. this literal is itself
            the argument to Ok(...) whose Result<T,E> is already known)
        field fi's declared type is τi (after substituting σ1..σn, or
            the outer expected type's own instantiation)
        ────────────────────────────────────────────
        ei is checked with expected type τi (D-0012 §3b / D-0013,
        recursively — a field's own initializer may itself be a nested
        struct/enum literal or generic call needing this same rule)

This is additive to D-0012 §3b's/D-0013's expected-type source list,
not a new inference *direction* — the same "checking against a known
type" principle, now recognized as recursing through literal fields the
way it already recurses through nothing else, because nothing else
nests an unresolved generic call inside an otherwise-fully-typed
construction. `Box<Vec<i32>>{value: Vec::new()}` now infers `new`'s
`T := i32` from `value`'s declared field type, itself already
`i32`-instantiated because `Box<Vec<i32>>` was explicit.

## Rejected alternatives

Requiring explicit instantiation at every field regardless (1).

## Semantic rationale

Once a struct/enum literal's own type is fully known — either because
it was explicitly instantiated, or because it is itself checked against
an outer expected type — every field's type is *already* a fixed,
concrete fact, not something to infer. Not propagating it to the
field's initializer would mean re-deriving information the type
checker already has, for no reason.

## Usability / Explainability / Implementation-feasibility

Usability: nested generic constructors inside a fully-typed literal
need no annotation of their own. Explainability: the inferred type is
still traceable to one nearby, already-fixed fact (the enclosing
literal's own type), never a distant one. Implementation-feasibility:
ordinary top-down recursion through an already fully-typed construction
tree; no new solving.

## Compatibility impact

None yet — retroactively makes `conf.generic-struct-monomorphize-
resource` (`spec/conformance.md`) actually well-formed under the rules,
rather than leaving it unverifiable. D-0012/D-0013 unedited.

## Prior-art status

Recursive expected-type propagation through a literal's fields is the
direct, minimal extension of D-0012's/D-0013's own already-accepted
checking principle — not a new technique.

## Invariant traceability

None — surface ergonomics only, same as D-0012/D-0013.

## Revisit conditions

None anticipated.
