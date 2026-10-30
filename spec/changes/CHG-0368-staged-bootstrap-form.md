# CHG-0368 — `COBALTC_BOOTSTRAP_UPTO=N`: the bootstrap form by stages

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-22)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0274
Affects: `impl/cobc/src/lower.rs`, `impl/cobc/src/lower/*.rs` (`Gen::bootstrap`, `Gen::boot`,
`bootstrap_level`); `impl/cobc/src/main.rs` (help text)

## What changed

No rule changed. D-0274 switched `cobc`'s optimisations off in ten stages, each passing the suites before the
next, behind one switch. Each of the switch's 41 tests now names its stage (`Gen::boot(stage)`), and
`COBALTC_BOOTSTRAP_UPTO=N` switches off stages 1..N only; `--bootstrap-form` (`COBALTC_BOOTSTRAP_FORM`) is
level 10, the default form level 0. Two tests in `closures.rs` moved to the stage of what they gate: a
closure's confined `Vec`s to stage 4 (the local analyses), its text passes to stage 1.

Step 4 of self-hosting ports the optimisations to the code generator written in CobaltC from the last stage
back (`private/SELFHOST-PLAN-2026-10-22.md`); level N is the oracle the port's C is compared with, byte for
byte, at each step.

## What changed for programs

Nothing: levels 0 and 10 write the C of Update 130's default and bootstrap forms, byte for byte, for all 901
tracked programs that compile (stress/selfhost4/p0/levels.sh).
