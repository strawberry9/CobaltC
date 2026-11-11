# CHG-0292 — coby: `Vec::push`, `Vec::filled` and `Vec::drop` shortcuts; cheaper type lookups

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; asked for by the owner: "fix the Vec::filled/push issue")
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: `spec/21` §1 (`Vec`'s bodies)
Affects: `impl/src/interp/native_vec.rs` (new), `calls.rs`, `destroy.rs`, `access.rs`, `objects.rs`,
`refs.rs`, `eval.rs`, `assign.rs`, `exec.rs`, `frames.rs`, `interp.rs`

## What changed

coby runs `Vec::push` with room in the buffer, `Vec::filled` of numbers or `bool` on one thread, and
`Vec::drop` of numbers or `bool` with no reclaimed element natively, taking the same steps through the same
checks as the `std` bodies; every other case runs the body. `COBALTC_COBY_NO_NATIVE=1` turns the shortcuts
off. Type lookups lend instead of cloning; a `for` step is no longer copied each iteration.

## What changed for programs

No outcome. Speed: `Vec::filled(50000, …)` 7,990M → 512M instructions, a 50,000-push loop 7,678M → 3,368M,
showcases 10–27% fewer instructions (`impl/CHANGELOG.md`).
