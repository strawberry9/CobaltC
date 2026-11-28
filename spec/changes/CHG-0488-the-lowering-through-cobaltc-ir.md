# CHG-0488 — The lowering through CobaltC IR (CIR)

Status: implemented (2026-11-28); Update 216
Affects: `spec/decisions/D-0342`; `impl/cir/` (new); `impl/back/` (all of it); `impl/cobc/main.cb`,
`impl/cobc/driver.cb`; `impl/front/dump.cb`; `impl/tests/cases/cobc/emit_cir_core.case`;
`impl/tests/checks/store_env.py`; `README.md`; `CARD.md`; `impl/STATUS.md`.

## Why

D-0342 §1: the compiler had no typing of its own and probed for every type it needed before writing; it lowered a
decided program twice to learn that it is decided; and every pass after the lowering read the generated C back as
text. The work was done in a copy of the tree while `cobc` stayed frozen for fixes (Updates 213 to 215 are the
faults it found on the way); this Update brings it back.

## What changes

- **The typer** (`impl/cir/typer.cb`, `typer_place.cb`, `typer_call.cb`, `typer_flow.cb`): the type the lowering
  gives an expression, as a query that lowers nothing. The lowering asks it wherever it probed; the probes, their
  discarded buffers and the rollback of the lowering's state are gone.
- **The verdict walk** (`impl/cir/reach.cb`): the instances, closures, types and checked call sites a program
  reaches, found by typing alone, and from them whether the program is decided. `generate_program` lowers a
  program the walk finds decided once; where that lowering asks for something the walk did not reach, or does not
  stand as decided, the program is lowered the two-pass way as before.
- **Protocol CIR** (`impl/cir/proto.cb`, `pemit.cb`, `rt.cb`): a function's statements, and then the whole
  program's, as nodes with a kind and parts, built by the places that emit them (630 of 684; the rest are
  classified from their text) and printed once at the end. `rt.cb` names the runtime's 238 entries with what the
  passes ask of each.
- **The passes on nodes** (`impl/cir/ppass.cb`, `ppass_loops.cb`, `pprog.cb`, `pprog_stack.cb`): every pass over
  a function's body and over the program works on the nodes; the text forms they replace are removed. What stays
  text is said at the top of `impl/back/passes.cb`.
- **The text forms.** `cobc --emit-cir=core [-o OUT] FILE`, `cobc --emit-cir=proto [-o OUT] FILE`,
  `cobc --from-cir [--c-name NAME] [-o OUT] FILE.cir` (D-0342 §2.5; `impl/cir/pdump.cb`, `gen::emit_core`,
  `dump::dump_block_typed`).
- **Developer settings**, reports on the compiler's standard error that change no program:
  `COBALTC_TYPER=check` (the typer's answer against the type the lowering computes, expression by expression) and
  `=stats`; `COBALTC_VERDICT=check` (the walk against the lowering) and `=stats`; `COBALTC_PROTO=stats`.
- A question the typer cannot answer stops the compiler with an internal error naming the reason and the place
  (status 4). None arises over the suites.

The generated C: for a program that keeps its paths, as before. For any program, what only a probe had asked for
is no longer written (a function instance, a closure's struct and its branch in a thunk, a variant table, an
unused label), temporaries are numbered without the holes probes left, and the stack check of a chain may sit in
another function of it. No instruction count of the bench or the perf guide moves by a hundredth of a percent.

## What was measured

On the build of this Update against Update 215's, both built by `impl/build.sh`:

- **The compiler compiling one source** (Update 215's `impl/cobc/main.cb`, `--emit-c`, a quiet machine): 351.9 G
  instructions, 153.4 s and 1,332 MB at its peak before; 198.2 G instructions (−43.7%), 88.6 s and 789 MB after.
  The C is 123.72 MB before and 123.59 MB after.
- **The programs it writes.** The bench (20 instruction counts) and the perf guide (72 rows): none more than 3%
  above or below Update 215's; the totals differ by less than 0.01%.
- **Decided programs.** Over the 7,371 programs of the decided sweep: 5,984 decided, 1,115 refused, 272 not
  decided, each program's verdict as before.
- **The suites**, under GCC and under Clang: the conformance cases, the specification's rows, the recorded
  programs, the tools' cases, `std`'s tests, the showcases, the book's examples and the differential tester, as
  before.
- **The code.** `impl/back` 64,092 lines before, 62,588 after; `impl/cir` 12,915 lines, new.

## What is left

- D-0342 §4: `cobc --check` with the typer; values, drops and compound assignments as nodes; a native code
  generator.
- A closure in a generic function whose value is one of two closures, with no result type written
  (`auto choose = move [](i64 a) { if (…) { return move [a](i64 b) { a - b }; } move [a](i64 b) { a + b } };`), is
  accepted by the check and fails in the C compiler: in a generic body the `fn` type the check gave the result is
  not taken. Outside a generic function it compiles. Listed in `impl/STATUS.md` with the `pick` gap of CHG-0486,
  to which it is kin.
