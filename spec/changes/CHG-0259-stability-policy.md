# CHG-0259 — The stability policy; the register keeps ten editions

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-12; D-0220)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0220
Affects: `spec/STABILITY.md` 1.0.0 (new), `spec/EDITIONS-HISTORY.md` (new), `spec/README.md`, `NAMING-POLICY.md`;
`htmlguide/generator/content_intro.py`

## What changed

- `spec/STABILITY.md`: the marks *moving*, *candidate* and *stable* for the language (*core*) and the standard
  library (*std*); the four kinds of change; the deprecation window; the conditions for a candidate.
- `spec/README.md`: a "Stability" paragraph; edition rows end with the marks from D-0220 on; the register holds
  the ten most recent editions.
- `spec/EDITIONS-HISTORY.md`: the 51 earlier rows, moved unchanged.
- `NAMING-POLICY.md`: a conformance claim inherits its edition's marks.
- The guide's introduction: two sentences on what the marks mean for a reader.

## What did not change

No rule, no program's behaviour, no edition's contents.
