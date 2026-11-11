# CHG-0371 — `replace`'s native bodies make `[Write]`'s check

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-24)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `rule.stdlib.swap` (`[Swap-Places]`, `[Swap-Places-Conflict]`), D-0235, D-0200
Affects: `spec/conformance.md` (three rows, 3.271.0); `impl/cobc/src/lower/native_text.rs` (`native_replace`);
`impl/selfhost/back/native_text.cb` (the same in `cbc`)

## What changed

No rule changed. `replace(r, v)` is `swap_places(r, &mut old)` with `old` bound to `v`, and D-0235 made each
place of a swap a `[Write]`: a live shared reborrow of `*r` beside `r` passed on whole is
`[Swap-Places-Conflict]`'s `diag.aliasing-conflict`, as `coby` reports it for every type. The native body
`cobc` and `cbc` give `replace` for a value an objectless local may hold (D-0200: a `String`, a `Vec` of
plain data) checked only that `r` was still valid, so such a program ran on and the reborrow saw the new
value; the body for numbers that P4 added the same day did the same (`conf.replace-passed-ref-beside-shared-reborrow`
regressed under `cobc`, found by the battery). A struct of plain data took the objectless body, which
received an object the caller never sends, and the runtime stopped with an internal error on any such
`replace`. Both bodies now check `r`'s validity and then its write (`cb_write`, as `swap_places` does; the
fresh `old` can fail neither), and plain data, a struct of numbers included, takes the body with nothing
received or sent.

## What changed for programs

Under `cobc` and `cbc`: `replace` beside a live shared reborrow faults `diag.aliasing-conflict` at the call,
as under `coby`, for a `String`, a `Vec`, a number or a struct; `replace` on a struct of plain data runs.

## Found by

The battery of Update 134 (`conformance under COBALTC_VERIFY (cobc, gcc)`, `trace_compare`, and the model
sample's `hand-r04.cb`), then probes of the other types, 2026-10-24.
