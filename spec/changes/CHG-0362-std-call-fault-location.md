# CHG-0362 — `cobc` reports a `std` call's fault at the call after an argument called a function; `cobc --bootstrap-form`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-21)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0274 (the bootstrap form), D-0209 (`--all-checks`)
Affects: `spec/conformance.md` (two rows); `impl/cobc/src/lower/frames.rs` (`at_after_arg_calls`);
`impl/cobc/src/lower/user_calls.rs`, `vec_calls.rs` (its three call sites); `impl/cobc/src/main.rs`,
`lower.rs`, `lower/program.rs`, `cache.rs` and the lowering files named below (`--bootstrap-form`)

## What changed

No rule changed.

1. A fault that a `std` function's body raises is reported at the call (`spec/21`), as `coby` reports it. When
   one of the call's arguments called a function of the program (`Vec::swap(&mut v, pick(0), pick(5))`), `cobc`
   left the current location where that function had last set it and reported the fault inside it (at
   `pick`'s last line). The call's line is now set again after its arguments, on the general call path and on
   the confined and checked-once forms. The new rows `conf.std-call-fault-after-argument-call` and
   `conf.std-call-through-ref-fault-after-argument-call` cover a local vector and one reached through a
   reference parameter.
2. `cobc --bootstrap-form` (`COBALTC_BOOTSTRAP_FORM=1`), D-0274: the general checked form, the reference the
   code generator written in CobaltC reproduces. It implies `--all-checks` and the object cache off, and
   switches off the text passes, the per-expression proofs (loop-step overflow, constant-array reads, bare guard
   lets), the call shortcuts (`lower_user_call`'s chain, direct arguments, `Box::new` inline), the local
   analyses (confined vectors, objectless strings, element-reference lets, quiet locals, frame-held references,
   fusion, `foreach` aliases, pure views, the compound and `_acc` pairs, `__raw` forms) and the native `std`
   bodies. Kept, because the semantics need them: the element access of `e[k]` (`[Index-Vec]`, D-0070:
   `native_elem_access`, `lower_vec_index_pre` for the sugar) and the discharge of plain locals that are never
   borrowed (`pinned_names`, `spec/14` §6). With no native bodies there are no `_pre`, `_raw`, `_nc` or
   `_once` variants: the sugar's `_pre` body is emitted inline at the call, and the `String` makers' `_raw`
   forms (`lower_raw_string`, `bind_fresh_objectless`) are not used. The default form is unchanged.

## What changed for programs

Under `cobc`, such a fault's message now names the call's line. Nothing else changes for programs.

## Found by

Testing the bootstrap form against the compiled suite
(`conf.vec-element-read-through-ref-param-index-call-dynamic` disagreed with `coby` once the call shortcuts
were off, and the probe showed the default form had the same fault on its confined path); 2026-10-21.
