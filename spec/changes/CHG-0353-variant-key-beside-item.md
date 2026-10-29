# CHG-0353 — A bare variant whose name is an item's is written with its enum

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-20)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: CHG-0334, CHG-0041, D-0055
Affects: `spec/conformance.md` (one row, 3.258.0); `impl/src/modres.rs` (`resolve_variant`);
`impl/selfhost/front/modres.cb`

## What changed

No rule changed. Name resolution writes a bare variant reference with its key, the variant's own name
unless more than one enum has a variant of that name, when it writes `E::name`. A name that was also an
item's key was written bare too, and then read as that item: CHG-0334's test of whether a value position
names a type (an enum's key) took it for the enum. So a program declaring at its root an enum named as a
variant `std` writes bare in its own code (`enum Refused`, beside `NetError::Refused` in a unittest of
`std::tls`) was rejected with a type error inside `std` (`--> <std>/tls.cb:61`), and a program's own bare
variant named as one of its root types was rejected as that type, where CHG-0334 says the bare name is the
variant. Such a variant is now written `E::name`, which names it whatever else has its name. The new row
`conf.root-type-named-as-variant` covers both.

## What changed for programs

A program with a root type named as a variant `std`'s code writes bare is accepted; one that
writes its own variant bare where a root type has its name gets the variant.

## Found by

The name resolution written in CobaltC (CHG-0352), which reads `std`'s declarations without their bodies, and
so resolved `showcase/tier2/file_modules/ledger.cb` (a module file that declares `enum Refused`), checked on
its own, where `coby` rejected it; 2026-10-20.
