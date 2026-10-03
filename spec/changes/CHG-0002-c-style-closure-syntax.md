# CHG-0002 — C++11-Style Closure Capture-List Syntax

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §21

## Problem / motivation

`spec/22` 2.0.0's closure syntax (`move? |params| expr`) was the one
Rust-shaped construct `CHG-0001`'s C-style pass left untouched. The
human owner directed a further revision to C++11 lambda-style
delimiters — a capture list in `[ ]`, parameters in `( )`, a brace
body — without changing capture semantics.

## Affected entities

No `rule.*`, `inv.*`, `type.*`, `state.*`, `term.*`, or `D-XXXX`
entity's capture-mode inference, ownership/move behavior, borrowing,
lifetime, resource-handling, or type-inference semantics changed.
`rule.fn.closure` (`spec/15` §6) gains exactly one new static
well-formedness check (below); everything else in its definition is
unchanged. Also touched: `spec/22-surface-syntax.md` (2.1.0, the
grammar and a new disambiguation), `spec/registry/diagnostics.md`
(1.2.0, one new entry), `spec/examples.md` (3.1.0), `spec/
conformance.md` (3.1.0) — all re-spelling closure fragments to the new
syntax, plus (found while re-spelling `[Callable-Fn]`, the closure
section's own neighbor) a residual gap in `06-arithmetic.md` and
`12-type-system.md` that `CHG-0001`'s sweep missed: `fn(...)`-shaped
type illustrations still spelled their return type with `->`. Fixed
here as the same class of correction, both files bumped to 1.1.0.
`spec/decisions/D-0017-retroactive-derivation-records.md` was checked
and needs no change — its closure section describes the capture-mode
mechanism in prose, with no literal syntax quoted.

## Previous semantics (concrete syntax; no rule semantics involved)

`closure ::= move? '|' params? '|' expr` — captures were implicit
(every free variable, automatically), with no way to write them down;
the body could be a bare expression or a block.

## New semantics (concrete syntax, plus one new checked fact)

`closure ::= move? '[' captures? ']' '(' params? ')' block`, with
`captures ::= identifier (',' identifier)*`. The set of captured
variables and each one's mode are derived exactly as before (D-0017's
AST scan) — the written capture list does not select, narrow, or
widen that set. What's new: the list must equal the derived set
exactly, checked by a new rule, `[Closure-Capture-List-Rejected]`,
reporting a new diagnostic, `diag.capture-list-mismatch`, on mismatch.
The closure body is now always a brace block (a pure grammar
narrowing — block was already a legal closure-body expression, so no
previously-expressible closure becomes inexpressible; `{ e }` says
what bare `e` used to say).

## Affected invariants

None. The new check is a syntactic well-formedness condition on the
closure literal itself (comparing two compile-time-derivable sets of
names), not a condition `spec/03-invariants.md` registers or any
existing invariant's enforcement path.

## Dependency impact

`rule.fn.closure`'s `Depends on`/`Affects` lines are unchanged (no new
state component, no new invariant). `spec/registry/diagnostics.md`
gains one new `Depends on: rule.fn.closure` entry for
`diag.capture-list-mismatch`.

## Compatibility classification

Source-breaking, semantics-preserving, same shape as `CHG-0001`: every
closure literal must be re-spelled, but no closure's *behavior*, once
re-spelled, differs from before — provided the capture list is written
to match the free-variable set the body already implied, which is now
mandatory rather than merely true.

## Migration implications

Every closure literal in existing CobaltC source must be re-spelled to
the new grammar, and its capture list populated with exactly its free
variables. There is no automated migration guarantee (same caveat as
`CHG-0001`): computing "the free variables of this closure body" is
mechanical, but the record does not certify a specific tool did it
correctly for any given corpus.

## Example changes

`ex.closure-capture` (`spec/examples.md`) re-spelled; one new mistake
case added (`diag.capture-list-mismatch`) to the same example, since
the category ("canonical / common mistake") already covers exactly
this shape of addition.

## Conformance changes

`conf.closure-borrow-capture`, `conf.closure-move-capture-invalidates-
source`, `conf.closure-drop-through-self-rejected`, `conf.closure-owns-
moved-resource`, and `conf.spawn-borrow-closure-rejected` re-spelled;
one new case added, `conf.closure-capture-list-mismatch-rejected`.
Every other row's fragment, outcome, and derivation are untouched.

## Future implementation implications

An implementation must compute a closure body's free-variable set
(already required for capture-mode inference, D-0017) and additionally
compare it against the written capture list at the same point — no new
analysis class, one new comparison against a fact already computed.
The `[` shared between array literals and closures (`spec/22` §2
disambiguation (7)) requires the same bounded-lookahead-past-a-closing-
delimiter technique the grammar already uses for generic-call-vs-
comparison; no new parsing technique is required.

## Prior-art status

Not applicable in the D-XXXX sense; a surface-syntax preference change
directed by the human owner, continuing `CHG-0001`'s redesign.

## Revisit conditions

None beyond `CHG-0001`'s own: a further syntax change layered on top
of this one is a new Change record, not an edit to this one.
