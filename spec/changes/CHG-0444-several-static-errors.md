# CHG-0444 — Each independent static error in one report (D-0321)

Status: implemented (2026-11-14), D-0321, D-0296; Update 188
Affects: `impl/src/lib.rs` (`earliest_first`, `set_more_static`, `render_static`, `Analysis::more`,
`take_static_more`), `impl/src/parser.rs` (`parse_reporting_all`, `Parser::recover`, `item_end`,
`item_start_at`, `restart_at`, `after_prelude`), `impl/src/modres.rs` (`resolve_items`, `resolve_item`),
`impl/src/typecheck.rs` (`Ck::run`), `impl/src/check.rs`, `impl/src/certificate.rs`, `impl/src/main.rs`,
`impl/cobc/src/main.rs`; their twins `impl/selfhost/front/parser.cb` (`parse_text_all`, `item_end`),
`modres.cb` (`resolve_item`, `join_more`), `typecheck.cb` (`earliest_first`), `check.cb` (`Rejected::more`,
`rejected`), `impl/selfhost/cbc/diags.cb` (`render_static`), `frontend.cb`, `driver.cb`, `main.cb`;
`spec/registry/diagnostics.md` (1.68.0); `spec/conformance.md` (3.311.0)

## What changes

- **Bodies.** `Ck::run` already checked every function and kept the earliest error; each error is now kept with
  its function's name, and `earliest_first` returns the earliest and keeps the earliest of each other function,
  in source order (an error text two instantiations share is kept once).
- **Names.** `resolve_items` records an item's error and goes on with the next item; the first item's error is
  the one returned, as before.
- **Syntax.** For the program's own parse (`parse_reporting_all`), an item's syntax error ends the item:
  `item_end` finds the next item by bracket depth from the item's start (past a `;` at depth 0, or past the `}`
  that brings the depth back to 0 when an item can begin after it) and parsing goes on there with the item's
  state dropped (`restart_at`). When the text's brackets do not balance, or the place found cannot begin an item,
  the rest is not parsed. `--check-syntax` and cobfmt still stop at the first error.
- **The report.** `render_static` shows each diagnostic as before, a blank line between, then `N errors`
  (`N errors; the first 10 are shown` past ten), from `coby`, `coby --check`, `cobc`, `cobc --check` and
  `--certificate`'s check. The selfhost carries the other errors after U+0002 in the error text until
  `check::rejected` takes them apart.
- **A bug the review found** (older than D-0321): the parser's pre-scan of type names (`spec/22` §2,
  disambiguation 4) ran over the prelude and the program as one text, so a program's `struct n` made `n` a type
  in `std`'s own text, and a correct program was rejected with a syntax error in `std/text.cb` (coby and cobc;
  cbc, which parses `std`'s bodies apart, accepted it). `after_prelude` keeps the program's type names out until
  its first item. Row `conf.struct-named-like-std-local`.

## Checked

- The review of D-0321 §4 (`stress/ease188/LOG.md`, `stress/ease188/mutants/mutate.py`).
- cbc and cobc give the same report, byte for byte, on the probes and the conformance cases.
- Rows `conf.several-syntax-errors-first` and `conf.struct-named-like-std-local`; tests in `impl/tests/cli.rs`.
