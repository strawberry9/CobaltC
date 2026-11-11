# CHG-0426 — `cobfmt` asks whether a file parses without parsing `std` again for it

Status: implemented (2026-11-10), on the owner's report (from another agent) that `cobfmt` reads the `std` prelude
twice, unlike `cobfmt-cb`
Affects: `impl/src/parser.rs` (`parses_after_prelude`; `prelude_skeleton`'s text split out as `skeleton_text`),
`impl/src/check.rs` (`syntax_after_prelude`), `impl/cobfmt/src/main.rs`, `impl/cobfmt/src/verify.rs`. No rule changes.
No file is formatted differently and no message changes.

## What was wrong

`cobfmt` refuses to format a file that does not parse, and checks that what it writes parses. Each check
(`check::syntax`) put the whole prelude, all of `std`'s source (about 28,000 lines), in front of the file and parsed
both, so a file cost two full parses of `std`, the second even when formatting changed nothing. `cobfmt-cb` asks the
parser written in CobaltC, which pre-scans the prelude's skeleton (its braces, modules, constants and type names) in
front of the file's tokens and parses only the file (`parse_text`), and asks again only when the text changed.

## What changes

1. **`parser::parses_after_prelude(src)`**: the skeleton (`prelude_skeleton`'s text, which `--prelude-summary`
   checks gives the pre-scans the whole prelude's tables) is lexed once in a process and put in front of the file's
   tokens; the parse starts at the file's first token. It answers whether the file parses, as `parse` of the prelude
   and the file does, with the file's own positions; it returns no program, so the counters the prelude would leave
   (which name hidden bindings only) are not needed.
2. **`check::syntax_after_prelude`** gives `check::syntax`'s answer and messages through it; `cobfmt` uses it.
   `coby --check-syntax` and `cobc --check-syntax` keep `check::syntax`, the whole prelude parsed: it is the oracle
   `compare_parser.sh` holds the parser written in CobaltC, and so the skeleton's method, to on every tracked file.
3. **`cobfmt` does not parse its result again when it is the original text**, as `cobfmt-cb` does not.

## Measured

`cobfmt --check` over the eight directories the gate's format unit checks (Update 176's binary, then this one): the
same output and exit status in each, the 53 "does not parse" messages of `impl/conformance` included; 312 s → 7.3 s
in all (`impl/conformance` 246.5 s → 1.5 s, `impl/selfhost` 18.9 s → 4.2 s). One small file, a process each: 353 ms
→ 116 ms, most of what is left lexing `std` once for its skeleton (`cobfmt-cb`, which reads a generated skeleton,
10 ms). `cargo test -p cobfmt`: 35 passed.
