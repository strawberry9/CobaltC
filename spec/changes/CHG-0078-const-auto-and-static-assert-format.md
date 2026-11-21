# CHG-0078 — `const auto`, and values in a `static_assert`'s message

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0068
Affects: rule.module.const, rule.module.static-assert

## Problem / motivation

D-0068.

## Decision

D-0068: `const auto N = e;`; `static_assert(c, "format", a…)` with
constant arguments.

## What changed

- **`spec/17` 2.7.0:** §1a (`const auto`), §1b (the message forms,
  `[Static-Assert-Not-Constant]`).
- **`spec/22` 2.22.0:** `const-decl`, `statement`.
- **`spec/registry/diagnostics.md` 1.30.0:** `diag.static-assert-failed`,
  `diag.static-assert-not-constant`.
- **`spec/conformance.md` 3.61.0:** the cases below.
- **`spec/02-schema.md` 1.0.51:** §5's "in use" ranges.
- **Implementations:** `FnDecl::auto_type`; the parser; `consts`
  (`resolve_auto`); the checker (`static_assert`'s `message_fmt`); the
  front end (`eval_static_message`). The guide (§17).

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.const-auto`, `conf.const-auto-undetermined`,
`conf.static-assert-format-message`, `conf.static-assert-message-variable`.

## Revisit conditions

None.
