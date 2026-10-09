# D-0174 — An HTTP/1.1 server in `std::http`

Status: ACCEPTED (2026-10-05, the owner: "accept D-0173 and D-0174")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0173 (the messages, the reader, the limits), D-0144 (`TcpListener`), D-0063 (threads, which handle connections)
Affects: `spec/21` §0, §2p (4.39.0), `spec/conformance.md` (3.165.0), the guide §21, `impl/std/http.cb`; `CHG-0202`

## Problem

The other half of the same intent: a service answers a health check, a
webhook or a small API. Without a server in `std`, every program parses
request lines, headers and bodies itself and gets keep-alive, `HEAD`,
`Expect: 100-continue` and the limits wrong in its own way. It also makes
the client testable hermetically: a conformance case runs a server thread
and a client in one program.

## Candidate mechanisms

1. **`HttpServer` over `TcpListener`: `accept` gives an `HttpConnection`,
   `request` parses the next request and `respond` writes a response,
   keep-alive as the client negotiates it.** Selected.
2. **A handler-callback server that spawns threads itself.** Hides the
   threads and the errors; CobaltC's `spawn` and `join` already express it
   in a few lines, and the program then chooses its concurrency. Rejected.
3. **A TLS server as well.** Needs the server side of the handshake, which
   `std::tls` does not have; `sign` and `PrivateKey` (D-0171) now make it
   feasible as its own decision. Deferred.

## Selected design

**`spec/21` §2p:**

- `HttpServer` (`max_body`, `timeout_ms`), `HttpServer::bind` (16 MiB,
  30 s), `local_addr`, `accept`.
- `HttpConnection::request`: the request line (`METHOD target HTTP/1.0|1.1`),
  the headers, `Host` required by 1.1, keep-alive by the version and
  `Connection`, `Expect: 100-continue` answered before the body, the body
  by `Content-Length` or chunked (none otherwise); `Ok(None)` when the
  client has closed or the last response closed the connection;
  `BadMessage` or `TooLarge` otherwise, after which the connection closes.
- `HttpConnection::respond(status, headers, body)`: the status line with
  RFC 9110's reason phrase, the headers given, `Content-Length` (none for
  1xx, 204 and 304), `Connection` as negotiated, the body omitted after a
  `HEAD`; `peer_addr`, `close`.

## Compatibility impact

Additive.

## Revisit conditions

- A TLS server.
- Streaming bodies (request or response) larger than memory.
