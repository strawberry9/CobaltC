# CHG-0424 — A probe undoes what it wrote to the function's name sets, instead of copying them

Status: implemented (2026-11-10), D-0200, D-0296; step 4 of the allocation plan (the agent's proposal of 2026-11-10 on the owner's speed test; the owner: "implement the proposal in full")
Affects: `impl/cobc/src/lower.rs` (`NameSnap`, `NameSet`, `NameMap`, `NameUndo`; `Fx.names_undo`, `name_marks`),
`impl/cobc/src/lower/matching.rs` (`snapshot_names`, `restore_names`, `names_add`, `names_drop`, `names_put`) and the
52 writes in `lower/` (`calls`, `closures`, `exprs`, `frames`, `fresh`, `intrinsics`, `matching`, …), and their twins
in `impl/selfhost/back/state.cb` and the back end's 52 writes. No rule changes. No program's outcome or C changes.

## What changes

A type probe lowers an expression into a discarded buffer and then restores the function's state, including nine
name sets keyed by C names (D-0200: the probe reuses the real pass's names). `snapshot_names` copied all nine and
`restore_names` moved the copies back, per probe, at a cost that grew with the function. Now:

- every write to the nine sets goes through `names_add`, `names_drop` or `names_put`, which, while a mark is open,
  log the write's inverse (remove a name added, put back a name removed, put back a value overwritten, remove a key
  added);
- `snapshot_names` is a mark (the log's length, and how many marks were open); `restore_names` undoes the log down
  to the mark and closes it, so an inner mark a failed probe left open is closed by the outer one;
- with no mark open nothing is logged;
- a closure's body is lowered twice when its result type is not written, a probing pass and the real one: only the
  probing pass takes a mark (the real pass's copy was never restored).

No code iterates the nine sets, so a name a probe removed and the undo puts back at the end of its set's order
changes nothing observable.

## Verified

`cobc`'s C with the undo log is byte for byte the C without it on every tracked `.cb` file (`compare_c.sh` with the
two `cobc`s); `cbc`'s C is `cobc`'s (`compare_c.sh`, the gate's `selfhost` unit) and `cbc` reproduces itself
(`fixpoint.sh`).

## Measured

The snapshot and its restore fell from 9.0% of `cbc`'s cycles on the Lisp program (Update 175) to 1.5%, and from
1.4% to 0.1% on `large_200.cb` (`CHG-0421`'s `HashMap::clone` took them to 4.0% first). `cobc` gains too: with
`CHG-0421` and `CHG-0423`, 5.4% on the Lisp program and 2.5% over the ten programs of the speed test.
