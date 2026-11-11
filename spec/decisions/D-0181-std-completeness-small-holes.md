# D-0181 — The small holes of the `std` completeness round

Status: ACCEPTED (2026-10-07, the owner: "implement C1, C2, C3, C4, C5 and C6, and all per-module holes", on the completeness assessment of the same day)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0134 (naming), D-0136 (submodules), D-0176 (each item with the items it works with), D-0101 (ASCII, the transcendental functions), D-0062 (`key_less`), D-0113 (`String::` twins), D-0128 (`chars`), D-0140 (`DateTime`), D-0141 (paths), D-0143 (processes), D-0144 (sockets), D-0151 (hashes), D-0166 (`HashKind`), D-0161 (`TlsStream`), D-0173 (HTTP), D-0180 (`PgRow`)
Affects: `spec/21` §0, §1, §1b, §2d, §2e, §2e′, §2h, §2j, §2k, §2l, §2m, §2o, §2p, §2q, §3d (4.44.0), `spec/conformance.md` (3.174.0), the guide §21, `impl/std/` (`text.cb`, `collections.cb`, `math.cb`, `random.cb`, `fs.cb`, `env.cb`, `process.cb`, `time.cb`, `net.cb`, `crypto.cb`, `tls.cb`, `http.cb`, `database.cb`), `src/typecheck.rs`, `src/interp.rs`, `src/numtext.rs`, `src/fileio.rs`, `src/procio.rs`, `src/netio.rs`, `cobc/src/lower.rs`, `cbrt`; `CHG-0209`

## Problem

The assessment of 2026-10-07 (`private/std-completeness-2026-10-07.md`)
read every export of `std` against what a deployed program meets and
found, beside five larger absences (D-0182 to D-0186), a list of
one-function holes inside modules that are otherwise complete: things
real programs wrote by hand in the stress rounds, each naming one very
common intent, so each passes the admission test of Master Instructions
§9 on its own. They are decided together because none changes a design;
each fills a gap in a decided one.

## Candidate mechanisms

1. **One function per hole, in the submodule its neighbours live in,
   under D-0134's names.** Selected.
2. **Leave them to programs** (each is a few lines). Rejected: the
   assessment found them written by hand in the showcases, the stress
   rounds and `std` itself (`x != x` in `database.cb`), which is the
   admission test's signal.
3. **A larger redesign of the modules concerned** (a `Path` type, a
   `Duration`, an `Iterator`). Rejected: each is a design the owner has
   declined or deferred, and none of the holes needs one.

## Selected design

Per module, each item with its rule in `spec/21`:

**`std::text`** (§2h `[Strip-Prefix]`, `[Lines]`, `[Split-Whitespace]`,
`[Code-Point]`; §2d `[Parse-Bool]`): `StringView::strip_prefix(v, t)`
and `strip_suffix` give `Some(the rest)` or `None`; `lines(v)` splits at
`\n` dropping a `\r` before it, with no empty line after a final `\n`;
`split_whitespace(v)` gives the runs between ASCII whitespace, none
empty; `code_point(v)` is the code point of the first character
(`None` for empty text); `String::push_code_point(s, cp)` appends the
UTF-8 of `cp`, and a surrogate or a value above U+10FFFF appends U+FFFD
(as decoders do, so the `String` stays valid; a `bool` or a `Result`
would make every caller handle a case that JSON and the like map to
U+FFFD anyway). Each has its `String::` twin (D-0113). `parse<bool>`
reads exactly `true` or `false` (`Invalid(k)` with the longest prefix
of either otherwise), realized natively like the numbers.

**`std::collections`** (§1 `[Retain]`, `[Dedup]`; §1b `[Sort-Float]`):
`Vec::retain(v, keep)` keeps the elements `keep` accepts, in order,
moving by `Vec::swap` and destroying the rest by `truncate` (so the
rejected elements are destroyed last first, after every `keep` call);
`Vec::dedup(v)` (`T: eq`) removes each element equal to the one before
it. `Vec::sort`, `binary_search` and `PriorityQueue::new` accept `f32`
and `f64`: a NaN sorts after every other value (and NaNs are equal to
one another for the order), `-0.0` and `0.0` are equal, so the order is
a strict weak order and the merge sort is stable over it. `StringView`
stays outside `sort` (its one field is a slice, which the key machinery
of both tools does not compare); `sort_by` with `StringView::less` is
the way, as before.

