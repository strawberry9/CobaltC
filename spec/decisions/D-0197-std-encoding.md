# D-0197 — `std::encoding`: JSON moved to `std::encoding::json`

Status: ACCEPTED (2026-10-09; the owner: "I think std::json needs to be std::serialization::json so that we can other serializations in the future", then, of the four options, "go with A, draft and implement it")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §18
Depends on: D-0136 (`std` divided by subject), D-0176 (no catch-all submodule), D-0182 (`std::json`), D-0187 (nested submodules)
Affects: `spec/21` §0, §2r (4.48.0); `spec/conformance.md` (3.190.0); the guide's library chapter; `impl/std/std.cb`, `encoding.cb`, `encoding/json.cb` (was `json.cb`); `impl/src/prelude.rs`; `CHG-0225`

## Problem

`std::json` was the one data format at `std`'s top level. A second
format (TOML, CSV, CBOR) would be another unrelated sibling, where
D-0187 groups a subject's variants under one parent
(`std::database::postgres`, `std::http::client`).

## Candidate mechanisms

1. **`std::encoding::json`.** Selected. Go's precedent (`encoding/json`,
   `encoding/csv`). The parent holds data formats: a value model read
   from text or bytes and written back. "Encoding" also means text
   encodings (UTF-8) and HTTP's content encodings, so §0 says what the
   module holds.
2. **`std::serialization::json`** (the owner's first proposal). Not
   chosen: `std::json` reads a document into a `Json` tree; it does not
   turn a program's own types into bytes. If CobaltC ever gets that kind
   of mechanism, `std::serialization` is its name, and this module would
   blur it. It is also the longest parent name in `std`.
3. **`std::format::json` / `std::formats::json`.** Not chosen: "format"
   is already `printf`'s and `sprintf`'s format strings.
4. **Keep `std::json`; later formats as siblings.** Not chosen: no churn,
   but it breaks D-0187's grouping.

## Decisions

1. `std::encoding` holds the data formats, one submodule per format,
   each re-exported by `std::encoding`, and `std::encoding` by `std`.
   `json` is the only one today.
2. `Json` and `JsonMember` keep their names and their keys (`std::Json`).
   `import std;` programs do not change.
3. Byte-to-text encodings (`to_hex`, `from_hex`, `base64_encode`,
   `base64_decode`) stay in `std::text`. Moving them here would be a
   separate decision. (Superseded by D-0198, the same day: they moved to
   `std::encoding::hex` and `std::encoding::base64`.) Compression stays in `std::compress`.
4. A format's submodule name, like any `std` submodule's, must not be an
   item's or another submodule's name that `std`'s root re-exports
   (D-0187 §6, `[Item-Duplicate]`).

## Consequences

- `spec/21` §0: the listing (which also gains `database`'s and
  `compress`'s lines it lacked) and the submodule table show
  `std::encoding` and `std::encoding::json`. The "Nested submodules"
  paragraph names it. §2r is in `std::encoding::json`.
- `spec/conformance.md`: `conf.std-encoding-json-paths`.
- Implementation: `impl/std/json.cb` → `impl/std/encoding/json.cb`; new
  `impl/std/encoding.cb`; `std.cb` and the prelude's file table. The
  tools themselves are unchanged.
- The old path stays reachable. A submodule is an exported item of its
  parent, so `export import` re-exports it (D-0187 §6). `std::encoding`
  re-exports `json`, `std` re-exports `std::encoding`'s items, and so
  `std::json` is an alias of `std::encoding::json`, as `std::digest` is
  of `std::crypto::digest`. No program written against D-0182 breaks.
  The guide and the specification name the full path.
