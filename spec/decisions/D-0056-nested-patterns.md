# D-0056 — Nested patterns

Status: ACCEPTED (2026-09-26, owner-delegated: "make the best decisions for CobaltC")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0006, D-0046, D-0049, rule.agg.match
Affects: rule.agg.match, `spec/22` `arm`

## Problem

A `match` arm tested one variant and bound its payload whole. Every
`read_line`, `File::read_line` included, returns
`Result<Option<String>, _>`, so reading a line took two nested
`match`es; an enum inside an enum, or an `Option` of one, took a
staircase of them. `spec/22` §5 listed "no nested patterns" as a
restriction and D-0046 as a revisit condition.

## Constraints

- No new tokens; the arm keeps its shape, `pattern ':' expr`.
- The resource rules hold at any depth: a resource is moved out only
  from a whole owned or temporary enum (`[Match-Move-Through-Ref]`), an
  arm that moves nothing leaves the scrutinee to its owner (D-0049),
  and a match through a reference binds references (D-0046).
- Exhaustiveness stays decidable from the declared variants (D-0006).

## Candidate mechanisms

Scope:

1. **Variant patterns at any depth** — `_`, a binder, `V`, `V(pattern)`.
   **Selected.**
2. And literal sub-patterns (`Some(0)`).
3. And struct patterns (`Some(Point{x, y})`).

An identifier inside parentheses:

4. **A binder, unless it names a variant of an enum** (resolution by
   name). **Selected.**
5. Decided by the payload's type.
6. Nested unit variants written qualified (`Some(Option::None)`).
7. A sigil for binders.

Arm order and unreachable arms:

8. **First match wins; an unreachable arm is rejected**
   (`diag.unreachable-arm`). **Selected.**
9. First match wins; unreachable arms accepted.

The non-exhaustive diagnostic:

10. **Names a pattern not covered** (`not covered: \`Ok(None)\``).
    **Selected.**
11. As before, without one.

## Selected design

Candidates 1, 4, 8 and 10.

- **Chains.** A variant has one payload, so a pattern is a chain
  `V1(V2(…Vk(q)…))`, `q` a binder, `_` or nothing, with at most one
  binder, which binds `Vk`'s payload. Each `Vj+1` must be a variant of
  the enum that is `Vj`'s payload type (`[Pattern-Type]`): a payload
  whose type is not an enum, a type parameter included, cannot be
  matched into, so a generic body's patterns mean the same at every
  instantiation.
- **Order.** Arms are tried in order; the first that matches is taken.
- **Coverage.** A chain is covered when an arm's chain is a prefix of
  it, or when its last variant's payload is an enum and every variant
  of that enum extends it to a covered chain. The match must cover the
  empty chain (`[Match-Non-Exhaustive]`, with the first uncovered chain
  as a pattern in the message); arm k must not be covered by the arms
  before it (`[Match-Unreachable]`, located at arm k).
- **Resources.** Binding a resource payload by value at any depth
  consumes the scrutinee; the levels above it own nothing else, since
  each variant has one payload. Through `*r` or a projection it is
  `[Match-Move-Through-Ref]`; through a reference every binder is a
  reference of its mode, derived through as many payload projections as
  the pattern has levels.

## Rejected alternatives

- **2:** matching integers is a feature of its own (exhaustiveness then
  needs `_` for every integer type); deferred.
- **3:** several binders per arm, and moving out of struct fields,
  which D-0044's destructuring restricts; deferred.
- **5:** a binder's meaning would depend on the payload's type, which
  in a generic body changes with the instantiation.
- **6:** noisy for the commonest case, `Ok(None)`.
- **7:** a new token.
- **9:** once order decides, a dead arm is a mistake the reader cannot
  see; CobaltC has no warnings, so the choice is between rejecting it
  and saying nothing.
- **11:** a nested miss is hard to find by eye; the checker has it.

## Semantic rationale

A nested pattern is the nested `match` it replaces, with the difference
D-0049 needs: nothing is moved out at an outer level, so `Ok(None)`
leaves the scrutinee where it was, as a match on `Ok(_)` would. The
coverage relation is the usual pattern-matrix usefulness check,
specialized to chains.

## Usability

    while (true)
    {
        match (File::read_line(&mut f))
        {
            Ok(Some(line)) : { … },
            Ok(None) : { break; },
            Err(Utf8(e)) : { … },
            Err(_) : { break; },
        }
    }

## Explainability

"An arm's pattern can say what the payload must be, as deep as you
like; arms are tried in order; every value must reach an arm, and every
arm must be reachable."

## Implementation-feasibility

`Arm` gains `nested` (the variants below the first); the parser reads
the chain; `modres` turns an identifier naming a variant into one; the
checker types each chain, computes coverage and reachability
(`pattern_missing`); `coby` takes the first arm whose chain the value's
variants begin with (`arm_matches`) and binds through that many payload
projections; `cobc` lowers an arm to a conjunction of tag tests down
the payload slots (`arm_levels`).

## Compatibility impact

Source-breaking for a program with an unreachable arm (a second arm for
a variant, or an arm after `_`), which was accepted and never taken; no
program in the repository had one. A binder spelled as a variant name
(`Some(None)` meaning a binding named `None`) now tests the variant.

## Prior-art status

- **Rust:** nested patterns; first match wins; unreachable arms are a
  warning; non-exhaustive errors list missing patterns
  (`Ok(None)` not covered).
- **OCaml / Haskell:** the pattern-matrix usefulness algorithm this
  specializes; unused match cases warned.
- **Swift:** nested enum patterns; unreachable cases warned.

## Invariant traceability

`inv.resource-authority`: a nested resource binder moves the payload
out of a whole owned or temporary enum only, and consumes it.
`inv.alias-validity`: a binder through a reference is a derived
reference.

## Revisit conditions

- Literal sub-patterns, and matching integers.
- Struct patterns; several binders per arm.
- Or-patterns (`Ok(None) | Err(_)`).
