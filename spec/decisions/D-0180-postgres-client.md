# D-0180 — A PostgreSQL client in `std::database`

Status: ACCEPTED (2026-10-07, the owner: "Accept D-0180 with all your recommendations and implement it")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test), §18
Depends on: D-0144 (`std::net`), D-0161 (`std::tls`, `TlsStream::client` over an open socket), D-0151 (`sha256`, `hmac_sha256`, `base64`), D-0170 (`pbkdf2`), D-0141 (`TrustStore::system`), D-0177 (`push_be`, `read_be`), D-0063 (threads, for the hermetic test)
Affects: `spec/21` §0, §2q (4.43.0), `spec/conformance.md` (3.173.0), the guide §21, `impl/std/database.cb`, `impl/std/std.cb`, `src/prelude.rs`; `CHG-0208`

Standing constraint (the owner, 2026-10-07): nothing in `std` links a
third-party C library. "A C compiler is all you need" holds. This rules
out libpq, libsqlite3 and libmysqlclient, so a database client in `std`
is a wire-protocol implementation in CobaltC over `std::net` and
`std::tls`, exactly as `std::http` is.

## Problem

A deployable program that keeps state keeps it in a database, and the
database it most often keeps it in is PostgreSQL. Without a client in
`std`, a CobaltC program cannot reach one at all: the protocol needs a
startup exchange, SCRAM-SHA-256 authentication, message framing, the
extended-query protocol for bound parameters, and the decoding of row
descriptions, data rows, command tags and error responses. None of that
is code a program should write for itself, and all of it is code that is
wrong in small ways everywhere (parameters spliced into SQL text being
the usual way). The admission test of Master Instructions §9 is met:
"run this statement with these parameters and give me the rows" names
one of the most common intents a deployable program has, and the
hand-written form is not clearer.

`std` already had every piece the protocol needs: TCP and TLS, the SCRAM
primitives (`sha256`, `hmac_sha256`, `pbkdf2`, `base64`), the big-endian
helpers for framing, and the system trust store for certificate
verification. The client is pure CobaltC and adds nothing to the runtime
of either implementation.

## Candidate mechanisms

1. **A PostgreSQL wire-protocol (v3) client in CobaltC: `PgConnection`
   over `TcpStream` or `TlsStream`, SCRAM-SHA-256, the extended-query
   protocol, text-format rows.** Selected.
2. **A generic `std::database` abstraction (`Connection`, `Row`,
   `Value`) with PostgreSQL as its first backend.** The abstraction
   would be shaped by one backend and guessed for the others. Written
   once a second backend exists and shows what is common. Deferred.
3. **MySQL/MariaDB.** A second wire protocol, with its own family of
   authentication plugins and more version drift. A decision of its own
   after this one. Deferred.
4. **SQLite.** Not a protocol: either a linked library, which the
   standing constraint forbids, or a native implementation of the file
   format, of which reading is feasible and writing (journal, locking,
   B-tree maintenance) is a large project. A linked library is declined;
   a native reader is a possible later decision.
5. **A guide chapter showing the protocol over `TlsStream`.** Every
   program keeps its own SCRAM exchange and row decoder. Rejected.

## Selected design

**Module.** `std::database`, a flat submodule re-exported by `std` like
every other (sub-decision 1). Its items carry the `Pg` prefix, so a later
client can share the module without a clash. Its bare enum variants are
named so that none repeats a variant of another `std` enum (`std`'s
resolver refuses an ambiguous bare variant): `BadConnectionUrl`,
`ProtocolViolation`, `ResultTooLarge`, `Disconnected`, `NoTrustedRoots`.

**`spec/21` §2q `rule.stdlib.postgres`:**

- `[Pg-Url]` `PgUrl::parse` takes
  `postgres://user[:password]@host[:port]/database[?sslmode=MODE]`
  (`postgresql://` the same). User, password and database are
  percent-decoded; the port defaults to 5432, the database to the user;
  `MODE` is `disable` or `verify-full` (the default). `BadConnectionUrl`
  for anything else, an unknown query key included (a misspelt `sslmode`
  must not silently connect).
