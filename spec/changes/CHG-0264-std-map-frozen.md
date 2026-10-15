# CHG-0264 — The `std` module map is frozen; five wrapping variants renamed; six text parameters are views

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-13; D-0223)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0223, D-0134, D-0180
Affects: `spec/21` §0 and its listings (4.57.0), `spec/conformance.md` (3.216.0); `impl/std` (`std.cb`, `tls.cb`,
`http.cb`, `http/client.cb`, `http/server.cb`, `crypto/kdf.cb`, `database/postgres.cb`); one new case

## What changed

- `spec/21` §0: "The map is frozen" paragraph; `impl/std/std.cb`'s comment names its lists as the map.
- Variants: `TlsError::TlsNetFailure`, `HttpError::HttpNetFailure`, `HttpError::HttpTlsFailure`,
  `PgError::PgNetFailure`, `PgError::PgTlsFailure` (were `NetFailure`, `NetFailure`, `TlsFailure`,
  `NetFailure`, `TlsFailure`).
- Parameters now `StringView`: `Header::new(StringView name, …)`, `Request::new(StringView method, …)`,
  `http_post(…, StringView content_type, …)`, `hkdf_expand_label(…, StringView label, …)`,
  `hkdf_expand_label_with(…, StringView label, …)`, `PgResult::column(…, StringView name)`.
- `spec/conformance.md`: `conf.std-error-wrapper-variants-unique`; `conf.tls-closed`'s text names
  `TlsNetFailure(Closed)`.

## What changed for programs

Breaking, before the freeze (`STABILITY.md`: `std` is *moving*): a program naming one of the five variants
uses the new name (a bare one now resolves, where it was `diag.ambiguous-name`); a `str` binding passed to one
of the six parameters is written `StringView::of(t)`. Literal arguments are unaffected.
