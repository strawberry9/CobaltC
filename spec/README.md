# CobaltC Specification — Reading Guide

This directory is the normative definition of CobaltC (Master
Instructions §13). Every artifact is at version 1.0.0 or later (each
carries its own Version and Change Log; `changes/` records every
normative change since 1.0.0); every entity `ACCEPTED`. This file is a
map, not a normative artifact.

## Edition

**CobaltC specification edition 2026.101009.**

The specification as a whole is identified by its edition,
`YYYY.MMDDnn`: the date it was published (UTC) and a two-digit counter
for that day, starting at 01. Editions before 2026-09-30 were numbered
`YYYY.nnn`, a counter within the year; both forms sort in publication
order, as text and as numbers. A new edition is published whenever an update of the
repository changes anything in this directory; an update that changes
only the implementations, the guide or the examples keeps the edition.
Each artifact still carries its own `Version` and Change Log, and each
normative change its `CHG` record; the edition names the set of them
published together. A conformance claim names an edition
(`NAMING-POLICY.md`: "conforms to CobaltC 2026.100306").

| Edition | Published with | Contents |
|---|---|---|
| 2026.001 | Commit Update 22, 2026-09-27 | The first numbered edition: every artifact as of D-0065 and `CHG-0075` |
| 2026.002 | Commit Update 24, 2026-09-27 | Local constants; fields and elements of constants are constants (D-0066, `CHG-0076`) |
| 2026.003 | Commit Update 26, 2026-09-27 | `diag.static-assert-not-constant`; `const auto` and values in a `static_assert` message; function-local types; a slice borrows its range (D-0067–D-0070, `CHG-0077`–`CHG-0080`) |
| 2026.004 | Commit Update 27, 2026-09-27 | `Vec::sort`'s source compares keys in place (`spec/21` 3.29.0; no rule changed) |
| 2026.005 | Commit Update 28, 2026-09-27 | A declaration by two names (`int x = 10;` is `diag.unbound-name`; `T x = a;`); syntax errors are `diag.syntax-error` (`CHG-0081`, `CHG-0082`) |
| 2026.006 | Commit Update 29, 2026-09-27 | Static refutation through references, handles and literal-bound locals; the stress rounds' frictions and fixes (`[e; N]`, constant array lengths, `diag.stack-exhausted`, text order); adjacent string literals; literal arguments typed by the call; `overwrite`, closure result types, `unwrap`/`expect`, `Vec::clone`, `fault` names (D-0071–D-0083, `CHG-0083`–`CHG-0099`) |
| 2026.007 | Commit Update 30, 2026-09-28 | Temporaries as arguments; `Vec::from_slice`/`truncate`; shifts bind tighter than `&` `^` `\|`; a use of a maybe-moved binding is rejected statically; an `Option<ref>` result borrows its first reference argument; `fn` values own their closures; the built-in bounds `eq`, `ordered`, `number`, `integer`; `min`, `max`, `abs`, `pow`, `sqrt`, `floor`, `ceil`, `round`, `trunc`, `HashMap::clear` (D-0084–D-0091, `CHG-0100`–`CHG-0106`) |
| 2026.008 | Commit Update 31, 2026-09-28 | `read_line` — on standard input and on a `File` — consumes one `\r` immediately before the `\n` it consumes (D-0092, `CHG-0107`) |
| 2026.009 | Commit Update 32, 2026-09-29 | A generic item as a value is instantiated by explicit type arguments or the expected fn type, and a binding with neither is rejected (`[T-Item-Generic]`, `[T-Item-Value-Uninferable]`); conformance rows for a spawned thread's bare-reference result, now compiled by `cobc` (D-0093, D-0094, `CHG-0108`, `CHG-0109`) |
| 2026.010 | Commit Update 33, 2026-09-29 | Overwriting a live part of an object — a field, a literal-index array element, a place reached through a valid reference — is refuted statically; what follows a false trusted claim: the rules describe the execution up to that step and constrain nothing after it (D-0095, D-0096, `CHG-0110`, `CHG-0111`) |
| 2026.011 | Commit Update 34, 2026-09-29 | `foreach` over an integer range: `foreach (x in lo..hi)` and `foreach (i, x in lo..hi)` count from `lo` up to, not including, `hi`; two plain-number bounds count in `usize` (D-0097, `CHG-0112`) |
| 2026.093001 | Commit Update 35, 2026-09-30 | The round-6 stress findings: the implementations enforce typing, visibility and constant rules the text already stated, a borrowing closure cannot escape its captures, destruction at a scope end is `solitary`-checked, spec/17's array-length text follows D-0074 (`CHG-0113`–`CHG-0116`); and the round-6 decisions: trailing commas, `_ = e;` for a discarded `Result`, printf `*` widths and `%v` enum names, maths and ASCII functions, byte literals typed by context, parts of temporaries as arguments, destructuring with `..` and renaming, qualified constant lengths, enum codes, pending argument borrows, comparisons on `String`/`str`/`bool`, patterns through references, struct and enum keys, the first static error in source order, and a reference binding ending at its last use (D-0098–D-0111, `CHG-0117`–`CHG-0131`) |
| 2026.093002 | Commit Update 36, 2026-09-30 | Method-call syntax recorded as a deliberate absence, the owner's final decision (`CHG-0132`, `spec/22` §5); §5's restrictions and absences brought up to date with D-0053, D-0090, D-0103 and D-0104 (`spec/22` 2.38.2; no rule changed) |
| 2026.093003 | Commit Update 37, 2026-09-30 | The findings on the examples' idioms and the extras: `Vec` helpers, views straight from a `String` and map lookups by `str`, `Option::ok_or`, `if (pattern = e)` and `while (pattern = e)`, the `clone` bound, the `std` errors as text, `bitstruct`, integers as bytes and CRC-32, a reproducible `Rng`, `read_volatile`/`write_volatile`, `checked_narrow`, bit counting and rotation, `sleep_ms`, `Weak<T>`, array lengths as constant expressions, and text literal patterns (D-0112–D-0127, `CHG-0133`–`CHG-0150`) |
| 2026.100101 | Commit Update 38, 2026-10-01 | The characters of a text: `String::chars` and `StringView::chars`, one view per character, and why no allocation-free `foreach` form exists (D-0128, `CHG-0151`) |
| 2026.100102 | Commit Update 39, 2026-10-01 | Room in advance: `Vec::reserve` and `String::reserve` (D-0129, `CHG-0152`); `?` on an `Option` in a function returning an `Option` (D-0130, `CHG-0153`); a local is declared once in its block — parameters and loop and pattern names count as declared in the block they govern — `diag.duplicate-local` (D-0131, `CHG-0154`); `spec/22` §1 notes the formatter `cobfmt` |
| 2026.100201 | Commit Update 42, 2026-10-02 | A `bitstruct` may be declared in a function, as a local type with its derived `bits` and `from_bits` (D-0132, `CHG-0155`) |
| 2026.100202 | Commit Update 43, 2026-10-02 | A local holding references and nothing to destroy (a map lookup's `Option<ref<…>>`, a struct of references) ends after its last use, as a reference does (D-0133, `CHG-0156`) |
| 2026.100203 | Commit Update 44, 2026-10-02 | The reading list notes that `std`'s bodies are normative for what they do, not how, and that `cobc` realizes the most-used ones natively (`21` §0); no rule changed |
| 2026.100204 | Commit Update 46, 2026-10-02 | D-0093's informative implementation note records that `cobc` now compiles a generic item passed as an argument whose type arguments the rest of the call fixes (`twice(id, 10)`); no rule changed |
| 2026.100301 | Commit Update 47, 2026-10-03 | `std` aligned with one design (D-0134, `CHG-0157`–`CHG-0161`): text a function only reads is a `StringView` (paths, needles, `File::write_text`), `StringView::of` and `[Str-Literal-View]`; `Rng::new`, `Result::map_err`, `String::parse`, `HashSet::key_at`, `stdout_write`/`stdin_read`; `remove` keeps order and `swap_remove` does not; `ReadError` folded into `FileError`; `HashSet::clear`; `spec/21` 4.0.0 with its tables grouped by type. Breaking (source) |
| 2026.100302 | Commit Update 48, 2026-10-03 | A `String` a call returns may be viewed as a call's argument (D-0135, `CHG-0162`): `[View-Form-Temporary-Argument]` extends D-0103 to `&f()[lo .. hi]`, so a computed path needs no binding; `spec/21` 4.1.0, `spec/09` 1.5.0. Additive |
| 2026.100303 | Commit Update 52, 2026-10-03 | `std` in submodules, still reached by `import std;` (D-0136): re-export, `export import p;` (`CHG-0163`, `spec/17` 2.14.0, `spec/22` 2.45.0); `std` divided into `std::core`, `collections`, `text`, `io`, `sys`, `memory`, `sync`, `random` and `math`, each re-exported by its root, and the floating-point functions moved from the intrinsics to `std::math` (`CHG-0164`, `spec/21` 4.2.0, `spec/06` 1.16.0). Breaking (source) only for a floating-point call without `import std;` |
| 2026.100304 | Commit Update 55, 2026-10-03 | Two collections in `std::collections` (D-0137, `CHG-0165`, `spec/21` 4.3.0 §2i, `spec/conformance.md` 3.127.0): `Queue<T>`, taken from and added to at either end in amortized constant time (`rule.stdlib.queue`), and `PriorityQueue<T>`, least first by the key types' order (`new`) or the caller's (`new_by`), equal elements first in, first out (`rule.stdlib.priority-queue`, `[Priority-Queue-Not-Key]`). Additive |
| 2026.100305 | Commit Update 57, 2026-10-03 | The set operations: `HashSet::union`, `HashSet::intersection` and `HashSet::difference`, each a new set of copies of the keys in the first set's order, `union` adding the second's new keys after it (D-0138, `CHG-0166`, `spec/21` 4.4.0 `[Set-Ops]`, `spec/conformance.md` 3.128.0). Additive |
| 2026.100306 | Commit Update 58, 2026-10-03 | Three text helpers (D-0139, `CHG-0167`, `spec/21` 4.5.0 §2h, `spec/conformance.md` 3.129.0): `contains` (`[Contains]`, whether `find` is `Some`), `replace` (`[Replace]`, a new `String` with every occurrence replaced, left to right without overlap; an empty `from` occurs nowhere) and `join` (`[Join]`, the inverse of `split`, for a slice of views or of `String`s), each on `StringView` and on `String`. |
| 2026.100401 | Commit Update 59, 2026-10-04 | A deployable `std` (D-0140–D-0145, `CHG-0168`–`CHG-0173`, `spec/21` 4.11.0, `spec/conformance.md` 3.135.0, `spec/registry/diagnostics.md` 1.44.0): UTC dates and times in a new `std::time` (`DateTime`, `Weekday`, `unix_ms`, `diag.invalid-datetime`); paths and file metadata in `std::sys` (`path_join` … `path_canonical`, `file_info`, `copy_file`, `make_dir_all`, `remove_dir_all`, `set_current_dir`, `temp_dir`, `home_dir`); the operating system's secure randomness in `std::random` (`os_random_bytes`, `os_random_u64`, `Rng::from_os`, `diag.entropy-unavailable`); child processes and interrupt requests in a new `std::process` (`Command`, `Child`, `watch_interrupts`, `interrupt_requested`; failures are `FileError`s); TCP, UDP and name resolution in a new `std::net`; `read_all` and `read_all_bytes`; no two `std` enums share a variant name. Additive |
| 2026.100402 | Commit Update 60, 2026-10-04 | A foreign function is declared `unsafe extern fn`, as Rust 2024 requires `unsafe extern`: the declaration's signature is an unchecked claim, now marked where it is made (`[Extern-Decl]`); plain `extern fn`, and `unsafe` before `extern "…";`, are syntax errors (D-0146, `CHG-0174`, `spec/22` 2.46.0, `spec/20` 1.12.0, `spec/conformance.md` 3.136.0). Breaking (source) |
| 2026.100403 | Commit Update 61, 2026-10-04 | Local time (`local_offset_seconds`, `DateTime::to_local`, D-0147); `min`/`max`/`abs`/`pow` in `std::math` (D-0148); text literals typed `String` by a declared position, a match arm and a generic parameter (D-0149, D-0155, D-0162); `exit` (D-0150); `std::crypto` (SHA-256/384/512, HMAC, HKDF, ChaCha20-Poly1305, X25519, RSA and ECDSA verification), `BigUint`, `std::x509` and a TLS 1.3 client in `std::tls` (D-0151–D-0161); `StringView::as_bytes` and `String == str` (D-0163, D-0164); a write to a finished child is `Err(Io)`, never SIGPIPE (`spec/21` 4.11.1). `CHG-0175`–`CHG-0192`, `spec/21` 4.29.0, `spec/conformance.md` 3.155.0, `spec/registry/diagnostics.md` 1.46.0, `spec/15` 1.12.0, `spec/12` 1.21.0, `spec/06` 1.17.0. Additive |
| 2026.100404 | Commit Update 62, 2026-10-04 | AES-GCM (`aes_encrypt_block`, `aes_gcm_seal`, `aes_gcm_open`, 16-, 24- and 32-byte keys) and P-256 key agreement (`p256_private_key`, `p256_public_key`, `p256_ecdh`) in `std::crypto`; the TLS client offers TLS_AES_128_GCM_SHA256 and P-256 and answers a HelloRetryRequest, RFC 8446's mandatory set (D-0165, `CHG-0193`, `spec/21` 4.30.0, `spec/conformance.md` 3.156.0, `spec/registry/diagnostics.md` 1.47.0); `TlsError::text` names the common alerts (`spec/21` 4.29.1). Additive |
| 2026.100501 | Commit Update 64, 2026-10-05 | `std::crypto` completed: HMAC and HKDF over any `HashKind`, Ed25519, EC keys with P-384 key agreement and RFC 6979 ECDSA signing, RSA private keys, signing and key generation, BLAKE2b, PBKDF2, Argon2id and password hashing (D-0166–D-0170, `CHG-0194`–`CHG-0198`); keys as SubjectPublicKeyInfo and PKCS #8 in DER and PEM, `sign`, Ed25519 certificates (D-0171, `CHG-0199`); the TLS client offers TLS_AES_256_GCM_SHA384, P-384 and Ed25519 (D-0172, `CHG-0200`); `std::http`, an HTTP/1.1 client and server (D-0173, D-0174, `CHG-0201`, `CHG-0202`); a handle destroyed by a fault's unwind does not wait (`spec/18` 1.5.3, `spec/19` 1.8.1) (`spec/21` 4.39.0, `spec/conformance.md` 3.166.0, `spec/registry/diagnostics.md` 1.48.0). Additive |
| 2026.100502 | Commit Update 66, 2026-10-05 | A `while (true)` that no `break` leaves has the type `never`, so a function may end with it (D-0175, `CHG-0203`, `spec/12` 1.22.0 `[T-While-Forever]`, `spec/14` 1.22.0, `spec/conformance.md` 3.167.0); unreachable values after such loops removed from `std`'s listings. Additive |
| 2026.100503 | Commit Update 67, 2026-10-05 | `std::sys` divided: files and paths (with `File`, the whole-file functions and `FileError` from `std::io`) in `std::fs`, the arguments and environment in `std::env`, the clocks in `std::time`; `std::io` the standard streams; no catch-all submodule (D-0176, `CHG-0204`, `spec/21` 4.40.0, `spec/conformance.md` 3.168.0; Master Instructions §9). No item changed; additive for programs importing `std` |
| 2026.100504 | Commit Update 69, 2026-10-05 | `read_le`/`read_be` moved from `std::text` to `std::collections`, beside `Vec::push_le`/`push_be`; `crc32` to `std::crypto`, marked as not cryptographic (D-0177, `CHG-0205`, `spec/21` 4.41.0, `spec/conformance.md` 3.169.0). No item changed; additive for programs importing `std` |
| 2026.100601 | Commit Update 71, 2026-10-06 | Standard output is buffered: the bytes written reach the operating system no later than the next write to standard error, read of standard input, child sharing standard output (`Command::status`), the program's end, a `flush_stdout()` call, and, on a terminal, the end of each line; `flush_stdout` added to `std::io` (D-0178, `CHG-0206`, `spec/21` 4.42.0 `[Print-Buffer]`, `spec/conformance.md` 3.170.0). Additive |
| 2026.100602 | Commit Update 72, 2026-10-06 | Two conformance cases where the implementations had diverged from the rules: an argument or struct literal field is read when it is evaluated, before a later one writes it (`conf.eval-order-reads`, `[LValue-To-RValue]`), and an element reached by `v[0].x = e` is stale when `e` grows `v` (`conf.elem-write-rhs-grows`, `[Write-Stale]`) (`spec/conformance.md` 3.171.0). No rule changed |
| 2026.100603 | Commit Update 73, 2026-10-06 | A path held by a value an argument computes (a view) is not pending; only a borrow the argument forms is (D-0179, `CHG-0207`, `spec/15` 1.12.1); three cases where the implementations had diverged from the rules: an index borrow checked at the whole element (`conf.index-borrow-whole-element`, `[Index-Vec]`), the first fault reported when a destructor faults during the unwind (`conf.fault-unwind-first-fault`, `[Fault-Unwind]`), and `conf.view-arg-not-pending` (`spec/conformance.md` 3.172.0). Clarification |
| 2026.100701 | Commit Update 78, 2026-10-07 | `std::database`: a PostgreSQL client over `std::net` and `std::tls`, SCRAM-SHA-256 from `std::crypto`, bound parameters through the extended query protocol, rows in the server's text form, scripts, `PgError` with the server's SQLSTATE (D-0180, `CHG-0208`, `spec/21` 4.43.0 `rule.stdlib.postgres`, `spec/conformance.md` 3.173.0). Additive |
| 2026.100702 | Commit Update 79, 2026-10-07 | The `std` completeness round: `std::json` (a value tree read and written), `std::compress` (DEFLATE, zlib, gzip, Adler-32; the HTTP client decodes gzip and deflate bodies), `Args` for command-line options, a TLS 1.3 server and `HttpServer::bind_tls`, `Channel::try_recv`/`recv_timeout_ms`, `cpu_count`, `TcpStream::try_clone`, and the small holes of thirteen modules (prefixes, lines, words, code points, `parse<bool>`, `Vec::retain`/`dedup`, floats in `sort`, `clamp`/`gcd`/`is_nan`/`is_finite`, `asin`/`acos`/`atan`, `shuffle`, `uuid_v4`, `File::create_new`/`flush`/`position`, `current_exe`, `hostname`, `process_id`, `Child::terminate`, milliseconds and HTTP dates, keepalive, connected UDP, SHA-1, `TlsStream::read_line`/`peer_certificate`, forms, typed PostgreSQL cells) (D-0181 to D-0186, `CHG-0209` to `CHG-0214`, `spec/21` 4.44.0, `spec/conformance.md` 3.174.0). Additive but for `HashKind::Sha1`, a fraction `from_iso` reads, and `HttpServer::accept` giving `HttpError` |
| 2026.100703 | Commit Update 80, 2026-10-07 | A conformance case for a temporary made in an expression match arm that borrows the arm's binding: it ends with the arm's expression, before the binding is destroyed (`[Match]`'s `stmt-scoped`), where the interpreter had kept it to the enclosing statement's end (`conf.match-arm-temporary-borrowing-binder`, `spec/conformance.md` 3.175.0). No rule changed |
| 2026.100704 | Commit Update 81, 2026-10-07 | `std` performance round, no rule changed: SHA-1/SHA-2/Blake2b/Argon2/Poly1305/AES-GCM, `deflate`/`inflate`, hex/base64, JSON, HTTP and PostgreSQL helpers rewritten so their loops need no checks at run time (`std` listings in `spec/21`); `PgRow::get_bytes` reads an empty `bytea` (`\x`) as an empty vector, as `[Pg-Cell]` says; three conformance cases for shapes the compiler's native bodies must keep (`spec/conformance.md` 3.176.0); `conf.datetime-iso` follows D-0181 (a fraction of the seconds is read) |
| 2026.100801 | Commit Update 83, 2026-10-08 | Nested `std` submodules (D-0187, `CHG-0215`): `std::crypto` → `digest`/`kdf`/`aead`/`pk`, `std::http` → `client`/`server`, `std::database` → `postgres`, each re-exported, every item keeping its name and key; `std::extensions`, the implementation-defined area `std` declares and does not re-export (`spec/00` `term.implementation-dependent-program`). Also, no rule changed: the static pass checks `std`'s items as a program reaches them (an empty program 0.85 s → 0.20 s), the staged battery (`impl/battery.sh`), `slow:` case headers, and the trimmed child-process and PostgreSQL cases |
| 2026.100802 | Commit Update 84, 2026-10-08 | No rule changed. `spec/08` §4 (1.2.0): a path's standing changes only in its own thread, and an access through a slice also meets the objects of the elements it covers (D-0188, `CHG-0216`; `spec/19` 1.9.0 points to it); `cobc` runs a function's reference arguments unchecked after one test at entry (D-0188 dual bodies) and keeps more `Vec`s unchecked (D-0189, `CHG-0217`: more confining calls, struct fields through a function's one reference, search keys, predicate literals). Conformance cases for each, and for a read through a slice meeting a held element, which `cobc` had missed (`spec/conformance.md` 3.178.0–3.182.0); `std::crypto`'s AES bitsliced and GHASH by integer multiplication (`spec/21` 4.45.1, non-normative) |
| 2026.100803 | Commit Update 85, 2026-10-08 | No rule changed. `cobc` keeps three more `Vec` shapes unchecked (D-0190, `CHG-0218`): a struct inside a `Vec` given to a looping function, `foreach` over a `Vec` parameter, and `retain`/`position` with a capturing closure literal; conformance cases for each (`spec/conformance.md` 3.183.0) |
| 2026.100804 | Commit Update 86, 2026-10-08 | No rule changed. `cobc` calls named functions and `fn` values holding them as `Vec` predicates with bare element addresses, and specialises `binary_search_by` (D-0191, `CHG-0219`); conformance cases for each (`spec/conformance.md` 3.184.0). Also, outside `spec/`: the guide's performance chapter and the benchmarks re-measured |
| 2026.100805 | Commit Update 87, 2026-10-08 | No rule changed. Performance close-out (D-0192, `CHG-0220`): `cobc` handles `Vec`s of `String`s and resources natively, passes search keys and capturing closure values without minting paths, runs `Vec::from_fn` literals inline, calls dual bodies directly where the call site proves the arguments disjoint; `cbrt` takes its lock shared for read-only checks and not at all once only the main thread runs; a native predicate loop's clash is reported at the call. Conformance cases for each (`spec/conformance.md` 3.185.0). Full battery green |
| 2026.100901 | Commit Update 88, 2026-10-09 | Round-8 findings and the owner's items after it. D-0193 (`CHG-0221`): closures capturing closures fixed in both tools, closure names unique across functions, native text and queue paths, `Vec::swap_remove`, `Response::text`, `char_count`. D-0194 (`CHG-0222`): quiet types, `=` replaces a value whose end only frees memory (`spec/05` `[Write-Quiet-Replace]`). D-0195 (`CHG-0223`): `ChildInput`/`ChildOutput`, `HttpClient::keep_alive`. D-0196 (`CHG-0224`): fewer runtime events for `String`s. `spec/21` 4.47.0, `spec/05` 1.5.0, `spec/conformance.md` 3.189.0. Full battery green |
| 2026.100902 | Commit Update 89, 2026-10-09 | `std::encoding`: data formats and encodings of bytes as text, one submodule each. D-0197 (`CHG-0225`): `std::json` is now `std::encoding::json`. D-0198 (`CHG-0226`): `to_hex`/`from_hex` move from `std::text` to `std::encoding::hex`, and `base64_encode`/`base64_decode` to `std::encoding::base64`. Item names and `import std;` programs are unchanged. `spec/21` 4.49.0, `spec/conformance.md` 3.191.0. Rapid, no battery |
| 2026.100903 | Commit Update 90, 2026-10-09 | Performance: far more proven at compile time, far less tracked at run time. D-0199 (`CHG-0227`): native text searches, fused substrings, `unwrap`, a cheaper runtime. D-0200 (`CHG-0228`): values with no runtime object, held element references, read-only closures, quiet locals, results passed on without an object, natives for text, files, channels and maps. No rule changed. `spec/conformance.md` 3.193.0. Full battery green |
| 2026.101001 | Commit Update 91, 2026-10-10 | Files and threads. D-0201 (`CHG-0229`): `File` writes are buffered, written out at fixed points (`[File-Buffer]`, `spec/21` 4.50.0). D-0202 (`CHG-0230`): the runtime keeps each running thread's records in a heap of its own, so threads run their checks in parallel; no rule changed. `spec/conformance.md` 3.195.0. Full battery green |
| 2026.101002 | Commit Update 92, 2026-10-10 | Concurrency. D-0203 (`CHG-0232`): `spec/19` §4 `rule.conc.guarantee` states what threads can rely on -- an interleaving of whole steps, no data races, nothing undefined -- and what an implementation may leave out; `[Spawn]` holds a reference argument from the spawn (`coby` now does). D-0202 phase 4 (`CHG-0231`): the runtime's remaining stop-every-thread steps made local, guard-free lock blocks in `cobc`, reused threads; no rule changed. `spec/conformance.md` 3.197.0. Battery owed |
| 2026.101003 | Commit Update 93, 2026-10-10 | Supervised tasks: a spawned thread's fault contained to it, `[Join-Failed]`, `cancel`, `[Lock-Poisoned]`, `[Deadlock]`, `std::sync` `try_join`/`is_finished`/`ThreadFailure`, `slice_parts` (D-0204, `CHG-0233`) |
| 2026.101004 | Commit Update 94, 2026-10-10 | Supervised tasks completed: a thread locking a mutex that may hold an exclusive reference (a channel of them) is lent; `[Deadlock]` without a cycle faults the ends of the chains of waits (D-0204, `CHG-0234`) |
| 2026.101005 | Commit Update 95, 2026-10-10 | A kept HTTP connection closed unanswered sends a request again only when its method is idempotent; `Json::get`/`Json::set` take the key as a `StringView` (`CHG-0235`) |
| 2026.101006 | Commit Update 96, 2026-10-10 | D-0205: `Result::ok`, `slice_eq`, `StringView::from_utf8`, `UdpSocket::try_clone`, `Child::kill_tree` (`CHG-0238`); frozen shared parameters in `cobc` and a soundness fix for vectors passed fresh (`CHG-0237`); runtime fixes (`CHG-0236`) |
| 2026.101007 | Commit Update 97, 2026-10-10 | No rule changed: `spec/21` 4.52.1 lays out one listed signature as `cobfmt` does |
| 2026.101008 | Commit Update 98, 2026-10-10 | Review repairs (D-0206, `CHG-0239`–`CHG-0244`): `term.safe-client`, `term.trusted-library-base` and the safe-program theorem — unconditional for a safe client on a conforming implementation, whose trusted library base (every `unsafe` block of `std`, listed in `spec/21` §0 with the fact that discharges it) makes only true claims; implementation-independent means the same rules under documented choices and declared variation, `term.portable-program`; outcome sets by listing or by condition, six judgment forms; the schema's Rule entry; the byte view over a range of cells, a reference's group one address image (`conf.ref-byte-image-width`, two `coby` fixes); the flow analysis defines the accepted set, `coby --check` its executable form. `spec/00` 1.3.0, `spec/01` 1.2.0, `spec/02` 1.0.68, `spec/06` 1.18.0, `spec/14` 1.23.0, `spec/20` 1.13.0, `spec/21` 4.53.0, `spec/conformance.md` 3.200.0. Clarification: no accepted program rejected, no outcome changed |
| 2026.101009 | Commit Update 99, 2026-10-10 | A machine check of the normative corpus (D-0207): `impl/tools/speccheck.py` checks entity links, tag vocabularies, label and diagnostic references, decision and change numbering, the edition, the rows' files and the trusted table, first in battery stage 1; its ten findings fixed (`CHG-0245`): two tags outside `spec/01` §5's vocabularies, four Depends-on targets that did not exist, three derivations citing renamed labels. `spec/19` 1.11.2, `spec/21` 4.53.1, `spec/conformance.md` 3.201.0, `spec/02` 1.0.69. Clarification: no rule or case changed |

**Licence.** Copyright © 2026 strawberry9. This specification — every
file in this directory — is licensed under the Creative Commons
Attribution-NoDerivatives 4.0 International License (CC BY-ND 4.0): it
may be copied and distributed unchanged, including commercially, with
attribution, but not modified
(https://creativecommons.org/licenses/by-nd/4.0/). Anyone may implement
the language it describes (`LICENSE-IMPLEMENTATION` at the repository's
root); the name is governed by `NAMING-POLICY.md` there.

## What is normative

Rules written in CFN (`01-metalanguage.md`) inside `03`–`21`, the
state definition in `04`, the grammar in `22`, the registries, and
the conformance suite. Prose explains; where prose and a rule differ,
the rule wins. Decision records (`decisions/`) are rationale, not
rules.

## Reading order for an implementer

1. `22-surface-syntax.md` — the grammar and the correspondence table
   from each production to its rule.
2. `04-abstract-state.md` — `Σ`, every component, every predicate.
3. `12-type-system.md` — static typing; then `06` §7 for
   representation and the implementation-defined parameters you must
   choose and document.
4. `13` → `05` → `14` → `15` — evaluation: results, `store`, statement
   and block exit, calls and `main`.
5. `08`, `07`, `16`, `09`, `10`, `11` — aliasing, resources and
   destructors, aggregates, references, escape checking, `let`.
6. `14` §6 — `rule.control.flow-analysis`: the exact static/dynamic
   boundary; your implementation must reject exactly the refuted
   instances.
7. `17`, `18`, `19`, `20`, `21` — modules, faults, threads, raw
   pointers and FFI, and the standard library module `std`: its intrinsics and the library
   types written in CobaltC itself (`Vec`, `String`, `HashMap`, `Rc`, `Box` and the rest),
   whose bodies are normative for what they do, not how: `cobc` realizes the most-used
   ones natively (`21` §0).
8. `conformance.md` — every case with its derivation; `registry/
   diagnostics.md` — every diagnostic you must be able to report.

## What you must decide and document

Listed in `22` §5: `AddrWidth`, byte order, enum discriminant width,
struct padding, `dangling<T>()`, `fn` value encoding, mutex cells,
the reporting form of termination outcomes, calling convention/ABI.
`IMPLEMENTATION-NOTES.md` (non-normative) is the consolidated form of
this list, alongside every point of permitted nondeterminism and every
`unsafe` trust condition, each with its governing rule.

## What the language does not provide

`22` §5's "deliberate absences" and "restrictions". Do not add them
in an implementation; propose them through a `D-XXXX` decision.

## Provenance

`AUDIT.md` (first audit), `AUDIT-2.md` (second audit, rules-level),
`AUDIT-STATUS.md` (closure of both, and the 2026-09-20 consistency
pass recorded as `CHG-0009`, and the per-case derivation of the
conformance suite recorded as `CHG-0010`, and the end-to-end program
derivations recorded as `CHG-0011`). D-0015 was superseded by D-0018;
D-0018 and D-0019 are the decisions behind the 1.0.0 rewrite.
