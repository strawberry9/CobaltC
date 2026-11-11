# CHG-0215 — Nested `std` submodules and `std::extensions`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-08; D-0187)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0187
Affects: `spec/21` (4.45.0) §0, §2m, §2p, §2q; `spec/17` §3; `spec/00` (1.2.0); `spec/conformance.md` (3.177.0); the guide; `impl/std/`, `impl/src/prelude.rs`, `impl/src/modres.rs`

## What changed

- **`spec/21` §0 `rule.stdlib.prelude`:** the listing and the submodule
  table show `std::crypto`'s `digest`, `kdf`, `aead` and `pk`,
  `std::http`'s `client` and `server`, `std::database`'s `postgres`, and
  `std::extensions`; the paragraphs "Nested submodules" and
  "`std::extensions`".
- **`spec/21` §2m, §2p, §2q:** each names the submodules its items are
  declared in.
- **`spec/17` §3 `rule.module.use`:** re-exports nest; `std::extensions`
  is declared and not re-exported.
- **`spec/00`:** `term.implementation-dependent-program`.
- **`spec/conformance.md`:** `conf.std-nested-submodule-path`,
  `conf.std-extensions-empty`.

## What did not change

No item, rule or behavior. Every `std` item keeps its short name, its
short path and its key (`std::sha256`), and every longer path to it
names the same item. `import std;` brings in what it did. A program that
names nothing in `std::extensions` is unaffected, and the reference
implementation's `std::extensions` is empty.
