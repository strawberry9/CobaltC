# D-0224 — Struct patterns, where a binder could stand

Status: ACCEPTED (2026-10-13; the owner's answer to Q5 of the 2026-10-12 plan, "accept", confirmed 2026-10-13)
Kind: Decision (`spec/02-schema.md`); a language extension
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0044 and D-0104 (destructuring), D-0056 (nested patterns), D-0046 (match by reference), D-0115
(`if`/`while` patterns)
Affects: `spec/22` §2 and §5 (2.47.0), `spec/16` §4 (1.27.0), the parser (`impl/src/parser.rs`), the checker
(`impl/src/typecheck.rs`); seven cases (CHG-0267)

## Problem

A payload that is a struct is taken apart in two steps: `if (Some(q) = ask()) { Question { name, end } = q; … }`.
Round-9 programs and the guide's own examples wrote the step again and again; destructuring already exists
for a declaration, but not where a pattern binds.

## Options

1. **A struct pattern where a binder could stand, with destructuring's rules.** `Some(Question { name, end })`,
   `Ok(Pt { x, .. })`, `Pt { x, y : h }` alone; every field named once or `..`; a field renamed with `: name`;
   fields taken by value. Selected (Q5).
2. Also a pattern per field (`Pt { x : 0, y }`), refutable: a pattern becomes a tree, and the exhaustiveness
   check, both evaluators and the compiler's match lowering, built on a variant chain, change with it.
   Not now.
3. Decline, and record the absence. Rejected (Q5).

## Decision

- Grammar: `pattern ::= … | path '{' dfield (',' dfield)* (',' '..')? '}' | path '{' '..' '}'` (`dfield` as for
  a destructuring declaration), allowed where a binder is: innermost in a variant chain, or the whole pattern.
- Meaning: the pattern binds the value to a hidden name and the arm (or the `if`/`while` body) begins with
  `Name { … } = it;` (`spec/11` `[Let-Destructure]`). So every field is named once or `..` ends the list, a field
  may be renamed, a resource field moved out consumes the scrutinee as a binder of a resource payload does,
  and for exhaustiveness the pattern is a binder.
- Through a reference (`match (&o)`, a reference scrutinee): `diag.type-mismatch`, with a hint (bind the
  payload, read its fields through it). Destructuring takes a struct by value, and through a reference every
  binder is a reference; binding fields by reference is a later decision (Revisit).
- On the way: destructuring a value that is not the struct named (a reference to it, another type) is now the
  static `diag.type-mismatch` `[T-Let-Destructure]` always required; `coby` had faulted when the statement ran
  and `cobc` had stopped with an internal error. A field the struct does not have is reported by name.

## Rationale

The parser makes the pattern into the two steps a program writes today, so every later pass, the checker,
both evaluators and the compiler, sees nothing new; the semantics are destructuring's, already specified.

## Results

Cases: `conf.struct-pattern`, `conf.struct-pattern-missing-field`, `conf.struct-pattern-unknown-field`,
`conf.struct-pattern-through-ref`, `conf.struct-pattern-consumes`, `conf.destructure-reference`. All agree under
`coby`, `cobc` with GCC and with Clang. `cobfmt` formats the form unchanged.

## Revisit

- Fields bound by reference through a reference scrutinee.
- A refutable pattern per field (option 2), if programs need it.
