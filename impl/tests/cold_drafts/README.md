# Cold drafts (CHG-0452)

Four programs written in one pass from `CARD.md` alone, before any of them was checked: a word count, an
expression parser, an inventory with closures and a threaded prime count. Each is kept as first written, with
its mistakes: guessed library names, another language's habits, a `Box` passed where its value was wanted.
They measure what the tools say to a newcomer's first attempt.

- `run.sh` checks each case (`TOOL="… --check"`, default `cobc --check`) and prints its whole report;
  `run.sh --compare expected.txt` exits 1 when the output differs (the gate's `records` unit runs it).
- `expected.txt` is the current output. CHG-0452 records how many `--check` runs each draft took to become
  a working program, applying only what each report said.
