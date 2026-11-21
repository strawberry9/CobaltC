# CHG-0407 — The gate's release profile run whole; what its first runs found (D-0307, D-0303)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-02)
Implements: D-0307 (its release profile), and repairs a gap of `CHG-0401` (D-0303)
Affects: `impl/conformance/run.py` and `impl/conformance/README.md` (the `flags:` header); `impl/tools/gate.py`;
`impl/tests/common/mod.rs` (what a case's key covers); `impl/cobc/tests/compiled_suite.rs` and
`compiled_spec_rows.rs`; `cobc`'s and `cbc`'s indexing of a `Vec` (`vec_index_call`)

## What changed

1. **`run.py` gives a case's `flags:`** (found by the release profile's `verify` unit). D-0303 added the header
   (`flags: --allow-deprecated`) and the Rust runners pass it, but the standalone runner, the one any
   implementation is checked with, ignored it, so `conf.deprecated-use-allowed` failed there. `run.py` passes the
   flags before the case's path, as the other runners do; `impl/conformance/README.md` lists the header.
2. **A case's key covers its fixtures and data, not the corpus's runner and documentation**: `run.py` and the
   `.md` files under `impl/conformance` are no input of any case, nor of the corpus units.
3. **The driver.** A gate stopped by a signal stops the unit it is running, every process of the unit's session
   (`timeout` makes groups of its own); the `model` unit builds the proofs before it replays through them (the
   replays run the machine `lake build` makes) and fails on a `sorry`; the self-hosted toolchain is rebuilt only
   when its sources, the Rust toolchain's or the tool versions changed; the sweep keeps a result per program;
   the snapshot's copy of the local `stress/` parts includes `crypto/` (the local TLS programs' keys); the
   `fixpoint` unit uses the `cbc` the `build` unit made; `verify` runs three cases at a time; an advisory failure
   of `external` is reported, not swallowed; old runs and results are pruned (twelve runs; thirty days); a
   snapshot whose objects a squash removed is cloned again.
4. **`StringView::lines(&s)[0]` under `cobc` and `cbc`** (found by `verify`: `conf.contained-fault-message-kept`,
   which Update 160 wrote so, failed with `cobc`'s internal error "slice of something that is not an array, a Vec
   or a slice"). `e[k]` on a `Vec` is lowered as `*Vec::index_shared(&e, k)` built of copies of `e` and `k`
   (`[Index-Vec]`), and the copies' nodes lacked the checker's facts on the originals': a call's result indexed
   whose argument is D-0302's view, `&s` or `&s[lo..hi]` of a `String`, lost the view's `SliceString` and was
   lowered as a slice. The copies take the facts their originals' parts had (`carry_facts` in `cobc`,
   `renumbered_with_facts` and `items.cb`'s `expr_ids` in `cbc`); the C both write is the same.
5. **An internal error in `cobc` fails a compiled case** (`compiled_suite.rs`, `compiled_spec_rows.rs`): exit
   status 3 with `unsupported:` was counted as a skip whatever followed it, so the corpus units passed the case
   item 4 names, listed among the skipped. A declared gap is still a skip; `internal error in cobc` is a failure.

## Verification

The release profile (`impl/gate.sh release`) green on this Update's commit, its verdicts named in
`spec/MANIFEST.md`.
