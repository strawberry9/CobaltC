# CHG-0431 — A pathless program's dual bodies run their direct body

Status: implemented (2026-11-10), D-0281, D-0188, D-0209; agenda item 3, Update 178
Affects: `impl/cbrt/include/cbrt.h` (under `CB_PATHLESS`: `cb_paths_disjoint`, `cb_paths_arrive`),
`impl/selfhost/back/header.cb` regenerated. No rule changes. No program's outcome changes.

## What was wrong

A function with two or more reference parameters, or a field confined for a fast half, is lowered twice (D-0188,
D-0189, D-0219): a direct body that reads its parameters unchecked and a checked one, chosen per call by
`cb_paths_disjoint` (or `cb_paths_arrive`), which runs the direct body only for paths it can vouch for, and answers
no for a 0 token. In a program lowered with no paths (`CB_PATHLESS`, D-0281 step 4c) every reference carries token
0, so every call took the checked body, whose path checks are themselves constants or no-ops there. In
`stress/vsrust/huffman.cb` `put_bits`'s checked body and the dispatch were 44% of the run.

## What changes

Under `CB_PATHLESS` both answer yes, as `cb_frozen_ok` already did for a frozen parameter: the checked twin vouches
for nothing the direct body does not. The dispatcher's other test, that no object lies over a confined vector's
cells (`cb_live_in`), stays.

## Measured

`stress/vsrust/run.sh`, Update 177's `cobc` against this one with CHG-0430 and CHG-0432 (time against checked Rust,
gcc / clang): huffman 1.86 / 1.81 → 1.19 / 1.18 (gcc 7.96 G → 4.54 G instructions).
