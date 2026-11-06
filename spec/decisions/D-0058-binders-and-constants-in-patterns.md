# D-0058 — A binder of the whole value, and constants, in patterns

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0056, D-0057, D-0036, rule.agg.match, rule.module.const
Affects: rule.agg.match, `spec/22` `pattern`

## Problem

Real-world testing (2026-09-27, `impl/STATUS.md`) met two gaps in D-0057's
pattern language:

- An arm could not bind the whole value. `n : Err(Fields(n))` and
  `other : other` were written three times and each replaced by a `let`
  before the `match`, since a binder existed only inside parentheses.
- A named constant could not be a pattern, so a VM's opcodes were bare
  literals with comments beside them.

## Constraints

- D-0057 closed the pattern language; neither addition may bring a form
  a reader has not already met.
- No new tokens.
- Coverage and reachability stay decidable and exact.

## Candidate mechanisms

1. **A binder alone at the top level: the whole value.** The binder
   already exists inside parentheses; this lets the same form stand at
   the top, with the same name rule. **Selected.**
2. Keep binding the whole value with a `let` before the `match`.
3. **A constant (integer or `bool`) wherever a literal may stand, found
   by name as a variant is.** A constant is already a compile-time
   value (`consts` folds it). **Selected.**
4. Constants only when qualified (`Op::PUSH`).
5. Reject a binder spelled like a visible constant.

## Selected design

Candidates 1 and 3:

    pat ::= '_' | x | lit | V | V '(' pat ')'       lit ::= … | C

- **Names.** An identifier (or qualified path) in a pattern is, in this
  order: a constant (`rule.module.const`) — its value, a literal by name;
  a variant of an enum; otherwise a binder.
- **A top-level binder** matches every value, as `_` does, and binds the
  scrutinee: by value it moves a resource (the scrutinee is consumed),
  through a reference it is that reference, through a projection a
  resource is `[Match-Move-Through-Ref]`. The scrutinee may be of any
  type when the first arm matches everything.
- **A constant** folds to its typed literal: it must be of the level's
  type exactly (`[Pattern-Type]`), and coverage and reachability use its
  value, so `PUSH` then `1` is an unreachable arm.

Candidate 5 proved unnecessary: a binder mistaken for a constant (or the
reverse) changes a catch-all into a single value, and the `match` on an
integer then lacks `_` — `diag.non-exhaustive-match` names it.

## Rejected alternatives

- **2:** a statement for what the pattern can say.
- **4:** noisy for a program's own constants, which are unqualified.
- **5:** a rule the exhaustiveness check already makes redundant.

## Semantic rationale

A binder at the top is the binder that already binds a payload, applied
to the scrutinee; a constant is a literal written by name. The closed
list is unchanged in kind.

## Usability

    match (op)
    {
        PUSH : push(vm),
        ADD : add(vm),
        other : illegal(vm, other),
    }

## Explainability

"A pattern is `_`, a name, a literal, a variant, or a variant with a
pattern for its payload; a name is a constant, a variant, or a new
binding, in that order."

## Implementation-feasibility

The parser keeps a lone name (or a qualified path) for `modres`, which
makes it a constant's call (`consts` folds it), a variant, or leaves it a
binder. The checker types a top-level binder as the scrutinee; `coby`
takes the whole value (`take_whole`); `cobc` binds the scrutinee's slot,
or, for a scrutinee that is not an enum, lowers the first arm as a flat
match (`lower_match_scalar`).

## Compatibility impact

Extension; a top-level identifier that named no variant was a type
error before.

## Prior-art status

- **Rust:** a binder pattern at any level; constants in patterns,
  resolved by name (with a lint for the binder/constant confusion).
- **OCaml, Haskell:** variables at any level; no constant patterns
  (literals only).
- **C:** `case` labels take constant expressions.

## Invariant traceability

`inv.resource-authority`: a top-level binder moves a resource out of a
whole owned or temporary value only.

## Revisit conditions

None.
