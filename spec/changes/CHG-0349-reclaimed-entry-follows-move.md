# CHG-0349 — A reclaimed object's entry goes when the object moves away

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0202, D-0205, D-0209
Affects: `impl/cbrt/src/lib.rs` (`cb_move_to`, `Reclaimed::remove_if`)

## What changed

No rule changed. `cbrt` keeps a table of the objects that lie over raw cells (a `Vec`'s elements), by
address. An object reclaimed from an element and moved into a binding (`Vec::pop`'s result, an element
moved out) changed its address in `cb_move_to`, but its entry stayed at the element's address. When the
object ended, `end_identity` looked for the entry at its new address and did not find it, so the entry
stayed for good, naming an ended object. Readers skip such entries, but a range scan reads them:
`cb_live_in`, which runs at every call of a function whose body takes a vector by frozen reference
(D-0205, the vector's buffer checked for objects over it) and at every view of a `String`. Once the buffer
was freed, the next one allocated at that address inherited them all. The formatter written in CobaltC
(CHG-0346) lost two thirds of its time to this: 515 million dead entries read in one run over a 590-line
file at `--width 40`, about 700 per call. `cb_move_to` now removes the entry when the object moves away
from the cells it names (`cb_raw_move_in` enters it at its new cells, as before). A stale entry could also
have ended the wrong object: a later move into those cells (`cb_raw_move_in`) ended whatever the entry
named, by then a live value in a binding.

`Reclaimed::remove_if` also missed an ordered entry when an ephemeral one lay at the same address (it
asked `get`, which answers with the ephemeral entry first); it now asks for the ordered entry itself.

## What changed for programs

Faster compiled programs where vectors of elements are popped or moved out and buffers reused: the slowed
lexer ran 6 times faster (18.6 s to 2.9 s), `cobfmt-cb` at the default width 4 times faster on the largest
files. No outcome changed.

## Found by

Profiling `cobfmt-cb` (`perf`, then a counting build of the runtime), 2026-10-19.
