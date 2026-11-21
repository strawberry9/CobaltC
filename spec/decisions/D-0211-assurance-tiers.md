# D-0211 — Assurance tiers, security profiles and a hostile-input round for `std`

Status: ACCEPTED (2026-10-11; the owner: "continue actioning the outstanding items in the Action plan" — WI-8)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §20
Depends on: D-0151, D-0160, D-0161, D-0173, D-0174, D-0180, D-0183, D-0186, D-0206
Affects: `spec/21` 4.54.0, `spec/conformance.md` 3.205.0, `spec/02` 1.0.73 (CHG-0249); `impl/std/x509.cb` (the PEM
readers), `impl/std/http/client.cb` (the status line); four cases; `htmlguide` §21; `stress/hostile_oct11/` (local: the runner and its logs)

## Problem

The review (F-08; `ChatGPT_review/ACTION-PLAN-2026-10-11.md` WI-8) held that a normative `std` of this breadth —
cryptography, TLS, HTTP, compression, a database client — enlarges the surface a conformance claim covers,
and asked that the claims be tiered and that each security-relevant module say what it targets, what it is
tested with, what it asserts about timing and what it bounds. The owner keeps the scope (the flagship is a
DNS-over-TLS client, and no third-party C library is linked), so the answer is to bound the claims, not the
library.

## Decision

1. **Four assurance tiers** (`spec/21` §0, non-normative): core-language safety (`term.safe-program`); the
   trusted library base (D-0206's table); functional API conformance (the rows, including the vectors); and
   the security profiles. A green suite shows the third tier and nothing more.
2. **A security profile per module** (`spec/21` §0, a table): standards, the rows holding the vectors, the
   constant-time claim, the input limits, and what was tested outside the suite. The constant-time column
   says only what the bodies do: the private operations of `std::crypto::pk` are constant time (masked
   ladders, windows and selects; the RSA result checked before it leaves), the AEADs and digests are, HMAC,
   HKDF and PBKDF2 are; Argon2id's second half is data-dependent by its definition; **verification, `BigUint`
   and RSA key generation are not**, and the table says so and says not to generate a key where an attacker
   can time it. The limits: a chain of at most ten certificates, a TLS record of at most 2^14 + 256 bytes, an
   HTTP head of at most 64 KiB and a body of at most `max_body`, a decompressed output of at most `max`, JSON
   nesting of at most 512, a PostgreSQL message of at most 1 GiB and a result of at most `max_result`. Each
   limit has a case: the two that had none get `conf.x509-chain-too-long` (a PKI of ten intermediates made
   with `openssl`) and `conf.http-head-too-large` (a 70,000-byte header both ways).
3. **A hostile-input round** (`stress/hostile_oct11/hostile.cb`, `run.sh`): every parser fed ten thousand
   mutated inputs — byte flips, bytes zeroed or saturated, truncations, insertions, deletions, length fields
   set to zero, the limit, past it and the maximum, duplicated regions, nesting past the limit — from seeded
   corpora, each input in its own thread so that a fault is a counted `ThreadFailure`, under GCC, Clang and
   Clang with AddressSanitizer and UBSan (the binary under `setarch -R`, the VM's ASLR note). The network
   parsers run against fake peers on loopback: a raw client against `HttpServer`, a fake origin under
   `http_get`, a fake PostgreSQL server that completes SCRAM correctly (so the mutations land on a valid
   conversation, one phase at a time), and a byte-mutating proxy between `TlsStream::client` and our own
   `TlsStream::server`.
4. **The guide** (§21) carries the tiers and a short form of the table.

## Results

The round (`stress/hostile_oct11/run.sh`, seed 20261011, ten thousand inputs per parser, each run under GCC,
Clang and Clang with AddressSanitizer and UBSan, detect_leaks off): every run ended with the same tally under
all three compilers, no fault, no hang, no sanitizer report.

| Parser | `Ok` | `Err` | skipped | fault |
|---|---|---|---|---|
| `inflate` / `zlib_inflate` / `gunzip` | 1,344 | 8,656 | 0 | 0 |
| `Json::parse` | 1,662 | 8,232 | 106 | 0 |
| DER (`Certificate::parse`, the key readers) | 2,232 | 7,768 | 0 | 0 |
| PEM (`certificates_from_pem`, `pem_block`) | 730 | 6,768 | 2,502 | 0 |
| HTTP request (`HttpServer`, a raw client) | 2,290 | 7,710 | 0 | 0 |
| HTTP response (`http_get`, a fake origin) | 2,617 | 7,383 | 0 | 0 (97 before the fix) |
| PostgreSQL framing and SCRAM (a fake server) | 1,394 | 8,606 | 0 | 0 |
| TLS records and handshake (a mutating proxy) | 2,546 / 2,547 | 7,454 / 7,453 | 0 | 0 |

"Skipped" counts a mutated input that is not UTF-8: `Json::parse` and the PEM readers take a `StringView`, so
such bytes never reach them (`StringView::from_utf8` refuses them first). The TLS tallies differ by one between
compilers because a mutated handshake can also time out, which the client reports as an error either way. The
PostgreSQL round is slow (SCRAM's PBKDF2 on every connection: thirty-six minutes for ten thousand under GCC)
and the instrumented binary overran the runner's hour, so it was run again under AddressSanitizer at two
thousand inputs with the same seed: ok 290, err 1,710, fault 0, no report.

The two defects, both found in the first pass and fixed with this edition: the PEM readers
(`certificates_from_pem`, `pem_block`) pushed each byte of a block with `String::push_ascii` and faulted
`diag.not-ascii` on a byte of 128 or more — a non-ASCII byte in a block is now `Err(BadEncoding)` from
`pem_block` and skips the block in `certificates_from_pem` (`conf.hostile-pem-not-ascii`); and the HTTP
client cut a status line at fixed byte offsets (`sub(line, 9, 12)` for the status code) and faulted
`diag.not-char-boundary` when a multi-byte character straddled one — the first thirteen bytes must now be
ASCII or the line is `Err(BadMessage)` (`conf.hostile-http-status-line`). `inflate`, `Json::parse`, the DER
readers, `HttpServer`, the PostgreSQL client and the TLS stack found nothing. Both fixes are in `impl/std`,
which both tools embed at build time.
