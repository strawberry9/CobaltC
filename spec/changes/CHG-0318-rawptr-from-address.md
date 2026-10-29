# CHG-0318 — An address as a raw pointer, and back

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §5, §21
Depends on: D-0121, D-0258 (part A)
Affects: `spec/20` (`rule.trust.rawptr`: two rules, `[Rawptr-Offset]`'s side condition), `spec/21` §0 (two
signatures), `spec/conformance.md` (two rows); `impl/src/typecheck.rs`, `impl/src/interp/intrinsics_raw.rs`,
`impl/cobc/src/lower/intrinsics.rs`; the guide (§20 "Raw pointers")

## What changed

Two intrinsics, both inside `unsafe`: `rawptr_from_address<T>(usize a) : rawptr<T>` (`[Rawptr-From-Address]`:
nothing checked, and nothing can be; the program vouches for what lives at `a`) and `rawptr_address(p) : usize`
(`[Rawptr-Address]`: the address as an integer, to print, align, hash or keep; beside its twin because it
reveals the layout). `[Rawptr-Offset]`'s side condition now also admits a region the program obtained with
`rawptr_from_address`, whose bounds are its own to keep. With `read_volatile` and `write_volatile` (D-0121)
this is memory-mapped I/O: `write_volatile(rawptr_from_address<u32>(0x1000_0000) + 1, 0x80)`.

`coby`'s storage is simulated: it makes the pointer when the address names storage it gave out (a place, an
allocation, interned text), so a round trip through `rawptr_address` works there, and refuses the call as
`unsupported` for any other address, saying so, rather than reading something else. `cobc` casts through
`uintptr_t`.

New rows: `conf.rawptr-address-roundtrip`, `conf.rawptr-from-address-outside-unsafe`.

## What changed for programs

Two names a program could not have declared as its own functions (every intrinsic's name is reserved); no
existing program changes.

## Found by

D-0258 (the low-level pieces), part A, 2026-10-17.
