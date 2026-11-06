# The std fuzzer and interop checks

Every parser in `std` should answer a hostile input with `Err` (or `Ok`, when
the input happens to be valid): never a fault, a crash, a hang or a
sanitizer report. This directory checks that, and checks the TLS and HTTP
implementations against independent ones (WI-7, CHG-0266).

| File | What it is |
|---|---|
| `fuzz.cb` | The fuzzer, in CobaltC: `fuzz PARSER COUNT [SEED]` mutates COUNT inputs from the parser's seed inputs and runs each in its own thread, so a fault is counted, not fatal. It prints a tally; its exit status is 1 when a fault was seen, and each faulting input is written beside it as `fault_PARSER_N.hex`. |
| `run.py` | Runs `fuzz.cb` to a time budget for each parser, under each build, in parallel. |
| `clang-asan` | Clang with AddressSanitizer and UBSan, for `cobc --cc`. |
| `interop/run.sh` | TLS against OpenSSL, HTTP against curl and Python, on loopback. |

## The parsers

| Parser | What it feeds |
|---|---|
| `inflate` | `inflate`, `zlib_inflate`, `gunzip` (output limits included) |
| `json` | `Json::parse` (nesting at and past the limit included) |
| `der` | `Certificate::parse`, `CertKey::from_der`, `PrivateKey::from_der`, `der_read` |
| `pem` | `certificates_from_pem`, `PrivateKey::from_pem`, `CertKey::from_pem` |
| `text` | `base64_decode`, `from_hex`, `BigUint::parse`, `StringView::parse` (`i64`, `u8`, `f64`, `f32`), `DateTime::from_iso`, `Url::parse`, `IpAddr::parse`, `SocketAddr::parse`, `PgUrl::parse` |
| `http-server` | the server's request parser, from a client sending mutated requests on loopback |
| `http-client` | the client's response parser, from a server sending mutated responses |
| `pg` | the PostgreSQL client's message framing and SCRAM, from a fake server |
| `tls` | TLS records and handshake messages, client and server, on loopback |

`std` has no DNS wire-format parser (the DNS-over-HTTPS showcase has its own),
so there is no `dns` round.

## Running it

    cargo build --release -p coby -p cobc -p cbrt          # in impl/
    python3 impl/fuzz/run.py                               # every parser, gcc, clang and asan, 600 s each
    python3 impl/fuzz/run.py --budget 60 --parsers json,text --builds asan
    python3 impl/fuzz/run.py --builds coby --budget 300    # the interpreter
    impl/fuzz/interop/run.sh                               # needs openssl, curl, python3

**The budget.** `--budget` is seconds per parser and build; each pair runs
`fuzz` again and again with a new seed until its budget is spent, and pairs
run `--jobs` at a time (default: every CPU). The battery's stage 5 runs every
parser for ten minutes under the sanitizer build (`FUZZ_BUDGET` changes it),
then the interop checks. A longer campaign is the same command with a larger
budget and all three builds; `--seed` repeats a run.

On Linux with clang 14 and kernel 6.8, a sanitized binary can fail at start
when ASLR is on; `run.py` runs the sanitizer build under `setarch -R`.

**Results.** In `--out` (default `stress/fuzz/runs/STAMP` when `stress/`
exists, else `~/.cobaltc-fuzz/runs/STAMP`): a directory per parser and build
with its log and any `fault_*.hex` inputs, and `SUMMARY`. A fault is a
defect in `std`: fix it, and add the input as a conformance case.

## Adding a seed input

Each round's seed inputs are the `bases` it builds at its start, in
`fuzz.cb` (`round_json`, `round_text`, …): valid inputs that reach the
parts of the parser worth mutating. To add one, push it there:

    Vec::push(&mut bases, bytes_of("{\"a\": [1, 2, {\"b\": null}]}"));

For `round_text`, push its kind beside it (the index in `run_text`'s match,
`0` for base64 through `11` for `f32`). For a binary input, build it in code
(as `round_der` builds DER from a certificate) rather than pasting bytes. A
new parser gets its own `run_NAME` (the call, returning 0 for `Ok`, 1 for
`Err`, 2 to skip), `round_NAME` (its bases and the loop), a line in `main`,
and its name in `run.py`'s `PARSERS`.

## Interop

`interop/run.sh` makes a fresh PKI with openssl (RSA, P-256, P-384 and
Ed25519 leaves, chains of depth one to three, an expired leaf) and checks:

- `std::tls`'s client against `openssl s_server`: each cipher suite and
  group, a HelloRetryRequest, each key type, each chain depth; a wrong host
  name and an expired certificate refused;
- `openssl s_client` against `std::tls`'s server: each suite, group and key
  type, a chain with an intermediate;
- curl against `std::http`'s server: GET, HEAD, keep-alive, a large body, a
  chunked upload, large headers, 404;
- `std::http`'s client against `python3 -m http.server` and a chunked
  answer.

Everything stays on loopback: no external server is contacted.
