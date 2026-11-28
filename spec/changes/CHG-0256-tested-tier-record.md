# CHG-0256 — The *tested* tier names a run: `impl/tools/suites.sh` and its `TESTED` record

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0212 amendment)
Governed by: `CobaltC_Master_Instructions.md` §21, §24
Depends on: D-0212
Affects: `spec/README.md` (edition paragraph, "Evidence tiers"), `spec/decisions/D-0212-edition-manifest.md`
(an amendment), `spec/MANIFEST.md` (regenerated); `impl/tools/suites.sh` (new), `impl/tools/manifest.py`

## What changed

- `impl/tools/suites.sh`: the targeted suites, logs per target, optional `--explore` sweep, a `TESTED` record.
- `impl/tools/manifest.py`: "The last suite run"; the *tested* claim qualified when no passing record carries the
  tree's fingerprint; the fingerprint excludes the top-level `README.md`.
- `spec/README.md`: *tested* defined by that run; a `MANIFEST.md`-only change keeps the edition.
- `impl/battery.sh`: a run stopped at preflight (the machine busy, say) wrote `result DONE` into `SUMMARY`, which
  `manifest.py` would have read as a green battery for the tree; it now writes `result STOPPED at preflight`.

## What did not change

No rule and no program's behaviour; evidence bookkeeping only.
