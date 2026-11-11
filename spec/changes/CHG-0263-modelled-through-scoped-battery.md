# CHG-0263 — *modelled* is carried through a scoped battery

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-13; under D-0212, D-0219)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0212, D-0219, CHG-0256
Affects: `spec/README.md` ("Evidence tiers"); `impl/battery.sh`, `impl/tools/manifest.py`

## Why

The manifest claims *battery* when the newest green battery's tree fingerprint is the current tree's, and
*modelled* when that run compared the model's sample. An edit made after a full battery (the edition line,
a change-log row) is checked by a scoped battery, whose scope leaves the model comparison out when nothing it
reads changed. The manifest then named only the scoped run, and lost *modelled* (Update 103's edition
2026.101202 was recorded as "battery, explored to 40" for that reason).

## What changed

- `impl/battery.sh`: a scoped run's `SUMMARY` records `base`, the green tree its scope was computed against.
- `impl/tools/manifest.py`: when the run the tier names is scoped and its scope did not hold the model
  comparison (stage 1's `tests` part), *modelled* is taken from the newest green run of its `base` tree,
  through any chain of scoped runs, back to one that compared the sample; a scoped run that held the
  comparison answers for itself. A run written before this change is followed through the `base` named in
  its `scope.txt`. The manifest's battery table shows `Scoped against` and where *modelled* came from.
- `spec/README.md`: the definition of *modelled* says so.

## What changed for programs

Nothing: a record-keeping change.
