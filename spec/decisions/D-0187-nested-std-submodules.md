# D-0187 — Nested `std` submodules, and the implementation-defined `std::extensions`

Status: ACCEPTED (2026-10-08, the owner: "Proceed with your recommendations, except make std:ext to be std::extensions", on `private/std-submodules-2-proposal.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §18
Depends on: D-0136 (`std` divided by subject, re-exported by its root), D-0176 (no catch-all submodule), D-0151 to D-0170 (`std::crypto`), D-0173, D-0174 (`std::http`), D-0180 (`std::database`)
Affects: `spec/21` §0, §2m, §2p, §2q (4.45.0); `spec/17` §3; `spec/00` (1.2.0, `term.implementation-dependent-program`); `spec/conformance.md` (3.177.0); the guide's library chapter; `impl/std/std.cb`, `crypto.cb` and `crypto/`, `http.cb` and `http/`, `database.cb` and `database/`, `extensions.cb`; `impl/src/prelude.rs`, `impl/src/modres.rs`; `CHG-0215`

## Problem

Three of `std`'s submodules had outgrown one subject. `std::database`
held only PostgreSQL (`Pg*` items and SCRAM), promising a generality it
did not have and leaving a second database nowhere to go. `std::crypto`
was a quarter of `std` in one file of 4,790 lines with six groups that
share nothing but the word. `std::http` was a client and a server over
shared messages. And an implementation (or a customer's build of one)
that wants to ship modules the specification does not define had no
place inside `std` for them, only a fork of it.

## Candidate mechanisms

1. **Nested submodules, re-exported by their parent, items keeping their
   short names and keys.** Selected. `std::crypto` → `digest`, `kdf`,
   `aead`, `pk`; `std::http` → `client`, `server`; `std::database` →
   `postgres`, with what the children share declared in the parent (the
   SCRAM client, the message reader, the hash-size tables). No program
   changes: the corpus writes 23 qualified `std::x::` paths, none to
   these modules, and the root still re-exports everything.
2. **Also drop the `Pg` prefixes and stop re-exporting the database
   clients at the root** (`import std::database::postgres;` then
   `Connection`). Not now: it breaks every program and makes the first
   exception to "`import std;` brings in the whole library". A later
   decision if several clients make the prefixes grate.
3. **Split `std::collections`, `std::text` and the rest too.** No: one
   subject each; paths without clarity.
4. **`std::extensions`, declared and not re-exported.** Selected. The
   area exists in every implementation, may be empty, holds nothing the
   specification defines, and conformance tests nothing in it. Not
   re-exported, so a program reaches an extension only by naming it and
   its dependence on one implementation shows in its source; items there
   keep their full paths as keys, so an extension can never share a key
   with, nor shadow, an item of `std`. A program that names one is
   conforming and implementation-dependent (`spec/00`). D-0176 stands:
   this is not a catch-all for the specification's own items, which are
   never declared there.
5. **The name.** `std::ext` was proposed (short, conventional); the
   owner chose `std::extensions`. `std::optional`, the first idea, would
   be read as the home of `Option<T>`.
6. **The digests' name.** `std::crypto::hash` was proposed; it is
   `digest`, because `hash` is a function of that module and a
   submodule's name re-exported into its parent must not be an item's
   (`[Item-Duplicate]`).

## Consequences

- `spec/21` §0: the listing and the submodule table show the nesting and
  `std::extensions`; a paragraph each on nested submodules and on the
  implementation-defined area. §2m, §2p, §2q name their submodules.
- `spec/17` §3: re-exports nest; `std::extensions` is the one submodule
  `std` declares and does not re-export.
- `spec/00`: `term.implementation-dependent-program`.
- `spec/conformance.md`: `conf.std-nested-submodule-path`,
  `conf.std-extensions-empty`.
- Implementation: the prelude assembler splices nested file-backed
  declarations recursively; module nodes are keyed by their full path
  (`modres::module_key`) while items under `std` keep D-0136's short keys
  and items under `std::extensions` keep full ones. No message, no case
  and nothing the tools name by string changed.
