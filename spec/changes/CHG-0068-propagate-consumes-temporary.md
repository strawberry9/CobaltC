# CHG-0068 — `?` consumes a temporary operand

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0060
Affects: rule.fail.propagate

## Problem / motivation

D-0060 (friction found by real-world testing, `impl/STATUS.md`).

## Decision

? ends a temporary operand at the ?, not with its statement.

## What changed

spec/18 1.3.0 ([Propagate] and its prose), spec/conformance.md 3.52.0. Implementations: coby eval_propagate takes the value out of a temporary; cobc ends a ? scrutinee temporary after its match (cb_end_moved_out). 18-error-failure-semantics/propagate_reference_lives_to_statement_end_rejected.cb (added 2026-09-27 for the previous rule) is replaced by propagate_temporary_ends_at_question_ok.cb.

## Compatibility classification

Extension.

## Conformance changes

**Added:** conf.propagate-temporary-consumed.

## Prior-art status

See D-0060.

## Revisit conditions

See D-0060.
