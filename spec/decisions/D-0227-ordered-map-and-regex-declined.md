# D-0227 — No ordered map and no regular expressions in `std`

Status: ACCEPTED (2026-10-13; the 2026-10-12 plan's recommendations for WI-8, accepted by the owner)
Kind: Decision (`spec/02-schema.md`); deliberate absences
Governed by: `CobaltC_Master_Instructions.md` §9 (the std helper admission test)
Depends on: D-0062 (sorting), D-0139 (text helpers)
Affects: `spec/22` §5 (2.48.0) (CHG-0269)

## Decision

- **Ordered map or set:** declined for now. A `HashMap`'s keys collected into a `Vec` and sorted with
  `Vec::sort` or `Vec::sort_by` give the order a program asks for; a tree would be a second map type to
  learn for a need no program has shown. Revisited if programs need range queries.
- **Regular expressions:** declined. A regex engine is large and its dialects differ; the common needs are met
  by `split`, `find`, `starts_with`, `strip_prefix` and the parsers. A glob matcher would be the smaller answer
  if a matcher is wanted.

Both are recorded in `spec/22` §5's deliberate absences.
