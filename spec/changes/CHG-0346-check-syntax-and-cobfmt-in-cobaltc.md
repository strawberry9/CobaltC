# CHG-0346 — `--check-syntax`, and cobfmt written in CobaltC

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0021, D-0207
Affects: `impl/src/check.rs`, `impl/src/main.rs`, `impl/cobc/src/main.rs`, `impl/cobfmt/src/main.rs`,
`impl/cobfmt/src/width.rs`;
`impl/selfhost/` (new); `README.md`, `impl/STATUS.md`, `impl/CHANGELOG.md`

## What changed

No rule changed. Two tool changes:

- `coby --check-syntax` and `cobc --check-syntax [--origin PATH] (FILE | -)` say whether one text parses,
  and nothing else: the text of FILE or of standard input, its file-backed modules (D-0021) found relative
  to PATH (FILE by default). Exit status 0 it parses, 1 it does not (the error, `PLACE: MESSAGE (column C)`,
  on standard error), 2 it cannot be read. A program the parser builds but rejects for a reason other than
  its syntax parses. This is the test the Rust `cobfmt` applies before it formats a file and to what it
  writes; it moved from `cobfmt` into the shared crate (`coby::check::syntax`), and `cobfmt` calls it there,
  so the three tools mean one thing by "parses". When a module file cannot be loaded (missing, a cycle, a
  duplicate) the text is parsed on its own with each such declaration read as an empty module, every line
  and column kept; before, `cobfmt` parsed the declaration itself and refused the file with a syntax error at
  its path (`expected {`), so a file whose module was not found from where `cobfmt` ran could not be formatted.
- `impl/selfhost/cobfmt/` is `cobfmt` written in CobaltC, built by `cobc` (`impl/selfhost/build.sh`) as
  `cobfmt-cb`: a module for each module of the Rust `cobfmt` (lexer, layout, line breaking, the safety check,
  the diff, files), with the same options, messages and exit statuses. Having no parser, it asks
  `cobc --check-syntax` whether a text parses. `impl/selfhost/compare.sh` runs both formatters on every `.cb`
  file git tracks, as they are, with `--no-wrap`, at `--width 40`, with indentation removed and with tabs,
  doubled spaces and `\r\n` line ends, and on the command line's options and errors, and compares standard
  output, standard error and the exit status byte for byte. The one difference by design is its usage text.
  It is the first part of the toolchain written in CobaltC; the Rust `cobfmt` stays the reference.

- `cobfmt --width N` looked for the tokens of each over-long line among every token of the file, once per
  line it broke; it now walks forward with the lines, the result unchanged (a 2,400-line file at
  `--width 40`: 10.8 s to 3.0 s). The CobaltC formatter does the same.

## What changed for programs

Nothing.

## Found on the way

`CHG-0345`, a `cobc` internal error on a `Vec<String>` compared by element. And a cost of compiled code that grew with the program's history: the runtime left an entry for each
object reclaimed from a vector's element and moved away, and every later frozen-reference call or view
over that memory read them all; fixed as `CHG-0349`. And a limit of the parser, fixed as `CHG-0347`: a
module file destructuring a struct of a sibling module did not parse on its own.