- `[Pg-Connect]` `PgConnection::connect(url)` and
  `PgConnection::connect_with(url, roots, timeout_ms)`: TCP to the host
  (an IP literal as it is, a name resolved, each address tried in turn),
  then, for verify-full, the `SSLRequest` message; the server's `S`
  upgrades the socket through `TlsStream::client` with the host as the
  server name and the roots given (`connect` loads the system roots, and
  is `NoTrustedRoots` when that fails); an `N` is `TlsRefused`. Then
  `StartupMessage` (protocol 3.0, `user`, `database`,
  `client_encoding=UTF8`), the authentication exchange, `ParameterStatus`
  messages kept (`PgConnection::parameter(&c, "server_version")`),
  `BackendKeyData` kept, `ReadyForQuery`. `connect` uses 30 s for
  connecting and for each read and write. `close` sends `Terminate`; a
  dropped connection is just closed.
- `[Pg-Auth]` `AuthenticationSASL` with `SCRAM-SHA-256` (RFC 7677 over
  RFC 5802): the client-first message with the `n,,` header (no channel
  binding) and an empty user (as libpq sends; the server ignores it),
  the server's nonce, salt and iteration count, the salted password by
  `pbkdf2` over SHA-256 of the password as given (no SASLprep), the
  client proof and the server signature verified; a wrong server
  signature or a server refusal of SQLSTATE class 28 during the exchange
  is `AuthFailed`. `AuthenticationCleartextPassword` is answered only on
  a TLS connection (`AuthRefused` otherwise). `MD5` and every other
  method: `UnsupportedAuth`. The exchange's pieces are exported as
  `Scram` (`start(user, nonce)`, `client_first`, `respond`, `verify`) and
  `scram_salted_password`, so the conformance suite can check them
  against RFC 7677's vectors and a program can speak SCRAM to another
  server.
- `[Pg-Query]` `PgConnection::query(&mut c, sql, params)`: one statement
  with `$1`, `$2`, ... through the extended-query protocol (`Parse` with
  the unnamed statement and no parameter types, `Bind` with every
  parameter in text format, `Describe`, `Execute` with no row limit,
  `Sync`), so a parameter is never part of the SQL text. `PgValue`:
  `Null`, `Text(String)`, `Int(i64)`, `Float(f64)`, `Bool(bool)`,
  `Bytes(Vec<u8>)`; each is sent as the text the server reads for it
  (`Bytes` as `bytea` hex, a non-finite `Float` as `Infinity`,
  `-Infinity` or `NaN`, `Bool` as `t`/`f`). The server infers the
  parameter types from the statement.
- `[Pg-Result]` `PgResult`: `columns` (`name`, `type_oid`), `rows`
  (`PgRow`, `cells: Vec<Option<String>>`, a `NULL` as `None`, every other
  value as the server's text form), `affected` (from the command tag; 0
  when the tag carries no count), `tag`. `PgRow::get(&r, i)` and
  `PgResult::column(&res, name)`. Typed reading is `StringView::parse<T>`
  on the cell (sub-decision 4). Rows over `max_result` (16 MiB,
  `c.max_result`) are dropped and the statement is `ResultTooLarge` once
  the server is ready again, so the connection stays usable.
- `[Pg-Execute]` `PgConnection::execute`: `query` with the rows
  discarded, giving the affected count. Transactions are
  `execute("BEGIN")`, `execute("COMMIT")`, `execute("ROLLBACK")`; no
  helper names them (§9: shorter, not clearer).
- `[Pg-Script]` `PgConnection::run_script(&mut c, sql)`: the simple-query
  protocol for several statements separated by `;` with no parameters,
  every result discarded, the first error returned (sub-decision 5).
- `[Pg-Error]` `PgError`: `NetFailure(NetError)`, `TlsFailure(TlsError)`,
  `TlsRefused`, `NoTrustedRoots`, `BadConnectionUrl`, `UnsupportedAuth`,
  `AuthRefused`, `AuthFailed`, `Server(PgServerError)`,
  `ProtocolViolation` (a message the protocol does not allow here, or one
  that does not parse), `ResultTooLarge`, `Disconnected` (the server
  ended the connection, or a call after a failure or `close`).
  `PgServerError`: `severity`, `code` (the five-character SQLSTATE),
  `message`, `detail`, `hint`, `position`. After a `Server` error the
  client reads to `ReadyForQuery`, so the connection stays usable; a
  `NetFailure`, `TlsFailure`, `ProtocolViolation` or `Disconnected` leaves
  it unusable and every later call returns `Disconnected`.
  `PgError::text` gives a `String` (not a `str` as the other errors'
  texts do), because the text a program wants is the server's message.
