# CHG-0372 — A sent datum's temporary stops naming its storage

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-25)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `rule.fn.return` (`[Return]`), `[Block-Exit]`, D-0200, D-0277
Affects: `spec/conformance.md` (one row, 3.272.0); `impl/cbrt/src/lib.rs` (`cb_send_datum`, `unname_sent`);
`impl/cobc/tests/returned_view_inlined.rs`

## What changed

No rule changed. When a function returns reference data (a `StringView`, a slice, an `Option` of one) and
when a call copies such an argument, the sender's side makes a temporary object over the value, sends the
value's paths on (`cb_send_datum`, received by `cb_recv_datum` into the caller's binding) and leaves the
temporary to end with the caller's statement, as every temporary does. Until now that object kept the
address of the storage it was made over, and its end forgot the reference slots in that range. For a
callee's result that storage is the callee's local, dead at the return; harmless while nothing lived there,
but the C compiler owns the stack. GCC at `-O1`, and with link-time optimisation at every level above `-O0`
since D-0277 compiles a program in parts, inlines the callee into the caller and gives the dead local's
storage to a later local of the caller. The returned view received into that local then lost its path's
holder at the end of the statement that bound it: a `[Write]` of its root beside it was not
`[Alias-Conflict]`, and the view read the written text. The runtime no longer lets the temporary name that
storage once the datum is sent (`unname_sent`: the sender's own, non-resource, non-reclaimed temporary at
that address in the current statement), so its end forgets nothing there; the paths it sent are the
receiver's, and a resource is moved by its receiver, not by this step. The generated C is unchanged.

## What changed for programs

Under `cobc` and `cbc` (one runtime): a sub-view a function returns keeps its holder in the caller however
the C compiler lays out the callee's storage, and a write to its root beside it faults
`diag.aliasing-conflict`, as under `coby`. Before, under GCC at `-O1` in the default form, and at `-O2` in
the bootstrap forms 1 to 10, `conf.returned-view-beside-write` printed `h` twice and ended normally.

## Found by

Fault probes of a prototype that gave views bound to locals no runtime object (P4, 2026-10-25), which
disagreed with `coby` at bootstrap levels 1 to 10 under GCC; then the same program at `-O1`, the default
form. Clang's builds happened to keep the storage apart. The test `returned_view_inlined` compiles the
conformance case at `-O1` and `-O2` under both C compilers and expects the fault each time.
