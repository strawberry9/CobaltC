# CHG-0357 — A program whose generic instantiations do not end is rejected

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0010, D-0012, CHG-0100, D-0240
Affects: `spec/15-function-semantics.md` (1.19.0, §5 `[Generic-Instantiation-Unbounded]`);
`spec/registry/diagnostics.md` (1.61.0, `diag.instantiation-unbounded`); `spec/conformance.md` (one row,
3.260.0); `impl/src/typecheck.rs` (`Ck::run`, `type_depth`); `impl/selfhost/front/typecheck.cb`

## What changed

The type checker works through a list of the functions and instantiations to check, starting with every
function of the program; each call it meets adds its callee. It stopped after 20,000 entries taken from that
list, a guard against a generic function that calls itself at an ever larger type, and then accepted the
program. Two faults followed:

- A large program reached the limit with functions still unchecked, and was accepted. The type checker
  written in CobaltC (about 40,000 lines with its front end) did: `coby --check` and `cobc --check` passed it
  with type errors in it, which `cobc` found only while lowering (by checking the remaining bodies on
  demand), and `coby` reported only at run time, when it first reached such a function.
- The guard did not stop the case it was for. `fn deep<T>(T x, u32 n)` calling `deep(Box::new(x), n - 1)`
  checked `deep<Box<…<i32>…>>` at a type that grew with each entry, so the work grew without bound before
  the count was reached: all three tools ran until killed.

Now no program is accepted with part of it unchecked. The list is worked through to its end, and only
concrete instantiations of generic functions count. A program is rejected statically with
`diag.instantiation-unbounded` when an instantiation's type argument nests more than 64 type constructors,
or when it needs more than 100,000 instantiations: the new rule `[Generic-Instantiation-Unbounded]`
(spec/15 §5), case `conf.generic-instantiation-unbounded`. `deep` is rejected in 0.3 s.

## What changed for programs

A program that was accepted with functions left unchecked gets its type errors at compile time, where it
got them while lowering or at run time before. A program whose instantiations do not end is rejected at
once instead of running the tools without end.

## Found by

The type checker written in CobaltC: `cobc --check impl/selfhost/cbparse/main.cb` passed, then `cobc`
stopped while lowering with an error in a body `--check` had not reached; an error planted in another such
body went unreported; 2026-10-20.