- `[Pg-Notice]` `NoticeResponse` messages are kept as `PgServerError`
  values in `c.notices`, cleared by the program; a program that ignores
  them loses nothing. `NotificationResponse` (`LISTEN`/`NOTIFY`) is
  dropped in this decision.

**Not in this decision (each a later one):** binary-format rows and
parameters, typed cell accessors, named prepared statements, cursors and
partial fetching, `COPY`, `LISTEN`/`NOTIFY`, query cancellation
(`CancelRequest` with the backend key, which the connection keeps), SCRAM
channel binding (`SCRAM-SHA-256-PLUS`, needs the end-entity certificate's
hash from `std::tls`), an unverified TLS mode (libpq's `require`;
`std::tls` verifies every connection, and a self-signed server is reached
by `connect_with` and a store holding its issuer), pipelining, connection
pools, a generic abstraction over backends, MySQL, a native SQLite reader.

## Sub-decisions (the owner accepted every recommendation)

1. **Where it lives:** `std::database`, a flat submodule holding the
   `Pg*` items, with room for a later `My*` family. (Not `std::postgres`;
   not a nested `std::database::postgres`, which `std` has no precedent
   for.)
2. **Default `sslmode`:** `verify-full`. Loopback development writes
   `?sslmode=disable` out. `prefer` and `require` are not offered.
3. **MD5 authentication:** `UnsupportedAuth`. PostgreSQL has deprecated
   it, every supported server speaks SCRAM, and the `md5` primitive would
   otherwise enter `std::crypto` for this alone.
4. **Typed cells:** text only; `StringView::parse<T>` on the cell. The row
   is exactly what the server said.
5. **`run_script`:** included; schema setup and migrations are a file of
   statements, and `query` cannot run them.
6. **`notices`:** kept on the connection.

## Testing

The battery is hermetic and assumes no server:

- `conf.scram-sha256-vectors` (`postgres_scram_vectors_ok.cb`): RFC
  7677 §3's exchange (its nonces and salt) with the iteration count 4
  instead of 4096, since the interpreter takes minutes over PBKDF2 at
  4096 (the values confirmed with Python's hashlib); the RFC's own
  4096-iteration values are checked by `stress/postgres/scram_rfc7677.cb`
  under `cobc`.
- `conf.postgres-url`, `conf.postgres-values` (`postgres_url_ok.cb`).
- `conf.postgres-loopback` (`postgres_loopback_ok.cb`): a mock backend
  in CobaltC on a loopback port in one thread, the client in the main
  thread: the SSLRequest answered N, a wrong password, a cleartext
  request, SASL without SCRAM-SHA-256, MD5; then a session exercising
  every rule, and a session the server breaks.

Outside the battery, `stress/postgres/smoke.cb` was run on 2026-10-07
against a private PostgreSQL 14.24 cluster (`stress/postgres/cluster.sh`:
loopback only, scram-sha-256, TLS with a test CA), compiled by `cobc`
with GCC and with Clang: `sslmode=disable`; `verify-full` through the
SSLRequest upgrade with the test CA in a `TrustStore`; `verify-full` with
the system roots refused ("certificate signed by an unknown authority");
a wrong password refused. Bound parameters of every kind, NULL, bytea,
non-ASCII text, a server error with its position, a rolled-back
transaction, `serial`, `count(*)` all as expected. The TLS upgrade is
covered only by this smoke test until `std::tls` has a server side.

## Compatibility impact

Additive: a new module `std::database`, re-exported by `std`. No new
tokens, no grammar change. Programs importing `std` gain the `Pg*`
names, `Scram` and `scram_salted_password`; no existing `std` name is
touched.

## Revisit conditions

- A second backend (MySQL) shows what a generic layer should hold.
- A program needs rows larger than memory (cursors), `COPY` for bulk
  loading, or `LISTEN`/`NOTIFY`.
- `std::tls` gains a server side: the loopback case then covers the TLS
  upgrade, and channel binding becomes a small addition.
- The flagship or a showcase needs query cancellation.
