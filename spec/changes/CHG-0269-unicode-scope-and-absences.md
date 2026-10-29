# CHG-0269 — The Unicode scope of `std`; no ordered map, no regular expressions

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-13; D-0226, D-0227)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0226, D-0227
Affects: `spec/21` §2h (4.60.0), `spec/22` §5 (2.48.0), the guide's "Which text type?"

## What changed

- `spec/21` §2h: "Unicode": text is UTF-8 bytes; `std` has no Unicode tables.
- The guide's "Which text type?": the same, in a paragraph.
- `spec/22` §5: an ordered map or set, and regular expressions, are deliberate absences of `std`.

## What changed for programs

Nothing.
