# D-0173 — An HTTP/1.1 client in `std::http`

Status: ACCEPTED (2026-10-05, the owner: "accept D-0173 and D-0174")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0144 (`std::net`), D-0161 (`std::tls`), D-0172 (its suites), D-0141 (`TrustStore::system` reads files)
Affects: `spec/21` §0, §2p (4.38.0), `spec/conformance.md` (3.164.0), the guide §21, `impl/std/http.cb`, `impl/std/std.cb`, `src/prelude.rs`; `CHG-0201`

## Problem

A deployable program almost always fetches a URL or posts to one: a
webhook, an API, a download, a health check. `std` had TCP and TLS, so
every such program framed its own request, parsed its own status line
and headers, chose between `Content-Length` and chunked bodies, and
followed its own redirects, as round 7's HTTPS tests did by hand. That is
code that is wrong in small ways everywhere, and the admission test of
Master Instructions §9 is met twice over: `http_get` and `http_post` name
two of the most common intents a program has, and the hand-written form
is not clearer. It also gives the flagship (D-0151's road) a second
transport: DNS over HTTPS (RFC 8484) is a POST of the same message.

## Candidate mechanisms

1. **An HTTP/1.1 client over `std::net` and `std::tls`: `Url`,
   `Header`, `Request`, `Response`, `HttpClient` with timeouts, a redirect
   limit and a body limit, `http_get` and `http_post`.** Selected.
2. **A guide chapter showing the sixty lines over `TlsStream`.** Every
   program keeps its own chunked decoder and redirect loop. Rejected.
3. **HTTP/2 as well.** Framing, HPACK and multiplexing for no gain to a
   program that makes one request at a time; every server speaks 1.1.
   Not adopted; a later decision if a peer requires it.
4. **Compression (gzip, deflate), cookies, proxies, multipart.** Each a
   decision of its own; none is needed to speak to a server that is not
   asked for them. Deferred.

## Selected design

**`spec/21` §2p `rule.stdlib.http`:**

- `Url::parse` (`scheme://host[:port]/path?query`, the fragment dropped,
  the scheme and host lower-cased, the scheme's default port, `/` for an
  empty path; `BadUrl` for no `://`, no host, user information or a bad
  port), `Url::target`, `Url::text`, `percent_encode`, `percent_decode`.
- `Header`, `Header::new`, `find_header` (names compared without regard to
  case), `Request` (`method`, `url`, `headers`, `body`), `Request::new`,
  `Request::header`, `Response` (`status`, `reason`, `headers`, `body`),
  `Response::header`, `reason_phrase`.
- `HttpClient` (`roots`, `timeout_ms`, `max_redirects`, `max_body`,
  `headers`), `HttpClient::new` (30 s, 10 redirects, 16 MiB, a
  `User-Agent`), `HttpClient::send`: one connection per request, `Host`,
  the client's then the request's headers, `Content-Length` for a body,
  `Connection: close`; interim 1xx responses skipped; the body by HEAD or
  204/304 (none), chunked (decoded, trailers dropped), `Content-Length`,
  or to the close; 301, 302 and 303 followed by GET without the body,
  307 and 308 with the method and body, `Location` absolute, origin-
  relative or path-relative; the system roots loaded into the client at
  its first https request. `http_get(url)` and `http_post(url,
  content_type, body)` with a fresh client.
- `HttpError`: `NetFailure`, `TlsFailure`, `BadUrl`, `UnsupportedScheme`,
  `NoRoots`, `BadMessage` (a malformed status line or header, both
  `Content-Length` and `Transfer-Encoding`, a non-chunked
  `Transfer-Encoding`), `TooLarge` (a head over 64 KiB or a body over
  `max_body`), `TooManyRedirects`, `ConnectionClosed`.

## Compatibility impact

Additive: a new module `std::http`, re-exported by `std`.

## Revisit conditions

- Compression, cookies, proxies, multipart, HTTP/2.
- Connection reuse across requests, if a program makes many to one host.
