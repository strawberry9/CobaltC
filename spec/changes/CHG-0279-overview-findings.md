# CHG-0279 — The text made consistent where the overview found it was not

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0233
Affects: `spec/00` 1.3.1, `spec/03` 1.6.1, `spec/04` 1.7.1, `spec/05` 1.8.1, `spec/08` 1.5.1, `spec/14` 1.26.1,
`spec/19` 1.15.1, `spec/20` 1.17.1, `spec/conformance.md` (3.224.0), `spec/02-schema.md` (1.0.85);
`impl/model/src/machine.rs`; one case in `impl/conformance`

## What changed

D-0233's decisions, each as text:

- `[Block-Exit]` orders the frame's objects by descending `formed-at` of their holders (last bound first).
  The model's block exit, call exit and unwind now do the same. Case:
  `conf.destruction-order-after-move`, which gives "drop 1, drop 2" under the text and both tools. The
  model has no user destructors, but it ends objects in the same order.
- `spec/08` §1's `clash` restates `spec/04` §2's in full, and `spec/03`'s `inv.alias-validity` names
  `pending`.
- `term.access-path`, `state.holder` and `steps` describe a lock path as derived from the locking
  reference.
- `spec/05` says where a safe program can see an address, and that the freedom stays `unspecified`.
- `rule.trust.unsafe` lists every trusted rule and says where `[Extern-Decl]`'s claim is made.
- `spec/19`'s Purpose names `[Thread-Affine]` and `[Thread-Lend]`.
- `term.trusted-library-base` includes the implementation's `std::extensions` bodies.

## What changed for programs

Nothing: no rule now admits or forbids anything it did not before, and both implementations already did
what the corrected `[Block-Exit]` says.
