# CHG-0066 — A binder of the whole value, and constants, in patterns

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0058
Affects: rule.agg.match

## Problem / motivation

D-0058 (friction found by real-world testing, `impl/STATUS.md`).

## Decision

A binder alone at the top of a pattern binds the whole value; a constant of integer or bool type stands where a literal may.

## What changed

spec/16 1.11.0 (the grammar folded to pat ::= _ | x | lit | V | V(pat), names, the prose), spec/22 2.20.0, spec/conformance.md 3.52.0, spec/02 1.0.42. Implementations: the parser keeps a lone name for modres (constant, variant, binder); consts folds a constant in a pattern; the checker types a top-level binder; coby take_whole and a first catch-all arm on any type; cobc binds the scrutinee slot, lowers a catch-all first arm on a non-enum flat. The guide (§16).

## Compatibility classification

Extension.

## Conformance changes

**Added:** conf.match-binder, conf.match-binder-moves, conf.match-arm-after-binder, conf.match-const, conf.match-const-wrong-type, conf.match-const-not-binder.

## Prior-art status

See D-0058.

## Revisit conditions

See D-0058.
