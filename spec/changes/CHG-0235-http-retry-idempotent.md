# CHG-0235 — A kept connection's request sent again only when idempotent; `Json` keys as views; clearer diagnostics

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; round-9 real-world programs)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0195, D-0173
Affects: `spec/21` 4.51.1 (`[Http-Keep-Alive]` and the `std::http::client` listing), `spec/conformance.md` 3.198.2;
`impl/std/http/client.cb`, `impl/std/encoding/json.cb`, `impl/src/typecheck.rs`

## What changed

- **`[Http-Keep-Alive]`:** when a kept connection closes before any of the response arrives, the request is sent
  again on a fresh connection only if its method is idempotent (GET, HEAD, PUT, DELETE, OPTIONS, TRACE: RFC 9110
  §9.2.2). Another (POST, PATCH) is `ConnectionClosed`: the server may have read it and acted on it, and sending it
  again could do twice what it asks. Found by a key-value service whose handler failed on a request: the client
  sent the request again, and a second handler failed (`conf.http-keep-alive-post-not-resent`).
- **`Json::get`, `Json::set`** take the key as a `StringView`, as D-0134 made `std`'s text parameters: with a `str`
  only a literal key could be asked for or set, so an object keyed by file names could not be built with `set`.
  Every call with a literal still checks (a literal is a `StringView` argument).
- **Diagnostics (no rule changed):** `==` or `!=` on a resource other than `String` (a `Vec`, a `HashMap`) says
  that it compares plain values and what to compare instead; reading a resource whole (`Vec<u64> b = a;` after a
  move, or a field) names the place and says to move, lend or clone it. Both were `diag.read-of-resource` with no
  message.

## What did not change

A GET, HEAD, PUT, DELETE, OPTIONS or TRACE behaves as before; a client without `keep_alive` keeps no connection.
