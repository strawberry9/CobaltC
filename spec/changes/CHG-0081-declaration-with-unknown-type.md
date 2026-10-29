# CHG-0081 — A declaration whose type names nothing is a declaration

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, fix reported by the owner)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0024
Affects: `spec/22` §2 disambiguation (4)

## Problem / motivation

`int x = 10;` (or `intXXX x = 10;`) was taken as an expression
statement, since `int` names no struct or enum, and rejected with a
syntax error — "expected Semi, found Ident" — that named neither the
unknown type nor a phase, and read as though the program had failed
while running. The same rule made `T x = a;` a syntax error in a
generic function, where `T` is a type parameter.

## Decision

A statement beginning with two identifiers followed by `=` or `;` is a
declaration: two names in a row begin no expression. Name resolution
then resolves its type: a type parameter, or `diag.unbound-name`
(static) at the statement for a name that is no type, as for the same
name in any other type position.

## What changed

- **`spec/22` 2.24.0:** disambiguation (4).
- **`spec/conformance.md` 3.64.0:** the cases below.
- **`spec/02-schema.md` 1.0.54:** §5's "in use" ranges.
- **Implementations:** the parser (the statement rule; `Stmt::Let`
  carries its line, so a declaration without an initializer is located
  too); name resolution tags the error with it.

## Compatibility classification

Extension (`T x = a;` is accepted) and clarification (`int x = 10;` is
rejected as before, with the diagnostic that names what is wrong).

## Conformance changes

**Added:** `conf.decl-unknown-type`, `conf.decl-type-parameter`.

## Revisit conditions

None.
