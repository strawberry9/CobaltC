# D-0296 — The self-hosted toolchain ported to the current edition: `cbc` writes `cobc`'s C again, and D-0281 is complete

Status: ACCEPTED (2026-10-30, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0278's amendment of 2026-10-28: the single port after phase 3. D-0281 §24:
the last remaining item.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0278, D-0281, D-0282–D-0295, D-0276, D-0274
Affects: `impl/selfhost/front/*`, `impl/selfhost/back/*`, `impl/selfhost/cbc/*`, `impl/selfhost/compare_c.sh`,
`impl/battery.sh`; `impl/STATUS.md`; `README.md`; `CHG-0391`

## Problem

D-0278 (2026-10-25, option 5) froze the toolchain written in CobaltC — `cbparse` (coby's front end), `cbc` (cobc)
and `cobfmt-cb` — at edition 2026.102501 (Update 135) while D-0281 rewrote the static pass's reference rules, the
lowering and the runtime's dynamic tier, so that each change was written once, in Rust, and measured at the Rust
tools' speed; its amendment of 2026-10-28 made the port a single one after phase 3. Phase 3 closed with D-0295
(Update 150). The delta to port was fifteen Updates of the reference: in the static pass, `typecheck.rs` +3,745
lines and `tier.rs` (629 lines, new) — the frame-reference rules (D-0282), point checks (D-0283), closure facts
(D-0288), scoped lending (D-0289), frame-held aggregates (D-0290), the real programs' repairs (D-0291) — and in
`cobc` about 1,500 changed lines across the lowering — the two-pass decided lowering (D-0284, D-0285), values
without objects (D-0286), lazy splitters (D-0287), the objectless remainder (D-0294), light references and inline
token tests (D-0292, D-0293, D-0295), `cb_hold_ref`/`cb_rec_ref`/`cb_frame_push_fn|rec`, point checks — and
`--emit-c -o`. Until the port, `cbc` wrote a C that kept every path, ran under the old runtime contract and refused
the programs the new rules accept; the README and `impl/STATUS.md` said so.

## Decision

1. **The port is made, once, and the fixed point holds at edition 2026.103001.** `impl/selfhost/front/`
   reproduces the static pass of `coby` function for function (`front/tier.cb` is `tier.rs`), with the tier facts
   the code generator reads kept on `Items` as the Rust keeps them (`tracking_needed`, `point_check_fns`,
   `point_checks`, the access summaries); `impl/selfhost/back/` and `cbc/` reproduce `cobc`'s lowering (new:
   `back/program.cb`, `pathless_blocker`; `back/flights.cb`, datum flights; `back/points.cb`, point checks); `cbc
   --emit-c [--c-name NAME] [-o OUT] FILE` as `cobc` has it. About 20,200 lines added and 7,400 replaced, besides
   the regenerated prelude and runtime header. The oracles, all at 0 differences: `compare_parser.sh` in every mode
   over every tracked file (21,157 comparisons), the tier report (`COBALTC_TIER_REPORT`) over the corpus (1,249
   bodies), `compare_c.sh` over every tracked file (1,162 programs), over the conformance suite with
   `--all-checks` (753) and over the showcases and the controls with `-g`, `--no-line-directives` and the
   bootstrap form (92 each), `compare.sh` (5,304), `fixpoint.sh` (stage 2 = stage 1 = `cobc`'s C, 129.7 MB),
   and `cargo test -p cobc --release` with `COBC=` the ported `cbc` (the compiled conformance suite and the
   showcases, GCC and Clang).

2. **D-0278's regime ends with D-0281: both toolchains are kept in step again (D-0278 option 1).** From this
   Update every change to the static pass or the lowering lands in the Rust tools and in `impl/selfhost/` in the
   same Update. The battery's selfhost stage compares the two compilers' C on the showcases, as it compared the
   formatters and the parsers (`compare_c.sh` now exits non-zero on a difference). Retiring `cobc` (D-0278
   option 2) is not decided now. Of its preconditions, `cobc`'s tests drive `cbc` (`COBC=`, item 1) and P4's bound
   holds (`cbc` takes 2.5 times `cobc`'s time on `anagrams.cb` and about 8.5 times on its own source; D-0276 asked ten), but the bootstrap seed is not chosen, and the byte-for-byte oracle is
   worth more than the saving while it keeps finding bugs (CHG-0391 item 2 is one in `cobc`). It is a decision
   for a quieter period of the language, not for the Update that completes D-0281.

3. **The measurement.** `cbc` is compiled by `cobc` and is decided, so it runs pathless and objectless; what it
   costs changed because it now does more: the static pass and the lowering of D-0282–D-0295. On `anagrams.cb`
   it executes 5.21 G instructions where the frozen port executed 3.44 G (`stress/d0281/phase3/runs/port` against
   `runs/elem`; every other row of the bench within 0.3%). It writes its own C in about 11 minutes (Update 135:
   about 18; `cobc` takes 79 s, from 26 s, for the same reason), and `cbparse` checks its own source in 34 s (the
   frozen port: 24 s; `coby`: 11 s). The programs it compiles are now `cobc`'s byte for byte: a decided program
   compiled by `cbc` runs pathless and objectless as D-0284–D-0295 made it.

4. **D-0281 is complete.** Its phases: 1 (the tier-S rules, normative since D-0282; frame-held aggregates, D-0290;
   the real programs, D-0291), 2 (decided programs pathless, D-0284; the decided lowering, D-0285; values without
   objects, D-0286; lazy splitters, D-0287; closures, D-0288; scoped lending, D-0289), 3 (light references, D-0292;
   inline token tests, D-0293; the objectless remainder, D-0294; light element references, D-0295), 4 (O3 and O5
   decided on corpus evidence, §24), and this port. The invariant map (§13), the cost contract (§14, restated in
   `spec/14` §6 by D-0295) and the review's preconditions (§12) stand. What stays open, recorded for the programs
   that ask for it: phase 3's steps 6–7 (D-0295 item 3), the statement scopes and frames of the objects that
   remain (D-0294's next round), D-0280's views without an object, and a gap found by the port: `cobc`'s decided
   lowering falls back (a `-v` note, the program kept tracked) for a `String` scrutinee moved out of a place that
   is not a binding (`match_text_patterns_ok`).

## Why these, and not the alternatives

- Porting once was D-0278's amendment; porting by D-record in the Rust's order, function for function, is how
  the front end was written in the first place (CHG-0350) and is what lets `compare_parser.sh`'s byte-for-byte
  comparison localise every difference. The front end and the code generator were ported side by side against a
  fixed interface (the facts on `Items` and three type questions), the code generator's half checked against
  `cobc` with the front end's facts stubbed, so each half's differences were its own; three differences of the
  joined toolchain were integration (the order point checks are recorded in, a cached probe), not port.
- Keeping both toolchains rather than retiring `cobc` now: the byte-for-byte oracle has found real bugs in both
  (D-0278 lists them) and found one more in this port; the cost of option 1 is per change, and the period of heavy
  change is over.

## Consequences

- `cbc`, `cbparse` and `cobfmt-cb` accept the current edition; the README's and `impl/STATUS.md`'s "accepts the
  language of edition 2026.102501" lines go.
- A change to the static pass or the lowering is now written twice, in Rust and in CobaltC, in the same Update.
- One finding in the Rust: a `match` that is an arm's whole value did not scope its own arms (CHG-0391 item 2), a
  false run-time fault in programs that keep paths, present since D-0103/D-0135.
