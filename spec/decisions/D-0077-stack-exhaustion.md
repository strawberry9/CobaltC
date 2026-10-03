# D-0077 — Running out of stack is a fault, not a crash

Status: ACCEPTED (2026-09-28, owner-delegated: "fix the frictions using your leanings")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8 (items 1, 2, 10), §12
Depends on: rule.fn.call, spec/18 (checked faults)
Affects: `spec/15` §2, `spec/registry/diagnostics.md`

## Problem

A program recursing a hundred million calls deep crashed both tools.
`coby` aborted with a host stack overflow, and `cobc`'s program was
killed by SIGSEGV. The specification said nothing about running out of
stack, so a valid safe program had an outcome no rule described: a
crash of the host rather than a diagnostic. `cobc` reached only
20 000–50 000 levels of a modest function on the process's 8 MiB main
stack. It went further only when the C compiler turned the recursion
into a loop.

## Candidate mechanisms

1. **A checked fault, `diag.stack-exhausted`, at the call**, with an
   implementation-defined depth limit and a guaranteed minimum.
   Selected. The limit is a property of the machine, as memory is for
   `diag.alloc-failure`.
2. **A fixed depth limit in the language.** Frames differ in size, and a
   limit low enough for every implementation would be needlessly low.
3. **Leave it.** A crash is not a defined outcome.

## Selected design

- `[Call-Stack-Exhausted]` (`spec/15` §2): a call the implementation
  cannot give a frame faults with `diag.stack-exhausted` at the call, and
  the program terminates as for any checked fault.
- **Guaranteed minimum:** 10 000 nested calls with frames of at most
  1 KiB of locals.
- **The tools:**
  - **`coby`** checks the remaining stack at each function and closure
    call, against the stack of the thread it runs the program on (2 GiB
    reserved, 512 MiB for a program thread).
  - **`cobc`**'s generated functions check the stack pointer against a
    per-thread floor at entry (`cb_stack_check`, one comparison). The
    program runs on a thread with a 256 MiB reserved stack (`cb_run_main`),
    and program threads get 64 MiB.

## Compatibility impact

Programs that crashed now terminate with a diagnostic, and native
programs recurse ten times deeper than before. No program that ran
changes.

## Revisit conditions

None.
