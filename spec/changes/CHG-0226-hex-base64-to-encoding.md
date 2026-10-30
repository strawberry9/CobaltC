# CHG-0226 — Hexadecimal and base64 move to `std::encoding`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0198)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0198
Affects: `spec/21` 4.49.0 §0, §2d; `spec/conformance.md` 3.191.0; the guide; `impl/std/text.cb`, `impl/std/encoding.cb`, `impl/std/encoding/hex.cb`, `impl/std/encoding/base64.cb`, `impl/src/prelude.rs`

## What changed

- **`spec/21` §0 `rule.stdlib.prelude`:** in the listing, `encoding` is
  now "with json, hex, base64". In the submodule table:
  - `std::text`'s row drops the four functions;
  - `std::encoding`'s row lists them;
  - `std::encoding::hex` and `std::encoding::base64` are new rows.

  The "Nested submodules" paragraph names them.
- **`spec/21` §2d `rule.stdlib.text`:** the section header, `[Hex]` and
  `[Base64]` name their submodules. Rule ids are unchanged.
- **`spec/conformance.md`:** `conf.std-encoding-hex-base64-paths`,
  `conf.std-text-has-no-hex`.
- **`impl`:**
  - The hexadecimal block of `std/text.cb` moved to
    `std/encoding/hex.cb`, and the base64 block to
    `std/encoding/base64.cb`.
  - `std/encoding.cb` declares and re-exports both.
  - `src/prelude.rs` has the two new file entries.

## What did not change

No item's name, key, rule or behavior. `import std;` brings in what it
did. Only `import std::text;` alone stops naming the four functions.
