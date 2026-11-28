# CHG-0456 — The feature registry for Update 189's plan; its last measures

Status: implemented (2026-11-14), D-0324; Update 191
Affects: `spec/registry/features.md` (1.11.0); `bench/RESULTS.md`

## Why

`private/EASE-PLAN-2-2026-11-14.md` §8 step 8 names `spec/registry/features.md`, which Updates 189 and 190 left
as it was; and its step 7 (the decided sweep, the compiled suites, the timing of `--check`) had not been run again
after Update 190 changed the checker. The owner asked that the bench be run again, on a quiet machine, before the
push.

## What changes

- `feat.variant-fields`, `ACCEPTED` (D-0324): a variant with named fields, and struct patterns through a
  reference. `feat.untyped-closure-parameters` and `feat.field-access-through-box`, `ABSENT`: the two forms the
  owner dropped from the plan on 2026-11-14, so code stays readable from its lines; their messages give the
  written form (`CHG-0452`).
- `bench/RESULTS.md` rewritten by `bench/run.sh` (9 runs a program; the machine otherwise idle but for its
  desktop, about one core in four): the instruction ratios against Rust with overflow checks are those of the
  previous run, to two places, and the times within their ranges.

## Checked

On Update 190's tools: the decided sweep (`stress/d0281/objectless/decided.list`) gives 6,027 decided and the
same 9 undecided as Update 188; the compiled suites passed in Update 190's gate (`corpus-cobc`); `coby --check`
of the showcases, the examples and the benchmarks takes 44.0 s, Update 188's coby 44.7 s in the same run.
