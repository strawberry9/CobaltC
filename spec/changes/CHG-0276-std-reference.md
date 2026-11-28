# CHG-0276 — The std reference, and what writing it found

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14; under the forward plan of 2026-10-12, WI-18)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0229 (`///` documentation), D-0230 (`unittest`, documented tests as examples)
Affects: every file of `impl/std`; `impl/tools/cobdoc.py`, `impl/battery.sh`; `impl/src/typecheck.rs`,
`impl/src/main.rs`, `impl/cobc/src/main.rs`, `impl/cobc/src/lower/program.rs`; `spec/21` (4.60.1),
`spec/conformance.md` (3.222.0), `spec/02-schema.md` (1.0.82); three cases in `impl/conformance`; the guide
(`htmlguide/generator/std_reference.py`, `std_inventory.py`, `gen.py`, `content_c.py`)

## Why

D-0229 made `///` before an item its documentation, and none of `std`'s 670 exported items had any. The
guide's §21 listed every item by name only, so a reader had to go to `spec/21` or the source to learn what
an item does.

## What changed

- **Documentation.** Every exported item of `impl/std` has a `///` block. Each block has a summary
  sentence, then what the signature does not say: what each case returns, the error variants, the faults,
  the cost where it is not the obvious one, and the item to use instead where two are easily confused. The
  words follow `spec/21` and the bodies. Only lines were added, besides the fixes below.
- **Examples.** 122 items have an example: a documented `unittest` block right after the item (D-0230).
  `coby --unittest-std` and `cobc --unittest-std` run them, and both tools give the same report under GCC
  and Clang. cobc's help now lists `--unittest-std`, and coby's usage line does too.
- **The reference chapter.** `htmlguide/generator/std_reference.py` reads `cobdoc.py`'s output when the guide
  is built. Its section R lists every module, each type with its associated functions, then the free
  functions and constants: the declaration, the documentation as Markdown, and the examples. `gen.py
  --check` runs each example as `fn main()` of a program that imports `std` and the item's module, under
  coby and under cobc with GCC and with Clang. §21's inventory links each exported name with a CobaltC
  body to its entry. The items both tools build in (`printf`, `sqrt`, …) have no CobaltC body and are
  still listed only in §21.
- **`cobdoc.py --check`** exits 1 when an exported item has no summary, or when a documented `unittest`
  follows no exported item. It runs in battery stage 0. Stage 1 runs `std`'s tests under coby and under
  cobc with each compiler.

## What writing it found

Reading each body against `spec/21`, item by item, found these. Each is fixed here, and each fix has a case
that fails without it.

- **`==` on an aggregate holding a `StringView`** (`Option<StringView>`, a struct with a view field). It was
  accepted. coby then compared the views' fields, so `Some(StringView::of("x")) == Some(StringView::of("x"))`
  was false, and cobc stopped with an internal error. `[Eq-Struct]` compares field by field, and a view's
  field is a slice, which has no equality (D-0047). The static pass now rejects the comparison
  (`diag.type-mismatch`); its message says to compare the payloads. Case: `conf.view-in-aggregate-eq`.
- **The PostgreSQL client accepted AuthenticationOk without the server's SCRAM signature.**
  `[Pg-Auth]` verifies the server's final message before AuthenticationOk counts. The client accepted an
  AuthenticationOk at any point, and `Scram::verify` accepted `v=` before `Scram::respond` had computed the
  signature to expect, since an empty signature equalled an empty one. A server that did not know the
  password could therefore authenticate. Now AuthenticationOk after a SCRAM start needs the verified
  signature, and `Scram::verify` is false before `respond`. Case: `conf.postgres-server-signature-required`.
- **`HttpConnection::request` kept the connection after two kinds of error.** A request line over the head
  limit, or not UTF-8, and a failed `100 Continue` write returned `Err` without closing.
  `[Http-Request]` closes the connection after an `Err`, so the next call read on into the stream. Case:
  `conf.http-error-closes-connection`.
- **`Args::finish` treated a prefix of a declared option as that option.** It decided "declared" by
  substring, so an undeclared `--ver` beside a declared `--verbose` was "missing a value for --ver"
  rather than "unknown option --ver". It now compares the spec's words, and `[Args-Finish]` says so. A
  `std` test covers it.
- **`PrivateKey::from_pem` took the first label in its list, not the first block in the text.** `[Key-Pem]`
  says the first PRIVATE KEY, RSA PRIVATE KEY or EC PRIVATE KEY block. An EC block before a PKCS #8 block
  gave the later one. `spec/21`'s transcription of `pem_block` is brought up to the source; it had also
  missed the existing refusal of a byte above 127.
- **`hkdf_expand_label` and `hkdf_expand_label_with` with a length over 65535** faulted
  `narrowing_overflow` while building the info, rather than `hkdf_length`, which `[Hkdf]` gives a length over
  255 blocks. They now fault `hkdf_length` first. A label over 249 bytes or a context over 255 still faults
  `narrowing_overflow`, and the documentation says so.
- **cobc could not compile a `std` function that calls `spawn`.** The object cache compiles a pure-`std`
  function in a translation unit of its own, but a spawn's argument block is defined among the thunks of the
  program's unit. No `std` function spawned until the `try_join` example did. A function that spawns is now
  compiled with the program.
- Stale developer comments were corrected in `collections.cb` (`Vec::truncate`, `HashMap::clear`,
  `slice_parts`), `fs.cb` (`File::read`), `http/client.cb` (`HttpClient::send`), `text.cb` (a note on
  `String::clone` that sat above `String::reserve`) and `tls.cb` (the module is client and server).

## Left for the owner

- `HttpClient::send` follows a redirect to another origin with all of the request's own headers, among
  them any `Authorization` or `Cookie` the program set. `spec/21` does not say. Common practice (browsers,
  curl) drops credentials on a cross-origin redirect.
- `base64_decode` accepts padding whose unused bits are not zero (`"QR=="`). RFC 4648 §3.5 allows a decoder
  to refuse it.
- `Args::options` can take as an option's value an item that a flag read earlier already took
  (`-o -v`). `[Args-Option]` ("followed by any item") allows it, but the result depends on the order of
  the calls.
- `Channel::recv_timeout_ms` multiplies `ms` by 1,000,000 with wrapping, so a very large timeout (above
  about 9.2·10¹²) wraps.
- `PgConnection::query` reports more than 65,535 parameters as `ResultTooLarge`, as `spec/21` says, but the
  name fits the case poorly.
- The TLS alert-40 text says "this client offers" on a server's stream too.
- cobc cannot infer `spawn(Option::unwrap, x)`'s type arguments, which coby infers. `Option::unwrap<i32>`
  works in both.

## What changed for programs

`==` and `!=` on an aggregate that holds a `StringView` or a slice are now rejected statically. A
PostgreSQL server that skips the SCRAM signature is refused (`AuthFailed`). An HTTP server's connection
closes after every `Err` from `HttpConnection::request`. Otherwise `std` behaves as before. Its items are now
documented, and its examples run.
