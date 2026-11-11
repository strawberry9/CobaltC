# D-0223 — The `std` module map is frozen

Status: ACCEPTED (2026-10-13; the owner's answer to Q3 of the 2026-10-12 plan, "yes, frozen"; renames the audit
required delegated to the agent, 2026-10-13)
Kind: Decision (`spec/02-schema.md`); a library stability commitment (`spec/STABILITY.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §20, §21
Depends on: D-0134 (the naming principles), D-0136 and D-0187 (submodules), D-0180 (unique variant names),
D-0220 (stability policy)
Affects: `spec/21` §0, `impl/std/std.cb`, `impl/std/tls.cb`, `impl/std/http.cb`, `impl/std/http/client.cb`,
`impl/std/http/server.cb`, `impl/std/crypto/kdf.cb`, `impl/std/database/postgres.cb`, `spec/conformance.md`
(CHG-0264)

## Problem

`std` has been reorganised five times (D-0134, D-0136, D-0176, D-0187, D-0197). A program, a guide and a
review all depend on where an item lives and what it is called; while either may change, `std` cannot become
*candidate* (`STABILITY.md`). Before the map is declared fixed, it is checked once more against the principles
`std` was aligned to, so that no rename it needs is left for after the freeze.

## Options

1. **Freeze the current map after an audit; make every rename the principles require now.** Selected.
2. Freeze without an audit: any later inconsistency is permanent or needs a deprecation.
3. Leave the map open until the std reference (WI-18) is written.

## Decision

- The submodules (`core`, `collections`, `text`, `io`, `fs`, `env`, `memory`, `sync`, `random`, `math`,
  `time`, `process`, `net`, `crypto::{digest, kdf, aead, pk}`, `x509`, `tls`, `http::{client, server}`,
  `database::postgres`, `encoding::{json, hex, base64}`, `compress`, and `extensions`, never re-exported) and
  the module each exported item lives in are fixed. Items may be added; a module may be added for a subject
  `std` does not cover; nothing moves or is renamed except through `STABILITY.md`'s deprecation window.
  `spec/21` §0 says so, and `impl/std/std.cb`'s comment names its lists as the map.
- Bare variant names stay unique across `std`.
- The renames below are made in this edition, before the freeze.

## Rationale

A map that has stopped moving is what lets the guide's §21 inventory, the std reference and the conformance
rows be written once. The principles are D-0134's; the audit applied them mechanically to every exported item,
so the freeze carries no known inconsistency.

## Results

The audit (`stress/plan_oct12/WI-5/audit.py`): 583 exported functions, 21 exported enums, 46 exported structs.

**Variant names** (D-0180: `std`'s bare variant names are unique, since `std`'s resolver refuses an ambiguous
one). Violated: `NetFailure` was a variant of `TlsError`, `HttpError` and `PgError`, and `TlsFailure` of
`HttpError` and `PgError`, so a bare one was `diag.ambiguous-name` even where the expected type named the
enum. Renamed, each wrapping variant carrying its enum's name as `Json…` does:
`TlsError::NetFailure` → `TlsNetFailure`; `HttpError::NetFailure` → `HttpNetFailure`; `HttpError::TlsFailure`
→ `HttpTlsFailure`; `PgError::NetFailure` → `PgNetFailure`; `PgError::TlsFailure` → `PgTlsFailure`.

**Text a function only reads is a `StringView`** (D-0134 §1). Changed from `str`:
`Header::new`'s name, `Request::new`'s method, `http_post`'s content type, `hkdf_expand_label`'s and
`hkdf_expand_label_with`'s label, `PgResult::column`'s name (and the private `derive_secret`'s label). A
literal argument is unaffected (`[Str-Literal-View]`, also through an `if` of literals); a `str` binding passed
to one needs `StringView::of(t)`.

Considered and kept:
- `Option::expect`, `Result::expect` (`str message`): the message is `fault`'s, which takes a `str`.
- `Args::flag`, `Args::option`, `Args::options` (`str` name, short form, help): kept for the help text, not
  only read.
- `find_param`, `find_header`, `Request::header`, `Response::header`, `PgConnection::parameter` (`str` name):
  each returns a view of its first argument, so by `rule.temporal.elision` it takes one borrowed parameter only
  (as `split`'s separator, D-0134).
- The `_str` twins (`String::eq_str`, `HashMap::get_str`, …): `_str` means "the twin that takes a `str`".
- Methods whose first parameter is the `String` itself (`ref<String, shared> s`): the namespace follows the
  first parameter.

**`remove` keeps order, `swap_remove` does not** (D-0134 §2): holds for `Vec`, `HashMap`, `HashSet` and the
`_str` twins; nothing to change.

**Every error enum has `::text`**: `CertError`, `CompressError`, `FileError`, `HttpError`, `NetError`,
`ParseError`, `PgError`, `TlsError` all do.

`stress/std-align/` and `private/std-proposal.md` hold no item left undone.

Breaking for a program that names one of the five variants or passes a `str` binding to one of the six
functions; nothing else changes. New case: `conf.std-error-wrapper-variants-unique`.
