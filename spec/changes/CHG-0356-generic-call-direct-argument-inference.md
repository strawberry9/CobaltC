# CHG-0356 — A generic call's direct arguments fix its type parameters

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: CHG-0100, D-0188, D-0192
Affects: `spec/conformance.md` (one row, 3.259.0); `impl/cobc/src/lower/user_calls.rs`
(`lower_user_call`)

## What changed

No rule changed. `cobc` lowers some arguments of a call by a direct path that passes a borrow without
minting a path for it (`lower_user_arg_direct`: a static's address, a reader parameter's element read in
place, a native search key, a pure direct parameter's place). Several of these paths returned the argument
without letting its type fix the callee's type parameters, as every other argument does. A generic called
with only such arguments then had type parameters nothing fixed, and `cobc` stopped with an internal error
(`cannot infer type arguments of std::HashSet::len`) where `coby` ran the program:
`HashSet::len(&m[j])`, `m` a `ref<Vec<HashSet<String>>, shared>`, is a reader parameter's element of a
resource type (D-0192). Every argument a direct path lowers now fixes the type parameters, at the one place
the call collects them. The new row `conf.generic-call-reader-element-arg` covers `HashSet::len`,
`HashMap::len`, `Vec::len` and a program's own generic.

## What changed for programs

A program `cobc` stopped on with that internal error compiles and runs as under `coby`.

## Found by

The type checker written in CobaltC (`impl/selfhost/front/typecheck.cb`), whose `ref_end_blocked` calls
`HashSet::len(&mentions[j])`: the program checked, and `cobc` could not compile it; 2026-10-20.
