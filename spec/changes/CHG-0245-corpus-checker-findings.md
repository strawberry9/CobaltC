# CHG-0245 — Stale references found by the corpus checker

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0207)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0207
Affects: `spec/19` 1.11.2, `spec/21` 4.53.1, `spec/conformance.md` 3.201.0, `spec/02` 1.0.69

## What changed

- `spec/21`: `[Push-Ascii-Not-Ascii]` is `disposition: checked`; `[Local-Offset]` is
  `outcome: unspecified { the platform's offset o, −64800 ≤ o ≤ 64800 }`; the supervised-task rules depend
  on `rule.fail.fault-unwind`; the digest rules depend on `rule.arith.checked` and `rule.arith.bitwise`.
- `spec/19`: `rule.conc.cancel` depends on `rule.fail.fault-unwind`.
- `spec/conformance.md`: `conf.ref-byte-image-width` cites `[Index-Checked]`, `conf.deref-non-reference`
  cites `[T-Deref]`, `conf.hmac-sha1` cites `[Hash]`.
- `spec/02` §5: the "in use" ranges reach D-0207 and CHG-0245.

## What did not change

No rule's premises, conclusion or diagnostic; no case's program or outcome. Each change names what the text
already meant by a name that exists.
