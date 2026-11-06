# D-0274 — The bootstrap form: `cobc --bootstrap-form`, the C the CobaltC compiler reproduces

Status: ACCEPTED (2026-10-20, decided by the assistant under the owner's delegation of 2026-10-20:
"make all necessary decisions to achieve the goal, by yourself")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9
Depends on: D-0209 (`--all-checks`), D-0188 (dual bodies), D-0200 (raw forms), the self-hosting plan
(decisions a–c of 2026-10-20; cbrt stays Rust until the fixed point)
Affects: `impl/cobc` (a new switch); `impl/selfhost/back/` (the CobaltC code generator); `CHG-0362` onward

## Problem

Step 2 of self-hosting is a code generator written in CobaltC. It needs an oracle as exact as the front end
had (coby's dumps, byte for byte). `cobc`'s output is that oracle only if the CobaltC generator reproduces all
of it: ~22,000 lines of lowering, more than half of them optimisations (native bodies for ~80 `std` functions,
elision analyses, dual and frozen bodies, raw forms, fused loops, text passes, the object cache). Porting
those first would put months of work before the first self-compiled program, and every one of them is an
optimisation with a correct general path beside it.

## Options

1. Port all of `cobc`, compare its C byte for byte. Exact, but the largest possible first step.
2. Write a new, simpler generator in CobaltC, test it only by running programs against `coby`. Smaller, but
   no byte-level oracle: a difference in tracking (an object not ended, a check not made) shows only when a
   test happens to exercise it.
3. A switch in the Rust `cobc` that turns every optimisation off and emits the general checked form
   (`--bootstrap-form`); the CobaltC generator reproduces that form byte for byte; both are also tested by
   running programs. The elided forms are ported later (step 4), each against `cobc`'s own output.

## Decision

Option 3. `cobc --bootstrap-form` (and `COBALTC_BOOTSTRAP_FORM=1`) implies `--all-checks` and the object cache
off, and switches off, in this order, each verified by the suites before the next: the text passes and
scope-marker dropping; the per-expression proofs; the call shortcuts (`user_calls.rs` fast chain, fresh and
raw forms, checked-once, literal calls); the local analyses (confined, objectless, quiet, element-ref lets,
fusion); the native `std` bodies. What remains is the reference for `impl/selfhost/back/`. Programs compiled
in the bootstrap form must pass the same suites as the default form (slower, never different).

## Consequences

- The first self-compiled compiler is slow (every check made), as expected for a bootstrap; speed returns as
  the elisions are ported after the fixed point.
- Turning the natives off compiles `std` bodies `cobc` has never compiled (the native always won): latent
  gaps there are found and fixed in `cobc` first.
- The default form is untouched; the switch is a separate path through the same code.
