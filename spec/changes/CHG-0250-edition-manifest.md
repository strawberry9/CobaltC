# CHG-0250 — The edition manifest and the evidence tiers

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0212)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0212
Affects: `spec/MANIFEST.md` (new, generated; not a normative artifact), `spec/README.md` ("The manifest",
"Evidence tiers"), `spec/02` 1.0.74; `impl/tools/manifest.py`, `impl/battery.sh`

## What changed

- `spec/README.md`: two paragraphs in "Edition" — the manifest, and the four evidence tiers; from this
  edition on each row ends with its tier.
- `spec/MANIFEST.md`: generated; regenerated at every edition bump.
- `impl/battery.sh`: writes `SUMMARY` (run, result, scope, tree fingerprint, toolchain) beside `REPORT.md`.

## What did not change

No normative artifact, rule, diagnostic or case: the manifest and the tiers describe how an edition was
checked, not what it says.
