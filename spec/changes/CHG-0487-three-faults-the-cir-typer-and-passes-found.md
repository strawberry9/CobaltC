# CHG-0487 — Three faults the CIR typer and passes found

Status: implemented (2026-11-28); Update 215
Affects: `spec/conformance.md` 3.331.0 (2 rows); `impl/back/gen.cb`; `impl/back/stack_checks.cb`;
`impl/tests/cases/cobc/reports_update215.case`;
`impl/conformance/21-standard-library-semantics/clone_of_map_lookup_by_text_key_ok.cb`;
`impl/conformance/16-aggregates/zero_sized_local_name_reused_ok.cb`.

## Why

`cobc` is frozen for fixes only while the CobaltC IR (CIR) lowering is built beside it. Two parts of that work read
the released compiler closely: the typer, which gives every expression's type without lowering it and is checked
against the lowering over the compiler's own sources, and the move of the whole-program passes from the generated
C's text to a list of statements, each pass kept line for line. They found three faults.

- **`clone` of a map lookup by a text key stopped the compiler.** `clone(Option::unwrap(HashMap::get(m, k)))`
  with `k` a `foreach` element of a `Vec<String>`, or a temporary text borrowed in place
  (`&String::clone(k)`), inside a loop or outside one, ended in `assert: slice of something that is not an
  array, a Vec or a slice` or `a view of a String with no access path`. The check rewrites a shared borrow of a
  text, given where a view of a text is taken, into the slice `&s[0..$]` in place, and what marks the node as a
  slice of a text is one record under the node's id. The lowering copies an expression under new ids in several
  places (`renumbered`: the `clone` intrinsic lowers a copy of its argument), and the copy had no record: it was
  lowered as a slice of an array. A correct program was refused with a compiler fault; no wrong program was
  produced.
- **A zero-sized local's anchored name outlived its function.** A local of a zero-sized type is declared with a
  one-byte anchor and read through it (`l_z_9` as `l_z_9_u.v`, F193-1). The pass that does it kept the anchored
  names to the end of the C unit, not the function. A function lowered in two forms numbers its locals alike in
  both, so with a `u32 z` in one block and a `Z z` of a zero-sized `Z` in another, the `u32` of one form bore the
  name the other form had anchored and was rewritten too (`uint32_t l_z_9_u.v;`): the C compiler refused it, and
  `cobc` ended with "cc failed" on a correct program.
- **A function entered from outside the program's functions could lose its stack check.** The stack check
  (D-0077) is kept only where the depth of calls can grow without bound or where a chain of functions without one
  could use more than 64 KB below the last check (D-0309). The pass read the callers of a function only in the
  bodies of the program's own functions. An `extern fn` that the program also calls lost its check as if `main`
  were its only caller, though C enters it through its stub on a stack of any depth; so did a function called
  from a helper the compiler writes. And a closure's frame was not counted: a function called from a closure
  with a 48 KB frame and from elsewhere went without a check although the two use more than the budget below
  the closure's. The rule D-0309 states did not hold there. Stack exhaustion on such a path is not met by the
  check: it reaches the guard page, where the runtime raises `diag.stack-exhausted` all the same (the frames are
  probed as they are made, so no memory the program does not own is reached). No program in the suites comes
  near it.

## What changes

- **A copy of an expression keeps the record that says what a node is** (`gen::renumbered` carries each node's
  `view_ops` entry to the copy's node). The facts about a node's context are still left behind, as the copies'
  callers expect. **The `clone` intrinsic's copy of its argument keeps all the check's per-node facts**
  (`renumbered_with_facts`): a temporary's borrow inside the argument is lowered as the check settled it.
- **The anchored names end with the function** (`stack_checks::anchor_unit`: at the `}` at the left margin).
- **The stack-check pass reads the other bodies too** (`stack_checks::foreign_calls`). What a closure's code
  (`cl_…`) calls is checked against the closure's frame as against any caller's (a closure that calls anything
  keeps its own check). What any other body calls — the stub of an `extern fn`, a thunk, a helper, the C `main`
  — is entered as an entry is and keeps its check; the program's `main`, entered once on a new stack, is as
  before.

A program none of the three touched compiles to the same C, byte for byte. A function that regains its check is no
longer inlined as a small one (CHG-0430).

## What is left

- The check numbers the expressions it makes from the same base in every body, and the lowering's own ids start at
  that base too. No fault was traced to it (moving the lowering's ids apart changed no generated C); the CIR
  lowering keeps its ids in a range of their own.
- A function that calls nothing has no stack check at all (D-0309), whatever its frame: a leaf with a frame of
  megabytes relies on the frame's probing alone. Left as it is.
