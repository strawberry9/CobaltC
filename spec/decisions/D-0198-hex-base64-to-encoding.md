# D-0198 — Hexadecimal and base64 move to `std::encoding`

Status: ACCEPTED (2026-10-09; the owner asked "are there items in std::text that would be better off in std::encodings::%something%", then of the three options: "go with A, draft and implement it")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §18
Depends on: D-0151 (`to_hex`, `from_hex`, `base64_encode`, `base64_decode`), D-0187 (nested submodules), D-0197 (`std::encoding`)
Affects: `spec/21` §0, §2d (4.49.0); `spec/conformance.md` (3.191.0); the guide; `impl/std/text.cb`, `encoding.cb`, `encoding/hex.cb`, `encoding/base64.cb`; `impl/src/prelude.rs`; `CHG-0226`

## Problem

`std::text` held four functions that turn bytes into text and back:
`to_hex`, `from_hex`, `base64_encode` and `base64_decode`. Everything
else in `std::text` is about text itself. D-0197 point 3 kept them there
and left the move to a later decision.

## Candidate mechanisms

1. **`std::encoding::hex` and `std::encoding::base64`.** Selected. One
   submodule per encoding, as `json` is one per format (D-0187's
   pattern). Go's precedent (`encoding/hex`, `encoding/base64`).
   `import std::encoding::base64;` reads as what it brings in.
2. **One `std::encoding::bytes` for all four.** Not chosen: "bytes" is
   vague, and it breaks one submodule per format.
3. **Leave them in `std::text`.** Not chosen: no churn, but the module
   that is about text keeps byte encodings.

## Decisions

1. `to_hex` and `from_hex` are in `std::encoding::hex`;
   `base64_encode` and `base64_decode` are in `std::encoding::base64`.
   `std::encoding` re-exports both, and `std`'s root re-exports
   `std::encoding`. Their private helpers move with them.
2. `std::encoding` holds data formats and encodings of bytes as text.
   This widens D-0197's description ("a value model read and written
   back") and replaces its point 3.
3. The names stay as they are. `std`'s item names are unique across
   `std`, so `hex::encode` is not available. Keys are unchanged
   (`std::to_hex`).
4. These stay where they are:
   - `percent_encode`, `percent_decode`, `form_encode` and `form_decode`
     stay in `std::http`. They follow URL and form rules (RFC 3986,
     HTML), not general byte encodings.
   - PEM reading stays private to `std::x509`.
5. `[Hex]` and `[Base64]` keep their rule, `rule.stdlib.text` (§2d), so
   no rule id changes. Each names its submodule.

## Consequences

- `import std;` programs are unchanged.
- A program that imports `std::text` alone and calls one of the four now
  gets `diag.unbound-name`. It adds `import std::encoding;` or the one
  submodule. No program in the repository did this.
- `spec/21` §0: the listing, the tables and the "Nested submodules"
  text. §2d: the section header, `[Hex]`, `[Base64]` and the comment on
  the source listing.
- `spec/conformance.md`: `conf.std-encoding-hex-base64-paths`,
  `conf.std-text-has-no-hex`.
