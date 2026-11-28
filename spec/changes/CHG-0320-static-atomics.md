# CHG-0320 — `static` atomics, and the address of a constant

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §5, §21
Depends on: D-0036, D-0249, D-0253, D-0258 (part B)
Affects: `spec/22` §3 (`static-decl`, `static` a keyword), `spec/17` §1a′ (`rule.module.static`) and §1a,
`spec/09` (`[Ref-Form-Static]`, `[Ref-Form-Const]`), `spec/04` (`static-item`, storage-kind `static`),
`spec/15` (`Σ_0`), `spec/10` §1, `spec/19` §2a, the diagnostics registry, `spec/conformance.md` (eight rows);
`impl/src/{lexer,ast,parser,modres,consts,typecheck,lib}.rs`, `impl/src/interp/{eval,borrows,objects}.rs`,
`impl/cobc/src/lower/{statics,exprs,user_calls}.rs`, `impl/cbrt/src/lib.rs` (`cb_frame_unhold` of token 0);
the guide

## What changed

**`static Atomic<T> NAME = literal;`** and **`static AtomicBool NAME = false;`** (`spec/17` §1a′): one object
for the program's whole run, established in `Σ_0`, reached by name from any function on any thread, never
ending. Only atomics: the one shared mutable state outside raw memory is changed through operations that
are sequentially consistent and form no path, so every thread reaches it with nothing lent. `&NAME` is a
shared reference with program extent; `&mut NAME` is `diag.write-through-shared`; a static read or moved
whole is `diag.read-of-resource`; another type, or a first value that is not a literal of the lane type,
is `diag.type-mismatch` (out of range: `diag.literal-out-of-range`). `static` becomes a keyword (it was a
C-ism error).

**`&K` of a constant** (`spec/09` `[Ref-Form-Const]`): when `K` is plain data (`spec/12` §1 `plain`) whose
initializer is written out as literals (numbers, `bool`s, arrays and structs of them), `&K` is a shared
reference to one object with program extent, a table with one address, held, passed and indexed as any
reference; `&mut K` is `diag.borrow-of-non-place`. (`cobc` already laid an indexed integer-array constant
out once as C static data; what was missing was its address. A computed constant's `&K` stays a temporary
borrow.)

**Implementation.** `coby`: the static's object is made on its first use (`static_object`), owned by the
main thread and never ended; an addressed constant's on its first borrow. `cobc`: a C global (`g_NAME`,
`k_NAME`, the latter `static const`) and `((cb_ref){ &g, 0 })`, token 0, as a `str`'s slice is: static data
that never ends and no path can write, so every check on it is proven; `cb_frame_unhold` now accepts token
0 as `cb_frame_hold` did.

**The model** (`impl/model`, D-0219): `static Atomic<i32> NAME = k;` in its fragment: one object in `Σ_0` with a
shared root path, `&NAME` from any thread and from closures with no capture (`[Ref-Form-Static]`), `&mut NAME`
and a whole read rejected as the tools reject them; the enumeration's alphabet gained the static's uses.

## What changed for programs

A new item form and a new keyword; `&K` of a literal constant, refused before, is accepted. No existing
program changes (none used `static` as a name: it was an error).

## Found by

D-0258 (the low-level pieces), part B, 2026-10-17; `stress/round10/statics/` and the eight rows.
