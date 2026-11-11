# CHG-0265 — Every exported `std` item is named by a case; every case file has a row

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-13; under D-0223 and the 2026-10-12 plan's WI-6)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0223, D-0207 (speccheck), D-0134
Affects: `spec/conformance.md` (3.217.0); `impl/tools/std_coverage.py` (new), `impl/battery.sh` (stage 1);
cases under `impl/conformance/`

## Why

With the module map frozen (D-0223), each exported item of `std` is a promise; a promise no case exercises can
break unnoticed. And 316 case files ran in the file suites without a row in `spec/conformance.md`, so the
table did not say what the suite holds.

## What changed

- `impl/tools/std_coverage.py`: lists every exported function, associated function, struct, enum and variant
  of `impl/std`, and the cases (case files and inline row fragments) that name each; a destructor counts as
  named with its type. An item no case names fails the check unless a `// coverage: exempt <reason>` line
  stands above it. It also fails on a case file no row names that is not a fixture (first line
  `// CONFORMANCE-FIXTURE`). Battery stage 1 runs it after speccheck.
- `spec/conformance.md`: 313 rows for the case files that had none (307 existing files and the six new cases below), under their sections ("Rows added for case
  files that had none"), generated from each file's header (its own `conf.` id where the header gives one, its
  expectation, its facility and first sentence); one case header's stale label corrected
  (`[Limits-Not-Integer]` → `[Limits-Not-Number]`).
- New cases for the 95 items no case named: `conf.std-error-variants-text` (every variant of `NetError`,
  `TlsError`, `HttpError`, `PgError`, `CertError`, `Utf8Error::text`, the `Weekday`s), `conf.std-small-items`,
  `conf.std-key-generators` (slow), `conf.net-addrs-and-http-peer`, `conf.tls-stream-bytes` (slow),
  `conf.postgres-connect-refused`; `conf.key-formats` (`x509_keys_ok.cb`) also reads every `PrivateKey` and
  `CertKey` variant, `CertKey::to_pem`, `der_read` and `RsaPkcs1`; `postgres_loopback_ok.cb` names `PgColumn`.

Result: 774 exported items, 773 named and one exempt (`hash_password`: Argon2id at 19 MiB is far too slow under `coby`; `argon2id` and `verify_password` are covered); 0 case files without a row or a fixture mark.

## What changed for programs

Nothing.
