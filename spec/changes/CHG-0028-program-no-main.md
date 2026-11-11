# CHG-0028 — `[Program-No-Main]` and `diag.no-main`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-22)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: rule.fn.program, term.program, rule.module.file, CHG-0026
Affects: rule.fn.program, diag.no-main, conf.no-main

## Problem / motivation

`rule.fn.program` requires "exactly one `fn main() : void` (or
`fn main()`) at the root, not generic, not `extern`" in prose, but had
no `disposition: rejected` form and no diagnostic for its absence, so
the registry had nothing an implementation could report. `CHG-0026`
made the gap visible: a file-backed module's body is not a program
and has no `main`, and running one on its own produced
`diag.unbound-name` with no location — the wrong diagnostic for a
well-formedness fact about the whole program.

## Decision

Add the rejection and its diagnostic. No other change to what a
program is.

## What changed

**`spec/15` 1.3.0** (§7): `rule.fn.program` gains
`[Program-No-Main]`. **`spec/registry/diagnostics.md` 1.6.0**:
`diag.no-main`. **`spec/conformance.md` 3.14.0** (§14): `conf.no-main`.

## Affected entities

`rule.fn.program` (one new labelled rejection, same prose),
`diag.no-main` (new).

## Previous semantics

A program without a root `main` was ill-formed by the prose; the
rejection had no label and no diagnostic id.

## New semantics

    [Program-No-Main]   disposition: rejected   no `fn main()` at the root, or it is generic or `extern`   ill-formed; diag.no-main

A `main` inside a module — including a file-backed one — is the
ordinary item `m::main` and does not satisfy the rule.

## Affected invariants

None.

## Dependency impact

None; `rule.fn.program`'s `Depends on` line is unchanged.

## Compatibility classification

Additive and semantics-preserving: no program's acceptance changes;
a rejected program now has a name for why.

## Migration implications

None.

## Example changes

None; `ex.module-file` (`CHG-0029`) shows which of two files to run.

## Conformance changes

`conf.no-main` (`spec/conformance.md` §14; file-based, since a table
fragment is always wrapped in a `main`).

## Future implementation implications

The check belongs at the start of the static pass (`coby`:
`typecheck::check_program`), before any item is examined, with the
program as a whole as its location.

## Prior-art status

Every hosted language rejects a missing entry point at link or load
time; this places it in the static pass, where the rest of
`rule.fn.program`'s well-formedness already lives.

## Revisit conditions

A library-only compilation mode (a program with no entry, checked
but not run): would need a separate decision, and would relax this
rule for that mode only.
