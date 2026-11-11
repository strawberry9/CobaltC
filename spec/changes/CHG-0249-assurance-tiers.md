# CHG-0249 — Assurance tiers and security profiles; two parser faults; two limit cases

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-11; D-0211)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0206, D-0211
Affects: `spec/21` 4.54.0 ("Purpose and scope": "Assurance", "Security profiles", "The hostile-input round"),
`spec/conformance.md` 3.205.0, `spec/02` 1.0.73; `impl/std/x509.cb` (`certificates_from_pem`, `pem_block`), `impl/std/http/client.cb` (`parse_status_line`);
`impl/conformance/21-standard-library-semantics/x509_pem_not_ascii_ok.cb`, `x509_chain_too_long_ok.cb`,
`http_head_too_large_ok.cb`, `http_status_line_not_ascii_ok.cb`; `htmlguide` §21

## What changed

- `spec/21` §0: three non-normative paragraphs and a table (the tiers, the profiles, the round).
- `std::x509`: a PEM block holding a byte that is not ASCII is malformed input — `certificates_from_pem`
  skips it, `pem_block` (behind `PrivateKey::from_pem`, `CertKey::from_pem`) is `BadEncoding`. Before, both
  pushed the body through `String::push_ascii` and faulted `diag.not-ascii` (`conf.hostile-pem-not-ascii`).
- `std::http::client`: a status line whose first thirteen bytes hold one of 128 or more is `BadMessage`.
  Before, `parse_status_line` took bytes 9 to 12 as the code and the rest from byte 13, and a multi-byte
  character across one of those offsets faulted `diag.not-char-boundary` (`conf.hostile-http-status-line`).
- `spec/conformance.md`: four rows.

## What did not change

No rule or diagnostic. The readers' accepted inputs are the same; in each, one class of malformed input that
was a fault is now an error, which is what their contracts (`Vec` of the certificates found; `Result`) already
said.
