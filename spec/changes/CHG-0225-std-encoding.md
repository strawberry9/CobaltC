# CHG-0225 — `std::encoding`: JSON moved to `std::encoding::json`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-09; D-0197)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0197
Affects: `spec/21` 4.48.0 §0, §2r; `spec/conformance.md` 3.190.0; the guide; `impl/std/std.cb`, `impl/std/encoding.cb`, `impl/std/encoding/json.cb`, `impl/src/prelude.rs`

## What changed

- **`spec/21` §0 `rule.stdlib.prelude`:**
  - The listing declares `encoding` (with `json`). It also gains the
    `database` and `compress` lines it was missing.
  - The submodule table replaces `std::json` with `std::encoding` and
    `std::encoding::json`.
  - The "Nested submodules" paragraph names `std::encoding`.
- **`spec/21` §2r `rule.stdlib.json`:** in `std::encoding::json`. The
  rule id and every item are unchanged.
- **`spec/conformance.md`:** `conf.std-encoding-json-paths`.
- **`impl`:**
  - `std/json.cb` moved to `std/encoding/json.cb`.
  - `std/encoding.cb` declares and re-exports it.
  - `std.cb` declares and re-exports `encoding` in place of `json`.
  - `src/prelude.rs` has the two new file entries.

## What did not change

No item, rule, key, message or behavior. `Json` is still `std::Json`,
and `import std;` brings in what it did. `std::json` still resolves, as
an alias that `std`'s re-export of `std::encoding` makes (D-0197).