**`std::math`** (§0 `[Clamp]`, `[Is-Nan]`, `[Is-Finite]`, `[Gcd]`; §3d
`[Float-Transcendental]`): `clamp(x, lo, hi)` (`T: ordered`; `lo` above
`hi` is `diag.assert-failed`); `is_nan(x)` is `x != x` and
`is_finite(x)` is `(x - x) == (x - x)`, both over `number`, so an
integer is finite and not a NaN; `gcd(a, b)` (`T: integer`) by Euclid
over magnitudes, never negative, `gcd(0, 0)` 0; `asin`, `acos`, `atan`
join the transcendental functions (the platform's, within 1 ulp).

**`std::random`** (§0 `[Rng-Shuffle]`, `[Uuid-V4]`): `Rng::shuffle(r,
s)` permutes a slice uniformly by Fisher-Yates over `below`, so one seed
gives one order everywhere; `uuid_v4()` is a version-4 UUID (RFC 9562
§5.4) from the operating system's secure source, as lowercase text.

**`std::fs`** (§2e `[Create-New]`, `[Flush]`, `[Position]`):
`File::create_new(path)` opens for writing only when nothing is there
and is `Ok(None)` when something is (`make_dir`'s convention, D-0061:
no new `FileError` variant, which would break every exhaustive match);
`File::flush(f)` makes everything written durable (`fsync`), `Err(Io)`
on a file open for reading; `File::position(f)` is the byte the next
read or write starts at, the read-ahead subtracted.

**`std::env`** (§2e′ `[Current-Exe]`, `[Hostname]`): `current_exe()` is
the running executable's path as the system reports it; `hostname()`
the machine's host name (`gethostname`; the DNS host name on Windows).

**`std::process`** (§2k `[Process-Id]`, `[Child-Terminate]`,
`[Child-Streams]`): `process_id()`; `Child::terminate(ch)` sends SIGTERM
(as `kill` on Windows), nothing to a child that has ended;
`Child::read_error_line(ch)` as `read_output_line` over standard error,
which gains its own read-ahead.

**`std::time`** (§2j `[Iso-Ms]`, `[From-Iso]`, `[Http-Date]`):
`iso_from_unix_ms(ms)` writes `2026-10-07T12:34:56.789Z`;
`unix_ms_from_iso(text)` reads it back, a fraction of any length to the
millisecond; `from_iso` now accepts a fraction and drops it. `DateTime`
itself keeps whole seconds: a field would break every program that
writes one as a struct literal, and a `Duration` stays out (owner,
2026-10-04). `DateTime::to_http_date(t)` writes RFC 9110's IMF-fixdate
(`Sun, 06 Nov 1994 08:49:37 GMT`; a year outside 0 ..= 9999 is
`diag.invalid-datetime`); `from_http_date(text)` reads that form and the
two obsolete ones a recipient must accept (RFC 850's two-digit year: 70
to 99 the 1900s, 00 to 69 the 2000s; asctime's), the weekday not
checked against the date.

**`std::net`** (§2l `[Keepalive]`, `[Udp-Connect]`):
`TcpStream::set_keepalive(s, on)` sets `SO_KEEPALIVE` (the system's own
`setsockopt`: Rust's `std::net` does not expose it); `UdpSocket::connect(u,
to)` fixes the peer, after which `send(u, data)` and `recv(u, max)` work
and a datagram from anyone else is dropped. (`try_clone` is D-0185's.)

**`std::crypto`** (§2m `[Sha1]`): `Sha1` (`new`, `update`, `finish`),
`sha1(data)`, and `HashKind::Sha1`, so `Hasher`, `hmac`, `pbkdf2` and
the HKDF forms take it: RFC 6238's default HMAC is SHA-1, git's object
names are SHA-1, and legacy signatures use it. The spec says plainly
that it is legacy, not for new signatures. `HashKind` gaining a variant
is the one change here that can break a program: an exhaustive `match`
on it; none exists in the repository.

**`std::tls`** (§2o `[Tls-Read-Line]`, `[Tls-Peer-Certificate]`):
`TlsStream::read_line` as `TcpStream`'s; `TlsStream::peer_certificate(s)`
the leaf the handshake verified (`None` on a server's side, D-0186);
`peer_addr`, `local_addr`.

**`std::http`** (§2p `[Form]`): `Param` (`name`, `value`, compared
exactly), `Param::new`, `find_param`, `form_decode(text)` (`&`- or
`;`-separated `name=value` pairs, `+` a space, percent-decoded; a part
that does not decode is kept as given), `form_encode(params)`,
`Url::query_pairs(u)`. A `Header` was not reused: header names compare
without case, form names with it.

**`std::database`** (§2q `[Pg-Cell]`): `PgRow::get_i64`, `get_f64`
(`NaN`, `Infinity`, `-Infinity` as the server writes them), `get_bool`
(`t`/`f`), `get_bytes` (`bytea`'s hex form); `None` for NULL, a missing
column, or text of another form, the text still there through `get`.

## Compatibility impact

Additive, with two exceptions, both recorded: `HashKind` gains `Sha1`
(an exhaustive `match` on `HashKind` in a program gains an arm; none in
the repository), and `DateTime::from_iso` accepts a fraction it refused
before (a program that relied on the refusal: none). No new tokens, no
grammar change, no new diagnostic. A program importing `std` gains the
names above; none of them was free for a program's own item to shadow
(`[Own-Declaration-Wins]` still applies).

## Testing

Seventeen hermetic file cases, all under `coby`, `cobc` with GCC and
with Clang, byte-identical: `conf.text-strip-prefix-suffix`,
`conf.text-lines-words`, `conf.text-code-points`, `conf.parse-bool`,
`conf.vec-retain`, `conf.vec-dedup`, `conf.vec-sort-floats`,
`conf.math-clamp`, `conf.math-nan-finite`, `conf.math-gcd`,
`conf.float-inverse-trig`, `conf.rng-shuffle`, `conf.uuid-v4`,
`conf.time-iso-ms`, `conf.time-http-date`, `conf.file-create-new`,
`conf.file-flush-position`, `conf.env-current-exe`, `conf.env-hostname`,
`conf.process-id`, `conf.child-terminate`, `conf.child-read-error-line`,
`conf.tcp-keepalive`, `conf.udp-connect`, `conf.sha1` (FIPS 180-4's
vectors, a million `a`s), `conf.hmac-sha1` (RFC 2202, PBKDF2 of RFC
6070, RFC 6238's TOTP), `conf.tls-read-line`,
`conf.tls-peer-certificate`, `conf.http-forms`,
`conf.postgres-typed-cells`.

## Revisit conditions

- `StringView` in `Vec::sort`, if the key machinery learns slices.
- A `Duration` or sub-second `DateTime`, if the owner reopens them.
- Non-ASCII case mapping and classification: a Unicode tables decision.
