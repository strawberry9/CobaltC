# CHG-0266 — Continuous fuzzing and interop checks for the security-relevant modules; their assurance label

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-13; the 2026-10-12 plan's WI-7 and Q4, accepted by the owner)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0211 (the hostile-input round), D-0223
Affects: `spec/21` §0 (4.58.0); the guide's sections on cryptography, certificates, TLS, HTTP and databases;
`impl/fuzz/` (new: `fuzz.cb`, `run.py`, `clang-asan`, `interop/`, `README.md`); `impl/battery.sh` (stage 5)

## Why

D-0211's hostile-input round fed every parser in `std` ten thousand mutated inputs once, from `stress/`
(local). A parser changed afterwards was not fed again, and nothing outside the suite checked the TLS and
HTTP implementations against independent ones as a matter of course. Until an independent audit, a reader of
the guide also deserves to be told how far these modules have been checked (Q4).

## What changed

- `impl/fuzz/fuzz.cb`: the round's program, kept in the repository, with a round for the text parsers
  (`base64_decode`, `from_hex`, `BigUint::parse`, the number parsers, `DateTime::from_iso`, `Url::parse`,
  `IpAddr::parse`, `SocketAddr::parse`, `PgUrl::parse`).
- `impl/fuzz/run.py`: every parser to a time budget under GCC, Clang, Clang with AddressSanitizer and UBSan,
  or `coby`, new seeds each run, pairs in parallel; faults, other exits, sanitizer reports and timeouts fail.
- `impl/fuzz/interop/run.sh`: a fresh PKI from openssl; `std::tls`'s client against `openssl s_server` (each
  suite and group, a HelloRetryRequest, RSA, P-256, P-384 and Ed25519 leaves, chains of depth one to three, a
  wrong host name and an expired certificate refused) and `openssl s_client` against its server; curl
  against `std::http`'s server and the client against Python's servers. Loopback only.
- Battery stage 5 runs both (the fuzzer ten minutes per parser under the sanitizer build).
- `spec/21` §0: "Continuous fuzzing and interop" and "Not independently audited"; the guide's five sections
  carry an "Assurance" note saying the same.

## Results

First full campaign (2026-10-13): every parser for ten minutes under each of GCC, Clang and Clang with
AddressSanitizer and UBSan, 27 runs, 15,265,700 inputs; no fault, no sanitizer report, no timeout. The network rounds
are slow per input (the PostgreSQL round 1,800 to 3,000 inputs in ten minutes, TLS about 5,000), the others
from 290,000 to 2 million. Interop: 35 checks, all passed.

## What changed for programs

Nothing.
