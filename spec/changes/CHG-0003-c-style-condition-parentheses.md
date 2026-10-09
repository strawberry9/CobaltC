# CHG-0003 — Mandatory Parentheses Around `if`/`while`/`match` Conditions

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §21

## Problem / motivation

`spec/22` 2.1.0's `if`, `while`, and `match` did not require
parentheses around their condition/scrutinee (`if c b`, `while c b`,
`match e { … }`) — Rust/Go-style, not C-style. This was the most
visually pervasive Rust-flavored construct remaining after `CHG-0001`
and `CHG-0002`, present in nearly every function body, and had not
been identified in either earlier pass. The human owner directed
adding mandatory parentheses, matching C's `if (c)`/`while (c)`/
`switch (e)`.

## Affected entities

No `rule.*`, `inv.*`, `type.*`, `state.*`, `term.*`, or `D-XXXX`
entity's semantics changed. Directly touched: `spec/22-surface-syntax.
md` (2.2.0, grammar and disambiguation); `spec/14-control-flow.md`
(1.2.0, `[If-*]`/`[While-*]`/`[Break]`/`[Continue]`'s reduction
subjects); `spec/12-type-system.md` (1.2.0, `[T-If]`/`[T-While]`);
`spec/16-aggregates.md` (1.1.0, `match`'s grammar line and
`[Match]`/`[Match-Conflict]`'s reduction subjects). These four are the
constructs' own formal definitions, not illustrative asides — the
same class of fix `CHG-0001` made to `rule.init.let`. Cascaded (below)
to every illustrative fragment elsewhere.

## Previous semantics (concrete syntax; no rule semantics involved)

`if c b (else ...)`, `while c b`, `match e { … }` — condition/
scrutinee unparenthesized, disambiguated from a struct-literal
condition by the removed disambiguation (1) ("parenthesize it" as an
author's manual workaround).

## New semantics (concrete syntax; no rule semantics involved)

`if (c) b (else ...)`, `while (c) b`, `match (e) { … }` —
parentheses now mandatory, every reduction/typing rule's subject
restated to match. This is not purely cosmetic: it eliminates the
ambiguity the removed disambiguation (1) existed to work around
(a struct literal directly following `if`/`while`/the bare `match`
scrutinee position could not, before this change, be told apart from
that construct's opening `{`). With the condition always delimited by
`(...)`, no such rule is needed — `spec/22` 2.2.0 has one fewer
disambiguation case than 2.1.0, not the same case renumbered.
Disambiguations (2)-(7) in 2.1.0 are renumbered (1)-(6) in 2.2.0.

## Affected invariants

None. `if`/`while`/`match`'s evaluation order, typing, and every
invariant they participate in (control flow, temporal validity via
`rule.control.block`/`stmt`, resource consumption via `[Match]`) are
unchanged; only the token sequence that names the condition/scrutinee
changed.

## Dependency impact

None. No `Depends on`/`Affects` line changes anywhere — the four
directly-touched rule files' own dependency lists are untouched, since
no dependency relationship changed.

## Compatibility classification

Source-breaking, semantics-preserving, same shape as `CHG-0001`/
`CHG-0002`: every `if`, `while`, and `match` in existing CobaltC
source must be re-spelled with parentheses; no program's meaning,
once re-spelled, changes.

## Migration implications

Mechanical: wrap every `if`/`while` condition and `match` scrutinee in
`( )`. Unlike the capture-list check `CHG-0002` added, this change
introduces no new fact a program must additionally satisfy — it is a
pure re-delimiting, so a syntax-directed rewrite (insert parens around
the existing condition expression, verbatim) is sound for every
program that was accepted under 2.1.0's grammar.

## Example changes

Every `if`/`while`/`match` fragment in `spec/examples.md` (3.2.0) and
every `if`/`while`/`match` fragment in `spec/conformance.md` (3.2.0)
re-spelled with mandatory parentheses. No `ex.*`/`conf.*` id,
category, expected outcome, or Depends-on link changed. `spec/
21-standard-library-semantics.md` (2.1.0) likewise — `Vec::push`,
`Vec::pop`, `Vec::grow`, `Vec::drop`, and `String::from_utf8` all use
`if`/`while` internally.

## Conformance changes

Covered above — every affected row's fragment re-spelled, outcome and
derivation untouched.

## Future implementation implications

None beyond ordinary parsing: mandatory parentheses are strictly
easier to parse than the 2.1.0 grammar, since the removed
disambiguation (1) required lookahead past the condition to decide
whether a leading `{` started a struct literal or the construct's own
body; that lookahead is no longer needed.

## Prior-art status

Not applicable in the D-XXXX sense; a surface-syntax preference change
directed by the human owner, continuing `CHG-0001`/`CHG-0002`'s
redesign.

## Revisit conditions

None beyond `CHG-0001`'s own: a further syntax change layered on top
of this one is a new Change record, not an edit to this one.
