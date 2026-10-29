# CHG-0359 — A plain copy made by `clone` keeps its type in coby

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0116, D-0060
Affects: `spec/conformance.md` (one row, 3.262.0); `impl/src/interp/intrinsics.rs` (`clone`)

## What changed

No rule changed. `coby` gave the `clone` of a value that is not a resource (a plain struct, an enum, an
array, a `simd`) as a bare value, which does not say its type. A generic it was passed to infers its type
parameters from its arguments' values, so `Box::new(clone(&s))` had none for `T`: it allocated
`Box::cells<T>()` for no type and wrote the struct where nothing was allocated, and the fields read back
were other memory (`0`, `117440512`). `Box::clone` of a `Box` of a struct is `Box::new(clone(Box::get(b)))`,
so every clone of such a box, and of a value holding one, came back wrong; `std`'s own case clones only a
`Box<i32>`. Now such a copy is a typed temporary of its statement, as `?`'s payload is (D-0060). `cobc` was
right throughout. The new row `conf.clone-struct-into-generic` covers the three forms.

## What changed for programs

A program cloning a `Box` of a struct, or passing a plain clone to a generic, gets the values under `coby`
that it got under `cobc`.

## Found by

`coby` running the CobaltC front end (`impl/selfhost/cbparse/main.cb`), whose constant folding clones an
enum holding a `Box` of a struct with an enum field: the field matched none of its enum's variants
(`diag.non-exhaustive-match` in `ast.cb`'s `int_ty_name`); 2026-10-20.
