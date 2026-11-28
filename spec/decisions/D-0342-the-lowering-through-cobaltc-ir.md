# D-0342 — The lowering through CobaltC IR (CIR)

Status: the direction is the owner's (2026-11-27: a typed IR between the checked program and the C, named
"CobaltC IR (CIR)", built in a copy of the tree while `cobc` stayed frozen for fixes; every decision of the work
delegated to the implementer, "nobody uses CobaltC yet, so all necessary changes are allowed"). §2 decided by the
implementer under it. Implemented by `CHG-0488`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0284, D-0285 (the decided lowering), D-0309 (the passes over the program's C), D-0326 (the compiler
written in CobaltC), D-0338 (one C file by default)
Affects: no rule of the language; `impl/back`, `impl/cir`, `impl/cobc`; the compiler's options

## 1. Why

`cobc` lowered the checked program to C in one step and kept no form in between. Three things followed. It had
no typing of its own: where a type was needed before anything could be written it *probed*, lowering the
expression into a buffer it threw away and rolling its state back, at every level of nesting. It learnt whether a
program is decided (D-0285) only by lowering it, so a decided program was lowered twice. And every pass after
the lowering (D-0309 and the passes before it) read the C back as text, line by line, recognising its own output
by its spelling: the three faults of Updates 213 to 215 that were missed checks or wrong C were of that kind (a
label written `L:;` not taken for a label, a name kept past its function, callers the text scan did not see).

## 2. Decision

1. **Two levels between the checked program and the C.** *Core CIR* is each reached function instance's checked
   tree with the typing tables beside it: the type of every expression, the instance each call resolves to, the
   closures' records. *Protocol CIR* is the lowered program as statements: each a node with its kind (a
   declaration, an assignment, a call, a runtime entry, a guard, a jump, a block's opening and end, …), its
   indentation and its parts, in the order the C has them.
2. **The typer.** One function gives the type the lowering gives an expression, without lowering it
   (`impl/cir/typer*.cb`). It was derived from the lowering rule by rule and checked against it on every
   expression of the corpus and of the tools (3.4 and 1.8 million; none different). The lowering asks it wherever
   it used to probe. No probe is left.
3. **The verdict before the lowering.** A walk of the program that types and lowers nothing else
   (`impl/cir/reach.cb`) reaches every instance, closure, type and checked call site the lowering would, and from
   them says whether the program is decided. A program it finds decided is lowered once, decided; the lowering is
   then compared with what the walk reached, and if it asked for anything the walk did not see, or does not stand
   as decided, the program goes the two-pass way as before.
4. **The lowering writes nodes, the passes read them.** Each place that emits a statement says what it emits
   (`impl/cir/pemit.cb`); a table names the runtime's entries and what the passes need to know of each
   (`impl/cir/rt.cb`); the passes after the lowering work on the nodes' kinds and parts (`impl/cir/ppass*.cb`,
   `pprog*.cb`), and the C is printed once, at the end. What is still text and why: the functions the compiler
   writes whole (thunks, descriptors, drop glue, the tail), and the inside of an expression, which stays a
   spelling.
5. **Both levels can be written out.** `cobc --emit-cir=core FILE` prints core CIR; `cobc --emit-cir=proto FILE`
   prints protocol CIR, and `cobc --from-cir FILE.cir` prints the C of one. The round trip gives the same C.
6. **The way it was done: transform a copy, not rewrite.** Each step left a complete compiler whose C was
   compared byte for byte with the released one's over the corpus and the tools, at the default form and with
   every check on; the steps that change the C on purpose (no probe leaves anything behind any more) were compared
   by shape and by running every suite. The released compiler was read closely on the way: Updates 213, 214 and
   215 are the six faults that reading found. The steps, as the sources' comments name them: T1 the typer; T2
   the probes ask it; T3 the verdict before the lowering; T4 protocol CIR (stage 0 and A: a body's statements as
   nodes; B: the whole program's, printed once; C: each place that emits says what it emits); T5 the passes on
   the nodes; T6 the probes and the text passes removed; T7 the copy brought back as this Update.
7. **Nothing of the language changes.** The same programs are accepted and refused, and a program's C is what it
   was but for what only a probe had asked for (an unused function instance, a closure's struct, a table).

## 3. What was measured

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

## 4. What is left

- `cobc --check` runs the front end alone; with the typer and the walk it could report what today only the
  lowering refuses.
- An expression's value is a spelling with no parts; a drop shares a line with its note's end; a compound
  assignment has no kind. About 3.5% of the compiler's own statements are still classified from their text.
- A native code generator reading core CIR, the reason the IR was first proposed, is not started.
