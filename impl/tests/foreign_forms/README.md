# Foreign forms (CHG-0445)

One short program per habit from another language (Rust, C, C++, Go, Python, Java), each with that one form
in it, and the first line of the diagnostic CobaltC gives it. The message should name the CobaltC form.

- `make_cases.py` writes `cases/` (run it after editing the list).
- `run.sh` checks each case (`TOOL="… --check"`, default coby) and prints `name | habit | first line`;
  `run.sh --compare expected.txt` exits 1 when a line differs (the gate's `records` unit runs it).
- `expected.txt` is the current table; `before.txt` is Update 187's, before CHG-0445.

`impl/tools/card.py` puts the table in CARD.md.
