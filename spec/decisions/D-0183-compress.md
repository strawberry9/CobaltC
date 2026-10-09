# D-0183 — `std::compress`: DEFLATE, zlib and gzip

Status: ACCEPTED (2026-10-07, the owner: "implement C1, C2, C3, C4, C5 and C6", on the completeness assessment of the same day)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §18
Depends on: D-0136 (submodules), D-0119/D-0177 (`crc32`, `read_le`, `push_le`), D-0173 (the HTTP client), D-0181 (`Vec::retain`)
Affects: `spec/21` §0, §2s, §2p (4.44.0), `spec/conformance.md` (3.174.0), the guide §21, `impl/std/compress.cb`, `impl/std/http.cb`, `impl/std/std.cb`, `src/prelude.rs`; `CHG-0211`

## Problem

Compressed bytes are everywhere a deployed program looks: HTTP bodies
(`Content-Encoding: gzip`), `.gz` logs and archives, zip, PNG, git
objects, and the zlib framing inside many file formats; `std` could make
or read none of them, and the HTTP client, which offered no encoding,
read every body uncompressed at the server's discretion. `crc32` and
base64 were already there; the one format under all of these is DEFLATE
(RFC 1951) with its two wrappers, zlib (RFC 1950) and gzip (RFC 1952).

## Candidate mechanisms

1. **DEFLATE, zlib and gzip in CobaltC: `deflate`/`inflate`,
   `zlib_deflate`/`zlib_inflate`, `gzip`/`gunzip`, `adler32`.** Selected.
2. **Link zlib.** Forbidden by the standing constraint (the owner,
   2026-10-07: nothing in `std` links a third-party C library).
3. **Decompression only.** Half of what a program needs: it must also
   write a `.gz` and answer a client that accepts gzip. Rejected.
4. **A choice of levels and strategies.** One fixed strategy serves:
   LZ77 over the 32 KiB window with hash chains (64 candidates),
   blocks of 32 768 symbols written as whichever block kind is smallest.
   A level parameter is a later decision if a program needs the
   trade-off.

## Selected design

`spec/21` §2s `rule.stdlib.compress`, in `std::compress`:

- `[Inflate]` `inflate(data, max)`: a raw DEFLATE stream, every block
  kind, at most `max` bytes out (`Oversize` beyond: a defence against a
  small stream that expands without bound); bytes after the last block
  are ignored. `[Deflate]` `deflate(data)`: a raw stream that every
  conforming inflater reads, and the same bytes in every implementation
  (the strategy is fixed by the listing). `CompressError`: `Corrupt(i)`
  (not DEFLATE, or not the wrapper asked for, or a checksum that does
  not match, first seen at byte `i`), `Truncated`, `Oversize`;
  `CompressError::text`.
- `[Zlib]` `zlib_deflate`/`zlib_inflate`: the two-byte header (DEFLATE,
  no preset dictionary; `Corrupt(0)` otherwise) and the Adler-32
  trailer, verified. `[Adler32]` `adler32(data)`, RFC 1950 §8.2.
- `[Gzip]` `gzip`/`gunzip`: a member with no name, time or extra
  fields written; every member read in turn, its optional fields
  skipped, its CRC-32 and length verified (`Corrupt` at the trailer).
- `[Content-Encoding]` (§2p): `HttpClient::new` sends
  `Accept-Encoding: gzip, deflate` unless the program sets its own; a
  response's `Content-Encoding: gzip` (or `x-gzip`) is gunzipped and
  `deflate` zlib-inflated (raw-inflated when the server sent raw
  DEFLATE, as some do), within `max_body` (`TooLarge` beyond), and the
  `Content-Encoding` and `Content-Length` headers are dropped so the
  response reads as a plain one; another encoding is left as it came. A
  body that does not decode is `BadMessage`.

Written in CobaltC; nothing native. The server side does not compress:
a program that wants to gzips its body and sets the header.

## Compatibility impact

Additive: a new module, re-exported by `std`; the HTTP client sends one
more header by default and decodes two encodings it did not ask for
before. New names: the seven functions and `CompressError` with its
three variants (unique across `std`). No new tokens.

## Testing

`conf.deflate-inflate`, `conf.zlib`, `conf.gzip`, `conf.adler32`,
`conf.inflate-errors` (`compress_ok.cb`): streams CPython's zlib made
(raw, zlib, a gzip member with a name) inflated; text, empty data, three
bytes, random bytes (stored blocks) and long runs round-tripped, with
the compressed bytes fixed; Adler-32's vectors; truncated, corrupt and
oversize streams. `conf.http-content-encoding` (`https_loopback_ok.cb`):
gzip and deflate bodies decoded, an unknown encoding left alone.
Outside the suite, `stress/std2/comp1.cb`: 17 000 and 200 000 bytes
round-tripped, and our output read back by CPython's `zlib` and `gzip`.

## Revisit conditions

- A level or a faster matcher, if compression time matters to a program.
- zip, PNG or git object readers, which this makes possible.
