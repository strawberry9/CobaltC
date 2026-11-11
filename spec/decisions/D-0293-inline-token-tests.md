# D-0293 — The token-kind tests inline: a decided function's references cost no call in any program

Status: ACCEPTED (2026-10-29, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 phase 3, step 4 of `stress/d0281/phase3/DESIGN.md` §5.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0281, D-0284, D-0292
Affects: `spec/IMPLEMENTATION-NOTES.md` (the runtime contract); `impl/cbrt/include/cbrt.h`, `impl/cbrt/src/lib.rs`,
`impl/cobc/src/lower/passes.rs`; `CHG-0388`

## Problem

D-0292 made a decided function's references light: recorded nowhere, checked once where they are formed. But
every borrow, read and write through one still *called* the runtime to learn that there was nothing to do — the
call took the heap's hold, resolved the object, read its holder list and returned — and so did every borrow from
the owner's own binding, whose object in nearly every case has no holder. The micro program `b` (200k string
appends in a program that keeps paths for an unrelated reason) spent 93% of its 94 M instructions in those calls:
`cb_borrow_unminted` on the owner's binding before each call, `cb_borrow_check` on the parameter inside
`String::append`, and under them `view` and `check_access_with` finding an empty list. D-0281 §14's tier-S line
for a reference in a tracked program ("form, copy, pass, return and end cost no runtime state") held for the state
and not for the time.

## Decision

1. **The token says, inline, whether a check can matter.** `cbrt.h` tests a token's kind before calling the
   runtime: a light reference (`CB_LIGHT` set, `CB_ROOT` clear) needs no check an entry could make; the owner's
   own binding (`CB_LIGHT | CB_ROOT`) needs none while its object has no path on it. A token is *vouched*
   (`cb_vouched`) in those two cases, and `cb_borrow_check`, `cb_borrow_unminted`, `cb_borrow_range_unminted` and
   `cb_read` of a vouched token make no call, in any body.
2. **The marks.** Whether an object has paths on it is one byte per object slot, per heap (`cb_marks`, exported
   by the runtime, kept by `mint_in` and `retire_token` under the object's heap lock, cleared when the object
   ends; an array that grows is never freed). The generated C reads it with no lock: a mark on an owner a thread
   accesses can rise only through that thread's own holder, or under a guard or through a lent reference
   (`stress/d0281/phase3/DESIGN.md` §2.9), which order the write before the read. A stale token whose slot now
   names another object reads that object's mark: a spurious call, never a missed one.
3. **A decided body forms light borrows inline.** `cobc` opens every decided body — a function or closure whose
   frame is light (D-0292's `G` marker), whether or not the frame is emitted — with `enum { cb_lf = 1 };`,
   shadowing the header's `cb_lf = 0`. In such a body `cb_borrow` and `cb_borrow_unstamped` of a vouched token are
   the token with `CB_ROOT` cleared (`LIGHT | obj`, what `light_borrow` returns) and no call; `cb_write` through a
   light reference is no call (a resource target is `diag.overwrite-of-live-resource`, as D-0292 §4). A recording
   body, and the `std` natives (fixed texts that push no frame and so run in their caller's), keep the runtime's
   borrow, which records in a recording frame and is light in a light one; their checks that mint nothing are
   inline as in 1.
4. **The oracles keep their meaning.** `COBALTC_RECORD=full|check` sets `cb_inline_off`, and no inline test then
   decides anything: under `full` every root is a recorded path (no light token exists), under `check` every
   light token reaching an entry of a recording frame is still seen by the runtime and reported.
5. **Nothing else changes.** No diagnostic, position or outcome moves: a vouched token is one on which the
   runtime's own entry would have found nothing to check (D-0292 §3–4: a light reference's conflicts were decided
   at its formation; an unmarked owner has no holder to clash with). The decided lowering (`CB_PATHLESS`) is
   untouched: its entries were already constants.

## Why these, and not the alternatives

- The test is on the token, not on a per-function switch, because the same native text serves light and
  recording callers (D-0292 "the frame, not the token, says whether a borrow records"); the body-level `cb_lf`
  is added only for the one operation whose answer depends on the frame, the borrow that would record.
- The mark is an exported byte array rather than a flag in the token (a token is a value in registers and
  slots; a holder formed later could not reach it) or a field of the object (reachable only through the arena).
  The dense array is the design's "3 loads and a compare" (DESIGN.md §2.3); the C reloads its base for each test
  because the arena grows.
- Check-only entries are inline in every body, borrows that mint only in decided bodies: a recording body's
  borrow from its own binding must mint the path the record consists of, and a check that mints nothing has the
  same answer in either frame.

## Consequences

- `impl/cbrt/include/cbrt.h`: `CB_LIGHT`, `CB_ROOT`, `cb_marks`, `cb_inline_off`, `cb_lf`, `cb_vouched` and the
  `_i` forms under `#ifndef CB_PATHLESS`, each a `static inline` behind a variadic macro of the entry's name (a
  compound literal in an argument carries commas); `impl/cbrt/src/lib.rs`: the marks (`ensure_mark` at every
  `objs.reserve`, `set_mark` at `mint_in`, `retire_token`, an object's end and its migration), the switch in
  `cb_init`; `impl/cobc/src/lower/passes.rs`: the `enum` line at the `G` marker, emitted by `resolve_scopes`
  itself (a line with a `cb_` identifier counts as a use of the frame to the scope resolver, which an emitted
  statement would have been: the first form of this change made every light frame real and cost `d` 9%).
- Measured (`stress/d0281/phase3/BENCH.md`, instructions, GCC `-O2`, against Update 147): `b` 94.2 M → 8.6 M
  (0.09; under Clang 18.6 M), `d0` 2.14 G → 2.00 G (0.93), `d` 11.03 G → 10.57 G (0.96), `h` 20.71 G → 20.30 G
  (0.98); the decided programs and the controls within 1% (their entries were constants already; `cbc` +0.9%, the
  marks kept at every object's creation and at the recorded roots its destructor calls still mint).
- Gates: the 167 dynamic conformance cases three ways (`coby`, light under `COBALTC_RECORD=check`, full) under
  GCC and Clang; the three-way differential over the 996 programs that keep paths; the release suites.
- What a tracked program now pays for is what its profile said before this step: the objects of values and
  temporaries (`d`: the element objects of its reads, `HeapReclaimed::get` 23%; `h`: the slot map of its stores),
  and for the self-hosted compiler, decided, the same objects alone (`cbc`: 60% of its instructions in
  `end_identity_in`, `detach`, `cb_stmt_pop`, `new_obj`, `make_pending`, `cb_frame_pop`, `cb_bind`,
  `cb_move_to`; of the 24,261 objects its C makes, 2,357 are objectless locals). The next work of D-0281 is
  therefore the objectless remainder — drops the compiler places for the values the objectless analysis does not
  yet cover (D-0286's option C) — before phase 3 steps 6–7 (frame shadows, block objects), which move only the
  programs that keep paths; then the single port of the self-hosted compiler and its measurement.
