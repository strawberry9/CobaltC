# D-0232 — The std reference round's open questions, decided

Status: ACCEPTED (2026-10-14; the owner delegated these questions: "can you resolve all these yourself?")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §21
Depends on: D-0063 (channels), D-0151 (base64), D-0173 (`std::http`), D-0180 (PostgreSQL), D-0184 (`Args`),
D-0186 (TLS server), CHG-0276 (where the questions were raised)
Affects: `spec/21` (4.61.0), `spec/conformance.md` (3.223.0) (CHG-0277); `impl/std` (`http/client.cb`,
`encoding/base64.cb`, `env.cb`, `sync.cb`, `tls.cb`); `impl/src/typecheck.rs`; `impl/cobc/src/lower/intrinsics.rs`

## Problem

Documenting `std` (CHG-0276) left seven questions that the text did not settle. Testing the last one,
`spawn`'s inference, found that `spawn` did not type its arguments at all.

## Decisions

1. **Credentials on a redirect to another origin are dropped.** Once a redirect leaves the request's origin
   (scheme, host and port), `HttpClient::send` stops sending `Authorization`, `Proxy-Authorization` and
   `Cookie`, whether the client or the request set them. It does not send them again even if a later
   redirect returns to the first origin. Browsers and curl do the same, so a server cannot collect a
   program's credentials for a third party by redirecting. `[Http-Redirect]`.
   - *Alternative:* refuse a cross-origin redirect (`Err`). Declined: a program that wants that can set
     `max_redirects` to 0 and follow redirects itself.
2. **`base64_decode` is canonical.** The bits that padding leaves over must be zero (RFC 4648 §3.5).
   `"QR=="` is `Err(Invalid(1))`, so each byte string has exactly one text. That was already what "what
   `base64_encode` writes" meant; `[Base64]` now says it. Signatures, keys and certificates read through
   `base64_decode` therefore have a single encoding.
3. **An argument another declaration took is no option's value.** `Args::options` no longer takes as its
   value an item that a flag read earlier already took. The option then counts as given without its value,
   and `Args::finish` says so. One item can no longer count twice. `[Args-Option]`.
   - Still true: an option read before a flag takes the flag-like item as its value (`-o -v` with `-o`
     declared first gives `-o` the value `-v`). That is ordinary POSIX practice for an option's argument.
4. **A wait longer than the clock can measure waits as long as it can.** In
   `Channel::recv_timeout_ms(ch, ms)`, a `ms` beyond about 9.2·10¹² saturates rather than wrapping round to
   a deadline in the past.
5. **`ResultTooLarge` stays the error for more than 65,535 parameters.** Adding a variant to `PgError`
   would break every exhaustive `match` on it for a limit no real statement reaches. The variant's
   documentation and `spec/21` name both causes.
6. **TLS alert texts name "the peer".** `TlsError::text` cannot tell a client's stream from a server's, so
   every alert text now says "the peer" (`the peer refused the handshake (alert 40): …`). Before, the texts
   said "the server" and "this client".
7. **`spawn` types its arguments as a call does.** `[Spawn]` gives `spawn(e_f, e_1 … e_n)` the arguments of a
   call `e_f(e_1 … e_n)`, but the static pass checked none of them. `spawn(f, true)` with `f(i32)`, or
   `spawn(f, 1, 2)`, ran in both tools, and a value where `f` takes a reference faulted at run time.
   - Now `spawn` passes the argument count and each argument through the call's own checks, after
     inferring a generic function's type arguments from the arguments.
   - Ordinary calls had a gap of the same family: `Result::is_ok(Ok(1))`, with `E` fixed by nothing, was
     not checked, since the argument's type was not fully known, and it faulted when run. A value where
     the parameter is a reference is now rejected whatever the type parameters are: a literal, a struct or
     array literal, or a constructed variant is a value by its form.
   - `cobc` now also infers a spawned generic function's type arguments from a constructor argument
     (`spawn(Option::unwrap, Some(5))`), as coby did.

## What changed for programs

Programs that were wrong and ran anyway are now rejected statically: `spawn` with arguments of the wrong
type or count, and a value passed where a reference is taken. A cross-origin redirect no longer carries
credentials. `base64_decode` refuses non-canonical padding. The TLS alert texts are worded for either side.
Nothing else changes.
