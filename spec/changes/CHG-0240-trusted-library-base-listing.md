# CHG-0240 — The trusted library base listed: every `unsafe` block of `std` with the fact that discharges it

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0206 (1))
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0206, CHG-0239
Affects: `spec/20` 1.13.0, `spec/21` 4.53.0, `spec/IMPLEMENTATION-NOTES.md` §3, `spec/conformance.md`
3.200.0 (`conf.safe-client-vec-trace`, `-string-trace`, `-box-trace`, `-rc-trace`, `-channel-trace`),
`impl/std/*.cb` (`// trusted:` comments), `impl/tools/trusted_table.py`

## What changed

- `spec/21` §0 "What `std` asserts": the paragraph and the generated table, one row per function and
  comment: module, item, block line(s), the trusted rule(s) in brackets, and why their side-conditions hold
  there. 93 blocks in 71 functions.
- Every `unsafe` block in `impl/std` is covered by a `// trusted:` comment (one per function, directly above
  its header, covering all of that function's blocks); `impl/tools/trusted_table.py` regenerates the table
  between `<!-- trusted-table:begin -->` and `<!-- trusted-table:end -->`, and `--check` fails when a block
  has no comment or the table is stale.
- `spec/20` §1: whose claims a safe client's execution reaches, and why the guarantee stays unconditional.
- `spec/IMPLEMENTATION-NOTES.md` §3's closing paragraph likewise.
- Five conformance rows walk a safe client through `Vec::new`/`push`/`grow`/`index_shared`/`pop`/`drop`,
  `String::from_str`/`clone`, `Box::new`/`get`/`into_inner`/`drop`, `Rc::new`/`clone`/`get`/`drop` and
  `Channel::new`/`send`/`recv`/`drop`, naming each trusted step and the table fact that discharges it.

## What did not change

No body of `std` changed but for comments (`cobfmt --check impl/std` passes); no rule, diagnostic or outcome.
The listings in `spec/21` §1–§3 are unchanged.
