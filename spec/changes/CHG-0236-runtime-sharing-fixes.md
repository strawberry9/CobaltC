# CHG-0236 — Runtime: an absent id touches no heap; three entries local

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; round-9 real-world programs)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0202, D-0204, CHG-0234
Affects: `impl/cbrt/src/lib.rs`

## What changed

- **An absent id (0) touches no heap.** Since CHG-0234 an entry given id 0 (a pathless reference) locks only the
  entering thread's own heap; a lookup of id 0 then still read heap 0's table, unlocked, while another thread
  might be growing it -- a data race in release builds (the lookup finds nothing either way), caught by the
  debug build's per-access assertion on every socket program. `get`, `get_mut` and `remove` answer `None` for 0
  at once.
- **`cb_frame_hold`, `cb_frame_unhold`, `cb_recv_datum_tok` are local entries** (the path's heap and the
  thread's own state). They were exclusive: `match (HashMap::get_mut(&mut m, &k)) { Some(c) : … }` in a hot
  loop stopped every other thread twice per iteration. Four threads each counting on their own data went from
  1.5x to 2.4x one thread (`stress/round9/scaling_owned.cb`); exclusive entries 854 000 -> 45 000.

## What did not change

No rule, no diagnostic, no program's output.
