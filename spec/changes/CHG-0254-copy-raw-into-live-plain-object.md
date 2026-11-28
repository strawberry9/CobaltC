# CHG-0254 — `[Copy-Raw]` may fill a live object of a plain, reference-free type

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0216)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0216
Affects: `spec/20` 1.15.0, `spec/conformance.md` 3.210.0; `impl/conformance/README.md`;
`impl/conformance/06-arithmetic/ref_byte_image_width_ok.cb` (header)

## What changed

- `spec/20` §2 `[Copy-Raw]`: the destination condition (a live object of a plain type holding no reference,
  under `[Rawptr-Write]`'s conditions; no reclaimed object, no resource); "Root paths and raw access".
- `spec/conformance.md`: `conf.ref-byte-image-width`'s derivation.
- `impl/conformance/README.md`: positive `unsafe` cases satisfy every trusted premise they cite.

## What did not change

No accepted program, outcome or tool: the condition is trusted, and the one case that relied on it is unchanged.
