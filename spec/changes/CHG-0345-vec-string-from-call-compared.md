# CHG-0345 — A `Vec<String>` bound from a call and compared by element

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0108, D-0196, D-0200
Affects: `spec/conformance.md` (one row); `impl/cobc/src/lower/frames.rs`, `impl/cobc/src/lower/stmts.rs`

## What changed

`[Cmp-Text]` is unchanged. `cobc` stopped with an internal error ("a resource binding without an object")
on a `Vec<String>` local initialised by a call to a function of the program and compared element by
element (`Vec<String> ca = f(…); … ca[i] == cb[i]`), nothing borrowing an element otherwise. Its analysis of
locals that need no tracking object (D-0200) found `ca` a candidate, and the `let` asked `f`'s untracked
form (`__raw`, D-0196) for the value; the binding then kept `ca` tracked, as it keeps any indexed vector of
elements that are not plain data, and found no object to track. The two now ask one question
(`Fx::objectless_binding`): the untracked form is called only when the binding will be untracked. The new
row `conf.vec-string-elements-compared` covers `==` and `<` on such elements. `coby` was already right.

## What changed for programs

Programs that `cobc` refused now compile and run as under `coby`.

## Found by

`impl/selfhost/cobfmt/verify.cb`, the comment comparison of cobfmt written in CobaltC (2026-10-19).
