# CobaltC Standard Library Semantics

Status: normative artifact
Version: 4.53.1
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.stdlib.*`; Kind: Type, `type.str`)
Governed by: `CobaltC_Master_Instructions.md` §1, §23

## Purpose and scope

Master Instructions §1 scopes the design to the language. This
artifact fixes the **prelude**: the intrinsics every conforming
implementation provides (operations whose semantics are rules in
`spec/06`, `spec/07`, `spec/20`), the prelude types (`Option`,
`Result`), the text value type `str` (§2a), and the library types —
`Vec<T>`, `String`, `HashMap<K, V>`, `HashSet<K>`, `Queue<T>`, `PriorityQueue<T>`, `Rc<T>`, `Box<T>` — written in CobaltC
itself as evidence that the language as specified is sufficient. Their bodies are normative for
their observable behavior; an implementation may realize them
differently provided every conformance case in `spec/conformance.md`
holds.

**What `std` asserts** (D-0206, `term.trusted-library-base`). Some of
those bodies hold `unsafe { }` blocks: a `Vec` writes its elements
through a raw pointer, a `File` calls the operating system. Those blocks
are the **trusted library base**: a program that writes no `unsafe`
block of its own (`term.safe-client`) still reaches their
`discharge: trusted` side-conditions when it calls `Vec::push`, and the
language's guarantee (`term.safe-program`) holds for it because keeping
every one of those claims true is the implementation's conformance
obligation (`term.conformance`), not the program's. The table lists
every block in the normative bodies (`impl/std`), the trusted rule(s) it
instantiates, and the fact that discharges their side-conditions there
— the fact a native realization of the same item must keep. It is
generated from the `// trusted:` comments in the bodies by
`impl/tools/trusted_table.py`, which also refuses a block that has
none; `spec/AUDIT-STATUS.md`'s seventh pass is its 2026-09-20
predecessor.

<!-- trusted-table:begin -->
| Module | Item | Block(s) | Rule(s) | Why the side-conditions hold |
|---|---|---|---|---|
| `std` | `byte_op` | `std.cb` 136 | `[Extern-Call]` | `[Extern-Call]` path_op, proc_op and net_op read the a and b ranges and write only [out, out + cap), ranges their callers keep live and own. |
| `std` | `fs_answer` | `std.cb` 238 | `[Extern-Call]` | `[Extern-Call]` fs_query reads [a, a + an) and writes [out.ptr, out.ptr + cap), out's reserved buffer. |
| `std::collections` | `Vec::push` | `collections.cb` 29 | `[Rawptr-Write]`, `[Rawptr-Move-In]` | `[Rawptr-Write]`/`[Rawptr-Move-In]` at index len: len < cap after grow, so the slot lies inside the buffer allocate returned, and no element object is established there (pop and take_raw release theirs). |
| `std::collections` | `Vec::pop` | `collections.cb` 44 | `[Rawptr-Read]`, `[Rawptr-Move-Out]`, `[Release]` | `[Rawptr-Read]`/`[Rawptr-Move-Out]` then `[Release]` of the last slot: index len, after the decrement, is the element push wrote last; no path reaches it, since an index borrow would have clashed with the &mut v argument. |
| `std::collections` | `Vec::index_shared` | `collections.cb` 60 | `[Reclaim]` | `[Reclaim]` of slot i: i < len was checked just above; the cells lie inside the buffer and hold the element push wrote there; a reclaimed object already there is re-attached, not re-established. |
| `std::collections` | `Vec::index_exclusive` | `collections.cb` 73 | `[Reclaim]` | `[Reclaim]` of slot i, as index_shared: i < len was checked just above. |
| `std::collections` | `Vec::grow` | `collections.cb` 96 | `[Copy-Raw]`, `[Deallocate]` | `[Copy-Raw]` into the range allocate just returned, fresh and overlapping nothing live; `[Deallocate]` of (ptr, cap * sizeof, align), exactly what the previous grow or reserve allocated and nobody has freed. |
| `std::collections` | `Vec::reserve` | `collections.cb` 135 | `[Copy-Raw]`, `[Deallocate]` | `[Copy-Raw]` and `[Deallocate]` as grow: the new range is fresh, the old one is the previous allocation. |
| `std::collections` | `Vec::drop` | `collections.cb` 153, 161 | `[Reclaim]`, `[Deallocate]`, `[Destroy]` | `[Reclaim]` of each slot below len, as index_shared, to destroy its element; then `[Deallocate]` of the buffer, the Vec's one deallocation, under `[Destroy]`'s single-use authority. |
| `std::collections` | `Vec::replace` | `collections.cb` 399 | `[Rawptr-Read]`, `[Rawptr-Move-Out]`, `[Release]`, `[Rawptr-Write]`, `[Rawptr-Move-In]` | `[Rawptr-Read]`/`[Rawptr-Move-Out]`, `[Release]` and `[Rawptr-Write]`/`[Rawptr-Move-In]` of slot i: the caller checked i < len; the old element is moved out and its object released before the new one is written. |
| `std::collections` | `Vec::take_raw` | `collections.cb` 459 | `[Rawptr-Read]`, `[Rawptr-Move-Out]`, `[Release]` | `[Rawptr-Read]`/`[Rawptr-Move-Out]` and `[Release]` of slot i: the callers pass i < len and overwrite the slot or shorten the Vec past it before any read of it. |
| `std::collections` | `Queue::shift` | `collections.cb` 1386 | `[Copy-Raw]` | `[Copy-Raw]` between the two buffers: both ranges lie below their Vecs' capacities, reserved by the caller, and the destination slots hold no element (to.len is 0 there). |
| `std::collections` | `PriorityQueue::before` | `collections.cb` 1531 | `[Rawptr-Read]` | `[Rawptr-Read]` of the sequence numbers at i and j, both below len: plain u64 cells no path reaches. |
| `std::collections` | `PriorityQueue::sift_up` | `collections.cb` 1600, 1612 | `[Copy-Raw]`, `[Release]`, `[Rawptr-Read]`, `[Rawptr-Write]`, `[Rawptr-Move-In]` | `[Copy-Raw]`, `[Release]` and `[Rawptr-Read]`/`[Rawptr-Write]`/`[Rawptr-Move-In]` on slots below len: an element's old slot is released as it moves, and the hole t is written once; the sequence numbers are plain cells. |
| `std::collections` | `PriorityQueue::pop` | `collections.cb` 1656, 1671 | `[Copy-Raw]`, `[Release]`, `[Rawptr-Read]`, `[Rawptr-Write]` | `[Copy-Raw]`, `[Release]` and `[Rawptr-Read]`/`[Rawptr-Write]` on slots below len, as sift_up: each moved element's old slot is released before the hole is written. |
| `std::env` | `arg_count` | `env.cb` 13 | `[Extern-Call]` | `[Extern-Call]` arg_bytes with no buffer (dangling, 0): it stores nothing and answers the length. |
| `std::env` | `arg` | `env.cb` 29, 44, 51, 60 | `[Extern-Call]`, `[Rawptr-Read]`, `[Deallocate]` | `[Extern-Call]` arg_bytes into [p, len), the allocation made just above; `[Rawptr-Read]` of each byte it filled; `[Deallocate]` of that one allocation. |
| `std::fs` | `read_file` | `fs.cb` 47, 58, 65, 75 | `[Extern-Call]`, `[Rawptr-Read]`, `[Deallocate]` | `[Extern-Call]` file_read into [buf, cap), an allocation of this function; `[Rawptr-Read]` of the n <= cap bytes it filled; `[Deallocate]` of that allocation, once on each path. |
| `std::fs` | `write_file` | `fs.cb` 91 | `[Extern-Call]` | `[Extern-Call]` file_write reads the path's and the text's bytes, which the views keep live for the call. |
| `std::fs` | `File::start` | `fs.cb` 128 | `[Extern-Call]` | `[Extern-Call]` file_op(open) reads the path's bytes, which the view keeps live. |
| `std::fs` | `File::take` | `fs.cb` 171 | `[Extern-Call]` | `[Extern-Call]` file_op(read) writes buf's spare capacity [len, len + want), reserved by the caller, where no element object lies. |
| `std::fs` | `File::read_line` | `fs.cb` 224 | `[Extern-Call]` | `[Extern-Call]` file_op with no buffer: it touches no program storage. |
| `std::fs` | `File::write` | `fs.cb` 266 | `[Extern-Call]` | `[Extern-Call]` file_op(write) reads the n bytes the slice covers and keeps live. |
| `std::fs` | `File::write_formatted` | `fs.cb` 294 | `[Extern-Call]` | `[Extern-Call]` file_op(write) reads the str's n bytes, program data. |
| `std::fs` | `File::seek` | `fs.cb` 309 | `[Extern-Call]` | `[Extern-Call]` file_at with no buffer: it touches no program storage. |
| `std::fs` | `File::len` | `fs.cb` 324 | `[Extern-Call]` | `[Extern-Call]` file_at with no buffer: it touches no program storage. |
| `std::fs` | `File::create_new` | `fs.cb` 342 | `[Extern-Call]` | `[Extern-Call]` file_op(open) reads the path's bytes, which the view keeps live. |
| `std::fs` | `File::flush` | `fs.cb` 364 | `[Extern-Call]` | `[Extern-Call]` file_op with no buffer: it touches no program storage. |
| `std::fs` | `File::position` | `fs.cb` 380 | `[Extern-Call]` | `[Extern-Call]` file_at with no buffer: it touches no program storage. |
| `std::fs` | `File::close` | `fs.cb` 397 | `[Extern-Call]` | `[Extern-Call]` file_op with no buffer: it touches no program storage. |
| `std::fs` | `File::drop` | `fs.cb` 413 | `[Extern-Call]` | `[Extern-Call]` file_op with no buffer: it touches no program storage. |
| `std::fs` | `fs_path_op` | `fs.cb` 459 | `[Extern-Call]` | `[Extern-Call]` fs_op reads the two views' bytes, which the views keep live. |
| `std::io` | `flush_stdout` | `io.cb` 17 | `[Extern-Call]` | `[Extern-Call]` stdout_flush touches no program storage. |
| `std::io` | `read_line` | `io.cb` 31 | `[Extern-Call]` | `[Extern-Call]` stdin_read_line writes bytes' spare capacity [len, cap), reserved just above, where no element object lies. |
| `std::io` | `read_all_bytes` | `io.cb` 78 | `[Extern-Call]` | `[Extern-Call]` stdin_read writes out's spare capacity [len, cap), reserved just above, where no element object lies. |
| `std::io` | `print_str` | `io.cb` 113 | `[Extern-Call]` | `[Extern-Call]` stdout_write reads the str's bytes, program data. |
| `std::io` | `print_string` | `io.cb` 122 | `[Extern-Call]` | `[Extern-Call]` stdout_write reads the String's len bytes, live for the call through the shared reference. |
| `std::io` | `eprint_str` | `io.cb` 131 | `[Extern-Call]` | `[Extern-Call]` stderr_write reads the str's bytes, program data. |
| `std::memory` | `Rc::new` | `memory.cb` 26 | `[Rawptr-Write]`, `[Rawptr-Move-In]` | `[Rawptr-Write]`/`[Rawptr-Move-In]` into the box allocate just returned: fresh cells with no object over them. |
| `std::memory` | `Rc::clone` | `memory.cb` 36 | `[Reclaim]` | `[Reclaim]` of the box: ptr came from Rc::new and the box lives while any Rc or Weak holds it; only count is borrowed, disjoint from a live Rc::get reference into value. |
| `std::memory` | `Rc::get` | `memory.cb` 49 | `[Reclaim]` | `[Reclaim]` of the box, as clone; a live Rc keeps value Some. |
| `std::memory` | `Rc::drop` | `memory.cb` 63, 72, 79 | `[Reclaim]`, `[Deallocate]` | `[Reclaim]` of the box, as clone; when count reaches 0 the value is ended through the reclaimed object, and with weak 0 too the box is destroyed and `[Deallocate]`d once, the single deallocation of what Rc::new allocated. |
| `std::memory` | `Rc::downgrade` | `memory.cb` 97 | `[Reclaim]` | `[Reclaim]` of the box, as clone: only weak is borrowed. |
| `std::memory` | `Weak::upgrade` | `memory.cb` 109 | `[Reclaim]` | `[Reclaim]` of the box, as Rc::clone: the box lives while any Weak holds it; only count is borrowed. |
| `std::memory` | `Weak::clone` | `memory.cb` 124 | `[Reclaim]` | `[Reclaim]` of the box, as Rc::clone: only weak is borrowed. |
| `std::memory` | `Weak::drop` | `memory.cb` 136, 144 | `[Reclaim]`, `[Deallocate]` | `[Reclaim]` of the box, as Rc::clone; when both counts are 0 this is the last handle, so the box is destroyed and `[Deallocate]`d once. |
| `std::memory` | `Box::new` | `memory.cb` 184 | `[Rawptr-Write]`, `[Rawptr-Move-In]` | `[Rawptr-Write]`/`[Rawptr-Move-In]` into the cells allocate just returned: fresh, with no object over them. |
| `std::memory` | `Box::get` | `memory.cb` 194 | `[Reclaim]` | `[Reclaim]` of the value: ptr came from Box::new and full holds while the Box lives. |
| `std::memory` | `Box::get_mut` | `memory.cb` 203 | `[Reclaim]` | `[Reclaim]` of the value, as get. |
| `std::memory` | `Box::into_inner` | `memory.cb` 220 | `[Rawptr-Read]`, `[Rawptr-Move-Out]`, `[Release]` | `[Rawptr-Read]`/`[Rawptr-Move-Out]` and `[Release]` of the value: full was true and is now false, so drop will not reclaim it again. |
| `std::memory` | `Box::drop` | `memory.cb` 231 | `[Reclaim]`, `[Deallocate]`, `[Destroy]` | `[Reclaim]` of the value when full, then `[Deallocate]` of the one allocation Box::new made, under `[Destroy]`'s single-use authority. |
| `std::net` | `TcpStream::take` | `net.cb` 459 | `[Rawptr-Offset]` | `[Rawptr-Offset]` to buf's spare capacity [len, len + want), reserved by the caller; the call below writes only there. |
| `std::process` | `Child::take` | `process.cb` 218 | `[Rawptr-Offset]` | `[Rawptr-Offset]` to buf's spare capacity [len, len + want), reserved by the caller; the call below writes only there. |
| `std::process` | `ChildOutput::read` | `process.cb` 448 | `[Rawptr-Offset]` | `[Rawptr-Offset]` to buf's spare capacity [len, len + want), reserved by the caller; the call below writes only there. |
| `std::random` | `os_random_vec` | `random.cb` 85 | `[Extern-Call]` | `[Extern-Call]` os_random writes n bytes of b's buffer, reserved just above, where no element object lies. |
| `std::sync` | `Channel::new` | `sync.cb` 50, 54 | `[Extern-Call]` | `[Extern-Call]` event_op(new) touches no program storage. |
| `std::sync` | `Channel::send` | `sync.cb` 81, 87, 92 | `[Extern-Call]` | `[Extern-Call]` event_op on the channel's own event ids: it touches no program storage. |
| `std::sync` | `Channel::recv` | `sync.cb` 112, 122, 127 | `[Extern-Call]` | `[Extern-Call]` event_op on the channel's own event ids: it touches no program storage. |
| `std::sync` | `Channel::try_recv` | `sync.cb` 145 | `[Extern-Call]` | `[Extern-Call]` event_op on the channel's own event ids: it touches no program storage. |
| `std::sync` | `Channel::recv_timeout_ms` | `sync.cb` 165, 178, 186 | `[Extern-Call]` | `[Extern-Call]` clock_read and sleep_ns touch no program storage. |
| `std::sync` | `Channel::close` | `sync.cb` 214 | `[Extern-Call]` | `[Extern-Call]` event_op on the channel's own event ids: it touches no program storage. |
| `std::sync` | `Channel::drop` | `sync.cb` 233 | `[Extern-Call]` | `[Extern-Call]` event_op(end) on the channel's own event ids: it touches no program storage. |
| `std::sync` | `failure_text` | `sync.cb` 299 | `[Extern-Call]` | `[Extern-Call]` task_op writes [out.ptr, out.ptr + cap), out's reserved buffer. |
| `std::text` | `String::append_string` | `text.cb` 947 | `[Rawptr-Read]` | `[Rawptr-Read]` of t's bytes below len, live for the call through the shared reference. |
| `std::text` | `String::append_text` | `text.cb` 964, 971, 978 | `[Extern-Call]`, `[Rawptr-Read]`, `[Deallocate]` | `[Extern-Call]` text_write writes at most 64 bytes at p, the allocation made just above; `[Rawptr-Read]` of the n it wrote; `[Deallocate]` of that allocation. |
| `std::text` | `parse_bytes` | `text.cb` 997, 1003 | `[Extern-Call]` | `[Extern-Call]` parse_check and parse_value read [p, p + n), bytes the caller keeps live. |
| `std::time` | `unix_ms` | `time.cb` 474 | `[Extern-Call]` | `[Extern-Call]` clock_read touches no program storage. |
| `std::time` | `local_offset_seconds` | `time.cb` 488 | `[Extern-Call]` | `[Extern-Call]` tz_offset touches no program storage. |
| `std::time` | `monotonic_ns` | `time.cb` 517 | `[Extern-Call]` | `[Extern-Call]` clock_read touches no program storage. |
| `std::time` | `unix_seconds` | `time.cb` 527 | `[Extern-Call]` | `[Extern-Call]` clock_read touches no program storage. |
| `std::time` | `sleep_ms` | `time.cb` 538 | `[Extern-Call]` | `[Extern-Call]` sleep_ns touches no program storage. |
<!-- trusted-table:end -->

**The module `std`** (`CHG-0033`, D-0024). Every declaration this
artifact gives in CobaltC — the types and functions of §0–§3d, the
externs `stdout_write`, `stdin_read` and `arg_bytes`, and
`Result::map_err` — is an item of the module `std`, declared at
the root of every program, or of one of its submodules (D-0136,
`CHG-0164`). Its names are `std::Vec`, `std::Vec::push`, `std::printf`,
…; a module uses them unqualified after `import std;` (or a single-name
import such as `import std::Vec;`), and any module may write them
qualified. Being a module, `std` keeps what it does not export:
`Vec`'s, `String`'s and `Rc`'s fields, `RcBox`, and `Vec::grow` are
reachable only from inside it. §0's tables say which names are which:
those headed **Language intrinsics** belong to the language and need no
import (with the type `str`); those headed **`std`** are items of `std`
(D-0134). An intrinsic is not an item and its name is not reserved: a
name that resolves to a local binding (`rule.value-object.binding-lookup`)
or to an item (`[Resolve-Unqualified]`, `spec/17`) means that binding or
item, and the intrinsic of the same name is reached only where nothing
else is (`CHG-0036`). An item of an intrinsic's name is found only in
its own module and where an `import` names it, never through an
enclosing module (`[Resolve-Unqualified]` (3), D-0055), so `std`'s own
code always reaches the intrinsics. A program may not declare its own
root-level `std` (`[Item-Duplicate]`).

**`std`'s submodules** (D-0136). `std` is divided by subject. Each
submodule declares the items of the sections listed, and `std`'s root
re-exports every one of them (`export import`, `spec/17` §3):

    export module std
    {
        -- the primitives private to std (§2b, §2e, §2e′, §3c)
        export module core "core.cb";
        export module collections "collections.cb";
        export module text "text.cb";
        export module io "io.cb";
        export module fs "fs.cb";
        export module env "env.cb";
        export module memory "memory.cb";
        export module sync "sync.cb";
        export module random "random.cb";
        export module math "math.cb";
        export module time "time.cb";
        export module process "process.cb";
        export module net "net.cb";
        export module crypto "crypto.cb";       -- with digest, kdf, aead, pk (D-0187)
        export module x509 "x509.cb";
        export module tls "tls.cb";
        export module http "http.cb";           -- with client, server (D-0187)
        export module database "database.cb";   -- with postgres (D-0187)
        export module encoding "encoding.cb";   -- with json, hex, base64 (D-0197, D-0198)
        export module compress "compress.cb";
        export module extensions "extensions.cb"; -- D-0187: declared, never re-exported
        export import std::core;
        export import std::collections;
        export import std::text;
        export import std::io;
        export import std::fs;
        export import std::env;
        export import std::memory;
        export import std::sync;
        export import std::random;
        export import std::math;
        export import std::time;
        export import std::process;
        export import std::net;
        export import std::crypto;
        export import std::x509;
        export import std::tls;
        export import std::http;
        export import std::database;
        export import std::encoding;
        export import std::compress;
    }

| Submodule | Items | Sections |
|---|---|---|
| `std::core` | `Option`, `Result`, `AllocError`, `assert`, `swap`, `replace`, `overwrite` | §0, §3b |
| `std::collections` | `Vec` (with sorting, `retain`, `dedup`, `Vec::push_le`/`push_be`), `read_le`/`read_be`, `HashMap`, `HashSet`, `Queue`, `PriorityQueue`, `slice_swap`, `slice_parts`, `slice_eq` | §1, §1b, §2g, §2i |
| `std::text` | `String`, `StringView` (with `strip_prefix`, `strip_suffix`, `lines`, `split_whitespace`, `code_point`; `String::push_code_point`), `Utf8Error`, `ParseError`, the `ascii_` functions, `sprintf`, `String::appendf` | §2, §2a, §2d, §2f, §2h |
| `std::io` | `printf`, `eprintf`, `flush_stdout`, `stdout_write`, `stdin_read`, `read_line`, `read_all`, `read_all_bytes` | §2a, §2b, §2f |
| `std::fs` | `FileError`, `File` (with `create_new`, `flush`, `position`), `read_file`, `write_file`, `read_bytes`, `write_bytes`, the directory functions, `PathKind`, the path functions, `FileInfo`, `file_info`, `copy_file`, `make_dir_all`, `remove_dir_all` | §2e, §2e′ (D-0176) |
| `std::env` | `arg_count`, `arg`, `arg_bytes`, `Args`, `env_var`, `current_dir`, `set_current_dir`, `temp_dir`, `home_dir`, `current_exe`, `hostname` | §2c, §2e′ (D-0176) |
| `std::memory` | `Box`, `Rc`, `Weak` | §3, §3a |
| `std::sync` | `Channel`, `cpu_count`, `try_join`, `is_finished`, `ThreadFailure` | §3c, §3c′ |
| `std::random` | `Rng` (with `shuffle`), `os_random_bytes`, `os_random_u64`, `uuid_v4` | §0 |
| `std::math` | `min`, `max`, `abs`, `pow`, `clamp`, `gcd`, `is_nan`, `is_finite`, `BigUint`, `BigDivision`, `sqrt`, `floor`, `ceil`, `round`, `trunc`, `ln`, `exp`, `log2`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `powf` | §0, §3d |
| `std::time` | `DateTime` (with `to_http_date`, `from_http_date`), `Weekday`, `unix_ms`, `iso_from_unix_ms`, `unix_ms_from_iso`, `local_offset_seconds`, `monotonic_ns`, `unix_seconds`, `sleep_ms` | §2e′, §2j |
| `std::process` | `Command`, `Output`, `Child` (with `terminate`, `read_error_line`), `process_id`, `watch_interrupts`, `interrupt_requested`, `exit` | §2k |
| `std::net` | `IpAddr`, `SocketAddr`, `resolve`, `TcpListener`, `TcpStream` (with `try_clone`, `set_keepalive`), `UdpSocket` (with `connect`, `send`, `recv`), `Datagram`, `NetError` | §2l |
| `std::crypto` (D-0187: four submodules, each re-exported here) | `crc32` (§2g; not cryptographic), `Sha256`, `sha256`, `HmacSha256`, `hmac_sha256`, `digest_eq`, `hkdf_extract`, `hkdf_expand`, `hkdf_sha256`, `hkdf_expand_label`, `chacha20`, `poly1305`, `chacha20_poly1305_seal`, `chacha20_poly1305_open`, `x25519`, `x25519_public_key`, `x25519_private_key`, `aes_encrypt_block`, `aes_gcm_seal`, `aes_gcm_open`, `p256_private_key`, `p256_public_key`, `p256_ecdh`, `Sha512`, `sha512`, `Sha384`, `sha384`, `HashKind`, `hash`, `RsaPublicKey`, `rsa_verify_pkcs1v15`, `rsa_verify_pss`, `EcCurve`, `ecdsa_verify`, `Hasher`, `Hmac`, `hmac`, `hkdf_extract_with`, `hkdf_expand_with`, `hkdf_with`, `hkdf_expand_label_with`, `ec_private_key`, `ec_public_key`, `ecdh`, `ecdsa_sign`, `ed25519_private_key`, `ed25519_public_key`, `ed25519_sign`, `ed25519_verify`, `RsaPrivateKey`, `rsa_sign_pkcs1v15`, `rsa_sign_pss`, `rsa_generate_key`, `Blake2b`, `blake2b`, `pbkdf2`, `argon2id`, `hash_password`, `verify_password`, `Sha1`, `sha1` (legacy) | §2g, §2m |
| `std::crypto::digest` | `Sha1`, `Sha256`, `Sha384`, `Sha512`, `sha1`, `sha256`, `sha384`, `sha512`, `Blake2b`, `blake2b`, `HashKind`, `hash`, `Hasher`, `digest_eq`, `crc32` | §2g, §2m |
| `std::crypto::kdf` | `HmacSha256`, `hmac_sha256`, `Hmac`, `hmac`, `hkdf_extract`, `hkdf_expand`, `hkdf_sha256`, `hkdf_expand_label`, the `_with` forms, `pbkdf2`, `argon2id`, `hash_password`, `verify_password` | §2m |
| `std::crypto::aead` | `chacha20`, `poly1305`, `chacha20_poly1305_seal`, `chacha20_poly1305_open`, `aes_encrypt_block`, `aes_gcm_seal`, `aes_gcm_open` | §2m |
| `std::crypto::pk` | `x25519`, `x25519_public_key`, `x25519_private_key`, `EcCurve`, `ec_private_key`, `ec_public_key`, `ecdh`, `p256_private_key`, `p256_public_key`, `p256_ecdh`, `ecdsa_verify`, `ecdsa_sign`, `ed25519_private_key`, `ed25519_public_key`, `ed25519_sign`, `ed25519_verify`, `RsaPublicKey`, `rsa_verify_pkcs1v15`, `rsa_verify_pss`, `RsaPrivateKey`, `rsa_sign_pkcs1v15`, `rsa_sign_pss`, `rsa_generate_key` | §2m |
| `std::x509` | `Der`, `der_read`, `Certificate`, `CertKey`, `EcPublicKey`, `SignatureScheme`, `CertError`, `certificates_from_pem`, `TrustStore`, `verify_signature`, `verify_chain`, `EcPrivateKey`, `PrivateKey`, `sign` | §2n |
| `std::tls` | `TlsStream` (client and server), `TlsError` | §2o |
| `std::http` (D-0187: two submodules, each re-exported here) | `Url`, `percent_encode`, `percent_decode`, `Param`, `find_param`, `form_decode`, `form_encode`, `Header`, `find_header`, `Request`, `Response`, `reason_phrase`, `HttpError` (declared here, used by both); `HttpClient`, `http_get`, `http_post`, `HttpServer` (`bind`, `bind_tls`), `HttpConnection` | §2p |
| `std::http::client` | `HttpClient`, `http_get`, `http_post` | §2p |
| `std::http::server` | `HttpServer` (`bind`, `bind_tls`), `HttpConnection` | §2p |
| `std::database` (D-0187: one submodule per server, each re-exported here) | `Scram`, `scram_salted_password` (declared here: what every client shares); `PgConnection`, `PgUrl`, `PgSslMode`, `PgValue`, `PgColumn`, `PgRow` (with `get_i64`, `get_f64`, `get_bool`, `get_bytes`), `PgResult`, `PgError`, `PgServerError` | §2q |
| `std::database::postgres` | `PgConnection`, `PgUrl`, `PgSslMode`, `PgValue`, `PgColumn`, `PgRow`, `PgResult`, `PgError`, `PgServerError` | §2q |
| `std::extensions` | nothing the specification defines (D-0187, below) | §0 |
| `std::encoding` (D-0197, D-0198: one submodule per data format or byte encoding, each re-exported here) | `Json`, `JsonMember`, `to_hex`, `from_hex`, `base64_encode`, `base64_decode` | §2d, §2r |
| `std::encoding::json` | `Json`, `JsonMember` | §2r (D-0182, D-0197) |
| `std::encoding::hex` | `to_hex`, `from_hex` | §2d `[Hex]` (D-0151, D-0198) |
| `std::encoding::base64` | `base64_encode`, `base64_decode` | §2d `[Base64]` (D-0151, D-0198) |
| `std::compress` | `deflate`, `inflate`, `zlib_deflate`, `zlib_inflate`, `gzip`, `gunzip`, `adler32`, `CompressError` | §2s (D-0183) |

So `import std;` brings in the whole library, as it did before the
division, and `std::collections::Vec` is `std::Vec`, one type. A
program may import one subject alone (`import std::text;`). An item
with several paths is named by its shortest in every message
(`std::Vec`). An associated function is declared in the submodule that
declares its type (`[Assoc-Fn-Foreign-Type]`), which is why `std` is
divided by type.

`std`'s submodules share one privacy: a private item or field of any of
them is visible in all of `std`'s code, as it was when `std` was one
module. `File::read` fills a `Vec<u8>`'s spare room, and a path is
handed to the system as a `StringView`'s bytes, so `std::fs` and
`std::env` need what `std::collections` and `std::text` keep private.

Each item lives in the submodule whose existing items it works with:
files and paths in `std::fs`, the program's arguments and environment in
`std::env`, clocks in `std::time`, the standard streams in `std::io`.
There is no catch-all submodule (D-0176).

**Nested submodules** (D-0187). A submodule with several subjects of its
own divides again, the same way: `std::crypto` into `digest`, `kdf`,
`aead` and `pk`; `std::http` into `client` and `server`; `std::database`
into one submodule per database server, `postgres` today;
`std::encoding` (D-0197, D-0198) into one submodule per data format
or encoding of bytes as text: `json`, `hex`, `base64`. Each is
re-exported by its parent, and the parent by `std`, so every item keeps
its short name (`sha256`, `PgConnection`) and every path to it names the
one item: `std::crypto::digest::sha256` is `std::sha256`. What the
children share, the parent declares (`std::database`'s SCRAM, the
message reader of `std::http`). A submodule's name is never an item's:
the digests are `std::crypto::digest`, since `hash` is a function.

**`std::extensions`** (D-0187) is the implementation-defined area:
`std::extensions` exists in every implementation, its submodules, if
any, are the implementation's own and documented by it, and it may be
empty. No item this specification defines is declared there, and
`spec/conformance.md` requires nothing of it and tests nothing in it.
`std`'s root declares it but does not re-export it (`spec/17` §3): a
program reaches an extension only by naming it, `import
std::extensions::m;` or `std::extensions::m::f`, so its dependence on one
implementation is visible in its source; such a program is conforming
and implementation-dependent (`spec/00`
`term.implementation-dependent-program`). An extension's items keep
their full paths as their keys and in every message, so an extension can
never share a key with, nor shadow, an item of `std`. The reference
implementation's `std::extensions` is empty.
No program is inside `std`, so no program can tell; to a program, each
submodule exports exactly what the tables below list.

## 0. Prelude

### `rule.stdlib.prelude`
**Status:** ACCEPTED

**Types in `std`** (unqualified after `import std;`):

    export enum Option<T>
    {
        Some(T),
        None
    }

    export enum Result<T, E>
    {
        Ok(T),
        Err(E)
    }

    export struct AllocError
    {
    }

    export struct Utf8Error
    {
        export usize offset;
    }

    export enum FileError                    -- rule.stdlib.file, rule.stdlib.read: files and standard input
    {
        NotFound,
        Denied,
        Io,
        Utf8(Utf8Error)
    }

    export enum PathKind                     -- rule.stdlib.fs
    {
        File(u64),
        Dir,
        Other
    }

    export struct FileInfo                   -- rule.stdlib.fs (D-0141)
    {
        export PathKind kind;
        export u64 len;
        export i64 modified_unix;
        export bool readonly;
    }

    export enum ParseError                   -- rule.stdlib.text
    {
        Empty,
        Invalid(usize),
        OutOfRange
    }

**Conventions** (D-0134). `std`'s names follow a few rules, so a
function's name and signature can be guessed from its intent:

- A function is named under the type of its first parameter when that
  type is one of `std`'s (`Vec::push(&mut v, x)`, `Result::map_err(r, f)`,
  `String::parse<T>(&s)`); otherwise its name stands alone (`min`, `swap`,
  `crc32`, `read_le(s, at)`). A built-in type's few accessors carry its
  name as a prefix instead (`str_len`, `slice_len`), since a built-in
  type has no namespace.
- `T::new(…)` makes a `T` from its defining arguments; `from_X` makes one
  from an `X`, `as_X` borrows as an `X`, `into_X` consumes into an `X`,
  `to_X` makes a new derived value.
- `_mut` is the exclusive twin (`get`/`get_mut`), `_by` takes the
  caller's function (`sort_by`), `_at` reaches by position (`key_at`),
  `is_` asks without consuming (`Option::is_some(&o)`), `_str` is the twin
  that takes a `str` (`HashMap::get_str`).
- Text a function only reads is a `StringView` (a path, a needle, text to
  write): a literal is one already (`[Str-Literal-View]`, §2a), a `String`
  passes `&s[0..$]`. The exception is `split`'s separator, a `str`:
  `split` returns views of its first argument, so by
  `rule.temporal.elision` it takes no other borrowed parameter.
- A failure a program can handle is a `Result`, an absence an `Option`;
  misuse (an index out of bounds, `unwrap` of `None`) is a fault. Every
  input and output failure is a `FileError`.
- `remove` keeps the order of what remains, in every collection;
  `swap_remove` is the constant-time removal that does not.
- A unit of measure is part of a name (`monotonic_ns`, `unix_seconds`,
  `sleep_ms`).

**Intrinsics and functions** — an intrinsic's call is defined by the
named rule rather than by a body; each is in scope unqualified; type
arguments are written `name<τ>(…)` where the signature needs them. The
functions of `std` are in scope after `import std;`:

**Language intrinsics: values and arithmetic**

| Signature | Rule |
|---|---|
| `drop<T>(T x)` (argument in place position) | `rule.resauth.destroy` `[Destroy]` on the place; a temporary argument is destroyed directly |
| `wrapping_add/sub/mul<T>(T a, T b) : T`, `saturating_*` | `rule.arith.alt` |
| `checked_add/sub/mul/div/rem<T>(T a, T b) : Option<T>` | `rule.arith.alt` |
| `checked_neg<T>(T a) : Option<T>` | `checked_sub(0, a)` (D-0072): `None` for the most negative value of a signed type and for every non-zero unsigned value |
| `widen<U>(T x) : U`, `narrow<U>`, `narrow_wrapping<U>`, `reinterpret<U>`, `to_float<U>`, `to_int<U>` | `rule.arith.convert` |
| `checked_narrow<U>(T x) : Option<U>` (`T`, `U` integer types) | `rule.arith.convert` `[Checked-Narrow]` (D-0122) |
| `count_ones(T x) : u32`, `leading_zeros(T x) : u32`, `trailing_zeros(T x) : u32`, `rotate_left(T x, u32 k) : T`, `rotate_right(T x, u32 k) : T` (`T` an integer type) | `rule.arith.bitwise` `[Count-Ones]`, `[Leading-Zeros]`, `[Trailing-Zeros]`, `[Rotate]` (D-0123) |
| `sizeof<T>() : usize`, `alignof<T>() : usize` | `rule.arith.sizeof` |
| `min_value<T>() : T`, `max_value<T>() : T` (`T` an integer type, `f32` or `f64`) | `rule.arith.limits` |

**Language intrinsics: text, slices, checks and faults**

| Signature | Rule |
|---|---|
| `str_len(str s) : usize`, `str_byte(str s, usize i) : u8`, `str_ptr(str s) : rawptr<u8>` | `rule.stdlib.str` `[Str-Len]`, `[Str-Byte]`, `[Str-Ptr]` (§2a); all safe |
| `slice_len(slice<τ, m> s) : usize` (also through a reference) | a slice's length, `spec/16` `rule.agg.slice` (D-0047); safe |
| `static_assert(bool c)`, `static_assert(bool c, str message)` : unit (`c` a constant expression, the message a string literal) | `rule.module.static-assert` (`spec/17` §1b): computed before the program runs; nothing at run time |
| `fault(d) : never`, `fault(d, m) : never` (D-0083; `d` names a diagnostic of phase `dynamic` or `both` in `spec/registry/diagnostics.md`, written with `_` for `-`, such as `index_out_of_bounds`, `alloc_failure`, `assert_failed`, `unwrap_failed`; `m` a `str` message) | `[Fault]`: `⟨fault(d), Σ⟩ ↛ diag.d` (with `m` shown after the location), `disposition: checked`; typed `never` (`[T-Never]`, `spec/12` §5), so it may stand where any type is expected. `[Fault-Name]`: any other `d` is `diag.unbound-name` (static) |

**Language intrinsics: raw memory and threads**

| Signature | Rule |
|---|---|
| `dangling<T>() : rawptr<T>` | `outcome: impl-defined { any T-aligned non-null address }`, documented; never dereferenced by prelude code |
| `rawptr_of<T>(ref<T, shared> r) : rawptr<T>`, and the `exclusive` overload | `rule.trust.rawptr` `[Rawptr-Of]` |
| `reinterpret_ptr<U>(rawptr<T> p) : rawptr<U>` | identity on the address; safe |
| `reclaim<T>(rawptr<T> p) : place of T` | `[Reclaim]` (unsafe) |
| `release(rawptr<u8> p, usize n)` | `[Release]` (unsafe) |
| `copy_raw(rawptr<u8> dst, rawptr<u8> src, usize n)` | `[Copy-Raw]` (unsafe) |
| `allocate(usize n, usize align) : Result<rawptr<u8>, AllocError>` | `[Allocate]` below |
| `deallocate(rawptr<u8> p, usize n, usize align)` | `[Deallocate]` below (unsafe) |
| `read_volatile<T>(rawptr<T> p) : T`, `write_volatile<T>(rawptr<T> p, T v)` (`T` not a resource; inside `unsafe`) | `rule.trust.rawptr` `[Rawptr-Read-Volatile]`, `[Rawptr-Write-Volatile]` (D-0121) |
| `lock<T>(ref<mutex<T>, shared> m) : guard<T>` | `rule.conc.lock` |
| `Mutex::new<T>(T v) : mutex<T>` | `rule.conc.lock` (§2's construction) |
| `spawn(fn(τ1..τn) : τr f, τ1 a1, …, τn an) : handle<τr>` — argument count and types match `f`'s own signature, not a fixed arity | `rule.conc.spawn` `[Spawn]`, typed by `[T-Spawn]` (`spec/12`); `f` must be a `fn` value or a `move` closure |
| `join<T>(handle<T> h) : T` | `rule.conc.join` `[Join]`, typed by `[T-Join]` (`spec/12`); a failed thread's failure is raised again, `[Join-Failed]` (D-0204) |
| `cancel<T>(ref<handle<T>, shared> h)` | `rule.conc.cancel` `[Cancel]` (`spec/19`, D-0204) |
| `join_status<T>(ref<handle<T>, shared> h, u8 mode) : u64` | D-0204, the primitive under `try_join` and `is_finished` (§3c′): 0 while the thread runs, 1 when it ended with a result, `k + 2` when it failed with failure `k`; mode 0 first waits for the thread to end and takes its failure (its handle then raises nothing), mode 1 only looks |

**`std::core`: `Option`, `Result` and the errors** (each error's `text` with its type: `std::io`, `std::text`)

| Signature | Rule |
|---|---|
| `Option::unwrap_or<T>(Option<T> o, T d) : T` | ordinary exported function of `std`: `match (o) { Some(v) : v, None : d }` (D-0033) |
| `Result::unwrap_or<T, E>(Result<T, E> r, T d) : T` | ordinary exported function of `std`: `match (r) { Ok(v) : v, Err(_) : d }` (D-0033); an unused `d` or error is destroyed at the call's end |
| `Result::ok<T, E>(Result<T, E> r) : Option<T>` | ordinary exported function of `std` (D-0205): `match (r) { Ok(v) : Some(v), Err(_) : None }`; the error, if any, is destroyed at the call's end |
| `Option::ok_or<T, E>(Option<T> o, E e) : Result<T, E>` | ordinary exported function of `std`: `match (o) { Some(v) : Ok(v), None : Err(e) }` (D-0114) — the `Option` as a `Result` for `?`; `e` is evaluated before the call and, unused, ends with the statement |
| `Option::unwrap<T>(Option<T> o) : T`, `Option::expect<T>(Option<T> o, str m) : T`, `Result::unwrap<T, E>(Result<T, E> r) : T`, `Result::expect<T, E>(Result<T, E> r, str m) : T` | ordinary exported functions of `std` (D-0082): the payload of `Some`/`Ok`; on `None`/`Err`, `fault(unwrap_failed, m)` (`diag.unwrap-failed`), `m` naming the call for `unwrap`; an `Err`'s payload is destroyed |
| `Option::is_some<T>(ref<Option<T>, shared> o) : bool`, `Option::is_none`, `Result::is_ok<T, E>(ref<Result<T, E>, shared> r) : bool`, `Result::is_err` | ordinary exported functions of `std`: which variant, by a `match` through the reference; nothing is consumed (D-0073) |
| `Result::map_err<T, E1, E2>(Result<T, E1> r, fn(E1) : E2 f) : Result<T, E2>` | ordinary exported function of `std`: `match (r) { Ok(v) : Ok(v), Err(e) : Err(f(e)) }` (table cell — exempt from Allman, `spec/22` §1) |
| `FileError::text(ref<FileError, shared> e) : str`, `ParseError::text(ref<ParseError, shared> e) : str`, `Utf8Error::text(ref<Utf8Error, shared> e) : str` | ordinary exported functions of `std` (D-0117): the error as a sentence for a person — `FileError`: `NotFound` "not found", `Denied` "permission denied", `Io` "input/output error", `Utf8(_)` "not UTF-8"; `ParseError`: `Empty` "empty", `Invalid(_)` "not a number", `OutOfRange` "out of range"; `Utf8Error` "not UTF-8". A `str`: nothing is allocated; an offset inside the error is read by `match` |

**`std::collections`: `Vec`**

| Signature | Rule |
|---|---|
| `Vec::retain<T>(ref<Vec<T>, exclusive> v, fn(ref<T, shared>) : bool keep)`, `Vec::dedup<T: eq>(ref<Vec<T>, exclusive> v)` | ordinary exported functions of `std` (D-0181): `[Retain]`, `[Dedup]` (§1) — the elements `keep` accepts kept in order and the rest destroyed; each element equal to the one before it removed |
| `Vec::insert<T>(ref<Vec<T>, exclusive> v, usize i, T x)` (`i ≤ len`), `Vec::remove<T>(ref<Vec<T>, exclusive> v, usize i) : T` (`i < len`) | ordinary exported functions of `std`: `x` pushed then moved down to `i` by `Vec::swap`, or the element at `i` moved to the end by `Vec::swap` and popped; `diag.index-out-of-bounds` otherwise (D-0073) |
| `Vec::swap_remove<T>(ref<Vec<T>, exclusive> v, usize i) : T` (`i < len`) | an ordinary exported function of `std`: the element at `i` exchanged with the last by `Vec::swap` and popped, so the order is not kept and no other element moves; `diag.index-out-of-bounds` otherwise (D-0193) |
| `Vec::clone<T: clone>(ref<Vec<T>, shared> v) : Vec<T>`, `Vec::clone_by<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>) : T copy) : Vec<T>` | ordinary exported functions of `std` (D-0082): a new `Vec` of the elements copied in order, each by `clone` (D-0116; a `T` that is not `clone` is `[Bound-Unsatisfied]` at the call) or by `copy` (`clone_by`) |
| `Vec::from_slice<T: clone>(slice<T, shared> s) : Vec<T>`, `Vec::truncate<T>(ref<Vec<T>, exclusive> v, usize n)` | ordinary exported functions of `std` (D-0085): a new `Vec` of the slice's elements, each copied by `clone` (D-0116); the first `n` elements kept and the rest destroyed, last first (`n` at or beyond the length leaves the `Vec` unchanged) |
| `Vec::filled<T: clone>(usize n, T x) : Vec<T>`, `Vec::from_fn<T>(usize n, fn(usize) : T f) : Vec<T>`, `Vec::append<T>(ref<Vec<T>, exclusive> a, Vec<T> b)`, `Vec::extend_from<T: clone>(ref<Vec<T>, exclusive> a, slice<T, shared> s)`, `Vec::position<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>) : bool p) : Option<usize>`, `Vec::index_of<T: eq>(ref<Vec<T>, shared> v, ref<T, shared> x) : Option<usize>`, `Vec::contains<T: eq>(ref<Vec<T>, shared> v, ref<T, shared> x) : bool`, `Vec::reverse<T>(ref<Vec<T>, exclusive> v)` | ordinary exported functions of `std` (D-0112): `filled` — `n` copies of `x` by `clone` (D-0116); `from_fn` — `f(0), f(1), …, f(n-1)` pushed in that order, any `T`; `append` — `b`'s elements moved onto the end of `a` in order, `b` consumed; `extend_from` — the slice's elements copied by `clone` onto the end of `a` in order (a slice of `a` itself is `diag.aliasing-conflict`, `[Slice-Form]`); `position` — `Some(i)` for the least `i` with `p(&v[i])`, else `None`, `p` called in index order and not past the first hit; `index_of` — the least `i` with `v[i] == *x` (`[Cmp-*]`, in place for `String`), else `None`; `contains` — whether `index_of` finds one; `reverse` — the order inverted in place by `Vec::swap`, nothing destroyed, an empty or one-element `Vec` unchanged |
| `slice_eq<T: eq>(slice<T, shared> a, slice<T, shared> b) : bool` | exported function of `std::collections` (D-0205): `[Slice-Eq]` — true when `slice_len(a) = slice_len(b)` and `a[i] == b[i]` (`[Cmp-*]`) for every `i`, compared in index order and not past the first difference |
| `Vec::sort`, `Vec::sort_by`, `Vec::binary_search`, `Vec::binary_search_by` | exported functions of `std`, written in CobaltC over the std-only intrinsic `key_less`: `rule.stdlib.sort` (§1b) |
| `Vec::push_le<T: integer>(ref<Vec<u8>, exclusive> out, T x)`, `Vec::push_be<T: integer>(ref<Vec<u8>, exclusive> out, T x)`, `read_le<T: integer>(slice<u8, shared> s, usize at) : T`, `read_be<T: integer>(slice<u8, shared> s, usize at) : T` | ordinary exported functions of `std` (D-0119), written in CobaltC over `sizeof<T>()`, `>>`, `<<` and `narrow_wrapping`: `[Bytes-LE]`/`[Bytes-BE]` below — the `sizeof<T>()` bytes of `x`'s two's-complement image, least (`le`) or most (`be`) significant first, pushed onto `out`; the `T` whose image those bytes are, starting at `s[at]`; `diag.index-out-of-bounds` when fewer than `sizeof<T>()` bytes remain from `at` |

**`std::text`: `String`, `StringView`, numbers as text**

| Signature | Rule |
|---|---|
| `StringView::strip_prefix(StringView v, StringView t) : Option<StringView>`, `strip_suffix`, `StringView::lines(StringView v) : Vec<StringView>`, `StringView::split_whitespace(StringView v) : Vec<StringView>`, `StringView::code_point(StringView v) : Option<u32>`, `String::push_code_point(ref<String, exclusive> s, u32 cp)`, and the `String::` twins of the first five | ordinary exported functions of `std` (D-0181): `[Strip-Prefix]`, `[Lines]`, `[Split-Whitespace]`, `[Code-Point]` (§2h); `parse<bool>` is `[Parse-Bool]` (§2d) |
| `StringView` and its functions (`StringView::of`, `String::view`, `StringView::sub`, `len`, `eq`, `eq_str`, `find`, `starts_with`, `ends_with`, `trim`, `trim_start`, `trim_end`, `split`, `chars`, `contains`, `replace`, `join`, `parse<T>`, `String::from_view`) | exported items of `std`, written in CobaltC: `rule.stdlib.stringview` (§2h); `find`, `starts_with`, `ends_with`, `contains` and `replace` take the text sought as a `StringView`, `split` its separator as a `str` (D-0134); `contains`, `replace` and `join` are `[Contains]`, `[Replace]`, `[Join]` (D-0139); `&s[lo .. hi]` on a `String` and `==` with a view are its `[View-Form]` and `[View-Eq]`; `StringView::as_bytes(StringView v) : slice<u8, shared>` is `[View-As-Bytes]` (D-0163) |
| `StringView::from_utf8(slice<u8, shared> bytes) : Result<StringView, Utf8Error>` | exported function of `std::text` (D-0205): `[View-From-Utf8]` — the bytes viewed as text, borrowing `bytes` as `[View-Form]` does, when they are well-formed UTF-8; otherwise `Err(Utf8Error)` with the offset of the first byte of the first ill-formed sequence, as `String::from_utf8` reports it |
| `String::as_view(ref<String, shared> s) : StringView`, `String::find(ref<String, shared> s, StringView t) : Option<usize>`, `String::starts_with(ref<String, shared> s, StringView t) : bool`, `String::ends_with`, `String::trim(ref<String, shared> s) : StringView`, `String::trim_start`, `String::trim_end`, `String::split(ref<String, shared> s, str sep) : Vec<StringView>`, `String::chars(ref<String, shared> s) : Vec<StringView>` (D-0128), `String::contains(ref<String, shared> s, StringView t) : bool`, `String::replace(ref<String, shared> s, StringView from, StringView to) : String` (D-0139) | ordinary exported functions of `std` (D-0113): `as_view(&s)` is `&s[0..$]` (`[View-Form]`), and each other is the `StringView::` function of the same name applied to `&s[0..$]`; a result that is a view borrows `s` as a view does (`replace`'s `String` is new and borrows nothing) |
| `String::join(slice<String, shared> parts, StringView sep) : String`, `StringView::join(slice<StringView, shared> parts, StringView sep) : String` | ordinary exported functions of `std` (D-0139): `[Join]` (§2h), the texts of `parts` with `sep` between each two, in a new `String` |
| `String::eq(ref<String, shared> a, ref<String, shared> b) : bool`, `String::eq_str(ref<String, shared> a, str t) : bool`, `String::eq_view(ref<String, shared> a, StringView v) : bool` | ordinary exported functions of `std`: the texts' `[View-Eq]` (§2h) (D-0073); `==` between a `String` and a `str` or a view is `[Text-Mixed-Eq]` (D-0164) |
| `StringView::less(StringView a, StringView b) : bool`, `String::less(ref<String, shared> a, ref<String, shared> b) : bool` | ordinary exported functions of `std`: byte order (for UTF-8, code point order), a prefix first; `String::less` fits `Vec::sort_by` (D-0076) |
| `String::truncate(ref<String, exclusive> s, usize n)` | ordinary exported function of `std`: keeps the first `n` bytes; `diag.index-out-of-bounds` for `n` beyond the length, `diag.not-char-boundary` inside a character (D-0076) |
| `String::append<T>(ref<String, exclusive> s, T x)` for a printable `T` | exported function of `std`, realized natively: `rule.stdlib.text` `[Append]` (§2d) |
| `String::parse<T>(ref<String, shared> s) : Result<T, ParseError>` for an integer or float `T` | exported function of `std`, realized natively: `rule.stdlib.text` `[Parse]` (§2d) |
| `ascii_is_digit(u8 b) : bool`, `ascii_is_alpha`, `ascii_is_alnum`, `ascii_is_upper`, `ascii_is_lower`, `ascii_is_space`; `ascii_to_lower(u8 b) : u8`, `ascii_to_upper`; `String::to_ascii_lower(ref<String, shared> s) : String`, `String::to_ascii_upper` | exported functions of `std` (D-0101): ASCII only — a byte outside ASCII is none of the classes and is left unchanged, so a `String` stays valid UTF-8; `ascii_is_space` is space, tab, line feed, vertical tab, form feed and carriage return |

**`std::collections`: hash tables and queues**

| Signature | Rule |
|---|---|
| `HashMap<K, V>`, `HashSet<K>` and their functions, for a key type `K` | exported types of `std`, written in CobaltC over the std-only intrinsics `key_hash` and `key_eq`: `rule.stdlib.hashmap` (§2g); `remove` keeps the other entries' order, `swap_remove` moves the last entry into the hole (D-0134); `HashSet::key_at` and `HashSet::clear` as `HashMap`'s |
| `HashMap::get_str<V>(ref<HashMap<String, V>, shared> m, str k) : Option<ref<V, shared>>`, `HashMap::get_mut_str<V>(ref<HashMap<String, V>, exclusive> m, str k) : Option<ref<V, exclusive>>`, `HashMap::contains_str<V>(…, str k) : bool`, `HashMap::remove_str<V>(ref<HashMap<String, V>, exclusive> m, str k) : Option<V>`, `HashSet::contains_str(ref<HashSet<String>, shared> s, str k) : bool`, `HashSet::remove_str(ref<HashSet<String>, exclusive> s, str k) : bool` | ordinary exported functions of `std` (D-0113), for a map or set keyed by `String` only: each is the lookup of the same name with a `String` key of `k`'s bytes, without making one (`[Lookup-Str]`, §2g); `remove_str` keeps order as `remove` does |
| `HashSet::union<K: clone>(ref<HashSet<K>, shared> a, ref<HashSet<K>, shared> b) : HashSet<K>`, `HashSet::intersection<K: clone>(…)`, `HashSet::difference<K: clone>(…)` | ordinary exported functions of `std` (D-0138), written in CobaltC: `[Set-Ops]` (§2g) — a new set of copies (by `clone`) of the keys in `a` or `b`, in both, or in `a` and not `b`; `a` and `b` unchanged |
| `Queue<T>` and its functions (`Queue::new`, `len`, `push_back`, `pop_front`, `push_front`, `pop_back`, `front`, `back`, `clear`); `PriorityQueue<T>` and its functions (`PriorityQueue::new`, `new_by`, `len`, `push`, `pop`, `peek`, `clear`) | exported types of `std` (D-0137), written in CobaltC: `rule.stdlib.queue`, `rule.stdlib.priority-queue` (§2i) — a queue taken from at either end in constant time; a queue that gives the least element first, by the key types' order (`new`, as `Vec::sort`) or the caller's `less` (`new_by`, as `sort_by`), equal elements first in, first out |

**`std::memory`, `std::core`, `std::sync`: `Box`, `Rc`, `Weak`, values behind references, channels**

| Signature | Rule |
|---|---|
| `Box::clone<T: clone>(ref<Box<T>, shared> b) : Box<T>`, `HashMap::clone<K: clone, V: clone>(ref<HashMap<K, V>, shared> m) : HashMap<K, V>`, `HashSet::clone<K: clone>(ref<HashSet<K>, shared> s) : HashSet<K>` | ordinary exported functions of `std` (D-0116): a `Box` of a copy of the value; a map or set of copies of the keys and values, in the same insertion order (`String::clone` and `Rc::clone`, which share, complete the `std` types that are `clone`; `Option<T>` and `Result<T, E>` are `clone` by derivation, `rule.agg.derived-clone`) |
| `Weak<T>`, `Rc::downgrade<T>(ref<Rc<T>, shared> r) : Weak<T>`, `Weak::upgrade<T>(ref<Weak<T>, shared> w) : Option<Rc<T>>`, `Weak::clone<T>(ref<Weak<T>, shared> w) : Weak<T>` | exported type and functions of `std` (D-0125), written in CobaltC: `rule.stdlib.rc` — a handle that does not keep the value alive; `upgrade` gives a new strong handle while any exists, else `None` |
| `overwrite<T>(ref<T, exclusive> r, T v)` | ordinary exported function of `std` (D-0080): `drop(replace(r, v))`, §3b |
| `Channel<T>` and its functions (`Channel::new`, `send`, `recv`, `close`) | exported items of `std`, written in CobaltC over `mutex` and one primitive private to `std`: `rule.conc.channel` (`spec/19` §3), `rule.stdlib.channel` (§3c) |

**`std::io`, `std::fs`, `std::env`: the standard streams, files, the program's environment**

| Signature | Rule |
|---|---|
| `File::create_new(StringView path) : Result<Option<File>, FileError>`, `File::flush(ref<File, exclusive> f) : Result<void, FileError>`, `File::position(ref<File, shared> f) : Result<u64, FileError>` | exported functions of `std` (D-0181): `[Create-New]`, `[Flush]`, `[Position]` (§2e) |
| `current_exe() : Result<String, FileError>`, `hostname() : Result<String, FileError>` | exported functions of `std` (D-0181) over the std-private `path_op`: `[Current-Exe]`, `[Hostname]` (§2e′) |
| `Args` and its functions (`Args::new`, `from`, `flag`, `option`, `options`, `finish`, `help`) | exported items of `std` (D-0184), written in CobaltC: `rule.stdlib.args` `[Args-Flag]`, `[Args-Option]`, `[Args-Finish]`, `[Args-Help]` (§2c) |
| `printf(f, a1, …, an)`, `String::appendf(ref<String, exclusive> s, f, a1, …, an)`, `sprintf(f, a1, …, an) : String`, `eprintf(f, a1, …, an)` — `f` a string literal, any number of arguments | exported functions of `std`, realized natively: `rule.stdlib.format` `[Printf]`, `[Appendf]`, `[Sprintf]`, `[Eprintf]` (§2f) |
| `print<T>(T x)` for a printable `T` | function of `std`, not exported (D-0039), realized natively: `rule.stdlib.print` (§2a) |
| `assert(c)`, `assert(c, f, a1, …, an)` — `c` a `bool`, `f` a string literal, as `printf`'s | exported function of `std`, realized natively: `rule.fail.assert` (`spec/18` §1a) |
| `export unsafe extern fn stdout_write(rawptr<u8> buf, usize len) : isize;` | `rule.trust.extern-call` `[Extern-Call]` (`spec/20`); an instance declared in `std`, no dedicated rule |
| `flush_stdout()` | ordinary exported function of `std` (D-0178), written in CobaltC over a primitive private to `std`: `rule.stdlib.print` `[Print-Buffer]` — standard output's buffer written out |
| `export unsafe extern fn stdin_read(rawptr<u8> buf, usize len) : isize;` | `rule.stdlib.read` `[Read]`, an instance of `[Extern-Call]` declared in `std` |
| `read_line() : Result<Option<String>, FileError>` | ordinary exported function of `std`, written in CobaltC: `rule.stdlib.read` |
| `read_all() : Result<String, FileError>`, `read_all_bytes() : Result<Vec<u8>, FileError>` | ordinary exported functions of `std` (D-0145), written in CobaltC over `stdin_read`: `rule.stdlib.read` `[Read-All]` — the rest of standard input at once |
| `export unsafe extern fn arg_bytes(usize i, rawptr<u8> buf, usize len) : isize;` | `rule.stdlib.args` `[Arg-Bytes]`, an instance of `[Extern-Call]` declared in `std` |
| `arg_count() : usize`, `arg(usize i) : Result<String, Utf8Error>` | ordinary exported functions of `std`, written in CobaltC: `rule.stdlib.args` |
| `read_file(StringView path) : Result<String, FileError>`, `write_file(StringView path, StringView text) : Result<void, FileError>` | exported functions of `std`, realized natively: `rule.stdlib.file` `[Read-File]`, `[Write-File]` (§2e) |
| `File` and its functions (`File::open`, `create`, `append`, `open_rw`, `read`, `read_to_end`, `read_line`, `write`, `write_text`, `printf`, `seek`, `len`, `close`); `read_bytes(StringView path) : Result<Vec<u8>, FileError>`, `write_bytes(StringView path, slice<u8, shared> data) : Result<void, FileError>`; every path a `StringView` (D-0134) | exported items of `std`, written in CobaltC over two primitives private to `std`: `rule.stdlib.file-handle` (§2e) |
| `make_dir`, `remove_file`, `remove_dir`, `rename`, `list_dir`, `path_kind` (each path a `StringView`; all `Result<…, FileError>`); `env_var(StringView name) : Result<Option<String>, Utf8Error>`, `current_dir() : Result<String, FileError>`, `monotonic_ns() : u64`, `unix_seconds() : i64` | exported functions of `std`, written in CobaltC over three primitives private to `std`: `rule.stdlib.fs`, `rule.stdlib.env` (§2e′) |
| `path_join(StringView base, StringView tail) : String`, `path_parent(StringView p) : Option<String>`, `path_file_name`, `path_stem`, `path_extension` (each `: Option<String>`), `path_is_absolute(StringView p) : bool`, `path_normalize(StringView p) : String`, `path_canonical(StringView p) : Result<String, FileError>`; `FileInfo` (`kind : PathKind`, `len : u64`, `modified_unix : i64`, `readonly : bool`), `file_info(StringView p) : Result<FileInfo, FileError>`, `copy_file(StringView from, StringView to) : Result<u64, FileError>`, `make_dir_all(StringView p) : Result<void, FileError>`, `remove_dir_all`, `set_current_dir` (the same), `temp_dir() : String`, `home_dir() : Option<String>` | exported functions of `std` (D-0141), written in CobaltC over the std-private `path_op`, realized by the platform's path library: `[Path-Join]`, `[Path-Parts]`, `[Path-Normalize]`, `[Path-Canonical]`, `[File-Info]`, `[Copy-File]`, `[Dir-All]`, `[Set-Current-Dir]`, `[Temp-Dir]`, `[Home-Dir]` (§2e′); the path grammar is the platform's, and the part functions and `path_normalize` never touch the file system |
| `sleep_ms(u64 ms)` | ordinary exported function of `std` (D-0124) over the std-private `unsafe extern fn sleep_ns(u64) : i64`: the calling thread does nothing for at least `ms` milliseconds (`[Sleep]`: how much longer is implementation-defined; other threads run meanwhile; a thread holding a guard keeps it; no fault) |

**`std::encoding`, `std::random`: bytes, checksums, random numbers** (the numeric helpers `min`, `max`, `abs`, `pow`: `std::math`, below)

| Signature | Rule |
|---|---|
| `Rng::shuffle<T>(ref<Rng, exclusive> r, slice<T, exclusive> s)`, `uuid_v4() : String` | exported functions of `std` (D-0181): `[Rng-Shuffle]` — Fisher-Yates over `below`: position `i`, from the last down to 1, exchanged by `slice_swap` with a position drawn below `i + 1`, so one seed gives one order in every implementation; `[Uuid-V4]` — a version-4 UUID (RFC 9562 §5.4) of 16 bytes from `[Os-Random]`, bits set to version 4 and variant 10, as 36 lowercase characters |
| `to_hex(slice<u8, shared> bytes) : String`, `from_hex(StringView text) : Result<Vec<u8>, ParseError>`, `base64_encode(slice<u8, shared> bytes) : String`, `base64_decode(StringView text) : Result<Vec<u8>, ParseError>` | ordinary exported functions of `std` (D-0151), in `std::encoding::hex` and `std::encoding::base64` (D-0198): `[Hex]`, `[Base64]` (§2d); lowercase hexadecimal two digits a byte, read in either case; RFC 4648 §4 base64 with `=` padding, required when read; `Err(Invalid(i))` at the first byte that does not fit, or at the length when the count is wrong |
| `crc32(slice<u8, shared> data) : u32` | ordinary exported function of `std` (D-0119): CRC-32/IEEE, `[CRC32]` below; `crc32` of the empty slice is 0, of the bytes of `"123456789"` is `0xCBF43926` |
| `os_random_bytes(slice<u8, exclusive> out)`, `os_random_u64() : u64`, `Rng::from_os() : Rng` | exported functions of `std` (D-0142), written in CobaltC over the std-private `unsafe extern fn os_random(rawptr<u8> buf, usize n) : isize`: `[Os-Random]`, `[Rng-From-Os]` below — bytes from the operating system's cryptographically secure source (suitable for keys, tokens and nonces), and an `Rng` seeded from them; `diag.entropy-unavailable` when the system has none |
| `Rng`, `Rng::new(u64 s) : Rng`, `Rng::next_u64(ref<Rng, exclusive> r) : u64`, `Rng::below(ref<Rng, exclusive> r, u64 n) : u64`, `Rng::unit_f64(ref<Rng, exclusive> r) : f64` | exported struct and functions of `std` (D-0120), written in CobaltC: a general-purpose pseudo-random generator, `[Rng]` below — the same seed gives the same sequence in every implementation; `below` is unbiased and `n = 0` is `diag.div-by-zero`; `unit_f64` is in `[0, 1)`. Not suitable for cryptographic purposes: its state is recoverable from its output, so it must not produce keys, tokens or nonces |

**`std::time`: dates and times** (D-0140)

| Signature | Rule |
|---|---|
| `iso_from_unix_ms(i64 ms) : String`, `unix_ms_from_iso(StringView text) : Result<i64, ParseError>`, `DateTime::to_http_date(ref<DateTime, shared> t) : String`, `DateTime::from_http_date(StringView text) : Result<DateTime, ParseError>` | exported functions of `std` (D-0181): `[Iso-Ms]`, `[Http-Date]` (§2j); `[From-Iso]` reads a seconds fraction |
| `DateTime` (fields `year : i64`, `month`, `day`, `hour`, `minute`, `second : u8`, all exported), `Weekday` (`Monday` … `Sunday`) | exported types of `std`: `rule.stdlib.time` (§2j) |
| `DateTime::from_unix(i64 s) : DateTime`, `DateTime::now() : DateTime`, `DateTime::new(i64 year, u8 month, u8 day, u8 hour, u8 minute, u8 second) : Option<DateTime>`, `DateTime::to_unix(ref<DateTime, shared> t) : i64`, `DateTime::weekday(ref<DateTime, shared> t) : Weekday`, `Weekday::text(ref<Weekday, shared> w) : str`, `DateTime::to_iso(ref<DateTime, shared> t) : String`, `DateTime::from_iso(StringView text) : Result<DateTime, ParseError>`, `DateTime::eq(ref<DateTime, shared> a, ref<DateTime, shared> b) : bool`, `DateTime::less(…) : bool`, `unix_ms() : i64`, `DateTime::to_local(ref<DateTime, shared> t) : DateTime`, `local_offset_seconds(i64 unix) : i64` | exported functions of `std`, written in CobaltC (the offset realized natively, over the platform's zone rule): `[From-Unix]`, `[To-Unix]`, `[Invalid-DateTime]` (`diag.invalid-datetime`), `[Weekday]`, `[To-Iso]`, `[From-Iso]`, `[Unix-Ms]`, `[Local-Offset]`, `[To-Local]` (§2j); UTC, and the machine's local time by `to_local` (D-0147) |

**`std::process`: other programs and interrupts** (D-0143)

| Signature | Rule |
|---|---|
| `process_id() : u64`, `Child::terminate(ref<Child, exclusive> ch) : Result<void, FileError>`, `Child::read_error_line(ref<Child, exclusive> ch) : Result<Option<String>, FileError>` | exported functions of `std` (D-0181): `[Process-Id]`, `[Child-Terminate]`, `[Child-Streams]` (§2k) |
| `Child::kill_tree(ref<Child, exclusive> ch) : Result<void, FileError>` | exported function of `std::process` (D-0205): `[Child-Kill-Tree]` — the child is killed, then every process that descends from it as the system records parents at that moment (Linux, from `/proc`); on Windows `taskkill /T /F` ends the tree; elsewhere the child alone, as `Child::kill`. A child already waited for is unchanged |
| `ChildInput`, `ChildOutput` (resources), `Child::take_input(ref<Child, exclusive>) : Option<ChildInput>`, `Child::take_output(ref<Child, exclusive>) : Option<ChildOutput>`, `ChildInput::write(ref<ChildInput, exclusive>, slice<u8, shared>) : Result<void, FileError>`, `ChildOutput::read(ref<ChildOutput, exclusive>, ref<Vec<u8>, exclusive>, usize) : Result<usize, FileError>`, `ChildOutput::read_line(ref<ChildOutput, exclusive>) : Result<Option<String>, FileError>` | exported `std` (D-0195): `[Child-Pipes]` (§2k) |
| `try_join<R>(handle<R> h) : Result<R, ThreadFailure>`, `is_finished<R>(ref<handle<R>, shared> h) : bool`; `ThreadFailure` and `ThreadFailure::text`, `diagnostic` (each `: String`), `is_cancelled` (`: bool`) | exported items of `std::sync` (D-0204), written in CobaltC over `join_status` and the std-private `task_op`: `[Try-Join]`, `[Is-Finished]`, `[Failure-Text]` (§3c′) |
| `slice_parts<T>(slice<T, exclusive> s, usize parts) : Vec<slice<T, exclusive>>` | exported function of `std::collections` (D-0204): `[Slice-Parts]` (§3c′) |
| `cpu_count() : usize` | exported function of `std::sync` (D-0185), over `proc_op`: `[Cpu-Count]` — how many threads the machine runs at once (its processors, or what the program may use of them), at least 1 |
| `Command` (private fields), `Command::new(StringView program) : Command`, `Command::arg(ref<Command, exclusive> c, StringView a)`, `Command::current_dir(ref<Command, exclusive> c, StringView dir)`, `Command::env(ref<Command, exclusive> c, StringView name, StringView value)`, `Command::clear_env(ref<Command, exclusive> c)` | exported items of `std`, written in CobaltC: `[Command]` (§2k) — no shell; arguments pass as given |
| `Output` (`status : i32`, `stdout : Vec<u8>`, `stderr : Vec<u8>`), `Command::output(ref<Command, shared> c) : Result<Output, FileError>`, `Command::output_with_input(ref<Command, shared> c, slice<u8, shared> input) : Result<Output, FileError>`, `Command::status(ref<Command, shared> c) : Result<i32, FileError>` | `[Output]`, `[Status]` (§2k): a status is the exit code, or minus the signal |
| `Child` (a `resource struct`), `Command::spawn(ref<Command, shared> c) : Result<Child, FileError>`, `Child::id`, `write_input`, `close_input`, `read_output`, `read_output_line`, `read_error`, `wait`, `try_wait`, `kill` | `[Spawn-Child]`, `[Child-Streams]`, `[Wait]`, `[Kill]` (§2k); a dropped `Child` keeps running; every failure a `FileError` (D-0134), `NotFound` for a program that is not there |
| `watch_interrupts()`, `interrupt_requested() : bool` | `[Interrupt-Flag]` (§2k): an interrupt becomes a flag the program polls |
| `exit(u8 status) : never` | `[Exit]` (§2k, D-0150), realized natively: the calling thread unwinds and the program ends with `status` (`rule.fn.program` `[Terminate-Exit]`) |

**`std::net`: TCP, UDP and name resolution** (D-0144)

| Signature | Rule |
|---|---|
| `TcpStream::try_clone(ref<TcpStream, shared> s) : Result<TcpStream, NetError>`, `TcpStream::set_keepalive(ref<TcpStream, exclusive> s, bool on)`, `UdpSocket::connect(ref<UdpSocket, exclusive> u, SocketAddr to) : Result<void, NetError>`, `UdpSocket::send(ref<UdpSocket, exclusive> u, slice<u8, shared> data) : Result<usize, NetError>`, `UdpSocket::recv(ref<UdpSocket, exclusive> u, usize max) : Result<Vec<u8>, NetError>` | exported functions of `std` (D-0185, D-0181): `[Try-Clone]`, `[Keepalive]`, `[Udp-Connect]` (§2l) |
| `IpAddr` (`V4(array<u8, 4>)`, `V6(array<u8, 16>)`), `SocketAddr` (`ip : IpAddr`, `port : u16`), `SocketAddr::new(IpAddr ip, u16 port) : SocketAddr`, `SocketAddr::parse(StringView text) : Result<SocketAddr, ParseError>`, `SocketAddr::text(ref<SocketAddr, shared> a) : String`, `IpAddr::parse(StringView text) : Result<IpAddr, ParseError>`, `IpAddr::text(ref<IpAddr, shared> ip) : String`, `IpAddr::localhost_v4() : IpAddr`, `unspecified_v4`, `localhost_v6`, `unspecified_v6`; `resolve(StringView host, u16 port) : Result<Vec<SocketAddr>, NetError>` | exported items of `std`, written in CobaltC: `[Addr]`, `[Resolve]` (§2l) |
| `TcpListener` (a `resource struct`), `TcpListener::bind(SocketAddr a) : Result<TcpListener, NetError>`, `local_addr`, `set_accept_timeout_ms(ref<TcpListener, exclusive> l, u64 ms)`, `accept(ref<TcpListener, exclusive> l) : Result<TcpStream, NetError>` | `[Bind]`, `[Accept]` (§2l) |
| `TcpStream` (a `resource struct`), `TcpStream::connect(SocketAddr a)`, `connect_timeout(SocketAddr a, u64 ms)` (each `: Result<TcpStream, NetError>`), `peer_addr`, `local_addr`, `set_read_timeout_ms`, `set_write_timeout_ms`, `set_nodelay`, `read(ref<TcpStream, exclusive> s, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, NetError>`, `read_exact(…, usize n) : Result<void, NetError>`, `read_line(ref<TcpStream, exclusive> s) : Result<Option<String>, NetError>`, `write(ref<TcpStream, exclusive> s, slice<u8, shared> data) : Result<void, NetError>`, `write_text(…, StringView t)`, `shutdown_write` | `[Connect]`, `[Stream-Read]`, `[Stream-Write]`, `[Stream-Timeout]`, `[Shutdown]` (§2l); a `File`'s reading and writing |
| `TcpStream::printf(ref<TcpStream, exclusive> s, f, a1, …, an) : Result<void, NetError>` — `f` a string literal | exported function of `std`, realized natively as `File::printf` is: `rule.stdlib.format`'s text, written by `[Stream-Write]` |
| `UdpSocket` (a `resource struct`), `UdpSocket::bind`, `local_addr`, `set_read_timeout_ms`, `send_to(ref<UdpSocket, exclusive> u, slice<u8, shared> data, SocketAddr to) : Result<usize, NetError>`, `recv_from(ref<UdpSocket, exclusive> u, usize max) : Result<Datagram, NetError>`; `Datagram` (`data : Vec<u8>`, `from : SocketAddr`) | `[Udp]` (§2l) |
| `UdpSocket::try_clone(ref<UdpSocket, shared> u) : Result<UdpSocket, NetError>` | exported function of `std::net` (D-0205): `[Udp-Try-Clone]` — a second handle on the same socket (the system's descriptor duplicated), as `TcpStream::try_clone` gives one; each handle is used and ended on its own, the socket closed when the last one ends |
| `NetError` (`Refused`, `Reset`, `TimedOut`, `Unreachable`, `AddrInUse`, `AddrNotAvailable`, `UnknownHost`, `NotPermitted`, `Closed`, `Failed`, `NotUtf8(Utf8Error)`), `NetError::text(ref<NetError, shared> e) : str` | `[Net-Error]` (§2l), `text` as D-0117's; no variant shares a name with `FileError`'s, so both may be written unqualified |

**`std::crypto`: hashes and MACs** (D-0151)

| Signature | Rule |
|---|---|
| `Sha1` (a plain struct), `Sha1::new`, `update`, `finish : array<u8, 20>`, `sha1(slice<u8, shared> data) : array<u8, 20>`; `HashKind::Sha1` | exported items of `std` (D-0181), written in CobaltC: `[Sha1]` (§2m), FIPS 180-4 — legacy, for the protocols that require it (RFC 6238, git), not for new signatures |
| `Sha256` (a plain struct), `Sha256::new() : Sha256`, `Sha256::update(ref<Sha256, exclusive> s, slice<u8, shared> data)`, `Sha256::finish(Sha256 s) : array<u8, 32>`, `sha256(slice<u8, shared> data) : array<u8, 32>` | exported items of `std`, written in CobaltC: `[Sha256]` (§2m), FIPS 180-4 |
| `HmacSha256` (a plain struct), `HmacSha256::new(slice<u8, shared> key) : HmacSha256`, `HmacSha256::update(ref<HmacSha256, exclusive> m, slice<u8, shared> data)`, `HmacSha256::finish(HmacSha256 m) : array<u8, 32>`, `hmac_sha256(slice<u8, shared> key, slice<u8, shared> data) : array<u8, 32>` | exported items of `std`, written in CobaltC: `[Hmac-Sha256]` (§2m), RFC 2104 over SHA-256 |
| `digest_eq(slice<u8, shared> a, slice<u8, shared> b) : bool` | exported function of `std`, written in CobaltC: `[Digest-Eq]` (§2m); equal lengths and bytes, in a time that depends only on the lengths |
| `hkdf_extract(slice<u8, shared> salt, slice<u8, shared> ikm) : array<u8, 32>`, `hkdf_expand(slice<u8, shared> prk, slice<u8, shared> info, usize len) : Vec<u8>`, `hkdf_sha256(slice<u8, shared> salt, slice<u8, shared> ikm, slice<u8, shared> info, usize len) : Vec<u8>` | exported functions of `std`, written in CobaltC: `[Hkdf]` (§2m), RFC 5869 over HMAC-SHA-256; `len` above 8160 is `diag.hkdf-length` (D-0152) |
| `chacha20(slice<u8, shared> key, slice<u8, shared> nonce, u32 counter, slice<u8, shared> data) : Vec<u8>`, `poly1305(slice<u8, shared> key, slice<u8, shared> message) : array<u8, 16>` | exported functions of `std`, written in CobaltC: `[ChaCha20]`, `[Poly1305]`, `[Crypto-Length]`, `[Constant-Time]` (§2m, D-0153) |
| `chacha20_poly1305_seal(slice<u8, shared> key, slice<u8, shared> nonce, slice<u8, shared> aad, slice<u8, shared> plaintext) : Vec<u8>`, `chacha20_poly1305_open(slice<u8, shared> key, slice<u8, shared> nonce, slice<u8, shared> aad, slice<u8, shared> sealed) : Option<Vec<u8>>` | exported functions of `std`, written in CobaltC: `[Aead-Seal]`, `[Aead-Open]` (§2m, D-0153), RFC 8439's AEAD |
| `x25519(slice<u8, shared> private_key, slice<u8, shared> public_key) : Option<array<u8, 32>>`, `x25519_public_key(slice<u8, shared> private_key) : array<u8, 32>`, `x25519_private_key() : array<u8, 32>` | exported functions of `std`, written in CobaltC: `[X25519]`, `[Constant-Time]` (§2m, D-0154), RFC 7748 |
| `aes_encrypt_block(slice<u8, shared> key, array<u8, 16> block) : array<u8, 16>`, `aes_gcm_seal(slice<u8, shared> key, slice<u8, shared> nonce, slice<u8, shared> aad, slice<u8, shared> plaintext) : Vec<u8>`, `aes_gcm_open(…, slice<u8, shared> sealed) : Option<Vec<u8>>` | exported functions of `std`, written in CobaltC: `[Aes]`, `[Aes-Gcm-Seal]`, `[Aes-Gcm-Open]`, `[Crypto-Length]`, `[Constant-Time]` (§2m, D-0165), FIPS 197 and SP 800-38D |
| `p256_private_key() : array<u8, 32>`, `p256_public_key(slice<u8, shared> private_key) : Option<array<u8, 65>>`, `p256_ecdh(slice<u8, shared> private_key, slice<u8, shared> public_key) : Option<array<u8, 32>>` | exported functions of `std`, written in CobaltC: `[P256-Ecdh]`, `[Constant-Time]` (§2m, D-0165) |
| `Sha512`, `Sha384` (plain structs) with `new`, `update`, `finish`; `sha512(slice<u8, shared> data) : array<u8, 64>`, `sha384(slice<u8, shared> data) : array<u8, 48>` | exported items of `std`, written in CobaltC: `[Sha512]` (§2m, D-0156) |
| `HashKind` (`Sha2_256`, `Sha2_384`, `Sha2_512`), `hash(HashKind kind, slice<u8, shared> data) : Vec<u8>`; `RsaPublicKey` (`n`, `e : BigUint`), `rsa_verify_pkcs1v15(ref<RsaPublicKey, shared> key, HashKind kind, slice<u8, shared> digest, slice<u8, shared> signature) : bool`, `rsa_verify_pss(…) : bool` | exported items of `std`, written in CobaltC: `[Hash]`, `[Rsa-Pkcs1v15]`, `[Rsa-Pss]`, `[Public-Verify]` (§2m, D-0158) |
| `EcCurve` (`P256`, `P384`), `ecdsa_verify(EcCurve curve, slice<u8, shared> public_key, slice<u8, shared> digest, slice<u8, shared> signature) : bool` | exported items of `std`, written in CobaltC: `[Ecdsa]`, `[Public-Verify]` (§2m, D-0159) |
| `hkdf_expand_label(slice<u8, shared> secret, str label, slice<u8, shared> context, usize len) : Vec<u8>` | exported function of `std`, written in CobaltC: `[Hkdf-Expand-Label]` (§2m, D-0161) |
| `Hasher` (a plain struct), `Hasher::new(HashKind kind) : Hasher`, `update`, `finish(Hasher h) : Vec<u8>`; `Hmac` (a plain struct), `Hmac::new(HashKind kind, slice<u8, shared> key) : Hmac`, `update`, `finish(Hmac m) : Vec<u8>`, `hmac(HashKind kind, slice<u8, shared> key, slice<u8, shared> data) : Vec<u8>` | exported items of `std`, written in CobaltC: `[Hasher]`, `[Hmac]` (§2m, D-0166) |
| `hkdf_extract_with(HashKind kind, slice<u8, shared> salt, slice<u8, shared> ikm) : Vec<u8>`, `hkdf_expand_with(HashKind kind, slice<u8, shared> prk, slice<u8, shared> info, usize len) : Vec<u8>`, `hkdf_with(HashKind kind, …, usize len) : Vec<u8>`, `hkdf_expand_label_with(HashKind kind, slice<u8, shared> secret, str label, slice<u8, shared> context, usize len) : Vec<u8>` | exported functions of `std`, written in CobaltC: `[Hkdf-With]` (§2m, D-0166) |
| `ed25519_private_key() : array<u8, 32>`, `ed25519_public_key(slice<u8, shared> private_key) : array<u8, 32>`, `ed25519_sign(slice<u8, shared> private_key, slice<u8, shared> message) : array<u8, 64>`, `ed25519_verify(slice<u8, shared> public_key, slice<u8, shared> message, slice<u8, shared> signature) : bool` | exported functions of `std`, written in CobaltC: `[Ed25519-Sign]`, `[Ed25519-Verify]`, `[Crypto-Length]`, `[Constant-Time]` (§2m, D-0167), RFC 8032 |
| `ec_private_key(EcCurve curve) : Vec<u8>`, `ec_public_key(EcCurve curve, slice<u8, shared> private_key) : Option<Vec<u8>>`, `ecdh(EcCurve curve, slice<u8, shared> private_key, slice<u8, shared> public_key) : Option<Vec<u8>>`, `ecdsa_sign(EcCurve curve, slice<u8, shared> private_key, slice<u8, shared> digest) : Option<Vec<u8>>` | exported functions of `std`, written in CobaltC: `[Ec-Keys]`, `[Ecdh]`, `[Ecdsa-Sign]`, `[Crypto-Length]`, `[Constant-Time]` (§2m, D-0168), RFC 6979 |
| `RsaPrivateKey` (`n`, `e`, `d`, `p`, `q : BigUint` exported), `RsaPrivateKey::new(BigUint n, BigUint e, BigUint d, BigUint p, BigUint q) : Option<RsaPrivateKey>`, `RsaPrivateKey::public_key(ref<RsaPrivateKey, shared> key) : RsaPublicKey`, `rsa_sign_pkcs1v15(ref<RsaPrivateKey, shared> key, HashKind kind, slice<u8, shared> digest) : Vec<u8>`, `rsa_sign_pss(…) : Vec<u8>`, `rsa_generate_key(usize bits) : RsaPrivateKey` | exported items of `std`, written in CobaltC: `[Rsa-Private-Key]`, `[Rsa-Sign-Pkcs1v15]`, `[Rsa-Sign-Pss]`, `[Rsa-Generate]`, `[Rsa-Private-Op]`, `[Crypto-Length]` (§2m, D-0169) |
| `Blake2b` (a plain struct), `Blake2b::new(usize out_len) : Blake2b`, `Blake2b::new_keyed(slice<u8, shared> key, usize out_len) : Blake2b`, `update`, `finish(Blake2b s) : Vec<u8>`, `blake2b(slice<u8, shared> data, usize out_len) : Vec<u8>`; `pbkdf2(HashKind kind, slice<u8, shared> password, slice<u8, shared> salt, usize iterations, usize len) : Vec<u8>`; `argon2id(slice<u8, shared> password, slice<u8, shared> salt, usize iterations, usize memory_kib, usize parallelism, usize len) : Vec<u8>`; `hash_password(slice<u8, shared> password) : String`, `verify_password(StringView stored, slice<u8, shared> password) : bool` | exported items of `std`, written in CobaltC: `[Blake2b]`, `[Pbkdf2]`, `[Argon2id]`, `[Password-Hash]`, `[Kdf-Parameters]`, `[Crypto-Length]` (§2m, D-0170) |

**`std::x509` and `std::tls`: certificates and TLS** (D-0160, D-0161)

| Signature | Rule |
|---|---|
| `TlsStream::server(TcpStream tcp, ref<PrivateKey, shared> key, slice<Certificate, shared> chain) : Result<TlsStream, TlsError>` | exported function of `std` (D-0186), written in CobaltC: `[Tls-Server]` (§2o) |
| `TlsStream::read_line(ref<TlsStream, exclusive> s) : Result<Option<String>, TlsError>`, `TlsStream::peer_certificate(ref<TlsStream, shared> s) : Option<ref<Certificate, shared>>`, `TlsStream::peer_addr`, `TlsStream::local_addr` | exported functions of `std` (D-0181): `[Tls-Read-Line]`, `[Tls-Peer-Certificate]` (§2o) |
| `Param` (`name`, `value`), `Param::new(StringView name, StringView value) : Param`, `find_param(slice<Param, shared> params, str name) : Option<StringView>`, `form_decode(StringView text) : Vec<Param>`, `form_encode(slice<Param, shared> params) : String`, `Url::query_pairs(ref<Url, shared> u) : Vec<Param>` | exported items of `std` (D-0181): `[Form]` (§2p) |
| `HttpServer::bind_tls(SocketAddr a, PrivateKey key, Vec<Certificate> chain) : Result<HttpServer, NetError>`; `HttpServer::accept(ref<HttpServer, exclusive> s) : Result<HttpConnection, HttpError>` | exported functions of `std` (D-0186): `[Https-Bind]`, `[Http-Accept]` (§2p); the client's `[Content-Encoding]` (D-0183) |
| `PgRow::get_i64(ref<PgRow, shared> r, usize i) : Option<i64>`, `get_f64 : Option<f64>`, `get_bool : Option<bool>`, `get_bytes : Option<Vec<u8>>` | exported functions of `std` (D-0181): `[Pg-Cell]` (§2q) |
| `Der` (`tag`, `start`, `end`, `next`), `der_read(slice<u8, shared> buf, usize at) : Option<Der>` | `[Der-Read]` (§2n) |
| `Certificate` (fields exported), `Certificate::parse(slice<u8, shared> der) : Result<Certificate, CertError>`, `Certificate::matches_host(ref<Certificate, shared> c, StringView host) : bool`, `certificates_from_pem(StringView text) : Vec<Certificate>`; `CertKey` (`RsaKey`, `EcKey(EcPublicKey)`, `Ed25519Key(array<u8, 32>)`, `UnsupportedKey`), `SignatureScheme` (`RsaPkcs1`, `RsaPss`, `Ecdsa`, `Ed25519`, `UnsupportedSignature`), `verify_signature(…) : bool` | `[Cert-Parse]`, `[Host-Match]` (§2n) |
| `CertKey::from_der(slice<u8, shared> der) : Result<CertKey, CertError>`, `CertKey::from_pem(StringView text) : Result<CertKey, CertError>`, `CertKey::to_der(ref<CertKey, shared> key) : Option<Vec<u8>>`, `CertKey::to_pem(…) : Option<String>`; `EcPrivateKey` (`curve`, `scalar`), `PrivateKey` (`RsaPrivate(RsaPrivateKey)`, `EcPrivate(EcPrivateKey)`, `Ed25519Private(array<u8, 32>)`), `PrivateKey::from_der`, `from_pem`, `to_der(ref<PrivateKey, shared> key) : Vec<u8>`, `to_pem(…) : String`, `public_key(…) : CertKey`; `sign(ref<PrivateKey, shared> key, ref<SignatureScheme, shared> scheme, slice<u8, shared> message) : Option<Vec<u8>>` | `[Key-Der]`, `[Key-Pem]`, `[Sign]` (§2n, D-0171) |
| `TrustStore` (`roots`), `TrustStore::new`, `add`, `from_pem_file(StringView path) : Result<TrustStore, FileError>`, `system() : Result<TrustStore, FileError>`; `verify_chain(ref<Certificate, shared> leaf, slice<Certificate, shared> intermediates, ref<TrustStore, shared> roots, StringView host, ref<DateTime, shared> now) : Result<void, CertError>`; `CertError` (`BadEncoding`, `Expired`, `NotYetValid`, `UnknownAuthority`, `HostMismatch`, `BadSignature`, `NotAuthority`, `UnsupportedAlgorithm`, `UnhandledCritical`, `WrongUsage`, `ChainTooLong`) and `CertError::text` | `[Trust-Store]`, `[Verify-Chain]` (§2n) |
| `TlsStream` (a `resource struct`), `TlsStream::client(TcpStream tcp, StringView server_name, ref<TrustStore, shared> roots) : Result<TlsStream, TlsError>`, `connect(SocketAddr addr, …)`, `read`, `read_exact`, `write`, `write_text`, `close`; `TlsError` (`NetFailure(NetError)`, `CertRejected(CertError)`, `HandshakeFailed`, `ProtocolError`, `PeerAlert(u8)`, `RecordRejected`) and `TlsError::text` | `[Tls-Client]`, `[Tls-Records]`, `[Tls-Error]` (§2o) |
| `Url` (`scheme`, `host`, `port`, `path`, `query`), `Url::parse(StringView text) : Result<Url, HttpError>`, `Url::target(ref<Url, shared> u) : String`, `Url::text(…) : String`, `percent_encode(slice<u8, shared> bytes) : String`, `percent_decode(StringView text) : Option<Vec<u8>>`; `Header` (`name`, `value`), `Header::new(str name, StringView value) : Header`, `find_header(slice<Header, shared> headers, str name) : Option<StringView>`; `Request` (`method`, `url`, `headers`, `body`), `Request::new(str method, StringView url) : Request`, `Request::header(…) : Option<StringView>`; `Response` (`status`, `reason`, `headers`, `body`), `Response::header(…)`, `Response::text(ref<Response, shared> r) : Result<String, Utf8Error>` (the body's bytes copied, as `String::from_utf8` takes them, D-0193); `reason_phrase(u16 status) : str` | `[Url-Parse]`, `[Percent]`, `[Header-Find]` (§2p, D-0173) |
| `HttpClient` (`roots : Option<TrustStore>`, `timeout_ms`, `max_redirects`, `max_body`, `headers`, `keep_alive : bool`, D-0195), `HttpClient::new() : HttpClient`, `HttpClient::send(ref<HttpClient, exclusive> c, ref<Request, shared> req) : Result<Response, HttpError>`, `http_get(StringView url) : Result<Response, HttpError>`, `http_post(StringView url, str content_type, slice<u8, shared> body) : Result<Response, HttpError>`; `HttpError` (`NetFailure(NetError)`, `TlsFailure(TlsError)`, `BadUrl`, `UnsupportedScheme`, `NoRoots`, `BadMessage`, `TooLarge`, `TooManyRedirects`, `ConnectionClosed`), `HttpError::text` | `[Http-Send]`, `[Http-Body]`, `[Http-Redirect]`, `[Http-Error]` (§2p, D-0173) |
| `HttpServer` (a `resource struct`; `max_body`, `timeout_ms`), `HttpServer::bind(SocketAddr a) : Result<HttpServer, NetError>`, `local_addr`, `HttpServer::accept(ref<HttpServer, exclusive> s) : Result<HttpConnection, NetError>`; `HttpConnection` (a `resource struct`), `HttpConnection::request(ref<HttpConnection, exclusive> c) : Result<Option<Request>, HttpError>`, `HttpConnection::respond(ref<HttpConnection, exclusive> c, u16 status, slice<Header, shared> headers, slice<u8, shared> body) : Result<void, HttpError>`, `peer_addr`, `close` | `[Http-Accept]`, `[Http-Request]`, `[Http-Respond]` (§2p, D-0174) |
| `PgUrl` (`user`, `password : Option<String>`, `host`, `port`, `database`, `sslmode : PgSslMode`), `PgUrl::parse(StringView text) : Result<PgUrl, PgError>`; `PgSslMode` (`Disable`, `VerifyFull`); `PgValue` (`Null`, `Text(String)`, `Int(i64)`, `Float(f64)`, `Bool(bool)`, `Bytes(Vec<u8>)`), `PgValue::text(ref<PgValue, shared> v) : Option<String>`; `PgColumn` (`name`, `type_oid`), `PgRow` (`cells : Vec<Option<String>>`), `PgRow::get(ref<PgRow, shared> r, usize i) : Option<StringView>`, `PgResult` (`columns`, `rows`, `affected`, `tag`), `PgResult::column(ref<PgResult, shared> res, str name) : Option<usize>` | `[Pg-Url]`, `[Pg-Query]`, `[Pg-Result]` (§2q, D-0180) |
| `PgConnection` (a `resource struct`; `notices : Vec<PgServerError>`, `max_result`), `PgConnection::connect(StringView url) : Result<PgConnection, PgError>`, `PgConnection::connect_with(StringView url, ref<TrustStore, shared> roots, u64 timeout_ms) : Result<PgConnection, PgError>`, `PgConnection::parameter(ref<PgConnection, shared> c, str name) : Option<StringView>`, `PgConnection::query(ref<PgConnection, exclusive> c, StringView sql, slice<PgValue, shared> params) : Result<PgResult, PgError>`, `PgConnection::execute(ref<PgConnection, exclusive> c, StringView sql, slice<PgValue, shared> params) : Result<u64, PgError>`, `PgConnection::run_script(ref<PgConnection, exclusive> c, StringView sql) : Result<void, PgError>`, `close`; `PgError` (`NetFailure(NetError)`, `TlsFailure(TlsError)`, `TlsRefused`, `NoTrustedRoots`, `BadConnectionUrl`, `UnsupportedAuth`, `AuthRefused`, `AuthFailed`, `Server(PgServerError)`, `ProtocolViolation`, `ResultTooLarge`, `Disconnected`), `PgError::text(ref<PgError, shared> e) : String`, `PgServerError` (`severity`, `code`, `message`, `detail`, `hint`, `position`); `Scram`, `Scram::start(StringView user, StringView client_nonce) : Scram`, `Scram::client_first(ref<Scram, shared> s) : String`, `Scram::respond(ref<Scram, exclusive> s, StringView server_first, StringView password) : Result<String, PgError>`, `Scram::verify(ref<Scram, shared> s, StringView server_final) : bool`, `scram_salted_password(StringView password, slice<u8, shared> salt, usize iterations) : Vec<u8>` | `[Pg-Connect]`, `[Pg-Auth]`, `[Pg-Query]`, `[Pg-Execute]`, `[Pg-Script]`, `[Pg-Notice]`, `[Pg-Error]` (§2q, D-0180) |

**`std::encoding::json`: JSON** (D-0182, D-0197)

| Signature | Rule |
|---|---|
| `Json` (`JsonNull`, `JsonBool(bool)`, `JsonNumber(f64)`, `JsonText(String)`, `JsonArray(Vec<Json>)`, `JsonObject(Vec<JsonMember>)`), `JsonMember` (`key : String`, `value : Json`) | exported types of `std`: `rule.stdlib.json` (§2r) |
| `Json::parse(StringView text) : Result<Json, ParseError>`, `Json::text(ref<Json, shared> j) : String`, `Json::pretty(ref<Json, shared> j) : String`, `Json::get(ref<Json, shared> j, StringView key) : Option<ref<Json, shared>>`, `Json::at(ref<Json, shared> j, usize i) : Option<ref<Json, shared>>`, `Json::is_null`, `as_bool : Option<bool>`, `as_f64 : Option<f64>`, `as_i64 : Option<i64>`, `as_text : Option<StringView>`, `as_array : Option<ref<Vec<Json>, shared>>`, `as_object : Option<ref<Vec<JsonMember>, shared>>`, `Json::set(ref<Json, exclusive> j, StringView key, Json v)`, `Json::push(ref<Json, exclusive> j, Json v)` | exported functions of `std`, written in CobaltC: `[Json-Parse]`, `[Json-Text]`, `[Json-Pretty]`, `[Json-Get]`, `[Json-Set]`, `[Json-Error]` (§2r) |

**`std::compress`: DEFLATE, zlib, gzip** (D-0183)

| Signature | Rule |
|---|---|
| `deflate(slice<u8, shared> data) : Vec<u8>`, `inflate(slice<u8, shared> data, usize max) : Result<Vec<u8>, CompressError>`, `zlib_deflate`, `zlib_inflate`, `gzip`, `gunzip` (the same signatures), `adler32(slice<u8, shared> data) : u32`; `CompressError` (`Corrupt(usize)`, `Truncated`, `Oversize`), `CompressError::text(ref<CompressError, shared> e) : str` | exported items of `std`, written in CobaltC: `[Deflate]`, `[Inflate]`, `[Zlib]`, `[Gzip]`, `[Adler32]`, `[Compress-Error]` (§2s) |

**`std::math`: numbers** (the four generic helpers moved here from `std::core` by D-0148; the floating-point functions by D-0136, before it language intrinsics)

| Signature | Rule |
|---|---|
| `clamp<T: ordered>(T x, T lo, T hi) : T`, `is_nan<T: number>(T x) : bool`, `is_finite<T: number>(T x) : bool`, `gcd<T: integer>(T a, T b) : T` | ordinary exported functions of `std` (D-0181): `[Clamp]` — `lo` when `x < lo`, `hi` when `hi < x`, else `x`; `lo` above `hi` is `diag.assert-failed`. `[Is-Nan]` — `x != x`. `[Is-Finite]` — `(x - x) == (x - x)`: true for every integer and every finite float, false for a NaN or an infinity. `[Gcd]` — Euclid over the magnitudes (`abs`), never negative, `gcd(0, 0)` 0 |
| `asin`, `acos`, `atan` | `[Float-Transcendental]` (§3d, D-0181): the platform's, within 1 ulp |
| `BigUint` and `BigDivision` (`quotient`, `remainder`), `BigUint::zero`, `from_u64`, `from_bytes_be`, `to_bytes_be`, `to_bytes_be_padded`, `is_zero`, `bit_len`, `bit`, `eq`, `less`, `add`, `sub`, `mul`, `div_rem`, `rem`, `mod_pow`, `shift_left`, `shift_right`, `text`, `parse`, `clone`, `mod_inverse(ref<BigUint, shared> a, ref<BigUint, shared> m) : Option<BigUint>` | exported items of `std`, written in CobaltC: `[BigUint]`, `[Big-Division]`, `[Big-Mod-Inverse]`, `[Big-Not-Constant-Time]` (§3e, D-0157, D-0169) |
| `min<T: ordered>(T a, T b) : T`, `max<T: ordered>(T a, T b) : T`, `abs<T: number>(T x) : T`, `pow<T: number>(T base, u32 exp) : T` | exported functions of `std`, written in CobaltC over D-0090's bounds (D-0091): `min`/`max` give `a` when the two are equal; `abs` and `pow` are checked (`diag.arith-overflow` for `abs` of a signed type's minimum, and for a power outside `T`), `abs(-0.0)` is `0.0`, `pow(x, 0)` is `1` |
| `sqrt(T x) : T`, `floor`, `ceil`, `round`, `trunc` (`T` `f32` or `f64`) | exported functions of `std`, realized natively: `rule.arith.float-fns` (§3d, D-0091) |
| `ln(T x) : T`, `exp`, `log2`, `log10`, `sin`, `cos`, `tan`; `atan2(T y, T x) : T`, `powf(T x, T y) : T` (`T` `f32` or `f64`) | exported functions of `std`, realized natively: `rule.arith.float-fns` `[Float-Transcendental]` (§3d, D-0101): the platform's, within 1 ulp |

    [Allocate]   disposition: fallible   outcome: unspecified { any fresh aligned range, or Err }
        n, align : usize
        ────────────────────────────────────────────
        ⟨allocate(n, align), Σ⟩ → ⟨Ok(p), Σ[ storage(x) := uninit for x ∈ [p, p+n) ]⟩   with [p, p+n) ∩ dom(Σ.storage) = ∅,
                                                                                          p mod align = 0
                                or ⟨Err(AllocError{}), Σ⟩

    [Deallocate]   disposition: trusted-unchecked
        ────────────────────────────────────────────
        ⟨deallocate(p, n, align), Σ⟩ → ⟨(), Σ' ⟩   where Σ' = release(p, n) then storage := Σ.storage \ [p, p+n)
        side-conditions: ⟦ (p, n, align) were returned by one allocate not yet deallocated ⟧ discharge: trusted

An allocation may fail (`Err`); succeeding is not guaranteed for any
size. `allocate(0, _)` returns `Ok(dangling)` without extending
storage.

**Depends on:** rule.arith.alt, rule.arith.convert, rule.arith.sizeof,
rule.resauth.destroy, rule.trust.rawptr, rule.conc.lock,
rule.conc.spawn, rule.conc.join, rule.agg.enum-construct,
rule.trust.extern-call
**Affects:** state.storage

## 1. `Vec<T>`

In `std::collections`.

### `rule.stdlib.vec`
**Status:** ACCEPTED

    export resource struct Vec<T>
    {
        rawptr<T> ptr;
        usize len;
        usize cap;
    }

    export fn Vec::new<T>() : Vec<T>
    {
        Vec { .ptr = dangling<T>(), .len = 0, .cap = 0 }
    }

    export fn Vec::len<T>(ref<Vec<T>, shared> v) : usize
    {
        v.len
    }

    export fn Vec::push<T>(ref<Vec<T>, exclusive> v, T x)
    {
        if (v.len == v.cap)
        {
            Vec::grow(v);
        }
        unsafe
        {
            *(v.ptr + reinterpret<isize>(v.len)) = x;      // [Rawptr-Write] or [Rawptr-Move-In]
        }
        v.len = v.len + 1;
    }

    export fn Vec::pop<T>(ref<Vec<T>, exclusive> v) : Option<T>
    {
        if (v.len == 0)
        {
            return None;
        }
        v.len = v.len - 1;
        unsafe
        {
            rawptr<T> p = v.ptr + reinterpret<isize>(v.len);
            auto x = *p;                                          // [Rawptr-Read] or [Rawptr-Move-Out]
            release(reinterpret_ptr<u8>(p), sizeof<T>());          // ends a reclaimed object left at this slot
                                                                     // (a no-op for a resource T: [Rawptr-Move-Out]
                                                                     // above already ended it) — see [Release]
            Some(x)
        }
    }

    export fn Vec::index_shared<T>(ref<Vec<T>, shared> v, usize i) : ref<T, shared>
    {
        if (i >= v.len)
        {
            fault(index_out_of_bounds);                    // see below
        }
        unsafe
        {
            &reclaim<T>(v.ptr + reinterpret<isize>(i))
        }
    }

    export fn Vec::index_exclusive<T>(ref<Vec<T>, exclusive> v, usize i) : ref<T, exclusive>
    {
        if (i >= v.len)
        {
            fault(index_out_of_bounds);
        }
        unsafe
        {
            &mut reclaim<T>(v.ptr + reinterpret<isize>(i))
        }
    }

    fn Vec::grow<T>(ref<Vec<T>, exclusive> v)
    {
        usize new_cap = if (v.cap == 0)
        {
            4
        }
        else
        {
            v.cap * 2
        };
        usize bytes = new_cap * sizeof<T>();
        rawptr<u8> p = match (allocate(bytes, alignof<T>()))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure)
        };
        unsafe
        {
            copy_raw(p, reinterpret_ptr<u8>(v.ptr), v.len * sizeof<T>());   // reclaimed element objects are
                                                                             // released by the deallocate below and
                                                                             // re-attached lazily by reclaim
            if (v.cap > 0)
            {
                deallocate(reinterpret_ptr<u8>(v.ptr), v.cap * sizeof<T>(), alignof<T>());
            }
        }
        v.ptr = reinterpret_ptr<T>(p);
        v.cap = new_cap;
    }

    // Room for n more elements (D-0129).
    export fn Vec::reserve<T>(ref<Vec<T>, exclusive> v, usize n)
    {
        usize need = v.len + n;                          // overflowing: diag.arith-overflow
        if (need <= v.cap)
        {
            return;
        }
        usize new_cap = if (v.cap > need - v.cap)
        {
            v.cap * 2
        }
        else
        {
            need
        };
        usize bytes = new_cap * sizeof<T>();
        rawptr<u8> p = match (allocate(bytes, alignof<T>()))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure)
        };
        unsafe
        {
            copy_raw(p, reinterpret_ptr<u8>(v.ptr), v.len * sizeof<T>());
            if (v.cap > 0)
            {
                deallocate(reinterpret_ptr<u8>(v.ptr), v.cap * sizeof<T>(), alignof<T>());
            }
        }
        v.ptr = reinterpret_ptr<T>(p);
        v.cap = new_cap;
    }

    export fn Vec::drop<T>(ref<Vec<T>, exclusive> self)
    {
        usize mut_i = 0;
        while (mut_i < self.len)
        {
            unsafe
            {
                drop(reclaim<T>(self.ptr + reinterpret<isize>(mut_i)));   // destroys element i if T is a
                                                                           // resource; ends the reclaimed object
            }
            mut_i = mut_i + 1;
        }
        if (self.cap > 0)
        {
            unsafe
            {
                deallocate(reinterpret_ptr<u8>(self.ptr), self.cap * sizeof<T>(), alignof<T>());
            }
        }
    }

`Vec::reserve(&mut v, n)` (D-0129) makes room for `n` more elements:
afterwards `cap >= len + n`. When the room is already there it changes
nothing; otherwise the elements move once, as `grow` moves them, to
cells for the larger of `len + n` and twice the capacity (doubling, as
`push` grows, so that small reserves in a loop stay amortized). It never
shrinks a `Vec` and never changes its length or elements; `len + n`
overflowing is `diag.arith-overflow`, before anything is allocated. A
reference into the old cells is stale afterwards, as after a `push`
that grows. No function reports a capacity, so `reserve` changes how
fast a program runs, not what it computes. `std`'s own builders
(`String::from_str`, `String::clone`, `Vec::from_slice`) reserve their
final length before filling.

`fault(d)` raises a checked fault with the named diagnostic
(`diag.index-out-of-bounds`, `diag.alloc-failure`), as `std` does here
and as a program implementing its own containers may (D-0083):
`[Fault]`: `⟨fault(d), Σ⟩ ↛ diag.d`, `disposition: checked`, typed `never`. `reinterpret_ptr<U>(rawptr<T> p) : rawptr<U>` is the
identity on addresses (an intrinsic, safe). `Vec` is declared
`resource` (its fields carry no authority; the allocation does), with
destructor `Vec::drop` invoked by `rule.resauth.destroy`. Element
references come from `reclaim`: a reclaimed element object persists
while the buffer does, so two references to element `i` are two
paths on one object and `clash` governs them; a reallocation
(`grow`) releases every reclaimed element, so a reference held across
a `push` that reallocates is caught as `diag.stale-binding` at its
next use (dynamic — `rule.control.flow-analysis` marks the referent
`escaped`). `push` of a resource moves it into raw storage as a
reclaimed object (`[Rawptr-Move-In]`); `pop` moves it back out
(`[Rawptr-Move-Out]`, which ends the reclaimed object as part of the
move) and, for a *plain* `T` — where the read that recovers the value
leaves the reclaimed object alone, correctly, since reading owns
nothing — additionally `release`s the popped slot explicitly, so a
reference into a popped-and-reused index is caught the same way as one
across a reallocation: `diag.stale-binding` at its next use, dynamic,
never a silent read of whatever a later `push` happens to write there
(`spec/AUDIT-STATUS.md` finding F-05, closed by `CHG-0019`). `drop`
destroys each element by reclaiming and destroying it (`[Destroy]` for
a resource `T`, `[Destroy-Plain]` otherwise — both end the reclaimed
object).


D-0181 adds two helpers over `Vec::swap` and `Vec::truncate`:

    [Retain]   (D-0181)   ⟨Vec::retain(v, keep), Σ⟩: keep(&v[i]) is called for i = 0, 1, … in order, before any element
               is destroyed; the elements it accepted move down by Vec::swap, keeping their order, and the rest
               (now at the end) are destroyed by Vec::truncate, last first
    [Dedup]    (D-0181)   ⟨Vec::dedup(v), Σ⟩ (T eq): every element equal ([Cmp-*], in place for String) to the element
               kept just before it is removed, the first of each run kept, the removed ones destroyed as [Retain]'s

    // D-0181: keeps the elements `keep` says to, in their order, and
    // destroys the rest. `keep` is called once per element, in index order,
    // before any is destroyed; the kept elements move down by `Vec::swap`, so
    // none is copied.
    export fn Vec::retain<T>(ref<Vec<T>, exclusive> v, fn(ref<T, shared>) : bool keep)
    {
        usize w = 0;
        for (usize i = 0; i < v.len; i += 1)
        {
            if (keep(Vec::index_shared(v, i)))
            {
                if (w != i)
                {
                    Vec::swap(v, w, i);
                }
                w += 1;
            }
        }
        Vec::truncate(v, w);
    }

    // D-0181: removes every element equal to the one before it, keeping the
    // first of each run (`[Cmp-*]`, in place for `String`): a sorted `Vec`
    // is left with one of each value. The removed elements are destroyed.
    export fn Vec::dedup<T: eq>(ref<Vec<T>, exclusive> v)
    {
        if (v.len < 2)
        {
            return;
        }
        usize w = 1;
        for (usize i = 1; i < v.len; i += 1)
        {
            if (!(*Vec::index_shared(v, i) == *Vec::index_shared(v, w - 1)))
            {
                if (w != i)
                {
                    Vec::swap(v, w, i);
                }
                w += 1;
            }
        }
        Vec::truncate(v, w);
    }

**Depends on:** rule.trust.rawptr, rule.stdlib.prelude,
rule.resauth.destroy, rule.type.is-resource, inv.spatial-validity,
D-0003, D-0008

## 1b. Sorting and searching

In `std::collections`.

### `rule.stdlib.sort`
**Status:** ACCEPTED

A `Vec` is sorted by a comparison, `less(a, b)` meaning that `a` goes
before `b`, or in its elements' own order when they are of a key type
(D-0062). Sorting is stable: equal elements (neither before the other)
keep their order. A search needs a `Vec` sorted by the same order.

    [Sort-By]         ⟨Vec::sort_by(v, less), Σ⟩ → the elements of v, each moved once, in an order
                      where no element is less than one before it, equal ones as they were;
                      `less` is called on references to elements (v is borrowed exclusively)
    [Sort]            ⟨Vec::sort(v), Σ⟩ is ⟨Vec::sort_by(v, key_before<T>), Σ⟩: integers by value,
                      `false` before `true`, `str` and `String` by their bytes (a prefix first);
                      a struct field by field in declaration order, an enum by the order its
                      variants are declared in, then by payload (D-0110)
    [Binary-Search]   ⟨Vec::binary_search_by(v, key, less), Σ⟩ → Ok(i), i the first position whose
                      element is equal to key, else Err(i), i the position key would take;
                      `binary_search(v, key)` in the key types' order
    [Sort-Not-Key]    disposition: rejected   `Vec::sort` or `Vec::binary_search` with T not a key
                      type (`[Key-Not-Hashable]`'s types)   ill-formed; diag.type-mismatch

`Vec::sort` compares in place, with the std-only intrinsic
`key_less_at(v, i, j)` (`key_less` of elements `i` and `j` of the `Vec`
`v` refers to, read while `v` is borrowed shared for the whole sort),
rather than through a reference to each element; the order, and every
other effect, is `[Sort]`'s.

`less` must be a strict weak order (never `less(a, a)`; transitive);
with one that is not, the elements still end as a permutation of what
they were, in an order this rule does not fix. An implementation may
realize `Vec::sort` of an integer or `bool` element type natively,
provided the elements end as `[Sort]` leaves them and the same checks
on `v` are made (`cobc` does, spec/21 §0's latitude).

The `std` source (`key_less` is an intrinsic private to `std`, the key
types' order):

    // Sorting and searching (`rule.stdlib.sort`, spec/21 §1b, D-0062). The
    // order `Vec::sort` and `Vec::binary_search` use: `key_less`, the key
    // types' own (integers by value, `false` before `true`, text by its
    // bytes), as a `fn` value.
    fn key_before<T>(ref<T, shared> a, ref<T, shared> b) : bool
    {
        key_less(a, b)
    }

    // The positions 0..n of `v` in the order `less` gives, equal elements in
    // their original order: a bottom-up merge sort of positions, which reads
    // the elements only through shared references.
    fn Vec::sorted_order<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>, ref<T, shared>) : bool less) : Vec<usize>
    {
        usize n = v.len;
        Vec<usize> a = Vec::new();
        Vec<usize> b = Vec::new();
        usize i = 0;
        while (i < n)
        {
            Vec::push(&mut a, i);
            Vec::push(&mut b, 0);
            i += 1;
        }
        usize width = 1;
        while (width < n)
        {
            usize lo = 0;
            while (lo < n)
            {
                usize mid = if (n - lo > width)
                {
                    lo + width
                }
                else
                {
                    n
                };
                usize hi = if (n - mid > width)
                {
                    mid + width
                }
                else
                {
                    n
                };
                usize l = lo;
                usize r = mid;
                usize k = lo;
                while (k < hi)
                {
                    // From the left run unless the right one's element is
                    // strictly less: equal elements keep their order.
                    bool take_left = l < mid && (r >= hi || !less(Vec::index_shared(v, a[r]), Vec::index_shared(v, a[l])));
                    if (take_left)
                    {
                        b[k] = a[l];
                        l += 1;
                    }
                    else
                    {
                        b[k] = a[r];
                        r += 1;
                    }
                    k += 1;
                }
                lo = hi;
            }
            swap(&mut a, &mut b);
            width = if (width > n)
            {
                n
            }
            else
            {
                width * 2
            };
        }
        a
    }

    // `sorted_order` in the key types' own order, comparing elements in
    // place (`key_less_at`) rather than through a reference to each.
    fn Vec::sorted_order_keys<T>(ref<Vec<T>, shared> v) : Vec<usize>
    {
        usize n = v.len;
        Vec<usize> a = Vec::new();
        Vec<usize> b = Vec::new();
        usize i = 0;
        while (i < n)
        {
            Vec::push(&mut a, i);
            Vec::push(&mut b, 0);
            i += 1;
        }
        usize width = 1;
        while (width < n)
        {
            usize lo = 0;
            while (lo < n)
            {
                usize mid = if (n - lo > width)
                {
                    lo + width
                }
                else
                {
                    n
                };
                usize hi = if (n - mid > width)
                {
                    mid + width
                }
                else
                {
                    n
                };
                usize l = lo;
                usize r = mid;
                usize k = lo;
                while (k < hi)
                {
                    bool take_left = l < mid && (r >= hi || !key_less_at(v, a[r], a[l]));
                    if (take_left)
                    {
                        b[k] = a[l];
                        l += 1;
                    }
                    else
                    {
                        b[k] = a[r];
                        r += 1;
                    }
                    k += 1;
                }
                lo = hi;
            }
            swap(&mut a, &mut b);
            width = if (width > n)
            {
                n
            }
            else
            {
                width * 2
            };
        }
        a
    }

    // Moves every element of `v` once, into the order `order` gives.
    fn Vec::apply_order<T>(ref<Vec<T>, exclusive> v, ref<Vec<usize>, shared> order)
    {
        Vec<T> out = Vec::new();
        foreach (p in order)
        {
            Vec::push(&mut out, Vec::take_raw(v, *p));
        }
        v.len = 0;
        swap(v, &mut out);
    }

    // Sorts `v` by `less` (true when its first argument goes before its
    // second), keeping equal elements in their order.
    export fn Vec::sort_by<T>(ref<Vec<T>, exclusive> v, fn(ref<T, shared>, ref<T, shared>) : bool less)
    {
        if (v.len < 2)
        {
            return;
        }
        Vec<usize> order = Vec::sorted_order(v, less);
        Vec::apply_order(v, &order);
    }

    // Sorts `v` in its elements' own order; `T` a key type (integers, `bool`,
    // `str`, `String`).
    export fn Vec::sort<T>(ref<Vec<T>, exclusive> v)
    {
        if (v.len < 2)
        {
            return;
        }
        Vec<usize> order = Vec::sorted_order_keys(v);
        Vec::apply_order(v, &order);
    }

    // In a `v` sorted by `less`: Ok(i), i the first position holding an
    // element equal to `key` (neither goes before the other), or Err(i), the
    // position where `key` would go.
    export fn Vec::binary_search_by<T>(
        ref<Vec<T>, shared> v,
        ref<T, shared> key,
        fn(ref<T, shared>, ref<T, shared>) : bool less,
    ) : Result<usize, usize>
    {
        usize lo = 0;
        usize hi = v.len;
        while (lo < hi)
        {
            usize mid = lo + (hi - lo) / 2;
            if (less(Vec::index_shared(v, mid), key))
            {
                lo = mid + 1;
            }
            else
            {
                hi = mid;
            }
        }
        if (lo < v.len && !less(key, Vec::index_shared(v, lo)))
        {
            return Ok(lo);
        }
        Err(lo)
    }

    // `binary_search_by` in the elements' own order; `T` a key type.
    export fn Vec::binary_search<T>(ref<Vec<T>, shared> v, ref<T, shared> key) : Result<usize, usize>
    {
        Vec::binary_search_by(v, key, key_before<T>)
    }


    [Sort-Float]   (D-0181)   T = f32 or f64 in Vec::sort, Vec::binary_search and PriorityQueue::new: key_less(a, b) is
                   a < b, or b a NaN and a not one — every NaN after every other value, NaNs equal to one another,
                   -0.0 and 0.0 equal (a strict weak order; the merge sort stays stable over it)

**Depends on:** rule.stdlib.vec, rule.stdlib.hashmap, rule.stdlib.swap, D-0062
**Affects:** state.storage (through `Vec::push`)

## 2. `String`

In `std::text`.

### `rule.stdlib.string`
**Status:** ACCEPTED

    export struct String
    {
        Vec<u8> bytes;                   // is-resource by derivation
    }

    // The offset of the first byte of `b` that is not well-formed UTF-8, or
    // its length when all of it is. `b` is the function's only reference,
    // read element by element: the compiler leaves those reads unchecked.
    fn utf8_error_at(slice<u8, shared> b) : usize
    {
        usize n = slice_len(b);
        usize i = 0;
        while (i < n)
        {
            u8 c0 = b[i];
            usize width =
                if (c0 < 128)
                {
                    1
                }
                else if ((c0 & 224) == 192 && c0 >= 194)
                {
                    2
                }
                else if ((c0 & 240) == 224)
                {
                    3
                }
                else if ((c0 & 248) == 240 && c0 <= 244)
                {
                    4
                }
                else
                {
                    return i;
                };
            if (i + width > n)
            {
                return i;
            }
            usize k = 1;
            while (k < width)
            {
                u8 c = b[i + k];
                if ((c & 192) != 128)
                {
                    return i + k;
                }
                k = k + 1;
            }
            if (width == 3)
            {
                u8 c1 = b[i + 1];
                if ((c0 == 224 && c1 < 160) || (c0 == 237 && c1 >= 160))
                {
                    return i;
                }
            }
            if (width == 4)
            {
                u8 c1 = b[i + 1];
                if ((c0 == 240 && c1 < 144) || (c0 == 244 && c1 >= 144))
                {
                    return i;
                }
            }
            i = i + width;
        }
        n
    }

    export fn String::from_utf8(Vec<u8> bytes) : Result<String, Utf8Error>
    {
        usize at = utf8_error_at(&bytes[0..$]);
        if (at < Vec::len(&bytes))
        {
            return Err(Utf8Error { .offset = at });
        }
        Ok(String { .bytes = bytes })
    }

    // D-0205: `bytes` viewed as text, after the check `String::from_utf8`
    // makes; nothing is copied.
    export fn StringView::from_utf8(slice<u8, shared> bytes) : Result<StringView, Utf8Error>
    {
        usize at = utf8_error_at(bytes);
        if (at < slice_len(bytes))
        {
            return Err(Utf8Error { .offset = at });
        }
        Ok(StringView { .bytes = bytes })
    }

    export fn String::len(ref<String, shared> s) : usize
    {
        Vec::len(&s.bytes)
    }

    export fn String::into_bytes(String s) : Vec<u8>
    {
        String { bytes } = s;   // rule.init.let [Let-Destructure]
        bytes
    }

    export fn String::from_str(str s) : String
    {
        Vec<u8> v = Vec::new();
        usize n = str_len(s);
        Vec::reserve(&mut v, n);
        usize i = 0;
        while (i < n)
        {
            Vec::push(&mut v, str_byte(s, i));
            i = i + 1;
        }
        String { .bytes = v }
    }

`from_utf8` is `[Trust-Transition]` instantiated: the byte vector is a
claim; the validator establishes `inv.string.utf8-validity` for the
resulting `String` (`spec/03`). `from_str` is the other constructor:
it copies the bytes of a `str` value, which are well-formed by
`inv.str.utf8-validity` (§2a), so the `String` invariant is
established without a validator — one invariant carried over into
another, not a trust transition. The only primitive that adds a
single byte, `String::push_ascii` (§2d), adds only ASCII, so the
invariant is preserved by construction. (`into_bytes` uses struct
destructuring, `spec/22` §2 — the one pattern form beyond `match` on
enums.)

**Depends on:** inv.string.utf8-validity, inv.str.utf8-validity,
inv.trust-transition, rule.stdlib.vec, rule.stdlib.str,
rule.trust.extern-call

## 2a. `str`

`str` is built in; its functions are intrinsics. `print` is private to `std`'s root.

### `type.str`
**Status:** ACCEPTED

`str` is the type of text values: its represented domain is the set
of finite byte sequences that are well-formed UTF-8. A `str` is a
value in the sense of `term.value` — an element of a domain,
independent of storage — exactly as an integer is: `is-resource(str)
= false` (`rule.type.is-resource`), so it is copied by `[Read]`,
never transferred, and carries no destroy obligation. It is not a
reference (`type.ref`): it has no target, no mode, no `held-by`, and
no temporal validity to check; the bytes it denotes are not cells of
any object in `Σ.objects`, and no access path reaches into them.
Equality is `[Eq-Str]` (`spec/12` §4) and order `[Cmp-Text]` (`spec/06`,
D-0108); arithmetic and logic are ill-typed on `str`. Its representation is `[Sizeof-Str]`
/`[Repr-Str]` (`spec/06` §7). `str ∉ FfiType` (`spec/20` §3).

The only way to write a `str` is a `str-literal` (`spec/22` §1); the
only operations on one are the three intrinsics below,
`String::from_str` (§2), and `StringView::of` (§2h, D-0134), a
read-only view of its bytes. There is no indexing syntax and no
slicing of a `str` itself.

**Depends on:** term.value, rule.type.is-resource, rule.type.eq,
rule.arith.sizeof, rule.arith.represent, inv.str.utf8-validity,
D-0006, D-0020

### `rule.stdlib.str`
**Status:** ACCEPTED

    [Str-Literal]
        L a str-literal (spec/22 §1) whose decoded bytes are b0..b_{n-1}
        ────────────────────────────────────────────
        Γ ⊢ L : str;   ⟨L, Σ⟩ → ⟨⟪b0..b_{n-1}⟫, Σ⟩
        side-conditions:
            ⟦ well-formed-utf8(b0..b_{n-1}) ⟧ discharge: static   -- by the grammar: str-char is a scalar value
                                                                    -- of UTF-8 source text, str-escape encodes one

    [Str-Literal-View]   (D-0134)
        L a str-literal, or a use of a constant whose value is one (rule.module.const),
        in a position whose declared type is StringView: an argument whose parameter is declared
        StringView, the initializer of a local declared StringView, a struct literal's field
        declared StringView
        ────────────────────────────────────────────
        L there  ≡  StringView::of(L)
        -- typing, not conversion: the literal has no type until its position gives it one, as an
        -- unsuffixed numeric literal has none (rule.arith.literal, D-0079). A str binding has type
        -- str and is not affected (StringView::of(t)); nor is a parameter of a type parameter.

    [Str-Literal-String]   (D-0149)
        L a str-literal, or a use of a constant whose value is one, in a position whose declared
        type is String: an argument whose parameter is declared String, the initializer of a local
        declared String, a struct literal's field declared String, or what a function declared
        `: String` gives (`return L;`, or L as its body's tail expression, through if and match arms)
        ────────────────────────────────────────────
        L there  ≡  String::from_str(L)
        -- D-0162: also an argument whose parameter mentions a type parameter, once the other
        -- arguments and the expected type have fixed that parameter to exactly String (as D-0079
        -- types a numeric literal argument): Result::unwrap_or(r, "default"), Vec::push(&mut v, "x")
        -- for a Vec<String>; where nothing fixes it the literal is a str; a literal a generic
        -- function's instantiations would type differently is diag.type-mismatch
        -- D-0155: in either rule, a position's literal may also be the value a block, an if branch
        -- or a match arm standing in that position gives (String s = match (o) { … : "x", … });
        -- each such tail is in the position too
        -- the same typing rule as [Str-Literal-View], for the one other text type a literal can
        -- name; it is the one place a literal allocates, and the declared type says String where
        -- the literal is written. Not with `auto` (the literal is a str), not for a str binding
        -- (String s = t; is diag.type-mismatch, D-0006), not for a parameter of a type parameter.

    [Str-Len]
        ⟨s, Σ⟩ →* ⟨⟪b0..b_{n-1}⟫, Σ'⟩
        ────────────────────────────────────────────
        ⟨str_len(s), Σ⟩ → ⟨n : usize, Σ'⟩

    [Str-Byte]   disposition: checked
        ⟨s, Σ⟩ →* ⟨⟪b0..b_{n-1}⟫, Σ1⟩    ⟨i, Σ1⟩ →* ⟨k : usize, Σ2⟩    k < n
        ────────────────────────────────────────────
        ⟨str_byte(s, i), Σ⟩ → ⟨b_k : u8, Σ2⟩

    [Str-Byte-Out-Of-Bounds]   disposition: checked
        k ≥ n
        ────────────────────────────────────────────
        ⟨str_byte(s, i), Σ⟩ ↛ diag.index-out-of-bounds

    [Str-Ptr]   outcome: impl-defined { any address }
        ⟨s, Σ⟩ →* ⟨⟪b0..b_{n-1}⟫, Σ'⟩
        ────────────────────────────────────────────
        ⟨str_ptr(s), Σ⟩ → ⟨p : rawptr<u8>, Σ'[ storage(p+j) := byte(b_j) for j < n ]⟩
        side-conditions: [p, p+n) ∩ (⋃ extent(o,Σ') for o ∈ Σ'.objects) = ∅

`⟪…⟫` is the `str` value whose bytes are listed (`spec/06` §7's
`Value` grammar). `str_len` and `str_byte` are total over the value
alone; `str_byte` is `checked` exactly as `[Index-Checked]` is, and
its literal-index case is refuted statically by
`rule.control.flow-analysis`'s value-range row the same way. `str_ptr`
is safe, like `[Rawptr-Of]`: it yields an address whose cells hold the
value's bytes, outside every live object's extent, so no access path
can be invalidated by reading them; what a program then does with the
address is governed by `rule.trust.rawptr` and `rule.trust.extern-call`
inside `unsafe`. Which address (and whether two evaluations of the same
bytes share one) is implementation-defined and must be documented.

**Depends on:** type.str, inv.str.utf8-validity, rule.stdlib.prelude,
rule.trust.rawptr, rule.agg.index, rule.control.flow-analysis, D-0020
**Affects:** state.storage

### `rule.stdlib.print`
**Status:** ACCEPTED

`print<T>(T x)` is a function of `std` that `std` does not export
(D-0039): a program writes output with `printf` (§2f), and `print` is
what `printf`, `%v` and `String::append` are defined through. It is
realized natively by both implementations (§0's latitude, as `map_err`
is), because its behaviour depends on `T` and a CobaltC body cannot
(D-0010, D-0028). Its printable types are `str`, every integer type,
`f32`, `f64`, `bool` and `ref<String, shared>`.

    [Print]
        T printable    ⟨x, Σ⟩ →* ⟨v, Σ1⟩    text(v) = b0..b_{n-1}
        ────────────────────────────────────────────
        Γ ⊢ print(x) : void;   ⟨print(x), Σ⟩ → ⟨(), Σ1⟩, writing b0..b_{n-1} to standard output
                               as [Extern-Call] of write(p, n) does

    [Print-Not-Printable]   disposition: rejected
        T not printable, or not one argument
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    text(⟪b0..b_{n-1}⟫ : str)            = b0..b_{n-1}
    text(v : ref<String, shared>)       = the bytes of the String v refers to
    text(v : τ integer)                 = [Print-Int]
    text(v : τ ∈ {f32, f64})            = [Print-Float]
    text(true), text(false)             = `true`, `false`

    [Print-Int]     the decimal digits of |v| with no leading zeros (`0` for zero), preceded by `-` if v < 0

    [Print-Float]
        NaN → `NaN`;   +∞ → `inf`;   −∞ → `-inf`;   otherwise `-` if v is negative (−0.0 included), then:
        let d1…dn and k be the shortest decimal digit string, and its exponent, with
            d1.d2…dn × 10^k read back (round to nearest, ties to even) as |v| in τ, d1 ≠ 0 unless v is zero;
            among equally short strings, the one nearest |v|, and on an exact tie the one whose dn is even;
        if −7 < k < 21:  positional, with at least one fraction digit:
            k ≥ 0:  d1…d_{k+1} (padded with 0s to k+1 digits) `.` d_{k+2}…dn (or `0` if none)
            k < 0:  `0.` followed by −k−1 zeros and d1…dn
        otherwise:       d1 `.` d2…dn (or `0` if n = 1) `e` k   (k in decimal, `-` if negative, no `+`)

For example `printf("%v", 0.1)` writes `0.1`, `printf("%v", 1.0)` `1.0`,
`printf("%v", 1.0 / 3.0)` `0.3333333333333333`, `printf("%v", 1.0e-7)` `1.0e-7`, and
`printf("%v", 0.1: f32)` `0.1` (the digits are chosen in `f32`, not in `f64`).
The shortest-digits rule is the one JavaScript's and Python's
number-to-text conversions use, so every printed float reads back as
exactly the value printed.

- **Where the type is checked.** A type parameter may be passed to
  `%v` (`rule.type.kind`); each instantiation is checked, and a `T`
  that is not printable is `[Print-Not-Printable]` there.
- **`String` by reference.** A `String` passed by value would be moved
  and destroyed; only `ref<String, shared>` is printable, so a program
  writes `printf("%v", &s)`.
- **`str` and `String`** are written through `stdout_write`, with the `unsafe`
  block inside `std`: a program that only prints never writes `unsafe`
  itself.
- **Failure.** `stdout_write`'s `isize` claim is discarded; a short or failed
  write is not observable (`feat.str-literal`; a fallible `printf` is a
  revisit condition of D-0020).
- **Buffering (D-0178).** What `print` writes to standard output, and
  what a program writes with `stdout_write`, reaches the operating
  system in pieces of the implementation's choosing, never later than:

      [Print-Buffer]
          the bytes written to standard output so far are written out before
            · any byte written to standard error (`eprintf`, a fault's report);
            · any read of standard input (`stdin_read`, so `read_line` and `read_all`);
            · a child that writes to the program's standard output starts (`Command::status`);
            · the program ends: `main` returns, `exit`, or a fault;
            · `flush_stdout()` returns;
            · when standard output is a terminal, the write that completes a line (byte 10) returns

  So within a program standard output and standard error keep the order
  the program wrote them in, a prompt is on the screen before the
  program waits for its answer, and a person at a terminal sees each
  line when it is complete. A program whose standard output is a pipe
  or a file, and which someone watches while it runs (progress, a
  log), calls `flush_stdout()` where the output must be visible.
  Output still buffered when the process is killed from outside (a
  signal, a debugger) is lost, as in C. Standard error is not buffered.
  Both implementations buffer up to 8 KiB, which the rule does not
  require. A program's own C code (`rule.trust.extern-code`) writing to
  standard output through C's `stdio` has a buffer of its own, and the
  two are not ordered; such a program calls `flush_stdout()` before
  handing over to its C code.
- **Not a program's name.** `print` is private to `std`
  (`spec/17`): in a program `print(x)` is `diag.unbound-name` and
  `std::print(x)` is `diag.name-not-visible`, and a program may
  declare its own `print`. Composite values (a `Vec`, an `Option`, a struct) are
  written by the program.

**Depends on:** type.str, rule.stdlib.string, rule.type.kind,
rule.trust.extern-call, rule.trust.unsafe, D-0020, D-0024, D-0028,
D-0039

## 2b. Standard input

In `std::io`.

### `rule.stdlib.read`
**Status:** ACCEPTED

    export unsafe extern fn stdin_read(rawptr<u8> buf, usize len) : isize;

    export fn read_line() : Result<Option<String>, FileError>
    {
        rawptr<u8> cell = match (allocate(1, 1))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure),
        };
        Vec<u8> bytes = Vec::new();
        isize n = 0;
        bool any = false;
        while (true)
        {
            n = unsafe
            {
                stdin_read(cell, 1)
            };
            if (n <= 0)
            {
                break;
            }
            any = true;
            u8 c = unsafe
            {
                *cell
            };
            if (c == 10)
            {
                if (bytes.len > 0 && bytes[bytes.len - 1] == 13)
                {
                    Vec::pop(&mut bytes);
                }
                break;
            }
            Vec::push(&mut bytes, c);
        }
        unsafe
        {
            deallocate(cell, 1, 1);
        }
        if (n < 0)
        {
            return Err(Io);
        }
        if (!any)
        {
            return Ok(None);
        }
        match (String::from_utf8(bytes))
        {
            Ok(s) : Ok(Some(s)),
            Err(e) : Err(Utf8(e)),
        }
    }

    [Read]   an instance of [Extern-Call] (spec/20 §3)   disposition: trusted-unchecked
        p : rawptr<u8>    n : usize    the program's standard input supplies its next k bytes b0..b_{k-1}, 0 ≤ k ≤ n
        ────────────────────────────────────────────
        ⟨stdin_read(p, n), Σ⟩ → ⟨claim : isize, Σ[ storage(p+j) := byte(b_j) for j < k ]⟩
            claim = k;  k = 0 only when n = 0 or the input has ended;  claim = −1, storing nothing, when
            the input cannot be read
        side-conditions: ⟦ [p, p+n) is storage outside every live object, as [Extern-Call] requires ⟧
                         discharge: trusted

`stdin_read` is `stdout_write`'s counterpart: an `extern` declared in `std`, called
inside `unsafe`, reading the bytes that come next on the program's
standard input. Its result is a claim like any extern's.

`read_line` is the safe way to read: it returns the next line as a
`String`, with no `unsafe` in the calling program. Its error is a
`FileError`, the one error of input and output (D-0134); of its
variants, only `Io` and `Utf8` occur for standard input.

- **A line** is every byte up to the next `\n` (byte 10), which is
  consumed and not included, or up to the end of input. A `\r` (byte
  13) immediately before that `\n` is consumed with it and not
  included (D-0092): a Windows line ending reads as a Unix one, on
  every platform. Any other `\r` is part of the line, and a last line
  ended by the end of input keeps a final `\r`, since no `\n` was
  consumed.
- **`Ok(None)`** means the input ended before the line's first byte.
  A last line with no `\n` is still `Ok(Some(…))`, and every call after
  the end is `Ok(None)` again.
- **`Err(Utf8(e))`** means the line is not UTF-8; `e.offset` is the bad
  byte's offset within the line. The line has been consumed, so the
  next call reads the line after it.
- **`Err(Io)`** means `stdin_read` reported that the input cannot be read.
  The bytes of that line already read are discarded.

**`read_all` and `read_all_bytes`** (D-0145) take the rest of standard
input at once, for a filter that wants all of it before it starts:

    [Read-All]   ⟨read_all_bytes(), Σ⟩ → Ok(b), b the bytes from here to the end of input (stdin_read
                 called until it gives 0), empty when the input has ended; Err(Io) when stdin_read
                 gives −1 (the bytes read are discarded). ⟨read_all(), Σ⟩ → Ok(s) for the same bytes
                 when they are UTF-8, Err(Utf8(e)) otherwise, e.offset the first bad byte's offset
                 among them. Either may follow read_line calls; the input is consumed by both alike

    // D-0145: the rest of standard input at once, as bytes; empty at its
    // end. Read in pieces of up to 64 KiB straight into the `Vec`'s room.
    export fn read_all_bytes() : Result<Vec<u8>, FileError>
    {
        Vec<u8> out = Vec::new();
        while (true)
        {
            while (out.cap - out.len < 65_536)
            {
                Vec::grow(&mut out);
            }
            isize n = unsafe
            {
                stdin_read(reinterpret_ptr<u8>(out.ptr + reinterpret<isize>(out.len)), out.cap - out.len)
            };
            if (n < 0)
            {
                return Err(FileError::Io);
            }
            if (n == 0)
            {
                break;
            }
            out.len = out.len + reinterpret<usize>(n);
        }
        Ok(out)
    }

    // D-0145: the rest of standard input at once, as text; `Err(Utf8(e))`
    // when it is not UTF-8.
    export fn read_all() : Result<String, FileError>
    {
        Vec<u8> b = read_all_bytes()?;
        match (String::from_utf8(b))
        {
            Ok(s)  : Ok(s),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

Standard input belongs to the program's environment, which is outside
this specification (Master Instructions §1). An implementation
documents whether it provides standard input. One that does not stops
the program at its first call of `stdin_read`, saying so; that stop is not a
diagnostic of this specification, and everything the program did
before it happened as usual. A program that never reads runs the same
either way.

**Depends on:** rule.trust.extern-call, rule.trust.unsafe,
rule.trust.rawptr, rule.stdlib.vec, rule.stdlib.string, D-0029
**Affects:** state.storage

## 2c. Program arguments

In `std::env` (D-0176).

### `rule.stdlib.args`
**Status:** ACCEPTED

    export unsafe extern fn arg_bytes(usize i, rawptr<u8> buf, usize len) : isize;

    export fn arg_count() : usize
    {
        usize i = 0;
        while (true)
        {
            isize n = unsafe
            {
                arg_bytes(i, dangling<u8>(), 0)
            };
            if (n < 0)
            {
                break;
            }
            i = i + 1;
        }
        i
    }

    export fn arg(usize i) : Result<String, Utf8Error>
    {
        isize n = unsafe
        {
            arg_bytes(i, dangling<u8>(), 0)
        };
        if (n < 0)
        {
            fault(index_out_of_bounds);
        }
        usize len = reinterpret<usize>(n);
        rawptr<u8> p = match (allocate(len, 1))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure),
        };
        Vec<u8> bytes = Vec::new();
        unsafe
        {
            arg_bytes(i, p, len);
        }
        usize j = 0;
        while (j < len)
        {
            u8 b = unsafe
            {
                *(p + reinterpret<isize>(j))
            };
            Vec::push(&mut bytes, b);
            j = j + 1;
        }
        if (len > 0)
        {
            unsafe
            {
                deallocate(p, len, 1);
            }
        }
        String::from_utf8(bytes)
    }

    [Arg-Bytes]   an instance of [Extern-Call] (spec/20 §3)   disposition: trusted-unchecked
        i : usize    p : rawptr<u8>    n : usize    the program's arguments are A_0 … A_{c−1}, byte sequences
        ────────────────────────────────────────────
        ⟨arg_bytes(i, p, n), Σ⟩ → ⟨claim : isize, Σ[ storage(p+j) := byte(A_i[j]) for j < min(n, |A_i|) ]⟩
            claim = |A_i| when i < c;  claim = −1, storing nothing, when i ≥ c
        side-conditions: ⟦ [p, p+n) is storage outside every live object, as [Extern-Call] requires ⟧
                         discharge: trusted

The program's **arguments** are a sequence of byte sequences its
environment gives it before `main` begins (`rule.fn.program`), and
they do not change while it runs. Argument 0 is the first argument
after the program itself: the program's own name is not one of them.
How an implementation is given them is implementation-defined and
documented; one with no way to receive arguments gives none (`c = 0`).

`arg_bytes` is an `extern` declared in `std`, called inside `unsafe`,
like `stdin_read`. It copies at most `len` bytes and always returns the
argument's whole length, so a call with `len = 0` asks only whether
argument `i` exists and how long it is. Its result is a claim like any
extern's.

`arg_count` and `arg` are the safe way to read arguments, with no
`unsafe` in the calling program.

- **`arg_count()`** is the number of arguments, `c`.
- **`arg(i)`** is argument `i` as a `String`, or `Err(e)` when it is
  not UTF-8, `e.offset` being the bad byte's offset within it. An
  argument that is not UTF-8 does not affect any other.
- **`arg(i)` with `i ≥ arg_count()`** faults `diag.index-out-of-bounds`
  (`[Fault]`), as `Vec::index_shared` does.

A program that needs an argument's bytes whatever they are calls
`arg_bytes` itself.


`Args` (D-0184) reads the arguments as options and positional
arguments, declared while they are read, so a program's option handling
and its help text come from the same calls:

    [Args-Flag]     (D-0184)   ⟨Args::flag(a, name, short, help), Σ⟩ → whether some item before `--` is "--" ‖ name
                    or "-" ‖ short (short one letter, or "" for none), every such item taken; the line
                    "-s, --name  help" added to [Args-Help]
    [Args-Option]   (D-0184)   ⟨Args::options(a, name, short, value_name, help), Σ⟩ → the values, in order, of every
                    untaken item "--name=value", or "--name" / "-s" followed by any item (the value, taken too),
                    before `--`; Args::option gives the last of them, or None. A "--name" or "-s" that is the
                    last item, or just before `--`, is left for [Args-Finish]
    [Args-Finish]   (D-0184)   ⟨Args::finish(a), Σ⟩ → Ok(the untaken items in order, every item after the first `--`
                    among them, `--` itself not) when no untaken item before `--` is "-" ‖ (one or more
                    characters); else Err("unknown option " ‖ item), or Err("missing a value for " ‖ item)
                    when the item is in a declared spec. "-" alone is an argument
    [Args-Help]     (D-0184)   ⟨Args::help(a), Σ⟩ → "Usage: " ‖ usage ‖ "\n", then, when options were declared,
                    "\nOptions:\n" and one line per declaration in order: two spaces, "-s, " (or four spaces),
                    "--name", " VALUE" for an option, padded to the longest spec plus two, the help, "\n"
    Args::new(usage) reads arg(0) … arg(arg_count() - 1) (Err(Utf8Error) for one that is not UTF-8);
    Args::from(items, usage) takes them as given.

    // ---- D-0184: command-line options ----
    //
    // The arguments read as options and positional arguments, declared as
    // they are read: each `flag` or `option` call names what it takes from
    // the arguments and adds a line to `help`; `finish` gives what is left
    // and refuses an option nobody declared. `--name`, `--name=value`,
    // `--name value` and `-s value` are read; `--` ends the options, and a
    // lone `-` is an argument.
    export struct Args
    {
        String usage;
        Vec<String> items;
        Vec<bool> used;
        usize end_of_options;               // the index of `--`, or the count
        Vec<String> specs;                  // for `help`: `-s, --name VALUE`
        Vec<String> helps;
    }

    // The program's arguments (`arg(0)` on), with the usage line `help`
    // shows (`prog [options] FILE…`); `Err` for an argument that is not
    // UTF-8.
    export fn Args::new(StringView usage) : Result<Args, Utf8Error>
    {
        Vec<String> items = Vec::new();
        for (usize i = 0; i < arg_count(); i += 1)
        {
            Vec::push(&mut items, arg(i)?);
        }
        Ok(Args::from(items, usage))
    }

    // `Args` over the arguments given, for a program that has them from
    // elsewhere, or a test.
    export fn Args::from(Vec<String> items, StringView usage) : Args
    {
        usize n = Vec::len(&items);
        usize end = n;
        for (usize i = 0; i < n; i += 1)
        {
            if (items[i] == "--")
            {
                end = i;
                break;
            }
        }
        Vec<bool> used = Vec::filled(n, false);
        if (end < n)
        {
            used[end] = true;
        }
        Args { .usage = String::from_view(usage), .items = items, .used = used, .end_of_options = end, .specs = Vec::new(), .helps = Vec::new() }
    }

    // Whether item `i` is the option `--name` (or `-short`): exactly, or,
    // for `with_value`, as `--name=…`.
    fn Args::is_option(ref<Args, shared> a, usize i, str name, str short, bool with_value) : bool
    {
        StringView item = String::as_view(&a.items[i]);
        if (Some(rest) = StringView::strip_prefix(item, "--"))
        {
            if (StringView::eq_str(rest, name))
            {
                return true;
            }
            if (with_value)
            {
                if (Some(after) = StringView::strip_prefix(rest, StringView::of(name)))
                {
                    return StringView::starts_with(after, "=");
                }
            }
            return false;
        }
        str_len(short) > 0 && StringView::len(item) == 2 && item.bytes[0] == b'-' && item.bytes[1] == str_byte(short, 0)
    }

    fn Args::spec(ref<Args, exclusive> a, str name, str short, str value_name, str help)
    {
        String spec = String::new();
        if (str_len(short) > 0)
        {
            String::append(&mut spec, "-");
            String::append(&mut spec, short);
            String::append(&mut spec, ", ");
        }
        else
        {
            String::append(&mut spec, "    ");
        }
        String::append(&mut spec, "--");
        String::append(&mut spec, name);
        if (str_len(value_name) > 0)
        {
            String::append(&mut spec, " ");
            String::append(&mut spec, value_name);
        }
        Vec::push(&mut a.specs, spec);
        Vec::push(&mut a.helps, String::from_str(help));
    }

    // Whether `--name` or `-short` (one letter, or `""` for none) is among
    // the arguments; every occurrence is taken. `help` is its line in `help`.
    export fn Args::flag(ref<Args, exclusive> a, str name, str short, str help) : bool
    {
        Args::spec(a, name, short, "", help);
        bool found = false;
        for (usize i = 0; i < a.end_of_options; i += 1)
        {
            if (!a.used[i] && Args::is_option(a, i, name, short, false))
            {
                a.used[i] = true;
                found = true;
            }
        }
        found
    }

    // Every value given for `--name` (as `--name=value` or `--name value`)
    // or `-short value`, in order; a `--name` as the last argument, with no
    // value, is left for `finish` to refuse.
    export fn Args::options(ref<Args, exclusive> a, str name, str short, str value_name, str help) : Vec<String>
    {
        Args::spec(a, name, short, value_name, help);
        Vec<String> out = Vec::new();
        for (usize i = 0; i < a.end_of_options; i += 1)
        {
            if (a.used[i] || !Args::is_option(a, i, name, short, true))
            {
                continue;
            }
            StringView item = String::as_view(&a.items[i]);
            if (Some(eq) = StringView::find(item, "="))
            {
                if (StringView::starts_with(item, "--"))
                {
                    a.used[i] = true;
                    Vec::push(&mut out, String::from_view(StringView::sub(item, eq + 1, StringView::len(item))));
                    continue;
                }
            }
            if (i + 1 < Vec::len(&a.items) && i + 1 != a.end_of_options)
            {
                a.used[i] = true;
                a.used[i + 1] = true;
                Vec::push(&mut out, String::clone(&a.items[i + 1]));
            }
        }
        out
    }

    // The value of `--name` (the last one given), or `None`.
    export fn Args::option(ref<Args, exclusive> a, str name, str short, str value_name, str help) : Option<String>
    {
        Vec<String> all = Args::options(a, name, short, value_name, help);
        Vec::pop(&mut all)
    }

    // The arguments that are not options, in order (everything after `--`
    // among them); `Err(message)` naming the first argument that looks like
    // an option but was not declared, or an option that is missing its
    // value, for the program to print.
    export fn Args::finish(ref<Args, exclusive> a) : Result<Vec<String>, String>
    {
        Vec<String> out = Vec::new();
        for (usize i = 0; i < Vec::len(&a.items); i += 1)
        {
            if (a.used[i])
            {
                continue;
            }
            StringView item = String::as_view(&a.items[i]);
            if (i < a.end_of_options && StringView::len(item) > 1 && StringView::starts_with(item, "-"))
            {
                bool declared = false;
                foreach (s in &a.specs)
                {
                    // `-s, --name VALUE`: a declared option given without its value.
                    if (StringView::contains(String::as_view(s), item))
                    {
                        declared = true;
                    }
                }
                String m = String::new();
                String::append(&mut m, if (declared) { "missing a value for " } else { "unknown option " });
                String::append(&mut m, item);
                return Err(m);
            }
            Vec::push(&mut out, String::clone(&a.items[i]));
        }
        Ok(out)
    }

    // The usage line and one line per option declared so far, aligned.
    export fn Args::help(ref<Args, shared> a) : String
    {
        String out = String::from_str("Usage: ");
        String::append(&mut out, &a.usage);
        String::append(&mut out, "\n");
        if (Vec::len(&a.specs) == 0)
        {
            return out;
        }
        String::append(&mut out, "\nOptions:\n");
        usize width = 0;
        foreach (s in &a.specs)
        {
            width = max(width, String::len(s));
        }
        for (usize i = 0; i < Vec::len(&a.specs); i += 1)
        {
            String::append(&mut out, "  ");
            String::append(&mut out, &a.specs[i]);
            for (usize k = String::len(&a.specs[i]); k < width + 2; k += 1)
            {
                String::append(&mut out, " ");
            }
            String::append(&mut out, &a.helps[i]);
            String::append(&mut out, "\n");
        }
        out
    }

**Depends on:** rule.trust.extern-call, rule.trust.unsafe,
rule.trust.rawptr, rule.stdlib.vec, rule.stdlib.string,
rule.fn.program, D-0031
**Affects:** state.storage

## 2d. Text conversions

In `std::text`; `[Hex]` in `std::encoding::hex` and `[Base64]` in
`std::encoding::base64` (D-0198).

### `rule.stdlib.text`
**Status:** ACCEPTED

    export fn String::new() : String
    {
        String { .bytes = Vec::new() }
    }

    export fn String::as_bytes(ref<String, shared> s) : ref<Vec<u8>, shared>
    {
        &s.bytes
    }

    // Room for n more bytes (D-0129): Vec::reserve on the bytes.
    export fn String::reserve(ref<String, exclusive> s, usize n)
    {
        Vec::reserve(&mut s.bytes, n);
    }

    // A copy of s: a new String with the same bytes (D-0044).
    export fn String::clone(ref<String, shared> s) : String
    {
        String c = String::new();
        String::append_string(&mut c, s);
        c
    }

    // Adds one ASCII byte (below 128) to s; any other byte would not be
    // UTF-8 on its own, and faults (D-0044). Text of arbitrary bytes is
    // built in a Vec<u8> and checked by String::from_utf8.
    export fn String::push_ascii(ref<String, exclusive> s, u8 b)
    {
        if (b >= 128)
        {
            fault(not_ascii);
        }
        Vec::push(&mut s.bytes, b);
    }

    // Empties s, keeping its buffer (D-0044).
    export fn String::clear(ref<String, exclusive> s)
    {
        Vec::clear(&mut s.bytes);
    }

    [Push-Ascii-Not-Ascii]   disposition: checked
        b ≥ 128
        ────────────────────────────────────────────
        ⟨String::push_ascii(s, b), Σ⟩ → fault diag.not-ascii (dynamic); s unchanged

`String::append<T>(ref<String, exclusive> s, T x)` and
`String::parse<T>(ref<String, shared> s) : Result<T, ParseError>` are exported
functions of `std`, realized natively by both implementations (§0's
latitude), because their behaviour depends on `T` and a CobaltC body
cannot (D-0010, D-0032).

    [Append]
        T printable (rule.stdlib.print)    ⟨s, Σ⟩ →* ⟨r, Σ1⟩    ⟨x, Σ1⟩ →* ⟨v, Σ2⟩    text(v) = b0..b_{n-1}
        ────────────────────────────────────────────
        Γ ⊢ String::append(s, x) : void;
        ⟨String::append(s, x), Σ⟩ → ⟨(), Σ3⟩, Σ3 as n calls Vec::push(&mut (*r).bytes, b_j), j = 0..n−1, give it

    [Append-Not-Printable]   disposition: rejected
        T not printable
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    [Parse]
        T an integer type, f32 or f64    the bytes of the String s refers to are c0..c_{n−1}
        ────────────────────────────────────────────
        Γ ⊢ String::parse<T>(s) : Result<T, ParseError>;   ⟨String::parse<T>(s), Σ⟩ → ⟨r, Σ'⟩ with
            r = Err(Empty)            when n = 0;
            r = Err(Invalid(k))       otherwise, when c0..c_{n−1} is not a sentence of number(T):
                                      k is the length of the longest prefix of c0..c_{n−1} that is a
                                      prefix of some sentence of number(T)  (so k = n when it ends early);
            r = Err(OutOfRange)       otherwise, when value(T) is not a value of T;
            r = Ok(value(T))          otherwise

    [Parse-Not-Numeric]   disposition: rejected
        T is not an integer type, f32 or f64
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    [Hex]              (D-0151, D-0198: in std::encoding::hex)   ⟨to_hex(b), Σ⟩ → the bytes of b as lowercase hexadecimal, two digits
                       each; ⟨from_hex(v), Σ⟩ → Ok(the bytes) when v is pairs of hexadecimal digits in
                       either case, Err(Empty) for "", Err(Invalid(i)) at the first byte that is not a
                       digit, or Err(Invalid(len)) when their count is odd
    [Base64]           (D-0151, D-0198: in std::encoding::base64)   ⟨base64_encode(b), Σ⟩ → RFC 4648 §4 base64 of b (A-Z a-z 0-9 + /,
                       padded with = to a multiple of four); ⟨base64_decode(v), Σ⟩ → Ok(the bytes) for
                       what base64_encode writes (padding required; "" is Ok of no bytes),
                       Err(Invalid(i)) at the first byte that is not a base64 character (a = before
                       the final one or two is such a byte), or Err(Invalid(len)) when len is not a
                       multiple of four

    number(T integer, signed)     := sign? digit+
    number(T integer, unsigned)   := '+'? digit+
    number(T ∈ {f32, f64})        := sign? ( digit+ ('.' digit+)? (('e' | 'E') sign? digit+)? | 'inf' ) | 'NaN'
    sign := '+' | '-'        digit := '0' … '9'

    value(T integer)   = the decimal number the digits denote, negated after a '-'
    value(T float)     = 'inf' → +∞ (−∞ after '-');  'NaN' → a NaN;  otherwise the decimal number
                         d × 10^e the text denotes, rounded to the nearest value of T (ties to even);
                         a finite number that rounds beyond T's largest finite magnitude is not a value of T

`String::append(&mut s, x)` adds the bytes `printf("%v", x)` would write to
`s`; building a line and printing it writes what the separate `printf`
calls would. `String::parse<T>(&s)` accepts the whole `String` or nothing:

- **Strict.** No leading or trailing whitespace, `_`, other base, or
  `.5` / `5.` forms; `-` is invalid for an unsigned `T`, even for
  `-0`. Leading zeros are allowed. `Invalid(k)` points at the first
  byte that cannot continue a number (`"12x4"` → `Invalid(2)`), or past
  the end when the text stops too early (`"-"` → `Invalid(1)`,
  `"1e"` → `Invalid(2)`).
- **Order.** An invalid byte anywhere is `Invalid`, even when the
  digits before it are already out of range.
- **Floats** are correctly rounded in `T` itself (`String::parse<f32>` does
  not round through `f64`). Every text `%v` writes for a float `v`
  parses back to `v` in the same type (a NaN to a NaN).
- **Where `T` is checked.** At the call, as for `%v`; a type
  parameter passed through is checked at each instantiation.
- **Names.** `String::parse`, `ParseError` and its variants are items
  of `std` (D-0134: `parse` until then); a program's own of the same
  name takes precedence (D-0024).

    // ---- `std::encoding::hex`, `std::encoding::base64` (D-0151, D-0198) ----

    // The bytes as lowercase hexadecimal, two digits each.
    // Built in a local `Vec<u8>` that is only pushed to, which the compiler
    // can confine (no check per byte), then made the `String`: ASCII only.
    export fn to_hex(slice<u8, shared> bytes) : String
    {
        str digits = "0123456789abcdef";
        Vec<u8> out = Vec::new();
        Vec::reserve(&mut out, slice_len(bytes) * 2);
        for (usize i = 0; i < slice_len(bytes); i += 1)
        {
            Vec::push(&mut out, str_byte(digits, widen<usize>(bytes[i] >> 4)));
            Vec::push(&mut out, str_byte(digits, widen<usize>(bytes[i] & 15)));
        }
        String { .bytes = out }
    }

    // The value of a hexadecimal digit in either case; 255 for any other byte.
    fn hex_value(u8 b) : u8
    {
        if (b >= b'0' && b <= b'9')
        {
            return b - b'0';
        }
        if (b >= b'a' && b <= b'f')
        {
            return b - b'a' + 10;
        }
        if (b >= b'A' && b <= b'F')
        {
            return b - b'A' + 10;
        }
        255
    }

    // Reads hexadecimal in either case, two digits a byte: `Err(Empty)` for
    // "", `Err(Invalid(i))` at the first byte that is not a digit, or at the
    // length when the digits are odd in number.
    export fn from_hex(StringView text) : Result<Vec<u8>, ParseError>
    {
        hex_decode(text.bytes)
    }

    // `from_hex` on the bytes: their slice the only reference, read unchecked,
    // into a local vector the compiler confines.
    fn hex_decode(slice<u8, shared> b) : Result<Vec<u8>, ParseError>
    {
        usize n = slice_len(b);
        if (n == 0)
        {
            return Err(Empty);
        }
        Vec<u8> out = Vec::new();
        Vec::reserve(&mut out, n / 2);
        for (usize i = 0; i < n; i += 1)
        {
            if (hex_value(b[i]) == 255)
            {
                return Err(Invalid(i));
            }
        }
        if (n % 2 != 0)
        {
            return Err(Invalid(n));
        }
        for (usize i = 0; i < n; i += 2)
        {
            Vec::push(&mut out, (hex_value(b[i]) << 4) | hex_value(b[i + 1]));
        }
        Ok(out)
    }

    // The bytes in base64 (RFC 4648 §4: `A`-`Z`, `a`-`z`, `0`-`9`, `+`, `/`,
    // padded with `=` to a multiple of four characters).
    export fn base64_encode(slice<u8, shared> bytes) : String
    {
        str alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        Vec<u8> out = Vec::new();
        usize n = slice_len(bytes);
        Vec::reserve(&mut out, (n + 2) / 3 * 4);
        usize i = 0;
        while (i + 3 <= n)
        {
            u32 v = (widen<u32>(bytes[i]) << 16) | (widen<u32>(bytes[i + 1]) << 8) | widen<u32>(bytes[i + 2]);
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>((v >> 18) & 63)));
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>((v >> 12) & 63)));
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>((v >> 6) & 63)));
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>(v & 63)));
            i += 3;
        }
        if (n - i == 1)
        {
            u32 v = widen<u32>(bytes[i]) << 16;
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>((v >> 18) & 63)));
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>((v >> 12) & 63)));
            Vec::push(&mut out, b'=');
            Vec::push(&mut out, b'=');
        }
        else if (n - i == 2)
        {
            u32 v = (widen<u32>(bytes[i]) << 16) | (widen<u32>(bytes[i + 1]) << 8);
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>((v >> 18) & 63)));
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>((v >> 12) & 63)));
            Vec::push(&mut out, str_byte(alphabet, narrow<usize>((v >> 6) & 63)));
            Vec::push(&mut out, b'=');
        }
        String { .bytes = out }
    }

    // The value of a base64 character; 255 for any other byte (`=` included).
    fn base64_value(u8 b) : u8
    {
        if (b >= b'A' && b <= b'Z')
        {
            return b - b'A';
        }
        if (b >= b'a' && b <= b'z')
        {
            return b - b'a' + 26;
        }
        if (b >= b'0' && b <= b'9')
        {
            return b - b'0' + 52;
        }
        if (b == b'+')
        {
            return 62;
        }
        if (b == b'/')
        {
            return 63;
        }
        255
    }

    // Reads base64 as `base64_encode` writes it, padding required: `Ok` of the
    // empty vector for "", `Err(Invalid(i))` at the first byte that is not a
    // base64 character (a `=` before the final one or two is one), or at the
    // length when it is not a multiple of four.
    export fn base64_decode(StringView text) : Result<Vec<u8>, ParseError>
    {
        base64_decode_bytes(text.bytes)
    }

    // `base64_decode` on the bytes (as `hex_decode`).
    fn base64_decode_bytes(slice<u8, shared> b) : Result<Vec<u8>, ParseError>
    {
        usize n = slice_len(b);
        Vec<u8> out = Vec::new();
        if (n == 0)
        {
            return Ok(Vec::new());
        }
        if (n % 4 != 0)
        {
            return Err(Invalid(n));
        }
        usize pad = 0;
        if (b[n - 1] == b'=')
        {
            pad = 1;
            if (b[n - 2] == b'=')
            {
                pad = 2;
            }
        }
        Vec::reserve(&mut out, n / 4 * 3);
        for (usize i = 0; i < n; i += 4)
        {
            u32 v = 0;
            for (usize j = 0; j < 4; j += 1)
            {
                u8 c = b[i + j];
                if (c == b'=' && i + j >= n - pad)
                {
                    v = v << 6;
                }
                else
                {
                    u8 d = base64_value(c);
                    if (d == 255)
                    {
                        return Err(Invalid(i + j));
                    }
                    v = (v << 6) | widen<u32>(d);
                }
            }
            Vec::push(&mut out, narrow_wrapping<u8>(v >> 16));
            if (i + 4 < n || pad < 2)
            {
                Vec::push(&mut out, narrow_wrapping<u8>(v >> 8));
            }
            if (i + 4 < n || pad < 1)
            {
                Vec::push(&mut out, narrow_wrapping<u8>(v));
            }
        }
        Ok(out)
    }


    [Parse-Bool]   (D-0181)   T = bool in String::parse<T> / StringView::parse<T>: Ok(true) for the text "true",
                   Ok(false) for "false", Err(Empty) for empty text, else Err(Invalid(k)) with k the length of
                   the longest prefix of either word that the text begins with

**Depends on:** rule.stdlib.print, rule.stdlib.string, rule.stdlib.vec,
rule.type.kind, rule.arith.represent, D-0032
**Affects:** state.storage (through `Vec::push`)

## 2e. Files

In `std::fs` (D-0176).

### `rule.stdlib.file`
**Status:** ACCEPTED

`read_file` and `write_file` are exported functions of `std`, realized
natively by both implementations (§0's latitude), since they reach the
program's environment, as `stdin_read` and `arg_bytes` do. A path, and
`write_file`'s text, is a `StringView` (D-0134): a literal
(`[Str-Literal-View]`), or `&s[0..$]` of a `String`.

    [Read-File]
        the bytes of the view path name a file of the environment;
        its contents are the bytes c0..c_{n−1}
        ────────────────────────────────────────────
        ⟨read_file(path), Σ⟩ → ⟨r, Σ'⟩ with
            r = Ok(s)                   the String of c0..c_{n−1}, when they are UTF-8 (String::from_utf8);
            r = Err(Utf8(e))            when they are not, e as String::from_utf8 gives it;
            r = Err(NotFound)           when no such file exists;
            r = Err(Denied)             when the environment refuses to let the program read it;
            r = Err(Io)                 for any other failure (a directory, a device error, …)

    [Write-File]
        the bytes of the view path name a file of the environment
        ────────────────────────────────────────────
        ⟨write_file(path, text), Σ⟩ → ⟨r, Σ'⟩ with
            r = Ok(())                  the file now holds exactly the bytes of text, created if it did
                                        not exist, its previous contents replaced if it did;
            r = Err(NotFound)           when a directory on the path does not exist;
            r = Err(Denied)             when the environment refuses the write;
            r = Err(Io)                 for any other failure

Files belong to the program's environment, like its arguments and
standard input (`rule.fn.program`). How a path names a file is
implementation-defined and documented; both implementations pass it to
the operating system, which resolves a relative path against the
working directory. A file's contents are whatever the environment
holds when it is read, so `read_file` of a file another program
changes is not deterministic; every other part of the program is
unaffected.

- **Whole files.** `read_file` returns the whole file, `write_file`
  replaces the whole file; `File` (`rule.stdlib.file-handle`) reads and
  writes one in pieces.
- **Text.** Both work in text: `read_file` checks the bytes are
  UTF-8, reporting the offset of the first that is not, as `read_line`
  does; `write_file` can write only UTF-8 (a view's bytes are).
- **One error.** `FileError` is the error of every input and output
  operation of `std`, standard input's included (D-0134).

**Depends on:** rule.stdlib.string, rule.stdlib.vec, rule.fn.program,
D-0034
**Affects:** the program's environment

### `rule.stdlib.file-handle`
**Status:** ACCEPTED

A `File` (D-0054) is an open file of the environment, read and written
in order from a position, for files too large to hold, for bytes that
are not text, and for appending. It is a resource (`spec/07`): one
owner, moved into a thread with `spawn` like any other, destroyed
exactly once, and its destructor closes the file. Its fields are
private to `std`, so every `File` comes from opening one.

    [File-Open]
        the bytes of the String path refers to name a file of the environment
        ────────────────────────────────────────────
        ⟨File::open(path), Σ⟩ → ⟨r, Σ'⟩     for reading; the file must exist
        ⟨File::create(path), Σ⟩ → ⟨r, Σ'⟩   for writing: created, or its contents discarded
        ⟨File::append(path), Σ⟩ → ⟨r, Σ'⟩   for writing at its end, created if it does not exist
        ⟨File::open_rw(path), Σ⟩ → ⟨r, Σ'⟩  for reading and writing, created if it does not exist,
                                             its contents kept
            r = Ok(f)          f open, its position 0;
            r = Err(NotFound)  no such file (open), or a directory on the path does not exist;
            r = Err(Denied)    the environment refuses;
            r = Err(Io)        any other failure (the path names a directory, …)

    [File-Read]
        f open for reading, at position p, the file's bytes c0..c_{n−1}; k = min(max, 2^20)
        ────────────────────────────────────────────
        ⟨File::read(f, buf, max), Σ⟩ → ⟨Ok(j), Σ'⟩
            buf gains c_p..c_{p+j−1} at its end, f's position becomes p + j, with
            j = min(k, n − p) (fewer only at the end of the file, CHG-0067);
        ⟨File::read_to_end(f, buf), Σ⟩ → ⟨Ok(n − p), Σ'⟩
            buf gains c_p..c_{n−1}; f's position becomes n

    [File-Read-Line]
        f open for reading, at position p; q the position after the first byte 10 at or
        after p, or n when there is none
        ────────────────────────────────────────────
        ⟨File::read_line(f), Σ⟩ → ⟨r, Σ'⟩, f's position becomes q
            r = Ok(None)              when p = n;
            r = Ok(Some(s))           s the String of c_p..c_{q−1} without a final byte 10,
                                      when they are UTF-8 (String::from_utf8);
            r = Err(Utf8(e))          when they are not, e's offset from c_p

    [File-Write]
        f open for writing, at position p
        ────────────────────────────────────────────
        ⟨File::write(f, data), Σ⟩ → ⟨Ok(()), Σ'⟩
            the file holds data's bytes from p (from its end, for append), growing as needed;
            f's position becomes the byte after them;
        ⟨File::write_text(f, text), Σ⟩ is ⟨File::write(f, text's bytes), Σ⟩      -- text a StringView (D-0134)

    [File-Seek]
        ⟨File::seek(f, pos), Σ⟩ → ⟨Ok(()), Σ'⟩, f's position becomes pos
        (beyond the end is allowed: reading there gives 0 bytes, writing there extends the file)
        ⟨File::len(f), Σ⟩ → ⟨Ok(n), Σ⟩, n the file's length in bytes

    [File-Printf]   (D-0059)
        ⟨File::printf(f, format, a1, …, an), Σ⟩ is ⟨File::write_formatted(f, text), Σ⟩ with text the
        formatted text [Printf] would print (its format checked the same way, [Format-Invalid],
        [Format-Arg-Mismatch]); `write_formatted`, private to `std`, writes its bytes as [File-Write] does

    [File-Close]
        ⟨File::close(f), Σ⟩ → ⟨r, Σ'⟩, f closed and destroyed
            r = Ok(())  when f was open only for reading, or its bytes reached the
                        environment's storage (made durable);
            r = Err(e)  when the environment reports a failure; f is closed all the same

    [File-Drop]
        destroying an open f closes it, without making it durable and without reporting a failure

Any operation above gives `Err(Io)` (or `Err(Denied)`, `Err(NotFound)`)
when the environment reports a failure, as `[Read-File]` does. `read` or
`read_line` on a file opened only for writing, and `write` on one opened
only for reading, give exactly `Err(Io)`, the same on every platform
(`CHG-0071`).

    [Read-Bytes]
        ⟨read_bytes(path), Σ⟩ → ⟨r, Σ'⟩ with r = Ok(v), v the file's bytes, or the error
        [File-Open]'s open or [File-Read]'s read_to_end gives
    [Write-Bytes]
        ⟨write_bytes(path, data), Σ⟩ → ⟨r, Σ'⟩ with r = Ok(()), the file holding exactly
        data's bytes, or the error [File-Open]'s create or [File-Write] gives

- **Buffering.** Reading is buffered: `read_line` does not read the
  environment a byte at a time, and a `read` returns `max` bytes (at
  most a MiB), fewer only at the end of the file. Writing is buffered
  too (D-0201), by `[File-Buffer]` below. Mixing them on an `open_rw`
  file is exact: a write lands at the position reading reached.

      [File-Buffer]   (D-0201)
          the bytes `write` (and `write_text`, `printf`) gives a file reach the environment
          no later than:
            - any other operation on the same file: a read, `read_line`, `seek`, `len`,
              `position`, `flush` or `close`, or its destruction;
            - any other file-system operation of the program, on any path: opening a file,
              `read_file`, `write_file`, `read_bytes`, `write_bytes`, `file_info`, listing,
              making or removing a directory, removing, renaming or copying a file, every
              path function that consults the file system;
            - the start of a child process (`Command::status`, `output`, `spawn`);
            - the program's end, however it ends (return from `main`, `exit`, a fault)
          so the program itself sees every byte it wrote, by any route. Only a process
          observing the file meanwhile (or a kill from outside) can tell. The buffer's size
          is not part of the rule (both implementations: 8 KiB; a larger write is not
          buffered). A failure the environment reports for buffered bytes is reported by the
          file's next operation (`close`'s result included), or, for a destroyed file, not at
          all, as `[File-Drop]` reports nothing.
- **Positions** are `u64`, not `usize`, so a file larger than the
  address space is still addressed.
- **Lines.** `read_line` removes the final `'\n'`, and one `'\r'`
  immediately before it, as `read_line` on standard input does
  (D-0092); any other `'\r'` stays.
- **Closing.** `close` reports a failure; the destructor cannot, and
  closes without waiting. A program that must know its bytes were
  stored calls `close`.
- **Handles.** A `File` holds the index of an entry in a table of the
  implementation's, not the operating system's handle; both
  implementations share one (`impl/src/fileio.rs`).


    [Create-New]   (D-0181)   ⟨File::create_new(p), Σ⟩ → Ok(Some(f)), f open for writing on a new file at p, when
                   nothing was there; Ok(None) when a file or directory is (nothing changed); Err(e) as
                   File::create's otherwise (NotFound for a missing directory above p)
    [Flush]        (D-0181)   ⟨File::flush(f), Σ⟩ → Ok(()) once everything written to f is handed to the device
                   (its buffered bytes written out, [File-Buffer], then the system's fsync), Err(Io) on a
                   file open for reading or a device failure
    [Position]     (D-0181)   ⟨File::position(f), Σ⟩ → Ok(the byte the next read or write starts at): 0 when opened
                   (the length, for append, once written), moved by every read, read_line, write and seek

    // D-0181: for writing, created only if nothing is there: `Ok(None)` when
    // a file (or a directory) of that name already exists, so two programs
    // that race to make the same file see one win. A lock file, a pid file,
    // a file that must never be overwritten.
    export fn File::create_new(StringView path) : Result<Option<File>, FileError>
    {
        isize r = unsafe
        {
            file_op(0, 4, StringView::ptr(path), StringView::len(path))
        };
        if (r == -4)
        {
            return Ok(None);
        }
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(Some(File { .id = reinterpret<usize>(r), .open = true }))
    }

    // D-0181: everything written so far made durable: handed to the
    // operating system and by it to the device, so it survives a crash or a
    // power loss from here on. The write-then-rename pattern flushes before
    // the rename.
    export fn File::flush(ref<File, exclusive> f) : Result<void, FileError>
    {
        isize r = unsafe
        {
            file_op(8, f.id, dangling<u8>(), 0)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // D-0181: the position the next read or write starts at, in bytes from
    // the start.
    export fn File::position(ref<File, shared> f) : Result<u64, FileError>
    {
        i64 r = unsafe
        {
            file_at(9, f.id, 0)
        };
        if (r < 0)
        {
            return Err(file_error_at(r));
        }
        Ok(reinterpret<u64>(r))
    }

The `std` source:

    // `File` (`rule.stdlib.file-handle`, spec/21 §2e, D-0054) is written over
    // these two, private to `std`: `op` names the operation, `h` the open file
    // (or the mode, for opening), `buf`/`n` its bytes or its count; `file_at`
    // takes or gives a position. Each returns a handle, a count, a length or
    // 0, or a failure code as `file_read`'s (src/fileio.rs).
    unsafe extern fn file_op(usize op, usize h, rawptr<u8> buf, usize n) : isize;
    unsafe extern fn file_at(usize op, usize h, u64 pos) : i64;

    // An open file (`rule.stdlib.file-handle`, spec/21 §2e, D-0054): its
    // bytes read and written in order from a position, which `seek` moves.
    // Its destructor closes it; `close` closes it and reports a failure.
    export resource struct File
    {
        usize id;
        bool open;                          // false once close has closed it
    }

    // `file_error` for `file_at`'s codes.
    fn file_error_at(i64 code) : FileError
    {
        if (code == -1)
        {
            return FileError::NotFound;
        }
        if (code == -2)
        {
            return FileError::Denied;
        }
        FileError::Io
    }

    fn File::start(StringView path, usize mode) : Result<File, FileError>
    {
        isize r = unsafe
        {
            file_op(0, mode, StringView::ptr(path), StringView::len(path))
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(File { .id = reinterpret<usize>(r), .open = true })
    }

    // For reading; the file must exist.
    export fn File::open(StringView path) : Result<File, FileError>
    {
        File::start(path, 0)
    }

    // For writing, from empty: created, or its contents discarded.
    export fn File::create(StringView path) : Result<File, FileError>
    {
        File::start(path, 1)
    }

    // For writing at its end, created if it does not exist.
    export fn File::append(StringView path) : Result<File, FileError>
    {
        File::start(path, 2)
    }

    // For reading and writing, created if it does not exist, its contents kept.
    export fn File::open_rw(StringView path) : Result<File, FileError>
    {
        File::start(path, 3)
    }

    // Appends exactly `want` bytes of what `file_op` reads, or fewer at the end.
    fn File::take(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf, usize want) : Result<usize, FileError>
    {
        while (buf.cap - buf.len < want)
        {
            Vec::grow(buf);
        }
        isize r = unsafe
        {
            file_op(3, f.id, reinterpret_ptr<u8>(buf.ptr + reinterpret<isize>(buf.len)), want)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        usize n = reinterpret<usize>(r);
        buf.len = buf.len + n;
        Ok(n)
    }

    // Appends up to `max` bytes to `buf` (at most a MiB, and fewer when fewer
    // are ready) and returns how many: 0 only at the end, when max > 0.
    export fn File::read(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, FileError>
    {
        usize want = if (max > 1_048_576)
        {
            1_048_576
        }
        else
        {
            max
        };
        File::take(f, buf, want)
    }

    // Appends the rest of the file to `buf` and returns how many bytes.
    export fn File::read_to_end(ref<File, exclusive> f, ref<Vec<u8>, exclusive> buf) : Result<usize, FileError>
    {
        usize total = 0;
        while (true)
        {
            usize n = match (File::read(f, buf, 65_536))
            {
                Ok(n) : n,
                Err(e) : return Err(e),
            };
            if (n == 0)
            {
                break;
            }
            total = total + n;
        }
        Ok(total)
    }

    // The next line without its '\n', nor a '\r' just before it (D-0092),
    // as `read_line` reads standard input; None at the end of the file.
    export fn File::read_line(ref<File, exclusive> f) : Result<Option<String>, FileError>
    {
        isize r = unsafe
        {
            file_op(4, f.id, dangling<u8>(), 0)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        if (r == 0)
        {
            return Ok(None);
        }
        Vec<u8> bytes = Vec::new();
        usize n = match (File::take(f, &mut bytes, reinterpret<usize>(r)))
        {
            Ok(n) : n,
            Err(e) : return Err(e),
        };
        if (n > 0 && bytes[n - 1] == 10)
        {
            Vec::pop(&mut bytes);
            if (n > 1 && bytes[n - 2] == 13)
            {
                Vec::pop(&mut bytes);
            }
        }
        match (String::from_utf8(bytes))
        {
            Ok(s) : Ok(Some(s)),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

    // Writes all of `data` at the file's position (at its end, for append).
    export fn File::write(ref<File, exclusive> f, slice<u8, shared> data) : Result<void, FileError>
    {
        usize n = slice_len(data);
        if (n == 0)
        {
            return Ok(());
        }
        isize r = unsafe
        {
            file_op(5, f.id, rawptr_of(&data[0]), n)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // D-0134: writes the text at the file's position.
    export fn File::write_text(ref<File, exclusive> f, StringView text) : Result<void, FileError>
    {
        File::write(f, text.bytes)
    }

    // `File::printf(f, format, …)` (D-0059) is `File::write_formatted(f,
    // $fmt(format, …))` after `modres`, as `String::appendf` is
    // `String::append` of it.
    fn File::write_formatted(ref<File, exclusive> f, str text) : Result<void, FileError>
    {
        usize n = str_len(text);
        if (n == 0)
        {
            return Ok(());
        }
        isize r = unsafe
        {
            file_op(5, f.id, str_ptr(text), n)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // Moves the position to byte `pos` from the start.
    export fn File::seek(ref<File, exclusive> f, u64 pos) : Result<void, FileError>
    {
        i64 r = unsafe
        {
            file_at(6, f.id, pos)
        };
        if (r < 0)
        {
            return Err(file_error_at(r));
        }
        Ok(())
    }

    // The file's length in bytes.
    export fn File::len(ref<File, shared> f) : Result<u64, FileError>
    {
        i64 r = unsafe
        {
            file_at(7, f.id, 0)
        };
        if (r < 0)
        {
            return Err(file_error_at(r));
        }
        Ok(reinterpret<u64>(r))
    }

    // Closes the file. A file open for writing is first made durable, so a
    // failure the environment reports late is reported here.
    export fn File::close(File f) : Result<void, FileError>
    {
        f.open = false;
        isize r = unsafe
        {
            file_op(1, f.id, dangling<u8>(), 0)
        };
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    export fn File::drop(ref<File, exclusive> self)
    {
        if (self.open)
        {
            isize r = unsafe
            {
                file_op(2, self.id, dangling<u8>(), 0)
            };
        }
    }

    // The whole file's bytes, as `read_file` reads its text.
    export fn read_bytes(StringView path) : Result<Vec<u8>, FileError>
    {
        File f = match (File::open(path))
        {
            Ok(f) : f,
            Err(e) : return Err(e),
        };
        Vec<u8> bytes = Vec::new();
        match (File::read_to_end(&mut f, &mut bytes))
        {
            Ok(_) : Ok(bytes),
            Err(e) : Err(e),
        }
    }

    // Makes the file hold exactly `data`, as `write_file` does its text.
    export fn write_bytes(StringView path, slice<u8, shared> data) : Result<void, FileError>
    {
        File f = match (File::create(path))
        {
            Ok(f) : f,
            Err(e) : return Err(e),
        };
        File::write(&mut f, data)
    }

**Depends on:** rule.stdlib.file, rule.stdlib.vec, rule.agg.slice,
rule.resauth.destroy, rule.conc.spawn, D-0054
**Affects:** the program's environment

## 2e′. Directories, the environment and the clocks

The directories, paths and file metadata in `std::fs`, the environment in
`std::env`, the clocks in `std::time` (D-0176).

### `rule.stdlib.fs`
**Status:** ACCEPTED

Paths are `String`s, resolved as `rule.stdlib.file`'s are. Failures are
`FileError`s, with `rule.stdlib.file`'s meanings (D-0061).

    [Make-Dir]      ⟨make_dir(p), Σ⟩ → Ok(true), the directory p made; Ok(false) when a directory
                    is already at p; Err(NotFound) when p's parent does not exist; Err(Io) when
                    something that is not a directory is at p
    [Remove-File]   ⟨remove_file(p), Σ⟩ → Ok(()), the file gone; Err(NotFound) when there is none
    [Remove-Dir]    ⟨remove_dir(p), Σ⟩ → Ok(()), the empty directory gone; Err(Io) when it is not
                    empty; Err(NotFound) when there is none
    [Rename]        ⟨rename(a, b), Σ⟩ → Ok(()), what was at a now at b (a file at b replaced)
    [List-Dir]      ⟨list_dir(p), Σ⟩ → Ok(names), the names of p's entries (never `.` or `..`)
                    sorted by their bytes; Err(Utf8(e)) for a name that is not UTF-8
    [Path-Kind]     ⟨path_kind(p), Σ⟩ → Ok(File(n)) a file of n bytes, Ok(Dir), Ok(Other) (a
                    device, a socket, …), following a symbolic link; Err(NotFound) when nothing
                    is at p

**Paths and file metadata** (D-0141). A path's grammar is the
platform's: `/` separates parts on Unix; `\` and `/` on Windows, where
`C:\` and `\\server\share\` begin absolute paths. So the results of the
part functions differ between platforms for the same text, as the
system's own path functions do. The part functions (`path_join`,
`path_parent`, `path_file_name`, `path_stem`, `path_extension`,
`path_is_absolute`) and `path_normalize` read the text alone and never
touch the file system; the others ask it. Each result is a new
`String`; a path that is not UTF-8 is `Err(Utf8(e))` where a `Result`
is returned, and `temp_dir` and `home_dir` replace any byte sequence that
is not UTF-8 with U+FFFD.

    [Path-Join]        ⟨path_join(base, tail), Σ⟩ → base and tail joined by the platform's separator
                       (none added after a separator); tail alone when it is absolute
    [Path-Parts]       ⟨path_parent(p)⟩ → Some(all of p but its last part), None for a root or ``;
                       ⟨path_file_name(p)⟩ → Some(the last part), None when it is `..` or there is
                       none; ⟨path_stem(p)⟩, ⟨path_extension(p)⟩ → the file name before and after its
                       last dot (no extension for a name whose only dot is its first byte);
                       ⟨path_is_absolute(p)⟩ → whether p names a place without the working directory
    [Path-Normalize]   ⟨path_normalize(p)⟩ → p's parts with every `.` removed and every `x/..` (x a
                       name) removed, repeatedly; a `..` after a root removed, a leading `..` kept;
                       `.` when no part is left. By the text alone: a link is not followed
    [Path-Canonical]   ⟨path_canonical(p), Σ⟩ → Ok(the absolute path the system resolves p to, every
                       link followed); Err(NotFound) when nothing is at p
    [File-Info]        ⟨file_info(p), Σ⟩ → Ok(FileInfo { kind, len, modified_unix, readonly }),
                       following a link, kind as path_kind's; modified_unix the last change's
                       seconds since 1970, rounded down; Err(NotFound) when nothing is at p
    [Copy-File]        ⟨copy_file(a, b), Σ⟩ → Ok(n), b a file of a's n bytes (a file at b replaced)
    [Dir-All]          ⟨make_dir_all(p), Σ⟩ → Ok(()), p and every missing directory above it made
                       (Ok when they exist); ⟨remove_dir_all(p), Σ⟩ → Ok(()), p and everything in it
                       gone; Err(NotFound) when nothing is at p
    [Set-Current-Dir]  ⟨set_current_dir(p), Σ⟩ → Ok(()), relative paths resolved against p from now
                       on, in every thread
    [Temp-Dir]         ⟨temp_dir(), Σ⟩ → the system's directory for temporary files
    [Home-Dir]         ⟨home_dir(), Σ⟩ → Some(the user's home directory: `HOME`, or `USERPROFILE` on
                       Windows), None when it is not set

### `rule.stdlib.env`
**Status:** ACCEPTED

    [Env-Var]       ⟨env_var(name), Σ⟩ → Ok(Some(v)) the variable's value, Ok(None) when it is not
                    set, Err(e) when its value is not UTF-8 (e's offset, as String::from_utf8's)
    [Current-Dir]   ⟨current_dir(), Σ⟩ → Ok(d), the working directory; Err(Utf8(e)) when its
                    name is not UTF-8
    [Clocks]        ⟨monotonic_ns(), Σ⟩ → n, nanoseconds on a clock that never goes backwards,
                    from a start fixed for the whole program; ⟨unix_seconds(), Σ⟩ → s, whole
                    seconds since 1970-01-01 00:00 UTC by the wall clock (which may be changed);
                    ⟨unix_ms(), Σ⟩ (`std::time`, D-0140) → the same clock's milliseconds

The filesystem, the environment and the clocks belong to the program's
environment (`rule.fn.program`): their answers are whatever the
environment gives when asked, so a program using them is not
deterministic in them; every other part of it is unaffected.


    [Current-Exe]   (D-0181)   ⟨current_exe(), Σ⟩ → Ok(the path of the running program's executable, as the operating
                    system reports it: absolute, links followed); Err(Utf8(_)) when it is not UTF-8, Err(Io) when
                    the system cannot say
    [Hostname]      (D-0181)   ⟨hostname(), Σ⟩ → Ok(the machine's host name: gethostname on Unix, the DNS host name on
                    Windows); Err(Utf8(_)), Err(Io) as [Current-Exe]'s

    // D-0181: the path of the running program's own executable, as the
    // operating system reports it (absolute, links followed): a program
    // finds the files installed beside it from here.
    export fn current_exe() : Result<String, FileError>
    {
        match (byte_answer(0, 15, 0, StringView::of("").bytes, StringView::of("").bytes))
        {
            Ok(v) : match (String::from_utf8(v))
            {
                Ok(s)  : Ok(s),
                Err(e) : Err(FileError::Utf8(e)),
            },
            Err(c) : Err(code_error(c)),
        }
    }

    // D-0181: the machine's host name, as the operating system knows it:
    // what a log line or a service's registration names the machine by.
    export fn hostname() : Result<String, FileError>
    {
        match (byte_answer(0, 16, 0, StringView::of("").bytes, StringView::of("").bytes))
        {
            Ok(v) : match (String::from_utf8(v))
            {
                Ok(s)  : Ok(s),
                Err(e) : Err(FileError::Utf8(e)),
            },
            Err(c) : Err(code_error(c)),
        }
    }

The `std` source:

    // The filesystem, the environment and the clocks (`rule.stdlib.fs`,
    // `rule.stdlib.env`, D-0061), over three primitives private to `std`
    // (src/fileio.rs): `fs_op` acts on one or two paths; `fs_query` copies up
    // to `cap` bytes of an answer into `buf` and returns its whole length
    // (asked again with room for all of it when longer); `clock_read` reads a
    // clock (0 monotonic nanoseconds, 1 wall-clock seconds, 2 wall-clock
    // milliseconds, D-0140). Failures are `file_read`'s codes.
    unsafe extern fn fs_op(usize op, rawptr<u8> a, usize an, rawptr<u8> b, usize bn) : isize;
    unsafe extern fn fs_query(usize op, rawptr<u8> a, usize an, rawptr<u8> buf, usize cap) : isize;
    unsafe extern fn clock_read(usize which) : i64;

    // D-0147: the seconds the machine's local time is ahead of UTC at the
    // moment `unix` (src/fileio.rs, the platform's zone rule).
    unsafe extern fn tz_offset(i64 unix) : i64;

    // What a path names (`rule.stdlib.fs`, D-0061): a file and its length in
    // bytes, a directory, or something else (a device, a socket, …).
    export enum PathKind
    {
        File(u64),
        Dir,
        Other,
    }

    fn fs_path_op(usize op, StringView a, StringView b) : isize
    {
        unsafe
        {
            fs_op(op, StringView::ptr(a), StringView::len(a), StringView::ptr(b), StringView::len(b))
        }
    }

    // The bytes of `fs_query`'s answer, or its failure code.
    fn fs_answer(usize op, rawptr<u8> a, usize an) : Result<Vec<u8>, isize>
    {
        Vec<u8> out = Vec::new();
        usize cap = 256;
        while (true)
        {
            while (out.cap < cap)
            {
                Vec::grow(&mut out);
            }
            isize n = unsafe
            {
                fs_query(op, a, an, reinterpret_ptr<u8>(out.ptr), cap)
            };
            if (n < 0)
            {
                return Err(n);
            }
            usize len = reinterpret<usize>(n);
            if (len <= cap)
            {
                out.len = len;
                return Ok(out);
            }
            cap = len;
        }
    }

    // Makes a directory: Ok(true) when it was made, Ok(false) when a
    // directory was already there (so it is safe to call again).
    export fn make_dir(StringView path) : Result<bool, FileError>
    {
        isize r = fs_path_op(0, path, "");
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(r == 1)
    }

    export fn remove_file(StringView path) : Result<void, FileError>
    {
        isize r = fs_path_op(1, path, "");
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // Removes an empty directory.
    export fn remove_dir(StringView path) : Result<void, FileError>
    {
        isize r = fs_path_op(2, path, "");
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // Renames (moves) `from` to `to`, replacing a file at `to`.
    export fn rename(StringView from, StringView to) : Result<void, FileError>
    {
        isize r = fs_path_op(3, from, to);
        if (r < 0)
        {
            return Err(file_error(r));
        }
        Ok(())
    }

    // The names in a directory (not `.` or `..`), sorted by their bytes.
    export fn list_dir(StringView path) : Result<Vec<String>, FileError>
    {
        Vec<u8> all = match (fs_answer(0, StringView::ptr(path), StringView::len(path)))
        {
            Ok(b) : b,
            Err(c) : return Err(file_error(c)),
        };
        Vec<String> names = Vec::new();
        Vec<u8> name = Vec::new();
        foreach (b in &all)
        {
            match (*b)
            {
                0 :
                {
                    Vec<u8> done = replace(&mut name, Vec::new());
                    match (String::from_utf8(done))
                    {
                        Ok(n) : Vec::push(&mut names, n),
                        Err(e) : return Err(FileError::Utf8(e)),
                    }
                },
                c : Vec::push(&mut name, c),
            }
        }
        Ok(names)
    }

    // What `path` names; `Err(NotFound)` when nothing does.
    export fn path_kind(StringView path) : Result<PathKind, FileError>
    {
        Vec<u8> b = match (fs_answer(1, StringView::ptr(path), StringView::len(path)))
        {
            Ok(b) : b,
            Err(c) : return Err(file_error(c)),
        };
        u64 len = 0;
        usize i = 8;
        while (i > 0)
        {
            len = (len << 8) | widen<u64>(b[i]);
            i -= 1;
        }
        match (b[0])
        {
            0 : Ok(File(len)),
            1 : Ok(Dir),
            _ : Ok(Other),
        }
    }

    // An environment variable's value; None when it is not set.
    export fn env_var(StringView name) : Result<Option<String>, Utf8Error>
    {
        match (fs_answer(2, StringView::ptr(name), StringView::len(name)))
        {
            Ok(b) : match (String::from_utf8(b))
            {
                Ok(v) : Ok(Some(v)),
                Err(e) : Err(e),
            },
            Err(_) : Ok(None),
        }
    }

    // The working directory, against which a relative path is resolved.
    export fn current_dir() : Result<String, FileError>
    {
        Vec<u8> b = match (fs_answer(3, dangling<u8>(), 0))
        {
            Ok(b) : b,
            Err(c) : return Err(file_error(c)),
        };
        match (String::from_utf8(b))
        {
            Ok(s) : Ok(s),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

    // A monotonic clock, in nanoseconds from an arbitrary start: for measuring
    // how long something takes; it never goes backwards.
    export fn monotonic_ns() : u64
    {
        reinterpret<u64>(unsafe
        {
            clock_read(0)
        })
    }

    // The wall clock: whole seconds since 1970-01-01 00:00 UTC.
    export fn unix_seconds() : i64
    {
        unsafe
        {
            clock_read(1)
        }
    }

    // Paths and file metadata (`rule.stdlib.fs`, D-0141), child processes
    // (`rule.stdlib.process`, D-0143) and sockets (`rule.stdlib.net`, D-0144),
    // over three primitives of one shape, private to `std` (src/fileio.rs,
    // src/procio.rs, src/netio.rs): `op` names the operation and `h` the
    // handle it acts on; `a`/`an` and `b`/`bn` are its byte strings in; up to
    // `cap` bytes of its answer are copied to `out`. Each returns a number (a
    // handle, a count, the answer's whole length, asked again with room for
    // all of it when longer) or a failure code.
    unsafe extern fn path_op(
        usize op,
        usize h,
        rawptr<u8> a,
        usize an,
        rawptr<u8> b,
        usize bn,
        rawptr<u8> out,
        usize cap,
    ) : i64;

    unsafe extern fn proc_op(
        usize op,
        usize h,
        rawptr<u8> a,
        usize an,
        rawptr<u8> b,
        usize bn,
        rawptr<u8> out,
        usize cap,
    ) : i64;

    unsafe extern fn net_op(
        usize op,
        usize h,
        rawptr<u8> a,
        usize an,
        rawptr<u8> b,
        usize bn,
        rawptr<u8> out,
        usize cap,
    ) : i64;

    // D-0142: `n` bytes from the operating system's secure source at `buf`;
    // 0, or -1 when it has none (src/osrand.rs).
    unsafe extern fn os_random(rawptr<u8> buf, usize n) : isize;

    // One of the three primitives above by number: 0 `path_op`, 1
    // `proc_op`, 2 `net_op`.
    fn byte_op(
        usize which,
        usize op,
        usize h,
        rawptr<u8> a,
        usize an,
        rawptr<u8> b,
        usize bn,
        rawptr<u8> out,
        usize cap,
    ) : i64
    {
        unsafe
        {
            match (which)
            {
                0 : path_op(op, h, a, an, b, bn, out, cap),
                1 : proc_op(op, h, a, an, b, bn, out, cap),
                _ : net_op(op, h, a, an, b, bn, out, cap),
            }
        }
    }

    // The bytes of a `byte_op`'s answer, or its failure code: asked again
    // with room for all of it when it is longer than the room given. Only for
    // operations whose answer does not change when asked twice.
    fn byte_answer(usize which, usize op, usize h, slice<u8, shared> a, slice<u8, shared> b) : Result<Vec<u8>, i64>
    {
        Vec<u8> out = Vec::new();
        usize cap = 256;
        while (true)
        {
            while (out.cap < cap)
            {
                Vec::grow(&mut out);
            }
            i64 n = byte_op(
                which,
                op,
                h,
                bytes_ptr(a),
                slice_len(a),
                bytes_ptr(b),
                slice_len(b),
                reinterpret_ptr<u8>(out.ptr),
                cap,
            );
            if (n < 0)
            {
                return Err(n);
            }
            usize len = reinterpret<usize>(n);
            if (len <= cap)
            {
                out.len = len;
                return Ok(out);
            }
            cap = len;
        }
    }

    // The address of a slice's first byte for the primitives; any address
    // when it is empty (they then read none).
    fn bytes_ptr(slice<u8, shared> s) : rawptr<u8>
    {
        if (slice_len(s) == 0)
        {
            return dangling<u8>();
        }
        rawptr_of(&s[0])
    }

    // `file_error` of an `i64` code, as the newer primitives give them.
    fn code_error(i64 code) : FileError
    {
        if (code == -1)
        {
            return FileError::NotFound;
        }
        if (code == -2)
        {
            return FileError::Denied;
        }
        FileError::Io
    }

    // ---- paths and file metadata (`rule.stdlib.fs`, D-0141) ----
    //
    // A path's grammar is the operating system's (`C:\` and `\\server\share`
    // on Windows), so these are realized by `path_op` (src/fileio.rs) over
    // the platform's own path library. The part functions (`path_join` to
    // `path_extension`) and `path_normalize` never touch the file system.

    fn path_text(StringView a, StringView b, usize op) : Option<String>
    {
        match (byte_answer(0, op, 0, a.bytes, b.bytes))
        {
            Ok(v) : match (String::from_utf8(v))
            {
                Ok(s)  : Some(s),
                Err(_) : None,
            },
            Err(_) : None,
        }
    }

    fn path_result(usize op, StringView a, StringView b) : Result<i64, FileError>
    {
        i64 r = byte_op(
            0,
            op,
            0,
            StringView::ptr(a),
            StringView::len(a),
            StringView::ptr(b),
            StringView::len(b),
            dangling<u8>(),
            0,
        );
        if (r < 0)
        {
            return Err(code_error(r));
        }
        Ok(r)
    }

    // `base` and then `tail`, with the platform's separator between; an
    // absolute `tail` replaces `base`.
    export fn path_join(StringView base, StringView tail) : String
    {
        Option::unwrap_or(path_text(base, tail, 0), String::from_view(tail))
    }

    // All but the last part: `None` for a root or an empty path; the empty
    // path for a single relative name.
    export fn path_parent(StringView p) : Option<String>
    {
        path_text(p, "", 1)
    }

    // The last part: `None` when it is `..`, or there is none (`/`, ``).
    export fn path_file_name(StringView p) : Option<String>
    {
        path_text(p, "", 2)
    }

    // The file name without its extension (`archive.tar` of `archive.tar.gz`;
    // `.bashrc` of `.bashrc`).
    export fn path_stem(StringView p) : Option<String>
    {
        path_text(p, "", 3)
    }

    // The file name's extension, without its dot (`gz` of `archive.tar.gz`);
    // `None` when there is none (a name with no dot, or only a leading one).
    export fn path_extension(StringView p) : Option<String>
    {
        path_text(p, "", 4)
    }

    export fn path_is_absolute(StringView p) : bool
    {
        match (path_result(5, p, ""))
        {
            Ok(r)  : r == 1,
            Err(_) : false,
        }
    }

    // The path with `.` parts removed and `x/..` resolved where `x` is a
    // name, by its text alone (a link is not followed); a leading `..` stays,
    // `/..` is `/`, and nothing left is `.`.
    export fn path_normalize(StringView p) : String
    {
        Option::unwrap_or(path_text(p, "", 6), String::from_view(p))
    }

    // The absolute path the operating system resolves `p` to, every link
    // followed; `p` must exist.
    export fn path_canonical(StringView p) : Result<String, FileError>
    {
        Vec<u8> b = match (byte_answer(0, 7, 0, p.bytes, StringView::of("").bytes))
        {
            Ok(b)  : b,
            Err(c) : return Err(code_error(c)),
        };
        match (String::from_utf8(b))
        {
            Ok(s)  : Ok(s),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

    // What `file_info` tells: the kind (as `path_kind`'s), the length in
    // bytes, the last modification in seconds since 1970-01-01T00:00:00Z,
    // and whether the file is read-only.
    export struct FileInfo
    {
        export PathKind kind;
        export u64 len;
        export i64 modified_unix;
        export bool readonly;
    }

    // Following a symbolic link, as `path_kind` does.
    export fn file_info(StringView path) : Result<FileInfo, FileError>
    {
        Vec<u8> b = match (byte_answer(0, 8, 0, path.bytes, StringView::of("").bytes))
        {
            Ok(b)  : b,
            Err(c) : return Err(code_error(c)),
        };
        u64 len = read_le<u64>(&b[0..$], 1);
        PathKind kind = match (b[0])
        {
            0 : File(len),
            1 : Dir,
            _ : Other,
        };
        Ok(FileInfo { .kind = kind, .len = len, .modified_unix = read_le<i64>(&b[0..$], 9), .readonly = b[17] != 0 })
    }

    // Copies the file `from` to `to`, replacing a file there; the number of
    // bytes copied.
    export fn copy_file(StringView from, StringView to) : Result<u64, FileError>
    {
        i64 n = path_result(9, from, to)?;
        Ok(reinterpret<u64>(n))
    }

    // Makes the directory and every missing directory above it; `Ok` when it
    // is there already.
    export fn make_dir_all(StringView path) : Result<void, FileError>
    {
        path_result(10, path, "")?;
        Ok(())
    }

    // Removes the directory and everything in it.
    export fn remove_dir_all(StringView path) : Result<void, FileError>
    {
        path_result(11, path, "")?;
        Ok(())
    }

    // Makes `path` the working directory, against which relative paths are
    // resolved from now on, in every thread.
    export fn set_current_dir(StringView path) : Result<void, FileError>
    {
        path_result(12, path, "")?;
        Ok(())
    }

    // The operating system's directory for temporary files.
    export fn temp_dir() : String
    {
        Option::unwrap_or(path_text("", "", 13), String::new())
    }

    // The user's home directory: `HOME`, or `USERPROFILE` on Windows; `None`
    // when neither is set.
    export fn home_dir() : Option<String>
    {
        path_text("", "", 14)
    }

**Depends on:** rule.stdlib.file, rule.stdlib.string, rule.fn.program, D-0061, D-0141
**Affects:** the program's environment

## 2f. Formatted output

In `std::io` (`printf`, `eprintf`, `File::printf`) and `std::text` (`sprintf`, `String::appendf`).

### `rule.stdlib.format`
**Status:** ACCEPTED

`printf(f, a1, …, an)` writes the text `format(f, a1, …, an)` to
standard output, as `print` of that text would (§2a). `String::appendf(s, f,
a1, …, an)` adds the same text to the `String` `*s`, as
`String::append` of it would, and `sprintf(f, a1, …, an)` returns it as
a new `String`; `eprintf(f, a1, …, an)` writes it to standard error.
All four are exported functions of `std`, realized natively (D-0038,
D-0039, D-0040); each call is typed on its own, with any number of
arguments, as `spawn`'s is. `printf` is a program's only way to write
to standard output (D-0039), and `eprintf` to standard error.

    [Printf]    ⟨printf(f, a1..an), Σ⟩ ≡ ⟨print(t), Σ⟩                     t = format(f, v1..vn), a_i evaluated left to right
    [Appendf]   ⟨String::appendf(s, f, a1..an), Σ⟩ ≡ ⟨String::append(s, t), Σ⟩   s, then a1..an, left to right
    [Sprintf]   ⟨sprintf(f, a1..an), Σ⟩ ≡ ⟨String::from_str(t), Σ⟩        Γ ⊢ sprintf(f, a1..an) : String
    [Eprintf]   ⟨eprintf(f, a1..an), Σ⟩ → ⟨(), Σ1⟩, writing t's bytes to standard error
                as [Extern-Call] of std's private stderr_write(p, n) does    (Σ1 as for [Printf])

Standard output's buffer is written out before `eprintf`'s bytes
(`[Print-Buffer]`, §2a), so the two streams interleave in program order.

    [Format-Invalid]   disposition: rejected
        f is not a string literal, or not a sentence of format (below)
        ────────────────────────────────────────────
        ill-formed; diag.format-invalid

    [Format-Arg-Mismatch]   disposition: rejected
        the number of arguments f takes (one per specifier, and one `usize` before it for each `*`)
        is not n, or a_i's type is not one its place takes
        (a type parameter: at each instantiation, rule.type.kind)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch

    format     := (byte other than '%' | '%%' | spec)*
    spec       := '%' flag* width? ('.' precision?)? conversion      -- width, precision: digits, at most 4096,
                                                                      -- or `*` (D-0100)
    flag       := '-' | '0' | '+' | ' ' | '#'

| Conversion | Argument | Text | Flags it takes |
|---|---|---|---|
| `d` `i` `u` | any integer type | the value in decimal | `- 0 + space`; a precision is a minimum digit count |
| `x` `X` `o` `b` | any integer type | the value's bits in its own width, in base 16 (lower/upper case), 8, 2 | `- 0 #` (`#`: prefix `0x`, `0X`, `0o`, `0b`); precision as for `d` |
| `f` `F` | `f32`, `f64` | fixed point, `precision` digits after the point (6 by default), correctly rounded | `- 0 + space #` |
| `e` `E` | `f32`, `f64` | `d.ddde±XX`, `precision` digits after the point (6), an exponent of at least two digits | as `f` |
| `g` `G` | `f32`, `f64` | `precision` significant digits (6; 0 is 1): `e` form when the exponent is below −4 or at least the precision, else `f` form; trailing zeros removed unless `#` | as `f` |
| `s` | `str`, `ref<String, shared>`, `bool` (`true`/`false`) | the text, at most `precision` characters | `-` |
| `v` | any printable type (§2a), or an enum (or a reference to one) | `text(v)`: what `String::append` adds — an integer in decimal, a float in the shortest form that reads back, text and `bool` as they are; an enum value is its variant's name (D-0100), its payload not shown | `-` (no precision) |

- **As C.** Every conversion prints what C's `printf` prints for the
  same value, but `%d` takes every integer type (there is no `%ld`),
  `%x`, `%o` and `%b` print a negative number's bits in its own width
  (`%x` of `-1: i8` is `ff`), and `%#o` writes CobaltC's octal prefix
  `0o` (a leading `0` alone is not octal here). `%b` is binary.
- **Padding.** A width pads with spaces on the left, or on the right
  with `-`; with `0`, a number is padded with zeros after its sign and
  prefix (not for an integer with a precision, nor for `inf`/`nan`).
  Widths and precisions count characters, not bytes, so text with
  multi-byte characters aligns and is never split.
- **Floats.** An `f32` is formatted from its exact value (as C does
  once it is promoted to `double`): `%f` of `0.1: f32` is `0.100000`.
  A NaN is `nan`, an infinity `inf` (`NAN`, `INF` for the upper-case
  conversions), with the sign and `+`/space flags as for a number.
- **Computed widths** (D-0100). `*` in place of a width or precision
  takes the next argument, a `usize`, before the value, as C's does:
  `printf("%-*s|", w, name)`. A computed width or precision above 4096
  counts as 4096.
- **Enums** (D-0100). `%v` of an enum value writes its variant's name
  (`Red`, `NotFound`, `Some`); print a payload with `match`.
- **Not included:** `%c` (a byte above 127 would not be UTF-8 in a
  `String`), `%p`, `%n`, and length modifiers.
- **References.** An argument that is a reference to a number, a
  `bool` or a `str` (either mode) is formatted as the value it refers
  to, and a `ref<String, exclusive>` as a `ref<String, shared>` (D-0042):
  in `foreach (x in &v)`, `printf("%d", x)` prints the element.
- **Checked when compiled.** The format is a literal, so a program
  whose `printf` would misprint is rejected; nothing about a format
  can fail when the program runs.
- **`%v`** is not C's: it is the conversion for "the value, as
  CobaltC writes it", so `printf("%v\n", x)` needs no thought about
  `x`'s type. A float written with `%v` reads back as the same value
  (`String::parse`, §2d); `%g` does not promise that.
- **Standard error.** `eprintf` is for messages that are not the
  program's output: errors, warnings, progress. Both streams are written
  at once, with nothing held back, so their text interleaves in the
  order the calls ran. `std` does not export `stderr_write`, the extern
  `eprintf` writes through; its failures are discarded as `stdout_write`'s are.
- **Names.** `printf`, `sprintf`, `eprintf` and `String::appendf` are items of
  `std`; a program's own of the same name takes precedence (D-0024),
  and the `std::` name always means this one.

**Depends on:** rule.stdlib.print, rule.stdlib.text, rule.stdlib.str,
rule.stdlib.string, rule.type.kind, rule.trust.extern-call, D-0038,
D-0039, D-0040
**Affects:** standard output; standard error (`eprintf`); `*s` (`String::appendf`); state.storage (`sprintf`'s `String`)

## 2g. Hash tables

In `std::collections`.

### `rule.stdlib.hashmap`
**Status:** ACCEPTED

`HashMap<K, V>` maps keys to values and `HashSet<K>` holds keys; both
are exported types of `std`, written in CobaltC below (D-0041). A **key
type** is an integer type, `bool`, `str` or `String`, or a struct whose
fields, or an enum whose payloads, are all key types (D-0110). Hashing and
equality depend on `K`, which a CobaltC body cannot inspect (D-0010),
so they are two std-only intrinsics realized natively, as
`append_native` is (§2d):

    [Key-Bytes]
        bytes(v : τ integer)   = v in two's complement, width(τ)/8 bytes, least significant first
        bytes(v : bool)        = one byte, 1 or 0
        bytes(v : str)         = its bytes;   bytes(v : String) = the bytes of v.bytes
        bytes(v : struct S)    = part(f1) … part(fn), its fields in declaration order   (D-0110)
        bytes(v : enum E)      = the variant's index as a u32 (as above), then part(payload) if any
            part(x) = bytes(x), except for text: its length as a u64, then its bytes

    [Key-Hash]    ⟨key_hash(r), Σ⟩ → ⟨h, Σ⟩     r : ref<K, shared> to v;  h : u64 = FNV-1a-64(bytes(v))
    [Key-Eq]      ⟨key_eq(r1, r2), Σ⟩ → ⟨b, Σ⟩   b = (bytes(v1) = bytes(v2))

        FNV-1a-64(b0..b_{n−1}):  h := 14695981039346656037;  for each bj: h := (h ⊕ bj) × 1099511628211 mod 2^64

`key_hash`'s value is not observable: it chooses only where an entry
sits in `slots`, and nothing a program can read depends on that.

**Bytes and checksums (D-0119).** `Vec::push_le`/`push_be` and their
inverses `read_le`/`read_be` are in `std::collections`; `crc32` is in
`std::crypto`, with the other checksums of bytes, though it is not
cryptographic (D-0177). For an integer type `T` of `n = sizeof<T>()`
bytes and a value `x`, let `u` be `x`'s two's-complement image as an
unsigned `8n`-bit number (`x` itself for an unsigned `T`; `x + 2^(8n)`
for a negative `x`), and `byte_k(u) = (u >> 8k) & 0xFF` for `k` in
`0 .. n`.

    [Bytes-LE]   Vec::push_le(&mut out, x)  pushes  byte_0(u), byte_1(u), …, byte_{n−1}(u)
                 read_le<T>(s, at)  =  the T whose image u has byte_k(u) = s[at + k]           -- at + n ≤ len(s), else diag.index-out-of-bounds
    [Bytes-BE]   Vec::push_be(&mut out, x)  pushes  byte_{n−1}(u), …, byte_0(u)
                 read_be<T>(s, at)  =  the T whose image u has byte_k(u) = s[at + n − 1 − k]
        -- so read_le<T>(&v[0..$], i) after Vec::push_le(&mut v, x) at position i gives x back, for every T and x; likewise be

    [CRC32]      crc32(data) : u32,  CRC-32/IEEE (IEEE 802.3; the checksum of zlib, gzip, PNG and Ethernet):
                     polynomial 0x04C11DB7, used reflected as 0xEDB88320; initial value 0xFFFFFFFF;
                     input reflected (bits taken least significant first); output reflected; final xor 0xFFFFFFFF.
                 Bit by bit, which is the definition and what std's body does:
                     crc := 0xFFFFFFFF
                     for each byte b of data, in order:
                         crc := crc xor b                       -- b as a u32, in the low eight bits
                         repeat 8 times:
                             if crc & 1 = 1 then crc := (crc >> 1) xor 0xEDB88320
                                            else crc := crc >> 1
                     result := crc xor 0xFFFFFFFF
                 Check values: crc32 of no bytes = 0x00000000; of the ASCII bytes "123456789" = 0xCBF43926.
                 An implementation may compute it by table or by hardware, but must give exactly this function.

    [Rng]        (D-0120)  state: four u64 words s0, s1, s2, s3.  All arithmetic below is on u64, wrapping (mod 2^64);
                 rotl(x, k) = (x << k) | (x >> (64 − k)).
                 Rng::new(seed): splitmix64 from `seed` gives s0, s1, s2, s3 in that order, where each step is
                     state := state + 0x9E3779B97F4A7C15
                     z := state;  z := (z xor (z >> 30)) × 0xBF58476D1CE4E5B9;  z := (z xor (z >> 27)) × 0x94D049BB133111EB
                     output z xor (z >> 31)
                 Rng::next_u64(r): xoshiro256**:
                     result := rotl(s1 × 5, 7) × 9
                     t := s1 << 17;  s2 := s2 xor s0;  s3 := s3 xor s1;  s1 := s1 xor s2;  s0 := s0 xor s3;  s2 := s2 xor t;  s3 := rotl(s3, 45)
                     output result
                 Rng::below(r, n): reject := (2^64 − n) mod n  (n = 0: diag.div-by-zero);  x := next_u64(r);
                     while x < reject: x := next_u64(r);   output x mod n        -- every value in 0 .. n equally likely
                 Rng::unit_f64(r): (next_u64(r) >> 11) as an f64, divided by 2^53          -- 0.0 ≤ x < 1.0, exact
                 Check values: seed 42 → next_u64: 1546998764402558742, 6990951692964543102, 12544586762248559009;
                     seed 0 → next_u64: 11091344671253066420;
                     seed 42 → below(10), below(10), below(6), below(1000000007), below(1): 2, 2, 5, 779106270, 0;
                     seed 42 → unit_f64: 0.083862971059882163 (printed `%.17g`).
                 The sequence is normative: an implementation computes exactly it. It is a general-purpose generator and
                 not suitable for cryptographic purposes: an observer of a few outputs can recover its state.

    [Os-Random]  (D-0142)  disposition: checked   outcome: unspecified { any bytes }
                 os_random_bytes(out): every byte of `out` set from the operating system's cryptographically secure
                     source (Linux getrandom(2), else /dev/urandom; Windows BCryptGenRandom); an empty `out` is left
                     as it is. os_random_u64(): the u64 whose little-endian bytes are 8 such bytes.
                 When the system gives no such bytes: ↛ diag.entropy-unavailable (never a clock or a fixed seed instead).
                 The bytes are suitable for keys, tokens and nonces.
    [Rng-From-Os] (D-0142) Rng::from_os() = Rng::new(os_random_u64()): an Rng whose seed is unpredictable; its
                 sequence is still [Rng]'s, so it is still not for cryptographic purposes.

    [Lookup-Str]   (D-0113)
        m : HashMap<String, V> (or s : HashSet<String>),  k : str,  k' : String with bytes(k') = bytes(k)
        ────────────────────────────────────────────
        HashMap::get_str(&m, k)      ≡  HashMap::get(&m, &k')            and likewise get_mut_str, contains_str,
        HashMap::remove_str(&mut m, k) ≡  HashMap::remove(&mut m, &k')     HashSet::contains_str, HashSet::remove_str
        -- the same slot: [Key-Bytes] gives a str and a String of one text the same bytes, so [Key-Hash] the same
        -- hash; the entry is found by comparing bytes (String::eq_str); no String is constructed


    [Set-Ops]   (D-0138)   a, b : HashSet<K>, K a key type that is clone;  keys(s) = s's keys in insertion order
        keys(HashSet::union(&a, &b))        = keys(a), then the k ∈ keys(b) with k ∉ a, in keys(b)'s order
        keys(HashSet::intersection(&a, &b)) = the k ∈ keys(a) with k ∈ b, in keys(a)'s order
        keys(HashSet::difference(&a, &b))   = the k ∈ keys(a) with k ∉ b, in keys(a)'s order
        -- each key of the result a copy, clone(&k); a and b are read through shared references and unchanged


    [Key-Not-Hashable]   disposition: rejected
        a call of a HashMap or HashSet function whose K is not a key type
        (a type parameter: at each instantiation, rule.type.kind)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    // Puts x in slot i of v and returns what was there. Private to `std`:
    // `HashMap::insert` replaces a value with it.
    fn Vec::replace<T>(ref<Vec<T>, exclusive> v, usize i, T x) : T
    {
        if (i >= v.len)
        {
            fault(index_out_of_bounds);
        }
        unsafe
        {
            rawptr<T> p = v.ptr + reinterpret<isize>(i);
            auto old = *p;
            release(reinterpret_ptr<u8>(p), sizeof<T>());
            *p = x;
            old
        }
    }

    // Takes element i out of v; the elements after it keep their order
    // (they come off the end and go back on). Private to `std`:
    // `HashMap::remove` and `swap_remove` use it.
    fn Vec::remove_at<T>(ref<Vec<T>, exclusive> v, usize i) : T
    {
        if (i >= v.len)
        {
            fault(index_out_of_bounds);
        }
        Vec<T> later = Vec::new();
        while (v.len > i + 1)
        {
            match (Vec::pop(v))
            {
                Some(x) :
                {
                    Vec::push(&mut later, x);
                },
                None :
                {
                },
            }
        }
        auto taken = match (Vec::pop(v))
        {
            Some(x) : x,
            None : fault(index_out_of_bounds),
        };
        while (Vec::len(&later) > 0)
        {
            match (Vec::pop(&mut later))
            {
                Some(x) :
                {
                    Vec::push(v, x);
                },
                None :
                {
                },
            }
        }
        taken
    }

    // Moves element i out of v, leaving its slot empty. Private to `std`:
    // only a holder that sets `len` to 0 before the Vec is destroyed may use
    // it (`Drain`, `HashDrain`).
    fn Vec::take_raw<T>(ref<Vec<T>, exclusive> v, usize i) : T
    {
        unsafe
        {
            rawptr<T> p = v.ptr + reinterpret<isize>(i);
            auto x = *p;
            release(reinterpret_ptr<u8>(p), sizeof<T>());
            x
        }
    }

    // Empties v: its elements are destroyed in order, first to last, as
    // the Vec's own destructor would; its buffer is kept for reuse (D-0044).
    export fn Vec::clear<T>(ref<Vec<T>, exclusive> v)
    {
        usize i = 0;
        while (i < v.len)
        {
            drop(Vec::take_raw(v, i));
            i = i + 1;
        }
        v.len = 0;
    }

    // A consumed Vec that `foreach` takes elements from in order
    // (`rule.control.foreach`, spec/14, D-0042). The elements not yet taken
    // when the loop ends are destroyed with it.
    resource struct Drain<T>
    {
        Vec<T> v;
        usize next;
    }

    fn Vec::drain<T>(Vec<T> v) : Drain<T>
    {
        Drain { .v = v, .next = 0 }
    }

    fn Drain::total<T>(ref<Drain<T>, shared> d) : usize
    {
        d.v.len
    }

    fn Drain::take<T>(ref<Drain<T>, exclusive> d) : T
    {
        auto x = Vec::take_raw(&mut d.v, d.next);
        d.next = d.next + 1;
        x
    }

    fn Drain::drop<T>(ref<Drain<T>, exclusive> self)
    {
        while (self.next < self.v.len)
        {
            drop(Vec::take_raw(&mut self.v, self.next));
            self.next = self.next + 1;
        }
        self.v.len = 0;                 // every element is gone: the Vec frees only its buffer
    }

    // Hash tables (`rule.stdlib.hashmap`, spec/21 §1a, D-0041). A key type
    // is an integer type, `bool`, `str` or `String`; the std-only intrinsics
    // `key_hash` (FNV-1a over the key's bytes) and `key_eq` realize hashing
    // and equality for each, and the front end checks `K` at every call.
    // Keys and values are kept in insertion order, entry i in keys[i] and
    // values[i], and the table holds their indices, so iteration is in
    // insertion order.
    export struct HashMap<K, V>
    {
        Vec<K> keys;                        // in insertion order
        Vec<V> values;                      // values[i] belongs to keys[i]
        Vec<usize> slots;                   // an entry index, or max_value<usize>() when empty
    }

    export fn HashMap::new<K, V>() : HashMap<K, V>
    {
        Vec<usize> slots = Vec::new();
        usize i = 0;
        while (i < 8)
        {
            Vec::push(&mut slots, max_value<usize>());
            i = i + 1;
        }
        HashMap { .keys = Vec::new(), .values = Vec::new(), .slots = slots }
    }

    export fn HashMap::clear<K, V>(ref<HashMap<K, V>, exclusive> m)
    {
        Vec::clear(&mut m.values);
        Vec::clear(&mut m.keys);
        Vec::clear(&mut m.slots);
        usize i = 0;
        while (i < 8)
        {
            Vec::push(&mut m.slots, max_value<usize>());
            i = i + 1;
        }
    }

    export fn HashMap::len<K, V>(ref<HashMap<K, V>, shared> m) : usize
    {
        Vec::len(&m.keys)
    }

    // Entry i in insertion order, for i < len.
    export fn HashMap::key_at<K, V>(ref<HashMap<K, V>, shared> m, usize i) : ref<K, shared>
    {
        Vec::index_shared(&m.keys, i)
    }

    export fn HashMap::value_at<K, V>(ref<HashMap<K, V>, shared> m, usize i) : ref<V, shared>
    {
        Vec::index_shared(&m.values, i)
    }

    // The slot holding k, or the empty slot where it belongs (linear
    // probing; the table is never full).
    fn HashMap::find<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : usize
    {
        usize mask = Vec::len(&m.slots) - 1;
        usize i = narrow_wrapping<usize>(key_hash(k)) & mask;
        bool searching = true;
        while (searching)
        {
            usize e = *Vec::index_shared(&m.slots, i);
            if (e == max_value<usize>())
            {
                searching = false;
            }
            else if (key_eq(Vec::index_shared(&m.keys, e), k))
            {
                searching = false;
            }
            else
            {
                i = (i + 1) & mask;
            }
        }
        i
    }

    // The entry index for k, or max_value<usize>() when k is absent.
    fn HashMap::index_of<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : usize
    {
        *Vec::index_shared(&m.slots, HashMap::find(m, k))
    }

    // Places every entry again in a table of n slots (n a power of two, at
    // least the current size).
    fn HashMap::reslot<K, V>(ref<HashMap<K, V>, exclusive> m, usize n)
    {
        usize old = Vec::len(&m.slots);
        usize i = 0;
        while (i < n)
        {
            if (i < old)
            {
                *Vec::index_exclusive(&mut m.slots, i) = max_value<usize>();
            }
            else
            {
                Vec::push(&mut m.slots, max_value<usize>());
            }
            i = i + 1;
        }
        usize e = 0;
        while (e < Vec::len(&m.keys))
        {
            usize s = HashMap::find(&*m, Vec::index_shared(&m.keys, e));
            *Vec::index_exclusive(&mut m.slots, s) = e;
            e = e + 1;
        }
    }

    // Room for one more entry: the table stays at most three-quarters full.
    fn HashMap::reserve_one<K, V>(ref<HashMap<K, V>, exclusive> m)
    {
        usize n = Vec::len(&m.slots);
        if ((Vec::len(&m.keys) + 1) * 4 > n * 3)
        {
            HashMap::reslot(m, n * 2);
        }
    }

    // Adds k with value v. If k was present, its value is replaced where it
    // stands and the old one returned; the stored key stays and k is
    // destroyed.
    export fn HashMap::insert<K, V>(ref<HashMap<K, V>, exclusive> m, K k, V v) : Option<V>
    {
        HashMap::reserve_one(m);
        usize s = HashMap::find(&*m, &k);
        usize e = *Vec::index_shared(&m.slots, s);
        if (e == max_value<usize>())
        {
            *Vec::index_exclusive(&mut m.slots, s) = Vec::len(&m.keys);
            Vec::push(&mut m.keys, k);
            Vec::push(&mut m.values, v);
            return None;
        }
        Some(Vec::replace(&mut m.values, e, v))
    }

    // The value for k. If k is absent, it is added with `fresh` first; if it
    // is present, `k` and `fresh` are not needed and are destroyed here.
    export fn HashMap::entry<K, V>(ref<HashMap<K, V>, exclusive> m, K k, V fresh) : ref<V, exclusive>
    {
        HashMap::reserve_one(m);
        usize s = HashMap::find(&*m, &k);
        usize e = *Vec::index_shared(&m.slots, s);
        if (e == max_value<usize>())
        {
            e = Vec::len(&m.keys);
            *Vec::index_exclusive(&mut m.slots, s) = e;
            Vec::push(&mut m.keys, k);
            Vec::push(&mut m.values, fresh);
        }
        Vec::index_exclusive(&mut m.values, e)
    }

    export fn HashMap::get<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : Option<ref<V, shared>>
    {
        usize e = HashMap::index_of(m, k);
        if (e == max_value<usize>())
        {
            return None;
        }
        Some(Vec::index_shared(&m.values, e))
    }

    export fn HashMap::get_mut<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<ref<V, exclusive>>
    {
        usize e = HashMap::index_of(&*m, k);
        if (e == max_value<usize>())
        {
            return None;
        }
        Some(Vec::index_exclusive(&mut m.values, e))
    }

    export fn HashMap::contains<K, V>(ref<HashMap<K, V>, shared> m, ref<K, shared> k) : bool
    {
        HashMap::index_of(m, k) != max_value<usize>()
    }

    // Empties slot s, moving later slots of its probe run back so that
    // every entry stays reachable from its home slot (backward-shift
    // deletion: the table needs no tombstones).
    fn HashMap::unslot<K, V>(ref<HashMap<K, V>, exclusive> m, usize s)
    {
        usize mask = Vec::len(&m.slots) - 1;
        usize i = s;
        usize j = s;
        bool shifting = true;
        while (shifting)
        {
            j = (j + 1) & mask;
            usize e = *Vec::index_shared(&m.slots, j);
            if (e == max_value<usize>())
            {
                shifting = false;
            }
            else
            {
                usize home = narrow_wrapping<usize>(key_hash(Vec::index_shared(&m.keys, e))) & mask;
                // The entry in j may move back to i unless its home lies in (i, j], cyclically.
                bool stays = if (i <= j)
                {
                    home > i && home <= j
                }
                else
                {
                    home > i || home <= j
                };
                if (!stays)
                {
                    *Vec::index_exclusive(&mut m.slots, i) = e;
                    i = j;
                }
            }
        }
        *Vec::index_exclusive(&mut m.slots, i) = max_value<usize>();
    }

    // Removes k and returns its value, in constant time: the last entry
    // moves into k's place, so insertion order holds only up to the first
    // such removal. `remove` keeps the order.
    export fn HashMap::swap_remove<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<V>
    {
        usize s = HashMap::find(&*m, k);
        usize e = *Vec::index_shared(&m.slots, s);
        if (e == max_value<usize>())
        {
            return None;
        }
        usize last = Vec::len(&m.keys) - 1;
        if (e != last)
        {
            // The last entry's slot names e from now on.
            usize t = HashMap::find(&*m, Vec::index_shared(&m.keys, last));
            *Vec::index_exclusive(&mut m.slots, t) = e;
        }
        auto k_last = Vec::remove_at(&mut m.keys, last);
        auto v_last = Vec::remove_at(&mut m.values, last);
        auto v = if (e != last)
        {
            drop(Vec::replace(&mut m.keys, e, k_last));
            Vec::replace(&mut m.values, e, v_last)
        }
        else
        {
            drop(k_last);
            v_last
        };
        HashMap::unslot(m, s);
        Some(v)
    }

    // Removes k and returns its value; the other entries keep their order.
    // It takes time in proportion to the size of the map (`swap_remove`
    // takes constant time, and moves the last entry into k's place).
    export fn HashMap::remove<K, V>(ref<HashMap<K, V>, exclusive> m, ref<K, shared> k) : Option<V>
    {
        usize s = HashMap::find(&*m, k);
        HashMap::remove_slot(m, s)
    }

    // The entry slot s names taken out, the others keeping their order;
    // None when s is empty. `remove` and `remove_str` find s.
    fn HashMap::remove_slot<K, V>(ref<HashMap<K, V>, exclusive> m, usize s) : Option<V>
    {
        usize e = *Vec::index_shared(&m.slots, s);
        if (e == max_value<usize>())
        {
            return None;
        }
        HashMap::unslot(m, s);
        drop(Vec::remove_at(&mut m.keys, e));
        auto v = Vec::remove_at(&mut m.values, e);
        // Entries after e moved down by one.
        usize i = 0;
        while (i < Vec::len(&m.slots))
        {
            usize x = *Vec::index_shared(&m.slots, i);
            if (x != max_value<usize>() && x > e)
            {
                *Vec::index_exclusive(&mut m.slots, i) = x - 1;
            }
            i = i + 1;
        }
        Some(v)
    }

    // A consumed HashMap that `foreach (k, v in m)` takes entries from, key
    // then value, in order; the entries not yet taken are destroyed with it.
    resource struct HashDrain<K, V>
    {
        HashMap<K, V> m;
        usize next;
    }

    fn HashMap::drain<K, V>(HashMap<K, V> m) : HashDrain<K, V>
    {
        HashDrain { .m = m, .next = 0 }
    }

    fn HashDrain::total<K, V>(ref<HashDrain<K, V>, shared> d) : usize
    {
        d.m.keys.len
    }

    fn HashDrain::take_key<K, V>(ref<HashDrain<K, V>, exclusive> d) : K
    {
        Vec::take_raw(&mut d.m.keys, d.next)
    }

    fn HashDrain::take_value<K, V>(ref<HashDrain<K, V>, exclusive> d) : V
    {
        auto v = Vec::take_raw(&mut d.m.values, d.next);
        d.next = d.next + 1;
        v
    }

    fn HashDrain::drop<K, V>(ref<HashDrain<K, V>, exclusive> self)
    {
        while (self.next < self.m.keys.len)
        {
            drop(Vec::take_raw(&mut self.m.keys, self.next));
            drop(Vec::take_raw(&mut self.m.values, self.next));
            self.next = self.next + 1;
        }
        self.m.keys.len = 0;
        self.m.values.len = 0;
    }

    // A set of keys: a HashMap whose values are not used.
    export struct HashSet<K>
    {
        HashMap<K, bool> map;
    }

    export fn HashSet::new<K>() : HashSet<K>
    {
        HashSet { .map = HashMap::new() }
    }

    export fn HashSet::len<K>(ref<HashSet<K>, shared> s) : usize
    {
        HashMap::len(&s.map)
    }

    // Adds k; true if it was not already present.
    export fn HashSet::insert<K>(ref<HashSet<K>, exclusive> s, K k) : bool
    {
        match (HashMap::insert(&mut s.map, k, true))
        {
            Some(_) : false,
            None : true,
        }
    }

    export fn HashSet::contains<K>(ref<HashSet<K>, shared> s, ref<K, shared> k) : bool
    {
        HashMap::contains(&s.map, k)
    }

    // Removes k; true if it was present. As `HashMap::swap_remove`, the
    // last key takes k's place.
    export fn HashSet::swap_remove<K>(ref<HashSet<K>, exclusive> s, ref<K, shared> k) : bool
    {
        match (HashMap::swap_remove(&mut s.map, k))
        {
            Some(_) : true,
            None : false,
        }
    }

    // Removes k keeping the order of the others; true if it was present.
    export fn HashSet::remove<K>(ref<HashSet<K>, exclusive> s, ref<K, shared> k) : bool
    {
        match (HashMap::remove(&mut s.map, k))
        {
            Some(_) : true,
            None : false,
        }
    }

    // A consumed HashSet that `foreach (k in s)` takes keys from.
    struct SetDrain<K>
    {
        HashDrain<K, bool> d;
    }

    fn HashSet::drain<K>(HashSet<K> s) : SetDrain<K>
    {
        HashSet { map } = s;
        SetDrain { .d = HashMap::drain(map) }
    }

    fn SetDrain::total<K>(ref<SetDrain<K>, shared> s) : usize
    {
        HashDrain::total(&s.d)
    }

    fn SetDrain::take<K>(ref<SetDrain<K>, exclusive> s) : K
    {
        auto k = HashDrain::take_key(&mut s.d);
        HashDrain::take_value(&mut s.d);
        k
    }

    // Key i in insertion order, for i < len.
    export fn HashSet::key_at<K>(ref<HashSet<K>, shared> s, usize i) : ref<K, shared>
    {
        HashMap::key_at(&s.map, i)
    }

    // D-0134: every key removed, as `HashMap::clear`.
    export fn HashSet::clear<K>(ref<HashSet<K>, exclusive> s)
    {
        HashMap::clear(&mut s.map);
    }

    // D-0138: the set operations. Each makes a new set, copying the keys it
    // keeps with `clone`, and leaves `a` and `b` as they are. The keys keep
    // `a`'s order; `union` then adds `b`'s keys that are not in `a`, in `b`'s
    // order.
    export fn HashSet::union<K: clone>(ref<HashSet<K>, shared> a, ref<HashSet<K>, shared> b) : HashSet<K>
    {
        HashSet<K> out = HashSet::clone(a);
        foreach (i in 0..HashSet::len(b))
        {
            ref<K, shared> k = HashSet::key_at(b, i);
            if (!HashSet::contains(a, k))
            {
                HashSet::insert(&mut out, clone(k));
            }
        }
        out
    }

    // The keys of `a` that are also in `b`.
    export fn HashSet::intersection<K: clone>(ref<HashSet<K>, shared> a, ref<HashSet<K>, shared> b) : HashSet<K>
    {
        HashSet<K> out = HashSet::new();
        foreach (i in 0..HashSet::len(a))
        {
            ref<K, shared> k = HashSet::key_at(a, i);
            if (HashSet::contains(b, k))
            {
                HashSet::insert(&mut out, clone(k));
            }
        }
        out
    }

    // The keys of `a` that are not in `b`.
    export fn HashSet::difference<K: clone>(ref<HashSet<K>, shared> a, ref<HashSet<K>, shared> b) : HashSet<K>
    {
        HashSet<K> out = HashSet::new();
        foreach (i in 0..HashSet::len(a))
        {
            ref<K, shared> k = HashSet::key_at(a, i);
            if (!HashSet::contains(b, k))
            {
                HashSet::insert(&mut out, clone(k));
            }
        }
        out
    }

- **Order.** Entry `i` (`key_at`, `value_at`, `HashSet::key_at`, for
  `i < len`) is in insertion order. Replacing a key's value keeps its
  place. `remove` (and `remove_str`) keeps every other entry's place and
  takes time in proportion to the map's size; `swap_remove` takes
  constant time by moving the last entry into the removed one's place
  (D-0134). `HashSet::clear` empties a set as `HashMap::clear` empties a
  map. The set operations (`[Set-Ops]`, D-0138) keep the first set's
  order, `union` adding the second's new keys after it. The order depends
  only on the calls, never on hash values, so it is the same in every
  implementation.
- **Ownership.** The map owns its keys and values and destroys them
  with itself. `insert` of a present key returns the old value and
  destroys the key passed in; `entry` of a present key destroys both
  its key and `fresh`.
- **Lookups borrow the key** (`ref<K, shared>`), so a lookup copies
  nothing. In a `HashMap<String, V>`, a literal key is looked up with
  the `_str` functions (`[Lookup-Str]`), which make no `String`.
- **Not keys:** `f32`/`f64` (a NaN is not equal to itself; `0.0` and
  `-0.0` are equal but differ in bytes), references, and composite
  types; a revisit condition of D-0041.
- **Names.** `HashMap`, `HashSet` and their functions are items of
  `std`; `Vec::clear` (D-0044) is too; the fields, `Vec::replace`,
  `Vec::remove_at`, `Vec::take_raw`,
  the holders `Drain`, `HashDrain` and `SetDrain` that a consuming
  `foreach` uses (`rule.control.foreach`, spec/14), and the other helpers
  without `export` are `std`'s own.

**Depends on:** rule.stdlib.vec, rule.stdlib.string, rule.stdlib.str,
rule.type.kind, D-0041
**Affects:** state.storage (through `Vec`)

## 2h. `StringView`

In `std::text`.

### `rule.stdlib.stringview`
**Status:** ACCEPTED

A read-only view of part of a `String`'s text, without copying it
(D-0053). `std` defines it as a struct holding a `slice<u8, shared>` of
the `String`'s bytes; the language adds three forms over it.

    [View-Form]
        s a place of type String (through a reference too), lo, hi : usize, `$` = the length of s in bytes
        ────────────────────────────────────────────
        &s[lo .. hi] : StringView  ≡  String::view(&s, lo, hi)        -- s borrowed shared, as `&s` borrows it
        &v[lo .. hi] : StringView  ≡  StringView::sub(v, lo, hi)      -- v a StringView (any value); `$` its length

    [View-Form-Temporary-Argument]                                    -- D-0135
        &s[lo .. hi] an argument of a call whose result type is not a reference, s of type
        String a temporary (a call's result) or part of one (spec/16 [Temp-Root])
        ────────────────────────────────────────────
        the view is formed as for a place; the String is a temporary of the current statement
        and ends at its exit, after the call (spec/09 [Ref-Form-Temporary-Part-Argument])

    [View-Boundary]   disposition: checked
        lo or hi is not the start of a character (a byte 0b10xxxxxx), the end, or 0
        ────────────────────────────────────────────
        ⟨&s[lo .. hi], Σ⟩ ↛ diag.not-char-boundary
        (lo > hi or hi beyond the length: diag.index-out-of-bounds, as for a slice)

    [View-Eq]
        a : StringView, b : StringView or str (either side)
        ────────────────────────────────────────────
        a == b  ≡  StringView::eq(a, b) / StringView::eq_str(a, b);   a != b  ≡  !(a == b)

    [Text-Mixed-Eq]   (D-0164)
        s : String, b : str or StringView (either side)
        ────────────────────────────────────────────
        s == b  ≡  String::eq_str(&s, b) / String::eq_view(&s, b);   s != b  ≡  !(s == b)
        -- s is borrowed shared, not moved; `<`, `<=`, `>`, `>=` stay within one text type (D-0108)

    [View-As-Bytes]   (D-0163)
        ⟨StringView::as_bytes(v), Σ⟩ → v's bytes as a slice<u8, shared>, not a copy, valid while v is

    [View-Mode-Rejected]   disposition: rejected
        &mut s[lo .. hi] with s a String or a StringView
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

A `String` a call returns may be viewed as an argument, as D-0103 lets
it be sliced (`[View-Form-Temporary-Argument]`, D-0135):
`read_file(&sprintf("%s/%s", dir, name)[0..$])` needs no binding.
Anywhere else a call's result is not a place and `&f()[lo .. hi]` is
`diag.borrow-of-non-place`; a view a call returns from such an argument
(`StringView::trim(&f()[0..$])`) and keeps past the statement is
`diag.destroy-while-aliased` when the `String` ends.

A view borrows its `String` as a `slice<u8, shared>` of it does: while
it lives the `String` cannot be changed (`diag.aliasing-conflict`), and
it cannot outlive it (`diag.reference-escapes-scope`, or
`diag.stale-binding` where the static analysis does not follow it). A
`StringView` counts as a borrowed type for `rule.temporal.elision`, so a
function returning one takes exactly one borrowed parameter — except
`StringView::of`. It is
printable (`%s`, `%v`, `String::append`), and no other operator applies
to it. Positions are byte positions; white space for `trim` is ASCII.

`StringView::of(t)` (D-0134) is a view of a `str`'s bytes. A `str` is a
value that never ends (`type.str`), so the object the view borrows never
ends either: such a view is valid for the rest of the program, and
borrowing it forbids nothing, since no program can change a `str`.
`StringView::of` is the one function returning a `StringView` with no
borrowed parameter. A literal where a `StringView` is expected is this
call (`[Str-Literal-View]`, §2a).

The text `find`, `starts_with` and `ends_with` look for is a
`StringView` (D-0134): a literal, a `String`'s `&s[0..$]`, or another
view. `split`'s separator is a `str`, because `split` returns views of
its first argument and so may take no other borrowed parameter.

`StringView::contains(v, t)` (D-0139, `[Contains]`) is whether
`StringView::find(v, t)` is `Some`; the empty text is in every text.
`StringView::replace(v, from, to)` (`[Replace]`) is a new `String`: `v`
with every occurrence of `from` replaced by `to`, the occurrences found
left to right and not overlapping (`"aaa"` with `"aa"` replaced by `"b"`
is `"ba"`); an empty `from` occurs nowhere, so the result is a copy of
`v`. Each occurrence is whole characters of `v`, since `from` is UTF-8,
so the result is UTF-8. `StringView::join(parts, sep)` (`[Join]`) is the
texts of the views in `parts`, in order, with `sep` between each two, in
a new `String`; no parts give an empty `String`; `String::join` is the
same for a slice of `String`s. `replace` and `join` return a `String`
that borrows nothing, so they may take several borrowed parameters.
`String::contains(&s, t)` and `String::replace(&s, from, to)` are the
`StringView::` functions of `&s[0..$]`.

`StringView::chars(v)` (D-0128) is the text's characters, in order, one
view per character: a character starts at byte 0 and at every byte that
is not a UTF-8 continuation byte (`0b10xxxxxx`), and runs to the next
start or the end, so each view is one to four bytes and together they
cover the text exactly once; empty text gives an empty `Vec`.
`String::chars(&s)` is `StringView::chars(&s[0..$])`.
`StringView::char_count(v) : usize` (D-0193) is how many characters that
is (the bytes that are not continuation bytes), without the `Vec`;
`String::char_count(&s)` is `StringView::char_count(&s[0..$])`. A character is a
Unicode scalar value (`"e\u{301}"` is two). The `Vec` is the caller's;
the views borrow what the argument borrows, as any view does. Building
the `Vec` is one allocation: `foreach` visits only the collections and
ranges the language defines, and a library type has no way to supply
the next element one step at a time (there is no iterator protocol), so
an allocation-free `foreach` over characters is not possible (D-0128
gives the reasons in full); a program that must not allocate walks
`String::as_bytes(&s)` instead.

    // D-0053: a borrowed, read-only view of part of a `String`'s text, made
    // by `&s[lo .. hi]` (`String::view`) and re-sliced by `&v[lo .. hi]`
    // (`StringView::sub`). Its bounds fall on character boundaries, so its
    // bytes are UTF-8. The `String` stays borrowed while a view of it lives.
    export struct StringView
    {
        slice<u8, shared> bytes;
    }

    // Whether byte position `i` of `b` starts a character (or is its end).
    fn StringView::boundary(slice<u8, shared> b, usize i) : bool
    {
        i == 0 || i == slice_len(b) || (b[i] & 0xC0: u8) != 0x80: u8
    }

    // D-0134: a view of a `str`'s text. A `str` is a value that never ends,
    // so the view is valid for the rest of the program; a literal where a
    // `StringView` is expected is this call (`[Str-Literal-View]`).
    export fn StringView::of(str s) : StringView
    {
        StringView { .bytes = str_slice(s) }
    }

    // The address of a view's first byte, for the file primitives; any
    // address when the view is empty (they then read none).
    fn StringView::ptr(StringView v) : rawptr<u8>
    {
        if (slice_len(v.bytes) == 0)
        {
            return dangling<u8>();
        }
        rawptr_of(&v.bytes[0])
    }

    export fn String::view(ref<String, shared> s, usize lo, usize hi) : StringView
    {
        slice<u8, shared> all = &s.bytes[0..$];
        if (lo > hi || hi > slice_len(all))
        {
            fault(index_out_of_bounds);
        }
        if (!StringView::boundary(all, lo) || !StringView::boundary(all, hi))
        {
            fault(not_char_boundary);
        }
        StringView { .bytes = &all[lo..hi] }
    }

    export fn StringView::sub(StringView v, usize lo, usize hi) : StringView
    {
        if (lo > hi || hi > slice_len(v.bytes))
        {
            fault(index_out_of_bounds);
        }
        if (!StringView::boundary(v.bytes, lo) || !StringView::boundary(v.bytes, hi))
        {
            fault(not_char_boundary);
        }
        StringView { .bytes = &v.bytes[lo..hi] }
    }

    export fn StringView::len(StringView v) : usize
    {
        slice_len(v.bytes)
    }

    export fn StringView::eq(StringView a, StringView b) : bool
    {
        usize n = slice_len(a.bytes);
        if (n != slice_len(b.bytes))
        {
            return false;
        }
        for (usize i = 0; i < n; i += 1)
        {
            if (a.bytes[i] != b.bytes[i])
            {
                return false;
            }
        }
        true
    }

    // D-0163: the view's bytes, the same bytes, not a copy, for whatever
    // takes `slice<u8, shared>` (a hash, a MAC, a socket's write): valid as
    // long as the view is. A literal in a `StringView` position is a view, and
    // `String::as_view(&s)` a `String`'s, so every text type reaches it.
    export fn StringView::as_bytes(StringView v) : slice<u8, shared>
    {
        v.bytes
    }

    // D-0164: whether a `String` holds the same text as a `StringView`, as
    // `s == v` writes it.
    export fn String::eq_view(ref<String, shared> a, StringView v) : bool
    {
        StringView::eq(String::as_view(a), v)
    }

    export fn StringView::eq_str(StringView a, str t) : bool
    {
        usize n = slice_len(a.bytes);
        if (n != str_len(t))
        {
            return false;
        }
        for (usize i = 0; i < n; i += 1)
        {
            if (a.bytes[i] != str_byte(t, i))
            {
                return false;
            }
        }
        true
    }

    // Whether `t` occurs in `v` at byte position `at`.
    fn StringView::at(StringView v, usize at, StringView t) : bool
    {
        usize m = slice_len(t.bytes);
        if (at + m > slice_len(v.bytes))
        {
            return false;
        }
        for (usize j = 0; j < m; j += 1)
        {
            if (v.bytes[at + j] != t.bytes[j])
            {
                return false;
            }
        }
        true
    }

    // The byte position of the first occurrence of `t` in `v`.
    export fn StringView::find(StringView v, StringView t) : Option<usize>
    {
        usize n = slice_len(v.bytes);
        usize m = slice_len(t.bytes);
        if (m > n)
        {
            return None;
        }
        for (usize i = 0; i <= n - m; i += 1)
        {
            if (StringView::at(v, i, t))
            {
                return Some(i);
            }
        }
        None
    }

    export fn StringView::starts_with(StringView v, StringView t) : bool
    {
        StringView::at(v, 0, t)
    }

    export fn StringView::ends_with(StringView v, StringView t) : bool
    {
        usize n = slice_len(v.bytes);
        usize m = slice_len(t.bytes);
        m <= n && StringView::at(v, n - m, t)
    }

    // Whether the text of `t` occurs in `v`.
    export fn StringView::contains(StringView v, StringView t) : bool
    {
        StringView::find(v, t) != None
    }

    // ASCII white space: space, tab, line feed, vertical tab, form feed, return.
    fn StringView::space(u8 b) : bool
    {
        b == b' ' || (b >= 9: u8 && b <= 13: u8)
    }

    export fn StringView::trim_start(StringView v) : StringView
    {
        usize n = slice_len(v.bytes);
        usize i = 0;
        while (i < n && StringView::space(v.bytes[i]))
        {
            i += 1;
        }
        StringView::sub(v, i, n)
    }

    export fn StringView::trim_end(StringView v) : StringView
    {
        usize n = slice_len(v.bytes);
        while (n > 0 && StringView::space(v.bytes[n - 1]))
        {
            n -= 1;
        }
        StringView::sub(v, 0, n)
    }

    export fn StringView::trim(StringView v) : StringView
    {
        // `trim_end(trim_start(v))`: the same range, taken once.
        usize n = slice_len(v.bytes);
        usize i = 0;
        while (i < n && StringView::space(v.bytes[i]))
        {
            i += 1;
        }
        while (n > i && StringView::space(v.bytes[n - 1]))
        {
            n -= 1;
        }
        StringView::sub(v, i, n)
    }

    // The parts of `v` between occurrences of `sep`, each a view of the same
    // `String`; an empty `sep` gives `v` whole.
    export fn StringView::split(StringView v, str sep) : Vec<StringView>
    {
        Vec<StringView> out = Vec::new();
        usize n = slice_len(v.bytes);
        usize m = str_len(sep);
        if (m == 0)
        {
            Vec::push(&mut out, v);
            return out;
        }
        StringView sv = StringView::of(sep);
        usize start = 0;
        usize i = 0;
        while (i + m <= n)
        {
            if (StringView::at(v, i, sv))
            {
                Vec::push(&mut out, StringView::sub(v, start, i));
                i += m;
                start = i;
            }
            else
            {
                i += 1;
            }
        }
        Vec::push(&mut out, StringView::sub(v, start, n));
        out
    }

    // D-0193: how many characters (code points) `v` holds; `len` counts bytes.
    // Each character starts with a byte that is not a continuation byte
    // (`10xxxxxx`), the view's bytes being UTF-8 (`[View-Form]`).
    export fn StringView::char_count(StringView v) : usize
    {
        usize n = 0;
        for (usize i = 0; i < slice_len(v.bytes); i += 1)
        {
            if ((v.bytes[i] & 0xc0) != 0x80)
            {
                n += 1;
            }
        }
        n
    }

    // D-0128: the characters of `v`, in order, each a view of that one
    // character's bytes (one to four): a character starts at every byte that
    // is not a UTF-8 continuation byte (0b10xxxxxx). Empty text gives no
    // views. The `Vec` is new; the views borrow what `v` borrows.
    export fn StringView::chars(StringView v) : Vec<StringView>
    {
        Vec<StringView> out = Vec::new();
        usize n = slice_len(v.bytes);
        usize start = 0;
        foreach (i in 1..n + 1)
        {
            if (StringView::boundary(v.bytes, i))
            {
                Vec::push(&mut out, StringView { .bytes = &v.bytes[start..i] });
                start = i;
            }
        }
        out
    }

    // A new `String`: `v` with every occurrence of `from` replaced by `to`.
    // Occurrences are found left to right and do not overlap (`"aaa"` with
    // `"aa"` replaced is one replacement and an `a`); an empty `from`
    // occurs nowhere, so the result is a copy of `v`.
    export fn StringView::replace(StringView v, StringView from, StringView to) : String
    {
        usize n = slice_len(v.bytes);
        usize m = slice_len(from.bytes);
        if (m == 0)
        {
            return String::from_view(v);
        }
        String out = String::new();
        usize start = 0;
        usize i = 0;
        while (i + m <= n)
        {
            if (StringView::at(v, i, from))
            {
                String::append_view(&mut out, StringView { .bytes = &v.bytes[start..i] });
                String::append_view(&mut out, to);
                i += m;
                start = i;
            }
            else
            {
                i += 1;
            }
        }
        String::append_view(&mut out, StringView { .bytes = &v.bytes[start..n] });
        out
    }

    // The texts of `parts`, in order, with `sep` between each two, in a new
    // `String`; no parts give an empty `String`.
    export fn StringView::join(slice<StringView, shared> parts, StringView sep) : String
    {
        String out = String::new();
        foreach (i, p in parts)
        {
            if (i > 0)
            {
                String::append_view(&mut out, sep);
            }
            String::append_view(&mut out, *p);
        }
        out
    }

    // `StringView::join` for `String`s: the texts of `parts` joined by `sep`.
    export fn String::join(slice<String, shared> parts, StringView sep) : String
    {
        String out = String::new();
        foreach (i, p in parts)
        {
            if (i > 0)
            {
                String::append_view(&mut out, sep);
            }
            String::append_string(&mut out, p);
        }
        out
    }

    // A new `String` holding a copy of the view's text.
    export fn String::from_view(StringView v) : String
    {
        String { .bytes = Vec::from_slice(v.bytes) }
    }

    export fn StringView::parse<T>(StringView v) : Result<T, ParseError>
    {
        String s = String::from_view(v);
        String::parse<T>(&s)
    }

    fn String::append_view(ref<String, exclusive> s, StringView v)
    {
        foreach (x in v.bytes)
        {
            Vec::push(&mut s.bytes, *x);
        }
    }


D-0181 adds prefixes and suffixes, lines, words and code points, each
with its `String::` twin (D-0113):

    [Strip-Prefix]      (D-0181)   ⟨StringView::strip_prefix(v, t), Σ⟩ → Some(&v[len(t) .. $]) when v starts with t, else
                        None; strip_suffix likewise from the end (Some(&v[0 .. $ - len(t)]))
    [Lines]             (D-0181)   ⟨StringView::lines(v), Σ⟩ → the parts of v between its "\n" bytes, each without a "\r"
                        just before its "\n", in order; no part after a final "\n"; empty text gives none
                        ("a\nb\n" and "a\nb" both give "a", "b"; "\n\n" gives "", "")
    [Split-Whitespace]  (D-0181)   ⟨StringView::split_whitespace(v), Σ⟩ → the maximal runs of bytes for which ascii_is_space
                        is false, in order (none empty)
    [Code-Point]        (D-0181)   ⟨StringView::code_point(v), Σ⟩ → None for empty v, else Some(the code point of v's first
                        character, decoded from its one to four UTF-8 bytes).
                        ⟨String::push_code_point(s, cp), Σ⟩ → ⟨(), Σ'⟩ with the UTF-8 bytes of cp appended to s,
                        or of U+FFFD when D800 ≤ cp ≤ DFFF or cp > 10FFFF (so s stays UTF-8)

    // `v` without `t` at its start: `Some(the rest)` when `v` begins with
    // `t`, else `None`. The idiom of `--name=value`: `strip_prefix(a,
    // "--name=")` gives the value or says the argument is another.
    export fn StringView::strip_prefix(StringView v, StringView t) : Option<StringView>
    {
        if (StringView::starts_with(v, t))
        {
            return Some(StringView::sub(v, StringView::len(t), StringView::len(v)));
        }
        None
    }

    // `v` without `t` at its end: `Some(the rest)` when `v` ends with `t`.
    export fn StringView::strip_suffix(StringView v, StringView t) : Option<StringView>
    {
        if (StringView::ends_with(v, t))
        {
            return Some(StringView::sub(v, 0, StringView::len(v) - StringView::len(t)));
        }
        None
    }

    // The lines of `v`: the parts between `\n`s, each without a `\r` just
    // before its `\n` (D-0092), and no empty line after a final `\n`, so
    // `"a\nb\n"` and `"a\nb"` both give `a` and `b`; empty text gives none.
    export fn StringView::lines(StringView v) : Vec<StringView>
    {
        Vec<StringView> out = Vec::new();
        usize n = slice_len(v.bytes);
        usize start = 0;
        while (true)
        {
            usize i = byte_index_from(v.bytes, start, b'\n');
            if (i >= n)
            {
                break;
            }
            usize end = if (i > start && v.bytes[i - 1] == b'\r') { i - 1 } else { i };
            Vec::push(&mut out, StringView::sub(v, start, end));
            start = i + 1;
        }
        if (start < n)
        {
            Vec::push(&mut out, StringView::sub(v, start, n));
        }
        out
    }

    // Scans over the bytes from `at`, their slice each function's only
    // reference (read unchecked): the first `c` (the length when none), and
    // the end of a run of ASCII white space (`space`) or of other bytes.
    fn byte_index_from(slice<u8, shared> b, usize at, u8 c) : usize
    {
        usize i = at;
        while (i < slice_len(b) && b[i] != c)
        {
            i += 1;
        }
        i
    }

    fn space_run_end(slice<u8, shared> b, usize at, bool space) : usize
    {
        usize i = at;
        while (i < slice_len(b) && ascii_is_space(b[i]) == space)
        {
            i += 1;
        }
        i
    }

    // The words of `v`: the runs of bytes that are not ASCII whitespace
    // (`ascii_is_space`), in order; no part is empty, so leading, trailing
    // and repeated whitespace make no difference.
    export fn StringView::split_whitespace(StringView v) : Vec<StringView>
    {
        Vec<StringView> out = Vec::new();
        usize n = slice_len(v.bytes);
        usize i = 0;
        while (i < n)
        {
            i = space_run_end(v.bytes, i, true);
            usize start = i;
            i = space_run_end(v.bytes, i, false);
            if (i > start)
            {
                Vec::push(&mut out, StringView::sub(v, start, i));
            }
        }
        out
    }

    // The code point of `v`'s first character (U+0000 to U+10FFFF); `None`
    // when `v` is empty. With `chars`, every character of a text in turn.
    export fn StringView::code_point(StringView v) : Option<u32>
    {
        usize n = slice_len(v.bytes);
        if (n == 0)
        {
            return None;
        }
        u8 b = v.bytes[0];
        if (b < 0x80)
        {
            return Some(widen<u32>(b));
        }
        // The view's bytes are UTF-8 (`[View-Form]`), so a lead byte's
        // continuation bytes are there.
        if ((b & 0xE0) == 0xC0)
        {
            return Some((widen<u32>(b & 0x1F) << 6) | widen<u32>(v.bytes[1] & 0x3F));
        }
        if ((b & 0xF0) == 0xE0)
        {
            return Some((widen<u32>(b & 0x0F) << 12) | (widen<u32>(v.bytes[1] & 0x3F) << 6) | widen<u32>(v.bytes[2] & 0x3F));
        }
        Some((widen<u32>(b & 0x07) << 18) | (widen<u32>(v.bytes[1] & 0x3F) << 12) | (widen<u32>(v.bytes[2] & 0x3F) << 6) | widen<u32>(v.bytes[3] & 0x3F))
    }

    // Appends the character with code point `cp` as UTF-8 (one to four
    // bytes). A value that is no character (a surrogate, D800 to DFFF, or
    // above 10FFFF) appends U+FFFD, the replacement character, as decoders
    // do, so the `String` stays valid UTF-8.
    export fn String::push_code_point(ref<String, exclusive> s, u32 cp)
    {
        u32 c = if ((cp >= 0xD800 && cp <= 0xDFFF) || cp > 0x10FFFF) { 0xFFFD } else { cp };
        if (c < 0x80)
        {
            Vec::push(&mut s.bytes, narrow<u8>(c));
        }
        else if (c < 0x800)
        {
            Vec::push(&mut s.bytes, 0xC0 | narrow<u8>(c >> 6));
            Vec::push(&mut s.bytes, 0x80 | narrow<u8>(c & 0x3F));
        }
        else if (c < 0x10000)
        {
            Vec::push(&mut s.bytes, 0xE0 | narrow<u8>(c >> 12));
            Vec::push(&mut s.bytes, 0x80 | narrow<u8>((c >> 6) & 0x3F));
            Vec::push(&mut s.bytes, 0x80 | narrow<u8>(c & 0x3F));
        }
        else
        {
            Vec::push(&mut s.bytes, 0xF0 | narrow<u8>(c >> 18));
            Vec::push(&mut s.bytes, 0x80 | narrow<u8>((c >> 12) & 0x3F));
            Vec::push(&mut s.bytes, 0x80 | narrow<u8>((c >> 6) & 0x3F));
            Vec::push(&mut s.bytes, 0x80 | narrow<u8>(c & 0x3F));
        }
    }

**Depends on:** rule.stdlib.string, rule.agg.slice, rule.temporal.elision, D-0047, D-0053
**Affects:** the `String` a view borrows

## 2i. Queues

In `std::collections` (D-0137).

### `rule.stdlib.queue`
**Status:** ACCEPTED

`Queue<T>` holds a sequence of elements and adds and takes them at
either end in constant time, amortized as a `Vec`'s `push` is. It is
written in CobaltC below as two `Vec`s used as stacks, so each step is
a `Vec`'s `push`, `pop` or index: `head` holds the front part, front
element last, and `tail` the back part, back element last.

    [Queue]   a queue's value is a sequence ⟨x1, …, xn⟩, x1 its front and xn its back
        Queue::new()            = ⟨⟩
        Queue::len(&q)          = n
        Queue::push_back(&mut q, x)    ⟨x1, …, xn⟩ → ⟨x1, …, xn, x⟩
        Queue::push_front(&mut q, x)   ⟨x1, …, xn⟩ → ⟨x, x1, …, xn⟩
        Queue::pop_front(&mut q)       ⟨x1, …, xn⟩ → ⟨x2, …, xn⟩, giving Some(x1);   ⟨⟩ → ⟨⟩, giving None
        Queue::pop_back(&mut q)        ⟨x1, …, xn⟩ → ⟨x1, …, xn−1⟩, giving Some(xn); ⟨⟩ → ⟨⟩, giving None
        Queue::front(&q) = Some(&x1),  Queue::back(&q) = Some(&xn);  None when n = 0
        Queue::clear(&mut q), and the queue's end: x1, …, xn destroyed in that order; clear leaves ⟨⟩

    // Queues (`rule.stdlib.queue`, spec/21 §2i, D-0137): two `Vec`s used
    // as stacks, so every step is a `Vec`'s `push`, `pop` or index. `head`
    // holds the front part with the front element last; `tail` holds the back
    // part in order, the back element last. Taking from an empty side first
    // moves elements across from the other (`Queue::shift`): all of them
    // while the queue is taken from at one end only, so each element moves
    // at most once; half of them once it has been taken from at both, so
    // alternating the ends still moves each element a bounded number of
    // times.
    export resource struct Queue<T>
    {
        Vec<T> head;                        // the front part, front element last
        Vec<T> tail;                        // the back part, back element last
        bool took_front;                    // `pop_front` has taken an element
        bool took_back;                     // `pop_back` has
    }

    export fn Queue::new<T>() : Queue<T>
    {
        Queue { .head = Vec::new(), .tail = Vec::new(), .took_front = false, .took_back = false }
    }

    export fn Queue::len<T>(ref<Queue<T>, shared> q) : usize
    {
        q.head.len + q.tail.len
    }

    // `to` is empty. With `half`, the half of `from`'s elements nearest `to`'s
    // end (its first ones, rounding up) move to `to`, the rest moving down to
    // the start of `from` in order; otherwise all of them move. Either way
    // they arrive in `to`'s order, `to`'s last being `from`'s first. Each
    // element moves by `copy_raw` into a slot that holds nothing, and the
    // slots left are released together.
    fn Queue::shift<T>(ref<Vec<T>, exclusive> from, ref<Vec<T>, exclusive> to, bool half)
    {
        usize n = from.len;
        usize stay = if (half)
        {
            n / 2
        }
        else
        {
            0
        };
        usize m = n - stay;
        usize size = sizeof<T>();
        Vec::reserve(to, m);
        rawptr<T> src = from.ptr;
        rawptr<T> dst = to.ptr;
        unsafe
        {
            for (usize k = 0; k < m; k += 1)
            {
                copy_raw(
                    reinterpret_ptr<u8>(dst + reinterpret<isize>(m - 1 - k)),
                    reinterpret_ptr<u8>(src + reinterpret<isize>(k)),
                    size,
                );
            }
            release(reinterpret_ptr<u8>(src), m * size);
            if (stay > 0)
            {
                // `m >= stay`: the ranges do not overlap.
                copy_raw(reinterpret_ptr<u8>(src), reinterpret_ptr<u8>(src + reinterpret<isize>(m)), stay * size);
                release(reinterpret_ptr<u8>(src + reinterpret<isize>(stay)), m * size);
            }
        }
        to.len = m;
        from.len = stay;
    }

    export fn Queue::push_back<T>(ref<Queue<T>, exclusive> q, T x)
    {
        Vec::push(&mut q.tail, x);
    }

    export fn Queue::push_front<T>(ref<Queue<T>, exclusive> q, T x)
    {
        Vec::push(&mut q.head, x);
    }

    export fn Queue::pop_front<T>(ref<Queue<T>, exclusive> q) : Option<T>
    {
        if (q.head.len == 0)
        {
            if (q.tail.len == 0)
            {
                return None;
            }
            q.took_front = true;
            Queue::shift(&mut q.tail, &mut q.head, q.took_back);
        }
        Vec::pop(&mut q.head)
    }

    export fn Queue::pop_back<T>(ref<Queue<T>, exclusive> q) : Option<T>
    {
        if (q.tail.len == 0)
        {
            if (q.head.len == 0)
            {
                return None;
            }
            q.took_back = true;
            Queue::shift(&mut q.head, &mut q.tail, q.took_front);
        }
        Vec::pop(&mut q.tail)
    }

    export fn Queue::front<T>(ref<Queue<T>, shared> q) : Option<ref<T, shared>>
    {
        if (q.head.len > 0)
        {
            return Some(Vec::index_shared(&q.head, q.head.len - 1));
        }
        if (q.tail.len > 0)
        {
            return Some(Vec::index_shared(&q.tail, 0));
        }
        None
    }

    export fn Queue::back<T>(ref<Queue<T>, shared> q) : Option<ref<T, shared>>
    {
        if (q.tail.len > 0)
        {
            return Some(Vec::index_shared(&q.tail, q.tail.len - 1));
        }
        if (q.head.len > 0)
        {
            return Some(Vec::index_shared(&q.head, 0));
        }
        None
    }

    // Empties q, destroying its elements front to back: `head` last first,
    // then `tail` first to last. The buffers are kept.
    export fn Queue::clear<T>(ref<Queue<T>, exclusive> q)
    {
        Vec::truncate(&mut q.head, 0);
        Vec::clear(&mut q.tail);
    }

    export fn Queue::drop<T>(ref<Queue<T>, exclusive> self)
    {
        Queue::clear(self);
    }

- **Ownership.** The queue owns its elements; one taken by `pop_front`
  or `pop_back` is the caller's. `front` and `back` give a shared
  reference that borrows the queue (`rule.temporal.elision`), so the
  queue cannot change while it is in use: a push may move every element
  to a new buffer.
- **Moving between the parts.** Taking from an empty part first moves
  elements across from the other, copying their bytes (`[Copy-Raw]`)
  and releasing the slots they left (`[Release]`): all of them while the
  queue has been taken from at one end only, so each element moves at
  most once; half of them once it has been taken from at both, so
  taking from the ends in turn still moves each element a bounded
  number of times. The buffers never shrink, and `clear` keeps them.
- **Not provided:** positional access (`q[i]`; `[Index-Vec]` is
  `Vec`'s), `foreach` over a queue (`rule.control.foreach` names its
  collections), `_mut` twins, `clone` and `reserve` — revisit conditions
  of D-0137.

**Depends on:** rule.stdlib.vec, rule.trust.rawptr, rule.resauth.destroy, D-0137
**Affects:** state.storage (through `Vec`)

### `rule.stdlib.priority-queue`
**Status:** ACCEPTED

`PriorityQueue<T>` gives its elements back least first, in an order
fixed when it is made: `PriorityQueue::new()` uses the key types' own
order (`key_less`, as `Vec::sort` does, §1b), `PriorityQueue::new_by(less)`
the caller's `less` (true when its first argument goes before its
second, as `sort_by`'s). Elements the order calls equal leave first in,
first out. It is a binary heap in a `Vec`, written in CobaltC below;
`push` and `pop` take time in proportion to the logarithm of the length.

    [Priority-Queue]   a queue's value is a set of pairs (x, k), k the element's arrival number:
                       0 for the first push after new or clear, then 1, 2, …
        (x, k) goes before (y, l)  ⟺  less(x, y)  ∨  (¬less(y, x) ∧ k < l)
            less = key_less (new) or the function given (new_by)
        PriorityQueue::push(&mut q, x)   adds (x, k), k the next arrival number
        PriorityQueue::pop(&mut q)       removes the pair that goes before every other, giving Some(x); None when empty
        PriorityQueue::peek(&q)          = Some(&x) for that pair, removing nothing; None when empty
        PriorityQueue::len(&q)           = the number of pairs
        PriorityQueue::clear(&mut q), and the queue's end: every element destroyed, in the order of the
            `Vec` that holds them (the heap's, which the body below determines)
        -- the body decides with one comparison, which is this order when less is a strict weak order
        -- a `less` that is not a strict weak order gives the order the body gives; nothing faults

    [Priority-Queue-Not-Key]   disposition: rejected
        PriorityQueue::new<T>() with T not a key type (§2g), at the call or at a generic function's instantiation
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch   (repair: PriorityQueue::new_by with a comparison)

    // Priority queues (`rule.stdlib.priority-queue`, spec/21 §2i, D-0137):
    // a binary heap in a `Vec`, each element's parent going before it. Each
    // element has an arrival number, kept at the same position of a second
    // `Vec`, so elements the order calls equal leave first in, first out.
    export struct PriorityQueue<T>
    {
        Vec<T> items;
        Vec<u64> seqs;                      // seqs[i] is items[i]'s arrival number
        bool keyed;                         // the key types' order, not `less`
        fn(ref<T, shared>, ref<T, shared>) : bool less;
        u64 next;                           // the next arrival number
    }

    // A queue in the key types' own order (as `Vec::sort`'s): smallest first.
    export fn PriorityQueue::new<T>() : PriorityQueue<T>
    {
        PriorityQueue { .items = Vec::new(), .seqs = Vec::new(), .keyed = true, .less = key_before<T>, .next = 0 }
    }

    // A queue in the order `less` gives (true when its first argument goes
    // before its second): the element no other goes before comes out first.
    export fn PriorityQueue::new_by<T>(fn(ref<T, shared>, ref<T, shared>) : bool less) : PriorityQueue<T>
    {
        PriorityQueue { .items = Vec::new(), .seqs = Vec::new(), .keyed = false, .less = less, .next = 0 }
    }

    export fn PriorityQueue::len<T>(ref<PriorityQueue<T>, shared> q) : usize
    {
        q.items.len
    }

    // Whether element i comes out before element j: the earlier arrival
    // unless the later goes before it, the later only if it goes before the
    // earlier. One comparison decides, the order being a strict weak order
    // (§2i); the key types' is compared in place (`key_less_at`, as
    // `Vec::sort` does).
    fn PriorityQueue::before<T>(
        ref<PriorityQueue<T>, shared> q,
        bool keyed,
        ref<Vec<T>, shared> items,
        rawptr<u64> sb,
        usize i,
        usize j,
    ) : bool
    {
        bool earlier = unsafe
        {
            *(sb + reinterpret<isize>(i)) < *(sb + reinterpret<isize>(j))
        };
        if (earlier)
        {
            if (keyed)
            {
                !key_less_at(items, j, i)
            }
            else
            {
                !(q.less)(Vec::index_shared(items, j), Vec::index_shared(items, i))
            }
        }
        else
        {
            if (keyed)
            {
                key_less_at(items, i, j)
            }
            else
            {
                (q.less)(Vec::index_shared(items, i), Vec::index_shared(items, j))
            }
        }
    }

    // The heap moves its elements by copying their bytes into a slot that
    // holds nothing (`copy_raw`) and releasing the slot they left, rather
    // than by `Vec::swap`, so no reference is formed for a move. Each loop
    // holds one shared reference to each `Vec` for its comparisons and
    // reaches the slots through `ib` and `sb`: the buffers do not move while
    // the loop runs.

    // Moves element i up past every ancestor it comes out before. The place
    // is found first, comparing with the element where it stands; then the
    // ancestors on the way move down one each and it takes the place.
    fn PriorityQueue::sift_up<T>(ref<PriorityQueue<T>, exclusive> q, usize i)
    {
        rawptr<u64> sb = q.seqs.ptr;
        usize t = i;
        {
            ref<PriorityQueue<T>, shared> s = &*q;
            ref<Vec<T>, shared> items = &s.items;
            bool keyed = s.keyed;
            while (t > 0)
            {
                usize p = (t - 1) / 2;
                if (!PriorityQueue::before(s, keyed, items, sb, i, p))
                {
                    break;
                }
                t = p;
            }
        }
        if (t == i)
        {
            return;
        }
        T x = Vec::take_raw(&mut q.items, i);
        u64 xs = q.seqs[i];
        usize size = sizeof<T>();
        rawptr<T> ib = q.items.ptr;
        usize j = i;
        while (j != t)
        {
            usize p = (j - 1) / 2;
            unsafe
            {
                copy_raw(
                    reinterpret_ptr<u8>(ib + reinterpret<isize>(j)),
                    reinterpret_ptr<u8>(ib + reinterpret<isize>(p)),
                    size,
                );
                release(reinterpret_ptr<u8>(ib + reinterpret<isize>(p)), size);
                *(sb + reinterpret<isize>(j)) = *(sb + reinterpret<isize>(p));
            }
            j = p;
        }
        unsafe
        {
            *(ib + reinterpret<isize>(t)) = x;
            *(sb + reinterpret<isize>(t)) = xs;
        }
    }

    export fn PriorityQueue::push<T>(ref<PriorityQueue<T>, exclusive> q, T x)
    {
        Vec::push(&mut q.items, x);
        Vec::push(&mut q.seqs, q.next);
        q.next = q.next + 1;
        PriorityQueue::sift_up(q, q.items.len - 1);
    }

    // The first element leaves a hole at the top. The hole moves down to a
    // leaf, the child that comes out first rising into it at each level (one
    // comparison per level); the last element fills the hole and moves up to
    // its place, which is seldom far.
    export fn PriorityQueue::pop<T>(ref<PriorityQueue<T>, exclusive> q) : Option<T>
    {
        usize n = q.items.len;
        if (n == 0)
        {
            return None;
        }
        T top = Vec::take_raw(&mut q.items, 0);
        usize size = sizeof<T>();
        rawptr<T> ib = q.items.ptr;
        rawptr<u64> sb = q.seqs.ptr;
        usize last = n - 1;
        usize hole = 0;
        {
            ref<PriorityQueue<T>, shared> s = &*q;
            ref<Vec<T>, shared> items = &s.items;
            bool keyed = s.keyed;
            while (2 * hole + 1 < last)
            {
                usize c = 2 * hole + 1;
                if (c + 1 < last && PriorityQueue::before(s, keyed, items, sb, c + 1, c))
                {
                    c = c + 1;
                }
                unsafe
                {
                    copy_raw(
                        reinterpret_ptr<u8>(ib + reinterpret<isize>(hole)),
                        reinterpret_ptr<u8>(ib + reinterpret<isize>(c)),
                        size,
                    );
                    release(reinterpret_ptr<u8>(ib + reinterpret<isize>(c)), size);
                    *(sb + reinterpret<isize>(hole)) = *(sb + reinterpret<isize>(c));
                }
                hole = c;
            }
        }
        if (hole != last)
        {
            unsafe
            {
                copy_raw(
                    reinterpret_ptr<u8>(ib + reinterpret<isize>(hole)),
                    reinterpret_ptr<u8>(ib + reinterpret<isize>(last)),
                    size,
                );
                release(reinterpret_ptr<u8>(ib + reinterpret<isize>(last)), size);
                *(sb + reinterpret<isize>(hole)) = *(sb + reinterpret<isize>(last));
            }
        }
        q.items.len = last;
        q.seqs.len = last;
        if (hole != last)
        {
            PriorityQueue::sift_up(q, hole);
        }
        Some(top)
    }

    export fn PriorityQueue::peek<T>(ref<PriorityQueue<T>, shared> q) : Option<ref<T, shared>>
    {
        if (q.items.len == 0)
        {
            return None;
        }
        Some(Vec::index_shared(&q.items, 0))
    }

    // Empties q, destroying its elements in the order the heap holds them.
    export fn PriorityQueue::clear<T>(ref<PriorityQueue<T>, exclusive> q)
    {
        Vec::clear(&mut q.items);
        Vec::clear(&mut q.seqs);
        q.next = 0;
    }

- **Order among equals.** First in, first out: an event simulation's
  simultaneous events run in the order they were scheduled. This is
  `Vec::sort`'s stability carried over; it costs one `u64` per element,
  kept in a second `Vec` at the same position. One comparison decides
  which of two elements comes out first: the earlier arrival, unless the
  later goes before it — which, for a strict weak order, is
  `[Priority-Queue]`'s condition.
- **How it moves elements.** `pop` takes the first element and moves
  the hole it leaves down to a leaf, the child that comes out first
  rising at each level, then moves the last element into the hole and
  up to its place (one comparison per level on the way down, and seldom
  any on the way up). Elements move by `copy_raw` into a slot that holds
  nothing and `release` of the slot they left (`[Copy-Raw]`,
  `[Release]`), not by `Vec::swap`, and the key types' order compares
  them in place (`key_less_at`), so neither forms a reference.
- **Which end.** Least first: `new` and `sort` agree on one order. A
  largest-first queue passes a `less` that answers "greater".
- **Ownership.** The queue owns its elements; one `pop` gives is the
  caller's. `peek`'s reference borrows the queue.
- **Not provided:** `peek_mut` (a changed element would need moving;
  `pop`, change and `push` does it), `foreach`, `clone`, `reserve` —
  revisit conditions of D-0137.
- **Names.** `Queue`, `PriorityQueue` and the functions above with
  `export` are items of `std`; their fields, `Queue::shift`,
  `PriorityQueue::before` and `PriorityQueue::sift_up` are `std`'s own.

**Depends on:** rule.stdlib.vec, rule.stdlib.sort, rule.trust.rawptr, D-0137
**Affects:** state.storage (through `Vec`)

## 2j. Dates and times

In `std::time` (D-0140).

### `rule.stdlib.time`
**Status:** ACCEPTED

A `DateTime` is a moment in UTC to the second: a date of the proleptic
Gregorian calendar (year 0 is 1 BC, year -1 is 2 BC) and a time of day.
There are no leap seconds: every day has 86,400 seconds. The one time
zone known is the machine's own, through its offset (`[Local-Offset]`,
D-0147). `unix(t)` below is the number of seconds from
1970-01-01T00:00:00Z to the moment `t` names, by that calendar.

    [From-Unix]          ⟨DateTime::from_unix(s), Σ⟩ → t   where unix(t) = s; total over i64 (t.year is an
                         i64, so every second names a moment)
    [To-Unix]            ⟨DateTime::to_unix(&t), Σ⟩ → unix(t)   when valid(t) and unix(t) fits in i64
    [Invalid-DateTime]   disposition: checked
                         ⟨DateTime::to_unix(&t), Σ⟩ ↛ diag.invalid-datetime   when ¬valid(t) or unix(t) ∉ i64
                         valid(t) ⟺ 1 ≤ month ≤ 12 ∧ 1 ≤ day ≤ days-in(year, month) ∧ hour < 24 ∧ minute < 60
                         ∧ second < 60; a leap year is divisible by 4 and not by 100, or by 400
    [Weekday]            ⟨DateTime::weekday(&t), Σ⟩ → the day of the week of t's date (1970-01-01 a
                         Thursday); defined for every valid t (the calendar repeats every 400 years,
                         a whole number of weeks); diag.invalid-datetime for an invalid t
    [To-Iso]             ⟨DateTime::to_iso(&t), Σ⟩ → "YYYY-MM-DDTHH:MM:SSZ", each field zero-padded to its
                         width; a year outside 0 ..= 9999 written with its sign ('-' or '+') and at
                         least four digits
    [From-Iso]           ⟨DateTime::from_iso(v), Σ⟩ → Ok(t) for to_iso's form or a bare date
                         "YYYY-MM-DD" (its midnight); the year four digits, or a sign and four or
                         more; Err(Empty) for "", Err(Invalid(i)) where byte i is the first that does
                         not fit the form (i = len when the text stops short), Err(OutOfRange) when
                         the fields name no moment or the year exceeds i64
    [Unix-Ms]            ⟨unix_ms(), Σ⟩ → the wall clock's milliseconds since 1970-01-01T00:00:00Z,
                         rounded down; unix_ms() / 1000 is unix_seconds() when read together
    [Local-Offset]       disposition: trusted-unchecked   outcome: unspecified { the platform's offset o, −64800 ≤ o ≤ 64800 }
                         ⟨local_offset_seconds(s), Σ⟩ → o, the seconds the machine's local time is ahead
                         of UTC at the moment s seconds after 1970-01-01T00:00:00Z (negative when
                         behind), daylight saving included, by the operating system's rule for the
                         zone the program runs in; −64800 ≤ o ≤ 64800; for a moment the system has
                         no rule for, the offset at the current moment
    [To-Local]           ⟨DateTime::to_local(&t), Σ⟩ → from_unix(unix(t) + local_offset_seconds(unix(t)))
                         when valid(t) and the sum fits in i64; diag.invalid-datetime otherwise

`DateTime::to_local`'s result is a `DateTime` like any other, and knows
nothing of its zone: `to_iso` of it would write `Z`, and `to_unix` of it
is the moment plus the offset. A program keeps the UTC value, converts
for display last, and prints a local value by its fields. The offset is
the environment's (`rule.fn.program`), as the clocks are: it differs
between machines, and on one machine between moments (D-0147).

`DateTime::new` gives `Some` exactly when `valid` holds of its fields;
`DateTime::eq` and `DateTime::less` compare the fields in order of
significance, which for valid values is the order of `unix`; `less`
fits `Vec::sort_by`. `DateTime::now()` is `from_unix(unix_seconds())`.
`Weekday::text` gives the day's English name. The wall clock belongs to
the program's environment, as `rule.stdlib.env`'s clocks do; elapsed
time is measured with `monotonic_ns`, since the wall clock may be
changed while a program runs.


    [Iso-Ms]      (D-0181)   ⟨iso_from_unix_ms(ms), Σ⟩ → to_iso of the moment floor(ms / 1000) seconds after 1970 with
                  "." and the three digits of ms mod 1000 before its "Z" (1791022620123 → "2026-10-03T10:17:00.123Z").
                  ⟨unix_ms_from_iso(t), Σ⟩ → Ok(to_unix(d) · 1000 + f) for the d and the fraction f (its first three
                  digits, fewer filled with zeros, further digits ignored) that [From-Iso] reads from t;
                  Err(OutOfRange) when the product leaves i64; from_iso's errors otherwise
    [From-Iso]    (D-0181)   the seconds may be followed by "." and one or more digits before "Z"; from_iso drops them
                  (a "." with no digit is Invalid at the digit's position)
    [Http-Date]   (D-0181, RFC 9110 §5.6.7)   ⟨DateTime::to_http_date(t), Σ⟩ → the IMF-fixdate "Sun, 06 Nov 1994 08:49:37 GMT":
                  the weekday's first three letters, the day in two digits, the month's three letters, the year in
                  four digits, the time, "GMT"; a year outside 0 ..= 9999 is ↛ diag.invalid-datetime.
                  ⟨DateTime::from_http_date(t), Σ⟩ → Ok(d) for t's words (runs between ASCII whitespace) in the
                  IMF-fixdate form (6 words ending in "GMT"), RFC 850's ("Sunday, 06-Nov-94 08:49:37 GMT": a two-digit
                  year, 70 to 99 as 1970 to 1999, 00 to 69 as 2000 to 2069) or asctime's ("Sun Nov  6 08:49:37 1994",
                  the day one or two digits), the weekday word not checked against the date; Err(Empty) for no words,
                  Err(Invalid(i)) with i the start of the first word that does not fit, Err(OutOfRange) for fields
                  that name no moment

The `std` source:

    // `std::time` (spec/21 §2j, D-0140): dates and times of day in UTC, the
    // wall clock in milliseconds, and the machine's local time (D-0147).
    // Calendar arithmetic written in CobaltC: the proleptic Gregorian
    // calendar, no leap seconds; the local offset is the platform's. Elapsed
    // time is measured with `monotonic_ns`, never with the wall clock, which
    // may be changed while a program runs.

    // A moment in UTC: a date of the proleptic Gregorian calendar (`year` 0
    // is 1 BC) and a time of day to the second. Any field values can be
    // written in a struct literal; `DateTime::new` checks them, and
    // `DateTime::to_unix` faults on a value that names no moment.
    export struct DateTime
    {
        export i64 year;
        export u8 month;
        export u8 day;
        export u8 hour;
        export u8 minute;
        export u8 second;
    }

    export enum Weekday
    {
        Monday,
        Tuesday,
        Wednesday,
        Thursday,
        Friday,
        Saturday,
        Sunday,
    }

    // The quotient rounded down and the remainder with the divisor's sign
    // (`/` and `%` round towards zero); `d` > 0.
    fn floor_div(i64 n, i64 d) : i64
    {
        i64 q = n / d;
        if (n % d < 0)
        {
            q -= 1;
        }
        q
    }

    fn floor_mod(i64 n, i64 d) : i64
    {
        i64 r = n % d;
        if (r < 0)
        {
            r += d;
        }
        r
    }

    fn leap_year(i64 y) : bool
    {
        floor_mod(y, 4) == 0 && (floor_mod(y, 100) != 0 || floor_mod(y, 400) == 0)
    }

    fn days_in_month(i64 y, u8 m) : u8
    {
        if (m == 2)
        {
            if (leap_year(y))
            {
                return 29;
            }
            return 28;
        }
        if (m == 4 || m == 6 || m == 9 || m == 11)
        {
            return 30;
        }
        31
    }

    fn DateTime::valid(ref<DateTime, shared> t) : bool
    {
        t.month >= 1 && t.month <= 12 && t.day >= 1 && t.day <= days_in_month(
            t.year,
            t.month,
        ) && t.hour < 24 && t.minute < 60 && t.second < 60
    }

    // D-0140: the moment `s` seconds after 1970-01-01T00:00:00Z (before it
    // when negative). Every `i64` names one, so this never fails. The days
    // are converted by H. Hinnant's `civil_from_days`.
    export fn DateTime::from_unix(i64 s) : DateTime
    {
        i64 z = floor_div(s, 86400) + 719468;
        i64 secs = floor_mod(s, 86400);
        i64 era = floor_div(z, 146097);
        i64 doe = z - era * 146097;
        i64 yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        i64 doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        i64 mp = (5 * doy + 2) / 153;
        i64 d = doy - (153 * mp + 2) / 5 + 1;
        i64 m = mp + 3;
        if (mp >= 10)
        {
            m = mp - 9;
        }
        i64 y = yoe + era * 400;
        if (m <= 2)
        {
            y += 1;
        }
        DateTime { .year = y, .month = narrow<u8>(m), .day = narrow<u8>(d), .hour = narrow<u8>(secs / 3600), .minute = narrow<u8>(secs % 3600 / 60), .second = narrow<u8>(secs % 60) }
    }

    // The moment now, by the wall clock, to the second.
    export fn DateTime::now() : DateTime
    {
        DateTime::from_unix(unix_seconds())
    }

    // The `DateTime` with these fields, or `None` when they name no moment
    // (a month 13, a 30 February, an hour 24, a second 60).
    export fn DateTime::new(i64 year, u8 month, u8 day, u8 hour, u8 minute, u8 second) : Option<DateTime>
    {
        DateTime t = DateTime { .year = year, .month = month, .day = day, .hour = hour, .minute = minute, .second = second };
        if (!DateTime::valid(&t))
        {
            return None;
        }
        Some(t)
    }

    // The seconds since 1970-01-01T00:00:00Z, `from_unix`'s inverse (H.
    // Hinnant's `days_from_civil`). A `DateTime` whose fields name no moment,
    // or one too far from 1970 for an `i64`, is `diag.invalid-datetime`.
    export fn DateTime::to_unix(ref<DateTime, shared> t) : i64
    {
        if (!DateTime::valid(t))
        {
            fault(invalid_datetime);
        }
        i64 m = widen<i64>(t.month);
        i64 y = t.year;
        if (m <= 2)
        {
            y = match (checked_sub(y, 1))
            {
                Some(v) : v,
                None    : fault(invalid_datetime),
            };
        }
        i64 era = floor_div(y, 400);
        i64 yoe = y - era * 400;
        i64 mp = m + 9;
        if (m > 2)
        {
            mp = m - 3;
        }
        i64 doy = (153 * mp + 2) / 5 + widen<i64>(t.day) - 1;
        i64 doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        i64 secs = widen<i64>(t.hour) * 3600 + widen<i64>(t.minute) * 60 + widen<i64>(t.second);
        Option<i64> days = match (checked_mul(era, 146097))
        {
            Some(e) : checked_add(e, doe - 719468),
            None    : None,
        };
        // Before 1970 the day after is multiplied, and the seconds counted back
        // from it, so the earliest `i64` second is reached without overflow.
        Option<i64> s = match (days)
        {
            Some(d) : if (d < 0)
            {
                match (checked_mul(d + 1, 86400))
                {
                    Some(x) : checked_add(x, secs - 86400),
                    None    : None,
                }
            }
            else
            {
                match (checked_mul(d, 86400))
                {
                    Some(x) : checked_add(x, secs),
                    None    : None,
                }
            },
            None : None,
        };
        match (s)
        {
            Some(v) : v,
            None    : fault(invalid_datetime),
        }
    }

    // The day of the week (1970-01-01 was a Thursday). The calendar repeats
    // every 400 years, a whole number of weeks, so any year is reduced to one
    // near 2000 first and no `DateTime` is too far from 1970 for this.
    export fn DateTime::weekday(ref<DateTime, shared> t) : Weekday
    {
        DateTime near = DateTime { .year = 2000 + floor_mod(
            t.year,
            400,
        ), .month = t.month, .day = t.day, .hour = 0, .minute = 0, .second = 0 };
        i64 days = DateTime::to_unix(&near) / 86400;
        match (floor_mod(days + 3, 7))
        {
            0 : Monday,
            1 : Tuesday,
            2 : Wednesday,
            3 : Thursday,
            4 : Friday,
            5 : Saturday,
            _ : Sunday,
        }
    }

    export fn Weekday::text(ref<Weekday, shared> w) : str
    {
        match (w)
        {
            Monday    : "Monday",
            Tuesday   : "Tuesday",
            Wednesday : "Wednesday",
            Thursday  : "Thursday",
            Friday    : "Friday",
            Saturday  : "Saturday",
            Sunday    : "Sunday",
        }
    }

    // ISO 8601 in UTC: `2026-10-03T10:17:00Z`. A year outside 0 ..= 9999 is
    // written with its sign and at least four digits (`-0001`, `+10000`).
    export fn DateTime::to_iso(ref<DateTime, shared> t) : String
    {
        String out = String::new();
        if (t.year >= 0 && t.year <= 9999)
        {
            String::appendf(&mut out, "%04d", t.year);
        }
        else if (t.year > 9999)
        {
            String::appendf(&mut out, "+%d", t.year);
        }
        else
        {
            String::appendf(&mut out, "-%04u", wrapping_sub(0: u64, reinterpret<u64>(t.year)));
        }
        String::appendf(&mut out, "-%02d-%02dT%02d:%02d:%02dZ", t.month, t.day, t.hour, t.minute, t.second);
        out
    }

    // The number in the `n` digits at `at`, or the position of the first
    // byte that is not a digit.
    fn iso_digits(StringView v, usize at, usize n) : Result<i64, usize>
    {
        i64 x = 0;
        for (usize i = at; i < at + n; i += 1)
        {
            if (i >= slice_len(v.bytes) || !ascii_is_digit(v.bytes[i]))
            {
                return Err(i);
            }
            x = x * 10 + widen<i64>(v.bytes[i] - 48);
        }
        Ok(x)
    }

    // Whether byte `at` is `c`; a missing byte is not.
    fn iso_byte(StringView v, usize at, u8 c) : bool
    {
        at < slice_len(v.bytes) && v.bytes[at] == c
    }

    // Reads `YYYY-MM-DDTHH:MM:SSZ`, or a date alone, `YYYY-MM-DD`, as its
    // midnight; `to_iso`'s form, nothing else, except that the seconds may
    // carry a fraction (`:37.25Z`, D-0181), which `from_iso` drops and
    // `unix_ms_from_iso` keeps to the millisecond. A year is four digits, or
    // a sign and four or more. `Invalid(i)` gives the first byte that does
    // not fit (the length when the text stops short); `OutOfRange` a field
    // that names no moment (a month 13, a 29 February in a common year, a
    // second 60) or a year beyond `i64`.
    export fn DateTime::from_iso(StringView text) : Result<DateTime, ParseError>
    {
        u16 ms = 0;
        DateTime::read_iso(text, &mut ms)
    }

    // `from_iso`, with the milliseconds of the fraction (rounded down) in
    // `ms`.
    fn DateTime::read_iso(StringView text, ref<u16, exclusive> ms) : Result<DateTime, ParseError>
    {
        usize n = slice_len(text.bytes);
        if (n == 0)
        {
            return Err(Empty);
        }
        usize i = 0;
        bool negative = false;
        bool signed = false;
        if (text.bytes[0] == b'+' || text.bytes[0] == b'-')
        {
            negative = text.bytes[0] == b'-';
            signed = true;
            i = 1;
        }
        usize start = i;
        i64 year = 0;
        bool too_big = false;
        while (i < n && ascii_is_digit(text.bytes[i]))
        {
            i64 d = widen<i64>(text.bytes[i] - 48);
            year = match (checked_mul(year, 10))
            {
                Some(y) : match (checked_add(y, d))
                {
                    Some(z) : z,
                    None    :
                    {
                        too_big = true;
                        0
                    },
                },
                None :
                {
                    too_big = true;
                    0
                },
            };
            i += 1;
        }
        if (i - start < 4)
        {
            return Err(Invalid(i));
        }
        if (!signed && i - start > 4)
        {
            return Err(Invalid(start + 4));
        }
        if (negative)
        {
            year = 0 - year;
        }
        if (!iso_byte(text, i, b'-'))
        {
            return Err(Invalid(i));
        }
        i64 month = match (iso_digits(text, i + 1, 2))
        {
            Ok(x)   : x,
            Err(at) : return Err(Invalid(at)),
        };
        if (!iso_byte(text, i + 3, b'-'))
        {
            return Err(Invalid(i + 3));
        }
        i64 day = match (iso_digits(text, i + 4, 2))
        {
            Ok(x)   : x,
            Err(at) : return Err(Invalid(at)),
        };
        i += 6;
        i64 hour = 0;
        i64 minute = 0;
        i64 second = 0;
        if (i < n)
        {
            if (!iso_byte(text, i, b'T'))
            {
                return Err(Invalid(i));
            }
            hour = match (iso_digits(text, i + 1, 2))
            {
                Ok(x)   : x,
                Err(at) : return Err(Invalid(at)),
            };
            if (!iso_byte(text, i + 3, b':'))
            {
                return Err(Invalid(i + 3));
            }
            minute = match (iso_digits(text, i + 4, 2))
            {
                Ok(x)   : x,
                Err(at) : return Err(Invalid(at)),
            };
            if (!iso_byte(text, i + 6, b':'))
            {
                return Err(Invalid(i + 6));
            }
            second = match (iso_digits(text, i + 7, 2))
            {
                Ok(x)   : x,
                Err(at) : return Err(Invalid(at)),
            };
            i += 9;
            if (iso_byte(text, i, b'.'))
            {
                usize start_frac = i + 1;
                i = start_frac;
                u16 scale = 100;
                u16 frac = 0;
                while (i < n && ascii_is_digit(text.bytes[i]))
                {
                    frac = frac + widen<u16>(text.bytes[i] - 48) * scale;
                    scale = scale / 10;
                    i += 1;
                }
                if (i == start_frac)
                {
                    return Err(Invalid(i));
                }
                *ms = frac;
            }
            if (!iso_byte(text, i, b'Z'))
            {
                return Err(Invalid(i));
            }
            i += 1;
            if (i < n)
            {
                return Err(Invalid(i));
            }
        }
        if (too_big)
        {
            return Err(OutOfRange);
        }
        match (DateTime::new(
            year,
            narrow<u8>(month),
            narrow<u8>(day),
            narrow<u8>(hour),
            narrow<u8>(minute),
            narrow<u8>(second),
        ))
        {
            Some(t) : Ok(t),
            None    : Err(OutOfRange),
        }
    }

    // Whether `a` and `b` are the same moment.
    export fn DateTime::eq(ref<DateTime, shared> a, ref<DateTime, shared> b) : bool
    {
        a.year == b.year && a.month == b.month && a.day == b.day && a.hour == b.hour && a.minute == b.minute && a.second == b.second
    }

    // Whether `a` is earlier than `b`; fits `Vec::sort_by`.
    export fn DateTime::less(ref<DateTime, shared> a, ref<DateTime, shared> b) : bool
    {
        if (a.year != b.year)
        {
            return a.year < b.year;
        }
        if (a.month != b.month)
        {
            return a.month < b.month;
        }
        if (a.day != b.day)
        {
            return a.day < b.day;
        }
        if (a.hour != b.hour)
        {
            return a.hour < b.hour;
        }
        if (a.minute != b.minute)
        {
            return a.minute < b.minute;
        }
        a.second < b.second
    }

    // The wall clock in milliseconds since 1970-01-01T00:00:00Z.
    export fn unix_ms() : i64
    {
        unsafe
        {
            clock_read(2)
        }
    }

    // D-0147: how many seconds the machine's local time is ahead of UTC
    // (behind, when negative) at the moment `unix` seconds after
    // 1970-01-01T00:00:00Z, daylight saving included, by the operating
    // system's rule for the zone the program runs in. For a moment the system
    // has no rule for, the offset now.
    export fn local_offset_seconds(i64 unix) : i64
    {
        unsafe
        {
            tz_offset(unix)
        }
    }

    // The moment `t` (in UTC) written in the machine's local time. The result
    // is a `DateTime` like any other and knows nothing of its zone: print its
    // fields, not `to_iso` (which writes `Z`), and keep the UTC value for
    // arithmetic. A moment within a day of an `i64` limit is
    // `diag.invalid-datetime`.
    export fn DateTime::to_local(ref<DateTime, shared> t) : DateTime
    {
        i64 s = DateTime::to_unix(t);
        match (checked_add(s, local_offset_seconds(s)))
        {
            Some(v) : DateTime::from_unix(v),
            None    : fault(invalid_datetime),
        }
    }

    // The clocks (D-0061, moved here from `std::sys` by D-0176): a monotonic
    // count for measuring durations, the wall clock in seconds, and sleeping.

    // A monotonic clock, in nanoseconds from an arbitrary start: for measuring
    // how long something takes; it never goes backwards.
    export fn monotonic_ns() : u64
    {
        reinterpret<u64>(unsafe
        {
            clock_read(0)
        })
    }

    // The wall clock: whole seconds since 1970-01-01 00:00 UTC.
    export fn unix_seconds() : i64
    {
        unsafe
        {
            clock_read(1)
        }
    }

    // D-0124: the calling thread does nothing for at least `ms` milliseconds;
    // other threads run meanwhile.
    export fn sleep_ms(u64 ms)
    {
        i64 ignored = unsafe
        {
            sleep_ns(wrapping_mul(ms, 1000000))
        };
    }

    // ---- D-0181: milliseconds in ISO text, and HTTP dates ----

    // `to_iso` of the moment `ms` milliseconds after 1970-01-01T00:00:00Z,
    // with the milliseconds: `2026-10-07T12:34:56.789Z`. A log line's or a
    // JSON document's timestamp.
    export fn iso_from_unix_ms(i64 ms) : String
    {
        i64 s = floor_div(ms, 1000);
        i64 frac = floor_mod(ms, 1000);
        DateTime t = DateTime::from_unix(s);
        String out = DateTime::to_iso(&t);
        String::truncate(&mut out, String::len(&out) - 1);
        String::appendf(&mut out, ".%03dZ", frac);
        out
    }

    // `from_iso`'s text as milliseconds since 1970-01-01T00:00:00Z, a
    // fraction of the seconds kept to the millisecond (`.5` is 500, `.1234`
    // is 123). The errors are `from_iso`'s; a moment beyond `i64`
    // milliseconds is `OutOfRange`.
    export fn unix_ms_from_iso(StringView text) : Result<i64, ParseError>
    {
        u16 ms = 0;
        DateTime t = DateTime::read_iso(text, &mut ms)?;
        match (checked_mul(DateTime::to_unix(&t), 1000))
        {
            Some(x) : match (checked_add(x, widen<i64>(ms)))
            {
                Some(y) : Ok(y),
                None    : Err(OutOfRange),
            },
            None : Err(OutOfRange),
        }
    }

    fn month_name(u8 m) : str
    {
        match (m)
        {
            1  : "Jan",
            2  : "Feb",
            3  : "Mar",
            4  : "Apr",
            5  : "May",
            6  : "Jun",
            7  : "Jul",
            8  : "Aug",
            9  : "Sep",
            10 : "Oct",
            11 : "Nov",
            _  : "Dec",
        }
    }

    fn month_number(StringView name) : Option<u8>
    {
        for (u8 m = 1; m <= 12; m += 1)
        {
            if (StringView::eq_str(name, month_name(m)))
            {
                return Some(m);
            }
        }
        None
    }

    // The moment in HTTP's date form (RFC 9110 §5.6.7, the IMF-fixdate):
    // `Sun, 06 Nov 1994 08:49:37 GMT`, as the `Date`, `Last-Modified` and
    // `Expires` headers carry it. The year is written in four digits
    // (`diag.invalid-datetime` outside 0 ..= 9999).
    export fn DateTime::to_http_date(ref<DateTime, shared> t) : String
    {
        if (t.year < 0 || t.year > 9999)
        {
            fault(invalid_datetime);
        }
        Weekday w = DateTime::weekday(t);
        str day = Weekday::text(&w);
        String out = String::new();
        String::append(&mut out, StringView::sub(StringView::of(day), 0, 3));
        String::appendf(
            &mut out,
            ", %02d %s %04d %02d:%02d:%02d GMT",
            t.day,
            month_name(t.month),
            t.year,
            t.hour,
            t.minute,
            t.second,
        );
        out
    }

    // The words of `text` and where each starts.
    fn http_date_words(StringView text, ref<Vec<usize>, exclusive> starts) : Vec<StringView>
    {
        Vec<StringView> out = Vec::new();
        usize n = slice_len(text.bytes);
        usize i = 0;
        while (i < n)
        {
            while (i < n && ascii_is_space(text.bytes[i]))
            {
                i += 1;
            }
            usize start = i;
            while (i < n && !ascii_is_space(text.bytes[i]))
            {
                i += 1;
            }
            if (i > start)
            {
                Vec::push(&mut out, StringView::sub(text, start, i));
                Vec::push(starts, start);
            }
        }
        out
    }

    // `hh:mm:ss` into its parts.
    fn http_time(StringView w, ref<u8, exclusive> h, ref<u8, exclusive> m, ref<u8, exclusive> s) : bool
    {
        if (StringView::len(w) != 8 || !iso_byte(w, 2, b':') || !iso_byte(w, 5, b':'))
        {
            return false;
        }
        i64 a = match (iso_digits(w, 0, 2)) { Ok(x) : x, Err(_) : return false };
        i64 b = match (iso_digits(w, 3, 2)) { Ok(x) : x, Err(_) : return false };
        i64 c = match (iso_digits(w, 6, 2)) { Ok(x) : x, Err(_) : return false };
        *h = narrow<u8>(a);
        *m = narrow<u8>(b);
        *s = narrow<u8>(c);
        true
    }

    // Reads an HTTP date in any of the three forms a recipient must accept
    // (RFC 9110 §5.6.7): `Sun, 06 Nov 1994 08:49:37 GMT` (the one to send),
    // the obsolete `Sunday, 06-Nov-94 08:49:37 GMT` (a two-digit year: 70 to
    // 99 are the 1900s, 00 to 69 the 2000s) and `Sun Nov  6 08:49:37 1994`.
    // The weekday is not checked against the date. `Invalid(i)` names the
    // first word that does not fit; `OutOfRange` fields that name no moment.
    export fn DateTime::from_http_date(StringView text) : Result<DateTime, ParseError>
    {
        Vec<usize> at = Vec::new();
        Vec<StringView> w = http_date_words(text, &mut at);
        if (Vec::len(&w) == 0)
        {
            return Err(Empty);
        }
        i64 year = 0;
        u8 month = 0;
        u8 day = 0;
        u8 hour = 0;
        u8 minute = 0;
        u8 second = 0;
        if (Vec::len(&w) == 6 && StringView::eq_str(w[5], "GMT"))
        {
            // IMF-fixdate: `Sun, 06 Nov 1994 08:49:37 GMT`.
            if (!StringView::ends_with(w[0], ",")) { return Err(Invalid(at[0])); }
            day = match (iso_digits(w[1], 0, 2)) { Ok(x) : narrow<u8>(x), Err(_) : return Err(Invalid(at[1])) };
            if (StringView::len(w[1]) != 2) { return Err(Invalid(at[1])); }
            month = match (month_number(w[2])) { Some(m) : m, None : return Err(Invalid(at[2])) };
            year = match (iso_digits(w[3], 0, 4)) { Ok(x) : x, Err(_) : return Err(Invalid(at[3])) };
            if (StringView::len(w[3]) != 4) { return Err(Invalid(at[3])); }
            if (!http_time(w[4], &mut hour, &mut minute, &mut second)) { return Err(Invalid(at[4])); }
        }
        else if (Vec::len(&w) == 4 && StringView::eq_str(w[3], "GMT"))
        {
            // RFC 850: `Sunday, 06-Nov-94 08:49:37 GMT`.
            if (!StringView::ends_with(w[0], ",")) { return Err(Invalid(at[0])); }
            StringView d = w[1];
            if (StringView::len(d) != 9 || !iso_byte(d, 2, b'-') || !iso_byte(d, 6, b'-')) { return Err(Invalid(at[1])); }
            day = match (iso_digits(d, 0, 2)) { Ok(x) : narrow<u8>(x), Err(_) : return Err(Invalid(at[1])) };
            month = match (month_number(StringView::sub(d, 3, 6))) { Some(m) : m, None : return Err(Invalid(at[1])) };
            i64 yy = match (iso_digits(d, 7, 2)) { Ok(x) : x, Err(_) : return Err(Invalid(at[1])) };
            year = if (yy >= 70) { 1900 + yy } else { 2000 + yy };
            if (!http_time(w[2], &mut hour, &mut minute, &mut second)) { return Err(Invalid(at[2])); }
        }
        else if (Vec::len(&w) == 5)
        {
            // asctime: `Sun Nov  6 08:49:37 1994`.
            month = match (month_number(w[1])) { Some(m) : m, None : return Err(Invalid(at[1])) };
            usize dl = StringView::len(w[2]);
            if (dl == 0 || dl > 2) { return Err(Invalid(at[2])); }
            day = match (iso_digits(w[2], 0, dl)) { Ok(x) : narrow<u8>(x), Err(_) : return Err(Invalid(at[2])) };
            if (!http_time(w[3], &mut hour, &mut minute, &mut second)) { return Err(Invalid(at[3])); }
            year = match (iso_digits(w[4], 0, 4)) { Ok(x) : x, Err(_) : return Err(Invalid(at[4])) };
            if (StringView::len(w[4]) != 4) { return Err(Invalid(at[4])); }
        }
        else
        {
            return Err(Invalid(at[0]));
        }
        match (DateTime::new(year, month, day, hour, minute, second))
        {
            Some(t) : Ok(t),
            None    : Err(OutOfRange),
        }
    }

**Depends on:** rule.stdlib.env, rule.stdlib.text, rule.stdlib.stringview, D-0140, D-0147
**Affects:** none (pure, but for the clocks and the local offset)

## 2k. Processes

In `std::process` (D-0143).

### `rule.stdlib.process`
**Status:** ACCEPTED

A `Command` names a program and how to run it; running it starts a
child process of the operating system. **No shell is involved:** the
program is the one named (a name without a separator is looked for on
`PATH`, as the system does), and each argument reaches it exactly as
given, one argument per `arg`, with no splitting, quoting or expansion.
A program that wants a shell runs one by name (`/bin/sh` with `-c`). The
child inherits this program's environment unless `clear_env` was
called, with each `env` variable set on top, and starts in this
program's working directory unless `current_dir` names another.

A status is an `i32`: the child's exit code, or, on Unix, minus the
number of the signal that ended it (`-9` for SIGKILL), as shells report
it in effect.

    [Command]          ⟨Command::new(p)⟩ → a command running p with no arguments, here, with this
                       environment; arg(&mut c, a) appends one argument, current_dir sets the
                       directory, env sets a variable, clear_env empties the inherited environment.
                       A command may be run any number of times. Failures are FileErrors (D-0134):
                       Err(NotFound) when there is no
                       such program; Err(Denied) when it may not be run; Err(Io) for any other
                       failure to start it (a working directory that does not exist among them)
    [Output]           ⟨Command::output(&c), Σ⟩ → Ok(Output { status, stdout, stderr }): the child run
                       to its end with an empty standard input, every byte it wrote to each stream
                       captured (both read at once, so neither pipe fills and stops it);
                       output_with_input gives it `input` as standard input
    [Status]           ⟨Command::status(&c), Σ⟩ → Ok(status): the child run to its end with this
                       program's standard input, output and error
    [Spawn-Child]      ⟨Command::spawn(&c), Σ⟩ → Ok(child), at once; the child's three streams are
                       pipes to this program
    [Child-Streams]    write_input writes all of `data` to the child's standard input; close_input
                       closes it (the child reads its end); read_output and read_error append what
                       has arrived on a pipe, up to `max` bytes (at most a MiB), waiting for at least
                       one, and give 0 at its end; read_output_line gives the next line without its
                       '\n' or a '\r' before it, None at the end, Err(Utf8(e)) for one that is not
                       UTF-8. A child that fills a pipe nobody reads stops until it is read; a
                       write to a child that has ended (or closed its input) is Err(Io), never
                       the program's end
    [Wait]             ⟨Child::wait(&mut ch), Σ⟩ → Ok(status) when the child has ended, its standard
                       input closed first; again, the same status. try_wait gives Some(status) or,
                       while it runs, None, without waiting
    [Kill]             ⟨Child::kill(&mut ch), Σ⟩ → Ok(()), the child ended at once (SIGKILL on Unix,
                       TerminateProcess on Windows); nothing for one that has ended. A dropped `Child`
                       has its pipes closed and is not ended: a running child runs on
    [Exit]             ⟨exit(s), Σ⟩ (s : u8) ↛↛ terminate(ok(s), Σ')   by rule.fn.program [Terminate-Exit]
                       (D-0150): the calling thread's frames are unwound and their destructors run,
                       other threads take no further steps, and the program ends with status s. Of
                       type never, so it stands where any value is expected. Realized natively by
                       both implementations (it ends the program, which no CobaltC body can)
    [Interrupt-Flag]   ⟨watch_interrupts()⟩: from then on, an interrupt (SIGINT or SIGTERM on Unix;
                       Ctrl-C, Ctrl-Break or the console closing on Windows) does not end the program
                       but sets a flag; ⟨interrupt_requested()⟩ → the flag. A blocking `std` call an
                       interrupt arrives in may return early (`accept` and a socket's `read` give
                       Err(TimedOut)), so a loop can look at the flag. There is no callback

A child process belongs to the program's environment
(`rule.fn.program`): what it writes and how it ends are whatever the
system and the program run make them.


    [Process-Id]       (D-0181)   ⟨process_id(), Σ⟩ → the operating system's number for the running program
    [Child-Terminate]  (D-0181)   ⟨Child::terminate(ch), Σ⟩ → Ok(()) with SIGTERM sent to the child on Unix (its status
                       then -15 unless it handles the signal), the child killed as [Kill] does elsewhere; nothing,
                       and Ok(()), for a child that has ended
    [Child-Streams]    (D-0181) also: Child::read_error_line(ch) is to standard error what read_output_line is to
                       standard output (each stream read ahead on its own); read_error takes what was read ahead first
    [Child-Pipes]      (D-0195)   ⟨Child::take_input(ch), Σ⟩ → Some(p), the child's standard input moved into p (a
                       ChildInput), or None when it was taken or closed; Child::write_input and close_input then
                       do nothing. ⟨Child::take_output(ch), Σ⟩ → Some(q), its standard output with what was
                       read ahead moved into q (a ChildOutput), or None; Child::read_output then finds the end.
                       ChildInput::write is write_input's, ChildOutput::read and read_line are read_output's and
                       read_output_line's. Each end is independent of ch and of the other end: one thread may
                       write it while another reads (one child's output feeding another's input), and it outlives
                       ch. Destroying an end closes it

The `std` source:

    // `std::process` (spec/21 §2k, D-0143): running other programs, and
    // interrupt requests. A failure is a `FileError`, as every input and
    // output failure is (D-0134): `NotFound` for a program that is not there
    // (nor on `PATH`, for a bare name). Written over `proc_op` (src/procio.rs), shared by
    // both implementations. No shell is ever involved: a program that wants
    // one runs `/bin/sh` with `-c` and says so.

    // A program to run: its name or path, its arguments, and how to run it.
    // Made by `Command::new` and changed by `arg`, `current_dir`, `env` and
    // `clear_env`; run by `output`, `output_with_input`, `status` or `spawn`,
    // any number of times.
    export struct Command
    {
        Vec<String> program_and_args;
        Option<String> dir;
        Vec<String> env_names;
        Vec<String> env_values;
        bool clear_env;
    }

    // How a run of `output` ended: the exit status (minus the signal that
    // ended it, on Unix: `-9` for SIGKILL) and every byte it wrote to its
    // standard output and standard error.
    export struct Output
    {
        export i32 status;
        export Vec<u8> stdout;
        export Vec<u8> stderr;
    }

    // The program `program`, with no arguments, run in this program's
    // working directory with its environment.
    export fn Command::new(StringView program) : Command
    {
        Vec<String> v = Vec::new();
        Vec::push(&mut v, String::from_view(program));
        Command { .program_and_args = v, .dir = None, .env_names = Vec::new(), .env_values = Vec::new(), .clear_env = false }
    }

    // Adds one argument, passed to the program exactly as given.
    export fn Command::arg(ref<Command, exclusive> c, StringView a)
    {
        Vec::push(&mut c.program_and_args, String::from_view(a));
    }

    // The working directory the program starts in.
    export fn Command::current_dir(ref<Command, exclusive> c, StringView dir)
    {
        c.dir = Some(String::from_view(dir));
    }

    // Sets an environment variable for the program (after `clear_env`, if
    // that is called too).
    export fn Command::env(ref<Command, exclusive> c, StringView name, StringView value)
    {
        Vec::push(&mut c.env_names, String::from_view(name));
        Vec::push(&mut c.env_values, String::from_view(value));
    }

    // The program starts with no environment variables but those `env` sets.
    export fn Command::clear_env(ref<Command, exclusive> c)
    {
        c.clear_env = true;
    }

    fn Command::put(ref<Vec<u8>, exclusive> out, ref<String, shared> s)
    {
        Vec::push_le(out, widen<u64>(String::len(s)));
        foreach (b in &s.bytes)
        {
            Vec::push(out, *b);
        }
    }

    // The command as `proc_op` reads it (src/procio.rs).
    fn Command::encode(ref<Command, shared> c) : Vec<u8>
    {
        Vec<u8> out = Vec::new();
        Vec::push_le(&mut out, widen<u64>(Vec::len(&c.program_and_args)));
        foreach (a in &c.program_and_args)
        {
            Command::put(&mut out, a);
        }
        Vec::push(&mut out, if (c.clear_env) { 1: u8 } else { 0: u8 });
        match (&c.dir)
        {
            Some(d) :
            {
                Vec::push(&mut out, 1: u8);
                Command::put(&mut out, d);
            },
            None    : Vec::push(&mut out, 0: u8),
        }
        Vec::push_le(&mut out, widen<u64>(Vec::len(&c.env_names)));
        for (usize i = 0; i < Vec::len(&c.env_names); i += 1)
        {
            Command::put(&mut out, &c.env_names[i]);
            Command::put(&mut out, &c.env_values[i]);
        }
        out
    }

    fn Command::run(ref<Command, shared> c, slice<u8, shared> input) : Result<Output, FileError>
    {
        Vec<u8> spec = Command::encode(c);
        i64 h = byte_op(
            1,
            0,
            0,
            bytes_ptr(&spec[0..$]),
            Vec::len(&spec),
            bytes_ptr(input),
            slice_len(input),
            dangling<u8>(),
            0,
        );
        if (h < 0)
        {
            return Err(code_error(h));
        }
        Vec<u8> rec = match (byte_answer(1, 1, reinterpret<usize>(h), &spec[0..0], &spec[0..0]))
        {
            Ok(r)  : r,
            Err(e) : return Err(code_error(e)),
        };
        usize n = narrow<usize>(read_le<u64>(&rec[0..$], 4));
        Ok(Output { .status = read_le<i32>(
            &rec[0..$],
            0,
        ), .stdout = Vec::from_slice(&rec[12..12 + n]), .stderr = Vec::from_slice(&rec[12 + n..$]) })
    }

    // Runs the program to its end with an empty standard input, and gives its
    // status and everything it wrote.
    export fn Command::output(ref<Command, shared> c) : Result<Output, FileError>
    {
        Command::run(c, StringView::of("").bytes)
    }

    // The same, with `input` as the program's standard input.
    export fn Command::output_with_input(ref<Command, shared> c, slice<u8, shared> input) : Result<Output, FileError>
    {
        Command::run(c, input)
    }

    // Runs the program to its end with this program's standard input, output
    // and error, and gives its status.
    export fn Command::status(ref<Command, shared> c) : Result<i32, FileError>
    {
        Vec<u8> spec = Command::encode(c);
        match (byte_answer(1, 2, 0, &spec[0..$], &spec[0..0]))
        {
            Ok(b)  : Ok(read_le<i32>(&b[0..$], 0)),
            Err(e) : Err(code_error(e)),
        }
    }

    // A running child, its standard input, output and error each a pipe to
    // this program. Dropping it closes the pipes but does not end it.
    export resource struct Child
    {
        usize id;
    }

    // Starts the program and returns at once.
    export fn Command::spawn(ref<Command, shared> c) : Result<Child, FileError>
    {
        Vec<u8> spec = Command::encode(c);
        i64 h = byte_op(1, 3, 0, bytes_ptr(&spec[0..$]), Vec::len(&spec), dangling<u8>(), 0, dangling<u8>(), 0);
        if (h < 0)
        {
            return Err(code_error(h));
        }
        Ok(Child { .id = reinterpret<usize>(h) })
    }

    fn Child::op(ref<Child, shared> ch, usize op, slice<u8, shared> data) : i64
    {
        byte_op(1, op, ch.id, dangling<u8>(), 0, bytes_ptr(data), slice_len(data), dangling<u8>(), 0)
    }

    fn Child::done(i64 r) : Result<void, FileError>
    {
        if (r < 0)
        {
            return Err(code_error(r));
        }
        Ok(())
    }

    // The operating system's number for the child.
    export fn Child::id(ref<Child, shared> ch) : u64
    {
        reinterpret<u64>(Child::op(ch, 4, StringView::of("").bytes))
    }

    // Writes all of `data` to the child's standard input.
    export fn Child::write_input(ref<Child, exclusive> ch, slice<u8, shared> data) : Result<void, FileError>
    {
        Child::done(Child::op(ch, 5, data))
    }

    // Closes the child's standard input: it reads the end there.
    export fn Child::close_input(ref<Child, exclusive> ch)
    {
        i64 r = Child::op(ch, 6, StringView::of("").bytes);
    }

    // Appends up to `want` bytes of a pipe (`op` 7 standard output, 9
    // standard error) to `buf`; how many, 0 at its end.
    fn Child::take(ref<Child, exclusive> ch, usize op, ref<Vec<u8>, exclusive> buf, usize want) : Result<usize, FileError>
    {
        while (buf.cap - buf.len < want)
        {
            Vec::grow(buf);
        }
        rawptr<u8> at = unsafe
        {
            reinterpret_ptr<u8>(buf.ptr + reinterpret<isize>(buf.len))
        };
        i64 r = byte_op(1, op, ch.id, dangling<u8>(), 0, dangling<u8>(), 0, at, want);
        if (r < 0)
        {
            return Err(code_error(r));
        }
        usize n = reinterpret<usize>(r);
        buf.len = buf.len + n;
        Ok(n)
    }

    fn Child::cap(usize max) : usize
    {
        if (max > 1_048_576)
        {
            return 1_048_576;
        }
        max
    }

    // Appends up to `max` bytes of the child's standard output to `buf`, as
    // many as are ready (at most a MiB), and returns how many: 0 only at its
    // end, when `max` > 0.
    export fn Child::read_output(
        ref<Child, exclusive> ch,
        ref<Vec<u8>, exclusive> buf,
        usize max,
    ) : Result<usize, FileError>
    {
        Child::take(ch, 7, buf, Child::cap(max))
    }

    // The next line of the child's standard output without its '\n' (nor a
    // '\r' before it); None at its end.
    export fn Child::read_output_line(ref<Child, exclusive> ch) : Result<Option<String>, FileError>
    {
        i64 r = Child::op(ch, 8, StringView::of("").bytes);
        if (r < 0)
        {
            return Err(code_error(r));
        }
        if (r == 0)
        {
            return Ok(None);
        }
        Vec<u8> bytes = Vec::new();
        usize n = Child::take(ch, 7, &mut bytes, reinterpret<usize>(r))?;
        if (n > 0 && bytes[n - 1] == 10)
        {
            Vec::pop(&mut bytes);
            if (n > 1 && bytes[n - 2] == 13)
            {
                Vec::pop(&mut bytes);
            }
        }
        match (String::from_utf8(bytes))
        {
            Ok(s)  : Ok(Some(s)),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

    // As `read_output`, from the child's standard error.
    export fn Child::read_error(ref<Child, exclusive> ch, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, FileError>
    {
        Child::take(ch, 9, buf, Child::cap(max))
    }

    // D-0181: as `read_output_line`, from the child's standard error.
    export fn Child::read_error_line(ref<Child, exclusive> ch) : Result<Option<String>, FileError>
    {
        i64 r = Child::op(ch, 18, StringView::of("").bytes);
        if (r < 0)
        {
            return Err(code_error(r));
        }
        if (r == 0)
        {
            return Ok(None);
        }
        Vec<u8> bytes = Vec::new();
        usize n = Child::take(ch, 9, &mut bytes, reinterpret<usize>(r))?;
        if (n > 0 && bytes[n - 1] == 10)
        {
            Vec::pop(&mut bytes);
            if (n > 1 && bytes[n - 2] == 13)
            {
                Vec::pop(&mut bytes);
            }
        }
        match (String::from_utf8(bytes))
        {
            Ok(s)  : Ok(Some(s)),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

    // Closes the child's standard input and waits for it to end; its status,
    // as `Output`'s. Asked again, the same status.
    export fn Child::wait(ref<Child, exclusive> ch) : Result<i32, FileError>
    {
        match (byte_answer(1, 10, ch.id, StringView::of("").bytes, StringView::of("").bytes))
        {
            Ok(b)  : Ok(read_le<i32>(&b[0..$], 0)),
            Err(e) : Err(code_error(e)),
        }
    }

    // The child's status if it has ended, without waiting; None while it runs.
    export fn Child::try_wait(ref<Child, exclusive> ch) : Result<Option<i32>, FileError>
    {
        Vec<u8> b = Vec::new();
        while (b.cap < 4)
        {
            Vec::grow(&mut b);
        }
        i64 r = byte_op(1, 11, ch.id, dangling<u8>(), 0, dangling<u8>(), 0, reinterpret_ptr<u8>(b.ptr), 4);
        if (r < 0)
        {
            return Err(code_error(r));
        }
        if (r == 0)
        {
            return Ok(None);
        }
        b.len = 4;
        Ok(Some(read_le<i32>(&b[0..$], 0)))
    }

    // Ends the child at once (SIGKILL on Unix); `wait` then gives its status.
    // Nothing happens to a child that has already ended.
    export fn Child::kill(ref<Child, exclusive> ch) : Result<void, FileError>
    {
        Child::done(Child::op(ch, 12, StringView::of("").bytes))
    }

    // D-0205: ends the child and every process it started, and theirs in
    // turn (Linux: found from /proc; Windows: `taskkill /T`; elsewhere the child
    // alone, as `kill`). `wait` then gives its status.
    export fn Child::kill_tree(ref<Child, exclusive> ch) : Result<void, FileError>
    {
        Child::done(Child::op(ch, 26, StringView::of("").bytes))
    }

    // D-0181: asks the child to end (SIGTERM on Unix, which a program may
    // catch to stop cleanly; on Windows, as `kill`); `wait` then gives its
    // status. Nothing happens to a child that has already ended.
    export fn Child::terminate(ref<Child, exclusive> ch) : Result<void, FileError>
    {
        Child::done(Child::op(ch, 17, StringView::of("").bytes))
    }

    export fn Child::drop(ref<Child, exclusive> self)
    {
        i64 r = byte_op(1, 13, self.id, dangling<u8>(), 0, dangling<u8>(), 0, dangling<u8>(), 0);
    }

    // D-0195: a child's standard input or output as a value of its own, so
    // one thread can write a child's input while another reads its output (or
    // another child's): each end is independent of the `Child` and of the
    // other end, and outlives the `Child`. Dropping an end closes it; a child
    // reading its input then finds its end.
    export resource struct ChildInput
    {
        usize id;
    }

    export resource struct ChildOutput
    {
        usize id;
    }

    fn pipe_op(usize op, usize id, slice<u8, shared> data) : i64
    {
        byte_op(1, op, id, dangling<u8>(), 0, bytes_ptr(data), slice_len(data), dangling<u8>(), 0)
    }

    // The child's standard input, taken out of it: `Child::write_input` and
    // `close_input` then do nothing (the end is the `ChildInput`'s). None if it
    // was taken or closed already.
    export fn Child::take_input(ref<Child, exclusive> ch) : Option<ChildInput>
    {
        i64 h = Child::op(ch, 20, StringView::of("").bytes);
        if (h < 0)
        {
            return None;
        }
        Some(ChildInput { .id = reinterpret<usize>(h) })
    }

    // The child's standard output, taken out of it with whatever was read
    // ahead: `Child::read_output` then finds its end. None if taken already.
    export fn Child::take_output(ref<Child, exclusive> ch) : Option<ChildOutput>
    {
        i64 h = Child::op(ch, 21, StringView::of("").bytes);
        if (h < 0)
        {
            return None;
        }
        Some(ChildOutput { .id = reinterpret<usize>(h) })
    }

    // Writes all of `data` to the child's standard input.
    export fn ChildInput::write(ref<ChildInput, exclusive> p, slice<u8, shared> data) : Result<void, FileError>
    {
        Child::done(pipe_op(22, p.id, data))
    }

    export fn ChildInput::drop(ref<ChildInput, exclusive> self)
    {
        i64 r = pipe_op(25, self.id, StringView::of("").bytes);
    }

    // As `Child::read_output`: up to `max` bytes (at most a MiB) appended to
    // `buf`; how many, 0 only at the end.
    export fn ChildOutput::read(
        ref<ChildOutput, exclusive> p,
        ref<Vec<u8>, exclusive> buf,
        usize max,
    ) : Result<usize, FileError>
    {
        usize want = Child::cap(max);
        while (buf.cap - buf.len < want)
        {
            Vec::grow(buf);
        }
        rawptr<u8> at = unsafe
        {
            reinterpret_ptr<u8>(buf.ptr + reinterpret<isize>(buf.len))
        };
        i64 r = byte_op(1, 23, p.id, dangling<u8>(), 0, dangling<u8>(), 0, at, want);
        if (r < 0)
        {
            return Err(code_error(r));
        }
        usize n = reinterpret<usize>(r);
        buf.len = buf.len + n;
        Ok(n)
    }

    // As `Child::read_output_line`.
    export fn ChildOutput::read_line(ref<ChildOutput, exclusive> p) : Result<Option<String>, FileError>
    {
        i64 r = pipe_op(24, p.id, StringView::of("").bytes);
        if (r < 0)
        {
            return Err(code_error(r));
        }
        if (r == 0)
        {
            return Ok(None);
        }
        Vec<u8> bytes = Vec::new();
        usize n = ChildOutput::read(p, &mut bytes, reinterpret<usize>(r))?;
        if (n > 0 && bytes[n - 1] == 10)
        {
            Vec::pop(&mut bytes);
            if (n > 1 && bytes[n - 2] == 13)
            {
                Vec::pop(&mut bytes);
            }
        }
        match (String::from_utf8(bytes))
        {
            Ok(s)  : Ok(Some(s)),
            Err(e) : Err(FileError::Utf8(e)),
        }
    }

    export fn ChildOutput::drop(ref<ChildOutput, exclusive> self)
    {
        i64 r = pipe_op(25, self.id, StringView::of("").bytes);
    }

    // D-0181: the operating system's number for this program (its process
    // id), as a pid file records it.
    export fn process_id() : u64
    {
        reinterpret<u64>(byte_op(1, 16, 0, dangling<u8>(), 0, dangling<u8>(), 0, dangling<u8>(), 0))
    }

    // From now on, an interrupt (Ctrl-C, SIGINT; SIGTERM; on Windows Ctrl-C,
    // Ctrl-Break and closing the console) no longer ends the program: it
    // sets the flag `interrupt_requested` reads, and a blocking call it
    // arrives in may return early. A long-running loop checks the flag and
    // stops cleanly.
    export fn watch_interrupts()
    {
        i64 r = byte_op(1, 14, 0, dangling<u8>(), 0, dangling<u8>(), 0, dangling<u8>(), 0);
    }

    // Whether an interrupt has arrived since `watch_interrupts`.
    export fn interrupt_requested() : bool
    {
        byte_op(1, 15, 0, dangling<u8>(), 0, dangling<u8>(), 0, dangling<u8>(), 0) == 1
    }

**Depends on:** rule.stdlib.env, rule.stdlib.text, rule.fn.program, D-0143
**Affects:** the program's environment

## 2l. Networking

In `std::net` (D-0144).

### `rule.stdlib.net`
**Status:** ACCEPTED

TCP connections, UDP datagrams and name resolution, over IPv4 and IPv6.
**Sockets block:** a call waits until it can complete, until its
socket's timeout passes (`Err(TimedOut)`, after which the socket is
still usable), or until a watched interrupt arrives (`[Interrupt-Flag]`,
§2k). A program serves several connections at once with a thread per
connection (`spawn` with a `move` closure or a function and the moved
`TcpStream`), and coordinates them with channels (§3c); while one
thread waits in a socket call, the others run. There is no TLS, no
non-blocking mode and no framing: **the bytes on the wire are exactly
those written**, in order, with no encoding added or removed, and a
`read` gives whatever part of them has arrived.

    [Addr]             IpAddr is V4(4 bytes) or V6(16 bytes); SocketAddr is an IpAddr and a u16 port.
                       IpAddr::parse reads the platform library's textual forms (dotted decimal; RFC
                       4291 IPv6 with `::`); IpAddr::text writes dotted decimal and RFC 5952's
                       shortest IPv6. SocketAddr::parse reads `a.b.c.d:port` or `[v6]:port`:
                       Err(Empty) for "", Err(Invalid(i)) at the first byte that does not fit (the
                       address's first byte when it is not an address), Err(OutOfRange) for a port
                       above 65535; SocketAddr::text writes the same forms
    [Resolve]          ⟨resolve(host, port), Σ⟩ → Ok(addresses), each with `port`, in the system
                       resolver's order (its hosts file, then DNS); "localhost" without a network;
                       Err(UnknownHost) when the name resolves to nothing or cannot be resolved
    [Bind]             ⟨TcpListener::bind(a), Σ⟩ → Ok(listener) listening at a; port 0 asks the system
                       for a free one (local_addr names it); Err(AddrInUse), Err(AddrNotAvailable),
                       Err(NotPermitted)
    [Accept]           ⟨TcpListener::accept(&mut l), Σ⟩ → Ok(stream) for the next connection, waiting
                       for one; Err(TimedOut) after set_accept_timeout_ms's milliseconds (0: none)
    [Connect]          ⟨TcpStream::connect(a), Σ⟩ → Ok(stream); connect_timeout waits at most `ms`;
                       Err(Refused) when nothing listens there, Err(TimedOut), Err(Unreachable)
    [Stream-Read]      read appends up to `max` bytes (at most a MiB) of what has arrived, waiting
                       until at least one has, and gives how many: 0 exactly when the peer has
                       closed its sending side; read_exact appends exactly `n` or gives Err(Closed)
                       (what arrived stays appended); read_line gives the next line without its
                       '\n' or a '\r' before it, None at the end, Err(NotUtf8(e)) for one that is not
                       UTF-8
    [Stream-Write]     write sends all of `data` (write_text a view's bytes, printf `printf`'s text)
                       or fails: Err(Closed) or Err(Reset) when the peer has gone
    [Stream-Timeout]   set_read_timeout_ms, set_write_timeout_ms (0: none): a read or write that
                       waits longer gives Err(TimedOut), and the stream stays usable; set_nodelay
                       sends small writes at once
    [Shutdown]         ⟨TcpStream::shutdown_write(&mut s), Σ⟩ → Ok(()): the peer reads the end (0)
                       after what was sent; this side can still read. Dropping a stream, listener
                       or UDP socket closes it
    [Udp]              UdpSocket::bind as TcpListener's; send_to sends `data` as one datagram and
                       gives its length; recv_from gives the next datagram's first `max` bytes (the
                       rest of a longer one is lost) and its sender, Err(TimedOut) after
                       set_read_timeout_ms's milliseconds. Datagrams may be lost, duplicated or
                       reordered by the network; on loopback they are not
    [Net-Error]        each failure is the NetError the system's error maps to: Refused, Reset,
                       TimedOut, Unreachable, AddrInUse, AddrNotAvailable, UnknownHost, NotPermitted,
                       Closed (the peer gone, or a read_exact cut short), Failed (any other);
                       NetError::text. No variant shares a name with FileError's

Sockets belong to the program's environment (`rule.fn.program`): what
arrives, and when, is whatever the network and the peer make it.


    [Try-Clone]     (D-0185)   ⟨TcpStream::try_clone(s), Σ⟩ → Ok(s'), a second TcpStream on the same connection (the
                    system's dup): its own read-ahead and timeouts; bytes read through either are not seen by the
                    other; shutdown_write through either ends sending for both; the connection closes when the
                    last stream on it is dropped. Err(e) as [Connect]'s
    [Keepalive]     (D-0181)   ⟨TcpStream::set_keepalive(s, on), Σ⟩ → ⟨(), Σ'⟩ with SO_KEEPALIVE set as `on` says: the
                    system probes an idle connection by its own schedule, so a silently gone peer makes a later
                    read fail rather than wait for ever
    [Udp-Connect]   (D-0181)   ⟨UdpSocket::connect(u, to), Σ⟩ → Ok(()) with `to` as u's peer: ⟨UdpSocket::send(u, d), Σ⟩
                    sends d to it as send_to does and ⟨UdpSocket::recv(u, max), Σ⟩ receives as recv_from does, a
                    datagram from any other address dropped; send and recv on a socket not connected are Err(Failed)

The `std` source:

    // `std::net` (spec/21 §2l, D-0144): TCP, UDP and name resolution, over
    // `net_op` (src/netio.rs), shared by both implementations. Sockets block,
    // each with its own timeouts; a connection is served by its own thread
    // (`spawn` with a `move` closure), and threads coordinate through
    // channels. IPv4 and IPv6; no TLS.

    // An IP address: four bytes, or sixteen.
    export enum IpAddr
    {
        V4(array<u8, 4>),
        V6(array<u8, 16>),
    }

    // An IP address and a port: where a socket is bound or connected.
    export struct SocketAddr
    {
        export IpAddr ip;
        export u16 port;
    }

    // What a socket's operation failed with. `TimedOut` leaves the socket
    // usable; `Closed` is a peer gone while writing, or a read that ended
    // short; `UnknownHost` a name that resolves to nothing; `Failed` any
    // other failure. No variant shares a name with `FileError`'s, so either
    // may be written unqualified where `std` is imported.
    export enum NetError
    {
        Refused,
        Reset,
        TimedOut,
        Unreachable,
        AddrInUse,
        AddrNotAvailable,
        UnknownHost,
        NotPermitted,
        Closed,
        Failed,
        NotUtf8(Utf8Error),
    }

    export fn NetError::text(ref<NetError, shared> e) : str
    {
        match (e)
        {
            Refused          : "connection refused",
            Reset            : "connection reset",
            TimedOut         : "timed out",
            Unreachable      : "unreachable",
            AddrInUse        : "address in use",
            AddrNotAvailable : "address not available",
            UnknownHost      : "unknown host",
            NotPermitted     : "permission denied",
            Closed           : "closed",
            Failed           : "input/output error",
            NotUtf8(_)       : "not UTF-8",
        }
    }

    fn net_error(i64 code) : NetError
    {
        if (code == -1)
        {
            return NetError::UnknownHost;
        }
        if (code == -2)
        {
            return NetError::NotPermitted;
        }
        if (code == -4)
        {
            return NetError::Refused;
        }
        if (code == -5)
        {
            return NetError::Reset;
        }
        if (code == -6)
        {
            return NetError::TimedOut;
        }
        if (code == -7)
        {
            return NetError::Unreachable;
        }
        if (code == -8)
        {
            return NetError::AddrInUse;
        }
        if (code == -9)
        {
            return NetError::AddrNotAvailable;
        }
        if (code == -10)
        {
            return NetError::Closed;
        }
        NetError::Failed
    }

    // `net_op` with a number for `cap` (a timeout, a flag) and nothing out.
    fn net_num(usize op, usize h, slice<u8, shared> a, slice<u8, shared> b, usize n) : i64
    {
        byte_op(2, op, h, bytes_ptr(a), slice_len(a), bytes_ptr(b), slice_len(b), dangling<u8>(), n)
    }

    // ---- addresses ----

    export fn IpAddr::localhost_v4() : IpAddr
    {
        V4([127, 0, 0, 1])
    }

    export fn IpAddr::unspecified_v4() : IpAddr
    {
        V4([0, 0, 0, 0])
    }

    export fn IpAddr::localhost_v6() : IpAddr
    {
        V6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
    }

    export fn IpAddr::unspecified_v6() : IpAddr
    {
        V6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    }

    export fn SocketAddr::new(IpAddr ip, u16 port) : SocketAddr
    {
        SocketAddr { .ip = ip, .port = port }
    }

    // The 19-byte record `net_op` reads (src/netio.rs): the family (4 or 6),
    // 16 bytes of address, the port, little-endian.
    fn SocketAddr::record(ref<SocketAddr, shared> a) : Vec<u8>
    {
        Vec<u8> r = Vec::filled(19, 0: u8);
        match (a.ip)
        {
            V4(b) :
            {
                r[0] = 4;
                for (usize i = 0; i < 4; i += 1)
                {
                    r[1 + i] = b[i];
                }
            },
            V6(b) :
            {
                r[0] = 6;
                for (usize i = 0; i < 16; i += 1)
                {
                    r[1 + i] = b[i];
                }
            },
        }
        r[17] = narrow_wrapping<u8>(a.port);
        r[18] = narrow_wrapping<u8>(a.port >> 8);
        r
    }

    fn SocketAddr::from_record(slice<u8, shared> r) : SocketAddr
    {
        u16 port = read_le<u16>(r, 17);
        if (r[0] == 4)
        {
            return SocketAddr { .ip = V4([r[1], r[2], r[3], r[4]]), .port = port };
        }
        SocketAddr { .ip = V6([
            r[1],
            r[2],
            r[3],
            r[4],
            r[5],
            r[6],
            r[7],
            r[8],
            r[9],
            r[10],
            r[11],
            r[12],
            r[13],
            r[14],
            r[15],
            r[16],
        ]), .port = port }
    }

    // Reads an address as the platform's library does: `127.0.0.1`, `::1`,
    // `2001:db8::8a2e:370:7334`, `::ffff:192.0.2.1`. `Invalid(0)` for text
    // that is not one.
    export fn IpAddr::parse(StringView text) : Result<IpAddr, ParseError>
    {
        if (StringView::len(text) == 0)
        {
            return Err(Empty);
        }
        match (byte_answer(2, 18, 0, text.bytes, StringView::of("").bytes))
        {
            Ok(r)  : Ok(SocketAddr::from_record(&r[0..$]).ip),
            Err(_) : Err(Invalid(0)),
        }
    }

    // The address as text, IPv6 in its shortest form (`::1`).
    export fn IpAddr::text(ref<IpAddr, shared> ip) : String
    {
        SocketAddr a = SocketAddr { .ip = *ip, .port = 0 };
        Vec<u8> r = SocketAddr::record(&a);
        match (byte_answer(2, 19, 0, &r[0..$], StringView::of("").bytes))
        {
            Ok(t)  : Result::unwrap_or(String::from_utf8(t), String::new()),
            Err(_) : String::new(),
        }
    }

    // Reads `127.0.0.1:8080` or `[::1]:8080`. `Invalid(i)` at the first byte
    // that does not fit (the address's first byte when the address is not
    // one); `OutOfRange` for a port above 65535.
    export fn SocketAddr::parse(StringView text) : Result<SocketAddr, ParseError>
    {
        usize n = StringView::len(text);
        if (n == 0)
        {
            return Err(Empty);
        }
        usize lo = 0;
        usize hi = 0;
        usize colon = 0;
        if (text.bytes[0] == b'[')
        {
            lo = 1;
            hi = match (StringView::find(text, "]"))
            {
                Some(i) : i,
                None    : return Err(Invalid(n)),
            };
            colon = hi + 1;
            if (colon >= n || text.bytes[colon] != b':')
            {
                return Err(Invalid(colon));
            }
        }
        else
        {
            usize i = n;
            while (i > 0 && text.bytes[i - 1] != b':')
            {
                i -= 1;
            }
            if (i == 0)
            {
                return Err(Invalid(n));
            }
            colon = i - 1;
            hi = colon;
            for (usize j = 0; j < colon; j += 1)
            {
                if (text.bytes[j] == b':')
                {
                    return Err(Invalid(j));
                }
            }
        }
        IpAddr ip = match (IpAddr::parse(StringView::sub(text, lo, hi)))
        {
            Ok(ip) : ip,
            Err(_) : return Err(Invalid(lo)),
        };
        if (text.bytes[0] == b'[')
        {
            if (V4(_) = ip)
            {
                return Err(Invalid(lo));
            }
        }
        else if (V6(_) = ip)
        {
            return Err(Invalid(lo));
        }
        StringView digits = StringView::sub(text, colon + 1, n);
        if (StringView::len(digits) == 0)
        {
            return Err(Invalid(n));
        }
        for (usize j = 0; j < StringView::len(digits); j += 1)
        {
            if (!ascii_is_digit(digits.bytes[j]))
            {
                return Err(Invalid(colon + 1 + j));
            }
        }
        match (StringView::parse<u16>(digits))
        {
            Ok(p)  : Ok(SocketAddr { .ip = ip, .port = p }),
            Err(_) : Err(OutOfRange),
        }
    }

    // `127.0.0.1:8080`, `[::1]:8080`.
    export fn SocketAddr::text(ref<SocketAddr, shared> a) : String
    {
        String ip = IpAddr::text(&a.ip);
        match (a.ip)
        {
            V4(_) : sprintf("%s:%u", &ip, a.port),
            V6(_) : sprintf("[%s]:%u", &ip, a.port),
        }
    }

    // The addresses `host` names (through the system's resolver: the hosts
    // file, then DNS), each with `port`. "localhost" resolves without a
    // network. `Err(UnknownHost)` when the name resolves to nothing.
    export fn resolve(StringView host, u16 port) : Result<Vec<SocketAddr>, NetError>
    {
        Vec<u8> p = Vec::new();
        Vec::push_le(&mut p, port);
        Vec<u8> all = match (byte_answer(2, 0, 0, host.bytes, &p[0..$]))
        {
            Ok(b)  : b,
            Err(c) : return Err(net_error(c)),
        };
        Vec<SocketAddr> out = Vec::new();
        for (usize i = 0; i + 19 <= Vec::len(&all); i += 19)
        {
            Vec::push(&mut out, SocketAddr::from_record(&all[i..i + 19]));
        }
        Ok(out)
    }

    fn net_addr(usize op, usize h) : SocketAddr
    {
        match (byte_answer(2, op, h, StringView::of("").bytes, StringView::of("").bytes))
        {
            Ok(r)  : SocketAddr::from_record(&r[0..$]),
            Err(_) : SocketAddr { .ip = IpAddr::unspecified_v4(), .port = 0 },
        }
    }

    fn net_open(usize op, ref<SocketAddr, shared> a, usize n) : Result<usize, NetError>
    {
        Vec<u8> r = SocketAddr::record(a);
        i64 h = net_num(op, 0, &r[0..$], StringView::of("").bytes, n);
        if (h < 0)
        {
            return Err(net_error(h));
        }
        Ok(reinterpret<usize>(h))
    }

    // ---- TCP ----

    // A socket that accepts connections.
    export resource struct TcpListener
    {
        usize id;
    }

    // A TCP connection: a stream of bytes each way.
    export resource struct TcpStream
    {
        usize id;
    }

    // Listens at `a`; port 0 picks a free port (`local_addr` tells which).
    export fn TcpListener::bind(SocketAddr a) : Result<TcpListener, NetError>
    {
        usize id = net_open(1, &a, 0)?;
        Ok(TcpListener { .id = id })
    }

    export fn TcpListener::local_addr(ref<TcpListener, shared> l) : SocketAddr
    {
        net_addr(2, l.id)
    }

    // `accept` gives `Err(TimedOut)` after waiting `ms` milliseconds with no
    // connection; 0 waits for ever. A server loop that checks
    // `interrupt_requested()` sets one.
    export fn TcpListener::set_accept_timeout_ms(ref<TcpListener, exclusive> l, u64 ms)
    {
        i64 r = net_num(4, l.id, StringView::of("").bytes, StringView::of("").bytes, narrow<usize>(ms));
    }

    // The next connection made to the listener.
    export fn TcpListener::accept(ref<TcpListener, exclusive> l) : Result<TcpStream, NetError>
    {
        i64 h = net_num(5, l.id, StringView::of("").bytes, StringView::of("").bytes, 0);
        if (h < 0)
        {
            return Err(net_error(h));
        }
        Ok(TcpStream { .id = reinterpret<usize>(h) })
    }

    export fn TcpListener::drop(ref<TcpListener, exclusive> self)
    {
        i64 r = net_num(14, self.id, StringView::of("").bytes, StringView::of("").bytes, 0);
    }

    // Connects to `a`, waiting as long as the system does.
    export fn TcpStream::connect(SocketAddr a) : Result<TcpStream, NetError>
    {
        usize id = net_open(6, &a, 0)?;
        Ok(TcpStream { .id = id })
    }

    // Connects to `a`, waiting at most `ms` milliseconds.
    export fn TcpStream::connect_timeout(SocketAddr a, u64 ms) : Result<TcpStream, NetError>
    {
        usize id = net_open(6, &a, narrow<usize>(max(ms, 1)))?;
        Ok(TcpStream { .id = id })
    }

    export fn TcpStream::peer_addr(ref<TcpStream, shared> s) : SocketAddr
    {
        net_addr(3, s.id)
    }

    export fn TcpStream::local_addr(ref<TcpStream, shared> s) : SocketAddr
    {
        net_addr(2, s.id)
    }

    // A read that waits `ms` milliseconds with nothing arriving gives
    // `Err(TimedOut)`; 0 waits for ever.
    export fn TcpStream::set_read_timeout_ms(ref<TcpStream, exclusive> s, u64 ms)
    {
        i64 r = net_num(7, s.id, StringView::of("").bytes, StringView::of("").bytes, narrow<usize>(ms));
    }

    // A write that cannot go on for `ms` milliseconds gives `Err(TimedOut)`;
    // 0 waits for ever.
    export fn TcpStream::set_write_timeout_ms(ref<TcpStream, exclusive> s, u64 ms)
    {
        i64 r = net_num(8, s.id, StringView::of("").bytes, StringView::of("").bytes, narrow<usize>(ms));
    }

    // Whether small writes go out at once (true) rather than being gathered
    // for a moment into fewer packets (false, the default).
    export fn TcpStream::set_nodelay(ref<TcpStream, exclusive> s, bool on)
    {
        usize flag = 0;
        if (on)
        {
            flag = 1;
        }
        i64 r = net_num(9, s.id, StringView::of("").bytes, StringView::of("").bytes, flag);
    }

    // Appends up to `want` bytes of what has arrived to `buf`.
    fn TcpStream::take(ref<TcpStream, exclusive> s, ref<Vec<u8>, exclusive> buf, usize want) : Result<usize, NetError>
    {
        while (buf.cap - buf.len < want)
        {
            Vec::grow(buf);
        }
        rawptr<u8> at = unsafe
        {
            reinterpret_ptr<u8>(buf.ptr + reinterpret<isize>(buf.len))
        };
        i64 r = byte_op(2, 10, s.id, dangling<u8>(), 0, dangling<u8>(), 0, at, want);
        if (r < 0)
        {
            return Err(net_error(r));
        }
        usize n = reinterpret<usize>(r);
        buf.len = buf.len + n;
        Ok(n)
    }

    // Appends up to `max` bytes to `buf` (at most a MiB), waiting until at
    // least one has arrived, and returns how many: 0 only when the peer has
    // closed its side (when `max` > 0).
    export fn TcpStream::read(ref<TcpStream, exclusive> s, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, NetError>
    {
        usize want = if (max > 1_048_576)
        {
            1_048_576
        }
        else
        {
            max
        };
        TcpStream::take(s, buf, want)
    }

    // Appends exactly `n` bytes to `buf`; `Err(Closed)` when the peer closes
    // first (what arrived stays in `buf`).
    export fn TcpStream::read_exact(
        ref<TcpStream, exclusive> s,
        ref<Vec<u8>, exclusive> buf,
        usize n,
    ) : Result<void, NetError>
    {
        usize got = 0;
        while (got < n)
        {
            usize k = TcpStream::read(s, buf, n - got)?;
            if (k == 0)
            {
                return Err(Closed);
            }
            got += k;
        }
        Ok(())
    }

    // The next line without its '\n' (nor a '\r' before it), as
    // `File::read_line`; None when the peer has closed its side.
    export fn TcpStream::read_line(ref<TcpStream, exclusive> s) : Result<Option<String>, NetError>
    {
        i64 r = net_num(11, s.id, StringView::of("").bytes, StringView::of("").bytes, 0);
        if (r < 0)
        {
            return Err(net_error(r));
        }
        if (r == 0)
        {
            return Ok(None);
        }
        Vec<u8> bytes = Vec::new();
        usize n = TcpStream::take(s, &mut bytes, reinterpret<usize>(r))?;
        if (n > 0 && bytes[n - 1] == 10)
        {
            Vec::pop(&mut bytes);
            if (n > 1 && bytes[n - 2] == 13)
            {
                Vec::pop(&mut bytes);
            }
        }
        match (String::from_utf8(bytes))
        {
            Ok(t)  : Ok(Some(t)),
            Err(e) : Err(NetError::NotUtf8(e)),
        }
    }

    // Sends all of `data`.
    export fn TcpStream::write(ref<TcpStream, exclusive> s, slice<u8, shared> data) : Result<void, NetError>
    {
        if (slice_len(data) == 0)
        {
            return Ok(());
        }
        i64 r = net_num(12, s.id, StringView::of("").bytes, data, 0);
        if (r < 0)
        {
            return Err(net_error(r));
        }
        Ok(())
    }

    export fn TcpStream::write_text(ref<TcpStream, exclusive> s, StringView text) : Result<void, NetError>
    {
        TcpStream::write(s, text.bytes)
    }

    // `TcpStream::printf(s, format, …)` is `TcpStream::write_formatted(s,
    // $fmt(format, …))` after `modres`, as `File::printf` is.
    fn TcpStream::write_formatted(ref<TcpStream, exclusive> s, str text) : Result<void, NetError>
    {
        TcpStream::write(s, StringView::of(text).bytes)
    }

    // Closes the sending side: the peer reads the end (0) after what was
    // sent; this side can still read.
    export fn TcpStream::shutdown_write(ref<TcpStream, exclusive> s) : Result<void, NetError>
    {
        i64 r = net_num(13, s.id, StringView::of("").bytes, StringView::of("").bytes, 0);
        if (r < 0)
        {
            return Err(net_error(r));
        }
        Ok(())
    }

    export fn TcpStream::drop(ref<TcpStream, exclusive> self)
    {
        i64 r = net_num(14, self.id, StringView::of("").bytes, StringView::of("").bytes, 0);
    }

    // ---- UDP ----

    // A socket that sends and receives datagrams: messages of up to about
    // 64 KiB, each delivered whole or not at all, in any order.
    export resource struct UdpSocket
    {
        usize id;
    }

    // A datagram received, and who sent it.
    export struct Datagram
    {
        export Vec<u8> data;
        export SocketAddr from;
    }

    export fn UdpSocket::bind(SocketAddr a) : Result<UdpSocket, NetError>
    {
        usize id = net_open(15, &a, 0)?;
        Ok(UdpSocket { .id = id })
    }

    // D-0205: a second handle on the same socket, as `TcpStream::try_clone`
    // gives one: a thread can wait for datagrams on one while others send on
    // another.
    export fn UdpSocket::try_clone(ref<UdpSocket, shared> u) : Result<UdpSocket, NetError>
    {
        i64 h = net_num(24, u.id, StringView::of("").bytes, StringView::of("").bytes, 0);
        if (h < 0)
        {
            return Err(net_error(h));
        }
        Ok(UdpSocket { .id = reinterpret<usize>(h) })
    }

    export fn UdpSocket::local_addr(ref<UdpSocket, shared> u) : SocketAddr
    {
        net_addr(2, u.id)
    }

    // `recv_from` gives `Err(TimedOut)` after `ms` milliseconds with nothing;
    // 0 waits for ever.
    export fn UdpSocket::set_read_timeout_ms(ref<UdpSocket, exclusive> u, u64 ms)
    {
        i64 r = net_num(7, u.id, StringView::of("").bytes, StringView::of("").bytes, narrow<usize>(ms));
    }

    // Sends `data` as one datagram to `to`; the number of bytes sent.
    export fn UdpSocket::send_to(
        ref<UdpSocket, exclusive> u,
        slice<u8, shared> data,
        SocketAddr to,
    ) : Result<usize, NetError>
    {
        Vec<u8> r = SocketAddr::record(&to);
        i64 n = net_num(16, u.id, &r[0..$], data, 0);
        if (n < 0)
        {
            return Err(net_error(n));
        }
        Ok(reinterpret<usize>(n))
    }

    // The next datagram, its first `max` bytes (the rest of a longer one is
    // lost), and its sender.
    export fn UdpSocket::recv_from(ref<UdpSocket, exclusive> u, usize max) : Result<Datagram, NetError>
    {
        Vec<u8> b = Vec::new();
        while (b.cap < max + 19)
        {
            Vec::grow(&mut b);
        }
        i64 n = byte_op(2, 17, u.id, dangling<u8>(), 0, dangling<u8>(), 0, reinterpret_ptr<u8>(b.ptr), max + 19);
        if (n < 0)
        {
            return Err(net_error(n));
        }
        b.len = reinterpret<usize>(n);
        Ok(Datagram { .data = Vec::from_slice(&b[19..$]), .from = SocketAddr::from_record(&b[0..19]) })
    }

    export fn UdpSocket::drop(ref<UdpSocket, exclusive> self)
    {
        i64 r = net_num(14, self.id, StringView::of("").bytes, StringView::of("").bytes, 0);
    }

    // ---- D-0181, D-0185: keepalive, a connected UDP socket, a second handle ----

    // D-0181: TCP keepalive on or off: with it on, the system probes an idle
    // connection now and then (after a couple of hours, by its default), so
    // a peer or a path that has silently gone is noticed and the next read
    // fails, rather than waiting for ever.
    export fn TcpStream::set_keepalive(ref<TcpStream, exclusive> s, bool on)
    {
        i64 r = net_num(20, s.id, StringView::of("").bytes, StringView::of("").bytes, if (on) { 1 } else { 0 });
    }

    // D-0185: a second `TcpStream` on the same connection, for reading in
    // one thread while another writes (each takes its own by `move`). Bytes
    // read through one are not seen by the other, and each has its own
    // timeouts; the connection closes when the last of them is dropped, and
    // `shutdown_write` through either ends sending for both.
    export fn TcpStream::try_clone(ref<TcpStream, shared> s) : Result<TcpStream, NetError>
    {
        i64 h = net_num(24, s.id, StringView::of("").bytes, StringView::of("").bytes, 0);
        if (h < 0)
        {
            return Err(net_error(h));
        }
        Ok(TcpStream { .id = reinterpret<usize>(h) })
    }

    // D-0181: fixes the peer `send` sends to and `recv` receives from (a
    // datagram from anyone else is dropped); `send_to` and `recv_from` still
    // work.
    export fn UdpSocket::connect(ref<UdpSocket, exclusive> u, SocketAddr to) : Result<void, NetError>
    {
        Vec<u8> r = SocketAddr::record(&to);
        i64 n = net_num(21, u.id, &r[0..$], StringView::of("").bytes, 0);
        if (n < 0)
        {
            return Err(net_error(n));
        }
        Ok(())
    }

    // Sends `data` as one datagram to the connected peer; how many bytes.
    export fn UdpSocket::send(ref<UdpSocket, exclusive> u, slice<u8, shared> data) : Result<usize, NetError>
    {
        i64 n = net_num(22, u.id, StringView::of("").bytes, data, 0);
        if (n < 0)
        {
            return Err(net_error(n));
        }
        Ok(reinterpret<usize>(n))
    }

    // The next datagram from the connected peer, its first `max` bytes.
    export fn UdpSocket::recv(ref<UdpSocket, exclusive> u, usize max) : Result<Vec<u8>, NetError>
    {
        Vec<u8> b = Vec::new();
        while (b.cap < max)
        {
            Vec::grow(&mut b);
        }
        i64 n = byte_op(2, 23, u.id, dangling<u8>(), 0, dangling<u8>(), 0, reinterpret_ptr<u8>(b.ptr), max);
        if (n < 0)
        {
            return Err(net_error(n));
        }
        b.len = reinterpret<usize>(n);
        Ok(b)
    }

**Depends on:** rule.stdlib.process, rule.stdlib.text, rule.stdlib.format, rule.conc.spawn, rule.fn.program, D-0144
**Affects:** the program's environment

## 3. `Rc<T>`

In `std::memory`.

### `rule.stdlib.rc`
**Status:** ACCEPTED

    // D-0125: `count` strong handles keep `value`; `weak` handles keep the
    // box. The value ends when `count` reaches 0, the box when both do.
    struct RcBox<T>
    {
        usize count;
        usize weak;
        Option<T> value;
    }

    export resource struct Rc<T>
    {
        rawptr<RcBox<T>> ptr;
    }

    export fn Rc::new<T>(T v) : Rc<T>
    {
        rawptr<u8> p = match (allocate(sizeof<RcBox<T>>(), alignof<RcBox<T>>()))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure),
        };
        rawptr<RcBox<T>> bp = reinterpret_ptr<RcBox<T>>(p);
        unsafe
        {
            *bp = RcBox { .count = 1, .weak = 0, .value = Some(v) };
        }
        Rc { .ptr = bp }
    }

    export fn Rc::clone<T>(ref<Rc<T>, shared> r) : Rc<T>
    {
        unsafe
        {
            // Only the count: a reference into the value (`Rc::get` of
            // another handle) may be live, and is disjoint from it.
            ref<usize, exclusive> c = &mut reclaim<RcBox<T>>(r.ptr).count;
            *c = *c + 1;
        }
        Rc { .ptr = r.ptr }
    }

    export fn Rc::get<T>(ref<Rc<T>, shared> r) : ref<T, shared>
    {
        unsafe
        {
            match (&reclaim<RcBox<T>>(r.ptr).value)
            {
                Some(v) : v,
                None : fault(unwrap_failed, "an Rc whose value has ended"),   // never: a live Rc keeps its value
            }
        }
    }

    export fn Rc::drop<T>(ref<Rc<T>, exclusive> self)
    {
        bool last = false;
        unsafe
        {
            ref<usize, exclusive> c = &mut reclaim<RcBox<T>>(self.ptr).count;
            *c = *c - 1;
            last = *c == 0;
        }
        if (last)
        {
            bool unwatched = false;
            unsafe
            {
                overwrite(&mut reclaim<RcBox<T>>(self.ptr).value, None);   // the value ends now (D-0125)
                unwatched = reclaim<RcBox<T>>(self.ptr).weak == 0;
            }
            if (unwatched)
            {
                unsafe
                {
                    drop(reclaim<RcBox<T>>(self.ptr));
                    deallocate(reinterpret_ptr<u8>(self.ptr), sizeof<RcBox<T>>(), alignof<RcBox<T>>());
                }
            }
        }
    }
    // D-0125: a handle that does not keep the value alive.
    export resource struct Weak<T>
    {
        rawptr<RcBox<T>> ptr;
    }
    export fn Rc::downgrade<T>(ref<Rc<T>, shared> r) : Weak<T>
    {
        unsafe
        {
            ref<usize, exclusive> w = &mut reclaim<RcBox<T>>(r.ptr).weak;
            *w = *w + 1;
        }
        Weak { .ptr = r.ptr }
    }
    export fn Weak::upgrade<T>(ref<Weak<T>, shared> w) : Option<Rc<T>>
    {
        bool alive = false;
        unsafe
        {
            ref<usize, exclusive> c = &mut reclaim<RcBox<T>>(w.ptr).count;
            if (*c > 0)
            {
                *c = *c + 1;
                alive = true;
            }
        }
        if (alive) { Some(Rc { .ptr = w.ptr }) } else { None }
    }
    export fn Weak::clone<T>(ref<Weak<T>, shared> w) : Weak<T>
    {
        unsafe
        {
            ref<usize, exclusive> k = &mut reclaim<RcBox<T>>(w.ptr).weak;
            *k = *k + 1;
        }
        Weak { .ptr = w.ptr }
    }
    export fn Weak::drop<T>(ref<Weak<T>, exclusive> self)
    {
        bool last = false;
        unsafe
        {
            ref<usize, exclusive> k = &mut reclaim<RcBox<T>>(self.ptr).weak;
            *k = *k - 1;
            last = *k == 0 && reclaim<RcBox<T>>(self.ptr).count == 0;
        }
        if (last)
        {
            unsafe
            {
                drop(reclaim<RcBox<T>>(self.ptr));
                deallocate(reinterpret_ptr<u8>(self.ptr), sizeof<RcBox<T>>(), alignof<RcBox<T>>());
            }
        }
    }
    export resource struct Rc<T>
    {
        rawptr<RcBox<T>> ptr;
    }

`Rc<T>` realizes shared destroy authority as a library pattern
(D-0003's allowance). A `Weak<T>` (D-0125) holds the box, not the
value: `count` strong handles keep the value, which ends (`overwrite`
to `None`) when the last of them does; `weak` handles keep the box,
which is deallocated when both counts are 0 — by the last `Rc::drop` if
no weak handle exists, else by the last `Weak::drop`. `Weak::upgrade`
makes a strong handle only while `count > 0`, so a value never comes
back once ended; `Rc::get` matches the `Option` and can never see
`None` through a live `Rc`. A cycle of `Rc`s still leaks; a parent
pointer, a cache or an observer list holds a `Weak` instead. `Rc::clone` and `Rc::drop` borrow only the box's
`count` (CHG-0100): a reference into the value that `Rc::get` gave
through another handle may be live, and it is disjoint from the count,
where a borrow of the whole box would clash with it. `Rc::drop` ends
that borrow (the block holding `c`) before destroying the box:
`[Destroy]` requires `solitary`, and a still-live `c` — or the
projection paths its own `if (*c == 0)` test would have formed inside
the same statement scope — would be a second path on the reclaimed box
object (`diag.destroy-while-aliased`, `CHG-0009`). More generally: `N` independent checked handles, one raw
allocation, a count arbitrating the final destroy. The `unsafe`
blocks assert what the count protects. `Rc::get` returns a shared
reference into the reclaimed box, so mutation through an `Rc` is
impossible without a `mutex` inside it. `Rc` is not thread-safe
(the count is not synchronized); sharing across threads is
`ref<mutex<T>, shared>` (`spec/19` §2).

**Depends on:** rule.trust.rawptr, rule.stdlib.prelude,
rule.resauth.destroy, D-0003

## 3a. `Box<T>`

In `std::memory`.

### `rule.stdlib.box`
**Status:** ACCEPTED

A value on the heap with one owner (D-0045): `Rc` (§3) without the
count. `Box<T>` holds a `rawptr<T>`, so a type may contain itself through
one (`[Type-Recursive]`, `spec/16`): `struct Node { i32 v;
Option<Box<Node>> next; }`. `get` and `get_mut` borrow the value;
`into_inner` moves it out and frees the memory; destroying the `Box`
destroys the value and frees the memory.

    export resource struct Box<T>
    {
        rawptr<T> ptr;
        bool full;                          // false once into_inner has taken the value
    }

    // The size Box allocates: at least one byte, so every Box has an address.
    fn Box::cells<T>() : usize
    {
        if (sizeof<T>() == 0)
        {
            1
        }
        else
        {
            sizeof<T>()
        }
    }

    export fn Box::new<T>(T v) : Box<T>
    {
        rawptr<u8> p = match (allocate(Box::cells<T>(), alignof<T>()))
        {
            Ok(p) : p,
            Err(_) : fault(alloc_failure),
        };
        rawptr<T> bp = reinterpret_ptr<T>(p);
        unsafe
        {
            *bp = v;
        }
        Box { .ptr = bp, .full = true }
    }

    export fn Box::get<T>(ref<Box<T>, shared> b) : ref<T, shared>
    {
        unsafe
        {
            &reclaim<T>(b.ptr)
        }
    }

    export fn Box::get_mut<T>(ref<Box<T>, exclusive> b) : ref<T, exclusive>
    {
        unsafe
        {
            &mut reclaim<T>(b.ptr)
        }
    }

    // The value, moved out; the Box's memory is freed.
    export fn Box::into_inner<T>(Box<T> b) : T
    {
        b.full = false;
        unsafe
        {
            auto v = *b.ptr;
            release(reinterpret_ptr<u8>(b.ptr), sizeof<T>());
            v
        }
    }

    export fn Box::drop<T>(ref<Box<T>, exclusive> self)
    {
        unsafe
        {
            if (self.full)
            {
                drop(reclaim<T>(self.ptr));
            }
            deallocate(reinterpret_ptr<u8>(self.ptr), Box::cells<T>(), alignof<T>());
        }
    }

- **One owner.** A `Box` is a resource: it moves, and is destroyed once.
  Changing the value through `get_mut` needs an exclusive borrow of the
  `Box`, as for any value.
- **Size.** A `Box` allocates `sizeof<T>()` bytes, or one for a
  zero-sized `T`, so that every `Box` has its own address.

**Depends on:** rule.stdlib.rc, rule.trust.unsafe, rule.agg.layout, D-0045
**Affects:** state.storage

## 3b. Exchanging values behind references

In `std::core`.

### `rule.stdlib.swap`
**Status:** ACCEPTED

A value cannot be moved out of a place reached through a reference
(`diag.move-out-of-field`), so a program cannot exchange two values it
holds only by reference — two fields of a borrowed struct, two elements
of a `Vec` or a slice of resources — itself (D-0048). `std` does it
with one std-only intrinsic:

    [Swap-Places]
        ⟨a, Σ⟩ →* ⟨ref to place p, Σ1⟩, ⟨b, Σ1⟩ →* ⟨ref to place q, Σ2⟩, both exclusive
        ────────────────────────────────────────────
        ⟨swap_places(a, b), Σ⟩ → ⟨(), Σ2[p := value(q), q := value(p)]⟩     (p = q: Σ2 unchanged)

A resource keeps its single owner: the place it now occupies, which is
destroyed as any place is. Nothing is copied or destroyed.

    export fn swap<T>(ref<T, exclusive> a, ref<T, exclusive> b)
    {
        swap_places(a, b);
    }

    // Puts v in *r and returns what was there.
    export fn replace<T>(ref<T, exclusive> r, T v) : T
    {
        auto old = v;
        swap_places(r, &mut old);
        old
    }

    // D-0080: puts v in *r and destroys what was there, where the call is
    // (`[Write-Resource-Overwrite-Rejected]`'s repair).
    export fn overwrite<T>(ref<T, exclusive> r, T v)
    {
        drop(replace(r, v));
    }

    // Exchanges elements i and j of v; nothing when i == j.
    export fn Vec::swap<T>(ref<Vec<T>, exclusive> v, usize i, usize j)
    {
        if (i >= v.len || j >= v.len)
        {
            fault(index_out_of_bounds);
        }
        if (i != j)
        {
            swap_places(&mut (*v)[i], &mut (*v)[j]);
        }
    }

    // Exchanges elements i and j of s; nothing when i == j.
    export fn slice_swap<T>(slice<T, exclusive> s, usize i, usize j)
    {
        if (i >= slice_len(s) || j >= slice_len(s))
        {
            fault(index_out_of_bounds);
        }
        if (i != j)
        {
            swap_places(&mut s[i], &mut s[j]);
        }
    }

- `Vec::swap` and `slice_swap` exist because `swap(&mut v[i], &mut v[j])`
  with `i = j` is two exclusive borrows of one place: they check the
  bounds and do nothing when `i = j`.
- `replace` is `swap` with a fresh value, returning the old one.

**Depends on:** rule.stdlib.vec, rule.agg.slice, rule.alias.borrow, D-0048
**Affects:** the places `swap`, `replace`, `Vec::swap` and `slice_swap` are given

## 3c. Channels

In `std::sync`.

### `rule.stdlib.channel`
**Status:** ACCEPTED

`Channel<T>` (D-0063) realizes `rule.conc.channel` (`spec/19` §3) in
CobaltC: a `mutex` around a ring of `cap` slots, and two counts that
waiting threads watch — one a `send` waits on, which `recv` and `close`
change, and one a `recv` waits on, which `send` and `close` change —
kept by one primitive private to `std`:

    [Event-Op]
        ⟨event_op(0, _, _), Σ⟩ → ⟨e, Σ[events(e) := 0]⟩, e fresh
        ⟨event_op(1, e, _), Σ⟩ → ⟨events(e), Σ⟩
        ⟨event_op(2, e, s), Σ⟩ → ⟨events(e), Σ⟩ when events(e) ≠ s; otherwise no step (it waits),
                                 or ↛ diag.channel-deadlock as [Channel-Deadlock] says
        ⟨event_op(3, e, _), Σ⟩ → ⟨events(e) + 1, Σ[events(e) := events(e) + 1]⟩ (wrapping)
        ⟨event_op(4, e, _), Σ⟩ → ⟨0, Σ[events(e) removed]⟩

A change to a channel's state adds one to the count of the threads it
may let proceed while the channel's lock is held, and a waiter reads
its count while holding it, then waits for it to differ once the lock is
released: no change made in between is missed, and a `send` wakes no
waiting sender.

    // Channels (`rule.conc.channel`, spec/19 §3, D-0063) wait on event
    // counts, private to `std`: `event_op(0, …)` makes one and returns its
    // id; 1 reads its count; 2 waits until its count differs from `seen` (a
    // fault when no other thread could change it); 3 adds one to it, waking
    // whoever waits on it; 4 frees it. A channel's counts change only under
    // its lock, so a waiter that read one under the lock misses no change.
    unsafe extern fn event_op(usize op, usize id, usize seen) : usize;

    // A bounded queue between threads (`rule.conc.channel`, spec/19 §3,
    // D-0063), shared by reference as a `mutex` is: `send` waits while it is
    // full, `recv` while it is empty; after `close`, `send` gives its value
    // back and `recv` drains what is left, then gives `None`.
    struct ChannelState<T>
    {
        Vec<Option<T>> slots;               // a ring of `cap` slots
        usize head;                         // the oldest value's slot
        usize count;
        bool closed;
    }

    // The oldest value, taken from its slot; `st.count` is not 0.
    fn ChannelState::pop<T>(ref<ChannelState<T>, exclusive> st) : Option<T>
    {
        usize at = st.head;
        Option<T> none = None;
        Option<T> x = replace(&mut st.slots[at], none);
        st.head = (at + 1) % Vec::len(&st.slots);
        st.count = st.count - 1;
        x
    }

    export resource struct Channel<T>
    {
        mutex<ChannelState<T>> state;
        usize cap;
        usize can_send;                     // counts a waiting `send` watches:
        usize can_recv;                     // `recv` adds to it; and `send` to this
    }

    // A channel holding at most `cap` values; `cap` is at least 1.
    export fn Channel::new<T>(usize cap) : Channel<T>
    {
        if (cap == 0)
        {
            fault(channel_zero_capacity);
        }
        Vec<Option<T>> slots = Vec::new();
        usize i = 0;
        while (i < cap)
        {
            Option<T> none = None;
            Vec::push(&mut slots, none);
            i = i + 1;
        }
        usize can_send = unsafe
        {
            event_op(0, 0, 0)
        };
        usize can_recv = unsafe
        {
            event_op(0, 0, 0)
        };
        ChannelState<T> st = ChannelState { .slots = slots, .head = 0, .count = 0, .closed = false };
        Channel { .state = Mutex::new(st), .cap = cap, .can_send = can_send, .can_recv = can_recv }
    }

    // Puts v at the back, waiting while the channel is full; `Err(v)` once it
    // is closed.
    export fn Channel::send<T>(ref<Channel<T>, shared> ch, T v) : Result<void, T>
    {
        while (true)
        {
            usize seen = 0;
            {
                auto g = lock(&ch.state);
                if (g.closed)
                {
                    return Err(v);
                }
                if (g.count < ch.cap)
                {
                    usize at = (g.head + g.count) % ch.cap;
                    g.slots[at] = Some(v);
                    g.count = g.count + 1;
                    unsafe
                    {
                        event_op(3, ch.can_recv, 0);
                    }
                    return Ok(());
                }
                seen = unsafe
                {
                    event_op(1, ch.can_send, 0)
                };
            }
            unsafe
            {
                event_op(2, ch.can_send, seen);
            }
        }
    }

    // Takes the value at the front, waiting while the channel is empty;
    // `None` once it is closed and empty.
    export fn Channel::recv<T>(ref<Channel<T>, shared> ch) : Option<T>
    {
        while (true)
        {
            usize seen = 0;
            {
                auto g = lock(&ch.state);
                if (g.count > 0)
                {
                    Option<T> x = ChannelState::pop(&mut *g);
                    unsafe
                    {
                        event_op(3, ch.can_send, 0);
                    }
                    return x;
                }
                if (g.closed)
                {
                    return None;
                }
                seen = unsafe
                {
                    event_op(1, ch.can_recv, 0)
                };
            }
            unsafe
            {
                event_op(2, ch.can_recv, seen);
            }
        }
    }

    // No more values: `send` fails from now on, and `recv` gives `None` once
    // the channel is empty. Closing a closed channel does nothing.
    export fn Channel::close<T>(ref<Channel<T>, shared> ch)
    {
        auto g = lock(&ch.state);
        g.closed = true;
        unsafe
        {
            event_op(3, ch.can_send, 0);
            event_op(3, ch.can_recv, 0);
        }
    }

    // The values still in the channel are destroyed oldest first.
    export fn Channel::drop<T>(ref<Channel<T>, exclusive> self)
    {
        {
            auto g = lock(&self.state);
            while (g.count > 0)
            {
                Option<T> x = ChannelState::pop(&mut *g);
                drop(x);
            }
        }
        unsafe
        {
            event_op(4, self.can_send, 0);
            event_op(4, self.can_recv, 0);
        }
    }

- `send` and `recv` wait in a loop: each wake-up is only a reason to
  look again, under the lock.
- `ChannelState::pop` serves `recv` and the destructor, which destroys
  the values left oldest first.


    [Try-Recv]      (D-0185)   ⟨Channel::try_recv(ch), Σ⟩ → Ok(Some(v)), v the oldest value, taken as recv takes it (a
                    waiting send woken), when the channel holds one; Ok(None) when it is empty and open; Err(())
                    when it is closed and empty. Never waits
    [Recv-Timeout]  (D-0185)   ⟨Channel::recv_timeout_ms(ch, ms), Σ⟩ → [Try-Recv]'s answer once it is Ok(Some(_)) or
                    Err(()), or Ok(None) once ms milliseconds (by monotonic_ns) have passed with the channel empty:
                    a poll, sleeping a millisecond between tries for the first twenty and five thereafter, so the
                    answer comes at most a few milliseconds late

    // D-0185: the value at the front if one is waiting, without waiting:
    // `Ok(Some(v))`; `Ok(None)` when the channel is empty; `Err(())` once it
    // is closed and empty, so a loop that also polls something else (a
    // socket, `interrupt_requested`) knows when the senders are done.
    export fn Channel::try_recv<T>(ref<Channel<T>, shared> ch) : Result<Option<T>, void>
    {
        auto g = lock(&ch.state);
        if (g.count > 0)
        {
            Option<T> x = ChannelState::pop(&mut *g);
            unsafe
            {
                event_op(3, ch.can_send, 0);
            }
            return Ok(x);
        }
        if (g.closed)
        {
            return Err(());
        }
        Ok(None)
    }

    // D-0185: as `recv`, but waiting at most `ms` milliseconds: `Ok(Some(v))`
    // a value, `Ok(None)` none arrived in time, `Err(())` the channel is
    // closed and empty. The wait is a poll (every millisecond at first, then
    // every five), so it ends at most a few milliseconds late.
    export fn Channel::recv_timeout_ms<T>(ref<Channel<T>, shared> ch, u64 ms) : Result<Option<T>, void>
    {
        i64 deadline = unsafe
        {
            clock_read(0)
        } + reinterpret<i64>(wrapping_mul(ms, 1000000));
        u64 round = 0;
        while (true)
        {
            match (Channel::try_recv(ch))
            {
                Ok(Some(v)) : return Ok(Some(v)),
                Ok(None)    : {},
                Err(_)      : return Err(()),
            }
            i64 now = unsafe
            {
                clock_read(0)
            };
            if (now >= deadline)
            {
                return Ok(None);
            }
            i64 ignored = unsafe
            {
                sleep_ns(if (round < 20) { 1000000 } else { 5000000 })
            };
            round += 1;
        }
    }

    // D-0185: how many threads the machine runs at once (its processors, or
    // what this program may use of them): the size of a worker pool. At
    // least 1.
    export fn cpu_count() : usize
    {
        i64 n = byte_op(1, 19, 0, dangling<u8>(), 0, dangling<u8>(), 0, dangling<u8>(), 0);
        if (n < 1)
        {
            return 1;
        }
        reinterpret<usize>(n)
    }

**Depends on:** rule.conc.channel, rule.conc.lock, rule.stdlib.vec,
rule.stdlib.swap, D-0063
**Affects:** the channel's state

## 3c′. Supervision and data parallelism

In `std::sync` (`try_join`, `is_finished`, `ThreadFailure`) and
`std::collections` (`slice_parts`); D-0204.

### `rule.stdlib.supervision`
**Status:** ACCEPTED

    [Try-Join]
        ⟨join_status(&h, 0), Σ⟩ →* ⟨k, Σ1⟩                      -- waits as [Join]
        ────────────────────────────────────────────
        ⟨try_join(h), Σ⟩ →* Ok(join(h))               if k = 1
                             Err(ThreadFailure{id: k − 2})   otherwise; the failure is taken: h's
                                                               destructor raises nothing ([Join-Failed])

    [Is-Finished]
        ⟨is_finished(&h), Σ⟩ → (join_status(&h, 1) ≠ 0)          -- never waits; takes nothing

    [Failure-Text]
        ThreadFailure::text(&f)        → the report the program would have printed for the failure,
                                          as a fault's report on standard error (spec/18), its first line
                                          naming the diagnostic, its second the location
        ThreadFailure::diagnostic(&f)  → the diagnostic's name alone, as `diag.…`
        ThreadFailure::is_cancelled(&f) → diagnostic(&f) = "diag.thread-cancelled"

    [Slice-Parts]
        n = slice_len(s),  p = parts
        ────────────────────────────────────────────
        ⟨slice_parts(s, p), Σ⟩ → the vector of p exclusive slices s[a_k .. a_k + n_k], k < p, where
            n_k = ⌊n / p⌋ + (1 if k < n mod p else 0) and a_k = Σ_{j<k} n_j; empty when p = 0

`try_join` is how a supervisor keeps a server running: each connection's
handler is a thread, and a handler's bug becomes a value to log rather
than the server's end. `is_finished` lets one loop accept new work and
take the threads that have ended. The parts of `slice_parts` are disjoint
by construction, so the checker's ranged paths (`rule.alias.borrow`)
accept handing each to a thread of its own; each thread then writes its
run with no lock, and the parts' borrow of `s` ends when the last of them
ends. A `ThreadFailure`'s text is kept by the runtime until the program
ends.

**Depends on:** rule.conc.join, rule.conc.cancel, rule.fail.fault-unwind, rule.agg.slice, D-0204
**Affects:** a thread's failure, once taken

## 3d. Floating-point functions

In `std::math` (D-0136). Before D-0136 these were language intrinsics,
in scope without an import; they are ordinary functions of a library in
everything but how they are typed. Each takes an `f32` or an `f64`
(`atan2` and `powf` two of one type) and gives that type, which no
bound can say (`rule.type.bound` has `number`, which admits integers),
so `[Float-Fn]` types them here and both implementations realize them
natively, as `printf` is. Without `import std;` (or an import naming
one of them) their names are unbound (`[Resolve-Unbound]`), and a
program's own `round` is an ordinary item, found as any other.

### `rule.arith.float-fns`
**Status:** ACCEPTED

    [Float-Fn]
        f ∈ {sqrt, floor, ceil, round, trunc}, resolved to std::math's (rule.module.resolve);  Γ ⊢ e : τ,  τ ∈ {f32, f64}
        ────────────────────────────────────────────
        Γ ⊢ f(e) : τ;   ⟨f(v), Σ⟩ → ⟨r, Σ⟩ with r IEEE-754's result at τ:
            sqrt  — the correctly rounded square root (a NaN for v < 0, -0.0 for -0.0)
            floor, ceil, trunc — v rounded to an integral value toward -∞, +∞, 0
            round — v rounded to the nearest integral value, halfway cases away from zero
        (each exact or correctly rounded, so every conforming implementation gives
        the same bits; a NaN in gives a NaN out)

    [Float-Fn-Operand]   disposition: rejected
        f(e) with e not of type f32 or f64 (a type parameter's value included)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch

The transcendental functions (D-0101) are the platform's: their last bit
differs between platforms' libraries, so they are specified to within
one unit in the last place, and every implementation on one platform
uses that platform's C library, so `coby` and `cobc` give the same bits
there.

    [Float-Transcendental]
        f ∈ {ln, exp, log2, log10, sin, cos, tan, asin, acos, atan};  Γ ⊢ e : τ,  τ ∈ {f32, f64}   -- asin, acos, atan: D-0181
        ────────────────────────────────────────────
        Γ ⊢ f(e) : τ;   ⟨f(v), Σ⟩ → ⟨r, Σ⟩, r within 1 ulp of the mathematical value at τ
            (ln: natural logarithm; a NaN for v < 0, -∞ for ±0; the angles in radians; asin and acos a NaN
            outside [-1, 1])

    [Float-Transcendental-2]
        f ∈ {atan2, powf};  Γ ⊢ e1 : τ,  Γ ⊢ e2 : τ,  τ ∈ {f32, f64}
        ────────────────────────────────────────────
        Γ ⊢ f(e1, e2) : τ;   atan2(y, x): the angle of (x, y) in (-π, π];  powf(x, y): x to the power y
            (C's atan2 and pow: within 1 ulp; `pow` is the integer-exponent power of spec/21)

    [Float-Transcendental-Operand]   disposition: rejected
        an operand not of type f32 or f64, or two operands of different types
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch

**Depends on:** rule.arith.represent, D-0091

## 3e. Big integers

In `std::math` (D-0157).

### `rule.stdlib.bigint`
**Status:** ACCEPTED

A `BigUint` is an unsigned integer of any size: 32-bit limbs, least
significant first, with no zero limb at the top. Its operations are the
integers' own: `add`, `sub`, `mul`, `div_rem` (giving a `BigDivision`,
`quotient` and `remainder`), `rem`, `mod_pow`, `mod_inverse`,
`shift_left`, `shift_right`, `eq` and `less`, `bit_len` and `bit`,
`clone`, conversions to and from big-endian bytes and decimal text.

    [BigUint]              each operation gives the exact integer result: a + b, a - b, a·b, ⌊a / b⌋ and
                           a mod b, base^exp mod m, ⌊a / 2^k⌋, a·2^k; to_bytes_be gives the minimal big-endian
                           bytes (none for zero), to_bytes_be_padded(a, w) exactly w of them; text and parse
                           decimal digits
    [Big-Division]         disposition: checked   b = 0 ↛ diag.div-by-zero (div_rem, rem, mod_pow);
                           a < b ↛ diag.arith-overflow (sub); a value wider than w bytes ↛
                           diag.arith-overflow (to_bytes_be_padded)
    [Big-Mod-Inverse]      (D-0169)   ⟨BigUint::mod_inverse(a, m), Σ⟩ → Some(x) with x < m and a·x ≡ 1 mod m
                           when m ≥ 2 and a and m have no common factor above 1, None otherwise
    [Big-Not-Constant-Time]   the time an operation takes depends on its operands' values: a BigUint is for
                           public values only, never a secret (the RSA private operation of §2m gives it
                           only public numbers)

A `BigUint` owns its limbs, so it is a resource: a program reassigns one
with `overwrite`, and takes a `BigDivision` apart by destructuring.

The `std` source:

    // D-0157: `BigUint`, an unsigned integer of any size, for public-key
    // verification (RSA, elliptic curves) and for numbers too large for `u128`.
    // Its operations take time that depends on the values, so a `BigUint` is
    // for public values only: never for a private key or anything secret.

    // The value Σ limbs[i]·2^(32·i): 32-bit limbs, least significant first,
    // with no zero limb at the top (zero has none).
    export struct BigUint
    {
        Vec<u32> limbs;
    }

    fn BigUint::trim(ref<BigUint, exclusive> a)
    {
        while (Vec::len(&a.limbs) > 0 && a.limbs[Vec::len(&a.limbs) - 1] == 0)
        {
            Vec::pop(&mut a.limbs);
        }
    }

    export fn BigUint::zero() : BigUint
    {
        BigUint { .limbs = Vec::new() }
    }

    export fn BigUint::from_u64(u64 x) : BigUint
    {
        BigUint r = BigUint { .limbs = Vec::new() };
        Vec::push(&mut r.limbs, narrow_wrapping<u32>(x));
        Vec::push(&mut r.limbs, narrow_wrapping<u32>(x >> 32));
        BigUint::trim(&mut r);
        r
    }

    // The number whose big-endian bytes these are (leading zero bytes allowed).
    export fn BigUint::from_bytes_be(slice<u8, shared> bytes) : BigUint
    {
        BigUint r = BigUint { .limbs = Vec::new() };
        usize n = slice_len(bytes);
        usize i = n;
        while (i > 0)
        {
            u32 limb = 0;
            usize lo = if (i >= 4) { i - 4 } else { 0 };
            for (usize k = lo; k < i; k += 1)
            {
                limb = (limb << 8) | widen<u32>(bytes[k]);
            }
            Vec::push(&mut r.limbs, limb);
            i = lo;
        }
        BigUint::trim(&mut r);
        r
    }

    // The big-endian bytes of `a`, with no leading zero byte (none for zero).
    export fn BigUint::to_bytes_be(ref<BigUint, shared> a) : Vec<u8>
    {
        BigUint::to_bytes_be_padded(a, (BigUint::bit_len(a) + 7) / 8)
    }

    // The big-endian bytes of `a` in exactly `width` bytes, zeros in front; a
    // value that does not fit is `diag.arith-overflow`.
    export fn BigUint::to_bytes_be_padded(ref<BigUint, shared> a, usize width) : Vec<u8>
    {
        if (BigUint::bit_len(a) > width * 8)
        {
            fault(arith_overflow);
        }
        Vec<u8> out = Vec::filled(width, 0: u8);
        for (usize i = 0; i < width; i += 1)
        {
            usize limb = i / 4;
            if (limb < Vec::len(&a.limbs))
            {
                out[width - 1 - i] = narrow_wrapping<u8>(a.limbs[limb] >> narrow<u32>(8 * (i % 4)));
            }
        }
        out
    }

    export fn BigUint::is_zero(ref<BigUint, shared> a) : bool
    {
        Vec::len(&a.limbs) == 0
    }

    // The number of bits needed to write `a` (0 for zero).
    export fn BigUint::bit_len(ref<BigUint, shared> a) : usize
    {
        usize n = Vec::len(&a.limbs);
        if (n == 0)
        {
            return 0;
        }
        32 * n - widen<usize>(leading_zeros(a.limbs[n - 1]))
    }

    // Bit `i` of `a` (bit 0 the least significant).
    export fn BigUint::bit(ref<BigUint, shared> a, usize i) : bool
    {
        usize limb = i / 32;
        limb < Vec::len(&a.limbs) && ((a.limbs[limb] >> narrow<u32>(i % 32)) & 1) == 1
    }

    // -1, 0 or 1 as `a` is less than, equal to or greater than `b`.
    fn BigUint::compare(ref<BigUint, shared> a, ref<BigUint, shared> b) : i32
    {
        usize n = Vec::len(&a.limbs);
        if (n != Vec::len(&b.limbs))
        {
            return if (n < Vec::len(&b.limbs)) { -1 } else { 1 };
        }
        usize i = n;
        while (i > 0)
        {
            i -= 1;
            if (a.limbs[i] != b.limbs[i])
            {
                return if (a.limbs[i] < b.limbs[i]) { -1 } else { 1 };
            }
        }
        0
    }

    export fn BigUint::eq(ref<BigUint, shared> a, ref<BigUint, shared> b) : bool
    {
        BigUint::compare(a, b) == 0
    }

    // Whether `a` is less than `b`; fits `Vec::sort_by`.
    export fn BigUint::less(ref<BigUint, shared> a, ref<BigUint, shared> b) : bool
    {
        BigUint::compare(a, b) < 0
    }

    export fn BigUint::add(ref<BigUint, shared> a, ref<BigUint, shared> b) : BigUint
    {
        usize n = max(Vec::len(&a.limbs), Vec::len(&b.limbs));
        BigUint r = BigUint { .limbs = Vec::new() };
        u64 carry = 0;
        for (usize i = 0; i < n; i += 1)
        {
            u64 x = if (i < Vec::len(&a.limbs)) { widen<u64>(a.limbs[i]) } else { 0 };
            u64 y = if (i < Vec::len(&b.limbs)) { widen<u64>(b.limbs[i]) } else { 0 };
            u64 t = x + y + carry;
            Vec::push(&mut r.limbs, narrow_wrapping<u32>(t));
            carry = t >> 32;
        }
        if (carry > 0)
        {
            Vec::push(&mut r.limbs, narrow_wrapping<u32>(carry));
        }
        r
    }

    // a - b; `b` greater than `a` is `diag.arith-overflow`, as for any
    // unsigned subtraction.
    export fn BigUint::sub(ref<BigUint, shared> a, ref<BigUint, shared> b) : BigUint
    {
        if (BigUint::compare(a, b) < 0)
        {
            fault(arith_overflow);
        }
        BigUint r = BigUint { .limbs = Vec::new() };
        i64 borrow = 0;
        for (usize i = 0; i < Vec::len(&a.limbs); i += 1)
        {
            i64 y = if (i < Vec::len(&b.limbs)) { widen<i64>(b.limbs[i]) } else { 0 };
            i64 t = widen<i64>(a.limbs[i]) - y - borrow;
            if (t < 0)
            {
                t += 0x100000000;
                borrow = 1;
            }
            else
            {
                borrow = 0;
            }
            Vec::push(&mut r.limbs, narrow<u32>(t));
        }
        BigUint::trim(&mut r);
        r
    }

    export fn BigUint::mul(ref<BigUint, shared> a, ref<BigUint, shared> b) : BigUint
    {
        usize n = Vec::len(&a.limbs);
        usize m = Vec::len(&b.limbs);
        BigUint r = BigUint { .limbs = Vec::filled(n + m, 0: u32) };
        for (usize i = 0; i < n; i += 1)
        {
            u64 carry = 0;
            u64 x = widen<u64>(a.limbs[i]);
            for (usize j = 0; j < m; j += 1)
            {
                u64 t = x * widen<u64>(b.limbs[j]) + widen<u64>(r.limbs[i + j]) + carry;
                r.limbs[i + j] = narrow_wrapping<u32>(t);
                carry = t >> 32;
            }
            r.limbs[i + m] = narrow_wrapping<u32>(carry);
        }
        BigUint::trim(&mut r);
        r
    }

    // `a` divided by 2^k, rounded down.
    export fn BigUint::shift_right(ref<BigUint, shared> a, usize k) : BigUint
    {
        usize whole = k / 32;
        u32 part = narrow<u32>(k % 32);
        BigUint r = BigUint { .limbs = Vec::new() };
        for (usize i = whole; i < Vec::len(&a.limbs); i += 1)
        {
            u32 lo = a.limbs[i] >> part;
            u32 hi = if (part == 0 || i + 1 >= Vec::len(&a.limbs)) { 0 } else { a.limbs[i + 1] << (32 - part) };
            Vec::push(&mut r.limbs, lo | hi);
        }
        BigUint::trim(&mut r);
        r
    }

    // `a` times 2^k.
    export fn BigUint::shift_left(ref<BigUint, shared> a, usize k) : BigUint
    {
        BigUint r = BigUint { .limbs = Vec::filled(k / 32, 0: u32) };
        Vec<u32> moved = BigUint::shifted_limbs(a, narrow<u32>(k % 32), 0);
        Vec::extend_from(&mut r.limbs, &moved[0..$]);
        BigUint::trim(&mut r);
        r
    }

    // `a` shifted left by `k` bits, with `extra` more zero limbs on top.
    fn BigUint::shifted_limbs(ref<BigUint, shared> a, u32 k, usize extra) : Vec<u32>
    {
        Vec<u32> out = Vec::new();
        u32 carry = 0;
        for (usize i = 0; i < Vec::len(&a.limbs); i += 1)
        {
            u32 x = a.limbs[i];
            if (k == 0)
            {
                Vec::push(&mut out, x);
            }
            else
            {
                Vec::push(&mut out, (x << k) | carry);
                carry = x >> (32 - k);
            }
        }
        Vec::push(&mut out, carry);
        for (usize i = 0; i < extra; i += 1)
        {
            Vec::push(&mut out, 0);
        }
        out
    }

    // The quotient and remainder of `a` by `b` (D. Knuth's algorithm D).
    // `b` zero is `diag.div-by-zero`.
    export struct BigDivision
    {
        export BigUint quotient;
        export BigUint remainder;
    }

    export fn BigUint::div_rem(ref<BigUint, shared> a, ref<BigUint, shared> b) : BigDivision
    {
        if (BigUint::is_zero(b))
        {
            fault(div_by_zero);
        }
        if (BigUint::compare(a, b) < 0)
        {
            return BigDivision { .quotient = BigUint::zero(), .remainder = BigUint::clone(a) };
        }
        usize n = Vec::len(&b.limbs);
        if (n == 1)
        {
            u64 d = widen<u64>(b.limbs[0]);
            BigUint q = BigUint { .limbs = Vec::filled(Vec::len(&a.limbs), 0: u32) };
            u64 rem = 0;
            usize i = Vec::len(&a.limbs);
            while (i > 0)
            {
                i -= 1;
                u64 cur = (rem << 32) | widen<u64>(a.limbs[i]);
                q.limbs[i] = narrow<u32>(cur / d);
                rem = cur % d;
            }
            BigUint::trim(&mut q);
            return BigDivision { .quotient = q, .remainder = BigUint::from_u64(rem) };
        }
        u32 s = leading_zeros(b.limbs[n - 1]);
        Vec<u32> bn = BigUint::shifted_limbs(b, s, 0);
        Vec::pop(&mut bn);
        Vec<u32> an = BigUint::shifted_limbs(a, s, 0);
        usize m = Vec::len(&a.limbs) - n;
        BigUint q = BigUint { .limbs = Vec::filled(m + 1, 0: u32) };
        u64 base = 0x100000000;
        u64 btop = widen<u64>(bn[n - 1]);
        u64 bnext = widen<u64>(bn[n - 2]);
        usize jj = m + 1;
        while (jj > 0)
        {
            jj -= 1;
            usize j = jj;
            u64 num = (widen<u64>(an[j + n]) << 32) | widen<u64>(an[j + n - 1]);
            u64 qhat = num / btop;
            u64 rhat = num % btop;
            while (qhat >= base || qhat * bnext > ((rhat << 32) | widen<u64>(an[j + n - 2])))
            {
                qhat -= 1;
                rhat += btop;
                if (rhat >= base)
                {
                    break;
                }
            }
            i64 k = 0;
            i64 t = 0;
            for (usize i = 0; i < n; i += 1)
            {
                u64 p = qhat * widen<u64>(bn[i]);
                t = widen<i64>(an[i + j]) - k - reinterpret<i64>(p & 0xffffffff);
                an[i + j] = narrow_wrapping<u32>(reinterpret<u64>(t));
                k = reinterpret<i64>(p >> 32) - (t >> 32);
            }
            t = widen<i64>(an[j + n]) - k;
            an[j + n] = narrow_wrapping<u32>(reinterpret<u64>(t));
            if (t < 0)
            {
                qhat -= 1;
                u64 c = 0;
                for (usize i = 0; i < n; i += 1)
                {
                    u64 sum = widen<u64>(an[i + j]) + widen<u64>(bn[i]) + c;
                    an[i + j] = narrow_wrapping<u32>(sum);
                    c = sum >> 32;
                }
                an[j + n] = wrapping_add(an[j + n], narrow_wrapping<u32>(c));
            }
            q.limbs[j] = narrow<u32>(qhat);
        }
        BigUint::trim(&mut q);
        BigUint r = BigUint { .limbs = Vec::new() };
        for (usize i = 0; i < n; i += 1)
        {
            u32 hi = if (s == 0 || i + 1 >= n) { 0 } else { an[i + 1] << (32 - s) };
            Vec::push(&mut r.limbs, (an[i] >> s) | hi);
        }
        BigUint::trim(&mut r);
        BigDivision { .quotient = q, .remainder = r }
    }

    // a mod m; `m` zero is `diag.div-by-zero`.
    export fn BigUint::rem(ref<BigUint, shared> a, ref<BigUint, shared> m) : BigUint
    {
        BigDivision { quotient, remainder } = BigUint::div_rem(a, m);
        remainder
    }

    // base^exp mod m, by squaring and multiplying, left to right over `exp`'s
    // bits. `m` zero is `diag.div-by-zero`; anything mod 1 is 0.
    export fn BigUint::mod_pow(ref<BigUint, shared> base, ref<BigUint, shared> exp, ref<BigUint, shared> m) : BigUint
    {
        BigUint one = BigUint::from_u64(1);
        BigUint result = BigUint::rem(&one, m);
        BigUint b = BigUint::rem(base, m);
        usize bits = BigUint::bit_len(exp);
        usize i = bits;
        while (i > 0)
        {
            i -= 1;
            overwrite(&mut result, BigUint::rem(&BigUint::mul(&result, &result), m));
            if (BigUint::bit(exp, i))
            {
                overwrite(&mut result, BigUint::rem(&BigUint::mul(&result, &b), m));
            }
        }
        result
    }

    export fn BigUint::clone(ref<BigUint, shared> a) : BigUint
    {
        BigUint { .limbs = Vec::clone(&a.limbs) }
    }

    // a⁻¹ mod m: the number below m whose product with a is 1 mod m, or None
    // when a and m share a factor (or m is below 2). Extended Euclid, the
    // coefficient kept below m (D-0169). Like every BigUint operation, in a
    // time that depends on the values: for public numbers and for one-time
    // blinding factors, not for a private key.
    export fn BigUint::mod_inverse(ref<BigUint, shared> a, ref<BigUint, shared> m) : Option<BigUint>
    {
        BigUint one = BigUint::from_u64(1);
        if (!BigUint::less(&one, m))
        {
            return None;
        }
        BigUint r0 = BigUint::clone(m);
        BigUint r1 = BigUint::rem(a, m);
        BigUint t0 = BigUint::zero();
        BigUint t1 = BigUint::from_u64(1);
        while (!BigUint::is_zero(&r1))
        {
            BigDivision { quotient, remainder } = BigUint::div_rem(&r0, &r1);
            // t2 = t0 - q·t1 mod m, computed without going below zero.
            BigUint qt = BigUint::rem(&BigUint::mul(&quotient, &t1), m);
            BigUint t2 = if (BigUint::less(&t0, &qt))
            {
                BigUint::sub(&BigUint::add(&t0, m), &qt)
            }
            else
            {
                BigUint::sub(&t0, &qt)
            };
            overwrite(&mut r0, r1);
            r1 = remainder;
            overwrite(&mut t0, t1);
            t1 = t2;
        }
        if (!BigUint::eq(&r0, &one))
        {
            return None;
        }
        Some(t0)
    }

    // The number in decimal digits.
    export fn BigUint::text(ref<BigUint, shared> a) : String
    {
        if (BigUint::is_zero(a))
        {
            return "0";
        }
        Vec<u32> chunks = Vec::new();
        BigUint billion = BigUint::from_u64(1000000000);
        BigUint cur = BigUint::clone(a);
        while (!BigUint::is_zero(&cur))
        {
            BigDivision { quotient, remainder } = BigUint::div_rem(&cur, &billion);
            u32 low = if (Vec::len(&remainder.limbs) == 0) { 0 } else { remainder.limbs[0] };
            Vec::push(&mut chunks, low);
            overwrite(&mut cur, quotient);
        }
        String out = String::new();
        usize i = Vec::len(&chunks);
        i -= 1;
        String::appendf(&mut out, "%u", chunks[i]);
        while (i > 0)
        {
            i -= 1;
            String::appendf(&mut out, "%09u", chunks[i]);
        }
        out
    }

    // Reads decimal digits: `Err(Empty)` for "", `Err(Invalid(i))` at the
    // first byte that is not a digit.
    export fn BigUint::parse(StringView text) : Result<BigUint, ParseError>
    {
        usize n = StringView::len(text);
        if (n == 0)
        {
            return Err(Empty);
        }
        BigUint r = BigUint::zero();
        BigUint ten = BigUint::from_u64(10);
        for (usize i = 0; i < n; i += 1)
        {
            u8 c = text.bytes[i];
            if (!ascii_is_digit(c))
            {
                return Err(Invalid(i));
            }
            overwrite(&mut r, BigUint::add(&BigUint::mul(&r, &ten), &BigUint::from_u64(widen<u64>(c - b'0'))));
        }
        Ok(r)
    }

**Depends on:** rule.stdlib.vec, rule.arith.checked, rule.stdlib.text, D-0157
**Affects:** none (pure)

## 2m. Cryptography

In `std::crypto` (D-0151), in four submodules it re-exports (D-0187):
`digest` (the hashes, `digest_eq`, `crc32`), `kdf` (HMAC, HKDF, PBKDF2,
Argon2id, password hashing), `aead` (ChaCha20-Poly1305, AES-GCM) and `pk`
(X25519, P-256/P-384, ECDSA, Ed25519, RSA).

### `rule.stdlib.crypto`
**Status:** ACCEPTED

Hashes, message authentication codes, key derivation, authenticated
encryption, key agreement, signatures and password hashing, written in
CobaltC so that both implementations compute exactly the listing below.
A digest is an `array<u8, 32>` (or a `Vec<u8>` when its length is chosen
at run time): a plain value, printed with `to_hex`. Keys and nonces come
from `os_random_bytes` (D-0142); every private key is checked where it is
made and where it is used.

`crc32` (`[CRC32]`, §2g) is in this submodule too, beside the hashes,
but it is **not** cryptographic: it detects accidental corruption (a
damaged file, a garbled packet), and anyone can make data with any CRC
they choose. To detect tampering, use a hash or a MAC (D-0177).

    [Sha256]           ⟨sha256(d), Σ⟩ → the SHA-256 digest of the bytes d (FIPS 180-4), 32 bytes;
                       Sha256::new, update and finish compute the same digest of the concatenation
                       of every update's bytes, however the message is cut; a Sha256 is a plain
                       value, so a copy continues on its own
    [Hmac-Sha256]      ⟨hmac_sha256(k, d), Σ⟩ → HMAC-SHA-256 of d under the key k (RFC 2104): a key
                       longer than 64 bytes is replaced by its SHA-256 digest, then padded with zero
                       bytes to 64; HmacSha256::new, update and finish as Sha256's
    [Digest-Eq]        ⟨digest_eq(a, b), Σ⟩ → whether a and b have the same length and the same bytes,
                       every byte being compared whatever the first difference, so the time taken
                       depends on the lengths alone
    [Hkdf]             (D-0152, RFC 5869)   ⟨hkdf_extract(salt, ikm), Σ⟩ → HMAC-SHA-256(salt, ikm), with
                       32 zero bytes for an empty salt; ⟨hkdf_expand(prk, info, len), Σ⟩ → the first len bytes
                       of T(1) ‖ T(2) ‖ …, T(i) = HMAC-SHA-256(prk, T(i-1) ‖ info ‖ i) with T(0) empty and i one
                       byte; len > 8160 is diag.hkdf-length; ⟨hkdf_sha256(salt, ikm, info, len), Σ⟩ ≡
                       hkdf_expand(hkdf_extract(salt, ikm), info, len)
    [ChaCha20]         (D-0153, RFC 8439 §2.4)   ⟨chacha20(key, nonce, counter, d), Σ⟩ → d exclusive-ored with
                       ChaCha20's keystream for the 32-byte key and 12-byte nonce from block counter;
                       a block counter past 2^32 - 1 is diag.crypto-length
    [Poly1305]         (D-0153, RFC 8439 §2.5)   ⟨poly1305(key, m), Σ⟩ → the 16-byte tag of m under the
                       one-time 32-byte key
    [Aead-Seal]        (D-0153, RFC 8439 §2.8)   ⟨chacha20_poly1305_seal(key, nonce, aad, p), Σ⟩ →
                       chacha20(key, nonce, 1, p) ‖ tag, the tag Poly1305 under the first 32 bytes of block
                       0 over aad, padded to 16, the ciphertext, padded to 16, and both lengths as 8-byte
                       little-endian numbers
    [Aead-Open]        (D-0153)   ⟨chacha20_poly1305_open(key, nonce, aad, s), Σ⟩ → Some(p) when s is the
                       sealing of some p under key, nonce and aad (its tag checked with digest_eq), None
                       otherwise, s shorter than 16 bytes included; nothing of a refused message is given
    [X25519]           (D-0154, RFC 7748)   ⟨x25519(k, u), Σ⟩ → Some(X25519(k, u)), the clamped scalar k
                       times the point with u-coordinate u on Curve25519, or None when that is all zeros
                       (a public key of small order); ⟨x25519_public_key(k), Σ⟩ → X25519(k, 9);
                       x25519_private_key() gives 32 bytes of os_random_bytes
    [Sha512]           (D-0156)   ⟨sha512(d), Σ⟩ → the SHA-512 digest of d (FIPS 180-4), 64 bytes; ⟨sha384(d), Σ⟩ →
                       SHA-384, 48 bytes; Sha512 and Sha384 new, update and finish as Sha256's
    [Hash]             (D-0158)   ⟨hash(k, d), Σ⟩ → sha256(d), sha384(d) or sha512(d) as k is Sha2_256,
                       Sha2_384 or Sha2_512, as a Vec<u8>
    [Rsa-Pkcs1v15]     (D-0158, RFC 8017 §8.2.2)   ⟨rsa_verify_pkcs1v15(key, k, digest, sig), Σ⟩ → true exactly
                       when sig is as long as key.n, below it, and sig^e mod n is 00 01 FF…FF 00 ‖ the
                       DigestInfo of k ‖ digest (at least eight FF bytes), the block compared whole
    [Rsa-Pss]          (D-0158, RFC 8017 §8.1.2)   ⟨rsa_verify_pss(key, k, digest, sig), Σ⟩ → true exactly when
                       EMSA-PSS-VERIFY accepts sig^e mod n with hash k, MGF1 over k, and a salt as long as
                       the digest
    [Ecdsa]            (D-0159, FIPS 186-4 §6.4.2)   ⟨ecdsa_verify(c, q, digest, sig), Σ⟩ → true exactly when q
                       is an uncompressed point on curve c, sig is the strict DER of integers r, s in
                       [1, n-1], and the x-coordinate of u1·G + u2·Q mod n is r, with e the digest's
                       leftmost bits, w = s⁻¹, u1 = e·w, u2 = r·w mod n
    [Hkdf-Expand-Label]  (D-0161, RFC 8446 §7.1)   ⟨hkdf_expand_label(secret, label, ctx, len), Σ⟩ →
                       hkdf_expand(secret, len as 2 bytes ‖ the length of "tls13 " ‖ label, in one byte ‖
                       "tls13 " ‖ label ‖ the length of ctx, in one byte ‖ ctx, len)
    [Public-Verify]    (D-0158, D-0159)   rsa_verify_* and ecdsa_verify use only public values and are
                       not bound by [Constant-Time]; a malformed key or signature gives false, never a fault
    [Aes]              (D-0165, FIPS 197)   ⟨aes_encrypt_block(key, b), Σ⟩ → the block b encrypted with
                       AES-128, AES-192 or AES-256 as key is 16, 24 or 32 bytes
    [Aes-Gcm-Seal]     (D-0165, SP 800-38D)   ⟨aes_gcm_seal(key, nonce, aad, p), Σ⟩ → GCTR(key, J0 + 1, p) ‖
                       tag, J0 = nonce ‖ 00 00 00 01, the tag E(key, J0) ⊕ GHASH under H = E(key, 0¹²⁸) of
                       aad, padded to 16, the ciphertext, padded to 16, and both lengths in bits as 8-byte
                       big-endian numbers
    [Aes-Gcm-Open]     (D-0165)   ⟨aes_gcm_open(key, nonce, aad, s), Σ⟩ → Some(p) when s is the sealing of
                       some p under key, nonce and aad (its tag checked with digest_eq), None otherwise, s
                       shorter than 16 bytes included; nothing of a refused message is given
    [P256-Ecdh]        (D-0165, SEC 1 §3.3.1)   ⟨p256_ecdh(k, q), Σ⟩ → Some(the x-coordinate of k·Q, 32
                       bytes) when k names a number in [1, n-1] and q is an uncompressed point (0x04 ‖ x ‖ y)
                       on P-256, None otherwise; ⟨p256_public_key(k), Σ⟩ → Some(0x04 ‖ x ‖ y of k·G) or None
                       when k is outside [1, n-1]; p256_private_key() gives 32 bytes of os_random_bytes
                       naming a number in [1, n-1], drawn again otherwise
    [Hasher]           (D-0166)   Hasher::new(k), update and finish compute hash(k, d) of the concatenation
                       of every update's bytes; a Hasher is a plain value
    [Hmac]             (D-0166, RFC 2104)   ⟨hmac(k, key, d), Σ⟩ → HMAC over the hash k of d under key: a key
                       longer than the hash's block (64 bytes for SHA-256, 128 for SHA-384/512) is
                       replaced by its digest, then padded with zero bytes to the block; Hmac::new, update
                       and finish as Hasher's; hmac(Sha2_256, key, d) ≡ hmac_sha256(key, d)
    [Hkdf-With]        (D-0166, RFC 5869)   hkdf_extract_with, hkdf_expand_with, hkdf_with and
                       hkdf_expand_label_with are [Hkdf] and [Hkdf-Expand-Label] with HMAC over the hash k in
                       place of HMAC-SHA-256 and k's digest length in place of 32: an empty salt stands for
                       that many zero bytes and len above 255 digests is diag.hkdf-length
    [Ed25519-Sign]     (D-0167, RFC 8032 §5.1)   ⟨ed25519_public_key(k), Σ⟩ → the encoding of s·B, s the
                       clamped first half of SHA-512(k); ⟨ed25519_sign(k, m), Σ⟩ → R ‖ S, R the encoding of
                       r·B with r = SHA-512(prefix ‖ m) mod L, prefix the second half of SHA-512(k), and
                       S = r + SHA-512(R ‖ A ‖ m)·s mod L as 32 little-endian bytes; the same for the same
                       k and m every time; ed25519_private_key() gives 32 bytes of os_random_bytes
    [Ed25519-Verify]   (D-0167, RFC 8032 §5.1.7)   ⟨ed25519_verify(a, m, sig), Σ⟩ → true exactly when a and
                       sig are 32 and 64 bytes, a and the first 32 bytes of sig are canonical encodings of
                       points A and R, S (the rest, little-endian) is below L, and S·B = R + k·A with
                       k = SHA-512(R ‖ A ‖ m) mod L
    [Ec-Keys]          (D-0168)   ⟨ec_public_key(c, k), Σ⟩ → Some(0x04 ‖ x ‖ y of k·G on c) when k, of
                       c's length (32 or 48 bytes), names a number in [1, n-1], None otherwise;
                       ec_private_key(c) gives that many bytes of os_random_bytes naming a number in
                       [1, n-1], drawn again otherwise
    [Ecdh]             (D-0168, SEC 1 §3.3.1)   ⟨ecdh(c, k, q), Σ⟩ → Some(the x-coordinate of k·Q, c's
                       length) when k names a number in [1, n-1] and q is an uncompressed point on c, None
                       otherwise; p256_private_key, p256_public_key and p256_ecdh ([P256-Ecdh]) are these
                       on P256
    [Ecdsa-Sign]       (D-0168, FIPS 186-4 §6.4.1, RFC 6979)   ⟨ecdsa_sign(c, k, digest), Σ⟩ → Some(the
                       strict DER of r, s) when k names a number in [1, n-1], None otherwise: e the
                       digest's leftmost bits as in [Ecdsa], the nonce RFC 6979 §3.2's from k and e over
                       HMAC-SHA-256 (P256) or HMAC-SHA-384 (P384), r the x-coordinate of nonce·G mod n,
                       s = nonce⁻¹·(e + r·k) mod n, the next nonce taken when r or s is 0; so
                       ecdsa_verify(c, ec_public_key(c, k), digest, sig) holds and the signature is the
                       same every time
    [Rsa-Private-Key]  (D-0169)   ⟨RsaPrivateKey::new(n, e, d, p, q), Σ⟩ → Some(key) exactly when p and q
                       are odd and above 2, p·q = n, and e·d ≡ 1 modulo p - 1 and modulo q - 1;
                       public_key gives n and e
    [Rsa-Private-Op]   (D-0169)   m^d mod n is computed separately modulo p and q with a constant-time
                       exponentiation, combined by the Chinese remainder theorem on the same
                       fixed-length arithmetic, and checked: s^e mod n = m, else the exponentiation
                       modulo n is used instead
    [Rsa-Sign-Pkcs1v15]  (D-0169, RFC 8017 §8.2.1)   ⟨rsa_sign_pkcs1v15(key, k, digest), Σ⟩ → the k-byte
                       signature such that rsa_verify_pkcs1v15(key.public_key(), k, digest, it) holds,
                       the same every time; a digest not of k's length, or a modulus shorter than the
                       encoding plus 11 bytes, is diag.crypto-length
    [Rsa-Sign-Pss]     (D-0169, RFC 8017 §8.1.1)   ⟨rsa_sign_pss(key, k, digest), Σ⟩ → a signature such that
                       rsa_verify_pss(key.public_key(), k, digest, it) holds, with MGF1 over k and a salt
                       of os_random_bytes as long as the digest (so different each time); a digest not of
                       k's length, or a modulus too short for two digests and two bytes, is
                       diag.crypto-length
    [Rsa-Generate]     (D-0169, FIPS 186-4 B.3.3)   ⟨rsa_generate_key(bits), Σ⟩ → a key with e = 65537
                       and n = p·q of exactly bits bits, p and q distinct primes of bits/2 bits beyond
                       reasonable doubt (no factor below 1000, five rounds of Miller–Rabin with random
                       bases), p - 1 and q - 1 prime to e; bits below 1024, above 8192 or not a multiple
                       of 64 is diag.crypto-length
    [Blake2b]          (D-0170, RFC 7693)   ⟨blake2b(d, len), Σ⟩ → the BLAKE2b digest of d of len bytes
                       (1 to 64); Blake2b::new(len), new_keyed(key, len) (key up to 64 bytes, the keyed
                       mode of §2.1), update and finish as Hasher's; another len or key length is
                       diag.crypto-length
    [Pbkdf2]           (D-0170, RFC 8018 §5.2)   ⟨pbkdf2(k, p, salt, c, len), Σ⟩ → the first len bytes of
                       T(1) ‖ T(2) ‖ …, T(i) = U(1) ⊕ … ⊕ U(c), U(1) = HMAC-k(p, salt ‖ i as 4 big-endian
                       bytes), U(j) = HMAC-k(p, U(j-1)); c = 0 is diag.kdf-parameters
    [Argon2id]         (D-0170, RFC 9106)   ⟨argon2id(p, salt, t, m, lanes, len), Σ⟩ → the Argon2id
                       (type 2, version 0x13) tag of len bytes from password p and salt with t passes over
                       4·lanes·⌊m / (4·lanes)⌋ KiB in lanes lanes, no secret key and no associated data
    [Password-Hash]    (D-0170)   ⟨hash_password(p), Σ⟩ → the text $argon2id$v=19$m=19456,t=2,p=1$S$H,
                       S the base64 (no padding) of 16 bytes of os_random_bytes and H that of
                       argon2id(p, salt, 2, 19456, 1, 32); ⟨verify_password(text, p), Σ⟩ → true exactly
                       when text has that shape with any m, t, p within [Kdf-Parameters]' bounds, and
                       argon2id(p, S, t, m, p, |H|) is H, compared with digest_eq; false otherwise
    [Kdf-Parameters]   disposition: checked   pbkdf2 with no iterations, or argon2id with no passes, no
                       lanes or more than 255, memory below 8 KiB per lane, a salt shorter than 8 bytes
                       or a result shorter than 4 ↛ diag.kdf-parameters
    [Crypto-Length]    disposition: checked   a ChaCha20, AEAD, X25519, P-256 or Ed25519 private key that
                       is not 32 bytes, a P-384 private key that is not 48, an AES key that is not 16, 24
                       or 32, a nonce that is not 12, a ChaCha20 counter that would pass 2^32 - 1, a
                       BLAKE2b digest length outside 1 to 64 or key above 64 bytes, an RSA digest not of
                       its hash's length, an RSA modulus too short for the signature encoding, or an RSA
                       key size outside 1024 to 8192 or not a multiple of 64 ↛ diag.crypto-length
    [Constant-Time]    (D-0153, D-0154, D-0165, D-0167, D-0168, D-0169)   the steps chacha20, poly1305,
                       both AEADs, AES, the X25519, P-256, P-384 and Ed25519 functions, and the RSA private
                       operation take depend on the lengths of their arguments only, never on the bytes of
                       a key, a nonce, a scalar or a message: no branch and no index is taken on them
                       (ecdsa_sign branches once, on whether a candidate nonce is below n; the RSA
                       operation's BigUint steps see only public numbers). Verification, and Argon2id's
                       password-dependent addressing, are outside it

A program that checks a MAC it received compares with `digest_eq`,
never `==`: an early-exit comparison tells a sender how many leading
bytes it guessed right.


    [Sha1]   (D-0181, FIPS 180-4)   Sha1, sha1(data) : array<u8, 20> as [Sha256] with SHA-1's five words, 80 rounds and
             functions; sha1("abc") is a9993e364706816aba3e25717850c26c9cd0d89d. HashKind::Sha1 names it in hash,
             Hasher, Hmac, hmac, pbkdf2 and the HKDF forms (a 20-byte digest, a 64-byte block) and in the DigestInfo of
             [Rsa-Pkcs1]. Legacy: for what older protocols require (RFC 6238's default HMAC, git's object names,
             signatures to check), never for a new signature — collisions have been made

The `std` source:

    // `std::crypto` (spec/21 §2m, D-0151): SHA-256 (FIPS 180-4), HMAC-SHA-256
    // (RFC 2104) and a comparison of digests that takes the same time whatever
    // they hold, then (D-0152–D-0159, D-0165) HKDF, ChaCha20-Poly1305, AES-GCM,
    // X25519 and P-256 key agreement, SHA-384/512, and RSA and ECDSA signature
    // verification; then (D-0166–D-0170) HMAC and HKDF over any of the hashes,
    // Ed25519, EC keys and ECDSA signing on P-256 and P-384, RSA private keys,
    // signing and key generation, and BLAKE2b, PBKDF2, Argon2id and password
    // hashing. Written in CobaltC, so both implementations compute exactly
    // this. Keys and nonces come from `os_random_bytes` (D-0142). Also
    // `crc32` (D-0119, D-0177), a checksum that is not cryptographic.

    const array<u32, 64> SHA256_K = [
        0x428a2f98,
        0x71374491,
        0xb5c0fbcf,
        0xe9b5dba5,
        0x3956c25b,
        0x59f111f1,
        0x923f82a4,
        0xab1c5ed5,
        0xd807aa98,
        0x12835b01,
        0x243185be,
        0x550c7dc3,
        0x72be5d74,
        0x80deb1fe,
        0x9bdc06a7,
        0xc19bf174,
        0xe49b69c1,
        0xefbe4786,
        0x0fc19dc6,
        0x240ca1cc,
        0x2de92c6f,
        0x4a7484aa,
        0x5cb0a9dc,
        0x76f988da,
        0x983e5152,
        0xa831c66d,
        0xb00327c8,
        0xbf597fc7,
        0xc6e00bf3,
        0xd5a79147,
        0x06ca6351,
        0x14292967,
        0x27b70a85,
        0x2e1b2138,
        0x4d2c6dfc,
        0x53380d13,
        0x650a7354,
        0x766a0abb,
        0x81c2c92e,
        0x92722c85,
        0xa2bfe8a1,
        0xa81a664b,
        0xc24b8b70,
        0xc76c51a3,
        0xd192e819,
        0xd6990624,
        0xf40e3585,
        0x106aa070,
        0x19a4c116,
        0x1e376c08,
        0x2748774c,
        0x34b0bcb5,
        0x391c0cb3,
        0x4ed8aa4a,
        0x5b9cca4f,
        0x682e6ff3,
        0x748f82ee,
        0x78a5636f,
        0x84c87814,
        0x8cc70208,
        0x90befffa,
        0xa4506ceb,
        0xbef9a3f7,
        0xc67178f2,
    ];

    // A SHA-256 computation in progress: the eight state words, the bytes of
    // the block not yet compressed, and how many bytes have been given in all.
    // A plain value: it can be copied to hash two continuations of one prefix.
    export struct Sha256
    {
        array<u32, 8> h;
        array<u8, 64> block;
        usize fill;
        u64 length;
    }

    export fn Sha256::new() : Sha256
    {
        Sha256 { .h = [
            0x6a09e667,
            0xbb67ae85,
            0x3c6ef372,
            0xa54ff53a,
            0x510e527f,
            0x9b05688c,
            0x1f83d9ab,
            0x5be0cd19,
        ], .block = [0; 64], .fill = 0, .length = 0 }
    }

    // One 64-byte block folded into the state `h`. Both are values: nothing
    // here goes through a reference, so nothing is checked at run time.
    fn sha256_block(array<u32, 8> h, array<u8, 64> blk) : array<u32, 8>
    {
        array<u32, 64> k = SHA256_K;
        array<u32, 64> w = [0; 64];
        for (usize i = 0; i < 16; i += 1)
        {
            w[i] = (widen<u32>(blk[4 * i]) << 24) | (widen<u32>(blk[4 * i + 1]) << 16) | (widen<u32>(blk[4 * i + 2]) << 8) | widen<u32>(blk[4 * i + 3]);
        }
        for (usize i = 16; i < 64; i += 1)
        {
            u32 s0 = rotate_right(w[i - 15], 7) ^ rotate_right(w[i - 15], 18) ^ (w[i - 15] >> 3);
            u32 s1 = rotate_right(w[i - 2], 17) ^ rotate_right(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = wrapping_add(wrapping_add(w[i - 16], s0), wrapping_add(w[i - 7], s1));
        }
        u32 a = h[0];
        u32 b = h[1];
        u32 c = h[2];
        u32 d = h[3];
        u32 e = h[4];
        u32 f = h[5];
        u32 g = h[6];
        u32 hh = h[7];
        for (usize i = 0; i < 64; i += 1)
        {
            u32 big_s1 = rotate_right(e, 6) ^ rotate_right(e, 11) ^ rotate_right(e, 25);
            u32 ch = (e & f) ^ (~e & g);
            u32 t1 = wrapping_add(wrapping_add(wrapping_add(hh, big_s1), wrapping_add(ch, k[i])), w[i]);
            u32 big_s0 = rotate_right(a, 2) ^ rotate_right(a, 13) ^ rotate_right(a, 22);
            u32 maj = (a & b) ^ (a & c) ^ (b & c);
            u32 t2 = wrapping_add(big_s0, maj);
            hh = g;
            g = f;
            f = e;
            e = wrapping_add(d, t1);
            d = c;
            c = b;
            b = a;
            a = wrapping_add(t1, t2);
        }
        array<u32, 8> out = h;
        out[0] = wrapping_add(h[0], a);
        out[1] = wrapping_add(h[1], b);
        out[2] = wrapping_add(h[2], c);
        out[3] = wrapping_add(h[3], d);
        out[4] = wrapping_add(h[4], e);
        out[5] = wrapping_add(h[5], f);
        out[6] = wrapping_add(h[6], g);
        out[7] = wrapping_add(h[7], hh);
        out
    }

    // `data` added to a state given and returned by value: `data` is then the
    // function's only reference, read element by element, which the compiler
    // can leave unchecked (`Sha256::update` itself holds two references).
    fn sha256_absorb(Sha256 s, slice<u8, shared> data) : Sha256
    {
        usize n = slice_len(data);
        usize i = 0;
        while (i < n)
        {
            if (s.fill == 0 && n - i >= 64)
            {
                array<u8, 64> blk = [0; 64];
                for (usize k = 0; k < 64; k += 1)
                {
                    blk[k] = data[i + k];
                }
                s.h = sha256_block(s.h, blk);
                i += 64;
            }
            else
            {
                s.block[s.fill] = data[i];
                s.fill += 1;
                i += 1;
                if (s.fill == 64)
                {
                    s.h = sha256_block(s.h, s.block);
                    s.fill = 0;
                }
            }
        }
        s.length = wrapping_add(s.length, widen<u64>(n));
        s
    }

    // Adds `data` to the message; call as often as the message arrives.
    export fn Sha256::update(ref<Sha256, exclusive> s, slice<u8, shared> data)
    {
        *s = sha256_absorb(*s, data);
    }

    // The digest of everything given: 32 bytes. The computation is consumed;
    // copy it first to continue hashing.
    export fn Sha256::finish(Sha256 s) : array<u8, 32>
    {
        u64 bits = wrapping_mul(s.length, 8);
        array<u32, 8> h = s.h;
        array<u8, 64> last = s.block;
        usize f = s.fill;
        last[f] = 0x80;
        for (usize i = f + 1; i < 64; i += 1)
        {
            last[i] = 0;
        }
        if (f >= 56)
        {
            h = sha256_block(h, last);
            last = [0; 64];
        }
        for (usize i = 0; i < 8; i += 1)
        {
            last[56 + i] = narrow_wrapping<u8>(bits >> narrow<u32>(56 - 8 * i));
        }
        h = sha256_block(h, last);
        array<u8, 32> out = [0; 32];
        for (usize i = 0; i < 8; i += 1)
        {
            out[4 * i] = narrow_wrapping<u8>(h[i] >> 24);
            out[4 * i + 1] = narrow_wrapping<u8>(h[i] >> 16);
            out[4 * i + 2] = narrow_wrapping<u8>(h[i] >> 8);
            out[4 * i + 3] = narrow_wrapping<u8>(h[i]);
        }
        out
    }

    // The SHA-256 digest of `data` in one call.
    export fn sha256(slice<u8, shared> data) : array<u8, 32>
    {
        Sha256 s = Sha256::new();
        Sha256::update(&mut s, data);
        Sha256::finish(s)
    }

    // An HMAC-SHA-256 computation in progress: the inner hash and the outer
    // one, each already fed its padded key. A plain value: a copy of a keyed
    // `HmacSha256` continues on its own.
    export struct HmacSha256
    {
        Sha256 inner;
        Sha256 outer;
    }

    // A MAC under `key`, of any length (one longer than 64 bytes is hashed
    // first, as RFC 2104 says).
    export fn HmacSha256::new(slice<u8, shared> key) : HmacSha256
    {
        array<u8, 64> k = [0; 64];
        if (slice_len(key) > 64)
        {
            array<u8, 32> d = sha256(key);
            for (usize i = 0; i < 32; i += 1)
            {
                k[i] = d[i];
            }
        }
        else
        {
            for (usize i = 0; i < slice_len(key); i += 1)
            {
                k[i] = key[i];
            }
        }
        array<u8, 64> ipad = [0; 64];
        array<u8, 64> opad = [0; 64];
        for (usize i = 0; i < 64; i += 1)
        {
            ipad[i] = k[i] ^ 0x36;
            opad[i] = k[i] ^ 0x5c;
        }
        Sha256 inner = Sha256::new();
        Sha256::update(&mut inner, &ipad[0..$]);
        Sha256 outer = Sha256::new();
        Sha256::update(&mut outer, &opad[0..$]);
        HmacSha256 { .inner = inner, .outer = outer }
    }

    export fn HmacSha256::update(ref<HmacSha256, exclusive> m, slice<u8, shared> data)
    {
        Sha256::update(&mut m.inner, data);
    }

    // The MAC of everything given: 32 bytes.
    export fn HmacSha256::finish(HmacSha256 m) : array<u8, 32>
    {
        array<u8, 32> inner_digest = Sha256::finish(m.inner);
        Sha256 outer = m.outer;
        Sha256::update(&mut outer, &inner_digest[0..$]);
        Sha256::finish(outer)
    }

    // The HMAC-SHA-256 of `data` under `key` in one call.
    export fn hmac_sha256(slice<u8, shared> key, slice<u8, shared> data) : array<u8, 32>
    {
        HmacSha256 m = HmacSha256::new(key);
        HmacSha256::update(&mut m, data);
        HmacSha256::finish(m)
    }

    // Whether two digests (or MACs) are the same bytes, in a time that depends
    // only on their lengths: every byte is compared, however early they
    // differ, so a sender guessing a MAC learns nothing from how long the
    // check took. Use this, never `==`, to check a MAC that arrived.
    export fn digest_eq(slice<u8, shared> a, slice<u8, shared> b) : bool
    {
        if (slice_len(a) != slice_len(b))
        {
            return false;
        }
        u8 acc = 0;
        for (usize i = 0; i < slice_len(a); i += 1)
        {
            acc = acc | (a[i] ^ b[i]);
        }
        acc == 0
    }

    // HKDF-SHA-256 (RFC 5869, D-0152): key derivation over HMAC-SHA-256.
    // `hkdf_extract` condenses input keying material and an optional salt
    // into a 32-byte pseudorandom key; `hkdf_expand` stretches that key,
    // bound to `info`, to `len` bytes, at most 8160 (255 blocks). TLS 1.3's
    // key schedule is these two.

    // The pseudorandom key of `ikm` under `salt`; an empty `salt` stands for
    // 32 zero bytes, as the RFC says.
    export fn hkdf_extract(slice<u8, shared> salt, slice<u8, shared> ikm) : array<u8, 32>
    {
        if (slice_len(salt) == 0)
        {
            array<u8, 32> zero = [0; 32];
            return hmac_sha256(&zero[0..$], ikm);
        }
        hmac_sha256(salt, ikm)
    }

    // `len` bytes of output keying material from `prk` and `info`: block i is
    // HMAC(prk, block i-1 ‖ info ‖ i), i counted from 1 in one byte. A `len`
    // above 8160 is `diag.hkdf-length`.
    export fn hkdf_expand(slice<u8, shared> prk, slice<u8, shared> info, usize len) : Vec<u8>
    {
        if (len > 8160)
        {
            fault(hkdf_length);
        }
        Vec<u8> out = Vec::new();
        Vec::reserve(&mut out, len);
        array<u8, 32> t = [0; 32];
        usize have = 0;
        u8 counter = 1;
        while (Vec::len(&out) < len)
        {
            HmacSha256 m = HmacSha256::new(prk);
            HmacSha256::update(&mut m, &t[0..have]);
            HmacSha256::update(&mut m, info);
            array<u8, 1> c = [counter];
            HmacSha256::update(&mut m, &c[0..$]);
            t = HmacSha256::finish(m);
            have = 32;
            for (usize i = 0; i < 32 && Vec::len(&out) < len; i += 1)
            {
                Vec::push(&mut out, t[i]);
            }
            if (Vec::len(&out) < len)
            {
                counter += 1;
            }
        }
        out
    }

    // Extract, then expand, in one call: `len` bytes derived from `ikm` with
    // `salt` and `info`.
    export fn hkdf_sha256(slice<u8, shared> salt, slice<u8, shared> ikm, slice<u8, shared> info, usize len) : Vec<u8>
    {
        array<u8, 32> prk = hkdf_extract(salt, ikm);
        hkdf_expand(&prk[0..$], info, len)
    }

    // ChaCha20 and ChaCha20-Poly1305 (RFC 8439, D-0153): the stream cipher
    // and the authenticated encryption built on it. Every operation on secret
    // data is an addition, a rotation, an exclusive or, a multiplication or a
    // mask: no branch and no array index depends on a key, a nonce or a
    // message byte, so the time taken depends only on the lengths.

    // A key is 32 bytes and a nonce 12; any other length is
    // `diag.crypto-length`.
    fn crypto_check_lengths(slice<u8, shared> key, slice<u8, shared> nonce)
    {
        if (slice_len(key) != 32 || slice_len(nonce) != 12)
        {
            fault(crypto_length);
        }
    }

    fn chacha_quarter(ref<array<u32, 16>, exclusive> x, usize a, usize b, usize c, usize d)
    {
        x[a] = wrapping_add(x[a], x[b]);
        x[d] = rotate_left(x[d] ^ x[a], 16);
        x[c] = wrapping_add(x[c], x[d]);
        x[b] = rotate_left(x[b] ^ x[c], 12);
        x[a] = wrapping_add(x[a], x[b]);
        x[d] = rotate_left(x[d] ^ x[a], 8);
        x[c] = wrapping_add(x[c], x[d]);
        x[b] = rotate_left(x[b] ^ x[c], 7);
    }

    // The 64-byte keystream block for `counter` (RFC 8439 §2.3).
    fn chacha_block(slice<u8, shared> key, slice<u8, shared> nonce, u32 counter) : array<u8, 64>
    {
        array<u32, 16> s = [0; 16];
        s[0] = 0x61707865;
        s[1] = 0x3320646e;
        s[2] = 0x79622d32;
        s[3] = 0x6b206574;
        for (usize i = 0; i < 8; i += 1)
        {
            s[4 + i] = read_le<u32>(key, 4 * i);
        }
        s[12] = counter;
        for (usize i = 0; i < 3; i += 1)
        {
            s[13 + i] = read_le<u32>(nonce, 4 * i);
        }
        array<u32, 16> x = s;
        for (usize round = 0; round < 10; round += 1)
        {
            chacha_quarter(&mut x, 0, 4, 8, 12);
            chacha_quarter(&mut x, 1, 5, 9, 13);
            chacha_quarter(&mut x, 2, 6, 10, 14);
            chacha_quarter(&mut x, 3, 7, 11, 15);
            chacha_quarter(&mut x, 0, 5, 10, 15);
            chacha_quarter(&mut x, 1, 6, 11, 12);
            chacha_quarter(&mut x, 2, 7, 8, 13);
            chacha_quarter(&mut x, 3, 4, 9, 14);
        }
        array<u8, 64> out = [0; 64];
        for (usize i = 0; i < 16; i += 1)
        {
            u32 w = wrapping_add(x[i], s[i]);
            out[4 * i] = narrow_wrapping<u8>(w);
            out[4 * i + 1] = narrow_wrapping<u8>(w >> 8);
            out[4 * i + 2] = narrow_wrapping<u8>(w >> 16);
            out[4 * i + 3] = narrow_wrapping<u8>(w >> 24);
        }
        out
    }

    // `data` exclusive-ored with ChaCha20's keystream for `key` and `nonce`,
    // starting at block `counter` (RFC 8439 §2.4): encryption and decryption
    // are the same operation. A stream cipher alone hides the data but does
    // not protect it from change; use `chacha20_poly1305_seal` for that. More
    // than 2^32 blocks from one counter is `diag.crypto-length`.
    export fn chacha20(slice<u8, shared> key, slice<u8, shared> nonce, u32 counter, slice<u8, shared> data) : Vec<u8>
    {
        crypto_check_lengths(key, nonce);
        usize n = slice_len(data);
        if (widen<u64>(n) > (widen<u64>(max_value<u32>()) - widen<u64>(counter) + 1) * 64)
        {
            fault(crypto_length);
        }
        Vec<u8> out = Vec::new();
        Vec::reserve(&mut out, n);
        u32 block = counter;
        usize at = 0;
        while (at < n)
        {
            array<u8, 64> ks = chacha_block(key, nonce, block);
            usize take = min(64, n - at);
            for (usize i = 0; i < take; i += 1)
            {
                Vec::push(&mut out, data[at + i] ^ ks[i]);
            }
            at += take;
            if (at < n)
            {
                block += 1;
            }
        }
        out
    }

    // Poly1305 (RFC 8439 §2.5) over five 26-bit limbs, as poly1305-donna does:
    // the accumulator, the clamped key `r` and `5·r`, the final pad `s`.
    struct Poly1305
    {
        array<u64, 5> h;
        array<u64, 5> r;
        array<u64, 4> pad;
    }

    fn Poly1305::new(slice<u8, shared> key) : Poly1305
    {
        u64 m26 = 0x3ffffff;
        array<u64, 5> r = [0; 5];
        r[0] = widen<u64>(read_le<u32>(key, 0)) & m26;
        r[1] = (widen<u64>(read_le<u32>(key, 3)) >> 2) & 0x3ffff03;
        r[2] = (widen<u64>(read_le<u32>(key, 6)) >> 4) & 0x3ffc0ff;
        r[3] = (widen<u64>(read_le<u32>(key, 9)) >> 6) & 0x3f03fff;
        r[4] = (widen<u64>(read_le<u32>(key, 12)) >> 8) & 0x00fffff;
        array<u64, 4> pad = [0; 4];
        for (usize i = 0; i < 4; i += 1)
        {
            pad[i] = widen<u64>(read_le<u32>(key, 16 + 4 * i));
        }
        Poly1305 { .h = [0; 5], .r = r, .pad = pad }
    }

    // One 16-byte block; `hibit` is 2^24 for a whole block (the appended 1
    // bit) and 0 for the last, already padded, partial one.
    fn Poly1305::block(ref<Poly1305, exclusive> p, slice<u8, shared> m, u64 hibit)
    {
        p.h = poly1305_blocks(p.h, p.r, m, hibit);
    }

    // The accumulator after every whole 16-byte block of `m`: the state as
    // values and `m` the only reference, read unchecked.
    fn poly1305_blocks(array<u64, 5> h, array<u64, 5> r, slice<u8, shared> m, u64 hibit) : array<u64, 5>
    {
        u64 m26 = 0x3ffffff;
        u64 r0 = r[0];
        u64 r1 = r[1];
        u64 r2 = r[2];
        u64 r3 = r[3];
        u64 r4 = r[4];
        u64 s1 = r1 * 5;
        u64 s2 = r2 * 5;
        u64 s3 = r3 * 5;
        u64 s4 = r4 * 5;
        u64 h0 = h[0];
        u64 h1 = h[1];
        u64 h2 = h[2];
        u64 h3 = h[3];
        u64 h4 = h[4];
        for (usize at = 0; at + 16 <= slice_len(m); at += 16)
        {
            // The little-endian words at 0, 3, 6, 9 and 12.
            array<u64, 5> t = [0; 5];
            for (usize k = 0; k < 5; k += 1)
            {
                usize o = at + 3 * k;
                t[k] = widen<u64>(m[o]) | (widen<u64>(m[o + 1]) << 8) | (widen<u64>(m[o + 2]) << 16) | (widen<u64>(m[o + 3]) << 24);
            }
            h0 += t[0] & m26;
            h1 += (t[1] >> 2) & m26;
            h2 += (t[2] >> 4) & m26;
            h3 += (t[3] >> 6) & m26;
            h4 += (t[4] >> 8) | hibit;
            u64 d0 = h0 * r0 + h1 * s4 + h2 * s3 + h3 * s2 + h4 * s1;
            u64 d1 = h0 * r1 + h1 * r0 + h2 * s4 + h3 * s3 + h4 * s2;
            u64 d2 = h0 * r2 + h1 * r1 + h2 * r0 + h3 * s4 + h4 * s3;
            u64 d3 = h0 * r3 + h1 * r2 + h2 * r1 + h3 * r0 + h4 * s4;
            u64 d4 = h0 * r4 + h1 * r3 + h2 * r2 + h3 * r1 + h4 * r0;
            u64 c = d0 >> 26;
            h0 = d0 & m26;
            d1 += c;
            c = d1 >> 26;
            h1 = d1 & m26;
            d2 += c;
            c = d2 >> 26;
            h2 = d2 & m26;
            d3 += c;
            c = d3 >> 26;
            h3 = d3 & m26;
            d4 += c;
            c = d4 >> 26;
            h4 = d4 & m26;
            h0 += c * 5;
            c = h0 >> 26;
            h0 = h0 & m26;
            h1 += c;
        }
        [h0, h1, h2, h3, h4]
    }

    // Adds `data` to the message, in whole blocks and a final padded one.
    // Called once per message part; each part is padded to 16 bytes, as the
    // AEAD construction pads its parts.
    fn Poly1305::update_padded(ref<Poly1305, exclusive> p, slice<u8, shared> data)
    {
        usize n = slice_len(data);
        usize at = n - n % 16;
        if (at > 0)
        {
            Poly1305::block(p, &data[0..at], 1 << 24);
        }
        if (at < n)
        {
            array<u8, 16> last = [0; 16];
            for (usize i = 0; at + i < n; i += 1)
            {
                last[i] = data[at + i];
            }
            Poly1305::block(p, &last[0..$], 1 << 24);
        }
    }

    // The final, unpadded partial block of a plain Poly1305 message: the 1
    // byte appended and zeros after it, with no extra bit.
    fn Poly1305::update_last(ref<Poly1305, exclusive> p, slice<u8, shared> data)
    {
        usize n = slice_len(data);
        usize at = 0;
        while (at + 16 <= n)
        {
            Poly1305::block(p, &data[at..at + 16], 1 << 24);
            at += 16;
        }
        if (at < n)
        {
            array<u8, 16> last = [0; 16];
            for (usize i = 0; at + i < n; i += 1)
            {
                last[i] = data[at + i];
            }
            last[n - at] = 1;
            Poly1305::block(p, &last[0..$], 0);
        }
    }

    // The 16-byte tag: h reduced mod 2^130 - 5 without a branch on its value,
    // then plus `s` mod 2^128.
    fn Poly1305::finish(Poly1305 p) : array<u8, 16>
    {
        u64 m26 = 0x3ffffff;
        u64 h0 = p.h[0];
        u64 h1 = p.h[1];
        u64 h2 = p.h[2];
        u64 h3 = p.h[3];
        u64 h4 = p.h[4];
        u64 c = h1 >> 26;
        h1 = h1 & m26;
        h2 += c;
        c = h2 >> 26;
        h2 = h2 & m26;
        h3 += c;
        c = h3 >> 26;
        h3 = h3 & m26;
        h4 += c;
        c = h4 >> 26;
        h4 = h4 & m26;
        h0 += c * 5;
        c = h0 >> 26;
        h0 = h0 & m26;
        h1 += c;
        // g = h + 5 - 2^130; it is chosen when it does not go below zero.
        u64 g0 = h0 + 5;
        c = g0 >> 26;
        g0 = g0 & m26;
        u64 g1 = h1 + c;
        c = g1 >> 26;
        g1 = g1 & m26;
        u64 g2 = h2 + c;
        c = g2 >> 26;
        g2 = g2 & m26;
        u64 g3 = h3 + c;
        c = g3 >> 26;
        g3 = g3 & m26;
        u64 g4 = wrapping_sub(h4 + c, 0x4000000);
        u64 take_g = wrapping_sub(g4 >> 63, 1);
        u64 take_h = ~take_g;
        h0 = (h0 & take_h) | (g0 & take_g);
        h1 = (h1 & take_h) | (g1 & take_g);
        h2 = (h2 & take_h) | (g2 & take_g);
        h3 = (h3 & take_h) | (g3 & take_g);
        h4 = (h4 & take_h) | (g4 & take_g);
        u64 m32 = 0xffffffff;
        u64 w0 = (h0 | (h1 << 26)) & m32;
        u64 w1 = ((h1 >> 6) | (h2 << 20)) & m32;
        u64 w2 = ((h2 >> 12) | (h3 << 14)) & m32;
        u64 w3 = ((h3 >> 18) | (h4 << 8)) & m32;
        u64 f = w0 + p.pad[0];
        w0 = f & m32;
        f = w1 + p.pad[1] + (f >> 32);
        w1 = f & m32;
        f = w2 + p.pad[2] + (f >> 32);
        w2 = f & m32;
        f = w3 + p.pad[3] + (f >> 32);
        w3 = f & m32;
        array<u64, 4> w = [w0, w1, w2, w3];
        array<u8, 16> tag = [0; 16];
        for (usize i = 0; i < 4; i += 1)
        {
            tag[4 * i] = narrow_wrapping<u8>(w[i]);
            tag[4 * i + 1] = narrow_wrapping<u8>(w[i] >> 8);
            tag[4 * i + 2] = narrow_wrapping<u8>(w[i] >> 16);
            tag[4 * i + 3] = narrow_wrapping<u8>(w[i] >> 24);
        }
        tag
    }

    // The Poly1305 tag of `message` under a one-time 32-byte key (RFC 8439
    // §2.5). A key must never authenticate two messages: the AEAD below derives
    // a fresh one per nonce. Another key length is `diag.crypto-length`.
    export fn poly1305(slice<u8, shared> key, slice<u8, shared> message) : array<u8, 16>
    {
        if (slice_len(key) != 32)
        {
            fault(crypto_length);
        }
        Poly1305 p = Poly1305::new(key);
        Poly1305::update_last(&mut p, message);
        Poly1305::finish(p)
    }

    // The AEAD tag (RFC 8439 §2.8) of `aad` and `ciphertext` under the
    // one-time key from block 0.
    fn aead_tag(
        slice<u8, shared> key,
        slice<u8, shared> nonce,
        slice<u8, shared> aad,
        slice<u8, shared> ciphertext,
    ) : array<u8, 16>
    {
        array<u8, 64> block0 = chacha_block(key, nonce, 0);
        Poly1305 p = Poly1305::new(&block0[0..32]);
        Poly1305::update_padded(&mut p, aad);
        Poly1305::update_padded(&mut p, ciphertext);
        Vec<u8> lengths = Vec::new();
        Vec::push_le(&mut lengths, widen<u64>(slice_len(aad)));
        Vec::push_le(&mut lengths, widen<u64>(slice_len(ciphertext)));
        Poly1305::block(&mut p, &lengths[0..$], 1 << 24);
        Poly1305::finish(p)
    }

    // Encrypts and authenticates (RFC 8439 §2.8): the ciphertext of
    // `plaintext`, as long as it, followed by a 16-byte tag over it and over
    // `aad`, data sent in the clear but protected from change. `key` is 32
    // bytes and `nonce` 12; a nonce must never be used twice with one key.
    export fn chacha20_poly1305_seal(
        slice<u8, shared> key,
        slice<u8, shared> nonce,
        slice<u8, shared> aad,
        slice<u8, shared> plaintext,
    ) : Vec<u8>
    {
        Vec<u8> out = chacha20(key, nonce, 1, plaintext);
        array<u8, 16> tag = aead_tag(key, nonce, aad, &out[0..$]);
        for (usize i = 0; i < 16; i += 1)
        {
            Vec::push(&mut out, tag[i]);
        }
        out
    }

    // Checks and decrypts what `chacha20_poly1305_seal` gave: the plaintext
    // when the tag matches `sealed` and `aad` under `key` and `nonce`, and
    // `None` when anything was changed (or `sealed` is shorter than a tag).
    // Nothing of a message that fails is returned. Lengths as for `seal`.
    export fn chacha20_poly1305_open(
        slice<u8, shared> key,
        slice<u8, shared> nonce,
        slice<u8, shared> aad,
        slice<u8, shared> sealed,
    ) : Option<Vec<u8>>
    {
        crypto_check_lengths(key, nonce);
        usize n = slice_len(sealed);
        if (n < 16)
        {
            return None;
        }
        array<u8, 16> want = aead_tag(key, nonce, aad, &sealed[0..n - 16]);
        if (!digest_eq(&want[0..$], &sealed[n - 16..n]))
        {
            return None;
        }
        Some(chacha20(key, nonce, 1, &sealed[0..n - 16]))
    }

    // X25519 (RFC 7748, D-0154): Diffie-Hellman over Curve25519. A field
    // element is five 51-bit limbs of a number below 2^255 - 19, multiplied
    // through `u128`; the Montgomery ladder swaps by mask, so no branch and
    // no array index depends on a scalar bit, and every call takes the same
    // steps.

    const u64 FE_MASK = 0x7ffffffffffff;

    // Carries each limb into the next, the top one round to the bottom times
    // 19 (2^255 = 19 mod p): every limb ends below 2^52.
    fn fe_carry(array<u64, 5> f) : array<u64, 5>
    {
        array<u64, 5> r = f;
        for (usize i = 0; i < 4; i += 1)
        {
            r[i + 1] += r[i] >> 51;
            r[i] = r[i] & FE_MASK;
        }
        u64 c = r[4] >> 51;
        r[4] = r[4] & FE_MASK;
        r[0] += c * 19;
        r[1] += r[0] >> 51;
        r[0] = r[0] & FE_MASK;
        r
    }

    fn fe_add(array<u64, 5> a, array<u64, 5> b) : array<u64, 5>
    {
        fe_carry([a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3], a[4] + b[4]])
    }

    // a - b, as a + 4p - b so no limb goes below zero (limbs below 2^52).
    fn fe_sub(array<u64, 5> a, array<u64, 5> b) : array<u64, 5>
    {
        fe_carry([
            a[0] + 0x1fffffffffffb4 - b[0],
            a[1] + 0x1ffffffffffffc - b[1],
            a[2] + 0x1ffffffffffffc - b[2],
            a[3] + 0x1ffffffffffffc - b[3],
            a[4] + 0x1ffffffffffffc - b[4],
        ])
    }

    fn fe_mul(array<u64, 5> a, array<u64, 5> b) : array<u64, 5>
    {
        u128 a0 = widen<u128>(a[0]);
        u128 a1 = widen<u128>(a[1]);
        u128 a2 = widen<u128>(a[2]);
        u128 a3 = widen<u128>(a[3]);
        u128 a4 = widen<u128>(a[4]);
        u128 b0 = widen<u128>(b[0]);
        u128 b1 = widen<u128>(b[1]);
        u128 b2 = widen<u128>(b[2]);
        u128 b3 = widen<u128>(b[3]);
        u128 b4 = widen<u128>(b[4]);
        u128 b1_19 = b1 * 19;
        u128 b2_19 = b2 * 19;
        u128 b3_19 = b3 * 19;
        u128 b4_19 = b4 * 19;
        u128 t0 = a0 * b0 + a1 * b4_19 + a2 * b3_19 + a3 * b2_19 + a4 * b1_19;
        u128 t1 = a0 * b1 + a1 * b0 + a2 * b4_19 + a3 * b3_19 + a4 * b2_19;
        u128 t2 = a0 * b2 + a1 * b1 + a2 * b0 + a3 * b4_19 + a4 * b3_19;
        u128 t3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0 + a4 * b4_19;
        u128 t4 = a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;
        u128 m = 0x7ffffffffffff;
        t1 += t0 >> 51;
        t2 += t1 >> 51;
        t3 += t2 >> 51;
        t4 += t3 >> 51;
        u128 r0 = (t0 & m) + (t4 >> 51) * 19;
        u128 r1 = (t1 & m) + (r0 >> 51);
        array<u64, 5> r = [
            narrow<u64>(r0 & m),
            narrow<u64>(r1),
            narrow<u64>(t2 & m),
            narrow<u64>(t3 & m),
            narrow<u64>(t4 & m),
        ];
        fe_carry(r)
    }

    // Exchanges a and b when `swap` is 1, leaves them when it is 0, by mask.
    fn fe_cswap(ref<array<u64, 5>, exclusive> a, ref<array<u64, 5>, exclusive> b, u64 swap)
    {
        u64 mask = wrapping_sub(0, swap);
        for (usize i = 0; i < 5; i += 1)
        {
            u64 x = mask & (a[i] ^ b[i]);
            a[i] = a[i] ^ x;
            b[i] = b[i] ^ x;
        }
    }

    // z^(p - 2) = 1/z mod p (Fermat); the exponent 2^255 - 21 is public.
    fn fe_invert(array<u64, 5> z) : array<u64, 5>
    {
        array<u64, 5> r = [1, 0, 0, 0, 0];
        for (usize k = 0; k < 255; k += 1)
        {
            usize i = 254 - k;
            r = fe_mul(r, r);
            if (i >= 5 || i == 3 || i == 1 || i == 0)
            {
                r = fe_mul(r, z);
            }
        }
        r
    }

    // 32 little-endian bytes as a field element, the top bit ignored (RFC 7748
    // §5); a value from p to 2^255 - 1 is taken mod p by the arithmetic.
    fn fe_from_bytes(slice<u8, shared> s) : array<u64, 5>
    {
        [
            read_le<u64>(s, 0) & FE_MASK,
            (read_le<u64>(s, 6) >> 3) & FE_MASK,
            (read_le<u64>(s, 12) >> 6) & FE_MASK,
            (read_le<u64>(s, 19) >> 1) & FE_MASK,
            (read_le<u64>(s, 24) >> 12) & FE_MASK,
        ]
    }

    // The element's value below p, as 32 little-endian bytes.
    fn fe_to_bytes(array<u64, 5> f) : array<u8, 32>
    {
        array<u64, 5> h = fe_carry(fe_carry(f));
        // q is 1 exactly when h ≥ p: h + 19 then reaches 2^255.
        u64 q = (h[0] + 19) >> 51;
        q = (h[1] + q) >> 51;
        q = (h[2] + q) >> 51;
        q = (h[3] + q) >> 51;
        q = (h[4] + q) >> 51;
        h[0] += 19 * q;
        for (usize i = 0; i < 4; i += 1)
        {
            h[i + 1] += h[i] >> 51;
            h[i] = h[i] & FE_MASK;
        }
        h[4] = h[4] & FE_MASK;
        array<u64, 4> w = [
            h[0] | (h[1] << 51),
            (h[1] >> 13) | (h[2] << 38),
            (h[2] >> 26) | (h[3] << 25),
            (h[3] >> 39) | (h[4] << 12),
        ];
        array<u8, 32> out = [0; 32];
        for (usize i = 0; i < 4; i += 1)
        {
            for (usize j = 0; j < 8; j += 1)
            {
                out[8 * i + j] = narrow_wrapping<u8>(w[i] >> narrow<u32>(8 * j));
            }
        }
        out
    }

    // The Montgomery ladder (RFC 7748 §5): the clamped `scalar` times the
    // point with u-coordinate `u`.
    fn x25519_ladder(slice<u8, shared> scalar, slice<u8, shared> u) : array<u8, 32>
    {
        array<u8, 32> k = [0; 32];
        for (usize i = 0; i < 32; i += 1)
        {
            k[i] = scalar[i];
        }
        k[0] = k[0] & 248;
        k[31] = (k[31] & 127) | 64;
        array<u64, 5> x1 = fe_from_bytes(u);
        array<u64, 5> x2 = [1, 0, 0, 0, 0];
        array<u64, 5> z2 = [0, 0, 0, 0, 0];
        array<u64, 5> x3 = x1;
        array<u64, 5> z3 = [1, 0, 0, 0, 0];
        array<u64, 5> a24 = [121665, 0, 0, 0, 0];
        u64 swap = 0;
        for (usize step = 0; step < 255; step += 1)
        {
            usize t = 254 - step;
            u64 bit = widen<u64>((k[t >> 3] >> narrow<u32>(t & 7)) & 1);
            swap = swap ^ bit;
            fe_cswap(&mut x2, &mut x3, swap);
            fe_cswap(&mut z2, &mut z3, swap);
            swap = bit;
            array<u64, 5> a = fe_add(x2, z2);
            array<u64, 5> aa = fe_mul(a, a);
            array<u64, 5> b = fe_sub(x2, z2);
            array<u64, 5> bb = fe_mul(b, b);
            array<u64, 5> e = fe_sub(aa, bb);
            array<u64, 5> c = fe_add(x3, z3);
            array<u64, 5> d = fe_sub(x3, z3);
            array<u64, 5> da = fe_mul(d, a);
            array<u64, 5> cb = fe_mul(c, b);
            array<u64, 5> sum = fe_add(da, cb);
            x3 = fe_mul(sum, sum);
            array<u64, 5> diff = fe_sub(da, cb);
            z3 = fe_mul(x1, fe_mul(diff, diff));
            x2 = fe_mul(aa, bb);
            z2 = fe_mul(e, fe_add(aa, fe_mul(a24, e)));
        }
        fe_cswap(&mut x2, &mut x3, swap);
        fe_cswap(&mut z2, &mut z3, swap);
        fe_to_bytes(fe_mul(x2, fe_invert(z2)))
    }

    // The shared secret of a private key and the other side's public key
    // (RFC 7748 §6.1), each 32 bytes (another length is `diag.crypto-length`):
    // `None` when the result is all zeros, which a public key of small order
    // gives and which must never be used as a secret (RFC 7748 §6.1; TLS 1.3
    // requires the check). The comparison with zero takes the same time
    // whatever the result.
    export fn x25519(slice<u8, shared> private_key, slice<u8, shared> public_key) : Option<array<u8, 32>>
    {
        if (slice_len(private_key) != 32 || slice_len(public_key) != 32)
        {
            fault(crypto_length);
        }
        array<u8, 32> shared = x25519_ladder(private_key, public_key);
        u8 acc = 0;
        for (usize i = 0; i < 32; i += 1)
        {
            acc = acc | shared[i];
        }
        if (acc == 0)
        {
            return None;
        }
        Some(shared)
    }

    // The public key of a 32-byte private key (another length is
    // `diag.crypto-length`): the private key times the base point, u = 9.
    export fn x25519_public_key(slice<u8, shared> private_key) : array<u8, 32>
    {
        if (slice_len(private_key) != 32)
        {
            fault(crypto_length);
        }
        array<u8, 32> base = [0; 32];
        base[0] = 9;
        x25519_ladder(private_key, &base[0..$])
    }

    // A new private key: 32 bytes from the operating system's secure source
    // (`os_random_bytes`, D-0142), never from `Rng`.
    export fn x25519_private_key() : array<u8, 32>
    {
        array<u8, 32> k = [0; 32];
        os_random_bytes(&mut k[0..$]);
        k
    }

    // SHA-512 and SHA-384 (FIPS 180-4, D-0156): the 64-bit-word hashes, SHA-384
    // being SHA-512 from other initial values, cut to 48 bytes. Certificates
    // signed with ECDSA P-384 or RSA-SHA-384, and TLS's SHA-384 cipher suites,
    // hash with them.

    const array<u64, 80> SHA512_K = [
        0x428a2f98d728ae22,
        0x7137449123ef65cd,
        0xb5c0fbcfec4d3b2f,
        0xe9b5dba58189dbbc,
        0x3956c25bf348b538,
        0x59f111f1b605d019,
        0x923f82a4af194f9b,
        0xab1c5ed5da6d8118,
        0xd807aa98a3030242,
        0x12835b0145706fbe,
        0x243185be4ee4b28c,
        0x550c7dc3d5ffb4e2,
        0x72be5d74f27b896f,
        0x80deb1fe3b1696b1,
        0x9bdc06a725c71235,
        0xc19bf174cf692694,
        0xe49b69c19ef14ad2,
        0xefbe4786384f25e3,
        0x0fc19dc68b8cd5b5,
        0x240ca1cc77ac9c65,
        0x2de92c6f592b0275,
        0x4a7484aa6ea6e483,
        0x5cb0a9dcbd41fbd4,
        0x76f988da831153b5,
        0x983e5152ee66dfab,
        0xa831c66d2db43210,
        0xb00327c898fb213f,
        0xbf597fc7beef0ee4,
        0xc6e00bf33da88fc2,
        0xd5a79147930aa725,
        0x06ca6351e003826f,
        0x142929670a0e6e70,
        0x27b70a8546d22ffc,
        0x2e1b21385c26c926,
        0x4d2c6dfc5ac42aed,
        0x53380d139d95b3df,
        0x650a73548baf63de,
        0x766a0abb3c77b2a8,
        0x81c2c92e47edaee6,
        0x92722c851482353b,
        0xa2bfe8a14cf10364,
        0xa81a664bbc423001,
        0xc24b8b70d0f89791,
        0xc76c51a30654be30,
        0xd192e819d6ef5218,
        0xd69906245565a910,
        0xf40e35855771202a,
        0x106aa07032bbd1b8,
        0x19a4c116b8d2d0c8,
        0x1e376c085141ab53,
        0x2748774cdf8eeb99,
        0x34b0bcb5e19b48a8,
        0x391c0cb3c5c95a63,
        0x4ed8aa4ae3418acb,
        0x5b9cca4f7763e373,
        0x682e6ff3d6b2b8a3,
        0x748f82ee5defb2fc,
        0x78a5636f43172f60,
        0x84c87814a1f0ab72,
        0x8cc702081a6439ec,
        0x90befffa23631e28,
        0xa4506cebde82bde9,
        0xbef9a3f7b2c67915,
        0xc67178f2e372532b,
        0xca273eceea26619c,
        0xd186b8c721c0c207,
        0xeada7dd6cde0eb1e,
        0xf57d4f7fee6ed178,
        0x06f067aa72176fba,
        0x0a637dc5a2c898a6,
        0x113f9804bef90dae,
        0x1b710b35131c471b,
        0x28db77f523047d84,
        0x32caab7b40c72493,
        0x3c9ebe0a15c9bebc,
        0x431d67c49c100d4c,
        0x4cc5d4becb3e42b6,
        0x597f299cfc657e2a,
        0x5fcb6fab3ad6faec,
        0x6c44198c4a475817,
    ];

    // A SHA-512 computation in progress, as `Sha256` is one: a plain value.
    export struct Sha512
    {
        array<u64, 8> h;
        array<u8, 128> block;
        usize fill;
        u64 length;
    }

    export fn Sha512::new() : Sha512
    {
        Sha512 { .h = [
            0x6a09e667f3bcc908,
            0xbb67ae8584caa73b,
            0x3c6ef372fe94f82b,
            0xa54ff53a5f1d36f1,
            0x510e527fade682d1,
            0x9b05688c2b3e6c1f,
            0x1f83d9abfb41bd6b,
            0x5be0cd19137e2179,
        ], .block = [0; 128], .fill = 0, .length = 0 }
    }

    // One 128-byte block folded into the state `h`, both values (as
    // `sha256_block`: nothing here is checked at run time).
    fn sha512_block(array<u64, 8> h, array<u8, 128> blk) : array<u64, 8>
    {
        array<u64, 80> w = [0; 80];
        for (usize i = 0; i < 16; i += 1)
        {
            u64 x = 0;
            for (usize j = 0; j < 8; j += 1)
            {
                x = (x << 8) | widen<u64>(blk[8 * i + j]);
            }
            w[i] = x;
        }
        for (usize i = 16; i < 80; i += 1)
        {
            u64 s0 = rotate_right(w[i - 15], 1) ^ rotate_right(w[i - 15], 8) ^ (w[i - 15] >> 7);
            u64 s1 = rotate_right(w[i - 2], 19) ^ rotate_right(w[i - 2], 61) ^ (w[i - 2] >> 6);
            w[i] = wrapping_add(wrapping_add(w[i - 16], s0), wrapping_add(w[i - 7], s1));
        }
        u64 a = h[0];
        u64 b = h[1];
        u64 c = h[2];
        u64 d = h[3];
        u64 e = h[4];
        u64 f = h[5];
        u64 g = h[6];
        u64 hh = h[7];
        for (usize i = 0; i < 80; i += 1)
        {
            u64 big_s1 = rotate_right(e, 14) ^ rotate_right(e, 18) ^ rotate_right(e, 41);
            u64 ch = (e & f) ^ (~e & g);
            u64 t1 = wrapping_add(wrapping_add(wrapping_add(hh, big_s1), wrapping_add(ch, SHA512_K[i])), w[i]);
            u64 big_s0 = rotate_right(a, 28) ^ rotate_right(a, 34) ^ rotate_right(a, 39);
            u64 maj = (a & b) ^ (a & c) ^ (b & c);
            u64 t2 = wrapping_add(big_s0, maj);
            hh = g;
            g = f;
            f = e;
            e = wrapping_add(d, t1);
            d = c;
            c = b;
            b = a;
            a = wrapping_add(t1, t2);
        }
        array<u64, 8> out = h;
        out[0] = wrapping_add(h[0], a);
        out[1] = wrapping_add(h[1], b);
        out[2] = wrapping_add(h[2], c);
        out[3] = wrapping_add(h[3], d);
        out[4] = wrapping_add(h[4], e);
        out[5] = wrapping_add(h[5], f);
        out[6] = wrapping_add(h[6], g);
        out[7] = wrapping_add(h[7], hh);
        out
    }

    // `data` added to a state given and returned by value (as `sha256_absorb`).
    fn sha512_absorb(Sha512 s, slice<u8, shared> data) : Sha512
    {
        usize n = slice_len(data);
        usize i = 0;
        while (i < n)
        {
            if (s.fill == 0 && n - i >= 128)
            {
                array<u8, 128> blk = [0; 128];
                for (usize k = 0; k < 128; k += 1)
                {
                    blk[k] = data[i + k];
                }
                s.h = sha512_block(s.h, blk);
                i += 128;
            }
            else
            {
                s.block[s.fill] = data[i];
                s.fill += 1;
                i += 1;
                if (s.fill == 128)
                {
                    s.h = sha512_block(s.h, s.block);
                    s.fill = 0;
                }
            }
        }
        s.length = wrapping_add(s.length, widen<u64>(n));
        s
    }

    export fn Sha512::update(ref<Sha512, exclusive> s, slice<u8, shared> data)
    {
        *s = sha512_absorb(*s, data);
    }

    // The final state words, after the padding and the 128-bit length.
    fn Sha512::final_words(Sha512 s) : array<u64, 8>
    {
        u64 bits = wrapping_mul(s.length, 8);
        array<u64, 8> h = s.h;
        array<u8, 128> last = s.block;
        usize f = s.fill;
        last[f] = 0x80;
        for (usize i = f + 1; i < 128; i += 1)
        {
            last[i] = 0;
        }
        if (f >= 112)
        {
            h = sha512_block(h, last);
            last = [0; 128];
        }
        for (usize i = 0; i < 8; i += 1)
        {
            last[112 + i] = narrow_wrapping<u8>(s.length >> 61 >> narrow<u32>(56 - 8 * i));
            last[120 + i] = narrow_wrapping<u8>(bits >> narrow<u32>(56 - 8 * i));
        }
        sha512_block(h, last)
    }

    fn sha512_bytes(array<u64, 8> h, usize words) : Vec<u8>
    {
        Vec<u8> out = Vec::new();
        for (usize i = 0; i < words; i += 1)
        {
            Vec::push_be(&mut out, h[i]);
        }
        out
    }

    // The 64-byte digest of everything given.
    export fn Sha512::finish(Sha512 s) : array<u8, 64>
    {
        Vec<u8> b = sha512_bytes(Sha512::final_words(s), 8);
        array<u8, 64> out = [0; 64];
        for (usize i = 0; i < 64; i += 1)
        {
            out[i] = b[i];
        }
        out
    }

    export fn sha512(slice<u8, shared> data) : array<u8, 64>
    {
        Sha512 s = Sha512::new();
        Sha512::update(&mut s, data);
        Sha512::finish(s)
    }

    // A SHA-384 computation in progress.
    export struct Sha384
    {
        Sha512 inner;
    }

    export fn Sha384::new() : Sha384
    {
        Sha384 { .inner = Sha512 { .h = [
            0xcbbb9d5dc1059ed8,
            0x629a292a367cd507,
            0x9159015a3070dd17,
            0x152fecd8f70e5939,
            0x67332667ffc00b31,
            0x8eb44a8768581511,
            0xdb0c2e0d64f98fa7,
            0x47b5481dbefa4fa4,
        ], .block = [0; 128], .fill = 0, .length = 0 } }
    }

    export fn Sha384::update(ref<Sha384, exclusive> s, slice<u8, shared> data)
    {
        Sha512::update(&mut s.inner, data);
    }

    // The 48-byte digest of everything given.
    export fn Sha384::finish(Sha384 s) : array<u8, 48>
    {
        Vec<u8> b = sha512_bytes(Sha512::final_words(s.inner), 6);
        array<u8, 48> out = [0; 48];
        for (usize i = 0; i < 48; i += 1)
        {
            out[i] = b[i];
        }
        out
    }

    export fn sha384(slice<u8, shared> data) : array<u8, 48>
    {
        Sha384 s = Sha384::new();
        Sha384::update(&mut s, data);
        Sha384::finish(s)
    }

    // ---- SHA-1 (D-0181) ----
    //
    // FIPS 180-4's SHA-1, as `Sha256` is written: a plain value. Legacy: it
    // is here for the protocols that still require it (RFC 6238's default
    // HMAC, git), not for new signatures (collisions have been made).
    export struct Sha1
    {
        array<u32, 5> h;
        array<u8, 64> block;
        usize fill;
        u64 length;
    }

    export fn Sha1::new() : Sha1
    {
        Sha1 { .h = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0], .block = [0; 64], .fill = 0, .length = 0 }
    }

    // One 64-byte block folded into the state `h`, both values (as
    // `sha256_block`).
    fn sha1_block(array<u32, 5> h, array<u8, 64> blk) : array<u32, 5>
    {
        array<u32, 80> w = [0; 80];
        for (usize i = 0; i < 16; i += 1)
        {
            w[i] = (widen<u32>(blk[4 * i]) << 24) | (widen<u32>(blk[4 * i + 1]) << 16) | (widen<u32>(blk[4 * i + 2]) << 8) | widen<u32>(blk[4 * i + 3]);
        }
        for (usize i = 16; i < 80; i += 1)
        {
            w[i] = rotate_left(w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16], 1);
        }
        u32 a = h[0];
        u32 b = h[1];
        u32 c = h[2];
        u32 d = h[3];
        u32 e = h[4];
        for (usize i = 0; i < 80; i += 1)
        {
            u32 f = 0;
            u32 k = 0;
            if (i < 20)
            {
                f = (b & c) | (~b & d);
                k = 0x5A827999;
            }
            else if (i < 40)
            {
                f = b ^ c ^ d;
                k = 0x6ED9EBA1;
            }
            else if (i < 60)
            {
                f = (b & c) | (b & d) | (c & d);
                k = 0x8F1BBCDC;
            }
            else
            {
                f = b ^ c ^ d;
                k = 0xCA62C1D6;
            }
            u32 t = wrapping_add(wrapping_add(rotate_left(a, 5), f), wrapping_add(wrapping_add(e, k), w[i]));
            e = d;
            d = c;
            c = rotate_left(b, 30);
            b = a;
            a = t;
        }
        array<u32, 5> out = h;
        out[0] = wrapping_add(h[0], a);
        out[1] = wrapping_add(h[1], b);
        out[2] = wrapping_add(h[2], c);
        out[3] = wrapping_add(h[3], d);
        out[4] = wrapping_add(h[4], e);
        out
    }

    // `data` added to a state given and returned by value (as `sha256_absorb`).
    fn sha1_absorb(Sha1 s, slice<u8, shared> data) : Sha1
    {
        usize n = slice_len(data);
        usize i = 0;
        while (i < n)
        {
            if (s.fill == 0 && n - i >= 64)
            {
                array<u8, 64> blk = [0; 64];
                for (usize k = 0; k < 64; k += 1)
                {
                    blk[k] = data[i + k];
                }
                s.h = sha1_block(s.h, blk);
                i += 64;
            }
            else
            {
                s.block[s.fill] = data[i];
                s.fill += 1;
                i += 1;
                if (s.fill == 64)
                {
                    s.h = sha1_block(s.h, s.block);
                    s.fill = 0;
                }
            }
        }
        s.length = wrapping_add(s.length, widen<u64>(n));
        s
    }

    export fn Sha1::update(ref<Sha1, exclusive> s, slice<u8, shared> data)
    {
        *s = sha1_absorb(*s, data);
    }

    // The 20-byte digest of everything given.
    export fn Sha1::finish(Sha1 s) : array<u8, 20>
    {
        u64 bits = wrapping_mul(s.length, 8);
        array<u32, 5> h = s.h;
        array<u8, 64> last = s.block;
        usize f = s.fill;
        last[f] = 0x80;
        for (usize i = f + 1; i < 64; i += 1)
        {
            last[i] = 0;
        }
        if (f >= 56)
        {
            h = sha1_block(h, last);
            last = [0; 64];
        }
        for (usize i = 0; i < 8; i += 1)
        {
            last[56 + i] = narrow_wrapping<u8>(bits >> narrow<u32>(56 - 8 * i));
        }
        h = sha1_block(h, last);
        array<u8, 20> out = [0; 20];
        for (usize i = 0; i < 5; i += 1)
        {
            out[4 * i] = narrow_wrapping<u8>(h[i] >> 24);
            out[4 * i + 1] = narrow_wrapping<u8>(h[i] >> 16);
            out[4 * i + 2] = narrow_wrapping<u8>(h[i] >> 8);
            out[4 * i + 3] = narrow_wrapping<u8>(h[i]);
        }
        out
    }

    // The SHA-1 digest of `data` in one call; `sha1("abc")` is
    // a9993e364706816aba3e25717850c26c9cd0d89d.
    export fn sha1(slice<u8, shared> data) : array<u8, 20>
    {
        Sha1 s = Sha1::new();
        Sha1::update(&mut s, data);
        Sha1::finish(s)
    }

    // ---- public-key signature verification (D-0158, D-0159) ----
    //
    // Checking a signature uses only public values (the key, the message, the
    // signature), so these use `BigUint` (std::math) and need not take the
    // same time for every input. Nothing here signs: a client verifies.

    // The hash a signature was made over. `Sha1` (D-0181) is legacy: for
    // what older protocols still require (HMAC-SHA-1 in RFC 6238 TOTP, git's
    // object names, old signatures to check), never for a new signature or
    // anything whose collision resistance matters.
    export enum HashKind
    {
        Sha1,
        Sha2_256,
        Sha2_384,
        Sha2_512,
    }

    // The digest of `data` by `kind`: 20, 32, 48 or 64 bytes.
    export fn hash(HashKind kind, slice<u8, shared> data) : Vec<u8>
    {
        match (kind)
        {
            Sha1     : Vec::from_slice(&sha1(data)[0..$]),
            Sha2_256 : Vec::from_slice(&sha256(data)[0..$]),
            Sha2_384 : Vec::from_slice(&sha384(data)[0..$]),
            Sha2_512 : Vec::from_slice(&sha512(data)[0..$]),
        }
    }

    fn hash_len(HashKind kind) : usize
    {
        match (kind)
        {
            Sha1     : 20,
            Sha2_256 : 32,
            Sha2_384 : 48,
            Sha2_512 : 64,
        }
    }

    // An RSA public key: the modulus and the public exponent.
    export struct RsaPublicKey
    {
        export BigUint n;
        export BigUint e;
    }

    // s^e mod n as exactly `k` bytes, or None when the signature is not a
    // number below n of n's byte length.
    fn rsa_open(ref<RsaPublicKey, shared> key, slice<u8, shared> signature, usize k) : Option<Vec<u8>>
    {
        BigUint s = BigUint::from_bytes_be(signature);
        if (!BigUint::less(&s, &key.n))
        {
            return None;
        }
        BigUint m = BigUint::mod_pow(&s, &key.e, &key.n);
        if (BigUint::bit_len(&m) > 8 * k)
        {
            return None;
        }
        Some(BigUint::to_bytes_be_padded(&m, k))
    }

    // The DER of the DigestInfo that precedes a digest in a PKCS #1 v1.5
    // signature (RFC 8017 §9.2, note 1).
    fn digest_info_prefix(HashKind kind) : Vec<u8>
    {
        Result::unwrap_or(from_hex(match (kind)
        {
            Sha1     : "3021300906052b0e03021a05000414",
            Sha2_256 : "3031300d060960864801650304020105000420",
            Sha2_384 : "3041300d060960864801650304020205000430",
            Sha2_512 : "3051300d060960864801650304020305000440",
        }), Vec::new())
    }

    // Whether `signature` is `key`'s RSASSA-PKCS1-v1_5 signature (RFC 8017
    // §8.2.2) of a message whose `kind` digest is `digest`. The encoded block
    // is rebuilt and compared whole, so no part of it is parsed.
    export fn rsa_verify_pkcs1v15(
        ref<RsaPublicKey, shared> key,
        HashKind kind,
        slice<u8, shared> digest,
        slice<u8, shared> signature,
    ) : bool
    {
        usize k = (BigUint::bit_len(&key.n) + 7) / 8;
        if (slice_len(signature) != k || slice_len(digest) != hash_len(kind))
        {
            return false;
        }
        Vec<u8> prefix = digest_info_prefix(kind);
        usize t_len = Vec::len(&prefix) + slice_len(digest);
        if (k < t_len + 11)
        {
            return false;
        }
        Vec<u8> want = Vec::new();
        Vec::push(&mut want, 0);
        Vec::push(&mut want, 1);
        for (usize i = 0; i < k - t_len - 3; i += 1)
        {
            Vec::push(&mut want, 0xff);
        }
        Vec::push(&mut want, 0);
        Vec::extend_from(&mut want, &prefix[0..$]);
        Vec::extend_from(&mut want, digest);
        match (rsa_open(key, signature, k))
        {
            Some(em) : digest_eq(&em[0..$], &want[0..$]),
            None     : false,
        }
    }

    // MGF1 (RFC 8017 §B.2.1): `len` bytes from hashing `seed` with a counter.
    fn mgf1(HashKind kind, slice<u8, shared> seed, usize len) : Vec<u8>
    {
        Vec<u8> out = Vec::new();
        u32 counter = 0;
        while (Vec::len(&out) < len)
        {
            Vec<u8> input = Vec::from_slice(seed);
            Vec::push_be(&mut input, counter);
            Vec<u8> h = hash(kind, &input[0..$]);
            for (usize i = 0; i < Vec::len(&h) && Vec::len(&out) < len; i += 1)
            {
                Vec::push(&mut out, h[i]);
            }
            counter += 1;
        }
        out
    }

    // Whether `signature` is `key`'s RSASSA-PSS signature (RFC 8017 §8.1.2)
    // of a message whose `kind` digest is `digest`, with MGF1 over the same
    // hash and a salt as long as the digest, as TLS 1.3 and most X.509 use.
    export fn rsa_verify_pss(
        ref<RsaPublicKey, shared> key,
        HashKind kind,
        slice<u8, shared> digest,
        slice<u8, shared> signature,
    ) : bool
    {
        usize mod_bits = BigUint::bit_len(&key.n);
        usize k = (mod_bits + 7) / 8;
        usize h_len = hash_len(kind);
        usize s_len = h_len;
        if (slice_len(signature) != k || slice_len(digest) != h_len || mod_bits < 2)
        {
            return false;
        }
        usize em_bits = mod_bits - 1;
        usize em_len = (em_bits + 7) / 8;
        if (em_len < h_len + s_len + 2)
        {
            return false;
        }
        Vec<u8> em = match (rsa_open(key, signature, em_len))
        {
            Some(e) : e,
            None    : return false,
        };
        if (em[em_len - 1] != 0xbc)
        {
            return false;
        }
        usize db_len = em_len - h_len - 1;
        u32 unused = narrow<u32>(8 * em_len - em_bits);
        u8 top_mask = narrow_wrapping<u8>((0xff: u32) >> unused);
        if ((em[0] & ~top_mask) != 0)
        {
            return false;
        }
        Vec<u8> db_mask = mgf1(kind, &em[db_len..db_len + h_len], db_len);
        Vec<u8> db = Vec::new();
        for (usize i = 0; i < db_len; i += 1)
        {
            Vec::push(&mut db, em[i] ^ db_mask[i]);
        }
        db[0] = db[0] & top_mask;
        usize ps_len = em_len - h_len - s_len - 2;
        for (usize i = 0; i < ps_len; i += 1)
        {
            if (db[i] != 0)
            {
                return false;
            }
        }
        if (db[ps_len] != 1)
        {
            return false;
        }
        Vec<u8> m2 = Vec::filled(8, 0: u8);
        Vec::extend_from(&mut m2, digest);
        Vec::extend_from(&mut m2, &db[db_len - s_len..$]);
        Vec<u8> h2 = hash(kind, &m2[0..$]);
        digest_eq(&h2[0..$], &em[db_len..db_len + h_len])
    }

    // The elliptic curves ECDSA signatures are checked on.
    export enum EcCurve
    {
        P256,
        P384,
    }

    // Arithmetic modulo a curve's prime or its group order, in Montgomery
    // form: up to six 64-bit limbs, least significant first; `n` of them used;
    // `minv` = -m⁻¹ mod 2^64; `r2` = R² mod m, R = 2^(64·n). An element is a
    // plain array, so it is copied, not owned.
    struct Field
    {
        array<u64, 6> m;
        usize n;
        u64 minv;
        array<u64, 6> r2;
    }

    fn field_of(EcCurve c, bool order) : Field
    {
        match (c)
        {
            P256 : if (order)
            {
                Field { .m = [
                    0xf3b9cac2fc632551,
                    0xbce6faada7179e84,
                    0xffffffffffffffff,
                    0xffffffff00000000,
                    0x0000000000000000,
                    0x0000000000000000,
                ], .n = 4, .minv = 0xccd1c8aaee00bc4f, .r2 = [
                    0x83244c95be79eea2,
                    0x4699799c49bd6fa6,
                    0x2845b2392b6bec59,
                    0x66e12d94f3d95620,
                    0x0000000000000000,
                    0x0000000000000000,
                ] }
            }
            else
            {
                Field { .m = [
                    0xffffffffffffffff,
                    0x00000000ffffffff,
                    0x0000000000000000,
                    0xffffffff00000001,
                    0x0000000000000000,
                    0x0000000000000000,
                ], .n = 4, .minv = 0x0000000000000001, .r2 = [
                    0x0000000000000003,
                    0xfffffffbffffffff,
                    0xfffffffffffffffe,
                    0x00000004fffffffd,
                    0x0000000000000000,
                    0x0000000000000000,
                ] }
            },
            P384 : if (order)
            {
                Field { .m = [
                    0xecec196accc52973,
                    0x581a0db248b0a77a,
                    0xc7634d81f4372ddf,
                    0xffffffffffffffff,
                    0xffffffffffffffff,
                    0xffffffffffffffff,
                ], .n = 6, .minv = 0x6ed46089e88fdc45, .r2 = [
                    0x2d319b2419b409a9,
                    0xff3d81e5df1aa419,
                    0xbc3e483afcb82947,
                    0xd40d49174aab1cc5,
                    0x3fb05b7a28266895,
                    0x0c84ee012b39bf21,
                ] }
            }
            else
            {
                Field { .m = [
                    0x00000000ffffffff,
                    0xffffffff00000000,
                    0xfffffffffffffffe,
                    0xffffffffffffffff,
                    0xffffffffffffffff,
                    0xffffffffffffffff,
                ], .n = 6, .minv = 0x0000000100000001, .r2 = [
                    0xfffffffe00000001,
                    0x0000000200000000,
                    0xfffffffe00000000,
                    0x0000000200000000,
                    0x0000000000000001,
                    0x0000000000000000,
                ] }
            },
        }
    }

    // a·b·R⁻¹ mod m (coarsely integrated operand scanning).
    fn mont_mul(ref<Field, shared> f, array<u64, 6> a, array<u64, 6> b) : array<u64, 6>
    {
        usize n = f.n;
        array<u64, 8> t = [0; 8];
        for (usize i = 0; i < n; i += 1)
        {
            u128 c = 0;
            for (usize j = 0; j < n; j += 1)
            {
                u128 x = widen<u128>(t[j]) + widen<u128>(a[j]) * widen<u128>(b[i]) + c;
                t[j] = narrow_wrapping<u64>(x);
                c = x >> 64;
            }
            u128 y = widen<u128>(t[n]) + c;
            t[n] = narrow_wrapping<u64>(y);
            t[n + 1] = narrow_wrapping<u64>(y >> 64);
            u64 k = wrapping_mul(t[0], f.minv);
            u128 z = widen<u128>(t[0]) + widen<u128>(k) * widen<u128>(f.m[0]);
            c = z >> 64;
            for (usize j = 1; j < n; j += 1)
            {
                u128 x = widen<u128>(t[j]) + widen<u128>(k) * widen<u128>(f.m[j]) + c;
                t[j - 1] = narrow_wrapping<u64>(x);
                c = x >> 64;
            }
            u128 w = widen<u128>(t[n]) + c;
            t[n - 1] = narrow_wrapping<u64>(w);
            t[n] = t[n + 1] + narrow_wrapping<u64>(w >> 64);
        }
        array<u64, 6> r = [0; 6];
        for (usize j = 0; j < n; j += 1)
        {
            r[j] = t[j];
        }
        // r - m unless r (with its carry t[n]) is below m: chosen by a mask.
        Borrowed d = f_sub_borrow(f, r, f.m);
        f_select(wrapping_sub(0, (1 - t[n]) & d.borrow), r, d.v)
    }

    fn f_less(ref<Field, shared> f, array<u64, 6> a, array<u64, 6> b) : bool
    {
        usize i = f.n;
        while (i > 0)
        {
            i -= 1;
            if (a[i] != b[i])
            {
                return a[i] < b[i];
            }
        }
        false
    }

    // A difference and whether it borrowed (1) past the top limb.
    struct Borrowed
    {
        array<u64, 6> v;
        u64 borrow;
    }

    // a - b, wrapping at 2^(64·n), and the borrow out; no branch on the limbs.
    fn f_sub_borrow(ref<Field, shared> f, array<u64, 6> a, array<u64, 6> b) : Borrowed
    {
        array<u64, 6> r = [0; 6];
        u64 borrow = 0;
        for (usize i = 0; i < f.n; i += 1)
        {
            u128 x = wrapping_sub(wrapping_sub(widen<u128>(a[i]), widen<u128>(b[i])), widen<u128>(borrow));
            r[i] = narrow_wrapping<u64>(x);
            borrow = narrow_wrapping<u64>(x >> 127);
        }
        Borrowed { .v = r, .borrow = borrow }
    }

    // a - b, wrapping at 2^(64·n).
    fn f_sub_raw(ref<Field, shared> f, array<u64, 6> a, array<u64, 6> b) : array<u64, 6>
    {
        f_sub_borrow(f, a, b).v
    }

    // a where mask is all ones, b where it is zero, limb by limb.
    fn f_select(u64 mask, array<u64, 6> a, array<u64, 6> b) : array<u64, 6>
    {
        array<u64, 6> r = [0; 6];
        for (usize i = 0; i < 6; i += 1)
        {
            r[i] = (a[i] & mask) | (b[i] & ~mask);
        }
        r
    }

    // The field operations take the same steps whatever the limbs hold: the
    // reduction is chosen by a mask (D-0165), so a secret scalar's arithmetic
    // (P-256 key agreement) shows nothing through its timing.
    fn f_add(ref<Field, shared> f, array<u64, 6> a, array<u64, 6> b) : array<u64, 6>
    {
        array<u64, 6> r = [0; 6];
        u64 carry = 0;
        for (usize i = 0; i < f.n; i += 1)
        {
            u128 s = widen<u128>(a[i]) + widen<u128>(b[i]) + widen<u128>(carry);
            r[i] = narrow_wrapping<u64>(s);
            carry = narrow_wrapping<u64>(s >> 64);
        }
        Borrowed d = f_sub_borrow(f, r, f.m);
        f_select(wrapping_sub(0, (1 - carry) & d.borrow), r, d.v)
    }

    fn f_sub(ref<Field, shared> f, array<u64, 6> a, array<u64, 6> b) : array<u64, 6>
    {
        Borrowed d = f_sub_borrow(f, a, b);
        array<u64, 6> m = f_select(wrapping_sub(0, d.borrow), f.m, [0; 6]);
        f_add_raw(f, d.v, m)
    }

    // a + b, wrapping at 2^(64·n).
    fn f_add_raw(ref<Field, shared> f, array<u64, 6> a, array<u64, 6> b) : array<u64, 6>
    {
        array<u64, 6> r = [0; 6];
        u64 carry = 0;
        for (usize i = 0; i < f.n; i += 1)
        {
            u128 s = widen<u128>(a[i]) + widen<u128>(b[i]) + widen<u128>(carry);
            r[i] = narrow_wrapping<u64>(s);
            carry = narrow_wrapping<u64>(s >> 64);
        }
        r
    }

    fn f_is_zero(ref<Field, shared> f, array<u64, 6> a) : bool
    {
        u64 acc = 0;
        for (usize i = 0; i < f.n; i += 1)
        {
            acc = acc | a[i];
        }
        acc == 0
    }

    fn f_to_mont(ref<Field, shared> f, array<u64, 6> a) : array<u64, 6>
    {
        mont_mul(f, a, f.r2)
    }

    fn f_from_mont(ref<Field, shared> f, array<u64, 6> a) : array<u64, 6>
    {
        mont_mul(f, a, [1, 0, 0, 0, 0, 0])
    }

    // a⁻¹ (Fermat: a^(m-2)), a and the result in Montgomery form.
    fn f_invert(ref<Field, shared> f, array<u64, 6> a) : array<u64, 6>
    {
        array<u64, 6> e = f_sub_raw(f, f.m, [2, 0, 0, 0, 0, 0]);
        array<u64, 6> r = f_to_mont(f, [1, 0, 0, 0, 0, 0]);
        usize bits = 64 * f.n;
        usize i = bits;
        while (i > 0)
        {
            i -= 1;
            r = mont_mul(f, r, r);
            if (((e[i / 64] >> narrow<u32>(i % 64)) & 1) == 1)
            {
                r = mont_mul(f, r, a);
            }
        }
        r
    }

    // Big-endian bytes (at most 8·n of them) as limbs; None when they do not
    // name a number below m.
    fn f_from_be(ref<Field, shared> f, slice<u8, shared> b) : Option<array<u64, 6>>
    {
        usize len = slice_len(b);
        if (len > 8 * f.n)
        {
            return None;
        }
        array<u64, 6> r = [0; 6];
        for (usize k = 0; k < len; k += 1)
        {
            usize pos = len - 1 - k;
            r[k / 8] = r[k / 8] | (widen<u64>(b[pos]) << narrow<u32>(8 * (k % 8)));
        }
        if (!f_less(f, r, f.m))
        {
            return None;
        }
        Some(r)
    }

    fn f_bit(array<u64, 6> a, usize i) : bool
    {
        ((a[i / 64] >> narrow<u32>(i % 64)) & 1) == 1
    }

    // k·a for a small public constant k (2, 3, 4, 8): doublings and one
    // addition per set bit below the top, not k additions.
    fn f_small(ref<Field, shared> f, u64 k, array<u64, 6> a) : array<u64, 6>
    {
        array<u64, 6> r = a;
        u32 top = 63 - leading_zeros(k);
        u32 i = top;
        while (i > 0)
        {
            i -= 1;
            r = f_add(f, r, r);
            if (((k >> i) & 1) == 1)
            {
                r = f_add(f, r, a);
            }
        }
        r
    }

    // A point in Jacobian coordinates (X/Z², Y/Z³), Montgomery form; Z = 0 is
    // the point at infinity.
    struct Jacobian
    {
        array<u64, 6> x;
        array<u64, 6> y;
        array<u64, 6> z;
    }

    fn jac_double(ref<Field, shared> f, Jacobian q) : Jacobian
    {
        if (f_is_zero(f, q.z) || f_is_zero(f, q.y))
        {
            return Jacobian { .x = q.x, .y = q.y, .z = [0; 6] };
        }
        array<u64, 6> delta = mont_mul(f, q.z, q.z);
        array<u64, 6> gamma = mont_mul(f, q.y, q.y);
        array<u64, 6> beta = mont_mul(f, q.x, gamma);
        array<u64, 6> alpha = f_small(f, 3, mont_mul(f, f_sub(f, q.x, delta), f_add(f, q.x, delta)));
        array<u64, 6> x3 = f_sub(f, mont_mul(f, alpha, alpha), f_small(f, 8, beta));
        array<u64, 6> yz = f_add(f, q.y, q.z);
        array<u64, 6> z3 = f_sub(f, f_sub(f, mont_mul(f, yz, yz), gamma), delta);
        array<u64, 6> y3 = f_sub(
            f,
            mont_mul(f, alpha, f_sub(f, f_small(f, 4, beta), x3)),
            f_small(f, 8, mont_mul(f, gamma, gamma)),
        );
        Jacobian { .x = x3, .y = y3, .z = z3 }
    }

    fn jac_add(ref<Field, shared> f, Jacobian a, Jacobian b) : Jacobian
    {
        if (f_is_zero(f, a.z))
        {
            return b;
        }
        if (f_is_zero(f, b.z))
        {
            return a;
        }
        array<u64, 6> z1z1 = mont_mul(f, a.z, a.z);
        array<u64, 6> z2z2 = mont_mul(f, b.z, b.z);
        array<u64, 6> u1 = mont_mul(f, a.x, z2z2);
        array<u64, 6> u2 = mont_mul(f, b.x, z1z1);
        array<u64, 6> s1 = mont_mul(f, mont_mul(f, a.y, b.z), z2z2);
        array<u64, 6> s2 = mont_mul(f, mont_mul(f, b.y, a.z), z1z1);
        array<u64, 6> h = f_sub(f, u2, u1);
        array<u64, 6> r = f_small(f, 2, f_sub(f, s2, s1));
        if (f_is_zero(f, h))
        {
            if (f_is_zero(f, r))
            {
                return jac_double(f, a);
            }
            return Jacobian { .x = a.x, .y = a.y, .z = [0; 6] };
        }
        array<u64, 6> h2 = f_small(f, 2, h);
        array<u64, 6> i = mont_mul(f, h2, h2);
        array<u64, 6> j = mont_mul(f, h, i);
        array<u64, 6> v = mont_mul(f, u1, i);
        array<u64, 6> x3 = f_sub(f, f_sub(f, mont_mul(f, r, r), j), f_small(f, 2, v));
        array<u64, 6> y3 = f_sub(f, mont_mul(f, r, f_sub(f, v, x3)), f_small(f, 2, mont_mul(f, s1, j)));
        array<u64, 6> zs = f_add(f, a.z, b.z);
        array<u64, 6> z3 = mont_mul(f, f_sub(f, f_sub(f, mont_mul(f, zs, zs), z1z1), z2z2), h);
        Jacobian { .x = x3, .y = y3, .z = z3 }
    }

    // 0·P to 15·P, P's multiples for a 4-bit window.
    fn jac_table(ref<Field, shared> f, Jacobian p) : Vec<Jacobian>
    {
        Vec<Jacobian> t = Vec::new();
        Vec::push(&mut t, Jacobian { .x = p.x, .y = p.y, .z = [0; 6] });
        Vec::push(&mut t, p);
        for (usize i = 2; i < 16; i += 1)
        {
            Jacobian next = if (i % 2 == 0) { jac_double(f, t[i / 2]) } else { jac_add(f, t[i - 1], p) };
            Vec::push(&mut t, next);
        }
        t
    }

    // The 4-bit digit of a at window w (bits 4w to 4w+3).
    fn f_nibble(array<u64, 6> a, usize w) : usize
    {
        narrow<usize>((a[w / 16] >> narrow<u32>(4 * (w % 16))) & 15)
    }

    // u1·G + u2·Q, sharing the doublings (Shamir's trick) over 4-bit windows:
    // four doublings and at most two additions per window. u1, u2 plain; the
    // values are public (verification), so the additions skipped for zero
    // digits show nothing secret.
    fn jac_twin(ref<Field, shared> f, array<u64, 6> u1, Jacobian g, array<u64, 6> u2, Jacobian q, usize bits) : Jacobian
    {
        Vec<Jacobian> tg = jac_table(f, g);
        Vec<Jacobian> tq = jac_table(f, q);
        Jacobian acc = tg[0];
        usize w = bits / 4;
        while (w > 0)
        {
            w -= 1;
            for (usize k = 0; k < 4; k += 1)
            {
                acc = jac_double(f, acc);
            }
            usize d1 = f_nibble(u1, w);
            usize d2 = f_nibble(u2, w);
            if (d1 != 0)
            {
                acc = jac_add(f, acc, tg[d1]);
            }
            if (d2 != 0)
            {
                acc = jac_add(f, acc, tq[d2]);
            }
        }
        acc
    }

    // The bit-at-a-time form, kept for reference by the windowed one above.
    fn jac_twin_bits(
        ref<Field, shared> f,
        array<u64, 6> u1,
        Jacobian g,
        array<u64, 6> u2,
        Jacobian q,
        usize bits,
    ) : Jacobian
    {
        Jacobian gq = jac_add(f, g, q);
        Jacobian acc = Jacobian { .x = g.x, .y = g.y, .z = [0; 6] };
        usize i = bits;
        while (i > 0)
        {
            i -= 1;
            acc = jac_double(f, acc);
            bool b1 = f_bit(u1, i);
            bool b2 = f_bit(u2, i);
            if (b1 && b2)
            {
                acc = jac_add(f, acc, gq);
            }
            else if (b1)
            {
                acc = jac_add(f, acc, g);
            }
            else if (b2)
            {
                acc = jac_add(f, acc, q);
            }
        }
        acc
    }

    fn curve_hex(ref<Field, shared> f, str hex) : array<u64, 6>
    {
        Vec<u8> b = Result::unwrap_or(from_hex(StringView::of(hex)), Vec::new());
        Option::unwrap(f_from_be(f, &b[0..$]))
    }

    // A positive DER INTEGER at `at`: its content bytes' range, or None.
    struct DerSpan
    {
        usize start;
        usize end;
    }

    fn der_positive_int(slice<u8, shared> d, usize at) : Option<DerSpan>
    {
        if (at + 2 > slice_len(d) || d[at] != 0x02)
        {
            return None;
        }
        usize len = widen<usize>(d[at + 1]);
        usize start = at + 2;
        if (len == 0 || len > 0x7f || start + len > slice_len(d))
        {
            return None;
        }
        if ((d[start] & 0x80) != 0)
        {
            return None;
        }
        if (len > 1 && d[start] == 0 && (d[start + 1] & 0x80) == 0)
        {
            return None;
        }
        // A leading zero byte only marks the number positive.
        usize from = if (d[start] == 0 && len > 1) { start + 1 } else { start };
        Some(DerSpan { .start = from, .end = start + len })
    }

    // Whether `signature`, the DER `SEQUENCE { r INTEGER, s INTEGER }` of
    // X.509 and TLS, is an ECDSA signature on `curve` by the holder of
    // `public_key` (an uncompressed point, 0x04 ‖ x ‖ y) of a message whose
    // digest is `digest` (FIPS 186-4 §6.4.2). A malformed key or signature is
    // false.
    export fn ecdsa_verify(
        EcCurve curve,
        slice<u8, shared> public_key,
        slice<u8, shared> digest,
        slice<u8, shared> signature,
    ) : bool
    {
        Field fp = field_of(curve, false);
        Field fn_ = field_of(curve, true);
        usize size = 8 * fp.n;
        if (slice_len(public_key) != 1 + 2 * size || public_key[0] != 0x04)
        {
            return false;
        }
        array<u64, 6> qx = match (f_from_be(&fp, &public_key[1..1 + size]))
        {
            Some(v) : f_to_mont(&fp, v),
            None    : return false,
        };
        array<u64, 6> qy = match (f_from_be(&fp, &public_key[1 + size..$]))
        {
            Some(v) : f_to_mont(&fp, v),
            None    : return false,
        };
        str b_hex = match (curve)
        {
            P256 : "5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b",
            P384 : "b3312fa7e23ee7e4988e056be3f82d19181d9c6efe8141120314088f5013875ac656398d8a2ed19d2a85c8edd3ec2aef",
        };
        array<u64, 6> b = f_to_mont(&fp, curve_hex(&fp, b_hex));
        // y² = x³ - 3x + b
        array<u64, 6> lhs = mont_mul(&fp, qy, qy);
        array<u64, 6> rhs = f_add(&fp, f_sub(&fp, mont_mul(&fp, mont_mul(&fp, qx, qx), qx), f_small(&fp, 3, qx)), b);
        if (!f_is_zero(&fp, f_sub(&fp, lhs, rhs)))
        {
            return false;
        }
        usize sl = slice_len(signature);
        if (sl < 8 || signature[0] != 0x30 || signature[1] > 0x7f || widen<usize>(signature[1]) != sl - 2)
        {
            return false;
        }
        DerSpan rs = match (der_positive_int(signature, 2))
        {
            Some(v) : v,
            None    : return false,
        };
        DerSpan ss = match (der_positive_int(signature, rs.end))
        {
            Some(v) : v,
            None    : return false,
        };
        if (ss.end != sl)
        {
            return false;
        }
        array<u64, 6> r = match (f_from_be(&fn_, &signature[rs.start..rs.end]))
        {
            Some(v) : v,
            None    : return false,
        };
        array<u64, 6> s = match (f_from_be(&fn_, &signature[ss.start..ss.end]))
        {
            Some(v) : v,
            None    : return false,
        };
        if (f_is_zero(&fn_, r) || f_is_zero(&fn_, s))
        {
            return false;
        }
        // e: the digest's leftmost 8·size bits (the order's bit length is the
        // prime's for both curves), reduced once below n.
        usize take = min(slice_len(digest), size);
        array<u64, 6> e = [0; 6];
        for (usize k = 0; k < take; k += 1)
        {
            usize pos = take - 1 - k;
            e[k / 8] = e[k / 8] | (widen<u64>(digest[pos]) << narrow<u32>(8 * (k % 8)));
        }
        if (!f_less(&fn_, e, fn_.m))
        {
            e = f_sub_raw(&fn_, e, fn_.m);
        }
        array<u64, 6> w = f_invert(&fn_, f_to_mont(&fn_, s));
        array<u64, 6> u1 = f_from_mont(&fn_, mont_mul(&fn_, f_to_mont(&fn_, e), w));
        array<u64, 6> u2 = f_from_mont(&fn_, mont_mul(&fn_, f_to_mont(&fn_, r), w));
        str gx = match (curve)
        {
            P256 : "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            P384 : "aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a385502f25dbf55296c3a545e3872760ab7",
        };
        str gy = match (curve)
        {
            P256 : "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            P384 : "3617de4a96262c6f5d9e98bf9292dc29f8f41dbd289a147ce9da3113b5f0b8c00a60b1ce1d7e819d7a431d7c90ea0e5f",
        };
        array<u64, 6> one = f_to_mont(&fp, [1, 0, 0, 0, 0, 0]);
        Jacobian g = Jacobian { .x = f_to_mont(
            &fp,
            curve_hex(&fp, gx),
        ), .y = f_to_mont(&fp, curve_hex(&fp, gy)), .z = one };
        Jacobian q = Jacobian { .x = qx, .y = qy, .z = one };
        Jacobian p = jac_twin(&fp, u1, g, u2, q, size * 8);
        if (f_is_zero(&fp, p.z))
        {
            return false;
        }
        // x mod n = r, without inverting Z: x = X/Z², and x mod n is r exactly
        // when x is r or (as x < p < 2n for both curves) r + n, the latter only
        // possible when r + n < p. So compare X with r·Z² and (r + n)·Z².
        array<u64, 6> z2 = mont_mul(&fp, p.z, p.z);
        if (f_is_zero(&fp, f_sub(&fp, p.x, mont_mul(&fp, f_to_mont(&fp, r), z2))))
        {
            return true;
        }
        array<u64, 6> rn = f_add_raw(&fp, r, fn_.m);
        if (f_less(&fn_, rn, fp.m) && f_less(&fn_, r, rn))
        {
            return f_is_zero(&fp, f_sub(&fp, p.x, mont_mul(&fp, f_to_mont(&fp, rn), z2)));
        }
        false
    }

    // Elliptic-curve keys, key agreement and signing on P-256 and P-384
    // (SEC 1 §3.3.1, RFC 8446 §7.4.2, FIPS 186-4 §6.4.1, RFC 6979; D-0165,
    // D-0168). A private key is 32 or 48 bytes naming a number in [1, n-1]; a
    // public key is the uncompressed point 0x04 ‖ x ‖ y; a shared secret is
    // the x-coordinate of the private key times the peer's point. The scalar
    // multiplication runs the same steps for every scalar: the scalar is made
    // one bit longer than the order by adding n or 2n (the same point), every
    // bit doubles and adds, and the sum is kept or dropped by a mask.

    fn curve_b_hex(EcCurve c) : str
    {
        match (c)
        {
            P256 : "5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b",
            P384 : "b3312fa7e23ee7e4988e056be3f82d19181d9c6efe8141120314088f5013875ac656398d8a2ed19d2a85c8edd3ec2aef",
        }
    }

    // The base point G, Montgomery form, z = 1.
    fn curve_base(ref<Field, shared> fp, EcCurve c) : Jacobian
    {
        str gx = match (c)
        {
            P256 : "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            P384 : "aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a385502f25dbf55296c3a545e3872760ab7",
        };
        str gy = match (c)
        {
            P256 : "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            P384 : "3617de4a96262c6f5d9e98bf9292dc29f8f41dbd289a147ce9da3113b5f0b8c00a60b1ce1d7e819d7a431d7c90ea0e5f",
        };
        Jacobian { .x = f_to_mont(
            fp,
            curve_hex(fp, gx),
        ), .y = f_to_mont(fp, curve_hex(fp, gy)), .z = f_to_mont(fp, [1, 0, 0, 0, 0, 0]) }
    }

    // The point 0x04 ‖ x ‖ y, Montgomery form, or None when the bytes are not
    // an uncompressed point on the curve (y² = x³ - 3x + b): a point off the
    // curve could leak the key it is multiplied by.
    fn curve_point(ref<Field, shared> fp, EcCurve c, slice<u8, shared> public_key) : Option<Jacobian>
    {
        usize size = 8 * fp.n;
        if (slice_len(public_key) != 1 + 2 * size || public_key[0] != 0x04)
        {
            return None;
        }
        array<u64, 6> qx = f_to_mont(fp, f_from_be(fp, &public_key[1..1 + size])?);
        array<u64, 6> qy = f_to_mont(fp, f_from_be(fp, &public_key[1 + size..$])?);
        array<u64, 6> b = f_to_mont(fp, curve_hex(fp, curve_b_hex(c)));
        array<u64, 6> lhs = mont_mul(fp, qy, qy);
        array<u64, 6> rhs = f_add(fp, f_sub(fp, mont_mul(fp, mont_mul(fp, qx, qx), qx), f_small(fp, 3, qx)), b);
        if (!f_is_zero(fp, f_sub(fp, lhs, rhs)))
        {
            return None;
        }
        Some(Jacobian { .x = qx, .y = qy, .z = f_to_mont(fp, [1, 0, 0, 0, 0, 0]) })
    }

    // a where mask is all ones, b where it is zero.
    fn jac_select(u64 mask, Jacobian a, Jacobian b) : Jacobian
    {
        Jacobian { .x = f_select(mask, a.x, b.x), .y = f_select(mask, a.y, b.y), .z = f_select(mask, a.z, b.z) }
    }

    // a + b over seven limbs (the scalar plus the order, one bit past it).
    fn limbs7_add(array<u64, 7> a, array<u64, 7> b) : array<u64, 7>
    {
        array<u64, 7> r = [0; 7];
        u64 carry = 0;
        for (usize i = 0; i < 7; i += 1)
        {
            u128 s = widen<u128>(a[i]) + widen<u128>(b[i]) + widen<u128>(carry);
            r[i] = narrow_wrapping<u64>(s);
            carry = narrow_wrapping<u64>(s >> 64);
        }
        r
    }

    // k·P for k in [1, n-1], P with z = 1, in Jacobian form; the same steps
    // whatever k is.
    fn ec_mul(ref<Field, shared> fp, ref<Field, shared> fn_, array<u64, 6> k, Jacobian p) : Jacobian
    {
        usize bits = 64 * fp.n;
        array<u64, 7> n7 = [fn_.m[0], fn_.m[1], fn_.m[2], fn_.m[3], fn_.m[4], fn_.m[5], 0];
        array<u64, 7> k1 = limbs7_add([k[0], k[1], k[2], k[3], k[4], k[5], 0], n7);
        array<u64, 7> k2 = limbs7_add(k1, n7);
        // k + n when that reaches 2^bits, k + 2n otherwise: bit `bits` is set
        // and nothing above it.
        u64 mask = wrapping_sub(0, k1[bits / 64] & 1);
        array<u64, 7> kh = [0; 7];
        for (usize i = 0; i < 7; i += 1)
        {
            kh[i] = (k1[i] & mask) | (k2[i] & ~mask);
        }
        Jacobian acc = p;
        usize i = bits;
        while (i > 0)
        {
            i -= 1;
            acc = jac_double(fp, acc);
            Jacobian sum = jac_add(fp, acc, p);
            u64 bit = (kh[i / 64] >> narrow<u32>(i % 64)) & 1;
            acc = jac_select(wrapping_sub(0, bit), sum, acc);
        }
        acc
    }

    // The low `len` bytes of the limbs, big-endian.
    fn limbs_be(array<u64, 6> a, usize len) : Vec<u8>
    {
        Vec<u8> b = Vec::filled(len, 0: u8);
        for (usize i = 0; i < len; i += 1)
        {
            b[len - 1 - i] = narrow_wrapping<u8>(a[i / 8] >> narrow<u32>(8 * (i % 8)));
        }
        b
    }

    // The private key as a scalar, None outside [1, n-1]; a length other than
    // the curve's is `diag.crypto-length`.
    fn ec_scalar(ref<Field, shared> fn_, slice<u8, shared> private_key) : Option<array<u64, 6>>
    {
        if (slice_len(private_key) != 8 * fn_.n)
        {
            fault(crypto_length);
        }
        match (f_from_be(fn_, private_key))
        {
            Some(k) : if (f_is_zero(fn_, k)) { None } else { Some(k) },
            None    : None,
        }
    }

    // Affine x and y (plain, not Montgomery) of a point that is not infinity.
    fn ec_affine(ref<Field, shared> fp, Jacobian q) : array<array<u64, 6>, 2>
    {
        array<u64, 6> zinv = f_invert(fp, q.z);
        array<u64, 6> zinv2 = mont_mul(fp, zinv, zinv);
        array<u64, 6> x = f_from_mont(fp, mont_mul(fp, q.x, zinv2));
        array<u64, 6> y = f_from_mont(fp, mont_mul(fp, q.y, mont_mul(fp, zinv2, zinv)));
        [x, y]
    }

    // A new private key for `curve`: 32 or 48 bytes of `os_random_bytes`
    // naming a number in [1, n-1] (drawn again in the rare case they do not).
    export fn ec_private_key(EcCurve curve) : Vec<u8>
    {
        Field fn_ = field_of(curve, true);
        Vec<u8> k = Vec::filled(8 * fn_.n, 0: u8);
        while (true)
        {
            os_random_bytes(&mut k[0..$]);
            if (Option::is_some(&ec_scalar(&fn_, &k[0..$])))
            {
                return k;
            }
        }
    }

    // The public key of `private_key` (the curve's length, another is
    // `diag.crypto-length`): 0x04 ‖ x ‖ y of the private key times the base
    // point, 65 or 97 bytes, or None when the private key is not in [1, n-1].
    export fn ec_public_key(EcCurve curve, slice<u8, shared> private_key) : Option<Vec<u8>>
    {
        Field fp = field_of(curve, false);
        Field fn_ = field_of(curve, true);
        array<u64, 6> k = ec_scalar(&fn_, private_key)?;
        usize size = 8 * fp.n;
        array<array<u64, 6>, 2> xy = ec_affine(&fp, ec_mul(&fp, &fn_, k, curve_base(&fp, curve)));
        Vec<u8> out = Vec::new();
        Vec::push(&mut out, 0x04);
        Vec::extend_from(&mut out, &limbs_be(xy[0], size)[0..$]);
        Vec::extend_from(&mut out, &limbs_be(xy[1], size)[0..$]);
        Some(out)
    }

    // The shared secret of `private_key` and the peer's `public_key` on
    // `curve`: the x-coordinate of their product, 32 or 48 bytes, or None when
    // the private key is not in [1, n-1] or the public key is not an
    // uncompressed point on the curve.
    export fn ecdh(EcCurve curve, slice<u8, shared> private_key, slice<u8, shared> public_key) : Option<Vec<u8>>
    {
        Field fp = field_of(curve, false);
        Field fn_ = field_of(curve, true);
        array<u64, 6> k = ec_scalar(&fn_, private_key)?;
        Jacobian q = curve_point(&fp, curve, public_key)?;
        Jacobian r = ec_mul(&fp, &fn_, k, q);
        if (f_is_zero(&fp, r.z))
        {
            return None;
        }
        Some(limbs_be(ec_affine(&fp, r)[0], 8 * fp.n))
    }

    // The P-256 functions of D-0165: the curve-generic ones on P256, with
    // their fixed-length results.
    export fn p256_private_key() : array<u8, 32>
    {
        Vec<u8> k = ec_private_key(P256);
        array<u8, 32> out = [0; 32];
        for (usize i = 0; i < 32; i += 1)
        {
            out[i] = k[i];
        }
        out
    }

    export fn p256_public_key(slice<u8, shared> private_key) : Option<array<u8, 65>>
    {
        Vec<u8> q = ec_public_key(P256, private_key)?;
        array<u8, 65> out = [0; 65];
        for (usize i = 0; i < 65; i += 1)
        {
            out[i] = q[i];
        }
        Some(out)
    }

    export fn p256_ecdh(slice<u8, shared> private_key, slice<u8, shared> public_key) : Option<array<u8, 32>>
    {
        Vec<u8> z = ecdh(P256, private_key, public_key)?;
        array<u8, 32> out = [0; 32];
        for (usize i = 0; i < 32; i += 1)
        {
            out[i] = z[i];
        }
        Some(out)
    }

    // The digest's leftmost 8·size bits as a number below n (bits2int of
    // RFC 6979 §2.3.2, then one reduction): ECDSA's e.
    fn digest_scalar(ref<Field, shared> fn_, slice<u8, shared> digest, usize size) : array<u64, 6>
    {
        usize take = min(slice_len(digest), size);
        array<u64, 6> e = [0; 6];
        for (usize k = 0; k < take; k += 1)
        {
            usize pos = take - 1 - k;
            e[k / 8] = e[k / 8] | (widen<u64>(digest[pos]) << narrow<u32>(8 * (k % 8)));
        }
        if (!f_less(fn_, e, fn_.m))
        {
            e = f_sub_raw(fn_, e, fn_.m);
        }
        e
    }

    // A DER INTEGER of the non-negative number with big-endian bytes `v`:
    // minimal, with a zero byte in front when the top bit is set.
    fn der_push_uint(ref<Vec<u8>, exclusive> out, slice<u8, shared> v)
    {
        usize from = 0;
        while (from + 1 < slice_len(v) && v[from] == 0)
        {
            from += 1;
        }
        bool pad = (v[from] & 0x80) != 0;
        Vec::push(out, 0x02);
        Vec::push(out, narrow<u8>(slice_len(v) - from + (if (pad) { 1 } else { 0 })));
        if (pad)
        {
            Vec::push(out, 0);
        }
        Vec::extend_from(out, &v[from..$]);
    }

    // The ECDSA signature (FIPS 186-4 §6.4.1) by `private_key` on `curve` of a
    // message whose digest is `digest`: the DER `SEQUENCE { r, s }` that
    // `ecdsa_verify` checks, or None when the private key is not in [1, n-1].
    // The nonce k is RFC 6979's, derived from the key and the digest with
    // HMAC-SHA-256 (P-256) or HMAC-SHA-384 (P-384), so the same message signs
    // the same way every time and no random number can repeat.
    export fn ecdsa_sign(EcCurve curve, slice<u8, shared> private_key, slice<u8, shared> digest) : Option<Vec<u8>>
    {
        Field fp = field_of(curve, false);
        Field fn_ = field_of(curve, true);
        usize size = 8 * fp.n;
        array<u64, 6> x = ec_scalar(&fn_, private_key)?;
        HashKind kind = match (curve)
        {
            P256 : Sha2_256,
            P384 : Sha2_384,
        };
        array<u64, 6> e = digest_scalar(&fn_, digest, size);
        Vec<u8> x_bytes = limbs_be(x, size);
        Vec<u8> e_bytes = limbs_be(e, size);
        Jacobian g = curve_base(&fp, curve);
        array<u64, 6> xm = f_to_mont(&fn_, x);
        array<u64, 6> em = f_to_mont(&fn_, e);
        // RFC 6979 §3.2 steps b to h.
        usize hlen = hash_len(kind);
        Vec<u8> v = Vec::filled(hlen, 1: u8);
        Vec<u8> key = Vec::filled(hlen, 0: u8);
        for (u8 round = 0; round < 2; round += 1)
        {
            Hmac m = Hmac::new(kind, &key[0..$]);
            Hmac::update(&mut m, &v[0..$]);
            array<u8, 1> sep = [round];
            Hmac::update(&mut m, &sep[0..$]);
            Hmac::update(&mut m, &x_bytes[0..$]);
            Hmac::update(&mut m, &e_bytes[0..$]);
            overwrite(&mut key, Hmac::finish(m));
            overwrite(&mut v, hmac(kind, &key[0..$], &v[0..$]));
        }
        while (true)
        {
            overwrite(&mut v, hmac(kind, &key[0..$], &v[0..$]));
            // T is V itself: the hash is as long as the order for both curves.
            Option<array<u64, 6>> candidate = f_from_be(&fn_, &v[0..size]);
            if (Some(k) = candidate)
            {
                if (!f_is_zero(&fn_, k))
                {
                    array<u64, 6> r = ec_affine(&fp, ec_mul(&fp, &fn_, k, g))[0];
                    if (!f_less(&fn_, r, fn_.m))
                    {
                        r = f_sub_raw(&fn_, r, fn_.m);
                    }
                    if (!f_is_zero(&fn_, r))
                    {
                        array<u64, 6> kinv = f_invert(&fn_, f_to_mont(&fn_, k));
                        array<u64, 6> rx = mont_mul(&fn_, f_to_mont(&fn_, r), xm);
                        array<u64, 6> s = f_from_mont(&fn_, mont_mul(&fn_, kinv, f_add(&fn_, em, rx)));
                        if (!f_is_zero(&fn_, s))
                        {
                            Vec<u8> body = Vec::new();
                            der_push_uint(&mut body, &limbs_be(r, size)[0..$]);
                            der_push_uint(&mut body, &limbs_be(s, size)[0..$]);
                            Vec<u8> out = Vec::new();
                            Vec::push(&mut out, 0x30);
                            Vec::push(&mut out, narrow<u8>(Vec::len(&body)));
                            Vec::extend_from(&mut out, &body[0..$]);
                            return Some(out);
                        }
                    }
                }
            }
            Hmac m = Hmac::new(kind, &key[0..$]);
            Hmac::update(&mut m, &v[0..$]);
            array<u8, 1> zero = [0];
            Hmac::update(&mut m, &zero[0..$]);
            overwrite(&mut key, Hmac::finish(m));
            overwrite(&mut v, hmac(kind, &key[0..$], &v[0..$]));
        }
    }

    // TLS 1.3's HKDF-Expand-Label (RFC 8446 §7.1, D-0161): HKDF-Expand of
    // `secret` with the label "tls13 " ‖ `label` and `context`, `len` bytes.
    // TLS's key schedule, and QUIC's and DTLS 1.3's, are built from it.
    export fn hkdf_expand_label(slice<u8, shared> secret, str label, slice<u8, shared> context, usize len) : Vec<u8>
    {
        Vec<u8> info = Vec::new();
        Vec::push_be(&mut info, narrow<u16>(len));
        Vec::push(&mut info, narrow<u8>(6 + str_len(label)));
        Vec::extend_from(&mut info, &b"tls13 "[0..$]);
        for (usize i = 0; i < str_len(label); i += 1)
        {
            Vec::push(&mut info, str_byte(label, i));
        }
        Vec::push(&mut info, narrow<u8>(slice_len(context)));
        Vec::extend_from(&mut info, context);
        hkdf_expand(secret, &info[0..$], len)
    }

    // AES (FIPS 197) and AES-GCM (NIST SP 800-38D; D-0165). AES is bitsliced:
    // four blocks at once, each step on every byte by masks and shifts, the
    // S-box a fixed circuit. GHASH multiplies by integer multiplications on
    // bits set four apart, whose carries cannot reach the bits kept. No branch
    // and no index depends on a key or a message byte, so the time taken
    // depends on the lengths alone.

    // Four blocks (64 bytes) as eight words: bit j of word b is bit b of byte
    // j, byte j being byte j % 16 of block j / 16, so each AES step works on
    // every byte of the four blocks at once, with masks and shifts and no
    // index or branch that depends on a key or a message (BearSSL's `aes_ct64`
    // approach, in a simpler layout).

    // The 8×8 bit matrix of x's bytes transposed: bit i of byte j becomes
    // bit j of byte i.
    fn aes_transpose8(u64 x) : u64
    {
        u64 v = x;
        u64 t = (v ^ (v >> 7)) & 0x00aa00aa00aa00aa;
        v = v ^ t ^ (t << 7);
        t = (v ^ (v >> 14)) & 0x0000cccc0000cccc;
        v = v ^ t ^ (t << 14);
        t = (v ^ (v >> 28)) & 0x00000000f0f0f0f0;
        v ^ t ^ (t << 28)
    }

    // 64 bytes in the bitsliced layout.
    fn aes_pack(array<u8, 64> b) : array<u64, 8>
    {
        array<u64, 8> q = [0; 8];
        for (usize k = 0; k < 8; k += 1)
        {
            u64 x = 0;
            for (usize i = 0; i < 8; i += 1)
            {
                x = x | (widen<u64>(b[8 * k + i]) << narrow<u32>(8 * i));
            }
            x = aes_transpose8(x);
            for (usize p = 0; p < 8; p += 1)
            {
                q[p] = q[p] | (((x >> narrow<u32>(8 * p)) & 0xff) << narrow<u32>(8 * k));
            }
        }
        q
    }

    // The 64 bytes of a bitsliced state.
    fn aes_unpack(array<u64, 8> q) : array<u8, 64>
    {
        array<u8, 64> b = [0; 64];
        for (usize k = 0; k < 8; k += 1)
        {
            u64 x = 0;
            for (usize p = 0; p < 8; p += 1)
            {
                x = x | (((q[p] >> narrow<u32>(8 * k)) & 0xff) << narrow<u32>(8 * p));
            }
            x = aes_transpose8(x);
            for (usize i = 0; i < 8; i += 1)
            {
                b[8 * k + i] = narrow_wrapping<u8>(x >> narrow<u32>(8 * i));
            }
        }
        b
    }

    // The S-box of every byte: Boyar and Peralta's circuit of 113 gates
    // (a linear layer, a non-linear middle of 32 ANDs, a linear layer).
    fn aes_sbox_ct(array<u64, 8> q) : array<u64, 8>
    {
        u64 x0 = q[7];
        u64 x1 = q[6];
        u64 x2 = q[5];
        u64 x3 = q[4];
        u64 x4 = q[3];
        u64 x5 = q[2];
        u64 x6 = q[1];
        u64 x7 = q[0];
        u64 y14 = x3 ^ x5;
        u64 y13 = x0 ^ x6;
        u64 y9 = x0 ^ x3;
        u64 y8 = x0 ^ x5;
        u64 t0 = x1 ^ x2;
        u64 y1 = t0 ^ x7;
        u64 y4 = y1 ^ x3;
        u64 y12 = y13 ^ y14;
        u64 y2 = y1 ^ x0;
        u64 y5 = y1 ^ x6;
        u64 y3 = y5 ^ y8;
        u64 t1 = x4 ^ y12;
        u64 y15 = t1 ^ x5;
        u64 y20 = t1 ^ x1;
        u64 y6 = y15 ^ x7;
        u64 y10 = y15 ^ t0;
        u64 y11 = y20 ^ y9;
        u64 y7 = x7 ^ y11;
        u64 y17 = y10 ^ y11;
        u64 y19 = y10 ^ y8;
        u64 y16 = t0 ^ y11;
        u64 y21 = y13 ^ y16;
        u64 y18 = x0 ^ y16;
        u64 t2 = y12 & y15;
        u64 t3 = y3 & y6;
        u64 t4 = t3 ^ t2;
        u64 t5 = y4 & x7;
        u64 t6 = t5 ^ t2;
        u64 t7 = y13 & y16;
        u64 t8 = y5 & y1;
        u64 t9 = t8 ^ t7;
        u64 t10 = y2 & y7;
        u64 t11 = t10 ^ t7;
        u64 t12 = y9 & y11;
        u64 t13 = y14 & y17;
        u64 t14 = t13 ^ t12;
        u64 t15 = y8 & y10;
        u64 t16 = t15 ^ t12;
        u64 t17 = t4 ^ t14;
        u64 t18 = t6 ^ t16;
        u64 t19 = t9 ^ t14;
        u64 t20 = t11 ^ t16;
        u64 t21 = t17 ^ y20;
        u64 t22 = t18 ^ y19;
        u64 t23 = t19 ^ y21;
        u64 t24 = t20 ^ y18;
        u64 t25 = t21 ^ t22;
        u64 t26 = t21 & t23;
        u64 t27 = t24 ^ t26;
        u64 t28 = t25 & t27;
        u64 t29 = t28 ^ t22;
        u64 t30 = t23 ^ t24;
        u64 t31 = t22 ^ t26;
        u64 t32 = t31 & t30;
        u64 t33 = t32 ^ t24;
        u64 t34 = t23 ^ t33;
        u64 t35 = t27 ^ t33;
        u64 t36 = t24 & t35;
        u64 t37 = t36 ^ t34;
        u64 t38 = t27 ^ t36;
        u64 t39 = t29 & t38;
        u64 t40 = t25 ^ t39;
        u64 t41 = t40 ^ t37;
        u64 t42 = t29 ^ t33;
        u64 t43 = t29 ^ t40;
        u64 t44 = t33 ^ t37;
        u64 t45 = t42 ^ t41;
        u64 z0 = t44 & y15;
        u64 z1 = t37 & y6;
        u64 z2 = t33 & x7;
        u64 z3 = t43 & y16;
        u64 z4 = t40 & y1;
        u64 z5 = t29 & y7;
        u64 z6 = t42 & y11;
        u64 z7 = t45 & y17;
        u64 z8 = t41 & y10;
        u64 z9 = t44 & y12;
        u64 z10 = t37 & y3;
        u64 z11 = t33 & y4;
        u64 z12 = t43 & y13;
        u64 z13 = t40 & y5;
        u64 z14 = t29 & y2;
        u64 z15 = t42 & y9;
        u64 z16 = t45 & y14;
        u64 z17 = t41 & y8;
        u64 t46 = z15 ^ z16;
        u64 t47 = z10 ^ z11;
        u64 t48 = z5 ^ z13;
        u64 t49 = z9 ^ z10;
        u64 t50 = z2 ^ z12;
        u64 t51 = z2 ^ z5;
        u64 t52 = z7 ^ z8;
        u64 t53 = z0 ^ z3;
        u64 t54 = z6 ^ z7;
        u64 t55 = z16 ^ z17;
        u64 t56 = z12 ^ t48;
        u64 t57 = t50 ^ t53;
        u64 t58 = z4 ^ t46;
        u64 t59 = z3 ^ t54;
        u64 t60 = t46 ^ t57;
        u64 t61 = z14 ^ t57;
        u64 t62 = t52 ^ t58;
        u64 t63 = t49 ^ t58;
        u64 t64 = z4 ^ t59;
        u64 t65 = t61 ^ t62;
        u64 t66 = z1 ^ t63;
        u64 s0 = t59 ^ t63;
        u64 s6 = t56 ^ ~t62;
        u64 s7 = t48 ^ ~t60;
        u64 t67 = t64 ^ t65;
        u64 s3 = t53 ^ t66;
        u64 s4 = t51 ^ t66;
        u64 s5 = t47 ^ t65;
        u64 s1 = t64 ^ ~s3;
        u64 s2 = t55 ^ ~t67;
        [s7, s6, s5, s4, s3, s2, s1, s0]
    }

    // The S-box of each of the eight bytes of x (the key expansion's SubWord).
    fn aes_sbox_bytes(u64 x) : u64
    {
        u64 t = aes_transpose8(x);
        array<u64, 8> q = [0; 8];
        for (usize p = 0; p < 8; p += 1)
        {
            q[p] = (t >> narrow<u32>(8 * p)) & 0xff;
        }
        q = aes_sbox_ct(q);
        u64 r = 0;
        for (usize p = 0; p < 8; p += 1)
        {
            r = r | ((q[p] & 0xff) << narrow<u32>(8 * p));
        }
        aes_transpose8(r)
    }

    // Masks of one row's bits in each 16-bit block, in the columns a row
    // moves right (`_LO`) and left (`_HI`) by in ShiftRows.
    const u64 AES_ROW0 = 0x1111111111111111;
    const u64 AES_ROW1_LO = 0x0222022202220222;
    const u64 AES_ROW1_HI = 0x2000200020002000;
    const u64 AES_ROW2_LO = 0x0044004400440044;
    const u64 AES_ROW2_HI = 0x4400440044004400;
    const u64 AES_ROW3_LO = 0x0008000800080008;
    const u64 AES_ROW3_HI = 0x8880888088808880;

    // ShiftRows: row r of each block moves r columns left, in every word.
    fn aes_shift_rows(array<u64, 8> q) : array<u64, 8>
    {
        array<u64, 8> r = [0; 8];
        for (usize b = 0; b < 8; b += 1)
        {
            u64 x = q[b];
            r[b] = (x & AES_ROW0)
                | ((x >> 4) & AES_ROW1_LO) | ((x << 12) & AES_ROW1_HI)
                | ((x >> 8) & AES_ROW2_LO) | ((x << 8) & AES_ROW2_HI)
                | ((x >> 12) & AES_ROW3_LO) | ((x << 4) & AES_ROW3_HI);
        }
        r
    }

    // Each column's rows rotated up by one, two and three: row r gets row
    // r + k's bit.
    fn aes_rot1(u64 x) : u64
    {
        ((x >> 1) & 0x7777777777777777) | ((x << 3) & 0x8888888888888888)
    }

    fn aes_rot2(u64 x) : u64
    {
        ((x >> 2) & 0x3333333333333333) | ((x << 2) & 0xcccccccccccccccc)
    }

    fn aes_rot3(u64 x) : u64
    {
        ((x >> 3) & 0x1111111111111111) | ((x << 1) & 0xeeeeeeeeeeeeeeee)
    }

    // MixColumns: row r of a column becomes a[r+1] ^ a[r+2] ^ a[r+3] ^
    // 2·(a[r] ^ a[r+1]), the doubling a shift across the words with the
    // reduction by 0x1b.
    fn aes_mix_columns(array<u64, 8> q) : array<u64, 8>
    {
        array<u64, 8> d = [0; 8];
        array<u64, 8> s = [0; 8];
        for (usize b = 0; b < 8; b += 1)
        {
            u64 r1 = aes_rot1(q[b]);
            d[b] = q[b] ^ r1;
            s[b] = r1 ^ aes_rot2(q[b]) ^ aes_rot3(q[b]);
        }
        [
            s[0] ^ d[7],
            s[1] ^ d[0] ^ d[7],
            s[2] ^ d[1],
            s[3] ^ d[2] ^ d[7],
            s[4] ^ d[3] ^ d[7],
            s[5] ^ d[4],
            s[6] ^ d[5],
            s[7] ^ d[6],
        ]
    }

    fn aes_xtime(u8 a) : u8
    {
        (a << 1) ^ (0x1b & wrapping_sub(0: u8, a >> 7))
    }

    // An expanded key: (rounds + 1) round keys, each bitsliced (`aes_pack`)
    // over four copies of itself, eight words a round.
    struct AesKey
    {
        array<u64, 120> sk;
        usize rounds;
    }

    // The key expansion (FIPS 197 §5.2) of a 16-, 24- or 32-byte key; another
    // length is `diag.crypto-length`. A word's byte k is its bits 8k to 8k+7.
    fn aes_expand(slice<u8, shared> key) : AesKey
    {
        usize nk = slice_len(key) / 4;
        if (slice_len(key) != 16 && slice_len(key) != 24 && slice_len(key) != 32)
        {
            fault(crypto_length);
        }
        usize rounds = nk + 6;
        array<u32, 60> w = [0; 60];
        for (usize i = 0; i < nk; i += 1)
        {
            w[i] = widen<u32>(key[4 * i]) | (widen<u32>(key[4 * i + 1]) << 8) | (widen<u32>(key[4 * i + 2]) << 16) | (widen<u32>(key[4 * i + 3]) << 24);
        }
        u8 rcon = 1;
        for (usize word = nk; word < 4 * (rounds + 1); word += 1)
        {
            u32 t = w[word - 1];
            if (word % nk == 0)
            {
                t = narrow_wrapping<u32>(aes_sbox_bytes(widen<u64>((t >> 8) | (t << 24)))) ^ widen<u32>(rcon);
                rcon = aes_xtime(rcon);
            }
            else if (nk > 6 && word % nk == 4)
            {
                t = narrow_wrapping<u32>(aes_sbox_bytes(widen<u64>(t)));
            }
            w[word] = w[word - nk] ^ t;
        }
        array<u64, 120> sk = [0; 120];
        for (usize r = 0; r <= rounds; r += 1)
        {
            array<u8, 64> b = [0; 64];
            for (usize j = 0; j < 16; j += 1)
            {
                u8 v = narrow_wrapping<u8>(w[4 * r + j / 4] >> narrow<u32>(8 * (j % 4)));
                for (usize k = 0; k < 4; k += 1)
                {
                    b[16 * k + j] = v;
                }
            }
            array<u64, 8> q = aes_pack(b);
            for (usize p = 0; p < 8; p += 1)
            {
                sk[8 * r + p] = q[p];
            }
        }
        AesKey { .sk = sk, .rounds = rounds }
    }

    // Four blocks, bitsliced, encrypted under an expanded key (FIPS 197 §5.1).
    // A block's byte r + 4c is row r of column c.
    fn aes_encrypt4(ref<AesKey, shared> key, array<u64, 8> input) : array<u64, 8>
    {
        array<u64, 8> q = input;
        for (usize p = 0; p < 8; p += 1)
        {
            q[p] = q[p] ^ key.sk[p];
        }
        for (usize round = 1; round <= key.rounds; round += 1)
        {
            q = aes_shift_rows(aes_sbox_ct(q));
            if (round < key.rounds)
            {
                q = aes_mix_columns(q);
            }
            for (usize p = 0; p < 8; p += 1)
            {
                q[p] = q[p] ^ key.sk[8 * round + p];
            }
        }
        q
    }

    // One block encrypted under an expanded key.
    fn aes_block(ref<AesKey, shared> key, array<u8, 16> input) : array<u8, 16>
    {
        array<u8, 64> b = [0; 64];
        for (usize i = 0; i < 16; i += 1)
        {
            b[i] = input[i];
        }
        array<u8, 64> e = aes_unpack(aes_encrypt4(key, aes_pack(b)));
        array<u8, 16> out = [0; 16];
        for (usize i = 0; i < 16; i += 1)
        {
            out[i] = e[i];
        }
        out
    }

    // One 16-byte block encrypted with AES under `key` (16, 24 or 32 bytes:
    // AES-128, AES-192 or AES-256). The bare block cipher, for building other
    // modes; `aes_gcm_seal` is what to encrypt data with.
    export fn aes_encrypt_block(slice<u8, shared> key, array<u8, 16> block) : array<u8, 16>
    {
        AesKey k = aes_expand(key);
        aes_block(&k, block)
    }

    // Sixteen bytes from `at` as a big-endian number, zeros past the end.
    fn gcm_block(slice<u8, shared> b, usize at) : u128
    {
        u128 x = 0;
        for (usize i = 0; i < 16; i += 1)
        {
            u8 v = if (at + i < slice_len(b)) { b[at + i] } else { 0 };
            x = (x << 8) | widen<u128>(v);
        }
        x
    }

    // The low 64 bits of the carry-less product of x and y. Each operand is
    // split into four sets of bits four apart, so that a product of two sets
    // has at most fifteen terms in any column below bit 64 and its carries
    // never reach a column of the set kept (BearSSL's `bmul64`).
    fn ghash_bmul64(u64 x, u64 y) : u64
    {
        u64 x0 = x & 0x1111111111111111;
        u64 x1 = x & 0x2222222222222222;
        u64 x2 = x & 0x4444444444444444;
        u64 x3 = x & 0x8888888888888888;
        u64 y0 = y & 0x1111111111111111;
        u64 y1 = y & 0x2222222222222222;
        u64 y2 = y & 0x4444444444444444;
        u64 y3 = y & 0x8888888888888888;
        u64 z0 = wrapping_mul(x0, y0) ^ wrapping_mul(x1, y3) ^ wrapping_mul(x2, y2) ^ wrapping_mul(x3, y1);
        u64 z1 = wrapping_mul(x0, y1) ^ wrapping_mul(x1, y0) ^ wrapping_mul(x2, y3) ^ wrapping_mul(x3, y2);
        u64 z2 = wrapping_mul(x0, y2) ^ wrapping_mul(x1, y1) ^ wrapping_mul(x2, y0) ^ wrapping_mul(x3, y3);
        u64 z3 = wrapping_mul(x0, y3) ^ wrapping_mul(x1, y2) ^ wrapping_mul(x2, y1) ^ wrapping_mul(x3, y0);
        (z0 & 0x1111111111111111) | (z1 & 0x2222222222222222) | (z2 & 0x4444444444444444) | (z3 & 0x8888888888888888)
    }

    // x with its 64 bits in reverse order.
    fn ghash_rev64(u64 x) : u64
    {
        u64 v = ((x & 0x5555555555555555) << 1) | ((x >> 1) & 0x5555555555555555);
        v = ((v & 0x3333333333333333) << 2) | ((v >> 2) & 0x3333333333333333);
        v = ((v & 0x0f0f0f0f0f0f0f0f) << 4) | ((v >> 4) & 0x0f0f0f0f0f0f0f0f);
        v = ((v & 0x00ff00ff00ff00ff) << 8) | ((v >> 8) & 0x00ff00ff00ff00ff);
        v = ((v & 0x0000ffff0000ffff) << 16) | ((v >> 16) & 0x0000ffff0000ffff);
        (v << 32) | (v >> 32)
    }

    // x·y in GF(2^128) with GCM's bit order (SP 800-38D §6.3): Karatsuba over
    // three 64-bit products, each half got by `ghash_bmul64` (the low half
    // directly, the high half from the bit-reversed operands), then the
    // reduction by x^128 + x^7 + x^2 + x + 1 (BearSSL's `ghash_ctmul64`).
    fn ghash_mul(u128 x, u128 y) : u128
    {
        u64 x1 = narrow_wrapping<u64>(x >> 64);
        u64 x0 = narrow_wrapping<u64>(x);
        u64 y1 = narrow_wrapping<u64>(y >> 64);
        u64 y0 = narrow_wrapping<u64>(y);
        u64 x0r = ghash_rev64(x0);
        u64 x1r = ghash_rev64(x1);
        u64 y0r = ghash_rev64(y0);
        u64 y1r = ghash_rev64(y1);
        u64 z0 = ghash_bmul64(x0, y0);
        u64 z1 = ghash_bmul64(x1, y1);
        u64 z2 = ghash_bmul64(x0 ^ x1, y0 ^ y1);
        u64 z0h = ghash_bmul64(x0r, y0r);
        u64 z1h = ghash_bmul64(x1r, y1r);
        u64 z2h = ghash_bmul64(x0r ^ x1r, y0r ^ y1r);
        z2 = z2 ^ z0 ^ z1;
        z2h = z2h ^ z0h ^ z1h;
        z0h = ghash_rev64(z0h) >> 1;
        z1h = ghash_rev64(z1h) >> 1;
        z2h = ghash_rev64(z2h) >> 1;
        u64 v0 = z0;
        u64 v1 = z0h ^ z2;
        u64 v2 = z1 ^ z2h;
        u64 v3 = z1h;
        v3 = (v3 << 1) | (v2 >> 63);
        v2 = (v2 << 1) | (v1 >> 63);
        v1 = (v1 << 1) | (v0 >> 63);
        v0 = v0 << 1;
        v2 = v2 ^ v0 ^ (v0 >> 1) ^ (v0 >> 2) ^ (v0 >> 7);
        v1 = v1 ^ (v0 << 63) ^ (v0 << 62) ^ (v0 << 57);
        v3 = v3 ^ v1 ^ (v1 >> 1) ^ (v1 >> 2) ^ (v1 >> 7);
        v2 = v2 ^ (v1 << 63) ^ (v1 << 62) ^ (v1 << 57);
        (widen<u128>(v3) << 64) | widen<u128>(v2)
    }

    fn ghash_update(u128 y, u128 h, slice<u8, shared> data) : u128
    {
        u128 acc = y;
        for (usize at = 0; at < slice_len(data); at += 16)
        {
            acc = ghash_mul(acc ^ gcm_block(data, at), h);
        }
        acc
    }

    fn gcm_bytes(u128 x) : array<u8, 16>
    {
        array<u8, 16> b = [0; 16];
        for (usize i = 0; i < 16; i += 1)
        {
            b[i] = narrow_wrapping<u8>(x >> narrow<u32>(8 * (15 - i)));
        }
        b
    }

    // `data` exclusive-ored with the keystream of the counter blocks after
    // `j0` (the last 32 bits incremented, wrapping), as a new vector; four
    // blocks at a time.
    fn gcm_ctr(ref<AesKey, shared> key, array<u8, 16> j0, slice<u8, shared> data) : Vec<u8>
    {
        Vec<u8> out = Vec::from_slice(data);
        array<u8, 64> cb = [0; 64];
        for (usize k = 0; k < 4; k += 1)
        {
            for (usize i = 0; i < 12; i += 1)
            {
                cb[16 * k + i] = j0[i];
            }
        }
        // The counter, kept apart from the blocks (whose last four bytes it is).
        u32 c = (widen<u32>(j0[12]) << 24) | (widen<u32>(j0[13]) << 16) | (widen<u32>(j0[14]) << 8) | widen<u32>(j0[15]);
        for (usize at = 0; at < slice_len(data); at += 64)
        {
            for (usize k = 0; k < 4; k += 1)
            {
                c = wrapping_add(c, 1);
                for (usize i = 0; i < 4; i += 1)
                {
                    cb[16 * k + 12 + i] = narrow_wrapping<u8>(c >> narrow<u32>(24 - 8 * i));
                }
            }
            array<u8, 64> ks = aes_unpack(aes_encrypt4(key, aes_pack(cb)));
            for (usize i = 0; i < 64 && at + i < slice_len(data); i += 1)
            {
                out[at + i] = out[at + i] ^ ks[i];
            }
        }
        out
    }

    // The tag: GHASH under H = E(0) of the aad, the ciphertext (each padded to
    // 16 bytes) and both bit lengths, exclusive-ored with E(j0).
    fn gcm_tag(
        ref<AesKey, shared> key,
        array<u8, 16> j0,
        slice<u8, shared> aad,
        slice<u8, shared> ciphertext,
    ) : array<u8, 16>
    {
        u128 h = gcm_block(&aes_block(key, [0; 16])[0..$], 0);
        u128 y = ghash_update(ghash_update(0, h, aad), h, ciphertext);
        u128 lengths = (widen<u128>(slice_len(aad)) * 8) << 64 | widen<u128>(slice_len(ciphertext)) * 8;
        y = ghash_mul(y ^ lengths, h);
        gcm_bytes(y ^ gcm_block(&aes_block(key, j0)[0..$], 0))
    }

    // The first counter block of a 12-byte nonce: nonce ‖ 0 0 0 1. Another
    // nonce length is `diag.crypto-length`.
    fn gcm_j0(slice<u8, shared> nonce) : array<u8, 16>
    {
        if (slice_len(nonce) != 12)
        {
            fault(crypto_length);
        }
        array<u8, 16> j = [0; 16];
        for (usize i = 0; i < 12; i += 1)
        {
            j[i] = nonce[i];
        }
        j[15] = 1;
        j
    }

    // Encrypts and authenticates `plaintext` with AES-GCM under `key` (16, 24
    // or 32 bytes) and the 12-byte `nonce`: the ciphertext, as long as the
    // plaintext, followed by a 16-byte tag over it and `aad`. A nonce must
    // never be used twice with one key.
    export fn aes_gcm_seal(
        slice<u8, shared> key,
        slice<u8, shared> nonce,
        slice<u8, shared> aad,
        slice<u8, shared> plaintext,
    ) : Vec<u8>
    {
        array<u8, 16> j0 = gcm_j0(nonce);
        AesKey k = aes_expand(key);
        Vec<u8> out = gcm_ctr(&k, j0, plaintext);
        array<u8, 16> tag = gcm_tag(&k, j0, aad, &out[0..$]);
        Vec::extend_from(&mut out, &tag[0..$]);
        out
    }

    // Checks and decrypts what `aes_gcm_seal` gave: Some(plaintext), or None
    // when the tag does not match (anything was changed, or the key, nonce or
    // aad differ); nothing of a refused message is returned.
    export fn aes_gcm_open(
        slice<u8, shared> key,
        slice<u8, shared> nonce,
        slice<u8, shared> aad,
        slice<u8, shared> sealed,
    ) : Option<Vec<u8>>
    {
        array<u8, 16> j0 = gcm_j0(nonce);
        AesKey k = aes_expand(key);
        usize n = slice_len(sealed);
        if (n < 16)
        {
            return None;
        }
        array<u8, 16> want = gcm_tag(&k, j0, aad, &sealed[0..n - 16]);
        if (!digest_eq(&want[0..$], &sealed[n - 16..n]))
        {
            return None;
        }
        Some(gcm_ctr(&k, j0, &sealed[0..n - 16]))
    }

    // ---- hashing, HMAC and HKDF over any `HashKind` (D-0166) ----
    //
    // `Hasher`, `Hmac` and the `_with` functions choose the hash at run time,
    // for protocols that negotiate it (TLS's SHA-384 suites, RFC 6979, PBKDF2)
    // and for programs that read it from a file format. The SHA-256 forms above
    // stay the simple spelling.

    // A hash computation of a `HashKind` chosen at run time: a plain value.
    export struct Hasher
    {
        HashKind kind;
        Sha1 s1;
        Sha256 s256;
        Sha512 s512;
    }

    export fn Hasher::new(HashKind kind) : Hasher
    {
        Hasher { .kind = kind, .s1 = Sha1::new(), .s256 = Sha256::new(), .s512 = match (kind)
        {
            Sha2_384 : Sha384::new().inner,
            _        : Sha512::new(),
        } }
    }

    export fn Hasher::update(ref<Hasher, exclusive> h, slice<u8, shared> data)
    {
        match (h.kind)
        {
            Sha1     : Sha1::update(&mut h.s1, data),
            Sha2_256 : Sha256::update(&mut h.s256, data),
            _        : Sha512::update(&mut h.s512, data),
        }
    }

    // The digest of everything given: 20, 32, 48 or 64 bytes.
    export fn Hasher::finish(Hasher h) : Vec<u8>
    {
        match (h.kind)
        {
            Sha1     : Vec::from_slice(&Sha1::finish(h.s1)[0..$]),
            Sha2_256 : Vec::from_slice(&Sha256::finish(h.s256)[0..$]),
            Sha2_384 : Vec::from_slice(&Sha384::finish(Sha384 { .inner = h.s512 })[0..$]),
            Sha2_512 : Vec::from_slice(&Sha512::finish(h.s512)[0..$]),
        }
    }

    // The block size of a hash, which HMAC pads its key to.
    fn block_len(HashKind kind) : usize
    {
        match (kind)
        {
            Sha1     : 64,
            Sha2_256 : 64,
            _        : 128,
        }
    }

    // An HMAC (RFC 2104) over a `HashKind` chosen at run time: the inner hash,
    // already fed the padded key, and the outer padded key for `finish`. A
    // plain value: a copy of a keyed `Hmac` continues on its own, which is how
    // `pbkdf2` reuses one key setup for every iteration.
    export struct Hmac
    {
        Hasher inner;
        array<u8, 128> outer_key;
    }

    // A MAC under `key`, of any length (one longer than the hash's block is
    // hashed first).
    export fn Hmac::new(HashKind kind, slice<u8, shared> key) : Hmac
    {
        usize b = block_len(kind);
        Vec<u8> k = Vec::filled(b, 0: u8);
        if (slice_len(key) > b)
        {
            Vec<u8> d = hash(kind, key);
            for (usize i = 0; i < Vec::len(&d); i += 1)
            {
                k[i] = d[i];
            }
        }
        else
        {
            for (usize i = 0; i < slice_len(key); i += 1)
            {
                k[i] = key[i];
            }
        }
        Vec<u8> ipad = Vec::filled(b, 0: u8);
        array<u8, 128> opad = [0; 128];
        for (usize i = 0; i < b; i += 1)
        {
            ipad[i] = k[i] ^ 0x36;
            opad[i] = k[i] ^ 0x5c;
        }
        Hasher inner = Hasher::new(kind);
        Hasher::update(&mut inner, &ipad[0..$]);
        Hmac { .inner = inner, .outer_key = opad }
    }

    export fn Hmac::update(ref<Hmac, exclusive> m, slice<u8, shared> data)
    {
        Hasher::update(&mut m.inner, data);
    }

    // The MAC of everything given: as long as the hash's digest.
    export fn Hmac::finish(Hmac m) : Vec<u8>
    {
        HashKind kind = m.inner.kind;
        Vec<u8> inner_digest = Hasher::finish(m.inner);
        Hasher outer = Hasher::new(kind);
        Hasher::update(&mut outer, &m.outer_key[0..block_len(kind)]);
        Hasher::update(&mut outer, &inner_digest[0..$]);
        Hasher::finish(outer)
    }

    // The HMAC of `data` under `key` over `kind`, in one call.
    export fn hmac(HashKind kind, slice<u8, shared> key, slice<u8, shared> data) : Vec<u8>
    {
        Hmac m = Hmac::new(kind, key);
        Hmac::update(&mut m, data);
        Hmac::finish(m)
    }

    // HKDF (RFC 5869) over `kind`: as `hkdf_extract`, `hkdf_expand` and
    // `hkdf_sha256`, with the hash's own digest length in place of 32 and a
    // `len` of at most 255 digests (`diag.hkdf-length` above that).
    export fn hkdf_extract_with(HashKind kind, slice<u8, shared> salt, slice<u8, shared> ikm) : Vec<u8>
    {
        if (slice_len(salt) == 0)
        {
            Vec<u8> zero = Vec::filled(hash_len(kind), 0: u8);
            return hmac(kind, &zero[0..$], ikm);
        }
        hmac(kind, salt, ikm)
    }

    export fn hkdf_expand_with(HashKind kind, slice<u8, shared> prk, slice<u8, shared> info, usize len) : Vec<u8>
    {
        usize h = hash_len(kind);
        if (len > 255 * h)
        {
            fault(hkdf_length);
        }
        Vec<u8> out = Vec::new();
        Vec::reserve(&mut out, len);
        Vec<u8> t = Vec::new();
        u8 counter = 1;
        while (Vec::len(&out) < len)
        {
            Hmac m = Hmac::new(kind, prk);
            Hmac::update(&mut m, &t[0..$]);
            Hmac::update(&mut m, info);
            array<u8, 1> c = [counter];
            Hmac::update(&mut m, &c[0..$]);
            overwrite(&mut t, Hmac::finish(m));
            for (usize i = 0; i < h && Vec::len(&out) < len; i += 1)
            {
                Vec::push(&mut out, t[i]);
            }
            if (Vec::len(&out) < len)
            {
                counter += 1;
            }
        }
        out
    }

    export fn hkdf_with(
        HashKind kind,
        slice<u8, shared> salt,
        slice<u8, shared> ikm,
        slice<u8, shared> info,
        usize len,
    ) : Vec<u8>
    {
        Vec<u8> prk = hkdf_extract_with(kind, salt, ikm);
        hkdf_expand_with(kind, &prk[0..$], info, len)
    }

    // TLS 1.3's HKDF-Expand-Label over `kind` (RFC 8446 §7.1), for the SHA-384
    // suites.
    export fn hkdf_expand_label_with(
        HashKind kind,
        slice<u8, shared> secret,
        str label,
        slice<u8, shared> context,
        usize len,
    ) : Vec<u8>
    {
        Vec<u8> info = Vec::new();
        Vec::push_be(&mut info, narrow<u16>(len));
        Vec::push(&mut info, narrow<u8>(6 + str_len(label)));
        Vec::extend_from(&mut info, &b"tls13 "[0..$]);
        for (usize i = 0; i < str_len(label); i += 1)
        {
            Vec::push(&mut info, str_byte(label, i));
        }
        Vec::push(&mut info, narrow<u8>(slice_len(context)));
        Vec::extend_from(&mut info, context);
        hkdf_expand_with(kind, secret, &info[0..$], len)
    }

    // ---- Ed25519 (RFC 8032, D-0167) ----
    //
    // Signatures over the twisted Edwards form of Curve25519, on the field
    // arithmetic X25519 uses (five 51-bit limbs) and, modulo the group order
    // L, on the Montgomery arithmetic the NIST curves use. A point is kept in
    // extended coordinates (x = X/Z, y = Y/Z, T = XY/Z); the addition formula
    // is complete, so one formula serves doubling, the identity and every
    // other case, and the scalar multiplication doubles and adds at every bit,
    // keeping the sum by a mask: no branch and no index depends on a secret.

    const array<u64, 5> ED_D = [0x34dca135978a3, 0x1a8283b156ebd, 0x5e7a26001c029, 0x739c663a03cbb, 0x52036cee2b6ff];

    const array<u64, 5> ED_2D = [0x69b9426b2f159, 0x35050762add7a, 0x3cf44c0038052, 0x6738cc7407977, 0x2406d9dc56dff];

    const array<u64, 5> ED_SQRT_M1 = [0x61b274a0ea0b0, 0xd5a5fc8f189d, 0x7ef5e9cbd0c60, 0x78595a6804c9e, 0x2b8324804fc1d];

    const array<u64, 5> ED_BX = [0x62d608f25d51a, 0x412a4b4f6592a, 0x75b7171a4b31d, 0x1ff60527118fe, 0x216936d3cd6e5];

    const array<u64, 5> ED_BY = [0x6666666666658, 0x4cccccccccccc, 0x1999999999999, 0x3333333333333, 0x6666666666666];

    fn fe_neg(array<u64, 5> a) : array<u64, 5>
    {
        fe_sub([0, 0, 0, 0, 0], a)
    }

    fn fe_is_zero(array<u64, 5> a) : bool
    {
        array<u8, 32> b = fe_to_bytes(a);
        u8 acc = 0;
        for (usize i = 0; i < 32; i += 1)
        {
            acc = acc | b[i];
        }
        acc == 0
    }

    fn fe_eq(array<u64, 5> a, array<u64, 5> b) : bool
    {
        fe_is_zero(fe_sub(a, b))
    }

    // a where mask is all ones, b where it is zero.
    fn fe_select(u64 mask, array<u64, 5> a, array<u64, 5> b) : array<u64, 5>
    {
        array<u64, 5> r = [0; 5];
        for (usize i = 0; i < 5; i += 1)
        {
            r[i] = (a[i] & mask) | (b[i] & ~mask);
        }
        r
    }

    // z^(2^252 - 3), the power RFC 8032 §5.1.3 takes a square root with; the
    // exponent is public.
    fn fe_pow_p58(array<u64, 5> z) : array<u64, 5>
    {
        array<u64, 5> r = [1, 0, 0, 0, 0];
        for (usize k = 0; k < 252; k += 1)
        {
            usize i = 251 - k;
            r = fe_mul(r, r);
            if (i != 1)
            {
                r = fe_mul(r, z);
            }
        }
        r
    }

    // A point in extended coordinates.
    struct Ed
    {
        array<u64, 5> x;
        array<u64, 5> y;
        array<u64, 5> z;
        array<u64, 5> t;
    }

    fn ed_identity() : Ed
    {
        Ed { .x = [0, 0, 0, 0, 0], .y = [1, 0, 0, 0, 0], .z = [1, 0, 0, 0, 0], .t = [0, 0, 0, 0, 0] }
    }

    fn ed_base() : Ed
    {
        Ed { .x = ED_BX, .y = ED_BY, .z = [1, 0, 0, 0, 0], .t = fe_mul(ED_BX, ED_BY) }
    }

    // p + q (RFC 8032 §5.1.4's complete formula).
    fn ed_add(Ed p, Ed q) : Ed
    {
        array<u64, 5> a = fe_mul(fe_sub(p.y, p.x), fe_sub(q.y, q.x));
        array<u64, 5> b = fe_mul(fe_add(p.y, p.x), fe_add(q.y, q.x));
        array<u64, 5> c = fe_mul(fe_mul(p.t, ED_2D), q.t);
        array<u64, 5> d = fe_mul(fe_add(p.z, p.z), q.z);
        array<u64, 5> e = fe_sub(b, a);
        array<u64, 5> f = fe_sub(d, c);
        array<u64, 5> g = fe_add(d, c);
        array<u64, 5> h = fe_add(b, a);
        Ed { .x = fe_mul(e, f), .y = fe_mul(g, h), .z = fe_mul(f, g), .t = fe_mul(e, h) }
    }

    fn ed_double(Ed p) : Ed
    {
        array<u64, 5> a = fe_mul(p.x, p.x);
        array<u64, 5> b = fe_mul(p.y, p.y);
        array<u64, 5> zz = fe_mul(p.z, p.z);
        array<u64, 5> c = fe_add(zz, zz);
        array<u64, 5> h = fe_add(a, b);
        array<u64, 5> xy = fe_add(p.x, p.y);
        array<u64, 5> e = fe_sub(h, fe_mul(xy, xy));
        array<u64, 5> g = fe_sub(a, b);
        array<u64, 5> f = fe_add(c, g);
        Ed { .x = fe_mul(e, f), .y = fe_mul(g, h), .z = fe_mul(f, g), .t = fe_mul(e, h) }
    }

    fn ed_select(u64 mask, Ed a, Ed b) : Ed
    {
        Ed { .x = fe_select(
            mask,
            a.x,
            b.x,
        ), .y = fe_select(mask, a.y, b.y), .z = fe_select(mask, a.z, b.z), .t = fe_select(mask, a.t, b.t) }
    }

    // The scalar (32 little-endian bytes) times p: a doubling and an addition
    // at every one of the 256 bits, the sum kept by a mask.
    fn ed_mul(slice<u8, shared> scalar, Ed p) : Ed
    {
        Ed acc = ed_identity();
        usize i = 256;
        while (i > 0)
        {
            i -= 1;
            acc = ed_double(acc);
            Ed sum = ed_add(acc, p);
            u64 bit = widen<u64>((scalar[i / 8] >> narrow<u32>(i % 8)) & 1);
            acc = ed_select(wrapping_sub(0, bit), sum, acc);
        }
        acc
    }

    // The point's encoding (RFC 8032 §5.1.2): y, little-endian, with the low
    // bit of x in the top bit.
    fn ed_encode(Ed p) : array<u8, 32>
    {
        array<u64, 5> zinv = fe_invert(p.z);
        array<u8, 32> out = fe_to_bytes(fe_mul(p.y, zinv));
        array<u8, 32> xb = fe_to_bytes(fe_mul(p.x, zinv));
        out[31] = out[31] | ((xb[0] & 1) << 7);
        out
    }

    // The point an encoding names (RFC 8032 §5.1.3), or None when it names
    // none: y not below p, no square root, or x = 0 with the sign bit set.
    fn ed_decode(slice<u8, shared> b) : Option<Ed>
    {
        array<u64, 5> y = fe_from_bytes(b);
        array<u8, 32> canon = fe_to_bytes(y);
        u8 diff = 0;
        for (usize i = 0; i < 31; i += 1)
        {
            diff = diff | (canon[i] ^ b[i]);
        }
        diff = diff | (canon[31] ^ (b[31] & 0x7f));
        if (diff != 0)
        {
            return None;
        }
        u8 sign = b[31] >> 7;
        array<u64, 5> y2 = fe_mul(y, y);
        array<u64, 5> u = fe_sub(y2, [1, 0, 0, 0, 0]);
        array<u64, 5> v = fe_add(fe_mul(ED_D, y2), [1, 0, 0, 0, 0]);
        array<u64, 5> v3 = fe_mul(fe_mul(v, v), v);
        array<u64, 5> v7 = fe_mul(fe_mul(v3, v3), v);
        array<u64, 5> x = fe_mul(fe_mul(u, v3), fe_pow_p58(fe_mul(u, v7)));
        array<u64, 5> vx2 = fe_mul(v, fe_mul(x, x));
        if (!fe_eq(vx2, u))
        {
            if (!fe_eq(vx2, fe_neg(u)))
            {
                return None;
            }
            x = fe_mul(x, ED_SQRT_M1);
        }
        array<u8, 32> xb = fe_to_bytes(x);
        if ((xb[0] & 1) == 0 && sign == 1 && fe_is_zero(x))
        {
            return None;
        }
        if ((xb[0] & 1) != sign)
        {
            x = fe_neg(x);
        }
        Some(Ed { .x = x, .y = y, .z = [1, 0, 0, 0, 0], .t = fe_mul(x, y) })
    }

    // Arithmetic modulo the group order L = 2^252 + 27742317777372353535851937790883648493.
    fn ed_order() : Field
    {
        Field { .m = [
            0x5812631a5cf5d3ed,
            0x14def9dea2f79cd6,
            0x0000000000000000,
            0x1000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ], .n = 4, .minv = 0xd2b51da312547e1b, .r2 = [
            0xa40611e3449c0f01,
            0xd00e1ba768859347,
            0xceec73d217f5be65,
            0x0399411b7c309a3d,
            0x0000000000000000,
            0x0000000000000000,
        ] }
    }

    // 32 little-endian bytes as four limbs (any value below 2^256).
    fn le_limbs(slice<u8, shared> b) : array<u64, 6>
    {
        array<u64, 6> r = [0; 6];
        for (usize i = 0; i < 32; i += 1)
        {
            r[i / 8] = r[i / 8] | (widen<u64>(b[i]) << narrow<u32>(8 * (i % 8)));
        }
        r
    }

    // A number below 2^256, as limbs, in Montgomery form mod L: one
    // multiplication by R² reduces it, whatever its size.
    fn sc_mont(ref<Field, shared> fl, array<u64, 6> a) : array<u64, 6>
    {
        mont_mul(fl, a, fl.r2)
    }

    // A 64-byte hash, little-endian, mod L, in Montgomery form: lo + hi·2^256.
    fn sc_from_hash(ref<Field, shared> fl, slice<u8, shared> h) : array<u64, 6>
    {
        array<u64, 6> lo = sc_mont(fl, le_limbs(&h[0..32]));
        array<u64, 6> hi = sc_mont(fl, sc_mont(fl, le_limbs(&h[32..64])));
        f_add(fl, lo, hi)
    }

    // A scalar in Montgomery form as 32 little-endian bytes.
    fn sc_bytes(ref<Field, shared> fl, array<u64, 6> m) : array<u8, 32>
    {
        array<u64, 6> a = f_from_mont(fl, m);
        array<u8, 32> out = [0; 32];
        for (usize i = 0; i < 32; i += 1)
        {
            out[i] = narrow_wrapping<u8>(a[i / 8] >> narrow<u32>(8 * (i % 8)));
        }
        out
    }

    // The expanded private key: the clamped scalar and the prefix (RFC 8032
    // §5.1.5).
    struct EdExpanded
    {
        array<u8, 32> scalar;
        array<u8, 32> prefix;
    }

    fn ed_expand(slice<u8, shared> private_key) : EdExpanded
    {
        if (slice_len(private_key) != 32)
        {
            fault(crypto_length);
        }
        array<u8, 64> h = sha512(private_key);
        array<u8, 32> s = [0; 32];
        array<u8, 32> prefix = [0; 32];
        for (usize i = 0; i < 32; i += 1)
        {
            s[i] = h[i];
            prefix[i] = h[32 + i];
        }
        s[0] = s[0] & 248;
        s[31] = (s[31] & 127) | 64;
        EdExpanded { .scalar = s, .prefix = prefix }
    }

    // A new private key: 32 bytes of `os_random_bytes`.
    export fn ed25519_private_key() : array<u8, 32>
    {
        array<u8, 32> k = [0; 32];
        os_random_bytes(&mut k[0..$]);
        k
    }

    // The public key of a 32-byte private key (another length is
    // `diag.crypto-length`): the encoding of the clamped scalar times the base
    // point.
    export fn ed25519_public_key(slice<u8, shared> private_key) : array<u8, 32>
    {
        EdExpanded e = ed_expand(private_key);
        ed_encode(ed_mul(&e.scalar[0..$], ed_base()))
    }

    // The signature of `message` by `private_key` (RFC 8032 §5.1.6): R ‖ S,
    // 64 bytes, the same for the same key and message every time, with no
    // random number to repeat.
    export fn ed25519_sign(slice<u8, shared> private_key, slice<u8, shared> message) : array<u8, 64>
    {
        EdExpanded e = ed_expand(private_key);
        Field fl = ed_order();
        array<u8, 32> a = ed_encode(ed_mul(&e.scalar[0..$], ed_base()));
        Sha512 hr = Sha512::new();
        Sha512::update(&mut hr, &e.prefix[0..$]);
        Sha512::update(&mut hr, message);
        array<u64, 6> r = sc_from_hash(&fl, &Sha512::finish(hr)[0..$]);
        array<u8, 32> r_bytes = sc_bytes(&fl, r);
        array<u8, 32> big_r = ed_encode(ed_mul(&r_bytes[0..$], ed_base()));
        Sha512 hk = Sha512::new();
        Sha512::update(&mut hk, &big_r[0..$]);
        Sha512::update(&mut hk, &a[0..$]);
        Sha512::update(&mut hk, message);
        array<u64, 6> k = sc_from_hash(&fl, &Sha512::finish(hk)[0..$]);
        array<u64, 6> s = f_add(&fl, r, mont_mul(&fl, k, sc_mont(&fl, le_limbs(&e.scalar[0..$]))));
        array<u8, 32> s_bytes = sc_bytes(&fl, s);
        array<u8, 64> out = [0; 64];
        for (usize i = 0; i < 32; i += 1)
        {
            out[i] = big_r[i];
            out[32 + i] = s_bytes[i];
        }
        out
    }

    // Whether `signature` is `public_key`'s Ed25519 signature of `message`
    // (RFC 8032 §5.1.7): [S]B = R + [k]A, with S below L and R and A valid
    // encodings. A key or signature of another length is false.
    export fn ed25519_verify(slice<u8, shared> public_key, slice<u8, shared> message, slice<u8, shared> signature) : bool
    {
        if (slice_len(public_key) != 32 || slice_len(signature) != 64)
        {
            return false;
        }
        Ed a = match (ed_decode(public_key))
        {
            Some(p) : p,
            None    : return false,
        };
        Ed big_r = match (ed_decode(&signature[0..32]))
        {
            Some(p) : p,
            None    : return false,
        };
        Field fl = ed_order();
        array<u64, 6> s = le_limbs(&signature[32..64]);
        if (!f_less(&fl, s, fl.m))
        {
            return false;
        }
        Sha512 hk = Sha512::new();
        Sha512::update(&mut hk, &signature[0..32]);
        Sha512::update(&mut hk, public_key);
        Sha512::update(&mut hk, message);
        array<u8, 32> k = sc_bytes(&fl, sc_from_hash(&fl, &Sha512::finish(hk)[0..$]));
        array<u8, 32> lhs = ed_encode(ed_mul(&signature[32..64], ed_base()));
        array<u8, 32> rhs = ed_encode(ed_add(big_r, ed_mul(&k[0..$], a)));
        digest_eq(&lhs[0..$], &rhs[0..$])
    }

    // ---- RSA private keys, signing and key generation (D-0169) ----
    //
    // The private operation runs on Montgomery arithmetic over 64-bit limbs of
    // a fixed length, the exponent taken in 4-bit windows whose table entry is
    // chosen by scanning the whole table with masks: the same multiplications
    // in the same order whatever the key. The two prime halves are combined by
    // the Chinese remainder theorem on the same fixed-length arithmetic, and
    // the result is checked against the public key before it is given out, so
    // a miscomputation can never reveal the primes. `BigUint` (std::math),
    // which takes time that depends on its values, touches only public
    // numbers: the padded message, the result and the public key.

    // Montgomery arithmetic modulo an odd `m` of `n` limbs: `minv` = -m⁻¹ mod
    // 2^64, `r2` = R² mod m, R = 2^(64·n).
    struct Mont
    {
        Vec<u64> m;
        usize n;
        u64 minv;
        Vec<u64> r2;
    }

    // The limbs of `a`, least significant first, padded to `n`.
    fn limbs_of(ref<BigUint, shared> a, usize n) : Vec<u64>
    {
        Vec<u8> be = BigUint::to_bytes_be_padded(a, 8 * n);
        Vec<u64> out = Vec::filled(n, 0: u64);
        for (usize i = 0; i < 8 * n; i += 1)
        {
            out[i / 8] = out[i / 8] | (widen<u64>(be[8 * n - 1 - i]) << narrow<u32>(8 * (i % 8)));
        }
        out
    }

    fn biguint_of(ref<Vec<u64>, shared> limbs) : BigUint
    {
        usize n = Vec::len(limbs);
        Vec<u8> be = Vec::filled(8 * n, 0: u8);
        for (usize i = 0; i < 8 * n; i += 1)
        {
            be[8 * n - 1 - i] = narrow_wrapping<u8>(limbs[i / 8] >> narrow<u32>(8 * (i % 8)));
        }
        BigUint::from_bytes_be(&be[0..$])
    }

    fn mont_new(ref<BigUint, shared> modulus) : Mont
    {
        usize n = (BigUint::bit_len(modulus) + 63) / 64;
        Vec<u64> m = limbs_of(modulus, n);
        // m⁻¹ mod 2^64 by Newton's iteration from the odd m's own low limb.
        u64 inv = 1;
        for (usize i = 0; i < 7; i += 1)
        {
            inv = wrapping_mul(inv, wrapping_sub(2, wrapping_mul(m[0], inv)));
        }
        BigUint one = BigUint::from_u64(1);
        BigUint r2 = BigUint::rem(&BigUint::shift_left(&one, 128 * n), modulus);
        Mont { .m = m, .n = n, .minv = wrapping_sub(0, inv), .r2 = limbs_of(&r2, n) }
    }

    // a·b·R⁻¹ mod m (coarsely integrated operand scanning), a and b below m.
    fn mont_mul_v(ref<Mont, shared> f, ref<Vec<u64>, shared> a, ref<Vec<u64>, shared> b) : Vec<u64>
    {
        usize n = f.n;
        Vec<u64> t = Vec::filled(n + 2, 0: u64);
        for (usize i = 0; i < n; i += 1)
        {
            u128 c = 0;
            u128 bi = widen<u128>(b[i]);
            for (usize j = 0; j < n; j += 1)
            {
                u128 x = widen<u128>(t[j]) + widen<u128>(a[j]) * bi + c;
                t[j] = narrow_wrapping<u64>(x);
                c = x >> 64;
            }
            u128 y = widen<u128>(t[n]) + c;
            t[n] = narrow_wrapping<u64>(y);
            t[n + 1] = narrow_wrapping<u64>(y >> 64);
            u64 k = wrapping_mul(t[0], f.minv);
            u128 kk = widen<u128>(k);
            u128 z = widen<u128>(t[0]) + kk * widen<u128>(f.m[0]);
            c = z >> 64;
            for (usize j = 1; j < n; j += 1)
            {
                u128 x = widen<u128>(t[j]) + kk * widen<u128>(f.m[j]) + c;
                t[j - 1] = narrow_wrapping<u64>(x);
                c = x >> 64;
            }
            u128 w = widen<u128>(t[n]) + c;
            t[n - 1] = narrow_wrapping<u64>(w);
            t[n] = t[n + 1] + narrow_wrapping<u64>(w >> 64);
        }
        // t - m unless t (with its carry) is below m: chosen by a mask.
        Vec<u64> r = Vec::filled(n, 0: u64);
        u64 borrow = 0;
        for (usize j = 0; j < n; j += 1)
        {
            u128 x = wrapping_sub(wrapping_sub(widen<u128>(t[j]), widen<u128>(f.m[j])), widen<u128>(borrow));
            r[j] = narrow_wrapping<u64>(x);
            borrow = narrow_wrapping<u64>(x >> 127);
        }
        u64 keep = wrapping_sub(0, (1 - t[n]) & borrow);
        for (usize j = 0; j < n; j += 1)
        {
            r[j] = (t[j] & keep) | (r[j] & ~keep);
        }
        r
    }

    // base^exp mod m, base below m: a fixed 4-bit window over every bit of
    // `exp`'s `n` limbs, the table entry selected by masks, so the work does
    // not depend on the exponent.
    fn mont_pow(ref<Mont, shared> f, ref<BigUint, shared> base, ref<BigUint, shared> exp) : BigUint
    {
        biguint_of(&mont_pow_limbs(f, &limbs_of(base, f.n), exp))
    }

    fn mont_pow_limbs(ref<Mont, shared> f, ref<Vec<u64>, shared> base, ref<BigUint, shared> exp) : Vec<u64>
    {
        usize n = f.n;
        Vec<u64> one = Vec::filled(n, 0: u64);
        one[0] = 1;
        Vec<u64> one_m = mont_mul_v(f, &one, &f.r2);
        Vec<u64> base_m = mont_mul_v(f, base, &f.r2);
        Vec<Vec<u64>> table = Vec::new();
        Vec::push(&mut table, Vec::clone(&one_m));
        for (usize i = 1; i < 16; i += 1)
        {
            Vec<u64> next = mont_mul_v(f, &table[i - 1], &base_m);
            Vec::push(&mut table, next);
        }
        Vec<u64> e = limbs_of(exp, n);
        Vec<u64> acc = Vec::clone(&one_m);
        usize windows = 16 * n;
        usize w = windows;
        while (w > 0)
        {
            w -= 1;
            for (usize s = 0; s < 4; s += 1)
            {
                overwrite(&mut acc, mont_mul_v(f, &acc, &acc));
            }
            u64 bits = (e[w / 16] >> narrow<u32>(4 * (w % 16))) & 15;
            Vec<u64> entry = Vec::filled(n, 0: u64);
            for (u64 i = 0; i < 16; i += 1)
            {
                u64 x = i ^ bits;
                u64 mask = wrapping_sub(((x | wrapping_sub(0, x)) >> 63), 1);
                for (usize j = 0; j < n; j += 1)
                {
                    entry[j] = entry[j] | (table[narrow<usize>(i)][j] & mask);
                }
            }
            overwrite(&mut acc, mont_mul_v(f, &acc, &entry));
        }
        mont_mul_v(f, &acc, &one)
    }

    // An RSA private key: the public modulus and exponent, the private
    // exponent and the two primes, with the CRT values the private operation
    // uses.
    export struct RsaPrivateKey
    {
        export BigUint n;
        export BigUint e;
        export BigUint d;
        export BigUint p;
        export BigUint q;
        BigUint dp;
        BigUint dq;
        BigUint qinv;
    }

    // The key with these parts, or None when they do not fit together: p·q
    // must be n, p and q odd and above 1, and e·d ≡ 1 modulo p-1 and q-1.
    export fn RsaPrivateKey::new(BigUint n, BigUint e, BigUint d, BigUint p, BigUint q) : Option<RsaPrivateKey>
    {
        BigUint one = BigUint::from_u64(1);
        BigUint two = BigUint::from_u64(2);
        if (!BigUint::less(&two, &p) || !BigUint::less(&two, &q) || !BigUint::bit(&p, 0) || !BigUint::bit(&q, 0))
        {
            return None;
        }
        if (!BigUint::eq(&BigUint::mul(&p, &q), &n))
        {
            return None;
        }
        BigUint p1 = BigUint::sub(&p, &one);
        BigUint q1 = BigUint::sub(&q, &one);
        BigUint ed = BigUint::mul(&e, &d);
        if (!BigUint::eq(&BigUint::rem(&ed, &p1), &one) || !BigUint::eq(&BigUint::rem(&ed, &q1), &one))
        {
            return None;
        }
        BigUint dp = BigUint::rem(&d, &p1);
        BigUint dq = BigUint::rem(&d, &q1);
        BigUint qinv = BigUint::mod_inverse(&q, &p)?;
        Some(RsaPrivateKey { .n = n, .e = e, .d = d, .p = p, .q = q, .dp = dp, .dq = dq, .qinv = qinv })
    }

    export fn RsaPrivateKey::public_key(ref<RsaPrivateKey, shared> key) : RsaPublicKey
    {
        RsaPublicKey { .n = BigUint::clone(&key.n), .e = BigUint::clone(&key.e) }
    }

    // a - b mod m over `n` limbs, a and b below m, by masks.
    fn limbs_sub_mod(ref<Mont, shared> f, ref<Vec<u64>, shared> a, ref<Vec<u64>, shared> b) : Vec<u64>
    {
        usize n = f.n;
        Vec<u64> r = Vec::filled(n, 0: u64);
        u64 borrow = 0;
        for (usize j = 0; j < n; j += 1)
        {
            u128 x = wrapping_sub(wrapping_sub(widen<u128>(a[j]), widen<u128>(b[j])), widen<u128>(borrow));
            r[j] = narrow_wrapping<u64>(x);
            borrow = narrow_wrapping<u64>(x >> 127);
        }
        // Add m back where it borrowed.
        u64 mask = wrapping_sub(0, borrow);
        u64 carry = 0;
        for (usize j = 0; j < n; j += 1)
        {
            u128 x = widen<u128>(r[j]) + widen<u128>(f.m[j] & mask) + widen<u128>(carry);
            r[j] = narrow_wrapping<u64>(x);
            carry = narrow_wrapping<u64>(x >> 64);
        }
        r
    }

    // m^d mod n for m below n: the two halves modulo p and q, combined by
    // Garner's formula on fixed-length limbs, and checked.
    fn rsa_private_op(ref<RsaPrivateKey, shared> key, ref<BigUint, shared> m) : BigUint
    {
        Mont mp = mont_new(&key.p);
        Mont mq = mont_new(&key.q);
        usize np = mp.n;
        usize nq = mq.n;
        // m1 = m^dp mod p, m2 = m^dq mod q (m reduced: public values).
        Vec<u64> m1 = mont_pow_limbs(&mp, &limbs_of(&BigUint::rem(m, &key.p), np), &key.dp);
        Vec<u64> m2 = mont_pow_limbs(&mq, &limbs_of(&BigUint::rem(m, &key.q), nq), &key.dq);
        // h = qinv·(m1 - m2 mod p) mod p, on p's arithmetic.
        Vec<u64> m2_mod_p = limbs_of(&BigUint::rem(&biguint_of(&m2), &key.p), np);
        Vec<u64> diff = limbs_sub_mod(&mp, &m1, &m2_mod_p);
        Vec<u64> qinv_m = mont_mul_v(&mp, &limbs_of(&key.qinv, np), &mp.r2);
        Vec<u64> h = mont_mul_v(&mp, &diff, &qinv_m);
        // s = m2 + h·q: a plain product over limbs, then the sum.
        Vec<u64> q_limbs = limbs_of(&key.q, nq);
        Vec<u64> s = Vec::filled(np + nq + 1, 0: u64);
        for (usize i = 0; i < np; i += 1)
        {
            u128 carry = 0;
            u128 hi = widen<u128>(h[i]);
            for (usize j = 0; j < nq; j += 1)
            {
                u128 x = widen<u128>(s[i + j]) + hi * widen<u128>(q_limbs[j]) + carry;
                s[i + j] = narrow_wrapping<u64>(x);
                carry = x >> 64;
            }
            s[i + nq] = narrow_wrapping<u64>(carry);
        }
        u128 c = 0;
        for (usize j = 0; j < np + nq + 1; j += 1)
        {
            u128 x = widen<u128>(s[j]) + (if (j < nq) { widen<u128>(m2[j]) } else { 0 }) + c;
            s[j] = narrow_wrapping<u64>(x);
            c = x >> 64;
        }
        BigUint result = biguint_of(&s);
        // Checked against the public key: a wrong result would leak the
        // primes, so a key that fails is used without the CRT instead.
        if (BigUint::eq(&BigUint::mod_pow(&result, &key.e, &key.n), m))
        {
            return result;
        }
        Mont mn = mont_new(&key.n);
        mont_pow(&mn, m, &key.d)
    }

    // The EMSA-PKCS1-v1_5 encoding (RFC 8017 §9.2) of `digest` in `k` bytes;
    // too short a modulus is `diag.crypto-length`.
    fn pkcs1v15_encode(HashKind kind, slice<u8, shared> digest, usize k) : Vec<u8>
    {
        Vec<u8> prefix = digest_info_prefix(kind);
        usize t_len = Vec::len(&prefix) + slice_len(digest);
        if (slice_len(digest) != hash_len(kind) || k < t_len + 11)
        {
            fault(crypto_length);
        }
        Vec<u8> em = Vec::new();
        Vec::push(&mut em, 0);
        Vec::push(&mut em, 1);
        for (usize i = 0; i < k - t_len - 3; i += 1)
        {
            Vec::push(&mut em, 0xff);
        }
        Vec::push(&mut em, 0);
        Vec::extend_from(&mut em, &prefix[0..$]);
        Vec::extend_from(&mut em, digest);
        em
    }

    // `key`'s RSASSA-PKCS1-v1_5 signature (RFC 8017 §8.2.1) of a message
    // whose `kind` digest is `digest`: as many bytes as the modulus, the same
    // every time. A digest of the wrong length, or a modulus too short for the
    // encoding, is `diag.crypto-length`.
    export fn rsa_sign_pkcs1v15(ref<RsaPrivateKey, shared> key, HashKind kind, slice<u8, shared> digest) : Vec<u8>
    {
        usize k = (BigUint::bit_len(&key.n) + 7) / 8;
        Vec<u8> em = pkcs1v15_encode(kind, digest, k);
        BigUint s = rsa_private_op(key, &BigUint::from_bytes_be(&em[0..$]));
        BigUint::to_bytes_be_padded(&s, k)
    }

    // `key`'s RSASSA-PSS signature (RFC 8017 §8.1.1) of a message whose
    // `kind` digest is `digest`, with MGF1 over the same hash and a random
    // salt as long as the digest, as `rsa_verify_pss` expects: a different
    // signature every time, each of them valid.
    export fn rsa_sign_pss(ref<RsaPrivateKey, shared> key, HashKind kind, slice<u8, shared> digest) : Vec<u8>
    {
        usize mod_bits = BigUint::bit_len(&key.n);
        usize k = (mod_bits + 7) / 8;
        usize h_len = hash_len(kind);
        usize em_bits = mod_bits - 1;
        usize em_len = (em_bits + 7) / 8;
        if (slice_len(digest) != h_len || em_len < 2 * h_len + 2)
        {
            fault(crypto_length);
        }
        Vec<u8> salt = Vec::filled(h_len, 0: u8);
        os_random_bytes(&mut salt[0..$]);
        Vec<u8> m2 = Vec::filled(8, 0: u8);
        Vec::extend_from(&mut m2, digest);
        Vec::extend_from(&mut m2, &salt[0..$]);
        Vec<u8> h = hash(kind, &m2[0..$]);
        usize db_len = em_len - h_len - 1;
        Vec<u8> db = Vec::filled(db_len - h_len - 1, 0: u8);
        Vec::push(&mut db, 1);
        Vec::extend_from(&mut db, &salt[0..$]);
        Vec<u8> mask = mgf1(kind, &h[0..$], db_len);
        Vec<u8> em = Vec::new();
        for (usize i = 0; i < db_len; i += 1)
        {
            Vec::push(&mut em, db[i] ^ mask[i]);
        }
        u32 unused = narrow<u32>(8 * em_len - em_bits);
        em[0] = em[0] & narrow_wrapping<u8>((0xff: u32) >> unused);
        Vec::extend_from(&mut em, &h[0..$]);
        Vec::push(&mut em, 0xbc);
        BigUint s = rsa_private_op(key, &BigUint::from_bytes_be(&em[0..$]));
        BigUint::to_bytes_be_padded(&s, k)
    }

    // The odd primes below 1000, for trial division.
    fn small_primes() : Vec<u32>
    {
        Vec<u32> out = Vec::new();
        for (u32 c = 3; c < 1000; c += 2)
        {
            bool prime = true;
            for (usize i = 0; i < Vec::len(&out) && out[i] * out[i] <= c; i += 1)
            {
                if (c % out[i] == 0)
                {
                    prime = false;
                    break;
                }
            }
            if (prime)
            {
                Vec::push(&mut out, c);
            }
        }
        out
    }

    // Whether the odd `c` is prime beyond reasonable doubt: no small factor,
    // then `rounds` of Miller-Rabin with random bases (FIPS 186-4 C.3.1).
    fn probably_prime(ref<BigUint, shared> c, ref<Vec<u32>, shared> primes, usize rounds) : bool
    {
        for (usize i = 0; i < Vec::len(primes); i += 1)
        {
            BigUint p = BigUint::from_u64(widen<u64>(primes[i]));
            if (BigUint::is_zero(&BigUint::rem(c, &p)))
            {
                return BigUint::eq(c, &p);
            }
        }
        BigUint one = BigUint::from_u64(1);
        BigUint c1 = BigUint::sub(c, &one);
        usize a = 0;
        while (!BigUint::bit(&c1, a))
        {
            a += 1;
        }
        BigUint m = BigUint::shift_right(&c1, a);
        Mont f = mont_new(c);
        usize k = (BigUint::bit_len(c) + 7) / 8;
        Vec<u8> rb = Vec::filled(k, 0: u8);
        BigUint two = BigUint::from_u64(2);
        for (usize round = 0; round < rounds; round += 1)
        {
            os_random_bytes(&mut rb[0..$]);
            BigUint b = BigUint::add(
                &BigUint::rem(&BigUint::from_bytes_be(&rb[0..$]), &BigUint::sub(c, &BigUint::from_u64(3))),
                &two,
            );
            BigUint z = mont_pow(&f, &b, &m);
            if (BigUint::eq(&z, &one) || BigUint::eq(&z, &c1))
            {
                continue;
            }
            bool witness = true;
            for (usize j = 1; j < a; j += 1)
            {
                overwrite(&mut z, BigUint::rem(&BigUint::mul(&z, &z), c));
                if (BigUint::eq(&z, &c1))
                {
                    witness = false;
                    break;
                }
                if (BigUint::eq(&z, &one))
                {
                    return false;
                }
            }
            if (witness)
            {
                return false;
            }
        }
        true
    }

    // A random prime of exactly `bits` bits (its top two bits set, so a
    // product of two is exactly twice as long), odd, with p - 1 prime to `e`.
    fn random_prime(usize bits, ref<BigUint, shared> e, ref<Vec<u32>, shared> primes) : BigUint
    {
        usize k = (bits + 7) / 8;
        Vec<u8> rb = Vec::filled(k, 0: u8);
        BigUint one = BigUint::from_u64(1);
        while (true)
        {
            os_random_bytes(&mut rb[0..$]);
            BigUint c = BigUint::from_bytes_be(&rb[0..$]);
            // Exactly `bits` bits with the top two set, and odd.
            BigUint top = BigUint::shift_left(&one, bits - 1);
            BigUint second = BigUint::shift_left(&one, bits - 2);
            BigUint mask = BigUint::sub(&BigUint::shift_left(&one, bits), &one);
            BigUint c2 = BigUint::rem(&c, &BigUint::add(&mask, &one));
            BigUint c3 = BigUint::add(&BigUint::rem(&c2, &second), &BigUint::add(&top, &second));
            if (!BigUint::bit(&c3, 0))
            {
                overwrite(&mut c3, BigUint::add(&c3, &one));
            }
            BigUint c4 = BigUint::sub(&c3, &one);
            if (BigUint::is_zero(&BigUint::rem(&c4, e)))
            {
                continue;
            }
            if (probably_prime(&c3, primes, 5))
            {
                return c3;
            }
        }
    }

    // A new key of `bits` bits (1024 to 8192, a multiple of 64; another is
    // `diag.crypto-length`) with the public exponent 65537: two random primes
    // of half the length, found by trial division and Miller-Rabin
    // (FIPS 186-4 B.3.3), from `os_random_bytes`. Slow under an interpreter,
    // as every key generation is.
    export fn rsa_generate_key(usize bits) : RsaPrivateKey
    {
        if (bits < 1024 || bits > 8192 || bits % 64 != 0)
        {
            fault(crypto_length);
        }
        Vec<u32> primes = small_primes();
        BigUint e = BigUint::from_u64(65537);
        BigUint one = BigUint::from_u64(1);
        while (true)
        {
            BigUint p = random_prime(bits / 2, &e, &primes);
            BigUint q = random_prime(bits / 2, &e, &primes);
            if (BigUint::eq(&p, &q))
            {
                continue;
            }
            BigUint n = BigUint::mul(&p, &q);
            BigUint p1 = BigUint::sub(&p, &one);
            BigUint q1 = BigUint::sub(&q, &one);
            BigUint phi = BigUint::mul(&p1, &q1);
            BigUint d = match (BigUint::mod_inverse(&e, &phi))
            {
                Some(v) : v,
                None    : continue,
            };
            if (Some(key) = RsaPrivateKey::new(n, BigUint::clone(&e), d, p, q))
            {
                return key;
            }
        }
    }

    // ---- BLAKE2b, PBKDF2, Argon2id and password hashing (D-0170) ----
    //
    // A stored password is never kept as a fast hash: `hash_password` runs
    // Argon2id (RFC 9106), which costs memory as well as time, so guessing
    // stays expensive on any hardware, and writes the parameters and salt next
    // to the result in the PHC string format every password library reads.
    // PBKDF2 (RFC 8018) is here for the formats that still use it. BLAKE2b
    // (RFC 7693) is Argon2's hash and a fast general one.

    const array<u64, 8> BLAKE2B_IV = [
        0x6a09e667f3bcc908,
        0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b,
        0xa54ff53a5f1d36f1,
        0x510e527fade682d1,
        0x9b05688c2b3e6c1f,
        0x1f83d9abfb41bd6b,
        0x5be0cd19137e2179,
    ];

    // The ten message schedules, sixteen indices each.
    const array<u8, 160> BLAKE2B_SIGMA = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3,
        11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4,
        7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8,
        9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13,
        2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9,
        12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11,
        13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10,
        6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5,
        10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0,
    ];

    // A BLAKE2b computation in progress: the state, the block not yet
    // compressed (the last block is always compressed by `finish`, with the
    // final flag), the bytes compressed so far, and the digest length.
    export struct Blake2b
    {
        array<u64, 8> h;
        array<u8, 128> block;
        usize fill;
        u64 length;
        usize out_len;
    }

    // The positions G mixes, four per call, eight calls per round (RFC 7693
    // §3.2): the columns, then the diagonals.
    const array<u8, 32> BLAKE2B_G = [
        0,
        4,
        8,
        12,
        1,
        5,
        9,
        13,
        2,
        6,
        10,
        14,
        3,
        7,
        11,
        15,
        0,
        5,
        10,
        15,
        1,
        6,
        11,
        12,
        2,
        7,
        8,
        13,
        3,
        4,
        9,
        14,
    ];

    // One block folded into the state `h`, all values (as `sha256_block`):
    // `length` is the byte count so far, `last` marks the final block.
    fn blake2b_block(array<u64, 8> h, array<u8, 128> blk, u64 length, bool last) : array<u64, 8>
    {
        array<u64, 16> m = [0; 16];
        for (usize i = 0; i < 16; i += 1)
        {
            u64 x = 0;
            for (usize j = 0; j < 8; j += 1)
            {
                x = x | (widen<u64>(blk[8 * i + j]) << narrow<u32>(8 * j));
            }
            m[i] = x;
        }
        array<u64, 16> v = [0; 16];
        for (usize i = 0; i < 8; i += 1)
        {
            v[i] = h[i];
            v[8 + i] = BLAKE2B_IV[i];
        }
        v[12] = v[12] ^ length;
        if (last)
        {
            v[14] = ~v[14];
        }
        for (usize r = 0; r < 12; r += 1)
        {
            usize o = 16 * (r % 10);
            for (usize g = 0; g < 8; g += 1)
            {
                usize a = widen<usize>(BLAKE2B_G[4 * g]);
                usize b = widen<usize>(BLAKE2B_G[4 * g + 1]);
                usize c = widen<usize>(BLAKE2B_G[4 * g + 2]);
                usize d = widen<usize>(BLAKE2B_G[4 * g + 3]);
                u64 x = m[widen<usize>(BLAKE2B_SIGMA[o + 2 * g])];
                u64 y = m[widen<usize>(BLAKE2B_SIGMA[o + 2 * g + 1])];
                v[a] = wrapping_add(wrapping_add(v[a], v[b]), x);
                v[d] = rotate_right(v[d] ^ v[a], 32);
                v[c] = wrapping_add(v[c], v[d]);
                v[b] = rotate_right(v[b] ^ v[c], 24);
                v[a] = wrapping_add(wrapping_add(v[a], v[b]), y);
                v[d] = rotate_right(v[d] ^ v[a], 16);
                v[c] = wrapping_add(v[c], v[d]);
                v[b] = rotate_right(v[b] ^ v[c], 63);
            }
        }
        array<u64, 8> out = h;
        for (usize i = 0; i < 8; i += 1)
        {
            out[i] = h[i] ^ v[i] ^ v[8 + i];
        }
        out
    }

    // `data` added to a state given and returned by value (as `sha256_absorb`).
    // The last block is kept back: `finish` compresses it with the final flag.
    fn blake2b_absorb(Blake2b s, slice<u8, shared> data) : Blake2b
    {
        usize at = 0;
        usize n = slice_len(data);
        while (at < n)
        {
            if (s.fill == 128)
            {
                s.length += 128;
                s.h = blake2b_block(s.h, s.block, s.length, false);
                s.fill = 0;
            }
            usize take = min(128 - s.fill, n - at);
            for (usize i = 0; i < take; i += 1)
            {
                s.block[s.fill + i] = data[at + i];
            }
            s.fill += take;
            at += take;
        }
        s
    }

    // A keyed BLAKE2b of `out_len` bytes (1 to 64) under `key` (up to 64
    // bytes, none for the plain hash); other lengths are `diag.crypto-length`.
    export fn Blake2b::new_keyed(slice<u8, shared> key, usize out_len) : Blake2b
    {
        usize klen = slice_len(key);
        if (out_len == 0 || out_len > 64 || klen > 64)
        {
            fault(crypto_length);
        }
        array<u64, 8> h = BLAKE2B_IV;
        h[0] = h[0] ^ 0x01010000 ^ (widen<u64>(klen) << 8) ^ widen<u64>(out_len);
        Blake2b s = Blake2b { .h = h, .block = [0; 128], .fill = 0, .length = 0, .out_len = out_len };
        if (klen > 0)
        {
            for (usize i = 0; i < klen; i += 1)
            {
                s.block[i] = key[i];
            }
            s.fill = 128;
        }
        s
    }

    export fn Blake2b::new(usize out_len) : Blake2b
    {
        Vec<u8> none = Vec::new();
        Blake2b::new_keyed(&none[0..$], out_len)
    }

    export fn Blake2b::update(ref<Blake2b, exclusive> s, slice<u8, shared> data)
    {
        *s = blake2b_absorb(*s, data);
    }

    // The digest of everything given: `out_len` bytes.
    export fn Blake2b::finish(Blake2b s) : Vec<u8>
    {
        s.length += widen<u64>(s.fill);
        for (usize i = s.fill; i < 128; i += 1)
        {
            s.block[i] = 0;
        }
        s.h = blake2b_block(s.h, s.block, s.length, true);
        Vec<u8> out = Vec::new();
        for (usize i = 0; i < s.out_len; i += 1)
        {
            Vec::push(&mut out, narrow_wrapping<u8>(s.h[i / 8] >> narrow<u32>(8 * (i % 8))));
        }
        out
    }

    // The BLAKE2b digest of `data`, `out_len` bytes (64 for BLAKE2b-512).
    export fn blake2b(slice<u8, shared> data, usize out_len) : Vec<u8>
    {
        Blake2b s = Blake2b::new(out_len);
        Blake2b::update(&mut s, data);
        Blake2b::finish(s)
    }

    // PBKDF2 (RFC 8018 §5.2) over HMAC of `kind`: `len` bytes from `password`
    // and `salt` after `iterations` rounds of HMAC per block; no iterations is
    // `diag.kdf-parameters`.
    export fn pbkdf2(
        HashKind kind,
        slice<u8, shared> password,
        slice<u8, shared> salt,
        usize iterations,
        usize len,
    ) : Vec<u8>
    {
        if (iterations == 0)
        {
            fault(kdf_parameters);
        }
        if (match (kind) { Sha2_256 : true, _ : false })
        {
            return pbkdf2_sha256(password, salt, iterations, len);
        }
        Hmac base = Hmac::new(kind, password);
        Vec<u8> out = Vec::new();
        u32 block = 1;
        while (Vec::len(&out) < len)
        {
            Hmac first = base;
            Hmac::update(&mut first, salt);
            Vec<u8> counter = Vec::new();
            Vec::push_be(&mut counter, block);
            Hmac::update(&mut first, &counter[0..$]);
            Vec<u8> u = Hmac::finish(first);
            Vec<u8> t = Vec::clone(&u);
            for (usize j = 1; j < iterations; j += 1)
            {
                Hmac m = base;
                Hmac::update(&mut m, &u[0..$]);
                overwrite(&mut u, Hmac::finish(m));
                for (usize i = 0; i < Vec::len(&t); i += 1)
                {
                    t[i] = t[i] ^ u[i];
                }
            }
            for (usize i = 0; i < Vec::len(&t) && Vec::len(&out) < len; i += 1)
            {
                Vec::push(&mut out, t[i]);
            }
            block += 1;
        }
        out
    }

    // `pbkdf2` over HMAC-SHA-256 (SCRAM's, among others): the same rounds on
    // fixed-size digests, with no `Vec` per round.
    fn pbkdf2_sha256(slice<u8, shared> password, slice<u8, shared> salt, usize iterations, usize len) : Vec<u8>
    {
        HmacSha256 base = HmacSha256::new(password);
        Vec<u8> out = Vec::new();
        u32 block = 1;
        while (Vec::len(&out) < len)
        {
            HmacSha256 first = base;
            HmacSha256::update(&mut first, salt);
            array<u8, 4> counter = [
                narrow_wrapping<u8>(block >> 24),
                narrow_wrapping<u8>(block >> 16),
                narrow_wrapping<u8>(block >> 8),
                narrow_wrapping<u8>(block),
            ];
            HmacSha256::update(&mut first, &counter[0..$]);
            array<u8, 32> u = HmacSha256::finish(first);
            array<u8, 32> t = u;
            for (usize j = 1; j < iterations; j += 1)
            {
                HmacSha256 m = base;
                HmacSha256::update(&mut m, &u[0..$]);
                u = HmacSha256::finish(m);
                for (usize i = 0; i < 32; i += 1)
                {
                    t[i] = t[i] ^ u[i];
                }
            }
            for (usize i = 0; i < 32 && Vec::len(&out) < len; i += 1)
            {
                Vec::push(&mut out, t[i]);
            }
            block += 1;
        }
        out
    }

    // Argon2's variable-length hash H' (RFC 9106 §3.3).
    fn argon2_h_prime(usize out_len, slice<u8, shared> x) : Vec<u8>
    {
        Vec<u8> prefix = Vec::new();
        Vec::push_le(&mut prefix, narrow<u32>(out_len));
        if (out_len <= 64)
        {
            Blake2b b = Blake2b::new(out_len);
            Blake2b::update(&mut b, &prefix[0..$]);
            Blake2b::update(&mut b, x);
            return Blake2b::finish(b);
        }
        usize r = (out_len + 31) / 32 - 2;
        Blake2b b = Blake2b::new(64);
        Blake2b::update(&mut b, &prefix[0..$]);
        Blake2b::update(&mut b, x);
        Vec<u8> v = Blake2b::finish(b);
        Vec<u8> out = Vec::from_slice(&v[0..32]);
        for (usize i = 1; i < r; i += 1)
        {
            overwrite(&mut v, blake2b(&v[0..$], 64));
            Vec::extend_from(&mut out, &v[0..32]);
        }
        Vec<u8> last = blake2b(&v[0..$], out_len - 32 * r);
        Vec::extend_from(&mut out, &last[0..$]);
        out
    }

    // Argon2's GB (RFC 9106 §3.6): BLAKE2b's G with a multiplication added.
    // The permutation P (RFC 9106 §3.6): BLAKE2b's round, its additions
    // carrying the products of the low halves, over the columns and the
    // diagonals of `v` (`BLAKE2B_G`'s positions). A value in and out.
    fn argon2_p(array<u64, 16> v) : array<u64, 16>
    {
        u64 lo = 0xffffffff;
        for (usize g = 0; g < 8; g += 1)
        {
            usize a = widen<usize>(BLAKE2B_G[4 * g]);
            usize b = widen<usize>(BLAKE2B_G[4 * g + 1]);
            usize c = widen<usize>(BLAKE2B_G[4 * g + 2]);
            usize d = widen<usize>(BLAKE2B_G[4 * g + 3]);
            v[a] = wrapping_add(wrapping_add(v[a], v[b]), wrapping_mul(2, (v[a] & lo) * (v[b] & lo)));
            v[d] = rotate_right(v[d] ^ v[a], 32);
            v[c] = wrapping_add(wrapping_add(v[c], v[d]), wrapping_mul(2, (v[c] & lo) * (v[d] & lo)));
            v[b] = rotate_right(v[b] ^ v[c], 24);
            v[a] = wrapping_add(wrapping_add(v[a], v[b]), wrapping_mul(2, (v[a] & lo) * (v[b] & lo)));
            v[d] = rotate_right(v[d] ^ v[a], 16);
            v[c] = wrapping_add(wrapping_add(v[c], v[d]), wrapping_mul(2, (v[c] & lo) * (v[d] & lo)));
            v[b] = rotate_right(v[b] ^ v[c], 63);
        }
        v
    }

    // The compression G(X, Y) (RFC 9106 §3.5): P over the rows, then the
    // columns, of X ⊕ Y, exclusive-ored with X ⊕ Y again.
    fn argon2_g(array<u64, 128> x, array<u64, 128> y) : array<u64, 128>
    {
        array<u64, 128> r = [0; 128];
        for (usize i = 0; i < 128; i += 1)
        {
            r[i] = x[i] ^ y[i];
        }
        array<u64, 128> q = r;
        for (usize row = 0; row < 8; row += 1)
        {
            array<u64, 16> v = [0; 16];
            for (usize i = 0; i < 16; i += 1)
            {
                v[i] = q[16 * row + i];
            }
            v = argon2_p(v);
            for (usize i = 0; i < 16; i += 1)
            {
                q[16 * row + i] = v[i];
            }
        }
        for (usize col = 0; col < 8; col += 1)
        {
            array<u64, 16> v = [0; 16];
            for (usize j = 0; j < 8; j += 1)
            {
                v[2 * j] = q[16 * j + 2 * col];
                v[2 * j + 1] = q[16 * j + 2 * col + 1];
            }
            v = argon2_p(v);
            for (usize j = 0; j < 8; j += 1)
            {
                q[16 * j + 2 * col] = v[2 * j];
                q[16 * j + 2 * col + 1] = v[2 * j + 1];
            }
        }
        for (usize i = 0; i < 128; i += 1)
        {
            q[i] = q[i] ^ r[i];
        }
        q
    }

    // A block's words out of memory, and back in (`xor`: combined with what
    // is there). The memory is reached through one slice per block, the
    // helper's only reference, whose element accesses need no check.
    fn argon2_get(slice<u64, shared> b) : array<u64, 128>
    {
        array<u64, 128> out = [0; 128];
        for (usize i = 0; i < 128; i += 1)
        {
            out[i] = b[i];
        }
        out
    }

    fn argon2_put(slice<u64, exclusive> b, array<u64, 128> v, bool xor)
    {
        for (usize i = 0; i < 128; i += 1)
        {
            b[i] = if (xor) { b[i] ^ v[i] } else { v[i] };
        }
    }

    fn argon2_load(ref<Vec<u64>, shared> mem, usize block) : array<u64, 128>
    {
        argon2_get(&mem[128 * block..128 * block + 128])
    }

    // Argon2id (RFC 9106, type 2, version 0x13): `len` bytes (at least 4)
    // from `password` and `salt` (at least 8 bytes) after `iterations` passes
    // (at least 1) over `memory_kib` kibibytes of memory (at least 8 per
    // lane) in `parallelism` lanes (1 to 255, computed one after another);
    // other parameters are `diag.kdf-parameters`. The first two slices of the
    // first pass address memory independently of the password, the rest
    // depend on it, as Argon2id prescribes.
    export fn argon2id(
        slice<u8, shared> password,
        slice<u8, shared> salt,
        usize iterations,
        usize memory_kib,
        usize parallelism,
        usize len,
    ) : Vec<u8>
    {
        if (iterations == 0 || parallelism == 0 || parallelism > 255 || memory_kib < 8 * parallelism || len < 4 || slice_len(salt) < 8)
        {
            fault(kdf_parameters);
        }
        usize lanes = parallelism;
        usize q = (memory_kib / (4 * lanes)) * 4;
        usize m_prime = q * lanes;
        usize seg = q / 4;
        // H0 (§3.2).
        Blake2b h = Blake2b::new(64);
        Vec<u8> head = Vec::new();
        Vec::push_le(&mut head, narrow<u32>(lanes));
        Vec::push_le(&mut head, narrow<u32>(len));
        Vec::push_le(&mut head, narrow<u32>(memory_kib));
        Vec::push_le(&mut head, narrow<u32>(iterations));
        Vec::push_le(&mut head, 0x13: u32);
        Vec::push_le(&mut head, 2: u32);
        Vec::push_le(&mut head, narrow<u32>(slice_len(password)));
        Blake2b::update(&mut h, &head[0..$]);
        Blake2b::update(&mut h, password);
        Vec<u8> mid = Vec::new();
        Vec::push_le(&mut mid, narrow<u32>(slice_len(salt)));
        Blake2b::update(&mut h, &mid[0..$]);
        Blake2b::update(&mut h, salt);
        array<u8, 8> none = [0; 8];
        Blake2b::update(&mut h, &none[0..$]);
        Vec<u8> h0 = Blake2b::finish(h);
        Vec<u64> mem = Vec::filled(128 * m_prime, 0: u64);
        for (usize l = 0; l < lanes; l += 1)
        {
            for (usize col = 0; col < 2; col += 1)
            {
                Vec<u8> input = Vec::clone(&h0);
                Vec::push_le(&mut input, narrow<u32>(col));
                Vec::push_le(&mut input, narrow<u32>(l));
                Vec<u8> block = argon2_h_prime(1024, &input[0..$]);
                for (usize i = 0; i < 128; i += 1)
                {
                    mem[128 * (l * q + col) + i] = read_le<u64>(&block[0..$], 8 * i);
                }
            }
        }
        array<u64, 128> zero = [0; 128];
        for (usize r = 0; r < iterations; r += 1)
        {
            for (usize s = 0; s < 4; s += 1)
            {
                for (usize l = 0; l < lanes; l += 1)
                {
                    bool independent = r == 0 && s < 2;
                    array<u64, 128> addresses = [0; 128];
                    array<u64, 128> input = [0; 128];
                    input[0] = widen<u64>(r);
                    input[1] = widen<u64>(l);
                    input[2] = widen<u64>(s);
                    input[3] = widen<u64>(m_prime);
                    input[4] = widen<u64>(iterations);
                    input[5] = 2;
                    usize start_i = if (r == 0 && s == 0) { 2 } else { 0 };
                    for (usize i = start_i; i < seg; i += 1)
                    {
                        usize j = s * seg + i;
                        usize prev = if (j == 0) { q - 1 } else { j - 1 };
                        u64 pseudo = 0;
                        if (independent)
                        {
                            if (i % 128 == 0 || i == start_i)
                            {
                                input[6] = widen<u64>(i / 128 + 1);
                                addresses = argon2_g(zero, argon2_g(zero, input));
                            }
                            pseudo = addresses[i % 128];
                        }
                        else
                        {
                            pseudo = mem[128 * (l * q + prev)];
                        }
                        u64 j1 = pseudo & 0xffffffff;
                        u64 j2 = pseudo >> 32;
                        usize ref_lane = if (r == 0 && s == 0) { l } else { narrow<usize>(j2 % widen<u64>(lanes)) };
                        usize w = 0;
                        if (r == 0)
                        {
                            w = if (ref_lane == l) { s * seg + i - 1 } else { s * seg - (if (i == 0) { 1 } else { 0 }) };
                        }
                        else
                        {
                            w = if (ref_lane == l) { q - seg + i - 1 } else { q - seg - (if (i == 0) { 1 } else { 0 }) };
                        }
                        u64 x = (j1 * j1) >> 32;
                        u64 y = narrow<u64>((widen<u128>(w) * widen<u128>(x)) >> 32);
                        usize z = w - 1 - narrow<usize>(y);
                        usize start = if (r == 0) { 0 } else { ((s + 1) % 4) * seg };
                        usize ref_col = (start + z) % q;
                        array<u64, 128> fresh = argon2_g(
                            argon2_load(&mem, l * q + prev),
                            argon2_load(&mem, ref_lane * q + ref_col),
                        );
                        usize at = 128 * (l * q + j);
                        argon2_put(&mut mem[at..at + 128], fresh, r > 0);
                    }
                }
            }
        }
        array<u64, 128> c = [0; 128];
        for (usize l = 0; l < lanes; l += 1)
        {
            for (usize k = 0; k < 128; k += 1)
            {
                c[k] = c[k] ^ mem[128 * (l * q + q - 1) + k];
            }
        }
        Vec<u8> c_bytes = Vec::new();
        for (usize k = 0; k < 128; k += 1)
        {
            Vec::push_le(&mut c_bytes, c[k]);
        }
        argon2_h_prime(len, &c_bytes[0..$])
    }

    // Base64 without padding, as PHC strings carry it.
    fn b64_unpadded(slice<u8, shared> bytes) : String
    {
        String s = base64_encode(bytes);
        usize n = String::len(&s);
        while (n > 0 && String::as_bytes(&s)[n - 1] == 0x3d)
        {
            n -= 1;
        }
        String::truncate(&mut s, n);
        s
    }

    fn b64_decode_unpadded(StringView text) : Option<Vec<u8>>
    {
        String padded = String::from_view(text);
        while (String::len(&padded) % 4 != 0)
        {
            String::push_ascii(&mut padded, 0x3d);
        }
        match (base64_decode(String::as_view(&padded)))
        {
            Ok(v)  : Some(v),
            Err(_) : None,
        }
    }

    // `password` hashed for storage: Argon2id with a fresh 16-byte salt, 19
    // MiB of memory, two passes and one lane (the OWASP recommendation), as
    // the PHC string `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`, which
    // `verify_password` and every password library read.
    export fn hash_password(slice<u8, shared> password) : String
    {
        array<u8, 16> salt = [0; 16];
        os_random_bytes(&mut salt[0..$]);
        Vec<u8> digest = argon2id(password, &salt[0..$], 2, 19456, 1, 32);
        String out = String::from_str("$argon2id$v=19$m=19456,t=2,p=1$");
        String::append(&mut out, String::as_view(&b64_unpadded(&salt[0..$])));
        String::append(&mut out, "$");
        String::append(&mut out, String::as_view(&b64_unpadded(&digest[0..$])));
        out
    }

    // Whether `password` is the one `stored`, a PHC string of `hash_password`
    // or of any Argon2id implementation, was made from; the parameters are
    // taken from the string. A string that is not such a hash is false.
    export fn verify_password(StringView stored, slice<u8, shared> password) : bool
    {
        Vec<StringView> parts = StringView::split(stored, "$");
        if (Vec::len(&parts) != 6 || StringView::len(parts[0]) != 0 || !StringView::eq(
            parts[1],
            "argon2id",
        ) || !StringView::eq(parts[2], "v=19"))
        {
            return false;
        }
        Vec<StringView> params = StringView::split(parts[3], ",");
        if (Vec::len(&params) != 3 || !StringView::starts_with(
            params[0],
            "m=",
        ) || !StringView::starts_with(params[1], "t=") || !StringView::starts_with(params[2], "p="))
        {
            return false;
        }
        usize m = match (StringView::parse<usize>(StringView::sub(params[0], 2, StringView::len(params[0]))))
        {
            Ok(v)  : v,
            Err(_) : return false,
        };
        usize t = match (StringView::parse<usize>(StringView::sub(params[1], 2, StringView::len(params[1]))))
        {
            Ok(v)  : v,
            Err(_) : return false,
        };
        usize p = match (StringView::parse<usize>(StringView::sub(params[2], 2, StringView::len(params[2]))))
        {
            Ok(v)  : v,
            Err(_) : return false,
        };
        Vec<u8> salt = match (b64_decode_unpadded(parts[4]))
        {
            Some(v) : v,
            None    : return false,
        };
        Vec<u8> want = match (b64_decode_unpadded(parts[5]))
        {
            Some(v) : v,
            None    : return false,
        };
        if (t == 0 || p == 0 || p > 255 || m < 8 * p || Vec::len(&want) < 4 || Vec::len(&salt) < 8)
        {
            return false;
        }
        Vec<u8> got = argon2id(password, &salt[0..$], t, m, p, Vec::len(&want));
        digest_eq(&got[0..$], &want[0..$])
    }

    // D-0119: CRC-32/IEEE (polynomial 0x04C11DB7, reflected 0xEDB88320,
    // initial value and final xor 0xFFFFFFFF, input and output reflected):
    // the checksum of zlib, PNG, gzip and Ethernet; crc32("123456789") is
    // 0xCBF43926. Bit by bit, as spec/21 `[CRC32]` writes it. Here beside the
    // hashes (D-0177), but not cryptographic: it detects accidental corruption,
    // and anyone can make data with a chosen CRC.
    export fn crc32(slice<u8, shared> data) : u32
    {
        u32 crc = 0xFFFFFFFF;
        for (usize i = 0; i < slice_len(data); i += 1)
        {
            crc = crc ^ widen<u32>(data[i]);
            for (u32 k = 0; k < 8; k += 1)
            {
                if ((crc & 1) == 1)
                {
                    crc = (crc >> 1) ^ 0xEDB88320;
                }
                else
                {
                    crc = crc >> 1;
                }
            }
        }
        crc ^ 0xFFFFFFFF
    }

**Depends on:** rule.stdlib.text, rule.arith.checked, rule.arith.bitwise, D-0151
**Affects:** none (pure)

## 2n. Certificates

In `std::x509` (D-0160).

### `rule.stdlib.x509`
**Status:** ACCEPTED

X.509 certificates (RFC 5280) as a TLS client verifies them: parsed from
DER, gathered from PEM, and checked as a chain from a server's leaf to a
trusted root for a host at a time.

    [Der-Read]         ⟨der_read(b, at), Σ⟩ → Some(the element at `at`: its tag, its contents' range, the next
                       position) when it is well-formed DER (a single-byte tag, the shortest length form, at
                       most four length bytes, contents inside b); None otherwise
    [Cert-Parse]       ⟨Certificate::parse(d), Σ⟩ → Ok(c) when d is one DER certificate (v3 or older), with the
                       inner and outer signature algorithms equal; c gives the signed part, the issuer's and
                       subject's DER, the last common name, the validity (UTCTime 50–99 as 19xx), the
                       subject alternative names' DNS names and IP addresses, basic constraints, key usage's
                       keyCertSign, extended key usage's server authentication (or any), the key and the
                       signature scheme; an unknown critical extension (other than certificate policies or
                       name constraints) marks c unusable; Err(BadEncoding) for anything else; a key or
                       signature algorithm of OID 1.3.101.112 is Ed25519Key / Ed25519 (D-0171)
    [Host-Match]       ⟨Certificate::matches_host(c, h), Σ⟩ → for an IP address literal h, whether c lists that
                       address; otherwise whether a DNS name of c equals h ignoring ASCII case and one final
                       dot, a leading "*." standing for exactly one whole label (RFC 6125 §6.4); the common
                       name is not consulted
    [Verify-Chain]     ⟨verify_chain(leaf, mids, roots, h, now), Σ⟩ → Ok(()) when leaf matches h, may serve
                       TLS, is in date at now and usable, and a sequence leaf = c0, c1, …, ck of at most ten
                       exists where each c(i+1) has c(i)'s issuer as its subject and its key verifies c(i)'s
                       signature, c1 … c(k-1) are among mids, ck is among roots, and every issuer is a CA
                       allowed to sign certificates, in date, within its path length; otherwise Err of the
                       first reason met, in the order HostMismatch, WrongUsage, Expired or NotYetValid or
                       UnhandledCritical, then the chain's
    [Trust-Store]      TrustStore::system reads the first of the usual Unix bundle files that holds a
                       certificate; Err(NotFound) where none does (Windows); from_pem_file reads one file
    [Key-Der]          (D-0171)   ⟨CertKey::from_der(d), Σ⟩ → Ok(the key of the SubjectPublicKeyInfo d),
                       Err(BadEncoding) when d is not one, Err(UnsupportedAlgorithm) when its algorithm
                       is not RSA, P-256, P-384 or Ed25519; to_der gives the SubjectPublicKeyInfo
                       from_der reads back (None for UnsupportedKey); ⟨PrivateKey::from_der(d), Σ⟩ →
                       Ok(the key) when d is a PKCS #8 PrivateKeyInfo (RFC 5958), a PKCS #1 RSAPrivateKey
                       or a SEC 1 ECPrivateKey (its curve from its own parameters) whose numbers satisfy
                       [Rsa-Private-Key] or [Ec-Keys], Err(BadEncoding) or Err(UnsupportedAlgorithm)
                       otherwise; to_der gives the PKCS #8 (version 0) form from_der reads back;
                       public_key gives the matching CertKey
    [Key-Pem]          (D-0171, RFC 7468)   from_pem reads the first PUBLIC KEY block (CertKey), or the
                       first PRIVATE KEY, RSA PRIVATE KEY or EC PRIVATE KEY block (PrivateKey), as
                       from_der; an ENCRYPTED PRIVATE KEY alone is Err(UnsupportedAlgorithm), no block
                       Err(BadEncoding); to_pem writes to_der in a PUBLIC KEY or PRIVATE KEY block of
                       64-character lines
    [Sign]             (D-0171)   ⟨sign(key, scheme, m), Σ⟩ → Some(sig) with verify_signature(key's
                       public key, scheme, m, sig) true: RSA PKCS #1 v1.5 or PSS over hash(h, m), ECDSA
                       over hash(h, m) ([Ecdsa-Sign]), Ed25519 over m; None when key is not of the
                       scheme's kind or the scheme is UnsupportedSignature

The `std` source:

    // `std::x509` (spec/21 §2n, D-0160): DER, X.509 certificates, and
    // verifying that a server's certificate chains to a trusted root and names
    // the server; then (D-0171) public and private keys in their standard
    // encodings (SubjectPublicKeyInfo, PKCS #8, PEM) and signing with them.
    // Written in CobaltC over `std::crypto`. Nothing here makes or signs a
    // certificate.

    // Why a certificate or a chain was refused.
    export enum CertError
    {
        BadEncoding,
        Expired,
        NotYetValid,
        UnknownAuthority,
        HostMismatch,
        BadSignature,
        NotAuthority,
        UnsupportedAlgorithm,
        UnhandledCritical,
        WrongUsage,
        ChainTooLong,
    }

    export fn CertError::text(ref<CertError, shared> e) : str
    {
        match (e)
        {
            BadEncoding          : "malformed certificate",
            Expired              : "certificate expired",
            NotYetValid          : "certificate not yet valid",
            UnknownAuthority     : "certificate signed by an unknown authority",
            HostMismatch         : "certificate is not for this host",
            BadSignature         : "certificate signature does not verify",
            NotAuthority         : "issuer is not a certificate authority",
            UnsupportedAlgorithm : "unsupported key or signature algorithm",
            UnhandledCritical    : "unhandled critical extension",
            WrongUsage           : "certificate not usable for a TLS server",
            ChainTooLong         : "certificate chain too long",
        }
    }

    // One DER element (ITU-T X.690): its tag byte and where its contents lie
    // in the buffer it was read from, and where the next element begins.
    export struct Der
    {
        export u8 tag;
        export usize start;
        export usize end;
        export usize next;
    }

    // The element at `at` in `buf`, or None when it is not well-formed DER
    // (a length in the long form when the short would do, more than four
    // length bytes, the indefinite form, or contents past the buffer).
    export fn der_read(slice<u8, shared> buf, usize at) : Option<Der>
    {
        usize n = slice_len(buf);
        if (at + 2 > n)
        {
            return None;
        }
        u8 tag = buf[at];
        if ((tag & 0x1f) == 0x1f)
        {
            return None;
        }
        u8 first = buf[at + 1];
        usize len = 0;
        usize start = at + 2;
        if (first < 0x80)
        {
            len = widen<usize>(first);
        }
        else
        {
            usize count = widen<usize>(first & 0x7f);
            if (count == 0 || count > 4 || start + count > n || buf[start] == 0)
            {
                return None;
            }
            for (usize i = 0; i < count; i += 1)
            {
                len = (len << 8) | widen<usize>(buf[start + i]);
            }
            if (len < 0x80)
            {
                return None;
            }
            start += count;
        }
        if (len > n - start)
        {
            return None;
        }
        Some(Der { .tag = tag, .start = start, .end = start + len, .next = start + len })
    }

    // The element at `at`, which must have tag `tag`.
    fn der_expect(slice<u8, shared> buf, usize at, u8 tag) : Result<Der, CertError>
    {
        match (der_read(buf, at))
        {
            Some(d) : if (d.tag == tag) { Ok(d) } else { Err(BadEncoding) },
            None    : Err(BadEncoding),
        }
    }

    fn der_bytes(slice<u8, shared> buf, Der d) : Vec<u8>
    {
        Vec::from_slice(&buf[d.start..d.end])
    }

    fn der_eq_hex(slice<u8, shared> buf, Der d, str hex) : bool
    {
        Vec<u8> want = Result::unwrap_or(from_hex(StringView::of(hex)), Vec::new());
        d.end - d.start == Vec::len(&want) && digest_eq(&buf[d.start..d.end], &want[0..$])
    }

    // An elliptic-curve public key: the curve and the uncompressed point.
    export struct EcPublicKey
    {
        export EcCurve curve;
        export Vec<u8> point;
    }

    // The key in a certificate.
    export enum CertKey
    {
        RsaKey(RsaPublicKey),
        EcKey(EcPublicKey),
        Ed25519Key(array<u8, 32>),
        UnsupportedKey,
    }

    // How a certificate (or a TLS handshake) is signed.
    export enum SignatureScheme
    {
        RsaPkcs1(HashKind),
        RsaPss(HashKind),
        Ecdsa(HashKind),
        Ed25519,
        UnsupportedSignature,
    }

    // A parsed X.509 v3 certificate (RFC 5280). `tbs` is the signed part;
    // `issuer` and `subject` are their names' DER, compared whole.
    export struct Certificate
    {
        export Vec<u8> der;
        export Vec<u8> tbs;
        export Vec<u8> issuer;
        export Vec<u8> subject;
        export String common_name;
        export DateTime not_before;
        export DateTime not_after;
        export Vec<String> dns_names;
        export Vec<IpAddr> ip_addresses;
        export bool is_ca;
        export i64 max_path_len;
        export bool can_sign_certificates;
        export bool server_auth;
        export bool unhandled_critical;
        export CertKey key;
        export SignatureScheme scheme;
        export Vec<u8> signature;
    }

    fn hash_oid(slice<u8, shared> buf, Der d) : Option<HashKind>
    {
        if (der_eq_hex(buf, d, "608648016503040201"))
        {
            return Some(Sha2_256);
        }
        if (der_eq_hex(buf, d, "608648016503040202"))
        {
            return Some(Sha2_384);
        }
        if (der_eq_hex(buf, d, "608648016503040203"))
        {
            return Some(Sha2_512);
        }
        None
    }

    // An AlgorithmIdentifier as a signature scheme.
    fn signature_algorithm(slice<u8, shared> buf, Der alg) : Result<SignatureScheme, CertError>
    {
        Der oid = der_expect(buf, alg.start, 0x06)?;
        if (der_eq_hex(buf, oid, "2a864886f70d01010b"))
        {
            return Ok(RsaPkcs1(Sha2_256));
        }
        if (der_eq_hex(buf, oid, "2a864886f70d01010c"))
        {
            return Ok(RsaPkcs1(Sha2_384));
        }
        if (der_eq_hex(buf, oid, "2a864886f70d01010d"))
        {
            return Ok(RsaPkcs1(Sha2_512));
        }
        if (der_eq_hex(buf, oid, "2a8648ce3d040302"))
        {
            return Ok(Ecdsa(Sha2_256));
        }
        if (der_eq_hex(buf, oid, "2a8648ce3d040303"))
        {
            return Ok(Ecdsa(Sha2_384));
        }
        if (der_eq_hex(buf, oid, "2a8648ce3d040304"))
        {
            return Ok(Ecdsa(Sha2_512));
        }
        if (der_eq_hex(buf, oid, "2b6570"))
        {
            return Ok(Ed25519);
        }
        if (der_eq_hex(buf, oid, "2a864886f70d01010a"))
        {
            // RSASSA-PSS-params: [0] hashAlgorithm, [1] maskGenAlgorithm,
            // [2] saltLength; the hash must be SHA-2 and MGF1 over it, and the
            // salt as long as the digest.
            Der params = der_expect(buf, oid.next, 0x30)?;
            Der h0 = der_expect(buf, params.start, 0xa0)?;
            Der h_alg = der_expect(buf, h0.start, 0x30)?;
            Der h_oid = der_expect(buf, h_alg.start, 0x06)?;
            HashKind kind = match (hash_oid(buf, h_oid))
            {
                Some(k) : k,
                None    : return Ok(UnsupportedSignature),
            };
            return Ok(RsaPss(kind));
        }
        Ok(UnsupportedSignature)
    }

    // The text of a DirectoryString-like value (UTF8String, PrintableString,
    // IA5String, or the T61String some old roots use for ASCII).
    fn der_text(slice<u8, shared> buf, Der d) : Option<String>
    {
        if (d.tag != 0x0c && d.tag != 0x13 && d.tag != 0x16 && d.tag != 0x14)
        {
            return None;
        }
        match (String::from_utf8(der_bytes(buf, d)))
        {
            Ok(s)  : Some(s),
            Err(_) : None,
        }
    }

    // The last commonName (2.5.4.3) in a Name, or "".
    fn common_name_of(slice<u8, shared> buf, Der name) : String
    {
        String cn = String::new();
        usize at = name.start;
        while (at < name.end)
        {
            Der set = match (der_read(buf, at))
            {
                Some(d) : d,
                None    : return cn,
            };
            usize inner = set.start;
            while (inner < set.end)
            {
                Der atv = match (der_read(buf, inner))
                {
                    Some(d) : d,
                    None    : return cn,
                };
                if (Some(oid) = der_read(buf, atv.start))
                {
                    if (oid.tag == 0x06 && der_eq_hex(buf, oid, "550403"))
                    {
                        if (Some(v) = der_read(buf, oid.next))
                        {
                            if (Some(t) = der_text(buf, v))
                            {
                                overwrite(&mut cn, t);
                            }
                        }
                    }
                }
                inner = atv.next;
            }
            at = set.next;
        }
        cn
    }

    fn digits(slice<u8, shared> b, usize at, usize n) : Option<i64>
    {
        i64 x = 0;
        for (usize i = at; i < at + n; i += 1)
        {
            if (i >= slice_len(b) || !ascii_is_digit(b[i]))
            {
                return None;
            }
            x = x * 10 + widen<i64>(b[i] - b'0');
        }
        Some(x)
    }

    // UTCTime (YYMMDDHHMMSSZ; 50–99 are 19xx) or GeneralizedTime
    // (YYYYMMDDHHMMSSZ), as RFC 5280 §4.1.2.5 restricts them.
    fn der_time(slice<u8, shared> buf, Der d) : Result<DateTime, CertError>
    {
        slice<u8, shared> t = &buf[d.start..d.end];
        usize n = slice_len(t);
        i64 year = 0;
        usize at = 0;
        if (d.tag == 0x17 && n == 13)
        {
            i64 yy = match (digits(t, 0, 2))
            {
                Some(y) : y,
                None    : return Err(BadEncoding),
            };
            year = if (yy < 50) { 2000 + yy } else { 1900 + yy };
            at = 2;
        }
        else if (d.tag == 0x18 && n == 15)
        {
            year = match (digits(t, 0, 4))
            {
                Some(y) : y,
                None    : return Err(BadEncoding),
            };
            at = 4;
        }
        else
        {
            return Err(BadEncoding);
        }
        if (t[n - 1] != b'Z')
        {
            return Err(BadEncoding);
        }
        Option<i64> mo = digits(t, at, 2);
        Option<i64> dd = digits(t, at + 2, 2);
        Option<i64> hh = digits(t, at + 4, 2);
        Option<i64> mi = digits(t, at + 6, 2);
        Option<i64> ss = digits(t, at + 8, 2);
        if (Some(m) = mo)
        {
            if (Some(day) = dd)
            {
                if (Some(h) = hh)
                {
                    if (Some(min) = mi)
                    {
                        if (Some(sec) = ss)
                        {
                            if (m <= 12 && day <= 31 && h <= 23 && min <= 59 && sec <= 59)
                            {
                                if (Some(dt) = DateTime::new(
                                    year,
                                    narrow<u8>(m),
                                    narrow<u8>(day),
                                    narrow<u8>(h),
                                    narrow<u8>(min),
                                    narrow<u8>(sec),
                                ))
                                {
                                    return Ok(dt);
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(BadEncoding)
    }

    fn public_key_of(slice<u8, shared> buf, Der spki) : Result<CertKey, CertError>
    {
        Der alg = der_expect(buf, spki.start, 0x30)?;
        Der bits = der_expect(buf, alg.next, 0x03)?;
        if (bits.end == bits.start || buf[bits.start] != 0)
        {
            return Err(BadEncoding);
        }
        usize key_start = bits.start + 1;
        Der oid = der_expect(buf, alg.start, 0x06)?;
        if (der_eq_hex(buf, oid, "2a864886f70d010101"))
        {
            Der seq = der_expect(buf, key_start, 0x30)?;
            Der n = der_expect(buf, seq.start, 0x02)?;
            Der e = der_expect(buf, n.next, 0x02)?;
            return Ok(RsaKey(RsaPublicKey { .n = BigUint::from_bytes_be(&buf[n.start..n.end]), .e = BigUint::from_bytes_be(&buf[e.start..e.end]) }));
        }
        if (der_eq_hex(buf, oid, "2a8648ce3d0201"))
        {
            Der curve = der_expect(buf, oid.next, 0x06)?;
            Vec<u8> point = Vec::from_slice(&buf[key_start..bits.end]);
            if (der_eq_hex(buf, curve, "2a8648ce3d030107"))
            {
                return Ok(EcKey(EcPublicKey { .curve = P256, .point = point }));
            }
            if (der_eq_hex(buf, curve, "2b81040022"))
            {
                return Ok(EcKey(EcPublicKey { .curve = P384, .point = point }));
            }
        }
        if (der_eq_hex(buf, oid, "2b6570"))
        {
            if (bits.end - key_start != 32)
            {
                return Err(BadEncoding);
            }
            array<u8, 32> k = [0; 32];
            for (usize i = 0; i < 32; i += 1)
            {
                k[i] = buf[key_start + i];
            }
            return Ok(Ed25519Key(k));
        }
        Ok(UnsupportedKey)
    }

    // Parses one certificate's DER (RFC 5280 §4.1).
    export fn Certificate::parse(slice<u8, shared> der) : Result<Certificate, CertError>
    {
        Der cert = der_expect(der, 0, 0x30)?;
        if (cert.next != slice_len(der))
        {
            return Err(BadEncoding);
        }
        Der tbs = der_expect(der, cert.start, 0x30)?;
        Der outer_alg = der_expect(der, tbs.next, 0x30)?;
        Der sig_bits = der_expect(der, outer_alg.next, 0x03)?;
        if (sig_bits.next != cert.end || sig_bits.end == sig_bits.start || der[sig_bits.start] != 0)
        {
            return Err(BadEncoding);
        }
        SignatureScheme scheme = signature_algorithm(der, outer_alg)?;
        usize at = tbs.start;
        Der first = match (der_read(der, at))
        {
            Some(d) : d,
            None    : return Err(BadEncoding),
        };
        if (first.tag == 0xa0)
        {
            at = first.next;
        }
        Der serial = der_expect(der, at, 0x02)?;
        Der inner_alg = der_expect(der, serial.next, 0x30)?;
        // The signature algorithm inside the signed part must be the outer one.
        if (inner_alg.end - inner_alg.start != outer_alg.end - outer_alg.start || !digest_eq(
            &der[inner_alg.start..inner_alg.end],
            &der[outer_alg.start..outer_alg.end],
        ))
        {
            return Err(BadEncoding);
        }
        Der issuer = der_expect(der, inner_alg.next, 0x30)?;
        Der validity = der_expect(der, issuer.next, 0x30)?;
        Der nb = match (der_read(der, validity.start))
        {
            Some(d) : d,
            None    : return Err(BadEncoding),
        };
        Der na = match (der_read(der, nb.next))
        {
            Some(d) : d,
            None    : return Err(BadEncoding),
        };
        DateTime not_before = der_time(der, nb)?;
        DateTime not_after = der_time(der, na)?;
        Der subject = der_expect(der, validity.next, 0x30)?;
        Der spki = der_expect(der, subject.next, 0x30)?;
        CertKey key = public_key_of(der, spki)?;
        Certificate c = Certificate { .der = Vec::from_slice(der), .tbs = Vec::from_slice(&der[cert.start..tbs.next]), .issuer = Vec::from_slice(&der[inner_alg.next..issuer.next]), .subject = Vec::from_slice(&der[validity.next..subject.next]), .common_name = common_name_of(
            der,
            subject,
        ), .not_before = not_before, .not_after = not_after, .dns_names = Vec::new(), .ip_addresses = Vec::new(), .is_ca = false, .max_path_len = -1, .can_sign_certificates = true, .server_auth = true, .unhandled_critical = false, .key = key, .scheme = scheme, .signature = Vec::from_slice(&der[sig_bits.start + 1..sig_bits.end]) };
        at = spki.next;
        while (at < tbs.end)
        {
            Der field = match (der_read(der, at))
            {
                Some(d) : d,
                None    : return Err(BadEncoding),
            };
            if (field.tag == 0xa3)
            {
                Der exts = der_expect(der, field.start, 0x30)?;
                usize e_at = exts.start;
                while (e_at < exts.end)
                {
                    Der ext = der_expect(der, e_at, 0x30)?;
                    Der oid = der_expect(der, ext.start, 0x06)?;
                    usize v_at = oid.next;
                    bool critical = false;
                    Der maybe = match (der_read(der, v_at))
                    {
                        Some(d) : d,
                        None    : return Err(BadEncoding),
                    };
                    if (maybe.tag == 0x01)
                    {
                        critical = maybe.end > maybe.start && der[maybe.start] != 0;
                        v_at = maybe.next;
                    }
                    Der value = der_expect(der, v_at, 0x04)?;
                    if (der_eq_hex(der, oid, "551d11"))
                    {
                        Der names = der_expect(der, value.start, 0x30)?;
                        usize n_at = names.start;
                        while (n_at < names.end)
                        {
                            Der g = match (der_read(der, n_at))
                            {
                                Some(d) : d,
                                None    : return Err(BadEncoding),
                            };
                            if (g.tag == 0x82)
                            {
                                if (Ok(s) = String::from_utf8(der_bytes(der, g)))
                                {
                                    Vec::push(&mut c.dns_names, s);
                                }
                            }
                            else if (g.tag == 0x87)
                            {
                                usize len = g.end - g.start;
                                if (len == 4)
                                {
                                    Vec::push(
                                        &mut c.ip_addresses,
                                        V4([der[g.start], der[g.start + 1], der[g.start + 2], der[g.start + 3]]),
                                    );
                                }
                                else if (len == 16)
                                {
                                    array<u8, 16> a = [0; 16];
                                    for (usize i = 0; i < 16; i += 1)
                                    {
                                        a[i] = der[g.start + i];
                                    }
                                    Vec::push(&mut c.ip_addresses, V6(a));
                                }
                            }
                            n_at = g.next;
                        }
                    }
                    else if (der_eq_hex(der, oid, "551d13"))
                    {
                        Der bc = der_expect(der, value.start, 0x30)?;
                        usize b_at = bc.start;
                        if (b_at < bc.end)
                        {
                            Der x = match (der_read(der, b_at))
                            {
                                Some(d) : d,
                                None    : return Err(BadEncoding),
                            };
                            if (x.tag == 0x01)
                            {
                                c.is_ca = x.end > x.start && der[x.start] != 0;
                                b_at = x.next;
                            }
                        }
                        if (b_at < bc.end)
                        {
                            Der pl = der_expect(der, b_at, 0x02)?;
                            i64 v = 0;
                            for (usize i = pl.start; i < pl.end && i < pl.start + 4; i += 1)
                            {
                                v = (v << 8) | widen<i64>(der[i]);
                            }
                            c.max_path_len = v;
                        }
                    }
                    else if (der_eq_hex(der, oid, "551d0f"))
                    {
                        Der ku = der_expect(der, value.start, 0x03)?;
                        // Bit 5, keyCertSign, of the first byte after the
                        // unused-bits count.
                        c.can_sign_certificates = ku.end - ku.start >= 2 && (der[ku.start + 1] & 0x04) != 0;
                    }
                    else if (der_eq_hex(der, oid, "551d25"))
                    {
                        Der eku = der_expect(der, value.start, 0x30)?;
                        bool server = false;
                        usize k_at = eku.start;
                        while (k_at < eku.end)
                        {
                            Der p = der_expect(der, k_at, 0x06)?;
                            if (der_eq_hex(der, p, "2b06010505070301") || der_eq_hex(der, p, "551d2500"))
                            {
                                server = true;
                            }
                            k_at = p.next;
                        }
                        c.server_auth = server;
                    }
                    else if (critical && !der_eq_hex(der, oid, "551d20") && !der_eq_hex(der, oid, "551d1e"))
                    {
                        // An unknown critical extension; certificate policies
                        // and name constraints are tolerated unread (D-0160).
                        c.unhandled_critical = true;
                    }
                    e_at = ext.next;
                }
            }
            at = field.next;
        }
        Ok(c)
    }

    // Every certificate in PEM text (RFC 7468's `-----BEGIN CERTIFICATE-----`
    // blocks); text between blocks is ignored. A block that is not base64,
    // or not a certificate, is skipped.
    export fn certificates_from_pem(StringView text) : Vec<Certificate>
    {
        Vec<Certificate> out = Vec::new();
        StringView begin = "-----BEGIN CERTIFICATE-----";
        StringView end = "-----END CERTIFICATE-----";
        usize at = 0;
        usize n = StringView::len(text);
        while (at < n)
        {
            StringView rest = StringView::sub(text, at, n);
            usize b = match (StringView::find(rest, begin))
            {
                Some(i) : i,
                None    : break,
            };
            StringView body = StringView::sub(rest, b + StringView::len(begin), StringView::len(rest));
            usize e = match (StringView::find(body, end))
            {
                Some(i) : i,
                None    : break,
            };
            String b64 = String::new();
            StringView block = StringView::sub(body, 0, e);
            for (usize i = 0; i < StringView::len(block); i += 1)
            {
                u8 c = block.bytes[i];
                if (!ascii_is_space(c))
                {
                    String::push_ascii(&mut b64, c);
                }
            }
            if (Ok(der) = base64_decode(String::as_view(&b64)))
            {
                if (Ok(cert) = Certificate::parse(&der[0..$]))
                {
                    Vec::push(&mut out, cert);
                }
            }
            at = at + b + StringView::len(begin) + e + StringView::len(end);
        }
        out
    }

    // The roots a chain may end at.
    export struct TrustStore
    {
        export Vec<Certificate> roots;
    }

    export fn TrustStore::new() : TrustStore
    {
        TrustStore { .roots = Vec::new() }
    }

    export fn TrustStore::add(ref<TrustStore, exclusive> t, Certificate root)
    {
        Vec::push(&mut t.roots, root);
    }

    // The roots in a PEM file (`/etc/ssl/certs/ca-certificates.crt` and the
    // like); `Err` when the file cannot be read.
    export fn TrustStore::from_pem_file(StringView path) : Result<TrustStore, FileError>
    {
        String text = read_file(path)?;
        Ok(TrustStore { .roots = certificates_from_pem(String::as_view(&text)) })
    }

    // The operating system's roots, from the first of the usual bundle files
    // that exists (Debian and Ubuntu, Fedora and RHEL, Alpine, macOS with a
    // package manager's OpenSSL). `Err(NotFound)` where there is none, as on
    // Windows: a program there passes a bundle to `from_pem_file`.
    export fn TrustStore::system() : Result<TrustStore, FileError>
    {
        array<str, 5> paths = [
            "/etc/ssl/certs/ca-certificates.crt",
            "/etc/pki/tls/certs/ca-bundle.crt",
            "/etc/ssl/ca-bundle.pem",
            "/etc/ssl/cert.pem",
            "/usr/local/etc/openssl/cert.pem",
        ];
        foreach (p in &paths)
        {
            if (Ok(t) = TrustStore::from_pem_file(StringView::of(*p)))
            {
                if (Vec::len(&t.roots) > 0)
                {
                    return Ok(t);
                }
            }
        }
        Err(NotFound)
    }

    // Whether `signature` over `message` verifies under `key` with `scheme`.
    export fn verify_signature(
        ref<CertKey, shared> key,
        ref<SignatureScheme, shared> scheme,
        slice<u8, shared> message,
        slice<u8, shared> signature,
    ) : bool
    {
        match (scheme)
        {
            RsaPkcs1(h) : match (key)
            {
                RsaKey(k) : rsa_verify_pkcs1v15(k, *h, &hash(*h, message)[0..$], signature),
                _         : false,
            },
            RsaPss(h) : match (key)
            {
                RsaKey(k) : rsa_verify_pss(k, *h, &hash(*h, message)[0..$], signature),
                _         : false,
            },
            Ecdsa(h) : match (key)
            {
                EcKey(k) : ecdsa_verify(k.curve, &k.point[0..$], &hash(*h, message)[0..$], signature),
                _        : false,
            },
            Ed25519 : match (key)
            {
                Ed25519Key(k) : ed25519_verify(&k[0..$], message, signature),
                _             : false,
            },
            UnsupportedSignature : false,
        }
    }

    fn ascii_lower_eq(StringView a, StringView b) : bool
    {
        if (StringView::len(a) != StringView::len(b))
        {
            return false;
        }
        for (usize i = 0; i < StringView::len(a); i += 1)
        {
            u8 x = a.bytes[i];
            u8 y = b.bytes[i];
            if (ascii_is_upper(x))
            {
                x += 32;
            }
            if (ascii_is_upper(y))
            {
                y += 32;
            }
            if (x != y)
            {
                return false;
            }
        }
        true
    }

    // RFC 6125 §6.4: a name matches the host case-insensitively; `*` stands
    // for exactly one whole leftmost label, and never for an IP address.
    fn name_matches(StringView pattern, StringView host) : bool
    {
        if (StringView::starts_with(pattern, "*."))
        {
            StringView suffix = StringView::sub(pattern, 1, StringView::len(pattern));
            match (StringView::find(host, "."))
            {
                Some(dot) : dot > 0 && ascii_lower_eq(
                    suffix,
                    StringView::sub(host, dot, StringView::len(host)),
                ) && Option::is_some(&StringView::find(StringView::sub(suffix, 1, StringView::len(suffix)), ".")),
                None : false,
            }
        }
        else
        {
            ascii_lower_eq(pattern, host)
        }
    }

    // Whether the certificate is for `host`: an IP address literal against its
    // IP addresses, a name against its DNS names (the common name is not
    // consulted, as RFC 6125 and current browsers decide).
    export fn Certificate::matches_host(ref<Certificate, shared> c, StringView host) : bool
    {
        if (Ok(ip) = IpAddr::parse(host))
        {
            foreach (a in &c.ip_addresses)
            {
                if (IpAddr::text(a) == IpAddr::text(&ip))
                {
                    return true;
                }
            }
            return false;
        }
        StringView h = if (StringView::ends_with(
            host,
            ".",
        )) { StringView::sub(host, 0, StringView::len(host) - 1) } else { host };
        foreach (n in &c.dns_names)
        {
            if (name_matches(String::as_view(n), h))
            {
                return true;
            }
        }
        false
    }

    fn check_time(ref<Certificate, shared> c, ref<DateTime, shared> now) : Result<void, CertError>
    {
        if (DateTime::less(now, &c.not_before))
        {
            return Err(NotYetValid);
        }
        if (DateTime::less(&c.not_after, now))
        {
            return Err(Expired);
        }
        if (c.unhandled_critical)
        {
            return Err(UnhandledCritical);
        }
        Ok(())
    }

    fn same_bytes(ref<Vec<u8>, shared> a, ref<Vec<u8>, shared> b) : bool
    {
        Vec::len(a) == Vec::len(b) && digest_eq(&a[0..$], &b[0..$])
    }

    // Verifies a TLS server's chain (RFC 5280 §6, as a client needs it): the
    // leaf is for `host`, usable for a server, and in date at `now`; each
    // certificate is signed by the next, found among `intermediates` (in any
    // order) or `roots`, by issuer name and signature; every issuer is a CA
    // whose key may sign certificates, within its path length; the chain ends
    // at a root of `roots`, at most 10 certificates long.
    export fn verify_chain(
        ref<Certificate, shared> leaf,
        slice<Certificate, shared> intermediates,
        ref<TrustStore, shared> roots,
        StringView host,
        ref<DateTime, shared> now,
    ) : Result<void, CertError>
    {
        if (!Certificate::matches_host(leaf, host))
        {
            return Err(HostMismatch);
        }
        if (!leaf.server_auth)
        {
            return Err(WrongUsage);
        }
        check_time(leaf, now)?;
        Certificate current = Certificate::clone(leaf);
        usize below = 0;
        CertError last = UnknownAuthority;
        for (usize depth = 0; depth < 10; depth += 1)
        {
            foreach (r in &roots.roots)
            {
                if (same_bytes(
                    &r.subject,
                    &current.issuer,
                ) && verify_signature(&r.key, &current.scheme, &current.tbs[0..$], &current.signature[0..$]))
                {
                    if (!r.is_ca || !r.can_sign_certificates)
                    {
                        last = NotAuthority;
                        continue;
                    }
                    if (Err(e) = check_time(r, now))
                    {
                        last = e;
                        continue;
                    }
                    return Ok(());
                }
            }
            Option<Certificate> next = None;
            for (usize i = 0; i < slice_len(intermediates); i += 1)
            {
                ref<Certificate, shared> m = &intermediates[i];
                if (same_bytes(&m.subject, &current.issuer) && !same_bytes(&m.der, &current.der))
                {
                    if (!verify_signature(&m.key, &current.scheme, &current.tbs[0..$], &current.signature[0..$]))
                    {
                        last = BadSignature;
                        continue;
                    }
                    if (!m.is_ca || !m.can_sign_certificates)
                    {
                        return Err(NotAuthority);
                    }
                    if (m.max_path_len >= 0 && narrow<i64>(below) > m.max_path_len)
                    {
                        return Err(NotAuthority);
                    }
                    check_time(m, now)?;
                    overwrite(&mut next, Some(Certificate::clone(m)));
                    break;
                }
            }
            match (next)
            {
                Some(n) :
                {
                    overwrite(&mut current, n);
                    below += 1;
                },
                None    : return Err(last),
            }
        }
        Err(ChainTooLong)
    }

    // ---- keys in their standard encodings (D-0171) ----
    //
    // The formats `openssl` and every other tool write: a public key as a
    // SubjectPublicKeyInfo (RFC 5280 §4.1), a private key as PKCS #8
    // (RFC 5958), each in DER or in PEM (RFC 7468). The older PKCS #1
    // `RSA PRIVATE KEY` and SEC 1 `EC PRIVATE KEY` forms are read too;
    // what is written is always PKCS #8. An encrypted private key is not read.

    // DER length and element writers.
    fn der_push_len(ref<Vec<u8>, exclusive> out, usize len)
    {
        if (len < 0x80)
        {
            Vec::push(out, narrow<u8>(len));
            return;
        }
        usize count = 1;
        while ((len >> narrow<u32>(8 * count)) != 0)
        {
            count += 1;
        }
        Vec::push(out, 0x80 | narrow<u8>(count));
        for (usize i = 0; i < count; i += 1)
        {
            Vec::push(out, narrow_wrapping<u8>(len >> narrow<u32>(8 * (count - 1 - i))));
        }
    }

    fn der_wrap(u8 tag, slice<u8, shared> content) : Vec<u8>
    {
        Vec<u8> out = Vec::new();
        Vec::push(&mut out, tag);
        der_push_len(&mut out, slice_len(content));
        Vec::extend_from(&mut out, content);
        out
    }

    // A DER INTEGER of the non-negative number with big-endian bytes `v`.
    fn der_uint(slice<u8, shared> v) : Vec<u8>
    {
        usize from = 0;
        while (from + 1 < slice_len(v) && v[from] == 0)
        {
            from += 1;
        }
        Vec<u8> content = Vec::new();
        if (slice_len(v) == 0 || (v[from] & 0x80) != 0)
        {
            Vec::push(&mut content, 0);
        }
        Vec::extend_from(&mut content, &v[from..$]);
        der_wrap(0x02, &content[0..$])
    }

    fn der_big(ref<BigUint, shared> a) : Vec<u8>
    {
        der_uint(&BigUint::to_bytes_be(a)[0..$])
    }

    fn der_oid_hex(str hex) : Vec<u8>
    {
        Vec<u8> b = Result::unwrap_or(from_hex(StringView::of(hex)), Vec::new());
        der_wrap(0x06, &b[0..$])
    }

    fn der_bit_string(slice<u8, shared> bytes) : Vec<u8>
    {
        Vec<u8> content = Vec::new();
        Vec::push(&mut content, 0);
        Vec::extend_from(&mut content, bytes);
        der_wrap(0x03, &content[0..$])
    }

    fn curve_oid_hex(EcCurve c) : str
    {
        match (c)
        {
            P256 : "2a8648ce3d030107",
            P384 : "2b81040022",
        }
    }

    // The AlgorithmIdentifier of a key's algorithm.
    fn key_algorithm(ref<CertKey, shared> key) : Vec<u8>
    {
        Vec<u8> content = match (key)
        {
            RsaKey(_)      :
            {
                Vec<u8> c = der_oid_hex("2a864886f70d010101");
                Vec::push(&mut c, 0x05);
                Vec::push(&mut c, 0x00);
                c
            },
            EcKey(k)       :
            {
                Vec<u8> c = der_oid_hex("2a8648ce3d0201");
                Vec::extend_from(&mut c, &der_oid_hex(curve_oid_hex(k.curve))[0..$]);
                c
            },
            Ed25519Key(_)  : der_oid_hex("2b6570"),
            UnsupportedKey : Vec::new(),
        };
        der_wrap(0x30, &content[0..$])
    }

    // The DER SubjectPublicKeyInfo of a key, or None for `UnsupportedKey`.
    export fn CertKey::to_der(ref<CertKey, shared> key) : Option<Vec<u8>>
    {
        Vec<u8> bits = match (key)
        {
            RsaKey(k)      :
            {
                Vec<u8> seq = der_big(&k.n);
                Vec::extend_from(&mut seq, &der_big(&k.e)[0..$]);
                der_wrap(0x30, &seq[0..$])
            },
            EcKey(k)       : Vec::clone(&k.point),
            Ed25519Key(k)  : Vec::from_slice(&k[0..$]),
            UnsupportedKey : return None,
        };
        Vec<u8> content = key_algorithm(key);
        Vec::extend_from(&mut content, &der_bit_string(&bits[0..$])[0..$]);
        Some(der_wrap(0x30, &content[0..$]))
    }

    // The key of a DER SubjectPublicKeyInfo: `BadEncoding` when it is not one,
    // `UnsupportedAlgorithm` when its algorithm is not RSA, P-256, P-384 or
    // Ed25519.
    export fn CertKey::from_der(slice<u8, shared> der) : Result<CertKey, CertError>
    {
        Der spki = der_expect(der, 0, 0x30)?;
        if (spki.next != slice_len(der))
        {
            return Err(BadEncoding);
        }
        match (public_key_of(der, spki)?)
        {
            UnsupportedKey : Err(UnsupportedAlgorithm),
            k              : Ok(k),
        }
    }

    // The one PEM block of `text` with one of the `labels`: its DER, or
    // `BadEncoding` when there is none (or the base64 is not).
    fn pem_block(StringView text, slice<str, shared> labels) : Result<Vec<u8>, CertError>
    {
        for (usize i = 0; i < slice_len(labels); i += 1)
        {
            String begin = String::from_str("-----BEGIN ");
            String::append(&mut begin, labels[i]);
            String::append(&mut begin, "-----");
            String end = String::from_str("-----END ");
            String::append(&mut end, labels[i]);
            String::append(&mut end, "-----");
            usize b = match (StringView::find(text, String::as_view(&begin)))
            {
                Some(v) : v,
                None    : continue,
            };
            StringView body = StringView::sub(text, b + String::len(&begin), StringView::len(text));
            usize e = match (StringView::find(body, String::as_view(&end)))
            {
                Some(v) : v,
                None    : return Err(BadEncoding),
            };
            String b64 = String::new();
            StringView block = StringView::sub(body, 0, e);
            for (usize k = 0; k < StringView::len(block); k += 1)
            {
                u8 c = block.bytes[k];
                if (!ascii_is_space(c))
                {
                    String::push_ascii(&mut b64, c);
                }
            }
            return match (base64_decode(String::as_view(&b64)))
            {
                Ok(der) : Ok(der),
                Err(_)  : Err(BadEncoding),
            };
        }
        if (Option::is_some(&StringView::find(text, "-----BEGIN ENCRYPTED PRIVATE KEY-----")))
        {
            return Err(UnsupportedAlgorithm);
        }
        Err(BadEncoding)
    }

    // `der` in a PEM block labelled `label`, in lines of 64 characters.
    fn pem_of(str label, slice<u8, shared> der) : String
    {
        String out = String::from_str("-----BEGIN ");
        String::append(&mut out, label);
        String::append(&mut out, "-----\n");
        String b64 = base64_encode(der);
        usize n = String::len(&b64);
        for (usize at = 0; at < n; at += 64)
        {
            String::append(&mut out, StringView::sub(String::as_view(&b64), at, min(at + 64, n)));
            String::append(&mut out, "\n");
        }
        String::append(&mut out, "-----END ");
        String::append(&mut out, label);
        String::append(&mut out, "-----\n");
        out
    }

    // The key of the `-----BEGIN PUBLIC KEY-----` block in `text`.
    export fn CertKey::from_pem(StringView text) : Result<CertKey, CertError>
    {
        array<str, 1> labels = ["PUBLIC KEY"];
        Vec<u8> der = pem_block(text, &labels[0..$])?;
        CertKey::from_der(&der[0..$])
    }

    export fn CertKey::to_pem(ref<CertKey, shared> key) : Option<String>
    {
        Some(pem_of("PUBLIC KEY", &CertKey::to_der(key)?[0..$]))
    }

    // An elliptic-curve private key: the curve and the scalar, 32 or 48
    // bytes.
    export struct EcPrivateKey
    {
        export EcCurve curve;
        export Vec<u8> scalar;
    }

    // A private key of one of the algorithms `std` signs with.
    export enum PrivateKey
    {
        RsaPrivate(RsaPrivateKey),
        EcPrivate(EcPrivateKey),
        Ed25519Private(array<u8, 32>),
    }

    // The public key of a private key.
    export fn PrivateKey::public_key(ref<PrivateKey, shared> key) : CertKey
    {
        match (key)
        {
            RsaPrivate(k)     : RsaKey(RsaPrivateKey::public_key(k)),
            EcPrivate(k) : EcKey(EcPublicKey { .curve = k.curve, .point = Option::unwrap_or(
                ec_public_key(k.curve, &k.scalar[0..$]),
                Vec::new(),
            ) }),
            Ed25519Private(k) : Ed25519Key(ed25519_public_key(&k[0..$])),
        }
    }

    // The RSAPrivateKey of PKCS #1 (RFC 8017 A.1.2) at `at` in `buf`.
    fn rsa_private_of(slice<u8, shared> buf, usize at) : Result<PrivateKey, CertError>
    {
        Der seq = der_expect(buf, at, 0x30)?;
        Der version = der_expect(buf, seq.start, 0x02)?;
        Der n = der_expect(buf, version.next, 0x02)?;
        Der e = der_expect(buf, n.next, 0x02)?;
        Der d = der_expect(buf, e.next, 0x02)?;
        Der p = der_expect(buf, d.next, 0x02)?;
        Der q = der_expect(buf, p.next, 0x02)?;
        match (RsaPrivateKey::new(
            BigUint::from_bytes_be(&buf[n.start..n.end]),
            BigUint::from_bytes_be(&buf[e.start..e.end]),
            BigUint::from_bytes_be(&buf[d.start..d.end]),
            BigUint::from_bytes_be(&buf[p.start..p.end]),
            BigUint::from_bytes_be(&buf[q.start..q.end]),
        ))
        {
            Some(k) : Ok(RsaPrivate(k)),
            None    : Err(BadEncoding),
        }
    }

    // The ECPrivateKey of SEC 1 (RFC 5915) at `at` in `buf`; the curve from
    // its own parameters when `curve` is None (the PKCS #8 wrapper names it).
    fn ec_private_of(slice<u8, shared> buf, usize at, Option<EcCurve> curve) : Result<PrivateKey, CertError>
    {
        Der seq = der_expect(buf, at, 0x30)?;
        Der version = der_expect(buf, seq.start, 0x02)?;
        Der scalar = der_expect(buf, version.next, 0x04)?;
        Option<EcCurve> found = curve;
        usize p_at = scalar.next;
        while (p_at < seq.end)
        {
            Der field = match (der_read(buf, p_at))
            {
                Some(v) : v,
                None    : return Err(BadEncoding),
            };
            if (field.tag == 0xa0)
            {
                Der oid = der_expect(buf, field.start, 0x06)?;
                if (der_eq_hex(buf, oid, "2a8648ce3d030107"))
                {
                    found = Some(P256);
                }
                else if (der_eq_hex(buf, oid, "2b81040022"))
                {
                    found = Some(P384);
                }
                else
                {
                    return Err(UnsupportedAlgorithm);
                }
            }
            p_at = field.next;
        }
        EcCurve c = match (found)
        {
            Some(v) : v,
            None    : return Err(UnsupportedAlgorithm),
        };
        Vec<u8> k = Vec::from_slice(&buf[scalar.start..scalar.end]);
        usize want = if (c == P256) { 32 } else { 48 };
        if (Vec::len(&k) != want || Option::is_none(&ec_public_key(c, &k[0..$])))
        {
            return Err(BadEncoding);
        }
        Ok(EcPrivate(EcPrivateKey { .curve = c, .scalar = k }))
    }

    // The key of a DER private key: PKCS #8 PrivateKeyInfo, PKCS #1
    // RSAPrivateKey or SEC 1 ECPrivateKey, told apart by their first fields.
    // `BadEncoding` when it is none of them or its numbers do not fit
    // together; `UnsupportedAlgorithm` for an algorithm or curve `std` does
    // not sign with.
    export fn PrivateKey::from_der(slice<u8, shared> der) : Result<PrivateKey, CertError>
    {
        Der seq = der_expect(der, 0, 0x30)?;
        if (seq.next != slice_len(der))
        {
            return Err(BadEncoding);
        }
        Der version = der_expect(der, seq.start, 0x02)?;
        Der second = match (der_read(der, version.next))
        {
            Some(v) : v,
            None    : return Err(BadEncoding),
        };
        if (second.tag == 0x02)
        {
            return rsa_private_of(der, 0);
        }
        if (second.tag == 0x04)
        {
            return ec_private_of(der, 0, None);
        }
        if (second.tag != 0x30)
        {
            return Err(BadEncoding);
        }
        Der oid = der_expect(der, second.start, 0x06)?;
        Der key = der_expect(der, second.next, 0x04)?;
        if (der_eq_hex(der, oid, "2a864886f70d010101"))
        {
            return rsa_private_of(der, key.start);
        }
        if (der_eq_hex(der, oid, "2a8648ce3d0201"))
        {
            Der curve = der_expect(der, oid.next, 0x06)?;
            if (der_eq_hex(der, curve, "2a8648ce3d030107"))
            {
                return ec_private_of(der, key.start, Some(P256));
            }
            if (der_eq_hex(der, curve, "2b81040022"))
            {
                return ec_private_of(der, key.start, Some(P384));
            }
            return Err(UnsupportedAlgorithm);
        }
        if (der_eq_hex(der, oid, "2b6570"))
        {
            Der inner = der_expect(der, key.start, 0x04)?;
            if (inner.end - inner.start != 32 || inner.next != key.end)
            {
                return Err(BadEncoding);
            }
            array<u8, 32> seed = [0; 32];
            for (usize i = 0; i < 32; i += 1)
            {
                seed[i] = der[inner.start + i];
            }
            return Ok(Ed25519Private(seed));
        }
        Err(UnsupportedAlgorithm)
    }

    // The key as DER PKCS #8 (version 0): an RSAPrivateKey with its CRT
    // values, an ECPrivateKey with its public key, or Ed25519's seed.
    export fn PrivateKey::to_der(ref<PrivateKey, shared> key) : Vec<u8>
    {
        CertKey public = PrivateKey::public_key(key);
        Vec<u8> inner = match (key)
        {
            RsaPrivate(k)     :
            {
                BigUint one = BigUint::from_u64(1);
                BigUint p1 = BigUint::sub(&k.p, &one);
                BigUint q1 = BigUint::sub(&k.q, &one);
                Vec<u8> seq = der_uint(&[0: u8][0..$]);
                Vec::extend_from(&mut seq, &der_big(&k.n)[0..$]);
                Vec::extend_from(&mut seq, &der_big(&k.e)[0..$]);
                Vec::extend_from(&mut seq, &der_big(&k.d)[0..$]);
                Vec::extend_from(&mut seq, &der_big(&k.p)[0..$]);
                Vec::extend_from(&mut seq, &der_big(&k.q)[0..$]);
                Vec::extend_from(&mut seq, &der_big(&BigUint::rem(&k.d, &p1))[0..$]);
                Vec::extend_from(&mut seq, &der_big(&BigUint::rem(&k.d, &q1))[0..$]);
                Vec::extend_from(
                    &mut seq,
                    &der_big(&Option::unwrap_or(BigUint::mod_inverse(&k.q, &k.p), BigUint::zero()))[0..$],
                );
                der_wrap(0x30, &seq[0..$])
            },
            EcPrivate(k)      :
            {
                Vec<u8> seq = der_uint(&[1: u8][0..$]);
                Vec::extend_from(&mut seq, &der_wrap(0x04, &k.scalar[0..$])[0..$]);
                Vec<u8> point = match (&public)
                {
                    EcKey(pk) : Vec::clone(&pk.point),
                    _         : Vec::new(),
                };
                Vec::extend_from(&mut seq, &der_wrap(0xa1, &der_bit_string(&point[0..$])[0..$])[0..$]);
                der_wrap(0x30, &seq[0..$])
            },
            Ed25519Private(k) : der_wrap(0x04, &k[0..$]),
        };
        Vec<u8> content = der_uint(&[0: u8][0..$]);
        Vec::extend_from(&mut content, &key_algorithm(&public)[0..$]);
        Vec::extend_from(&mut content, &der_wrap(0x04, &inner[0..$])[0..$]);
        der_wrap(0x30, &content[0..$])
    }

    // The key of the first `PRIVATE KEY`, `RSA PRIVATE KEY` or `EC PRIVATE
    // KEY` block in `text`; an `ENCRYPTED PRIVATE KEY` is `UnsupportedAlgorithm`.
    export fn PrivateKey::from_pem(StringView text) : Result<PrivateKey, CertError>
    {
        array<str, 3> labels = ["PRIVATE KEY", "RSA PRIVATE KEY", "EC PRIVATE KEY"];
        Vec<u8> der = pem_block(text, &labels[0..$])?;
        PrivateKey::from_der(&der[0..$])
    }

    export fn PrivateKey::to_pem(ref<PrivateKey, shared> key) : String
    {
        pem_of("PRIVATE KEY", &PrivateKey::to_der(key)[0..$])
    }

    // `key`'s signature of `message` under `scheme`, as `verify_signature`
    // checks it: the message hashed with the scheme's hash and signed with
    // RSA or ECDSA, or signed whole with Ed25519. None when the key is not of
    // the scheme's kind (or the scheme is unsupported).
    export fn sign(
        ref<PrivateKey, shared> key,
        ref<SignatureScheme, shared> scheme,
        slice<u8, shared> message,
    ) : Option<Vec<u8>>
    {
        match (scheme)
        {
            RsaPkcs1(h) : match (key)
            {
                RsaPrivate(k) : Some(rsa_sign_pkcs1v15(k, *h, &hash(*h, message)[0..$])),
                _             : None,
            },
            RsaPss(h) : match (key)
            {
                RsaPrivate(k) : Some(rsa_sign_pss(k, *h, &hash(*h, message)[0..$])),
                _             : None,
            },
            Ecdsa(h) : match (key)
            {
                EcPrivate(k) : ecdsa_sign(k.curve, &k.scalar[0..$], &hash(*h, message)[0..$]),
                _            : None,
            },
            Ed25519 : match (key)
            {
                Ed25519Private(k) : Some(Vec::from_slice(&ed25519_sign(&k[0..$], message)[0..$])),
                _                 : None,
            },
            UnsupportedSignature : None,
        }
    }

**Depends on:** rule.stdlib.crypto, rule.stdlib.time, rule.stdlib.net, rule.stdlib.file, D-0160
**Affects:** none (pure, but for reading trust files)

## 2o. TLS

In `std::tls` (D-0161).

### `rule.stdlib.tls`
**Status:** ACCEPTED

A TLS 1.3 client (RFC 8446) with the cipher suites
TLS_CHACHA20_POLY1305_SHA256, TLS_AES_128_GCM_SHA256 and
TLS_AES_256_GCM_SHA384, the key exchanges X25519, P-256 and P-384, and
ECDSA, RSA-PSS and Ed25519 signatures (D-0165, D-0172: RFC 8446 §9.1's
mandatory set and the common additions), over a `TcpStream`.

    [Tls-Client]       ⟨TlsStream::client(tcp, name, roots), Σ⟩ → Ok(stream) when the handshake completes:
                       the ClientHello offers the suites TLS_CHACHA20_POLY1305_SHA256,
                       TLS_AES_128_GCM_SHA256 and TLS_AES_256_GCM_SHA384 (D-0172), the groups X25519,
                       P-256 and P-384 with an X25519 key share, TLS 1.3, the signature schemes ECDSA
                       P-256/SHA-256, P-384/SHA-384, RSA-PSS with SHA-2, Ed25519, and RSA PKCS #1 v1.5
                       for certificates, and SNI when name is not an IP address; a HelloRetryRequest
                       asking for P-256 or P-384 is answered with a second ClientHello carrying a share of
                       that group and its cookie, the transcript restarting from
                       message_hash(ClientHello1) under the suite's hash (§4.4.1); the ServerHello selects
                       TLS 1.3, an offered suite (the same as the HelloRetryRequest's) and the group of
                       the share sent; the key schedule, transcript, Finished and key updates run over
                       the suite's hash (SHA-384 for TLS_AES_256_GCM_SHA384); the server's certificates
                       pass verify_chain(leaf, the rest, roots, name, now); its CertificateVerify
                       verifies over the transcript and its Finished matches; the client's Finished is
                       sent. Otherwise Err (a second HelloRetryRequest, or one asking for another group,
                       is HandshakeFailed).
                       connect(addr, …) is TcpStream::connect, then
                       client
    [Tls-Records]      write sends the data in records of at most 16 KiB under the client's application
                       keys; read gives the server's application data in order, 0 after close_notify; a
                       NewSessionTicket is ignored; a KeyUpdate changes the receiving keys and, when it
                       asks, is answered and the sending keys changed; a record that does not authenticate
                       is RecordRejected; close sends close_notify
    [Tls-Error]        a network failure is NetFailure(e), a refused chain CertRejected(e), an alert from
                       the server PeerAlert(description), a malformed message ProtocolError, a negotiation
                       this client cannot complete HandshakeFailed


    [Tls-Server]            (D-0186)   ⟨TlsStream::server(tcp, key, chain), Σ⟩ → Ok(s) after the handshake as the server over
                            the accepted tcp with key (a PrivateKey) and chain (the server's certificate, then its
                            issuers), s sending under the server application keys and receiving under the client's,
                            peer_certificate None; Err(HandshakeFailed) otherwise, an alert sent first where the
                            client can be told: 70 (protocol_version) when supported_versions lacks 0x0304; 40
                            (handshake_failure) when the client offers none of the suites 0x1303, 0x1301, 0x1302, or
                            its signature_algorithms lack the key's scheme (0x0804 rsa_pss_rsae_sha256 for an RSA
                            key, 0x0403 for P-256, 0x0503 for P-384, 0x0807 for Ed25519), or it has no key share and
                            no supported group among X25519, P-256, P-384; 47 (illegal_parameter) for a bad share or
                            a second ClientHello that differs from the first. The suite is the client's first that
                            the server has; the group its first key share the server has; with no such share but a
                            supported group, a HelloRetryRequest for the client's first such group (RFC 8446 §4.1.4:
                            the HRR random, the group in key_share, the transcript restarted from
                            message_hash(ClientHello1)), and the second ClientHello must carry that share, the suite
                            and the session id. Then ServerHello (a random, the session id echoed, the suite,
                            supported_versions 0x0304, key_share), a compatibility ChangeCipherSpec, and under the
                            server handshake keys EncryptedExtensions (none), Certificate (chain's DERs, no
                            extensions), CertificateVerify (sign of the 64 spaces ‖ "TLS 1.3, server
                            CertificateVerify" ‖ 0 ‖ the transcript hash, under the scheme) and Finished; the
                            client's Finished read under its handshake keys and verified (HandshakeFailed otherwise),
                            the application keys by §7.1 as [Tls-Client]'s. A malformed message is ProtocolError; a
                            client's alert PeerAlert; the socket's failures NetFailure
    [Tls-Read-Line]         (D-0181)   ⟨TlsStream::read_line(s), Σ⟩ → as TcpStream::read_line over the decrypted bytes: the
                            next line without its "\n" (nor a "\r" before it), Ok(None) at close_notify before any
                            byte, Err(NetFailure(NotUtf8(_))) for a line that is not UTF-8
    [Tls-Peer-Certificate]  (D-0181)   ⟨TlsStream::peer_certificate(s), Σ⟩ → Some(a reference to the leaf [Tls-Client]
                            verified) on a client's stream, None on a server's; peer_addr and local_addr the
                            socket's

The `std` source:

    // `std::tls` (spec/21 §2o, D-0161): a TLS 1.3 client (RFC 8446) over a
    // `TcpStream`. The cipher suites TLS_CHACHA20_POLY1305_SHA256,
    // TLS_AES_128_GCM_SHA256 (D-0165) and TLS_AES_256_GCM_SHA384 (D-0172),
    // the key exchanges X25519 and, when the server asks for one with a
    // HelloRetryRequest, P-256 or P-384, the signature schemes ECDSA, RSA-PSS
    // and Ed25519, and the server's certificate chain verified with
    // `std::x509` against a `TrustStore` and the server's name: RFC 8446's
    // mandatory set and the common additions. No TLS 1.2, no session
    // resumption, no early data, no client certificates. Written in CobaltC
    // over `std::crypto`.

    // Why a TLS connection failed.
    export enum TlsError
    {
        NetFailure(NetError),
        CertRejected(CertError),
        HandshakeFailed,
        ProtocolError,
        PeerAlert(u8),
        RecordRejected,
    }

    export fn TlsError::text(ref<TlsError, shared> e) : str
    {
        match (e)
        {
            NetFailure(n)   : NetError::text(n),
            CertRejected(c) : CertError::text(c),
            HandshakeFailed : "TLS handshake failed",
            ProtocolError   : "TLS protocol violation",
            PeerAlert(code) : match (*code)
            {
                40  : "the server refused the handshake (alert 40): it supports none of the suites (ChaCha20-Poly1305, AES-128-GCM, AES-256-GCM) or key exchanges (X25519, P-256, P-384) this client offers",
                42  : "the server refused a certificate (alert 42)",
                70  : "the server does not speak TLS 1.3 (alert 70)",
                80  : "the server failed internally (alert 80)",
                109 : "the server requires an extension this client does not send (alert 109)",
                112 : "the server does not know the name asked for (alert 112)",
                _   : "the server sent a TLS alert",
            },
            RecordRejected  : "a TLS record did not authenticate",
        }
    }

    // The hash of a cipher suite's key schedule, and its AEAD key length.
    fn suite_hash(u16 suite) : HashKind
    {
        if (suite == 0x1302) { Sha2_384 } else { Sha2_256 }
    }

    fn suite_key_len(u16 suite) : usize
    {
        if (suite == 0x1301) { 16 } else { 32 }
    }

    // One direction's record protection: the key, the IV and the sequence
    // number (RFC 8446 §5.3), the traffic secret they came from, and the
    // suite (0x1303 ChaCha20-Poly1305, 0x1301 AES-128-GCM, 0x1302
    // AES-256-GCM).
    struct RecordKeys
    {
        Vec<u8> key;
        Vec<u8> iv;
        u64 seq;
        Vec<u8> secret;
        u16 suite;
    }

    fn keys_from(slice<u8, shared> secret, u16 suite) : RecordKeys
    {
        Vec<u8> none = Vec::new();
        HashKind kind = suite_hash(suite);
        RecordKeys {
            .key = hkdf_expand_label_with(kind, secret, "key", &none[0..$], suite_key_len(suite)),
            .iv = hkdf_expand_label_with(kind, secret, "iv", &none[0..$], 12),
            .seq = 0,
            .secret = Vec::from_slice(secret),
            .suite = suite,
        }
    }

    fn record_nonce(ref<RecordKeys, shared> k) : Vec<u8>
    {
        Vec<u8> n = Vec::clone(&k.iv);
        for (usize i = 0; i < 8; i += 1)
        {
            n[4 + i] = n[4 + i] ^ narrow_wrapping<u8>(k.seq >> narrow<u32>(56 - 8 * i));
        }
        n
    }

    // A TLS 1.3 connection: as a client, to a server whose certificate has
    // been verified (kept in `peer`); as a server (D-0186), to a client.
    export resource struct TlsStream
    {
        TcpStream tcp;
        RecordKeys send;
        RecordKeys recv;
        Vec<u8> pending;
        usize pending_at;
        bool closed;
        Option<Certificate> peer;
    }

    // D-0181: the peer's certificate, the leaf the handshake verified: for a
    // client, the server's; `None` on a server's side (clients send none).
    // A program that pins a key or a certificate compares it here.
    export fn TlsStream::peer_certificate(ref<TlsStream, shared> s) : Option<ref<Certificate, shared>>
    {
        match (&s.peer)
        {
            Some(c) : Some(c),
            None    : None,
        }
    }

    export fn TlsStream::peer_addr(ref<TlsStream, shared> s) : SocketAddr
    {
        TcpStream::peer_addr(&s.tcp)
    }

    export fn TlsStream::local_addr(ref<TlsStream, shared> s) : SocketAddr
    {
        TcpStream::local_addr(&s.tcp)
    }

    // The record header and body: one record from the network.
    struct Record
    {
        u8 kind;
        Vec<u8> header;
        Vec<u8> body;
    }

    fn read_record(ref<TcpStream, exclusive> tcp) : Result<Record, TlsError>
    {
        Vec<u8> header = Vec::new();
        match (TcpStream::read_exact(tcp, &mut header, 5))
        {
            Ok(_)  : {},
            Err(e) : return Err(NetFailure(e)),
        }
        usize len = widen<usize>(read_be<u16>(&header[0..$], 3));
        if (len > 16384 + 256)
        {
            return Err(ProtocolError);
        }
        Vec<u8> body = Vec::new();
        match (TcpStream::read_exact(tcp, &mut body, len))
        {
            Ok(_)  : {},
            Err(e) : return Err(NetFailure(e)),
        }
        Ok(Record { .kind = header[0], .header = header, .body = body })
    }

    fn write_all(ref<TcpStream, exclusive> tcp, slice<u8, shared> data) : Result<void, TlsError>
    {
        match (TcpStream::write(tcp, data))
        {
            Ok(_)  : Ok(()),
            Err(e) : Err(NetFailure(e)),
        }
    }

    // An encrypted record (application_data on the wire) of `content` with
    // inner type `kind`.
    fn seal_record(ref<RecordKeys, exclusive> k, u8 kind, slice<u8, shared> content) : Vec<u8>
    {
        Vec<u8> inner = Vec::from_slice(content);
        Vec::push(&mut inner, kind);
        Vec<u8> header = Vec::new();
        Vec::push(&mut header, 23);
        Vec::push_be(&mut header, 0x0303: u16);
        Vec::push_be(&mut header, narrow<u16>(Vec::len(&inner) + 16));
        Vec<u8> nonce = record_nonce(k);
        Vec<u8> sealed = if (k.suite != 0x1303)
        {
            aes_gcm_seal(&k.key[0..$], &nonce[0..$], &header[0..$], &inner[0..$])
        }
        else
        {
            chacha20_poly1305_seal(&k.key[0..$], &nonce[0..$], &header[0..$], &inner[0..$])
        };
        k.seq += 1;
        Vec::extend_from(&mut header, &sealed[0..$]);
        header
    }

    // The content and inner type of an encrypted record.
    struct Inner
    {
        u8 kind;
        Vec<u8> content;
    }

    fn open_record(ref<RecordKeys, exclusive> k, ref<Record, shared> r) : Result<Inner, TlsError>
    {
        if (r.kind != 23)
        {
            return Err(ProtocolError);
        }
        Vec<u8> nonce = record_nonce(k);
        Option<Vec<u8>> opened = if (k.suite != 0x1303)
        {
            aes_gcm_open(&k.key[0..$], &nonce[0..$], &r.header[0..$], &r.body[0..$])
        }
        else
        {
            chacha20_poly1305_open(&k.key[0..$], &nonce[0..$], &r.header[0..$], &r.body[0..$])
        };
        Vec<u8> plain = match (opened)
        {
            Some(p) : p,
            None    : return Err(RecordRejected),
        };
        k.seq += 1;
        usize n = Vec::len(&plain);
        while (n > 0 && plain[n - 1] == 0)
        {
            n -= 1;
        }
        if (n == 0 || n - 1 > 16384)
        {
            return Err(ProtocolError);
        }
        u8 kind = plain[n - 1];
        Vec::truncate(&mut plain, n - 1);
        Ok(Inner { .kind = kind, .content = plain })
    }

    // The handshake messages arriving in records, and the transcript of every
    // message so far: kept under both hashes until the ServerHello names the
    // suite, then read from the suite's.
    struct Handshake
    {
        Vec<u8> buffer;
        Hasher t256;
        Hasher t384;
        HashKind kind;
    }

    fn transcript_add(ref<Handshake, exclusive> hs, slice<u8, shared> msg)
    {
        Hasher::update(&mut hs.t256, msg);
        Hasher::update(&mut hs.t384, msg);
    }

    // The next whole handshake message (type, 3-byte length, body), read from
    // `buffer` or, when it holds less, from more records; the message joins
    // the transcript.
    fn next_message(
        ref<Handshake, exclusive> hs,
        ref<TcpStream, exclusive> tcp,
        ref<Option<RecordKeys>, exclusive> keys,
    ) : Result<Vec<u8>, TlsError>
    {
        while (Vec::len(&hs.buffer) < 4 || Vec::len(&hs.buffer) < 4 + (widen<usize>(hs.buffer[1]) << 16 | widen<usize>(hs.buffer[2]) << 8 | widen<usize>(hs.buffer[3])))
        {
            Record r = read_record(tcp)?;
            if (r.kind == 20)
            {
                // A compatibility ChangeCipherSpec (RFC 8446 §D.4): ignored.
                continue;
            }
            if (r.kind == 21)
            {
                if (Vec::len(&r.body) == 2)
                {
                    return Err(PeerAlert(r.body[1]));
                }
                return Err(ProtocolError);
            }
            match (keys)
            {
                Some(k) :
                {
                    Inner i = open_record(k, &r)?;
                    if (i.kind == 21)
                    {
                        return Err(PeerAlert(if (Vec::len(&i.content) == 2) { i.content[1] } else { 0 }));
                    }
                    if (i.kind != 22)
                    {
                        return Err(ProtocolError);
                    }
                    Vec::extend_from(&mut hs.buffer, &i.content[0..$]);
                },
                None    :
                {
                    if (r.kind != 22)
                    {
                        return Err(ProtocolError);
                    }
                    Vec::extend_from(&mut hs.buffer, &r.body[0..$]);
                },
            }
        }
        usize len = 4 + (widen<usize>(hs.buffer[1]) << 16 | widen<usize>(hs.buffer[2]) << 8 | widen<usize>(hs.buffer[3]));
        Vec<u8> msg = Vec::from_slice(&hs.buffer[0..len]);
        Vec<u8> rest = Vec::from_slice(&hs.buffer[len..$]);
        overwrite(&mut hs.buffer, rest);
        transcript_add(hs, &msg[0..$]);
        Ok(msg)
    }

    fn push_u24(ref<Vec<u8>, exclusive> v, usize n)
    {
        Vec::push(v, narrow<u8>(n >> 16));
        Vec::push(v, narrow_wrapping<u8>(n >> 8));
        Vec::push(v, narrow_wrapping<u8>(n));
    }

    // One extension: its type and its data with a 2-byte length.
    fn push_extension(ref<Vec<u8>, exclusive> out, u16 kind, slice<u8, shared> data)
    {
        Vec::push_be(out, kind);
        Vec::push_be(out, narrow<u16>(slice_len(data)));
        Vec::extend_from(out, data);
    }

    // The ClientHello: a key share for `group` (X25519 first, P-256 or P-384
    // after a HelloRetryRequest asks for one), and the HelloRetryRequest's
    // cookie when it gave one.
    fn client_hello(
        StringView server_name,
        slice<u8, shared> random,
        slice<u8, shared> session,
        u16 group,
        slice<u8, shared> public_key,
        slice<u8, shared> cookie,
    ) : Vec<u8>
    {
        Vec<u8> ext = Vec::new();
        if (Result::is_err(&IpAddr::parse(server_name)))
        {
            Vec<u8> sni = Vec::new();
            Vec::push_be(&mut sni, narrow<u16>(StringView::len(server_name) + 3));
            Vec::push(&mut sni, 0);
            Vec::push_be(&mut sni, narrow<u16>(StringView::len(server_name)));
            Vec::extend_from(&mut sni, server_name.bytes);
            push_extension(&mut ext, 0x0000, &sni[0..$]);
        }
        array<u8, 8> groups = [0x00, 0x06, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18];
        push_extension(&mut ext, 0x000a, &groups[0..$]);
        array<u8, 20> algs = [
            0x00,
            0x12,
            0x04,
            0x03,
            0x05,
            0x03,
            0x08,
            0x04,
            0x08,
            0x05,
            0x08,
            0x06,
            0x08,
            0x07,
            0x04,
            0x01,
            0x05,
            0x01,
            0x06,
            0x01,
        ];
        push_extension(&mut ext, 0x000d, &algs[0..$]);
        array<u8, 3> versions = [0x02, 0x03, 0x04];
        push_extension(&mut ext, 0x002b, &versions[0..$]);
        Vec<u8> share = Vec::new();
        Vec::push_be(&mut share, narrow<u16>(4 + slice_len(public_key)));
        Vec::push_be(&mut share, group);
        Vec::push_be(&mut share, narrow<u16>(slice_len(public_key)));
        Vec::extend_from(&mut share, public_key);
        push_extension(&mut ext, 0x0033, &share[0..$]);
        if (slice_len(cookie) > 0)
        {
            push_extension(&mut ext, 0x002c, cookie);
        }
        Vec<u8> body = Vec::new();
        Vec::push_be(&mut body, 0x0303: u16);
        Vec::extend_from(&mut body, random);
        Vec::push(&mut body, 32);
        Vec::extend_from(&mut body, session);
        array<u8, 8> suites = [0x00, 0x06, 0x13, 0x03, 0x13, 0x01, 0x13, 0x02];
        Vec::extend_from(&mut body, &suites[0..$]);
        Vec::push(&mut body, 1);
        Vec::push(&mut body, 0);
        Vec::push_be(&mut body, narrow<u16>(Vec::len(&ext)));
        Vec::extend_from(&mut body, &ext[0..$]);
        Vec<u8> msg = Vec::new();
        Vec::push(&mut msg, 1);
        push_u24(&mut msg, Vec::len(&body));
        Vec::extend_from(&mut msg, &body[0..$]);
        msg
    }

    // What a ServerHello or HelloRetryRequest chose.
    struct ServerHello
    {
        bool retry;
        u16 suite;
        u16 group;
        Vec<u8> share;
        Vec<u8> cookie;
    }

    // The suite, the group and the key share of a ServerHello, or the suite,
    // the group asked for and the cookie of a HelloRetryRequest (RFC 8446
    // §4.1.3, §4.1.4), after checking the version, the session id echo and
    // that the suite is one offered.
    fn server_hello(slice<u8, shared> sh, slice<u8, shared> session) : Result<ServerHello, TlsError>
    {
        usize n = slice_len(sh);
        if (n < 4 + 2 + 32 + 1 || sh[0] != 2)
        {
            return Err(ProtocolError);
        }
        // HelloRetryRequest's fixed random.
        Vec<u8> hrr = Result::unwrap_or(
            from_hex("cf21ad74e59a6111be1d8c021e65b891c2a211167abb8c5e079e09e2c8a8339c"),
            Vec::new(),
        );
        bool retry = digest_eq(&sh[6..38], &hrr[0..$]);
        usize at = 38;
        usize sid_len = widen<usize>(sh[at]);
        if (sid_len != slice_len(session) || at + 1 + sid_len + 3 + 2 > n || !digest_eq(
            &sh[at + 1..at + 1 + sid_len],
            session,
        ))
        {
            return Err(ProtocolError);
        }
        at = at + 1 + sid_len;
        u16 suite = read_be<u16>(sh, at);
        if ((suite != 0x1303 && suite != 0x1301 && suite != 0x1302) || sh[at + 2] != 0)
        {
            return Err(HandshakeFailed);
        }
        at += 3;
        usize ext_end = at + 2 + widen<usize>(read_be<u16>(sh, at));
        if (ext_end != n)
        {
            return Err(ProtocolError);
        }
        at += 2;
        bool tls13 = false;
        Option<u16> group = None;
        Vec<u8> share = Vec::new();
        Vec<u8> cookie = Vec::new();
        while (at + 4 <= ext_end)
        {
            u16 kind = read_be<u16>(sh, at);
            usize len = widen<usize>(read_be<u16>(sh, at + 2));
            usize start = at + 4;
            if (start + len > ext_end)
            {
                return Err(ProtocolError);
            }
            if (kind == 0x002b)
            {
                tls13 = len == 2 && sh[start] == 0x03 && sh[start + 1] == 0x04;
            }
            else if (kind == 0x0033)
            {
                if (len < 2)
                {
                    return Err(ProtocolError);
                }
                u16 g = read_be<u16>(sh, start);
                if (g != 0x001d && g != 0x0017 && g != 0x0018)
                {
                    return Err(HandshakeFailed);
                }
                group = Some(g);
                if (!retry)
                {
                    usize want = if (g == 0x001d) { 32 } else if (g == 0x0017) { 65 } else { 97 };
                    if (len != 4 + want || widen<usize>(read_be<u16>(sh, start + 2)) != want)
                    {
                        return Err(HandshakeFailed);
                    }
                    overwrite(&mut share, Vec::from_slice(&sh[start + 4..start + len]));
                }
                else if (len != 2)
                {
                    return Err(ProtocolError);
                }
            }
            else if (kind == 0x002c && retry)
            {
                overwrite(&mut cookie, Vec::from_slice(&sh[start..start + len]));
            }
            at = start + len;
        }
        if (!tls13)
        {
            return Err(HandshakeFailed);
        }
        match (group)
        {
            Some(g) : Ok(ServerHello { .retry = retry, .suite = suite, .group = g, .share = share, .cookie = cookie }),
            None    : Err(HandshakeFailed),
        }
    }

    // A handshake record carrying `msg`.
    fn handshake_record(slice<u8, shared> msg, u16 version) : Vec<u8>
    {
        Vec<u8> rec = Vec::new();
        Vec::push(&mut rec, 22);
        Vec::push_be(&mut rec, version);
        Vec::push_be(&mut rec, narrow<u16>(slice_len(msg)));
        Vec::extend_from(&mut rec, msg);
        rec
    }

    fn transcript_hash(ref<Handshake, shared> hs) : Vec<u8>
    {
        match (hs.kind)
        {
            Sha2_384 : Hasher::finish(hs.t384),
            _        : Hasher::finish(hs.t256),
        }
    }

    fn derive_secret(HashKind kind, slice<u8, shared> secret, str label, slice<u8, shared> transcript) : Vec<u8>
    {
        hkdf_expand_label_with(kind, secret, label, transcript, slice_len(secret))
    }

    // The certificates of a Certificate message (RFC 8446 §4.4.2), leaf first.
    fn certificate_list(slice<u8, shared> m) : Result<Vec<Certificate>, TlsError>
    {
        usize n = slice_len(m);
        if (n < 8 || m[0] != 11 || m[4] != 0)
        {
            return Err(ProtocolError);
        }
        usize at = 5;
        usize list_end = at + 3 + (widen<usize>(m[at]) << 16 | widen<usize>(m[at + 1]) << 8 | widen<usize>(m[at + 2]));
        if (list_end != n)
        {
            return Err(ProtocolError);
        }
        at += 3;
        Vec<Certificate> out = Vec::new();
        while (at < list_end)
        {
            if (at + 3 > list_end)
            {
                return Err(ProtocolError);
            }
            usize len = widen<usize>(m[at]) << 16 | widen<usize>(m[at + 1]) << 8 | widen<usize>(m[at + 2]);
            usize start = at + 3;
            if (start + len + 2 > list_end)
            {
                return Err(ProtocolError);
            }
            match (Certificate::parse(&m[start..start + len]))
            {
                Ok(c)  : Vec::push(&mut out, c),
                Err(e) : return Err(CertRejected(e)),
            }
            usize ext_len = widen<usize>(read_be<u16>(m, start + len));
            at = start + len + 2 + ext_len;
        }
        if (Vec::len(&out) == 0)
        {
            return Err(HandshakeFailed);
        }
        Ok(out)
    }

    // Whether a CertificateVerify (RFC 8446 §4.4.3) holds for the leaf's key
    // over the transcript hash so far.
    fn certificate_verify_ok(slice<u8, shared> m, ref<Certificate, shared> leaf, slice<u8, shared> transcript) : bool
    {
        if (slice_len(m) < 8 || m[0] != 15)
        {
            return false;
        }
        u16 code = read_be<u16>(m, 4);
        usize len = widen<usize>(read_be<u16>(m, 6));
        if (8 + len != slice_len(m))
        {
            return false;
        }
        SignatureScheme scheme = UnsupportedSignature;
        bool curve_ok = true;
        if (code == 0x0403 || code == 0x0503)
        {
            EcCurve want = if (code == 0x0403) { P256 } else { P384 };
            curve_ok = match (&leaf.key)
            {
                EcKey(k) : k.curve == want,
                _        : false,
            };
            scheme = Ecdsa(if (code == 0x0403) { Sha2_256 } else { Sha2_384 });
        }
        else if (code == 0x0804)
        {
            scheme = RsaPss(Sha2_256);
        }
        else if (code == 0x0805)
        {
            scheme = RsaPss(Sha2_384);
        }
        else if (code == 0x0806)
        {
            scheme = RsaPss(Sha2_512);
        }
        else if (code == 0x0807)
        {
            scheme = Ed25519;
        }
        if (!curve_ok)
        {
            return false;
        }
        Vec<u8> content = Vec::filled(64, 0x20: u8);
        Vec::extend_from(&mut content, &b"TLS 1.3, server CertificateVerify"[0..$]);
        Vec::push(&mut content, 0);
        Vec::extend_from(&mut content, transcript);
        verify_signature(&leaf.key, &scheme, &content[0..$], &m[8..$])
    }

    fn finished_data(HashKind kind, slice<u8, shared> secret, slice<u8, shared> transcript) : Vec<u8>
    {
        Vec<u8> none = Vec::new();
        Vec<u8> key = hkdf_expand_label_with(kind, secret, "finished", &none[0..$], slice_len(secret));
        hmac(kind, &key[0..$], transcript)
    }

    // Performs the TLS 1.3 handshake over a connected `tcp` as a client of
    // `server_name` (a DNS name, sent as SNI and checked against the
    // certificate, or an IP address literal, checked against its IP
    // addresses), verifying the server's chain against `roots` at the current
    // time. The stream's own timeouts apply to every read and write.
    export fn TlsStream::client(
        TcpStream tcp,
        StringView server_name,
        ref<TrustStore, shared> roots,
    ) : Result<TlsStream, TlsError>
    {
        array<u8, 32> private_key = x25519_private_key();
        array<u8, 32> public_key = x25519_public_key(&private_key[0..$]);
        array<u8, 32> random = [0; 32];
        os_random_bytes(&mut random[0..$]);
        array<u8, 32> session = [0; 32];
        os_random_bytes(&mut session[0..$]);
        Handshake hs = Handshake { .buffer = Vec::new(), .t256 = Hasher::new(Sha2_256), .t384 = Hasher::new(Sha2_384), .kind = Sha2_256 };
        Vec<u8> none = Vec::new();
        Vec<u8> ch = client_hello(server_name, &random[0..$], &session[0..$], 0x001d, &public_key[0..$], &none[0..$]);
        transcript_add(&mut hs, &ch[0..$]);
        write_all(&mut tcp, &handshake_record(&ch[0..$], 0x0301)[0..$])?;
        Option<RecordKeys> plain = None;
        Vec<u8> sh = next_message(&mut hs, &mut tcp, &mut plain)?;
        ServerHello hello = server_hello(&sh[0..$], &session[0..$])?;
        hs.kind = suite_hash(hello.suite);
        HashKind kind = hs.kind;
        Vec<u8> ec_key = Vec::new();
        if (hello.retry)
        {
            // A HelloRetryRequest (RFC 8446 §4.1.4): it may only ask for a
            // group this client offered without a share, P-256 or P-384. The
            // transcript restarts from message_hash(ClientHello1) ‖
            // HelloRetryRequest, under the suite's hash.
            if ((hello.group != 0x0017 && hello.group != 0x0018) || Vec::len(&hs.buffer) != 0)
            {
                return Err(HandshakeFailed);
            }
            EcCurve curve = if (hello.group == 0x0017) { P256 } else { P384 };
            Vec<u8> ch_hash = hash(kind, &ch[0..$]);
            array<u8, 4> head = [254, 0, 0, narrow<u8>(Vec::len(&ch_hash))];
            Handshake restarted = Handshake { .buffer = Vec::new(), .t256 = Hasher::new(Sha2_256), .t384 = Hasher::new(Sha2_384), .kind = kind };
            transcript_add(&mut restarted, &head[0..$]);
            transcript_add(&mut restarted, &ch_hash[0..$]);
            transcript_add(&mut restarted, &sh[0..$]);
            overwrite(&mut ec_key, ec_private_key(curve));
            Vec<u8> ec_public = Option::unwrap(ec_public_key(curve, &ec_key[0..$]));
            Vec<u8> ch2 = client_hello(
                server_name,
                &random[0..$],
                &session[0..$],
                hello.group,
                &ec_public[0..$],
                &hello.cookie[0..$],
            );
            transcript_add(&mut restarted, &ch2[0..$]);
            overwrite(&mut hs, restarted);
            write_all(&mut tcp, &handshake_record(&ch2[0..$], 0x0303)[0..$])?;
            Vec<u8> sh2 = next_message(&mut hs, &mut tcp, &mut plain)?;
            ServerHello second = server_hello(&sh2[0..$], &session[0..$])?;
            if (second.retry || second.group != hello.group || second.suite != hello.suite)
            {
                return Err(HandshakeFailed);
            }
            overwrite(&mut hello, second);
        }
        else if (hello.group != 0x001d)
        {
            return Err(HandshakeFailed);
        }
        u16 suite = hello.suite;
        Vec<u8> shared = Vec::new();
        if (hello.group == 0x001d)
        {
            match (x25519(&private_key[0..$], &hello.share[0..$]))
            {
                Some(z) : Vec::extend_from(&mut shared, &z[0..$]),
                None    : return Err(HandshakeFailed),
            }
        }
        else
        {
            EcCurve curve = if (hello.group == 0x0017) { P256 } else { P384 };
            match (ecdh(curve, &ec_key[0..$], &hello.share[0..$]))
            {
                Some(z) : overwrite(&mut shared, z),
                None    : return Err(HandshakeFailed),
            }
        }
        // The key schedule (RFC 8446 §7.1), under the suite's hash.
        Vec<u8> empty_hash = hash(kind, &none[0..$]);
        Vec<u8> zeros = Vec::filled(Vec::len(&empty_hash), 0: u8);
        Vec<u8> early = hkdf_extract_with(kind, &none[0..$], &zeros[0..$]);
        Vec<u8> derived = derive_secret(kind, &early[0..$], "derived", &empty_hash[0..$]);
        Vec<u8> handshake_secret = hkdf_extract_with(kind, &derived[0..$], &shared[0..$]);
        Vec<u8> th = transcript_hash(&hs);
        Vec<u8> c_hs = derive_secret(kind, &handshake_secret[0..$], "c hs traffic", &th[0..$]);
        Vec<u8> s_hs = derive_secret(kind, &handshake_secret[0..$], "s hs traffic", &th[0..$]);
        Option<RecordKeys> server_keys = Some(keys_from(&s_hs[0..$], suite));
        Vec<u8> ee = next_message(&mut hs, &mut tcp, &mut server_keys)?;
        if (Vec::len(&ee) < 6 || ee[0] != 8)
        {
            return Err(ProtocolError);
        }
        Vec<u8> cm = next_message(&mut hs, &mut tcp, &mut server_keys)?;
        Vec<Certificate> chain = certificate_list(&cm[0..$])?;
        Certificate leaf = Vec::remove(&mut chain, 0);
        DateTime now = DateTime::now();
        match (verify_chain(&leaf, &chain[0..$], roots, server_name, &now))
        {
            Ok(_)  : {},
            Err(e) : return Err(CertRejected(e)),
        }
        Vec<u8> th_cert = transcript_hash(&hs);
        Vec<u8> cv = next_message(&mut hs, &mut tcp, &mut server_keys)?;
        if (!certificate_verify_ok(&cv[0..$], &leaf, &th_cert[0..$]))
        {
            return Err(HandshakeFailed);
        }
        Vec<u8> th_cv = transcript_hash(&hs);
        Vec<u8> fin = next_message(&mut hs, &mut tcp, &mut server_keys)?;
        Vec<u8> want = finished_data(kind, &s_hs[0..$], &th_cv[0..$]);
        if (Vec::len(&fin) != 4 + Vec::len(&want) || fin[0] != 20 || !digest_eq(&fin[4..$], &want[0..$]))
        {
            return Err(HandshakeFailed);
        }
        if (Vec::len(&hs.buffer) != 0)
        {
            return Err(ProtocolError);
        }
        Vec<u8> th_fin = transcript_hash(&hs);
        Vec<u8> derived2 = derive_secret(kind, &handshake_secret[0..$], "derived", &empty_hash[0..$]);
        Vec<u8> master = hkdf_extract_with(kind, &derived2[0..$], &zeros[0..$]);
        Vec<u8> c_ap = derive_secret(kind, &master[0..$], "c ap traffic", &th_fin[0..$]);
        Vec<u8> s_ap = derive_secret(kind, &master[0..$], "s ap traffic", &th_fin[0..$]);
        // The client's flight: a compatibility ChangeCipherSpec, then Finished
        // under the client handshake keys.
        array<u8, 6> ccs = [20, 0x03, 0x03, 0x00, 0x01, 0x01];
        write_all(&mut tcp, &ccs[0..$])?;
        Vec<u8> mine = finished_data(kind, &c_hs[0..$], &th_fin[0..$]);
        Vec<u8> cfin = Vec::new();
        Vec::push(&mut cfin, 20);
        push_u24(&mut cfin, Vec::len(&mine));
        Vec::extend_from(&mut cfin, &mine[0..$]);
        RecordKeys client_hs = keys_from(&c_hs[0..$], suite);
        Vec<u8> out = seal_record(&mut client_hs, 22, &cfin[0..$]);
        write_all(&mut tcp, &out[0..$])?;
        Ok(TlsStream {
            .tcp = tcp,
            .send = keys_from(&c_ap[0..$], suite),
            .recv = keys_from(&s_ap[0..$], suite),
            .pending = Vec::new(),
            .pending_at = 0,
            .closed = false,
            .peer = Some(leaf),
        })
    }

    // Connects to `addr` and performs the handshake as `client` does.
    export fn TlsStream::connect(
        SocketAddr addr,
        StringView server_name,
        ref<TrustStore, shared> roots,
    ) : Result<TlsStream, TlsError>
    {
        match (TcpStream::connect(addr))
        {
            Ok(tcp) : TlsStream::client(tcp, server_name, roots),
            Err(e)  : Err(NetFailure(e)),
        }
    }

    // Sends all of `data`, in records of at most 16 KiB.
    export fn TlsStream::write(ref<TlsStream, exclusive> s, slice<u8, shared> data) : Result<void, TlsError>
    {
        usize at = 0;
        usize n = slice_len(data);
        while (at < n)
        {
            usize take = min(n - at, 16384);
            Vec<u8> rec = seal_record(&mut s.send, 23, &data[at..at + take]);
            write_all(&mut s.tcp, &rec[0..$])?;
            at += take;
        }
        Ok(())
    }

    export fn TlsStream::write_text(ref<TlsStream, exclusive> s, StringView text) : Result<void, TlsError>
    {
        TlsStream::write(s, text.bytes)
    }

    // Reads records until application data arrives, handling what TLS 1.3
    // sends after the handshake: a NewSessionTicket is ignored, a KeyUpdate
    // changes the keys (and is answered when asked), close_notify ends the
    // stream.
    fn TlsStream::fill(ref<TlsStream, exclusive> s) : Result<void, TlsError>
    {
        while (s.pending_at >= Vec::len(&s.pending) && !s.closed)
        {
            Record r = read_record(&mut s.tcp)?;
            if (r.kind == 20)
            {
                continue;
            }
            Inner { kind, content } = open_record(&mut s.recv, &r)?;
            if (kind == 23)
            {
                overwrite(&mut s.pending, content);
                s.pending_at = 0;
            }
            else if (kind == 21)
            {
                if (Vec::len(&content) == 2 && content[1] == 0)
                {
                    s.closed = true;
                }
                else
                {
                    return Err(PeerAlert(if (Vec::len(&content) == 2) { content[1] } else { 0 }));
                }
            }
            else if (kind == 22)
            {
                usize at = 0;
                while (at + 4 <= Vec::len(&content))
                {
                    u8 t = content[at];
                    usize len = widen<usize>(content[at + 1]) << 16 | widen<usize>(content[at + 2]) << 8 | widen<usize>(content[at + 3]);
                    if (at + 4 + len > Vec::len(&content))
                    {
                        return Err(ProtocolError);
                    }
                    if (t == 24)
                    {
                        if (len != 1)
                        {
                            return Err(ProtocolError);
                        }
                        Vec<u8> none = Vec::new();
                        HashKind kind = suite_hash(s.recv.suite);
                        Vec<u8> next = hkdf_expand_label_with(
                            kind,
                            &s.recv.secret[0..$],
                            "traffic upd",
                            &none[0..$],
                            Vec::len(&s.recv.secret),
                        );
                        overwrite(&mut s.recv, keys_from(&next[0..$], s.recv.suite));
                        if (content[at + 4] == 1)
                        {
                            array<u8, 5> reply = [24, 0, 0, 1, 0];
                            Vec<u8> rec = seal_record(&mut s.send, 22, &reply[0..$]);
                            write_all(&mut s.tcp, &rec[0..$])?;
                            Vec<u8> mine = hkdf_expand_label_with(
                                kind,
                                &s.send.secret[0..$],
                                "traffic upd",
                                &none[0..$],
                                Vec::len(&s.send.secret),
                            );
                            overwrite(&mut s.send, keys_from(&mine[0..$], s.send.suite));
                        }
                    }
                    else if (t != 4)
                    {
                        return Err(ProtocolError);
                    }
                    at = at + 4 + len;
                }
            }
            else
            {
                return Err(ProtocolError);
            }
        }
        Ok(())
    }

    // Appends up to `max` bytes the server sent to `buf`, waiting for some;
    // 0 when the server has closed the connection (close_notify).
    export fn TlsStream::read(ref<TlsStream, exclusive> s, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, TlsError>
    {
        if (max == 0)
        {
            return Ok(0);
        }
        TlsStream::fill(s)?;
        usize have = Vec::len(&s.pending) - s.pending_at;
        usize take = min(have, max);
        Vec::extend_from(buf, &s.pending[s.pending_at..s.pending_at + take]);
        s.pending_at += take;
        Ok(take)
    }

    // Appends exactly `n` bytes to `buf`; `Err(NetFailure(Closed))` when the
    // server closes first.
    export fn TlsStream::read_exact(
        ref<TlsStream, exclusive> s,
        ref<Vec<u8>, exclusive> buf,
        usize n,
    ) : Result<void, TlsError>
    {
        usize got = 0;
        while (got < n)
        {
            usize k = TlsStream::read(s, buf, n - got)?;
            if (k == 0)
            {
                return Err(NetFailure(Closed));
            }
            got += k;
        }
        Ok(())
    }

    // D-0181: the next line the peer sent, without its `\n` (nor a `\r`
    // before it), as `TcpStream::read_line` reads one; `None` when the peer
    // has closed (close_notify) before any byte.
    export fn TlsStream::read_line(ref<TlsStream, exclusive> s) : Result<Option<String>, TlsError>
    {
        Vec<u8> line = Vec::new();
        while (true)
        {
            TlsStream::fill(s)?;
            usize n = Vec::len(&s.pending);
            if (s.pending_at >= n)
            {
                if (Vec::len(&line) == 0)
                {
                    return Ok(None);
                }
                break;
            }
            usize i = s.pending_at;
            while (i < n && s.pending[i] != 10)
            {
                i += 1;
            }
            if (i < n)
            {
                Vec::extend_from(&mut line, &s.pending[s.pending_at..i]);
                s.pending_at = i + 1;
                break;
            }
            Vec::extend_from(&mut line, &s.pending[s.pending_at..n]);
            s.pending_at = n;
        }
        usize k = Vec::len(&line);
        if (k > 0 && line[k - 1] == 13)
        {
            Vec::pop(&mut line);
        }
        match (String::from_utf8(line))
        {
            Ok(t)  : Ok(Some(t)),
            Err(e) : Err(NetFailure(NotUtf8(e))),
        }
    }

    // Sends close_notify (RFC 8446 §6.1); the server may still send what it
    // had in flight. Dropping a `TlsStream` without `close` just closes the
    // connection.
    export fn TlsStream::close(ref<TlsStream, exclusive> s) : Result<void, TlsError>
    {
        array<u8, 2> alert = [1, 0];
        Vec<u8> rec = seal_record(&mut s.send, 21, &alert[0..$]);
        write_all(&mut s.tcp, &rec[0..$])
    }

    // ---- the server side (D-0186) ----
    //
    // `TlsStream::server` answers a client's handshake with the server's own
    // certificate chain and private key: the same suites, groups and
    // signature schemes the client side speaks, chosen in the client's order
    // of preference; a HelloRetryRequest when the client sent a key share
    // for none of the server's groups; no client certificates, no
    // resumption, no early data.

    // What a ClientHello offers.
    struct ClientHello
    {
        Vec<u8> session;
        Vec<u16> suites;
        Vec<u16> groups;                    // supported_groups
        Vec<u16> schemes;                   // signature_algorithms
        Vec<u16> share_groups;              // key_share, in order
        Vec<Vec<u8>> shares;
        bool tls13;
        bool has_cookie;
    }

    fn u16_list(slice<u8, shared> b, usize start, usize end, ref<Vec<u16>, exclusive> out) : bool
    {
        if ((end - start) % 2 != 0)
        {
            return false;
        }
        usize at = start;
        while (at < end)
        {
            Vec::push(out, read_be<u16>(b, at));
            at += 2;
        }
        true
    }

    // A ClientHello (RFC 8446 §4.1.2) taken apart; ProtocolError when
    // malformed.
    fn client_hello_parse(slice<u8, shared> m) : Result<ClientHello, TlsError>
    {
        usize n = slice_len(m);
        if (n < 4 + 2 + 32 + 1 || m[0] != 1)
        {
            return Err(ProtocolError);
        }
        usize at = 38;
        usize sid_len = widen<usize>(m[at]);
        if (sid_len > 32 || at + 1 + sid_len + 2 > n)
        {
            return Err(ProtocolError);
        }
        Vec<u8> session = Vec::from_slice(&m[at + 1..at + 1 + sid_len]);
        at = at + 1 + sid_len;
        usize suites_len = widen<usize>(read_be<u16>(m, at));
        at += 2;
        if (at + suites_len + 1 > n)
        {
            return Err(ProtocolError);
        }
        Vec<u16> suites = Vec::new();
        if (!u16_list(m, at, at + suites_len, &mut suites))
        {
            return Err(ProtocolError);
        }
        at += suites_len;
        usize comp_len = widen<usize>(m[at]);
        at += 1 + comp_len;
        ClientHello ch = ClientHello {
            .session = session,
            .suites = suites,
            .groups = Vec::new(),
            .schemes = Vec::new(),
            .share_groups = Vec::new(),
            .shares = Vec::new(),
            .tls13 = false,
            .has_cookie = false,
        };
        if (at == n)
        {
            return Ok(ch);
        }
        if (at + 2 > n)
        {
            return Err(ProtocolError);
        }
        usize ext_end = at + 2 + widen<usize>(read_be<u16>(m, at));
        if (ext_end != n)
        {
            return Err(ProtocolError);
        }
        at += 2;
        while (at + 4 <= ext_end)
        {
            u16 kind = read_be<u16>(m, at);
            usize len = widen<usize>(read_be<u16>(m, at + 2));
            usize start = at + 4;
            if (start + len > ext_end)
            {
                return Err(ProtocolError);
            }
            if (kind == 0x002b)
            {
                // supported_versions: a 1-byte length and the versions.
                if (len < 1 || widen<usize>(m[start]) + 1 != len)
                {
                    return Err(ProtocolError);
                }
                Vec<u16> versions = Vec::new();
                if (!u16_list(m, start + 1, start + len, &mut versions))
                {
                    return Err(ProtocolError);
                }
                foreach (v in &versions)
                {
                    if (*v == 0x0304)
                    {
                        ch.tls13 = true;
                    }
                }
            }
            else if (kind == 0x000a || kind == 0x000d)
            {
                if (len < 2 || widen<usize>(read_be<u16>(m, start)) + 2 != len)
                {
                    return Err(ProtocolError);
                }
                Vec<u16> list = Vec::new();
                if (!u16_list(m, start + 2, start + len, &mut list))
                {
                    return Err(ProtocolError);
                }
                if (kind == 0x000a)
                {
                    overwrite(&mut ch.groups, list);
                }
                else
                {
                    overwrite(&mut ch.schemes, list);
                }
            }
            else if (kind == 0x0033)
            {
                if (len < 2 || widen<usize>(read_be<u16>(m, start)) + 2 != len)
                {
                    return Err(ProtocolError);
                }
                usize p = start + 2;
                while (p + 4 <= start + len)
                {
                    u16 g = read_be<u16>(m, p);
                    usize klen = widen<usize>(read_be<u16>(m, p + 2));
                    if (p + 4 + klen > start + len)
                    {
                        return Err(ProtocolError);
                    }
                    Vec::push(&mut ch.share_groups, g);
                    Vec::push(&mut ch.shares, Vec::from_slice(&m[p + 4..p + 4 + klen]));
                    p += 4 + klen;
                }
                if (p != start + len)
                {
                    return Err(ProtocolError);
                }
            }
            else if (kind == 0x002c)
            {
                ch.has_cookie = true;
            }
            at = start + len;
        }
        if (at != ext_end)
        {
            return Err(ProtocolError);
        }
        Ok(ch)
    }

    // A plaintext alert (before the handshake keys exist), best effort.
    fn send_plain_alert(ref<TcpStream, exclusive> tcp, u8 code)
    {
        array<u8, 7> rec = [21, 0x03, 0x03, 0, 2, 2, code];
        _ = TcpStream::write(tcp, &rec[0..$]);
    }

    // The ServerHello, or the HelloRetryRequest when `retry` (then `share`
    // is empty and the key_share names the group alone).
    fn server_hello_bytes(
        slice<u8, shared> random,
        slice<u8, shared> session,
        u16 suite,
        u16 group,
        slice<u8, shared> share,
        bool retry,
    ) : Vec<u8>
    {
        Vec<u8> ext = Vec::new();
        array<u8, 2> v13 = [0x03, 0x04];
        push_extension(&mut ext, 0x002b, &v13[0..$]);
        Vec<u8> ks = Vec::new();
        Vec::push_be(&mut ks, group);
        if (!retry)
        {
            Vec::push_be(&mut ks, narrow<u16>(slice_len(share)));
            Vec::extend_from(&mut ks, share);
        }
        push_extension(&mut ext, 0x0033, &ks[0..$]);
        Vec<u8> body = Vec::new();
        Vec::push_be(&mut body, 0x0303: u16);
        Vec::extend_from(&mut body, random);
        Vec::push(&mut body, narrow<u8>(slice_len(session)));
        Vec::extend_from(&mut body, session);
        Vec::push_be(&mut body, suite);
        Vec::push(&mut body, 0);
        Vec::push_be(&mut body, narrow<u16>(Vec::len(&ext)));
        Vec::extend_from(&mut body, &ext[0..$]);
        Vec<u8> msg = Vec::new();
        Vec::push(&mut msg, 2);
        push_u24(&mut msg, Vec::len(&body));
        Vec::extend_from(&mut msg, &body[0..$]);
        msg
    }

    // A handshake message sent encrypted under `keys`, in records of at most
    // 16 KiB, and added to the transcript.
    fn send_handshake(
        ref<TcpStream, exclusive> tcp,
        ref<RecordKeys, exclusive> keys,
        ref<Handshake, exclusive> hs,
        slice<u8, shared> msg,
    ) : Result<void, TlsError>
    {
        transcript_add(hs, msg);
        usize at = 0;
        while (at < slice_len(msg))
        {
            usize take = min(slice_len(msg) - at, 16384);
            Vec<u8> rec = seal_record(keys, 22, &msg[at..at + take]);
            write_all(tcp, &rec[0..$])?;
            at += take;
        }
        Ok(())
    }

    // The signature scheme the server's key signs with, if the client
    // accepts it.
    fn server_scheme(ref<PrivateKey, shared> key, ref<Vec<u16>, shared> offered) : Option<u16>
    {
        u16 want = match (key)
        {
            RsaPrivate(_)     : 0x0804,
            EcPrivate(k)      : if (k.curve == P256) { 0x0403 } else { 0x0503 },
            Ed25519Private(_) : 0x0807,
        };
        foreach (s in offered)
        {
            if (*s == want)
            {
                return Some(want);
            }
        }
        None
    }

    fn scheme_of(u16 code) : SignatureScheme
    {
        if (code == 0x0403) { Ecdsa(Sha2_256) }
        else if (code == 0x0503) { Ecdsa(Sha2_384) }
        else if (code == 0x0807) { Ed25519 }
        else { RsaPss(Sha2_256) }
    }

    // Performs the TLS 1.3 handshake over an accepted `tcp` as the server
    // holding `key` and `chain` (the server's certificate first, then its
    // issuers up to, but not including, the root); the client verifies them.
    // The stream's own timeouts apply to every read and write. A client that
    // does not speak TLS 1.3, or offers no suite, group or signature scheme
    // in common, is answered with an alert and `HandshakeFailed`.
    export fn TlsStream::server(
        TcpStream tcp,
        ref<PrivateKey, shared> key,
        slice<Certificate, shared> chain,
    ) : Result<TlsStream, TlsError>
    {
        if (slice_len(chain) == 0)
        {
            return Err(HandshakeFailed);
        }
        Handshake hs = Handshake { .buffer = Vec::new(), .t256 = Hasher::new(Sha2_256), .t384 = Hasher::new(Sha2_384), .kind = Sha2_256 };
        Option<RecordKeys> plain = None;
        Vec<u8> chm = next_message(&mut hs, &mut tcp, &mut plain)?;
        ClientHello ch = client_hello_parse(&chm[0..$])?;
        if (!ch.tls13)
        {
            send_plain_alert(&mut tcp, 70);
            return Err(HandshakeFailed);
        }
        // The suite: the client's first that this server has.
        u16 suite = 0;
        foreach (s in &ch.suites)
        {
            if (suite == 0 && (*s == 0x1303 || *s == 0x1301 || *s == 0x1302))
            {
                suite = *s;
            }
        }
        u16 scheme_code = match (server_scheme(key, &ch.schemes))
        {
            Some(c) : c,
            None    : 0,
        };
        if (suite == 0 || scheme_code == 0)
        {
            send_plain_alert(&mut tcp, 40);
            return Err(HandshakeFailed);
        }
        hs.kind = suite_hash(suite);
        HashKind kind = hs.kind;
        // The group: the client's first key share for a group this server
        // has; else a HelloRetryRequest for the first group it supports.
        u16 group = 0;
        Vec<u8> client_share = Vec::new();
        for (usize i = 0; i < Vec::len(&ch.share_groups) && group == 0; i += 1)
        {
            u16 g = ch.share_groups[i];
            if (g == 0x001d || g == 0x0017 || g == 0x0018)
            {
                group = g;
                overwrite(&mut client_share, Vec::clone(&ch.shares[i]));
            }
        }
        Vec<u8> session = Vec::clone(&ch.session);
        if (group == 0)
        {
            u16 ask = 0;
            foreach (g in &ch.groups)
            {
                if (ask == 0 && (*g == 0x001d || *g == 0x0017 || *g == 0x0018))
                {
                    ask = *g;
                }
            }
            if (ask == 0 || ch.has_cookie)
            {
                send_plain_alert(&mut tcp, 40);
                return Err(HandshakeFailed);
            }
            // RFC 8446 §4.1.4: the transcript restarts from
            // message_hash(ClientHello1) ‖ HelloRetryRequest.
            Vec<u8> ch_hash = hash(kind, &chm[0..$]);
            array<u8, 4> head = [254, 0, 0, narrow<u8>(Vec::len(&ch_hash))];
            Handshake restarted = Handshake { .buffer = Vec::new(), .t256 = Hasher::new(Sha2_256), .t384 = Hasher::new(Sha2_384), .kind = kind };
            transcript_add(&mut restarted, &head[0..$]);
            transcript_add(&mut restarted, &ch_hash[0..$]);
            Vec<u8> hrr_random = Result::unwrap_or(
                from_hex("cf21ad74e59a6111be1d8c021e65b891c2a211167abb8c5e079e09e2c8a8339c"),
                Vec::new(),
            );
            Vec<u8> none = Vec::new();
            Vec<u8> hrr = server_hello_bytes(&hrr_random[0..$], &session[0..$], suite, ask, &none[0..$], true);
            transcript_add(&mut restarted, &hrr[0..$]);
            overwrite(&mut hs, restarted);
            write_all(&mut tcp, &handshake_record(&hrr[0..$], 0x0303)[0..$])?;
            Vec<u8> chm2 = next_message(&mut hs, &mut tcp, &mut plain)?;
            ClientHello ch2 = client_hello_parse(&chm2[0..$])?;
            bool suite_ok = false;
            foreach (s in &ch2.suites)
            {
                if (*s == suite)
                {
                    suite_ok = true;
                }
            }
            for (usize i = 0; i < Vec::len(&ch2.share_groups) && group == 0; i += 1)
            {
                if (ch2.share_groups[i] == ask)
                {
                    group = ask;
                    overwrite(&mut client_share, Vec::clone(&ch2.shares[i]));
                }
            }
            if (group == 0 || !suite_ok || !ch2.tls13 || !same_session(&ch2.session, &session))
            {
                send_plain_alert(&mut tcp, 47);
                return Err(HandshakeFailed);
            }
        }
        // This server's share and the shared secret.
        Vec<u8> public_key = Vec::new();
        Vec<u8> shared = Vec::new();
        if (group == 0x001d)
        {
            array<u8, 32> private_key = x25519_private_key();
            Vec::extend_from(&mut public_key, &x25519_public_key(&private_key[0..$])[0..$]);
            match (x25519(&private_key[0..$], &client_share[0..$]))
            {
                Some(z) : Vec::extend_from(&mut shared, &z[0..$]),
                None    :
                {
                    send_plain_alert(&mut tcp, 47);
                    return Err(HandshakeFailed);
                },
            }
        }
        else
        {
            EcCurve curve = if (group == 0x0017) { P256 } else { P384 };
            Vec<u8> ec_key = ec_private_key(curve);
            overwrite(&mut public_key, Option::unwrap(ec_public_key(curve, &ec_key[0..$])));
            match (ecdh(curve, &ec_key[0..$], &client_share[0..$]))
            {
                Some(z) : overwrite(&mut shared, z),
                None    :
                {
                    send_plain_alert(&mut tcp, 47);
                    return Err(HandshakeFailed);
                },
            }
        }
        array<u8, 32> random = [0; 32];
        os_random_bytes(&mut random[0..$]);
        Vec<u8> sh = server_hello_bytes(&random[0..$], &session[0..$], suite, group, &public_key[0..$], false);
        transcript_add(&mut hs, &sh[0..$]);
        write_all(&mut tcp, &handshake_record(&sh[0..$], 0x0303)[0..$])?;
        array<u8, 6> ccs = [20, 0x03, 0x03, 0x00, 0x01, 0x01];
        write_all(&mut tcp, &ccs[0..$])?;
        // The key schedule (RFC 8446 §7.1).
        Vec<u8> none = Vec::new();
        Vec<u8> empty_hash = hash(kind, &none[0..$]);
        Vec<u8> zeros = Vec::filled(Vec::len(&empty_hash), 0: u8);
        Vec<u8> early = hkdf_extract_with(kind, &none[0..$], &zeros[0..$]);
        Vec<u8> derived = derive_secret(kind, &early[0..$], "derived", &empty_hash[0..$]);
        Vec<u8> handshake_secret = hkdf_extract_with(kind, &derived[0..$], &shared[0..$]);
        Vec<u8> th = transcript_hash(&hs);
        Vec<u8> c_hs = derive_secret(kind, &handshake_secret[0..$], "c hs traffic", &th[0..$]);
        Vec<u8> s_hs = derive_secret(kind, &handshake_secret[0..$], "s hs traffic", &th[0..$]);
        RecordKeys server_hs = keys_from(&s_hs[0..$], suite);
        // EncryptedExtensions (none), Certificate, CertificateVerify, Finished.
        array<u8, 6> ee = [8, 0, 0, 2, 0, 0];
        send_handshake(&mut tcp, &mut server_hs, &mut hs, &ee[0..$])?;
        Vec<u8> list = Vec::new();
        foreach (c in chain)
        {
            push_u24(&mut list, Vec::len(&c.der));
            Vec::extend_from(&mut list, &c.der[0..$]);
            Vec::push_be(&mut list, 0: u16);
        }
        Vec<u8> cert = Vec::new();
        Vec::push(&mut cert, 11);
        push_u24(&mut cert, 4 + Vec::len(&list));
        Vec::push(&mut cert, 0);
        push_u24(&mut cert, Vec::len(&list));
        Vec::extend_from(&mut cert, &list[0..$]);
        send_handshake(&mut tcp, &mut server_hs, &mut hs, &cert[0..$])?;
        Vec<u8> th_cert = transcript_hash(&hs);
        Vec<u8> content = Vec::filled(64, 0x20: u8);
        Vec::extend_from(&mut content, &b"TLS 1.3, server CertificateVerify"[0..$]);
        Vec::push(&mut content, 0);
        Vec::extend_from(&mut content, &th_cert[0..$]);
        SignatureScheme scheme = scheme_of(scheme_code);
        Vec<u8> signature = match (sign(key, &scheme, &content[0..$]))
        {
            Some(s) : s,
            None    : return Err(HandshakeFailed),
        };
        Vec<u8> cv = Vec::new();
        Vec::push(&mut cv, 15);
        push_u24(&mut cv, 4 + Vec::len(&signature));
        Vec::push_be(&mut cv, scheme_code);
        Vec::push_be(&mut cv, narrow<u16>(Vec::len(&signature)));
        Vec::extend_from(&mut cv, &signature[0..$]);
        send_handshake(&mut tcp, &mut server_hs, &mut hs, &cv[0..$])?;
        Vec<u8> th_cv = transcript_hash(&hs);
        Vec<u8> fin_data = finished_data(kind, &s_hs[0..$], &th_cv[0..$]);
        Vec<u8> fin = Vec::new();
        Vec::push(&mut fin, 20);
        push_u24(&mut fin, Vec::len(&fin_data));
        Vec::extend_from(&mut fin, &fin_data[0..$]);
        send_handshake(&mut tcp, &mut server_hs, &mut hs, &fin[0..$])?;
        Vec<u8> th_fin = transcript_hash(&hs);
        Vec<u8> derived2 = derive_secret(kind, &handshake_secret[0..$], "derived", &empty_hash[0..$]);
        Vec<u8> master = hkdf_extract_with(kind, &derived2[0..$], &zeros[0..$]);
        Vec<u8> c_ap = derive_secret(kind, &master[0..$], "c ap traffic", &th_fin[0..$]);
        Vec<u8> s_ap = derive_secret(kind, &master[0..$], "s ap traffic", &th_fin[0..$]);
        // The client's Finished, under its handshake keys.
        Option<RecordKeys> client_hs = Some(keys_from(&c_hs[0..$], suite));
        Vec<u8> cfin = next_message(&mut hs, &mut tcp, &mut client_hs)?;
        Vec<u8> want = finished_data(kind, &c_hs[0..$], &th_fin[0..$]);
        if (Vec::len(&cfin) != 4 + Vec::len(&want) || cfin[0] != 20 || !digest_eq(&cfin[4..$], &want[0..$]))
        {
            return Err(HandshakeFailed);
        }
        if (Vec::len(&hs.buffer) != 0)
        {
            return Err(ProtocolError);
        }
        Ok(TlsStream {
            .tcp = tcp,
            .send = keys_from(&s_ap[0..$], suite),
            .recv = keys_from(&c_ap[0..$], suite),
            .pending = Vec::new(),
            .pending_at = 0,
            .closed = false,
            .peer = None,
        })
    }

    fn same_session(ref<Vec<u8>, shared> a, ref<Vec<u8>, shared> b) : bool
    {
        Vec::len(a) == Vec::len(b) && digest_eq(&(*a)[0..$], &(*b)[0..$])
    }

**Depends on:** rule.stdlib.crypto, rule.stdlib.x509, rule.stdlib.net, D-0161
**Affects:** the network (rule.stdlib.net)

## 2p. HTTP

In `std::http` (D-0173, D-0174): the errors, URLs, forms, headers and
messages; the client in `std::http::client` and the server in
`std::http::server`, both re-exported (D-0187).

### `rule.stdlib.http`
**Status:** ACCEPTED

HTTP/1.1 (RFC 9110, RFC 9112) over `std::net` and `std::tls`: a client
that sends one request on its own connection and reads the whole
response, following redirects within limits, and a server that reads
requests from a connection and writes responses, keeping the connection
as the client negotiated. HTTP/1.1 only; no compression, cookies, proxies
or HTTP/2; the server speaks plain HTTP.

    [Url-Parse]        (D-0173, RFC 3986)   ⟨Url::parse(t), Σ⟩ → Ok(u) when t is scheme "://" authority
                       [path] ["?" query] ["#" fragment] with a non-empty authority holding no "@": u.scheme
                       and u.host lower-cased (a bracketed IPv6 host kept bracketed), u.port the number after
                       the host's ":" or the scheme's default (443 for https, 80 otherwise), u.path "/" when
                       empty, u.query the text after "?" (empty when none), the fragment dropped; Err(BadUrl)
                       otherwise; Url::target(u) = path ‖ "?" query (the query only when non-empty);
                       Url::text(u) = scheme "://" host [":" port, when not the default] ‖ target
    [Percent]          (D-0173, RFC 3986 §2.1)   ⟨percent_encode(b), Σ⟩ → each unreserved byte (ALPHA, DIGIT,
                       "-", ".", "_", "~") as itself and every other as "%" and two upper-case hex digits;
                       ⟨percent_decode(t), Σ⟩ → Some(the bytes, each "%XX" decoded) or None for a "%" not
                       followed by two hex digits
    [Header-Find]      (D-0173)   ⟨find_header(hs, n), Σ⟩ → Some(the value of the first header whose name
                       equals n ignoring ASCII case), None when none does; Request::header and
                       Response::header are this over the message's headers
    [Http-Send]        (D-0173)   ⟨HttpClient::send(c, r), Σ⟩ → the response to r (its url parsed by
                       [Url-Parse]; a scheme other than http or https is UnsupportedScheme): a TCP connection
                       to the host's addresses in turn, each within c.timeout_ms, with that timeout on reads
                       and writes; for https a TLS connection by [Tls-Client] with the server name the host
                       and the roots c.roots, loaded by TrustStore::system at the first https request (NoRoots
                       when that fails); the request line r.method ‖ " " ‖ target ‖ " HTTP/1.1", Host
                       (host, and ":" port when not the default), c.headers then r.headers (a header of r
                       replaces c's of the same name; Host, Content-Length and Connection of r are ignored),
                       Content-Length when r.body is non-empty or the method is POST, PUT or PATCH,
                       Connection: close (unless c.keep_alive), then r.body; interim responses (status 1xx)
                       are read past; the response's status, reason, headers and body ([Http-Body]) are given
                       and the connection closed, or kept ([Http-Keep-Alive])
    [Http-Keep-Alive]  (D-0195)   with c.keep_alive (false from HttpClient::new): a connection is kept after a
                       response when it was HTTP/1.1, had no Connection: close, its body's end was known
                       without the connection's (no body, Content-Length or chunked), and nothing beyond it
                       was read; the client keeps at most one, and the next exchange with the same origin
                       (scheme, host, port) is made on it. If that connection proves closed before any of the
                       response arrives (the write fails, or the stream ends or fails with nothing read), the
                       exchange is made again on a fresh connection when the method is idempotent (GET, HEAD,
                       PUT, DELETE, OPTIONS, TRACE: RFC 9110 §9.2.2), and is `ConnectionClosed` for another
                       (POST, PATCH: the server may have acted on it); any other failure is the request's.
                       A kept connection to another origin is closed when a request needs a new one
    [Http-Body]        (D-0173, RFC 9112 §6)   a response to HEAD, or with status 204 or 304, has no body;
                       otherwise with Transfer-Encoding: chunked the body is the chunks' data concatenated
                       (chunk extensions and trailers read and dropped); otherwise with Content-Length n
                       exactly n bytes; otherwise everything to the close; a Transfer-Encoding that is not
                       "chunked", or one beside a Content-Length, a Content-Length that is not a number, or
                       a malformed chunk size is BadMessage; a head (status or request line and headers)
                       over 65536 bytes, or a body over the limit (c.max_body, or the server's), is
                       TooLarge; a stream ending inside a message is ConnectionClosed
    [Http-Redirect]    (D-0173, RFC 9110 §15.4)   a response with status 301, 302, 303, 307 or 308 and a
                       Location is followed: Location taken as absolute when it holds "://", as a path on
                       the same scheme, host and port when it begins with "/", and relative to the directory
                       of the current path otherwise; 301, 302 and 303 by GET without the body (HEAD stays
                       HEAD), 307 and 308 with the method and body; after c.max_redirects redirects the
                       next is TooManyRedirects; a redirect without Location is given as it is;
                       http_get(url) and http_post(url, t, b) are send with a fresh HttpClient::new (a 30 s
                       timeout, 10 redirects, 16 MiB, a User-Agent header) and a GET, or a POST with
                       Content-Type t and body b
    [Http-Error]       (D-0173)   a network failure is NetFailure(e), a TLS failure TlsFailure(e); every
                       HttpError has a text; a refused message gives nothing of itself
    [Http-Accept]      (D-0174)   ⟨HttpServer::bind(a), Σ⟩ → Ok(a server listening at a) with max_body
                       16 MiB and timeout_ms 30 s, or the listener's Err; ⟨HttpServer::accept(s), Σ⟩ → the
                       next client's connection, its reads and writes under s.timeout_ms
    [Http-Request]     (D-0174, RFC 9112 §3)   ⟨HttpConnection::request(c), Σ⟩ → Ok(Some(r)) with r.method,
                       r.url (the target as sent) and r.headers from a request line "METHOD SP target SP
                       HTTP/1.0 | HTTP/1.1" and the headers after it, and r.body by [Http-Body] (no body
                       without Content-Length or chunked), after "HTTP/1.1 100 Continue" when the request
                       expects 100-continue; Ok(None) when the client closed before a request or the last
                       response ended the connection; Err(BadMessage) for another request line, a 1.1
                       request without Host, or a malformed header or body; Err(TooLarge) by [Http-Body];
                       after an Err the connection closes. The connection stays open for the next request
                       when the request is HTTP/1.1 without Connection: close, or HTTP/1.0 with
                       Connection: keep-alive
    [Http-Respond]     (D-0174)   ⟨HttpConnection::respond(c, st, hs, b), Σ⟩ → writes "HTTP/1.1 " st " "
                       reason_phrase(st), the headers hs (their Content-Length and Connection dropped),
                       Content-Length: |b| unless st is 1xx, 204 or 304, Connection: keep-alive or close as
                       negotiated, and b, except after a HEAD request or when st has no body; a response
                       that ends the connection closes it, and request then gives None;
                       ⟨reason_phrase(st), Σ⟩ → RFC 9110's phrase for st, "" for a status it does not list

A client sends `Connection: close` and reads to the end, so a server
that neither frames its body nor closes keeps it waiting until the
timeout; with `keep_alive` set (D-0195) it sends no `Connection` header
and reuses the connection while the server allows, so a server meant for
such a client must not count on one connection per request. A server program handles each connection in a thread of its own
(`spawn`), as the guide shows.


    [Form]               (D-0181)   ⟨form_decode(t), Σ⟩ → the Params of t's parts between "&" (and ";"), skipping empty
                         ones: each part's name before its first "=" and value after it ("" when none), each with "+"
                         read as a space and then percent-decoded (a part that fails to decode, or is not UTF-8, is
                         kept as given); names compare exactly (find_param). ⟨form_encode(ps), Σ⟩ → "name=value"
                         pairs joined by "&", each name and value percent-encoded with a space as "+"; form_decode of
                         it gives ps. ⟨Url::query_pairs(u), Σ⟩ → form_decode(u.query)
    [Content-Encoding]   (D-0183)   HttpClient::new's headers include "Accept-Encoding: gzip, deflate". In [Http-Send], a
                         response with a body and "Content-Encoding: gzip" (or x-gzip) has its body replaced by
                         gunzip(body, max_body), one with "deflate" by zlib_inflate(body, max_body) or, when that is
                         not a zlib stream, inflate(body, max_body); then the Content-Encoding and Content-Length
                         headers are dropped. Oversize is Err(TooLarge), another failure Err(BadMessage); any other
                         encoding leaves the body and the header as they came
    [Https-Bind]         (D-0186)   ⟨HttpServer::bind_tls(a, key, chain), Σ⟩ → Ok(s) bound as [Http-Bind] with key and chain
                         kept; [Http-Accept] on it is TcpListener::accept, the timeouts and nodelay set, then
                         TlsStream::server(tcp, key, chain): Ok(a connection over the TlsStream), or
                         Err(TlsFailure(e)) and the server ready for the next client. HttpServer::accept gives
                         Result<HttpConnection, HttpError> (Err(NetFailure(e)) for the listener's failures)

The `std` source:

    // `std::http` (spec/21 §2p, D-0173, D-0174): HTTP/1.1 (RFC 9110, RFC 9112)
    // over `std::net` and `std::tls`. A client that fetches a URL or posts to
    // one, following redirects and decoding chunked bodies, with limits on
    // what it will read; and a server that accepts connections, parses their
    // requests and writes responses, keeping a connection open for the next
    // request when the client asks. Written in CobaltC. HTTP/1.1 only: no
    // HTTP/2, no compression, no cookies, no proxies; a TLS server is not
    // here (`std::tls` has the client side).

    // Why a request or a connection failed.
    export enum HttpError
    {
        NetFailure(NetError),
        TlsFailure(TlsError),
        BadUrl,
        UnsupportedScheme,
        NoRoots,
        BadMessage,
        TooLarge,
        TooManyRedirects,
        ConnectionClosed,
    }

    export fn HttpError::text(ref<HttpError, shared> e) : str
    {
        match (e)
        {
            NetFailure(n)     : NetError::text(n),
            TlsFailure(t)     : TlsError::text(t),
            BadUrl            : "the URL is malformed",
            UnsupportedScheme : "the URL's scheme is neither http nor https",
            NoRoots           : "no trusted roots for https (TrustStore::system failed)",
            BadMessage        : "the HTTP message is malformed",
            TooLarge          : "the HTTP message is larger than the limit",
            TooManyRedirects  : "too many redirects",
            ConnectionClosed  : "the connection was closed",
        }
    }

    // ---- URLs ----

    // A parsed `scheme://host[:port]/path?query`; the fragment, which never
    // reaches a server, is dropped. `port` is the scheme's default when the
    // text names none; `path` is "/" when it names none.
    export struct Url
    {
        export String scheme;
        export String host;
        export u16 port;
        export String path;
        export String query;
    }

    fn default_port(StringView scheme) : u16
    {
        if (StringView::eq(scheme, "https")) { 443 } else { 80 }
    }

    // `Err(BadUrl)` for text without `://`, with no host, with user
    // information (`user@`), or with a port that is not a number.
    export fn Url::parse(StringView text) : Result<Url, HttpError>
    {
        usize sep = match (StringView::find(text, "://"))
        {
            Some(i) : i,
            None    : return Err(BadUrl),
        };
        String scheme = String::to_ascii_lower(&String::from_view(StringView::sub(text, 0, sep)));
        StringView rest = StringView::sub(text, sep + 3, StringView::len(text));
        usize n = StringView::len(rest);
        slice<u8, shared> b = StringView::as_bytes(rest);
        usize auth_end = 0;
        while (auth_end < n && b[auth_end] != b'/' && b[auth_end] != b'?' && b[auth_end] != b'#')
        {
            auth_end += 1;
        }
        StringView authority = StringView::sub(rest, 0, auth_end);
        if (auth_end == 0 || Option::is_some(&StringView::find(authority, "@")))
        {
            return Err(BadUrl);
        }
        slice<u8, shared> ab = StringView::as_bytes(authority);
        usize host_end = auth_end;
        if (ab[0] == b'[')
        {
            host_end = match (StringView::find(authority, "]"))
            {
                Some(i) : i + 1,
                None    : return Err(BadUrl),
            };
        }
        else
        {
            host_end = 0;
            while (host_end < auth_end && ab[host_end] != b':')
            {
                host_end += 1;
            }
        }
        String host = String::to_ascii_lower(&String::from_view(StringView::sub(authority, 0, host_end)));
        u16 port = default_port(String::as_view(&scheme));
        if (host_end < auth_end)
        {
            if (ab[host_end] != b':')
            {
                return Err(BadUrl);
            }
            port = match (StringView::parse<u16>(StringView::sub(authority, host_end + 1, auth_end)))
            {
                Ok(p)  : p,
                Err(_) : return Err(BadUrl),
            };
        }
        usize path_end = auth_end;
        while (path_end < n && b[path_end] != b'?' && b[path_end] != b'#')
        {
            path_end += 1;
        }
        String path = if (path_end == auth_end) { String::from_str("/") } else
        {
            String::from_view(StringView::sub(
                rest,
                auth_end,
                path_end,
            ))
        };
        String query = String::new();
        if (path_end < n && b[path_end] == b'?')
        {
            usize q_end = path_end + 1;
            while (q_end < n && b[q_end] != b'#')
            {
                q_end += 1;
            }
            String::append(&mut query, StringView::sub(rest, path_end + 1, q_end));
        }
        Ok(Url { .scheme = scheme, .host = host, .port = port, .path = path, .query = query })
    }

    // The request target: the path, and `?query` when there is one.
    export fn Url::target(ref<Url, shared> u) : String
    {
        String t = String::clone(&u.path);
        if (String::len(&u.query) > 0)
        {
            String::append(&mut t, "?");
            String::append(&mut t, String::as_view(&u.query));
        }
        t
    }

    // The host as a `Host` header or a URL writes it: with the port when it
    // is not the scheme's default.
    fn host_text(ref<Url, shared> u) : String
    {
        String h = String::clone(&u.host);
        if (u.port != default_port(String::as_view(&u.scheme)))
        {
            String::append(&mut h, ":");
            String::append(&mut h, u.port);
        }
        h
    }

    // The URL as text again.
    export fn Url::text(ref<Url, shared> u) : String
    {
        String t = String::clone(&u.scheme);
        String::append(&mut t, "://");
        String::append(&mut t, String::as_view(&host_text(u)));
        String::append(&mut t, String::as_view(&Url::target(u)));
        t
    }

    fn is_unreserved(u8 b) : bool
    {
        ascii_is_alnum(b) || b == b'-' || b == b'.' || b == b'_' || b == b'~'
    }

    // RFC 3986 percent-encoding: unreserved bytes as they are, every other
    // as `%XX` (upper-case hex).
    export fn percent_encode(slice<u8, shared> bytes) : String
    {
        String out = String::new();
        str digits = "0123456789ABCDEF";
        foreach (c in bytes)
        {
            if (is_unreserved(*c))
            {
                String::push_ascii(&mut out, *c);
            }
            else
            {
                String::push_ascii(&mut out, b'%');
                String::push_ascii(&mut out, str_byte(digits, widen<usize>(*c >> 4)));
                String::push_ascii(&mut out, str_byte(digits, widen<usize>(*c & 15)));
            }
        }
        out
    }

    fn hex_digit(u8 c) : Option<u8>
    {
        if (c >= b'0' && c <= b'9') { Some(c - b'0') }
        else if (c >= b'a' && c <= b'f') { Some(c - b'a' + 10) }
        else if (c >= b'A' && c <= b'F') { Some(c - b'A' + 10) }
        else { None }
    }

    // The bytes `text` percent-encodes; None for a `%` not followed by two
    // hex digits.
    export fn percent_decode(StringView text) : Option<Vec<u8>>
    {
        slice<u8, shared> b = StringView::as_bytes(text);
        Vec<u8> out = Vec::new();
        usize i = 0;
        while (i < slice_len(b))
        {
            if (b[i] == b'%')
            {
                if (i + 3 > slice_len(b))
                {
                    return None;
                }
                u8 hi = hex_digit(b[i + 1])?;
                u8 lo = hex_digit(b[i + 2])?;
                Vec::push(&mut out, hi * 16 + lo);
                i += 3;
            }
            else
            {
                Vec::push(&mut out, b[i]);
                i += 1;
            }
        }
        Some(out)
    }

    // ---- D-0181: query strings and form bodies ----

    // One name and value of a query string or an
    // `application/x-www-form-urlencoded` body. Names compare exactly (unlike
    // a header's).
    export struct Param
    {
        export String name;
        export String value;
    }

    export fn Param::new(StringView name, StringView value) : Param
    {
        Param { .name = String::from_view(name), .value = String::from_view(value) }
    }

    // The value of the first parameter named `name`, exactly.
    export fn find_param(slice<Param, shared> params, str name) : Option<StringView>
    {
        for (usize i = 0; i < slice_len(params); i += 1)
        {
            if (String::eq_str(&params[i].name, name))
            {
                return Some(String::as_view(&params[i].value));
            }
        }
        None
    }

    // The parameters of `text`, a query string (without its `?`) or a form
    // body: `name=value` pairs separated by `&` (or `;`), each name and value
    // percent-decoded with `+` read as a space; a pair without `=` has an
    // empty value; empty pairs are skipped. A name or value that is not
    // UTF-8 after decoding, or a bad percent-escape, is kept as the text
    // given, so nothing is lost.
    export fn form_decode(StringView text) : Vec<Param>
    {
        Vec<Param> out = Vec::new();
        foreach (pair in StringView::split(text, "&"))
        {
            foreach (p in StringView::split(pair, ";"))
            {
                if (StringView::len(p) == 0)
                {
                    continue;
                }
                StringView name = p;
                StringView value = StringView::of("");
                if (Some(eq) = StringView::find(p, "="))
                {
                    name = StringView::sub(p, 0, eq);
                    value = StringView::sub(p, eq + 1, StringView::len(p));
                }
                Vec::push(&mut out, Param { .name = form_part(name), .value = form_part(value) });
            }
        }
        out
    }

    // One part of a form, decoded: `+` a space, then percent-decoding.
    fn form_part(StringView v) : String
    {
        String plus = StringView::replace(v, "+", " ");
        match (percent_decode(String::as_view(&plus)))
        {
            Some(b) : match (String::from_utf8(b))
            {
                Ok(s)  : s,
                Err(_) : plus,
            },
            None : plus,
        }
    }

    // `params` as a query string or form body: `name=value` pairs joined by
    // `&`, each name and value percent-encoded (a space as `+`), so
    // `form_decode` reads it back. Without a leading `?`.
    export fn form_encode(slice<Param, shared> params) : String
    {
        String out = String::new();
        for (usize i = 0; i < slice_len(params); i += 1)
        {
            if (i > 0)
            {
                String::push_ascii(&mut out, b'&');
            }
            String::append(&mut out, String::as_view(&form_encode_part(&params[i].name)));
            String::push_ascii(&mut out, b'=');
            String::append(&mut out, String::as_view(&form_encode_part(&params[i].value)));
        }
        out
    }

    fn form_encode_part(ref<String, shared> s) : String
    {
        String enc = percent_encode(StringView::as_bytes(String::as_view(s)));
        String::replace(&enc, "%20", "+")
    }

    // The parameters of the URL's query, by `form_decode`.
    export fn Url::query_pairs(ref<Url, shared> u) : Vec<Param>
    {
        form_decode(String::as_view(&u.query))
    }

    // ---- headers and messages ----

    // One header field. Names compare without regard to case.
    export struct Header
    {
        export String name;
        export String value;
    }

    export fn Header::new(str name, StringView value) : Header
    {
        Header { .name = String::from_str(name), .value = String::from_view(value) }
    }

    fn name_eq(StringView a, str b) : bool
    {
        ascii_ieq_str(a.bytes, b)
    }

    fn names_eq(StringView a, StringView b) : bool
    {
        ascii_ieq(a.bytes, b.bytes)
    }

    // Whether two names are equal ignoring ASCII case, on the bytes: shared
    // slices of plain bytes, read unchecked.
    fn ascii_ieq(slice<u8, shared> a, slice<u8, shared> b) : bool
    {
        if (slice_len(a) != slice_len(b))
        {
            return false;
        }
        for (usize i = 0; i < slice_len(a); i += 1)
        {
            if (ascii_to_lower(a[i]) != ascii_to_lower(b[i]))
            {
                return false;
            }
        }
        true
    }

    fn ascii_ieq_str(slice<u8, shared> a, str b) : bool
    {
        if (slice_len(a) != str_len(b))
        {
            return false;
        }
        for (usize i = 0; i < slice_len(a); i += 1)
        {
            if (ascii_to_lower(a[i]) != ascii_to_lower(str_byte(b, i)))
            {
                return false;
            }
        }
        true
    }

    fn has_header(slice<Header, shared> headers, StringView name) : bool
    {
        for (usize i = 0; i < slice_len(headers); i += 1)
        {
            if (names_eq(String::as_view(&headers[i].name), name))
            {
                return true;
            }
        }
        false
    }

    // The value of the first header named `name`, whatever its case.
    export fn find_header(slice<Header, shared> headers, str name) : Option<StringView>
    {
        for (usize i = 0; i < slice_len(headers); i += 1)
        {
            if (name_eq(String::as_view(&headers[i].name), name))
            {
                return Some(String::as_view(&headers[i].value));
            }
        }
        None
    }

    // A request: for the client, `url` is absolute (`https://…`); for a
    // request a server received, it is the target as sent (`/path?query`).
    export struct Request
    {
        export String method;
        export String url;
        export Vec<Header> headers;
        export Vec<u8> body;
    }

    export fn Request::new(str method, StringView url) : Request
    {
        Request { .method = String::from_str(method), .url = String::from_view(url), .headers = Vec::new(), .body = Vec::new() }
    }

    export fn Request::header(ref<Request, shared> r, str name) : Option<StringView>
    {
        find_header(&r.headers[0..$], name)
    }

    // A response: the status, its reason phrase, the headers and the whole
    // body (decoded if it was chunked).
    export struct Response
    {
        export u16 status;
        export String reason;
        export Vec<Header> headers;
        export Vec<u8> body;
    }

    export fn Response::header(ref<Response, shared> r, str name) : Option<StringView>
    {
        find_header(&r.headers[0..$], name)
    }

    // D-0193: the body as text: a copy of its bytes, which must be UTF-8.
    export fn Response::text(ref<Response, shared> r) : Result<String, Utf8Error>
    {
        String::from_utf8(Vec::clone(&r.body))
    }

    // The reason phrase of a status code, as RFC 9110 lists them.
    export fn reason_phrase(u16 status) : str
    {
        match (status)
        {
            100 : "Continue",
            101 : "Switching Protocols",
            200 : "OK",
            201 : "Created",
            202 : "Accepted",
            204 : "No Content",
            206 : "Partial Content",
            301 : "Moved Permanently",
            302 : "Found",
            303 : "See Other",
            304 : "Not Modified",
            307 : "Temporary Redirect",
            308 : "Permanent Redirect",
            400 : "Bad Request",
            401 : "Unauthorized",
            403 : "Forbidden",
            404 : "Not Found",
            405 : "Method Not Allowed",
            408 : "Request Timeout",
            409 : "Conflict",
            411 : "Length Required",
            413 : "Content Too Large",
            415 : "Unsupported Media Type",
            429 : "Too Many Requests",
            500 : "Internal Server Error",
            501 : "Not Implemented",
            502 : "Bad Gateway",
            503 : "Service Unavailable",
            504 : "Gateway Timeout",
            _   : "",
        }
    }

    // The most a message's head (request or status line and headers) may
    // take, and a chunk-size line.
    const usize MAX_HEAD = 65536;

    // ---- a buffered reader over a plain or a TLS connection ----

    enum Conn
    {
        Plain(TcpStream),
        Secure(TlsStream),
    }

    fn conn_read(ref<Conn, exclusive> c, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, HttpError>
    {
        match (c)
        {
            Plain(s) : match (TcpStream::read(s, buf, max))
            {
                Ok(n)  : Ok(n),
                Err(e) : Err(NetFailure(e)),
            },
            Secure(s) : match (TlsStream::read(s, buf, max))
            {
                Ok(n)  : Ok(n),
                Err(e) : Err(TlsFailure(e)),
            },
        }
    }

    fn conn_write(ref<Conn, exclusive> c, slice<u8, shared> data) : Result<void, HttpError>
    {
        match (c)
        {
            Plain(s) : match (TcpStream::write(s, data))
            {
                Ok(_)  : Ok(()),
                Err(e) : Err(NetFailure(e)),
            },
            Secure(s) : match (TlsStream::write(s, data))
            {
                Ok(_)  : Ok(()),
                Err(e) : Err(TlsFailure(e)),
            },
        }
    }

    fn conn_close(ref<Conn, exclusive> c)
    {
        match (c)
        {
            Plain(s)  : { _ = TcpStream::shutdown_write(s); },
            Secure(s) : { _ = TlsStream::close(s); },
        }
    }

    // Bytes read ahead of what the parser has consumed.
    struct Reader
    {
        Conn conn;
        Vec<u8> buf;
        usize at;
    }

    // More bytes into the buffer; false at the end of the stream.
    fn fill_buffer(ref<Reader, exclusive> r) : Result<bool, HttpError>
    {
        if (r.at > 0 && r.at == Vec::len(&r.buf))
        {
            Vec::clear(&mut r.buf);
            r.at = 0;
        }
        usize n = conn_read(&mut r.conn, &mut r.buf, 16384)?;
        Ok(n > 0)
    }

    fn available(ref<Reader, shared> r) : usize
    {
        Vec::len(&r.buf) - r.at
    }

    // A line without its CRLF (a bare LF is accepted too); None at the end
    // of the stream before any byte; TooLarge past `limit` bytes.
    // The first newline in `b` from `at`, or its length: the bytes are the
    // function's only reference, read unchecked.
    fn http_newline_from(slice<u8, shared> b, usize at) : usize
    {
        usize i = at;
        while (i < slice_len(b) && b[i] != b'\n')
        {
            i += 1;
        }
        i
    }

    fn http_line(ref<Reader, exclusive> r, usize limit) : Result<Option<String>, HttpError>
    {
        usize scanned = 0;
        while (true)
        {
            usize i = http_newline_from(&r.buf[0..$], r.at + scanned);
            if (i < Vec::len(&r.buf))
            {
                usize end = if (i > r.at && r.buf[i - 1] == b'\r') { i - 1 } else { i };
                Vec<u8> bytes = Vec::from_slice(&r.buf[r.at..end]);
                r.at = i + 1;
                return match (String::from_utf8(bytes))
                {
                    Ok(s)  : Ok(Some(s)),
                    Err(_) : Err(BadMessage),
                };
            }
            scanned = Vec::len(&r.buf) - r.at;
            if (scanned > limit)
            {
                return Err(TooLarge);
            }
            if (!fill_buffer(r)?)
            {
                return if (scanned == 0) { Ok(None) } else { Err(BadMessage) };
            }
        }
    }

    // Exactly `n` bytes appended to `out`; ConnectionClosed when the stream ends first.
    fn read_n(ref<Reader, exclusive> r, ref<Vec<u8>, exclusive> out, usize n) : Result<void, HttpError>
    {
        usize need = n;
        while (need > 0)
        {
            if (available(r) == 0)
            {
                if (!fill_buffer(r)?)
                {
                    return Err(ConnectionClosed);
                }
            }
            usize take = min(need, available(r));
            Vec::extend_from(out, &r.buf[r.at..r.at + take]);
            r.at += take;
            need -= take;
        }
        Ok(())
    }

    // Everything to the end of the stream, at most `max` bytes.
    fn read_to_end(ref<Reader, exclusive> r, ref<Vec<u8>, exclusive> out, usize max) : Result<void, HttpError>
    {
        while (true)
        {
            usize have = available(r);
            if (have > 0)
            {
                if (Vec::len(out) + have > max)
                {
                    return Err(TooLarge);
                }
                Vec::extend_from(out, &r.buf[r.at..$]);
                r.at = Vec::len(&r.buf);
            }
            if (!fill_buffer(r)?)
            {
                return Ok(());
            }
        }
    }

    // Header lines up to the empty line; the head may not exceed MAX_HEAD.
    fn read_headers(ref<Reader, exclusive> r, usize used) : Result<Vec<Header>, HttpError>
    {
        Vec<Header> headers = Vec::new();
        usize total = used;
        while (true)
        {
            String line = match (http_line(r, MAX_HEAD)?)
            {
                Some(l) : l,
                None    : return Err(BadMessage),
            };
            total += String::len(&line) + 2;
            if (total > MAX_HEAD)
            {
                return Err(TooLarge);
            }
            if (String::len(&line) == 0)
            {
                return Ok(headers);
            }
            StringView v = String::as_view(&line);
            usize colon = match (StringView::find(v, ":"))
            {
                Some(i) : i,
                None    : return Err(BadMessage),
            };
            StringView name = StringView::sub(v, 0, colon);
            if (colon == 0 || StringView::len(StringView::trim(name)) != colon)
            {
                return Err(BadMessage);
            }
            Vec::push(
                &mut headers,
                Header { .name = String::from_view(name), .value = String::from_view(StringView::trim(StringView::sub(
                    v,
                    colon + 1,
                    StringView::len(v),
                ))) },
            );
        }
    }

    // How a message's body is delimited (RFC 9112 §6).
    enum BodyLength
    {
        Length(usize),
        Chunked,
        UntilClose,
        NoBody,
    }

    fn body_length(slice<Header, shared> headers, bool response) : Result<BodyLength, HttpError>
    {
        Option<StringView> te = find_header(headers, "transfer-encoding");
        Option<StringView> cl = find_header(headers, "content-length");
        if (Some(t) = te)
        {
            if (Option::is_some(&cl) || !name_eq(StringView::trim(t), "chunked"))
            {
                return Err(BadMessage);
            }
            return Ok(Chunked);
        }
        if (Some(c) = cl)
        {
            return match (StringView::parse<usize>(StringView::trim(c)))
            {
                Ok(n)  : Ok(Length(n)),
                Err(_) : Err(BadMessage),
            };
        }
        Ok(if (response) { UntilClose } else { NoBody })
    }

    // The body, decoded, at most `max` bytes; the chunked trailers are read
    // and dropped.
    fn read_body(ref<Reader, exclusive> r, BodyLength how, usize max) : Result<Vec<u8>, HttpError>
    {
        Vec<u8> body = Vec::new();
        match (how)
        {
            NoBody     : {},
            Length(n)  :
            {
                if (n > max)
                {
                    return Err(TooLarge);
                }
                read_n(r, &mut body, n)?;
            },
            UntilClose : read_to_end(r, &mut body, max)?,
            Chunked    :
            {
                while (true)
                {
                    String line = match (http_line(r, MAX_HEAD)?)
                    {
                        Some(l) : l,
                        None    : return Err(ConnectionClosed),
                    };
                    StringView size_text = String::as_view(&line);
                    if (Some(semi) = StringView::find(size_text, ";"))
                    {
                        size_text = StringView::sub(size_text, 0, semi);
                    }
                    usize size = 0;
                    slice<u8, shared> sb = StringView::as_bytes(StringView::trim(size_text));
                    if (slice_len(sb) == 0 || slice_len(sb) > 8)
                    {
                        return Err(BadMessage);
                    }
                    foreach (c in sb)
                    {
                        size = size * 16 + widen<usize>(Option::ok_or(hex_digit(*c), BadMessage)?);
                    }
                    if (size == 0)
                    {
                        break;
                    }
                    if (Vec::len(&body) + size > max)
                    {
                        return Err(TooLarge);
                    }
                    read_n(r, &mut body, size)?;
                    match (http_line(r, 2)?)
                    {
                        Some(end) : if (String::len(&end) != 0) { return Err(BadMessage); },
                        None      : return Err(ConnectionClosed),
                    }
                }
                // Trailers, up to the empty line.
                while (true)
                {
                    match (http_line(r, MAX_HEAD)?)
                    {
                        Some(t) : if (String::len(&t) == 0) { break; },
                        None    : return Err(ConnectionClosed),
                    }
                }
            },
        }
        Ok(body)
    }

    // ---- the client (D-0173) ----

    // What a client carries from request to request: the roots `https`
    // verifies against (the system's, loaded at the first https request, or
    // a store the program puts there), the timeout for connecting and for
    // each read and write, how many redirects to follow, the most a body may
    // be, and headers sent with every request. A program that makes several
    // https requests keeps one client, so the roots are loaded once.
    // D-0195: with `keep_alive` set, the client keeps the last connection open
    // when the server allows it, and sends the next request to the same origin
    // on it; otherwise (the default) each request has a connection of its own.
    export struct HttpClient
    {
        export Option<TrustStore> roots;
        export u64 timeout_ms;
        export usize max_redirects;
        export usize max_body;
        export Vec<Header> headers;
        export bool keep_alive;
        Option<IdleConn> idle;
    }

    // A connection kept for the next request to `origin` (scheme://host:port).
    struct IdleConn
    {
        String origin;
        Reader r;
    }

    // A client with no roots loaded yet, a 30 s timeout, up to 10 redirects,
    // a 16 MiB body limit, and a `User-Agent` header.
    export fn HttpClient::new() : HttpClient
    {
        Vec<Header> headers = Vec::new();
        Vec::push(&mut headers, Header::new("User-Agent", "CobaltC-std-http/1"));
        Vec::push(&mut headers, Header::new("Accept-Encoding", "gzip, deflate"));
        HttpClient { .roots = None, .timeout_ms = 30000, .max_redirects = 10, .max_body = 16777216, .headers = headers, .keep_alive = false, .idle = None }
    }

    fn method_has_body(StringView m) : bool
    {
        StringView::eq(m, "POST") || StringView::eq(m, "PUT") || StringView::eq(m, "PATCH")
    }

    // Opens a connection to `u`: TCP, then TLS for https, the system roots
    // loaded into the client the first time they are needed.
    fn open_connection(ref<HttpClient, exclusive> c, ref<Url, shared> u) : Result<Conn, HttpError>
    {
        bool secure = StringView::eq(String::as_view(&u.scheme), "https");
        if (!secure && !StringView::eq(String::as_view(&u.scheme), "http"))
        {
            return Err(UnsupportedScheme);
        }
        if (secure && Option::is_none(&c.roots))
        {
            match (TrustStore::system())
            {
                Ok(t)  : overwrite(&mut c.roots, Some(t)),
                Err(_) : return Err(NoRoots),
            }
        }
        // A bracketed IPv6 host loses its brackets for resolution.
        String host = String::clone(&u.host);
        if (String::starts_with(&host, "[") && String::ends_with(&host, "]"))
        {
            overwrite(&mut host, String::from_view(StringView::sub(String::as_view(&u.host), 1, String::len(&u.host) - 1)));
        }
        Vec<SocketAddr> addrs = match (IpAddr::parse(String::as_view(&host)))
        {
            Ok(ip) :
            {
                Vec<SocketAddr> one = Vec::new();
                Vec::push(&mut one, SocketAddr::new(ip, u.port));
                one
            },
            Err(_) : match (resolve(String::as_view(&host), u.port))
            {
                Ok(a)  : a,
                Err(e) : return Err(NetFailure(e)),
            },
        };
        if (Vec::len(&addrs) == 0)
        {
            return Err(NetFailure(UnknownHost));
        }
        NetError last = Failed;
        for (usize i = 0; i < Vec::len(&addrs); i += 1)
        {
            TcpStream tcp = match (TcpStream::connect_timeout(addrs[i], c.timeout_ms))
            {
                Ok(t)  : t,
                Err(e) :
                {
                    last = e;
                    continue;
                },
            };
            TcpStream::set_read_timeout_ms(&mut tcp, c.timeout_ms);
            TcpStream::set_write_timeout_ms(&mut tcp, c.timeout_ms);
            TcpStream::set_nodelay(&mut tcp, true);
            if (!secure)
            {
                return Ok(Plain(tcp));
            }
            ref<TrustStore, shared> roots = match (&c.roots)
            {
                Some(r) : r,
                None    : return Err(NoRoots),
            };
            return match (TlsStream::client(tcp, String::as_view(&host), roots))
            {
                Ok(s)  : Ok(Secure(s)),
                Err(e) : Err(TlsFailure(e)),
            };
        }
        Err(NetFailure(last))
    }

    // The request as bytes: the request line, the Host, the client's and the
    // request's headers (the request's win), a Content-Length for a body,
    // and Connection: close unless the client keeps connections alive.
    fn request_bytes(
        ref<HttpClient, shared> c,
        ref<Request, shared> req,
        ref<Url, shared> u,
        StringView method,
        slice<u8, shared> body,
        bool with_content,
    ) : Vec<u8>
    {
        String head = String::from_view(method);
        String::append(&mut head, " ");
        String::append(&mut head, String::as_view(&Url::target(u)));
        String::append(&mut head, " HTTP/1.1\r\nHost: ");
        String::append(&mut head, String::as_view(&host_text(u)));
        String::append(&mut head, "\r\n");
        foreach (h in &c.headers)
        {
            if (!has_header(&req.headers[0..$], String::as_view(&h.name)))
            {
                String::append(&mut head, String::as_view(&h.name));
                String::append(&mut head, ": ");
                String::append(&mut head, String::as_view(&h.value));
                String::append(&mut head, "\r\n");
            }
        }
        foreach (h in &req.headers)
        {
            if (name_eq(
                String::as_view(&h.name),
                "host",
            ) || name_eq(String::as_view(&h.name), "content-length") || name_eq(String::as_view(&h.name), "connection"))
            {
                continue;
            }
            // After a redirect that dropped the body, its Content-Type goes too.
            if (!with_content && name_eq(String::as_view(&h.name), "content-type"))
            {
                continue;
            }
            String::append(&mut head, String::as_view(&h.name));
            String::append(&mut head, ": ");
            String::append(&mut head, String::as_view(&h.value));
            String::append(&mut head, "\r\n");
        }
        if (slice_len(body) > 0 || method_has_body(method))
        {
            String::append(&mut head, "Content-Length: ");
            String::append(&mut head, slice_len(body));
            String::append(&mut head, "\r\n");
        }
        if (!c.keep_alive)
        {
            String::append(&mut head, "Connection: close\r\n");
        }
        String::append(&mut head, "\r\n");
        Vec<u8> out = String::into_bytes(head);
        Vec::extend_from(&mut out, body);
        out
    }

    fn not_content_encoding(ref<Header, shared> h) : bool
    {
        !name_eq(String::as_view(&h.name), "content-encoding") && !name_eq(String::as_view(&h.name), "content-length")
    }

    // The status line `HTTP/1.x NNN reason`.
    struct StatusLine
    {
        u16 status;
        String reason;
    }

    fn parse_status_line(StringView line) : Result<StatusLine, HttpError>
    {
        if (!StringView::starts_with(line, "HTTP/1.") || StringView::len(line) < 12)
        {
            return Err(BadMessage);
        }
        u16 status = match (StringView::parse<u16>(StringView::sub(line, 9, 12)))
        {
            Ok(v)  : v,
            Err(_) : return Err(BadMessage),
        };
        if (status < 100 || status > 599)
        {
            return Err(BadMessage);
        }
        String reason = if (StringView::len(line) > 13)
        {
            String::from_view(StringView::sub(
                line,
                13,
                StringView::len(line),
            ))
        }
        else { String::new() };
        Ok(StatusLine { .status = status, .reason = reason })
    }

    fn origin_of(ref<Url, shared> u) : String
    {
        String o = String::clone(&u.scheme);
        String::append(&mut o, "://");
        String::append(&mut o, String::as_view(&host_text(u)));
        o
    }

    // The methods whose effect is the same however often they are sent
    // (RFC 9110 §9.2.2).
    fn idempotent(StringView method) : bool
    {
        method == "GET" || method == "HEAD" || method == "PUT" || method == "DELETE" || method == "OPTIONS" || method == "TRACE"
    }

    // One exchange: on the kept connection to `u`'s origin if there is one
    // (D-0195), again on a fresh one if that connection turns out closed
    // before any of the response arrived and the method is idempotent; else on
    // a fresh connection.
    fn exchange(
        ref<HttpClient, exclusive> c,
        ref<Request, shared> req,
        ref<Url, shared> u,
        StringView method,
        slice<u8, shared> body,
        bool with_content,
    ) : Result<Response, HttpError>
    {
        String want = origin_of(u);
        if (Some(ic) = replace(&mut c.idle, None))
        {
            IdleConn { origin, r } = ic;
            if (origin == want)
            {
                if (Some(resp) = exchange_on(c, r, true, req, u, method, body, with_content)?)
                {
                    return Ok(resp);
                }
                // Closed with nothing answered: the server may still have
                // acted on the request, so only an idempotent one is sent
                // again (RFC 9110 §9.2.2) -- a POST twice could do twice what
                // it asks.
                if (!idempotent(method))
                {
                    return Err(ConnectionClosed);
                }
            }
            else
            {
                conn_close(&mut r.conn);
            }
        }
        Reader fresh = Reader { .conn = open_connection(c, u)?, .buf = Vec::new(), .at = 0 };
        match (exchange_on(c, fresh, false, req, u, method, body, with_content)?)
        {
            Some(resp) : Ok(resp),
            None       : Err(ConnectionClosed),
        }
    }

    // The exchange on `r`. None when `r` was a kept connection (`reused`) that
    // failed before any of the response arrived: the caller tries a fresh one.
    fn exchange_on(
        ref<HttpClient, exclusive> c,
        Reader r,
        bool reused,
        ref<Request, shared> req,
        ref<Url, shared> u,
        StringView method,
        slice<u8, shared> body,
        bool with_content,
    ) : Result<Option<Response>, HttpError>
    {
        match (conn_write(&mut r.conn, &request_bytes(c, req, u, method, body, with_content)[0..$]))
        {
            Ok(_)  : {},
            Err(e) :
            {
                if (reused)
                {
                    return Ok(None);
                }
                return Err(e);
            },
        }
        bool first = true;
        // Interim 1xx responses (a 100 Continue) are read past.
        while (true)
        {
            String line = match (http_line(&mut r, MAX_HEAD))
            {
                Ok(Some(l)) : l,
                Ok(None)    :
                {
                    if (reused && first)
                    {
                        return Ok(None);
                    }
                    return Err(ConnectionClosed);
                },
                Err(e)      :
                {
                    if (reused && first && Vec::len(&r.buf) == 0)
                    {
                        return Ok(None);
                    }
                    return Err(e);
                },
            };
            first = false;
            bool http11 = StringView::starts_with(String::as_view(&line), "HTTP/1.1");
            StatusLine { status, reason } = parse_status_line(String::as_view(&line))?;
            Vec<Header> headers = read_headers(&mut r, String::len(&line) + 2)?;
            if (status >= 100 && status < 200)
            {
                continue;
            }
            bool no_body = StringView::eq(method, "HEAD") || status == 204 || status == 304;
            BodyLength how = if (no_body) { NoBody } else { body_length(&headers[0..$], true)? };
            bool until_close = match (how)
            {
                UntilClose : true,
                _          : false,
            };
            Vec<u8> data = read_body(&mut r, how, c.max_body)?;
            // Kept when asked for, allowed (HTTP/1.1, no `Connection: close`),
            // the body's end known without the connection's, and nothing more
            // read than the response.
            bool keep = c.keep_alive && http11 && !until_close && r.at == Vec::len(&r.buf);
            if (Some(v) = find_header(&headers[0..$], "connection"))
            {
                if (name_eq(StringView::trim(v), "close"))
                {
                    keep = false;
                }
            }
            if (keep)
            {
                c.idle = Some(IdleConn { .origin = String::clone(&origin_of(u)), .r = r });
            }
            else
            {
                conn_close(&mut r.conn);
            }
            // D-0183: a body sent compressed (`Content-Encoding: gzip` or
            // `deflate`, which the client offers) arrives decoded, within
            // `max_body`, and the header is dropped so the response reads as
            // a plain one.
            bool strip = false;
            if (!no_body)
            {
                if (Some(enc) = find_header(&headers[0..$], "content-encoding"))
                {
                    StringView e = StringView::trim(enc);
                    Option<Result<Vec<u8>, CompressError>> decoded = if (name_eq(e, "gzip") || name_eq(e, "x-gzip"))
                    {
                        Some(gunzip(&data[0..$], c.max_body))
                    }
                    else if (name_eq(e, "deflate"))
                    {
                        Some(match (zlib_inflate(&data[0..$], c.max_body))
                        {
                            Ok(v)  : Ok(v),
                            Err(_) : inflate(&data[0..$], c.max_body),
                        })
                    }
                    else { None };
                    if (Some(d) = decoded)
                    {
                        overwrite(&mut data, match (d)
                        {
                            Ok(v)         : v,
                            Err(Oversize) : return Err(TooLarge),
                            Err(_)        : return Err(BadMessage),
                        });
                        strip = true;
                    }
                }
            }
            if (strip)
            {
                Vec::retain(&mut headers, not_content_encoding);
            }
            return Ok(Some(Response { .status = status, .reason = reason, .headers = headers, .body = data }));
        }
    }

    // `location` resolved against `base`: absolute, or a path on the same
    // origin, or relative to the base path's directory.
    fn redirect_target(ref<Url, shared> base, StringView location) : Result<Url, HttpError>
    {
        if (Option::is_some(&StringView::find(location, "://")))
        {
            return Url::parse(location);
        }
        String text = String::clone(&base.scheme);
        String::append(&mut text, "://");
        String::append(&mut text, String::as_view(&host_text(base)));
        if (StringView::starts_with(location, "/"))
        {
            String::append(&mut text, location);
        }
        else
        {
            StringView path = String::as_view(&base.path);
            usize dir = 0;
            slice<u8, shared> pb = StringView::as_bytes(path);
            for (usize i = 0; i < slice_len(pb); i += 1)
            {
                if (pb[i] == b'/')
                {
                    dir = i + 1;
                }
            }
            String::append(&mut text, StringView::sub(path, 0, dir));
            String::append(&mut text, location);
        }
        Url::parse(String::as_view(&text))
    }

    // Sends `req`, following up to `max_redirects` redirects (301, 302 and
    // 303 by GET without the body, 307 and 308 with the method and body
    // kept), and gives the final response. Each request opens its own
    // connection and closes it.
    export fn HttpClient::send(ref<HttpClient, exclusive> c, ref<Request, shared> req) : Result<Response, HttpError>
    {
        Url u = Url::parse(String::as_view(&req.url))?;
        String method = String::clone(&req.method);
        Vec<u8> body = Vec::clone(&req.body);
        usize hops = 0;
        bool with_content = true;
        while (true)
        {
            Response resp = exchange(c, req, &u, String::as_view(&method), &body[0..$], with_content)?;
            bool redirect = resp.status == 301 || resp.status == 302 || resp.status == 303 || resp.status == 307 || resp.status == 308;
            if (!redirect)
            {
                return Ok(resp);
            }
            StringView location = match (Response::header(&resp, "location"))
            {
                Some(l) : l,
                None    : return Ok(resp),
            };
            if (hops == c.max_redirects)
            {
                return Err(TooManyRedirects);
            }
            hops += 1;
            Url next = redirect_target(&u, location)?;
            overwrite(&mut u, next);
            if (resp.status != 307 && resp.status != 308 && !StringView::eq(String::as_view(&method), "HEAD"))
            {
                overwrite(&mut method, String::from_str("GET"));
                Vec::clear(&mut body);
                with_content = false;
            }
        }
    }

    // A GET of `url` with a fresh default client.
    export fn http_get(StringView url) : Result<Response, HttpError>
    {
        HttpClient c = HttpClient::new();
        Request req = Request::new("GET", url);
        HttpClient::send(&mut c, &req)
    }

    // A POST of `body` as `content_type` to `url` with a fresh default client.
    export fn http_post(StringView url, str content_type, slice<u8, shared> body) : Result<Response, HttpError>
    {
        HttpClient c = HttpClient::new();
        Request req = Request::new("POST", url);
        Vec::push(&mut req.headers, Header::new("Content-Type", StringView::of(content_type)));
        Vec::extend_from(&mut req.body, body);
        HttpClient::send(&mut c, &req)
    }

    // ---- the server (D-0174) ----

    // A listening HTTP server: `accept` gives each client's connection. The
    // limits apply to every connection it accepts.
    export resource struct HttpServer
    {
        TcpListener listener;
        export usize max_body;
        export u64 timeout_ms;
        Option<PrivateKey> key;             // D-0186: set for an https server
        Vec<Certificate> chain;
    }

    // Bound to `a` (port 0 for any free port), with a 16 MiB body limit and
    // a 30 s timeout per read and write.
    export fn HttpServer::bind(SocketAddr a) : Result<HttpServer, NetError>
    {
        TcpListener l = TcpListener::bind(a)?;
        Ok(HttpServer { .listener = l, .max_body = 16777216, .timeout_ms = 30000, .key = None, .chain = Vec::new() })
    }

    // D-0186: an https server: bound as `bind`, and every connection it
    // accepts first completes a TLS 1.3 handshake with `key` and `chain`
    // (the server's certificate, then its issuers) through
    // `TlsStream::server`.
    export fn HttpServer::bind_tls(SocketAddr a, PrivateKey key, Vec<Certificate> chain) : Result<HttpServer, NetError>
    {
        TcpListener l = TcpListener::bind(a)?;
        Ok(HttpServer { .listener = l, .max_body = 16777216, .timeout_ms = 30000, .key = Some(key), .chain = chain })
    }

    export fn HttpServer::local_addr(ref<HttpServer, shared> s) : SocketAddr
    {
        TcpListener::local_addr(&s.listener)
    }

    // Waits for the next client; for an https server, through its
    // handshake (a client that fails it is `TlsFailure`, and the server goes
    // on to the next).
    export fn HttpServer::accept(ref<HttpServer, exclusive> s) : Result<HttpConnection, HttpError>
    {
        TcpStream tcp = match (TcpListener::accept(&mut s.listener))
        {
            Ok(t)  : t,
            Err(e) : return Err(NetFailure(e)),
        };
        TcpStream::set_read_timeout_ms(&mut tcp, s.timeout_ms);
        TcpStream::set_write_timeout_ms(&mut tcp, s.timeout_ms);
        TcpStream::set_nodelay(&mut tcp, true);
        Conn conn = match (&s.key)
        {
            Some(k) : match (TlsStream::server(tcp, k, &s.chain[0..$]))
            {
                Ok(t)  : Secure(t),
                Err(e) : return Err(TlsFailure(e)),
            },
            None : Plain(tcp),
        };
        Ok(HttpConnection { .reader = Reader { .conn = conn, .buf = Vec::new(), .at = 0 }, .max_body = s.max_body, .keep_alive = true, .head_request = false, .closed = false })
    }

    // One client's connection: `request` then `respond`, repeated while the
    // client keeps it open.
    export resource struct HttpConnection
    {
        Reader reader;
        usize max_body;
        bool keep_alive;
        bool head_request;
        bool closed;
    }

    export fn HttpConnection::peer_addr(ref<HttpConnection, shared> c) : SocketAddr
    {
        match (&c.reader.conn)
        {
            Plain(s)  : TcpStream::peer_addr(s),
            Secure(s) : TlsStream::peer_addr(s),
        }
    }

    // The next request, or None when the client has closed the connection
    // (or asked for it to close after the last response). A malformed
    // request is `BadMessage` and a too-large one `TooLarge`; the caller
    // answers 400 or 413 if it wishes, and the connection then closes.
    export fn HttpConnection::request(ref<HttpConnection, exclusive> c) : Result<Option<Request>, HttpError>
    {
        if (c.closed || !c.keep_alive)
        {
            return Ok(None);
        }
        String line = match (http_line(&mut c.reader, MAX_HEAD)?)
        {
            Some(l) : l,
            None    : return Ok(None),
        };
        // A request line: METHOD SP target SP HTTP/1.x.
        Vec<StringView> parts = StringView::split(String::as_view(&line), " ");
        if (Vec::len(&parts) != 3 || StringView::len(parts[0]) == 0 || StringView::len(parts[1]) == 0)
        {
            c.keep_alive = false;
            return Err(BadMessage);
        }
        bool v11 = StringView::eq(parts[2], "HTTP/1.1");
        if (!v11 && !StringView::eq(parts[2], "HTTP/1.0"))
        {
            c.keep_alive = false;
            return Err(BadMessage);
        }
        Vec<Header> headers = match (read_headers(&mut c.reader, String::len(&line) + 2))
        {
            Ok(h)  : h,
            Err(e) :
            {
                c.keep_alive = false;
                return Err(e);
            },
        };
        if (v11 && Option::is_none(&find_header(&headers[0..$], "host")))
        {
            c.keep_alive = false;
            return Err(BadMessage);
        }
        // Keep-alive: HTTP/1.1 unless `Connection: close`, HTTP/1.0 only with
        // `Connection: keep-alive`.
        c.keep_alive = match (find_header(&headers[0..$], "connection"))
        {
            Some(v) : if (v11)
            {
                !name_eq(
                    StringView::trim(v),
                    "close",
                )
            }
            else { name_eq(StringView::trim(v), "keep-alive") },
            None : v11,
        };
        c.head_request = StringView::eq(parts[0], "HEAD");
        BodyLength how = match (body_length(&headers[0..$], false))
        {
            Ok(h)  : h,
            Err(e) :
            {
                c.keep_alive = false;
                return Err(e);
            },
        };
        if (Some(expect) = find_header(&headers[0..$], "expect"))
        {
            if (name_eq(StringView::trim(expect), "100-continue"))
            {
                conn_write(&mut c.reader.conn, &b"HTTP/1.1 100 Continue\r\n\r\n"[0..$])?;
            }
        }
        Vec<u8> body = match (read_body(&mut c.reader, how, c.max_body))
        {
            Ok(b)  : b,
            Err(e) :
            {
                c.keep_alive = false;
                return Err(e);
            },
        };
        Ok(Some(Request { .method = String::from_view(parts[0]), .url = String::from_view(parts[1]), .headers = headers, .body = body }))
    }

    // Writes a response: the status line (RFC 9110's reason phrase), the
    // headers given, Content-Length, and Connection as negotiated; the body
    // is omitted after a HEAD request. After a response that ends the
    // connection, `request` gives None.
    export fn HttpConnection::respond(
        ref<HttpConnection, exclusive> c,
        u16 status,
        slice<Header, shared> headers,
        slice<u8, shared> body,
    ) : Result<void, HttpError>
    {
        String head = String::from_str("HTTP/1.1 ");
        String::append(&mut head, status);
        String::append(&mut head, " ");
        String::append(&mut head, reason_phrase(status));
        String::append(&mut head, "\r\n");
        for (usize i = 0; i < slice_len(headers); i += 1)
        {
            if (name_eq(
                String::as_view(&headers[i].name),
                "content-length",
            ) || name_eq(String::as_view(&headers[i].name), "connection"))
            {
                continue;
            }
            String::append(&mut head, String::as_view(&headers[i].name));
            String::append(&mut head, ": ");
            String::append(&mut head, String::as_view(&headers[i].value));
            String::append(&mut head, "\r\n");
        }
        bool no_body = status == 204 || status == 304 || (status >= 100 && status < 200);
        if (!no_body)
        {
            String::append(&mut head, "Content-Length: ");
            String::append(&mut head, slice_len(body));
            String::append(&mut head, "\r\n");
        }
        String::append(
            &mut head,
            if (c.keep_alive) { "Connection: keep-alive\r\n\r\n" } else { "Connection: close\r\n\r\n" },
        );
        Vec<u8> out = String::into_bytes(head);
        if (!no_body && !c.head_request)
        {
            Vec::extend_from(&mut out, body);
        }
        conn_write(&mut c.reader.conn, &out[0..$])?;
        if (!c.keep_alive)
        {
            conn_close(&mut c.reader.conn);
            c.closed = true;
        }
        Ok(())
    }

    // Ends the connection.
    export fn HttpConnection::close(ref<HttpConnection, exclusive> c)
    {
        if (!c.closed)
        {
            conn_close(&mut c.reader.conn);
            c.closed = true;
        }
    }

**Depends on:** rule.stdlib.net, rule.stdlib.tls, rule.stdlib.text, D-0173, D-0174
**Affects:** the network (rule.stdlib.net)

## 2q. PostgreSQL

In `std::database::postgres` (D-0180, D-0187), re-exported by
`std::database`, which declares the SCRAM-SHA-256 client every database
client shares.

### `rule.stdlib.postgres`
**Status:** ACCEPTED

A client of PostgreSQL's wire protocol (version 3.0) over `std::net` and
`std::tls`, authenticating by SCRAM-SHA-256 (RFC 5802, RFC 7677) with
`std::crypto`: a connection opened from a URL, one statement at a time
with its parameters bound through the extended query protocol, the rows
in the server's text form, and scripts through the simple query
protocol. Written in CobaltC; nothing is linked. No binary rows, cursors,
COPY, LISTEN/NOTIFY, cancellation or channel binding; a second backend,
and a layer over several, are later decisions.

    [Pg-Url]           (D-0180)   ⟨PgUrl::parse(t), Σ⟩ → Ok(u) when t is ("postgres" | "postgresql") "://" user
                       [":" password] "@" host [":" port] ["/" [database]] ["?sslmode=" ("disable" | "verify-full")],
                       u.user, u.password and u.database percent-decoded (RFC 3986 §2.1) as UTF-8, u.host a name,
                       an IPv4 literal or a bracketed IPv6 literal (kept without its brackets), u.port 5432 when
                       none, u.database the user when none or empty, u.sslmode VerifyFull when none;
                       Err(BadConnectionUrl) otherwise: another scheme, no user, no host, a port that is not a
                       number, another sslmode or any other query key, a bad percent-encoding
    [Pg-Connect]       (D-0180)   ⟨PgConnection::connect_with(t, roots, ms), Σ⟩ → a connection by the u of
                       [Pg-Url]: TCP to the host's addresses in turn (an IP literal as it is, a name as resolve
                       gives them, §2l), each within ms, which then bounds every read and write (the first
                       address that connects is used; Err(NetFailure(e)) when none does); for VerifyFull an
                       SSLRequest, the server's "S" followed by the TLS handshake of §2o (TlsStream::client with
                       u.host as the server name and roots; its failure is TlsFailure(e)), its "N" by
                       Err(TlsRefused), anything else ProtocolViolation; then StartupMessage (protocol 3.0;
                       user, database, client_encoding UTF8), [Pg-Auth], every ParameterStatus kept
                       (PgConnection::parameter(c, n) → Some(the value reported for n), None when none),
                       BackendKeyData kept, every NoticeResponse as [Pg-Notice], to ReadyForQuery; c.max_result
                       is 16 MiB. ⟨PgConnection::connect(t), Σ⟩ → connect_with with 30 000 ms and
                       TrustStore::system (Err(NoTrustedRoots) when it fails; not loaded for Disable).
                       PgConnection::close(c) sends Terminate and closes the connection; a connection dropped
                       without close is closed without it
    [Pg-Auth]          (D-0180, RFC 5802, RFC 7677)   AuthenticationSASL that offers SCRAM-SHA-256 is answered
                       with SASLInitialResponse "n,,n=,r=" ‖ nonce (the base64 of 18 random bytes); the
                       server's first message "r=" nonce' ",s=" salt ",i=" count (nonce' beginning with nonce
                       and longer, else AuthFailed; malformed, an "m=" extension, a count of 0, a salt that is
                       not base64: ProtocolViolation) is answered with "c=biws,r=" nonce' ",p=" proof, the
                       proof by RFC 5802 §3 from the salted password PBKDF2-HMAC-SHA-256(password, salt,
                       count, 32) of the password as given (no SASLprep); the server's final message "v="
                       signature is verified (another signature, or "e=", is AuthFailed), then
                       AuthenticationOk. AuthenticationCleartextPassword is answered with the password only
                       on a TLS connection (AuthRefused otherwise); AuthenticationMD5Password, a SASL offer
                       without SCRAM-SHA-256 and every other method are UnsupportedAuth; an ErrorResponse
                       during the exchange whose SQLSTATE begins with "28" is AuthFailed, another Server(e).
                       The password is u.password, "" when none. Scram::start(user, nonce) (user with "," and
                       "=" as "=2C" and "=3D"), Scram::client_first, Scram::respond(s, server_first, password)
                       and Scram::verify(s, server_final) are the client's steps of this exchange, and
                       scram_salted_password(password, salt, count) its salted password, exported
    [Pg-Query]         (D-0180)   ⟨PgConnection::query(c, sql, ps), Σ⟩ → the result of sql with $1 … $n bound
                       to ps through Parse (the unnamed statement, no parameter types), Bind (the unnamed
                       portal, every parameter in text format: Null as NULL, Text(s) as s, Int(i) as its
                       decimal text, Float(f) as its shortest round-trip text or "Infinity", "-Infinity",
                       "NaN", Bool(b) as "t" | "f", Bytes(b) as "\x" ‖ the hex of b; PgValue::text(v) is this
                       text, None for Null), Describe (the portal), Execute (no row limit) and Sync, then
                       [Pg-Result]; more than 65 535 parameters is Err(ResultTooLarge); on a closed connection
                       Err(Disconnected)
    [Pg-Result]        (D-0180)   the messages to ReadyForQuery: RowDescription gives res.columns (name,
                       type_oid), each DataRow a row of res.rows (a cell NULL as None, every other as its text;
                       a cell that is not UTF-8 is ProtocolViolation), CommandComplete res.tag and res.affected
                       (the tag's last word when it is a number, 0 otherwise), NoticeResponse as [Pg-Notice],
                       ParseComplete, BindComplete, NoData, EmptyQueryResponse, PortalSuspended,
                       NotificationResponse and ParameterStatus nothing; the first ErrorResponse is kept and
                       the statement is Err(Server(e)) once ReadyForQuery arrives (e.severity, e.code, e.message,
                       e.detail, e.hint, e.position from the fields V or S, C, M, D, H, P); rows whose bytes
                       together exceed c.max_result are dropped and the statement is Err(ResultTooLarge) once
                       ReadyForQuery arrives; either way the connection stays usable. A message that does not
                       parse, or one the protocol does not allow here (a COPY among them), is
                       Err(ProtocolViolation); a network failure Err(NetFailure(e)), a TLS failure
                       Err(TlsFailure(e)), the stream ending Err(Disconnected); after any of these the
                       connection is closed and every later query, execute or run_script gives
                       Err(Disconnected). PgRow::get(r, i) → Some(the cell's text) or None (NULL, or i at or
                       past the row's length); PgResult::column(res, n) → Some(the index of the first column
                       named n exactly) or None
    [Pg-Execute]       (D-0180)   ⟨PgConnection::execute(c, sql, ps), Σ⟩ → Ok(res.affected) of [Pg-Query], or
                       its Err
    [Pg-Script]        (D-0180)   ⟨PgConnection::run_script(c, sql), Σ⟩ → Ok(()) after Query sql (the simple
                       query protocol: statements separated by ";", no parameters) and the messages to
                       ReadyForQuery as [Pg-Result] reads them with every row dropped, the first ErrorResponse
                       Err(Server(e)); on a closed connection Err(Disconnected)
    [Pg-Notice]        (D-0180)   a NoticeResponse received at any point is appended to c.notices as a
                       PgServerError (its fields as [Pg-Result] reads an ErrorResponse's); the program reads
                       and clears them
    [Pg-Error]         (D-0180)   every PgError has a text (PgError::text, a String; a server error's is
                       severity ": " message " (" code ")"); a refused message gives nothing of itself

A connection is one thread's: a program that queries from several
threads keeps a connection per thread. Transactions are statements
(`execute("BEGIN")`, `execute("COMMIT")`, `execute("ROLLBACK")`).


    [Pg-Cell]   (D-0181)   ⟨PgRow::get_i64(r, i), Σ⟩ → Some(n) when get(r, i) is Some(t) and t is number(i64), else None;
                get_f64: number(f64), or "NaN", "Infinity", "-Infinity" as PostgreSQL writes them; get_bool: "t" or
                "f"; get_bytes: "\\x" followed by hexadecimal (bytea's hex form), decoded. None for NULL, a column past
                the last, or text of another form, the text itself still given by get

The `std` source:

    // `std::database` (spec/21 §2q, D-0180): a PostgreSQL client speaking the
    // server's wire protocol (version 3.0) over `std::net` and `std::tls`,
    // with SCRAM-SHA-256 authentication from `std::crypto`. A connection is
    // opened from a URL, runs one statement at a time with its parameters
    // bound (never spliced into the SQL text) and gives the rows back in the
    // server's text form. Written in CobaltC; nothing is linked. No binary
    // rows, cursors, COPY, LISTEN/NOTIFY, cancellation or channel binding.
    // The items carry the `Pg` prefix so a later client may share the module.

    // Why a connection or a statement failed.
    export enum PgError
    {
        NetFailure(NetError),
        TlsFailure(TlsError),
        TlsRefused,
        NoTrustedRoots,
        BadConnectionUrl,
        UnsupportedAuth,
        AuthRefused,
        AuthFailed,
        Server(PgServerError),
        ProtocolViolation,
        ResultTooLarge,
        Disconnected,
    }

    // What the server reported: an ErrorResponse or a NoticeResponse, its
    // severity ("ERROR", "FATAL", "NOTICE", ...), its five-character SQLSTATE
    // code, its message, and the detail, hint and character position (from
    // 1 into the statement) when it gave them.
    export struct PgServerError
    {
        export String severity;
        export String code;
        export String message;
        export Option<String> detail;
        export Option<String> hint;
        export Option<u32> position;
    }

    // The error as text; a server error as `severity: message (code)`.
    export fn PgError::text(ref<PgError, shared> e) : String
    {
        match (e)
        {
            NetFailure(n)     : String::from_str(NetError::text(n)),
            TlsFailure(t)     : String::from_str(TlsError::text(t)),
            TlsRefused        : String::from_str("the server does not accept TLS"),
            NoTrustedRoots    : String::from_str("no trusted roots for TLS (TrustStore::system failed)"),
            BadConnectionUrl  : String::from_str("the connection URL is malformed"),
            UnsupportedAuth   : String::from_str("the server asks for an authentication method that is not supported"),
            AuthRefused       : String::from_str("the server asks for the password in clear over an unencrypted connection"),
            AuthFailed        : String::from_str("authentication failed"),
            Server(s)         :
            {
                String t = String::clone(&s.severity);
                String::append(&mut t, ": ");
                String::append(&mut t, String::as_view(&s.message));
                String::append(&mut t, " (");
                String::append(&mut t, String::as_view(&s.code));
                String::append(&mut t, ")");
                t
            },
            ProtocolViolation : String::from_str("the server's message is malformed"),
            ResultTooLarge    : String::from_str("the result is larger than the limit"),
            Disconnected      : String::from_str("the connection is closed"),
        }
    }

    // ---- the connection URL ----

    // Whether the connection is encrypted: not at all, or by TLS with the
    // server's certificate verified against the roots given (the system's
    // by default). An unverified TLS connection is not offered.
    export enum PgSslMode
    {
        Disable,
        VerifyFull,
    }

    // A parsed `postgres://user[:password]@host[:port]/database[?sslmode=MODE]`.
    export struct PgUrl
    {
        export String user;
        export Option<String> password;
        export String host;
        export u16 port;
        export String database;
        export PgSslMode sslmode;
    }

    // Percent-decoded, as UTF-8.
    fn decoded(StringView v) : Result<String, PgError>
    {
        Vec<u8> bytes = match (percent_decode(v))
        {
            Some(b) : b,
            None    : return Err(BadConnectionUrl),
        };
        match (String::from_utf8(bytes))
        {
            Ok(s)  : Ok(s),
            Err(_) : Err(BadConnectionUrl),
        }
    }

    // `Err(BadConnectionUrl)` for a scheme other than postgres or postgresql,
    // no user, no host, a port that is not a number, an sslmode other than
    // disable or verify-full, any other query key, or a bad percent-encoding.
    // The port defaults to 5432, the database to the user, sslmode to
    // verify-full.
    export fn PgUrl::parse(StringView text) : Result<PgUrl, PgError>
    {
        usize sep = match (StringView::find(text, "://"))
        {
            Some(i) : i,
            None    : return Err(BadConnectionUrl),
        };
        StringView scheme = StringView::sub(text, 0, sep);
        if (!StringView::eq(scheme, "postgres") && !StringView::eq(scheme, "postgresql"))
        {
            return Err(BadConnectionUrl);
        }
        StringView rest = StringView::sub(text, sep + 3, StringView::len(text));
        usize n = StringView::len(rest);
        slice<u8, shared> b = StringView::as_bytes(rest);
        usize auth_end = 0;
        while (auth_end < n && b[auth_end] != b'/' && b[auth_end] != b'?')
        {
            auth_end += 1;
        }
        // The user information ends at the last '@' of the authority.
        usize at_sign = auth_end;
        for (usize i = 0; i < auth_end; i += 1)
        {
            if (b[i] == b'@')
            {
                at_sign = i;
            }
        }
        if (at_sign == auth_end || at_sign == 0)
        {
            return Err(BadConnectionUrl);
        }
        StringView userinfo = StringView::sub(rest, 0, at_sign);
        String user = String::new();
        Option<String> password = None;
        match (StringView::find(userinfo, ":"))
        {
            Some(colon) :
            {
                overwrite(&mut user, decoded(StringView::sub(userinfo, 0, colon))?);
                overwrite(&mut password, Some(decoded(StringView::sub(userinfo, colon + 1, StringView::len(userinfo)))?));
            },
            None        : overwrite(&mut user, decoded(userinfo)?),
        }
        if (String::len(&user) == 0)
        {
            return Err(BadConnectionUrl);
        }
        StringView hostport = StringView::sub(rest, at_sign + 1, auth_end);
        usize hp = StringView::len(hostport);
        slice<u8, shared> hb = StringView::as_bytes(hostport);
        if (hp == 0)
        {
            return Err(BadConnectionUrl);
        }
        String host = String::new();
        usize host_end = 0;
        if (hb[0] == b'[')
        {
            usize close = match (StringView::find(hostport, "]"))
            {
                Some(i) : i,
                None    : return Err(BadConnectionUrl),
            };
            String::append(&mut host, StringView::sub(hostport, 1, close));
            host_end = close + 1;
        }
        else
        {
            while (host_end < hp && hb[host_end] != b':')
            {
                host_end += 1;
            }
            String::append(&mut host, StringView::sub(hostport, 0, host_end));
        }
        if (String::len(&host) == 0)
        {
            return Err(BadConnectionUrl);
        }
        u16 port = 5432;
        if (host_end < hp)
        {
            if (hb[host_end] != b':')
            {
                return Err(BadConnectionUrl);
            }
            port = match (StringView::parse<u16>(StringView::sub(hostport, host_end + 1, hp)))
            {
                Ok(p)  : p,
                Err(_) : return Err(BadConnectionUrl),
            };
        }
        // The database: the path without its '/', the user when none.
        usize path_end = auth_end;
        while (path_end < n && b[path_end] != b'?')
        {
            path_end += 1;
        }
        String database = String::new();
        if (path_end > auth_end + 1)
        {
            overwrite(&mut database, decoded(StringView::sub(rest, auth_end + 1, path_end))?);
        }
        if (String::len(&database) == 0)
        {
            overwrite(&mut database, String::clone(&user));
        }
        PgSslMode sslmode = VerifyFull;
        if (path_end < n)
        {
            StringView query = StringView::sub(rest, path_end + 1, n);
            if (StringView::len(query) > 0)
            {
                Vec<StringView> pairs = StringView::split(query, "&");
                foreach (pair in &pairs)
                {
                    if (StringView::starts_with(*pair, "sslmode="))
                    {
                        StringView mode = StringView::sub(*pair, 8, StringView::len(*pair));
                        if (StringView::eq(mode, "disable"))
                        {
                            sslmode = Disable;
                        }
                        else if (StringView::eq(mode, "verify-full"))
                        {
                            sslmode = VerifyFull;
                        }
                        else
                        {
                            return Err(BadConnectionUrl);
                        }
                    }
                    else
                    {
                        return Err(BadConnectionUrl);
                    }
                }
            }
        }
        Ok(PgUrl { .user = user, .password = password, .host = host, .port = port, .database = database, .sslmode = sslmode })
    }

    // ---- values, columns, rows, results ----

    // A statement parameter. Each is sent in the text form the server reads
    // for it: an integer and a float as their decimal text (a float that is
    // not finite as Infinity, -Infinity or NaN), a bool as t or f, bytes as
    // bytea's hex form. The server infers each parameter's type from the
    // statement.
    export enum PgValue
    {
        Null,
        Text(String),
        Int(i64),
        Float(f64),
        Bool(bool),
        Bytes(Vec<u8>),
    }

    fn float_text(f64 x) : String
    {
        if (x != x)
        {
            return String::from_str("NaN");
        }
        if (x > 1.7976931348623157e308)
        {
            return String::from_str("Infinity");
        }
        if (x < -1.7976931348623157e308)
        {
            return String::from_str("-Infinity");
        }
        String t = String::new();
        String::append(&mut t, x);
        t
    }

    // The text the value is sent as; None for Null.
    export fn PgValue::text(ref<PgValue, shared> value) : Option<String>
    {
        match (value)
        {
            Null     : None,
            Text(s)  : Some(String::clone(s)),
            Int(i)   :
            {
                String t = String::new();
                String::append(&mut t, *i);
                Some(t)
            },
            Float(f) : Some(float_text(*f)),
            Bool(b)  : Some(String::from_str(if (*b) { "t" } else { "f" })),
            Bytes(v) :
            {
                String t = String::from_str("\\x");
                String::append(&mut t, String::as_view(&to_hex(&v[0..$])));
                Some(t)
            },
        }
    }

    // One column of a result: its name and the OID of its type (23 for
    // int4, 25 for text, 16 for bool, ...).
    export struct PgColumn
    {
        export String name;
        export u32 type_oid;
    }

    // One row: a cell per column, NULL as None, every other value as the
    // server's text form.
    export struct PgRow
    {
        export Vec<Option<String>> cells;
    }

    // The cell at `i`: None for NULL or past the last column.
    export fn PgRow::get(ref<PgRow, shared> r, usize i) : Option<StringView>
    {
        if (i >= Vec::len(&r.cells))
        {
            return None;
        }
        match (&r.cells[i])
        {
            Some(s) : Some(String::as_view(s)),
            None    : None,
        }
    }

    // D-0181: a cell read as a value: `None` for NULL, for a column past the
    // last, and for text that is not of the form asked for (an integer, a
    // float as the server writes one, `t`/`f`, `bytea`'s hex form); the
    // text itself is still there through `get`.
    export fn PgRow::get_i64(ref<PgRow, shared> r, usize i) : Option<i64>
    {
        match (PgRow::get(r, i))
        {
            Some(t) : match (StringView::parse<i64>(t))
            {
                Ok(v)  : Some(v),
                Err(_) : None,
            },
            None : None,
        }
    }

    export fn PgRow::get_f64(ref<PgRow, shared> r, usize i) : Option<f64>
    {
        match (PgRow::get(r, i))
        {
            Some(t) :
            {
                if (StringView::eq(t, "NaN"))
                {
                    return Some(0.0 / 0.0);
                }
                if (StringView::eq(t, "Infinity"))
                {
                    return Some(1.0 / 0.0);
                }
                if (StringView::eq(t, "-Infinity"))
                {
                    return Some(-1.0 / 0.0);
                }
                match (StringView::parse<f64>(t))
                {
                    Ok(v)  : Some(v),
                    Err(_) : None,
                }
            },
            None    : None,
        }
    }

    export fn PgRow::get_bool(ref<PgRow, shared> r, usize i) : Option<bool>
    {
        match (PgRow::get(r, i))
        {
            Some(t) :
            {
                if (StringView::eq(t, "t"))
                {
                    return Some(true);
                }
                if (StringView::eq(t, "f"))
                {
                    return Some(false);
                }
                None
            },
            None    : None,
        }
    }

    export fn PgRow::get_bytes(ref<PgRow, shared> r, usize i) : Option<Vec<u8>>
    {
        match (PgRow::get(r, i))
        {
            Some(t) : match (StringView::strip_prefix(t, "\\x"))
            {
                // `\x` alone is the empty bytea, which from_hex refuses.
                Some(hex) : if (StringView::len(hex) == 0)
                {
                    Some(Vec::new())
                }
                else
                {
                    match (from_hex(hex))
                    {
                        Ok(v)  : Some(v),
                        Err(_) : None,
                    }
                },
                None : None,
            },
            None : None,
        }
    }

    // What a statement gave: the columns and rows (none for a statement
    // without a result set), the rows affected as the command tag counts
    // them (0 when it carries no count), and the tag itself ("SELECT 3",
    // "INSERT 0 1", "CREATE TABLE", ...).
    export struct PgResult
    {
        export Vec<PgColumn> columns;
        export Vec<PgRow> rows;
        export u64 affected;
        export String tag;
    }

    // The index of the column named `name` (exactly, as the server names it).
    export fn PgResult::column(ref<PgResult, shared> res, str name) : Option<usize>
    {
        for (usize i = 0; i < Vec::len(&res.columns); i += 1)
        {
            if (String::eq_str(&res.columns[i].name, name))
            {
                return Some(i);
            }
        }
        None
    }

    // The count a command tag ends with, 0 when it ends with none.
    fn tag_count(StringView tag) : u64
    {
        Vec<StringView> words = StringView::split(tag, " ");
        if (Vec::len(&words) < 2)
        {
            return 0;
        }
        match (StringView::parse<u64>(words[Vec::len(&words) - 1]))
        {
            Ok(n)  : n,
            Err(_) : 0,
        }
    }

    // ---- SCRAM-SHA-256 (RFC 5802, RFC 7677), the client side ----

    // The salted password: PBKDF2 over HMAC-SHA-256, 32 bytes. The password
    // is used as given (no SASLprep).
    export fn scram_salted_password(StringView password, slice<u8, shared> salt, usize iterations) : Vec<u8>
    {
        pbkdf2(Sha2_256, StringView::as_bytes(password), salt, iterations, 32)
    }

    // One SCRAM-SHA-256 exchange, without channel binding: `start` with the
    // user (its "," and "=" escaped; PostgreSQL ignores it and is sent "")
    // and the client's nonce, `client_first` to send, `respond` to the
    // server's first message gives the client's final message, `verify`
    // checks the server's.
    // Exported so the exchange can be checked against RFC 7677's vectors and
    // so a program can speak it to another server.
    export struct Scram
    {
        String client_nonce;
        String client_first_bare;
        Vec<u8> server_signature;
    }

    export fn Scram::start(StringView user, StringView client_nonce) : Scram
    {
        String bare = String::from_str("n=");
        foreach (c in StringView::as_bytes(user))
        {
            if (*c == b',')
            {
                String::append(&mut bare, "=2C");
            }
            else if (*c == b'=')
            {
                String::append(&mut bare, "=3D");
            }
            else
            {
                String::push_ascii(&mut bare, *c);
            }
        }
        String::append(&mut bare, ",r=");
        String::append(&mut bare, client_nonce);
        Scram { .client_nonce = String::from_view(client_nonce), .client_first_bare = bare, .server_signature = Vec::new() }
    }

    // `n,,n=<user>,r=<nonce>`: no channel binding.
    export fn Scram::client_first(ref<Scram, shared> s) : String
    {
        String m = String::from_str("n,,");
        String::append(&mut m, String::as_view(&s.client_first_bare));
        m
    }

    fn attribute(slice<StringView, shared> parts, str name) : Option<StringView>
    {
        foreach (p in parts)
        {
            if (StringView::starts_with(*p, StringView::of(name)))
            {
                return Some(StringView::sub(*p, 2, StringView::len(*p)));
            }
        }
        None
    }

    // The client's final message for the server's first (`r=…,s=…,i=…`):
    // the proof of `password`. `AuthFailed` when the server's nonce does not
    // begin with the client's, `ProtocolViolation` when the message is malformed or
    // asks for an extension.
    export fn Scram::respond(
        ref<Scram, exclusive> s,
        StringView server_first,
        StringView password,
    ) : Result<String, PgError>
    {
        Vec<StringView> parts = StringView::split(server_first, ",");
        if (Option::is_some(&attribute(&parts[0..$], "m=")))
        {
            return Err(ProtocolViolation);
        }
        StringView nonce = Option::ok_or(attribute(&parts[0..$], "r="), ProtocolViolation)?;
        StringView salt_text = Option::ok_or(attribute(&parts[0..$], "s="), ProtocolViolation)?;
        StringView iter_text = Option::ok_or(attribute(&parts[0..$], "i="), ProtocolViolation)?;
        if (!StringView::starts_with(
            nonce,
            String::as_view(&s.client_nonce),
        ) || StringView::len(nonce) <= String::len(&s.client_nonce))
        {
            return Err(AuthFailed);
        }
        Vec<u8> salt = match (base64_decode(salt_text))
        {
            Ok(v)  : v,
            Err(_) : return Err(ProtocolViolation),
        };
        usize iterations = match (StringView::parse<usize>(iter_text))
        {
            Ok(n)  : n,
            Err(_) : return Err(ProtocolViolation),
        };
        if (iterations == 0)
        {
            return Err(ProtocolViolation);
        }
        Vec<u8> salted = scram_salted_password(password, &salt[0..$], iterations);
        array<u8, 32> client_key = hmac_sha256(&salted[0..$], &b"Client Key"[0..$]);
        array<u8, 32> stored_key = sha256(&client_key[0..$]);
        String without_proof = String::from_str("c=biws,r=");
        String::append(&mut without_proof, nonce);
        String auth = String::clone(&s.client_first_bare);
        String::append(&mut auth, ",");
        String::append(&mut auth, server_first);
        String::append(&mut auth, ",");
        String::append(&mut auth, String::as_view(&without_proof));
        array<u8, 32> client_signature = hmac_sha256(&stored_key[0..$], StringView::as_bytes(String::as_view(&auth)));
        Vec<u8> proof = Vec::new();
        for (usize i = 0; i < 32; i += 1)
        {
            Vec::push(&mut proof, client_key[i] ^ client_signature[i]);
        }
        array<u8, 32> server_key = hmac_sha256(&salted[0..$], &b"Server Key"[0..$]);
        array<u8, 32> server_signature = hmac_sha256(&server_key[0..$], StringView::as_bytes(String::as_view(&auth)));
        overwrite(&mut s.server_signature, Vec::from_slice(&server_signature[0..$]));
        String::append(&mut without_proof, ",p=");
        String::append(&mut without_proof, String::as_view(&base64_encode(&proof[0..$])));
        Ok(without_proof)
    }

    // Whether the server's final message (`v=…`) carries the signature the
    // exchange expects; false for anything else (an `e=` error among them).
    export fn Scram::verify(ref<Scram, shared> s, StringView server_final) : bool
    {
        if (!StringView::starts_with(server_final, "v="))
        {
            return false;
        }
        match (base64_decode(StringView::sub(server_final, 2, StringView::len(server_final))))
        {
            Ok(v)  : digest_eq(&v[0..$], &s.server_signature[0..$]),
            Err(_) : false,
        }
    }

    // ---- messages over a plain or a TLS connection ----

    enum PgConn
    {
        Plain(TcpStream),
        Secure(TlsStream),
    }

    fn pg_read(ref<PgConn, exclusive> c, ref<Vec<u8>, exclusive> buf, usize max) : Result<usize, PgError>
    {
        match (c)
        {
            Plain(s) : match (TcpStream::read(s, buf, max))
            {
                Ok(n)  : Ok(n),
                Err(e) : Err(NetFailure(e)),
            },
            Secure(s) : match (TlsStream::read(s, buf, max))
            {
                Ok(n)  : Ok(n),
                Err(e) : Err(TlsFailure(e)),
            },
        }
    }

    fn pg_write(ref<PgConn, exclusive> c, slice<u8, shared> data) : Result<void, PgError>
    {
        match (c)
        {
            Plain(s) : match (TcpStream::write(s, data))
            {
                Ok(_)  : Ok(()),
                Err(e) : Err(NetFailure(e)),
            },
            Secure(s) : match (TlsStream::write(s, data))
            {
                Ok(_)  : Ok(()),
                Err(e) : Err(TlsFailure(e)),
            },
        }
    }

    fn pg_close(ref<PgConn, exclusive> c)
    {
        match (c)
        {
            Plain(s)  : { _ = TcpStream::shutdown_write(s); },
            Secure(s) : { _ = TlsStream::close(s); },
        }
    }

    // Bytes read ahead of what the decoder has consumed.
    struct PgReader
    {
        PgConn conn;
        Vec<u8> buf;
        usize at;
    }

    fn pg_fill(ref<PgReader, exclusive> r) : Result<bool, PgError>
    {
        if (r.at > 0 && r.at == Vec::len(&r.buf))
        {
            Vec::clear(&mut r.buf);
            r.at = 0;
        }
        usize n = pg_read(&mut r.conn, &mut r.buf, 16384)?;
        Ok(n > 0)
    }

    fn pg_available(ref<PgReader, shared> r) : usize
    {
        Vec::len(&r.buf) - r.at
    }

    // Exactly `n` bytes appended to `out`; Disconnected when the stream ends first.
    fn pg_read_n(ref<PgReader, exclusive> r, ref<Vec<u8>, exclusive> out, usize n) : Result<void, PgError>
    {
        usize need = n;
        while (need > 0)
        {
            if (pg_available(r) == 0)
            {
                if (!pg_fill(r)?)
                {
                    return Err(Disconnected);
                }
            }
            usize take = min(need, pg_available(r));
            Vec::extend_from(out, &r.buf[r.at..r.at + take]);
            r.at += take;
            need -= take;
        }
        Ok(())
    }

    // The most one message from the server may be.
    const usize MAX_MESSAGE = 1073741824;

    // A message from the server: its type byte and its body (the length
    // word dropped).
    struct Message
    {
        u8 kind;
        Vec<u8> body;
    }

    fn read_message(ref<PgReader, exclusive> r) : Result<Message, PgError>
    {
        // Most messages are already whole in the buffer: taken from it at once.
        usize avail = pg_available(r);
        if (avail >= 5)
        {
            u8 kind = r.buf[r.at];
            u32 len = read_be<u32>(&r.buf[0..$], r.at + 1);
            if (len < 4 || widen<usize>(len) - 4 > MAX_MESSAGE)
            {
                return Err(ProtocolViolation);
            }
            usize n = widen<usize>(len) - 4;
            if (avail - 5 >= n)
            {
                Vec<u8> body = Vec::from_slice(&r.buf[r.at + 5..r.at + 5 + n]);
                r.at += 5 + n;
                return Ok(Message { .kind = kind, .body = body });
            }
        }
        Vec<u8> head = Vec::new();
        pg_read_n(r, &mut head, 5)?;
        u8 kind = head[0];
        u32 len = read_be<u32>(&head[0..$], 1);
        if (len < 4 || widen<usize>(len) - 4 > MAX_MESSAGE)
        {
            return Err(ProtocolViolation);
        }
        Vec<u8> body = Vec::new();
        pg_read_n(r, &mut body, widen<usize>(len) - 4)?;
        Ok(Message { .kind = kind, .body = body })
    }

    // A message to the server: its type byte, its length (counting itself),
    // its body.
    fn frame(u8 kind, slice<u8, shared> body) : Vec<u8>
    {
        Vec<u8> out = Vec::new();
        Vec::push(&mut out, kind);
        Vec::push_be(&mut out, narrow<u32>(slice_len(body) + 4));
        Vec::extend_from(&mut out, body);
        out
    }

    fn push_cstr(ref<Vec<u8>, exclusive> out, StringView s)
    {
        Vec::extend_from(out, StringView::as_bytes(s));
        Vec::push(out, 0);
    }

    // The NUL-terminated string at `*at`, which moves past it; ProtocolViolation
    // without a NUL or when it is not UTF-8.
    fn cstr_at(slice<u8, shared> b, ref<usize, exclusive> at) : Result<String, PgError>
    {
        usize end = *at;
        while (end < slice_len(b) && b[end] != 0)
        {
            end += 1;
        }
        if (end == slice_len(b))
        {
            return Err(ProtocolViolation);
        }
        Vec<u8> bytes = Vec::from_slice(&b[*at..end]);
        *at = end + 1;
        match (String::from_utf8(bytes))
        {
            Ok(s)  : Ok(s),
            Err(_) : Err(ProtocolViolation),
        }
    }

    // An ErrorResponse or NoticeResponse body: fields of a type byte and a
    // string, to a NUL.
    fn decode_server_error(slice<u8, shared> b) : Result<PgServerError, PgError>
    {
        PgServerError e = PgServerError { .severity = String::new(), .code = String::new(), .message = String::new(), .detail = None, .hint = None, .position = None };
        usize at = 0;
        while (at < slice_len(b) && b[at] != 0)
        {
            u8 field = b[at];
            at += 1;
            String t = cstr_at(b, &mut at)?;
            if (field == b'S' || field == b'V')
            {
                overwrite(&mut e.severity, t);
            }
            else if (field == b'C')
            {
                overwrite(&mut e.code, t);
            }
            else if (field == b'M')
            {
                overwrite(&mut e.message, t);
            }
            else if (field == b'D')
            {
                overwrite(&mut e.detail, Some(t));
            }
            else if (field == b'H')
            {
                overwrite(&mut e.hint, Some(t));
            }
            else if (field == b'P')
            {
                match (String::parse<u32>(&t))
                {
                    Ok(p)  : overwrite(&mut e.position, Some(p)),
                    Err(_) : {},
                }
            }
        }
        Ok(e)
    }

    // A RowDescription body.
    fn decode_columns(slice<u8, shared> b) : Result<Vec<PgColumn>, PgError>
    {
        if (slice_len(b) < 2)
        {
            return Err(ProtocolViolation);
        }
        usize n = widen<usize>(read_be<u16>(b, 0));
        usize at = 2;
        Vec<PgColumn> columns = Vec::new();
        for (usize i = 0; i < n; i += 1)
        {
            String name = cstr_at(b, &mut at)?;
            if (at + 18 > slice_len(b))
            {
                return Err(ProtocolViolation);
            }
            u32 type_oid = read_be<u32>(b, at + 6);
            at += 18;
            Vec::push(&mut columns, PgColumn { .name = name, .type_oid = type_oid });
        }
        Ok(columns)
    }

    // A DataRow body.
    fn decode_row(slice<u8, shared> b) : Result<PgRow, PgError>
    {
        if (slice_len(b) < 2)
        {
            return Err(ProtocolViolation);
        }
        usize n = widen<usize>(read_be<u16>(b, 0));
        usize at = 2;
        Vec<Option<String>> cells = Vec::new();
        for (usize i = 0; i < n; i += 1)
        {
            if (at + 4 > slice_len(b))
            {
                return Err(ProtocolViolation);
            }
            i32 len = read_be<i32>(b, at);
            at += 4;
            if (len < 0)
            {
                Vec::push(&mut cells, None);
                continue;
            }
            usize size = narrow<usize>(len);
            if (at + size > slice_len(b))
            {
                return Err(ProtocolViolation);
            }
            Vec<u8> bytes = Vec::from_slice(&b[at..at + size]);
            at += size;
            match (String::from_utf8(bytes))
            {
                Ok(s)  : Vec::push(&mut cells, Some(s)),
                Err(_) : return Err(ProtocolViolation),
            }
        }
        Ok(PgRow { .cells = cells })
    }

    // ---- the connection ----

    struct PgParameter
    {
        String name;
        String value;
    }

    // An open connection to a server. `notices` holds the NoticeResponse
    // messages received so far, for the program to read and clear;
    // `max_result` is the most a statement's rows may take (16 MiB).
    // Dropping a connection without `close` just closes the socket.
    export resource struct PgConnection
    {
        PgReader reader;
        bool secure;
        bool closed;
        Vec<PgParameter> parameters;
        u32 backend_pid;
        u32 backend_key;
        export Vec<PgServerError> notices;
        export usize max_result;
    }

    // TCP to `host`'s addresses in turn, each within `timeout_ms`, which then
    // bounds every read and write.
    fn open_tcp(StringView host, u16 port, u64 timeout_ms) : Result<TcpStream, PgError>
    {
        Vec<SocketAddr> addrs = match (IpAddr::parse(host))
        {
            Ok(ip) :
            {
                Vec<SocketAddr> one = Vec::new();
                Vec::push(&mut one, SocketAddr::new(ip, port));
                one
            },
            Err(_) : match (resolve(host, port))
            {
                Ok(a)  : a,
                Err(e) : return Err(NetFailure(e)),
            },
        };
        if (Vec::len(&addrs) == 0)
        {
            return Err(NetFailure(UnknownHost));
        }
        NetError last = Failed;
        for (usize i = 0; i < Vec::len(&addrs); i += 1)
        {
            TcpStream tcp = match (TcpStream::connect_timeout(addrs[i], timeout_ms))
            {
                Ok(t)  : t,
                Err(e) :
                {
                    last = e;
                    continue;
                },
            };
            TcpStream::set_read_timeout_ms(&mut tcp, timeout_ms);
            TcpStream::set_write_timeout_ms(&mut tcp, timeout_ms);
            TcpStream::set_nodelay(&mut tcp, true);
            return Ok(tcp);
        }
        Err(NetFailure(last))
    }

    // Connects by `url` with a 30 s timeout; for verify-full, the system
    // roots (NoTrustedRoots when they cannot be loaded).
    export fn PgConnection::connect(StringView url) : Result<PgConnection, PgError>
    {
        PgUrl u = PgUrl::parse(url)?;
        TrustStore roots = match (u.sslmode)
        {
            Disable : TrustStore::new(),
            VerifyFull : match (TrustStore::system())
            {
                Ok(t)  : t,
                Err(_) : return Err(NoTrustedRoots),
            },
        };
        connect_url(&u, &roots, 30000)
    }

    // Connects by `url`, verifying the server's certificate against `roots`
    // (unused for sslmode=disable), with `timeout_ms` for connecting and for
    // each read and write.
    export fn PgConnection::connect_with(
        StringView url,
        ref<TrustStore, shared> roots,
        u64 timeout_ms,
    ) : Result<PgConnection, PgError>
    {
        PgUrl u = PgUrl::parse(url)?;
        connect_url(&u, roots, timeout_ms)
    }

    fn connect_url(ref<PgUrl, shared> u, ref<TrustStore, shared> roots, u64 timeout_ms) : Result<PgConnection, PgError>
    {
        TcpStream tcp = open_tcp(String::as_view(&u.host), u.port, timeout_ms)?;
        bool secure = false;
        PgConn conn = match (u.sslmode)
        {
            Disable    : Plain(tcp),
            VerifyFull :
            {
                // SSLRequest: the server answers S (go ahead) or N (no TLS).
                Vec<u8> req = Vec::new();
                Vec::push_be<u32>(&mut req, 8);
                Vec::push_be<u32>(&mut req, 80877103);
                match (TcpStream::write(&mut tcp, &req[0..$]))
                {
                    Ok(_)  : {},
                    Err(e) : return Err(NetFailure(e)),
                }
                Vec<u8> answer = Vec::new();
                match (TcpStream::read_exact(&mut tcp, &mut answer, 1))
                {
                    Ok(_)  : {},
                    Err(e) : return Err(NetFailure(e)),
                }
                if (answer[0] == b'N')
                {
                    return Err(TlsRefused);
                }
                if (answer[0] != b'S')
                {
                    return Err(ProtocolViolation);
                }
                secure = true;
                match (TlsStream::client(tcp, String::as_view(&u.host), roots))
                {
                    Ok(s)  : Secure(s),
                    Err(e) : return Err(TlsFailure(e)),
                }
            },
        };
        PgConnection c = PgConnection { .reader = PgReader { .conn = conn, .buf = Vec::new(), .at = 0 }, .secure = secure, .closed = false, .parameters = Vec::new(), .backend_pid = 0, .backend_key = 0, .notices = Vec::new(), .max_result = 16777216 };
        // StartupMessage: protocol 3.0, the user, the database, UTF-8.
        Vec<u8> body = Vec::new();
        Vec::push_be<u32>(&mut body, 196608);
        push_cstr(&mut body, "user");
        push_cstr(&mut body, String::as_view(&u.user));
        push_cstr(&mut body, "database");
        push_cstr(&mut body, String::as_view(&u.database));
        push_cstr(&mut body, "client_encoding");
        push_cstr(&mut body, "UTF8");
        Vec::push(&mut body, 0);
        Vec<u8> startup = Vec::new();
        Vec::push_be(&mut startup, narrow<u32>(Vec::len(&body) + 4));
        Vec::extend_from(&mut startup, &body[0..$]);
        match (pg_write(&mut c.reader.conn, &startup[0..$]))
        {
            Ok(_)  : {},
            Err(e) : return Err(e),
        }
        match (authenticate(&mut c, u))
        {
            Ok(_)  : Ok(c),
            Err(e) :
            {
                pg_close(&mut c.reader.conn);
                Err(e)
            },
        }
    }

    // The authentication exchange and the rest of the startup, to the first
    // ReadyForQuery.
    fn authenticate(ref<PgConnection, exclusive> c, ref<PgUrl, shared> u) : Result<void, PgError>
    {
        Option<Scram> scram = None;
        String password = match (&u.password)
        {
            Some(p) : String::clone(p),
            None    : String::new(),
        };
        while (true)
        {
            Message m = read_message(&mut c.reader)?;
            slice<u8, shared> b = &m.body[0..$];
            if (m.kind == b'R')
            {
                if (slice_len(b) < 4)
                {
                    return Err(ProtocolViolation);
                }
                u32 code = read_be<u32>(b, 0);
                if (code == 0)
                {
                    continue;
                }
                else if (code == 3)
                {
                    if (!c.secure)
                    {
                        return Err(AuthRefused);
                    }
                    Vec<u8> pw = Vec::new();
                    push_cstr(&mut pw, String::as_view(&password));
                    pg_write(&mut c.reader.conn, &frame(b'p', &pw[0..$])[0..$])?;
                }
                else if (code == 10)
                {
                    bool offered = false;
                    usize at = 4;
                    while (at < slice_len(b) && b[at] != 0)
                    {
                        String mech = cstr_at(b, &mut at)?;
                        if (String::eq_str(&mech, "SCRAM-SHA-256"))
                        {
                            offered = true;
                        }
                    }
                    if (!offered)
                    {
                        return Err(UnsupportedAuth);
                    }
                    array<u8, 18> raw = [0; 18];
                    os_random_bytes(&mut raw[0..$]);
                    String nonce = base64_encode(&raw[0..$]);
                    Scram s = Scram::start("", String::as_view(&nonce));
                    String first = Scram::client_first(&s);
                    Vec<u8> body = Vec::new();
                    push_cstr(&mut body, "SCRAM-SHA-256");
                    Vec::push_be(&mut body, narrow<u32>(String::len(&first)));
                    Vec::extend_from(&mut body, StringView::as_bytes(String::as_view(&first)));
                    pg_write(&mut c.reader.conn, &frame(b'p', &body[0..$])[0..$])?;
                    overwrite(&mut scram, Some(s));
                }
                else if (code == 11)
                {
                    String server_first = match (String::from_utf8(Vec::from_slice(&b[4..$])))
                    {
                        Ok(s)  : s,
                        Err(_) : return Err(ProtocolViolation),
                    };
                    String client_final = match (&mut scram)
                    {
                        Some(s) : Scram::respond(s, String::as_view(&server_first), String::as_view(&password))?,
                        None    : return Err(ProtocolViolation),
                    };
                    pg_write(&mut c.reader.conn, &frame(b'p', StringView::as_bytes(String::as_view(&client_final)))[0..$])?;
                }
                else if (code == 12)
                {
                    String server_final = match (String::from_utf8(Vec::from_slice(&b[4..$])))
                    {
                        Ok(s)  : s,
                        Err(_) : return Err(ProtocolViolation),
                    };
                    bool ok = match (&scram)
                    {
                        Some(s) : Scram::verify(s, String::as_view(&server_final)),
                        None    : false,
                    };
                    if (!ok)
                    {
                        return Err(AuthFailed);
                    }
                }
                else
                {
                    return Err(UnsupportedAuth);
                }
            }
            else if (m.kind == b'S')
            {
                usize at = 0;
                String name = cstr_at(b, &mut at)?;
                String value = cstr_at(b, &mut at)?;
                Vec::push(&mut c.parameters, PgParameter { .name = name, .value = value });
            }
            else if (m.kind == b'K')
            {
                if (slice_len(b) < 8)
                {
                    return Err(ProtocolViolation);
                }
                c.backend_pid = read_be<u32>(b, 0);
                c.backend_key = read_be<u32>(b, 4);
            }
            else if (m.kind == b'Z')
            {
                return Ok(());
            }
            else if (m.kind == b'E')
            {
                PgServerError e = decode_server_error(b)?;
                return Err(if (String::starts_with(&e.code, "28")) { AuthFailed } else { Server(e) });
            }
            else if (m.kind == b'N')
            {
                Vec::push(&mut c.notices, decode_server_error(b)?);
            }
            else
            {
                return Err(ProtocolViolation);
            }
        }
    }

    // The value the server reported for a run-time parameter at startup
    // ("server_version", "server_encoding", "TimeZone", ...).
    export fn PgConnection::parameter(ref<PgConnection, shared> c, str name) : Option<StringView>
    {
        foreach (p in &c.parameters)
        {
            if (String::eq_str(&p.name, name))
            {
                return Some(String::as_view(&p.value));
            }
        }
        None
    }

    // Marks the connection unusable and gives the error back.
    fn broken(ref<PgConnection, exclusive> c, PgError e) : PgError
    {
        c.closed = true;
        e
    }

    // Reads the server's answer to a statement or a script, to ReadyForQuery.
    // A server error is kept and given after the server is ready again; rows
    // are kept only when `keep` and while they fit in `max_result`.
    fn collect(ref<PgConnection, exclusive> c, bool keep) : Result<PgResult, PgError>
    {
        PgResult res = PgResult { .columns = Vec::new(), .rows = Vec::new(), .affected = 0, .tag = String::new() };
        Option<PgServerError> failure = None;
        bool too_large = false;
        usize bytes = 0;
        while (true)
        {
            Message m = match (read_message(&mut c.reader))
            {
                Ok(msg) : msg,
                Err(e)  : return Err(broken(c, e)),
            };
            slice<u8, shared> b = &m.body[0..$];
            if (m.kind == b'T')
            {
                match (decode_columns(b))
                {
                    Ok(cols) : overwrite(&mut res.columns, cols),
                    Err(e)   : return Err(broken(c, e)),
                }
            }
            else if (m.kind == b'D')
            {
                if (keep && !too_large)
                {
                    bytes += slice_len(b);
                    if (bytes > c.max_result)
                    {
                        too_large = true;
                        Vec::clear(&mut res.rows);
                    }
                    else
                    {
                        match (decode_row(b))
                        {
                            Ok(row) : Vec::push(&mut res.rows, row),
                            Err(e)  : return Err(broken(c, e)),
                        }
                    }
                }
            }
            else if (m.kind == b'C')
            {
                usize at = 0;
                String tag = match (cstr_at(b, &mut at))
                {
                    Ok(t)  : t,
                    Err(e) : return Err(broken(c, e)),
                };
                res.affected = tag_count(String::as_view(&tag));
                overwrite(&mut res.tag, tag);
            }
            else if (m.kind == b'E')
            {
                PgServerError reported = match (decode_server_error(b))
                {
                    Ok(se) : se,
                    Err(e) : return Err(broken(c, e)),
                };
                if (Option::is_none(&failure))
                {
                    overwrite(&mut failure, Some(reported));
                }
            }
            else if (m.kind == b'N')
            {
                match (decode_server_error(b))
                {
                    Ok(se) : Vec::push(&mut c.notices, se),
                    Err(e) : return Err(broken(c, e)),
                }
            }
            else if (m.kind == b'Z')
            {
                break;
            }
            else if (m.kind == b'1' || m.kind == b'2' || m.kind == b'n' || m.kind == b'I' || m.kind == b's' || m.kind == b'A' || m.kind == b'S')
            {
                // ParseComplete, BindComplete, NoData, EmptyQueryResponse,
                // PortalSuspended, a notification, a parameter changed.
                continue;
            }
            else
            {
                return Err(broken(c, ProtocolViolation));
            }
        }
        if (Some(e) = failure)
        {
            return Err(Server(e));
        }
        if (too_large)
        {
            return Err(ResultTooLarge);
        }
        Ok(res)
    }

    // Runs one statement with `$1`, `$2`, ... bound to `params` (the extended
    // query protocol: Parse, Bind, Describe, Execute, Sync) and gives its
    // result. A server error leaves the connection usable; a network or
    // protocol failure does not, and every later call gives Disconnected.
    export fn PgConnection::query(
        ref<PgConnection, exclusive> c,
        StringView sql,
        slice<PgValue, shared> params,
    ) : Result<PgResult, PgError>
    {
        if (c.closed)
        {
            return Err(Disconnected);
        }
        if (slice_len(params) > 65535)
        {
            return Err(ResultTooLarge);
        }
        Vec<u8> out = Vec::new();
        Vec<u8> parse = Vec::new();
        push_cstr(&mut parse, "");
        push_cstr(&mut parse, sql);
        Vec::push_be<u16>(&mut parse, 0);
        Vec::extend_from(&mut out, &frame(b'P', &parse[0..$])[0..$]);
        Vec<u8> bind = Vec::new();
        push_cstr(&mut bind, "");
        push_cstr(&mut bind, "");
        Vec::push_be<u16>(&mut bind, 0);
        Vec::push_be(&mut bind, narrow<u16>(slice_len(params)));
        foreach (p in params)
        {
            match (PgValue::text(p))
            {
                Some(t) :
                {
                    Vec::push_be(&mut bind, narrow<u32>(String::len(&t)));
                    Vec::extend_from(&mut bind, StringView::as_bytes(String::as_view(&t)));
                },
                None    : Vec::push_be<i32>(&mut bind, -1),
            }
        }
        Vec::push_be<u16>(&mut bind, 0);
        Vec::extend_from(&mut out, &frame(b'B', &bind[0..$])[0..$]);
        Vec<u8> describe = Vec::new();
        Vec::push(&mut describe, b'P');
        Vec::push(&mut describe, 0);
        Vec::extend_from(&mut out, &frame(b'D', &describe[0..$])[0..$]);
        Vec<u8> execute = Vec::new();
        push_cstr(&mut execute, "");
        Vec::push_be<u32>(&mut execute, 0);
        Vec::extend_from(&mut out, &frame(b'E', &execute[0..$])[0..$]);
        Vec<u8> none = Vec::new();
        Vec::extend_from(&mut out, &frame(b'S', &none[0..$])[0..$]);
        match (pg_write(&mut c.reader.conn, &out[0..$]))
        {
            Ok(_)  : {},
            Err(e) : return Err(broken(c, e)),
        }
        collect(c, true)
    }

    // `query` for a statement without rows: the count of rows it affected.
    export fn PgConnection::execute(
        ref<PgConnection, exclusive> c,
        StringView sql,
        slice<PgValue, shared> params,
    ) : Result<u64, PgError>
    {
        PgResult r = PgConnection::query(c, sql, params)?;
        Ok(r.affected)
    }

    // Runs `sql` as a script: statements separated by `;`, without
    // parameters (the simple query protocol), every result discarded, the
    // first server error given. Not for COPY.
    export fn PgConnection::run_script(ref<PgConnection, exclusive> c, StringView sql) : Result<void, PgError>
    {
        if (c.closed)
        {
            return Err(Disconnected);
        }
        Vec<u8> q = Vec::new();
        push_cstr(&mut q, sql);
        match (pg_write(&mut c.reader.conn, &frame(b'Q', &q[0..$])[0..$]))
        {
            Ok(_)  : {},
            Err(e) : return Err(broken(c, e)),
        }
        _ = collect(c, false)?;
        Ok(())
    }

    // Sends Terminate and closes the connection.
    export fn PgConnection::close(ref<PgConnection, exclusive> c)
    {
        if (!c.closed)
        {
            Vec<u8> none = Vec::new();
            _ = pg_write(&mut c.reader.conn, &frame(b'X', &none[0..$])[0..$]);
            pg_close(&mut c.reader.conn);
            c.closed = true;
        }
    }

**Depends on:** rule.stdlib.net, rule.stdlib.tls, rule.stdlib.crypto, rule.stdlib.text, D-0180
**Affects:** the network (rule.stdlib.net)

## 2r. JSON

In `std::encoding::json` (D-0182, D-0197).

### `rule.stdlib.json`
**Status:** ACCEPTED

JSON (RFC 8259) read into a tree of `Json` values and written back,
compact or indented. An object keeps its members in order (duplicate
keys as read, `get` finding the first); numbers are `f64`, a whole one
written without a fraction. Written in CobaltC over `std::text`; the
variant names carry `Json` because `std`'s bare variant names are unique.

    [Json-Parse]   (D-0182)   ⟨Json::parse(t), Σ⟩ → Ok(j) for t one JSON value (RFC 8259) with whitespace (space, tab,
                   LF, CR) around it: null, true, false → JsonNull, JsonBool; a number (-? (0 | [1-9] digit*)
                   (. digit+)? ([eE] [+-]? digit+)?) → JsonNumber(parse<f64> of its text); a string → JsonText with
                   \" \\ \/ \b \f \n \r \t and \uXXXX decoded, a high surrogate followed by \u of a low one joined,
                   any other surrogate U+FFFD; [ … ] → JsonArray; { "key": value, … } → JsonObject in order.
                   Err(Empty) when t has no value; Err(Invalid(i)) at the first byte that does not fit (a raw byte
                   below 0x20 in a string, a trailing comma, text after the value, …; the length when the text stops
                   short); Err(OutOfRange) for a number beyond f64 or an array or object nested deeper than 512
    [Json-Text]    (D-0182)   ⟨Json::text(j), Σ⟩ → j as JSON with no whitespace: null, true, false; a JsonNumber x that is
                   whole and |x| < 2^53 as its integer, another finite x as String::append's text of it, a NaN or
                   infinity as null; a JsonText with ", \ and the bytes below 0x20 escaped (\n \r \t \b \f, else
                   \u00XX) and every other byte as it is; members and elements in order. parse(text(j)) is j
    [Json-Pretty]  (D-0182)   ⟨Json::pretty(j), Σ⟩ → [Json-Text]'s with each element or member on its own line, two spaces
                   per level, ": " after a key, [] and {} for empty containers, no newline at the end
    [Json-Get]     (D-0182)   ⟨Json::get(j, k), Σ⟩ → Some(a reference to the value of the first member named k) when j is
                   an object with one, else None; ⟨Json::at(j, i), Σ⟩ → Some(element i) when j is an array with one;
                   is_null; as_bool, as_f64, as_text, as_array, as_object → Some(the payload) for the kind named, else
                   None; as_i64 → Some(the integer) for a JsonNumber that is whole and within i64
    [Json-Set]     (D-0182)   ⟨Json::set(j, k, v), Σ⟩ → ⟨(), Σ'⟩ with the first member named k of the object j given v
                   (the old value destroyed), or a new member appended; ⟨Json::push(j, v), Σ⟩ with v appended to the
                   array j; on a value of another kind each is ↛ diag.assert-failed
    [Json-Error]   (D-0182)   Json::parse's errors are ParseError's: Empty, Invalid(offset), OutOfRange, as above

The `std` source:

    // `std::encoding::json` (spec/21 §2r, D-0182, D-0197): JSON (RFC 8259)
    // read into a value tree and written back, compact or indented. A `Json`
    // is one of the six kinds; an object keeps its members in order, so a
    // document reads back as it was written. Numbers are `f64` (an integer
    // beyond 2^53 loses precision, as it does in every JSON reader that uses
    // doubles); a `JsonNumber` that is a whole number is written without a
    // fraction. Written in CobaltC over `std::text`.

    // A JSON value. The variant names carry `Json` so that none clashes with
    // another `std` enum's (`PgValue::Null`, `PgValue::Text`).
    export enum Json
    {
        JsonNull,
        JsonBool(bool),
        JsonNumber(f64),
        JsonText(String),
        JsonArray(Vec<Json>),
        JsonObject(Vec<JsonMember>),
    }

    // One member of an object: its key and value.
    export struct JsonMember
    {
        export String key;
        export Json value;
    }

    // ---- reading ----

    // How deep arrays and objects may nest.
    const usize MAX_DEPTH = 512;

    struct JsonReader
    {
        StringView text;
        usize at;
    }

    fn JsonReader::peek(ref<JsonReader, shared> r) : u8
    {
        if (r.at < StringView::len(r.text))
        {
            return r.text.bytes[r.at];
        }
        0
    }

    // Scans over the text's bytes from `at`, each its function's only
    // reference, so the compiler reads the bytes unchecked: the end of the
    // white space, and of the digits.
    fn json_space_end(slice<u8, shared> b, usize at) : usize
    {
        usize i = at;
        while (i < slice_len(b) && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r'))
        {
            i += 1;
        }
        i
    }

    fn json_digits_end(slice<u8, shared> b, usize at) : usize
    {
        usize i = at;
        while (i < slice_len(b) && ascii_is_digit(b[i]))
        {
            i += 1;
        }
        i
    }

    fn JsonReader::skip_space(ref<JsonReader, exclusive> r)
    {
        r.at = json_space_end(r.text.bytes, r.at);
    }

    // Whether the literal `word` is at the reader's position; then it is
    // consumed.
    fn JsonReader::word(ref<JsonReader, exclusive> r, str word) : bool
    {
        usize n = str_len(word);
        if (r.at + n > StringView::len(r.text))
        {
            return false;
        }
        for (usize i = 0; i < n; i += 1)
        {
            if (r.text.bytes[r.at + i] != str_byte(word, i))
            {
                return false;
            }
        }
        r.at += n;
        true
    }

    fn JsonReader::digits(ref<JsonReader, exclusive> r) : usize
    {
        usize start = r.at;
        r.at = json_digits_end(r.text.bytes, r.at);
        r.at - start
    }

    // `-? (0 | [1-9] digit*) (. digit+)? ([eE] [+-]? digit+)?`, read with
    // `parse<f64>`; `OutOfRange` beyond `f64`.
    fn JsonReader::number(ref<JsonReader, exclusive> r) : Result<Json, ParseError>
    {
        usize start = r.at;
        if (JsonReader::peek(r) == b'-')
        {
            r.at += 1;
        }
        if (JsonReader::peek(r) == b'0')
        {
            r.at += 1;
        }
        else if (JsonReader::digits(r) == 0)
        {
            return Err(Invalid(r.at));
        }
        if (JsonReader::peek(r) == b'.')
        {
            r.at += 1;
            if (JsonReader::digits(r) == 0)
            {
                return Err(Invalid(r.at));
            }
        }
        u8 e = JsonReader::peek(r);
        if (e == b'e' || e == b'E')
        {
            r.at += 1;
            u8 sign = JsonReader::peek(r);
            if (sign == b'+' || sign == b'-')
            {
                r.at += 1;
            }
            if (JsonReader::digits(r) == 0)
            {
                return Err(Invalid(r.at));
            }
        }
        match (StringView::parse<f64>(StringView::sub(r.text, start, r.at)))
        {
            Ok(v)  : Ok(JsonNumber(v)),
            Err(_) : Err(OutOfRange),
        }
    }

    // A string, the opening quote already consumed: its escapes decoded, a
    // surrogate pair joined, a lone surrogate U+FFFD.
    fn JsonReader::text(ref<JsonReader, exclusive> r) : Result<String, ParseError>
    {
        JsonStr { bytes, pos, err } = json_string(r.text.bytes, r.at);
        r.at = pos;
        if (err > 0)
        {
            return Err(Invalid(err - 1));
        }
        // Runs of the (UTF-8) text and encoded code points: well-formed.
        match (String::from_utf8(bytes))
        {
            Ok(s)  : Ok(s),
            Err(_) : Err(Invalid(pos)),
        }
    }

    // `JsonReader::text`'s work on the text's bytes alone (its only
    // reference, read unchecked) and a local vector: the string's bytes
    // decoded, the position after the closing quote, and `err`: 0, or one
    // more than the position `Invalid` names.
    struct JsonStr
    {
        Vec<u8> bytes;
        usize pos;
        usize err;
    }

    // Four hexadecimal digits, `q`, read at `at` (a byte past the text's end
    // is 0); a bad one: -1 - its position. Given as a value: a slice handed
    // on would make `json_string`'s reads checked.
    fn json_hex4(array<u8, 4> q, usize at) : i64
    {
        i64 v = 0;
        for (usize i = 0; i < 4; i += 1)
        {
            usize p = at + i;
            u8 c = q[i];
            i64 d = if (c >= b'0' && c <= b'9') { widen<i64>(c - b'0') }
            else if (c >= b'a' && c <= b'f') { widen<i64>(c - b'a') + 10 }
            else if (c >= b'A' && c <= b'F') { widen<i64>(c - b'A') + 10 }
            else { return -1 - narrow<i64>(p); };
            v = v * 16 + d;
        }
        v
    }

    fn json_string(slice<u8, shared> b, usize start) : JsonStr
    {
        Vec<u8> out = Vec::new();
        usize n = slice_len(b);
        usize at = start;
        usize err = 0;
        while (true)
        {
            if (at >= n)
            {
                err = n + 1;
                break;
            }
            u8 c = b[at];
            if (c == b'"')
            {
                at += 1;
                break;
            }
            if (c < 0x20)
            {
                err = at + 1;
                break;
            }
            if (c != b'\\')
            {
                Vec::push(&mut out, c);
                at += 1;
                continue;
            }
            at += 1;
            u8 e = if (at < n) { b[at] } else { 0 };
            at += 1;
            if (e == b'"') { Vec::push(&mut out, b'"'); }
            else if (e == b'\\') { Vec::push(&mut out, b'\\'); }
            else if (e == b'/') { Vec::push(&mut out, b'/'); }
            else if (e == b'b') { Vec::push(&mut out, 8); }
            else if (e == b'f') { Vec::push(&mut out, 12); }
            else if (e == b'n') { Vec::push(&mut out, 10); }
            else if (e == b'r') { Vec::push(&mut out, 13); }
            else if (e == b't') { Vec::push(&mut out, 9); }
            else if (e == b'u')
            {
                array<u8, 4> q = [0; 4];
                for (usize k = 0; k < 4 && at + k < n; k += 1)
                {
                    q[k] = b[at + k];
                }
                i64 h = json_hex4(q, at);
                if (h < 0)
                {
                    err = narrow<usize>(0 - h);
                    break;
                }
                at += 4;
                u32 cp = narrow<u32>(h);
                if (cp >= 0xD800 && cp <= 0xDBFF)
                {
                    // A high surrogate: joined with a `\uDC00`..`\uDFFF` that
                    // follows; alone, U+FFFD.
                    usize save = at;
                    bool joined = false;
                    if (at < n && b[at] == b'\\')
                    {
                        at += 1;
                        if (at < n && b[at] == b'u')
                        {
                            at += 1;
                            array<u8, 4> q2 = [0; 4];
                            for (usize k = 0; k < 4 && at + k < n; k += 1)
                            {
                                q2[k] = b[at + k];
                            }
                            i64 lo = json_hex4(q2, at);
                            if (lo < 0)
                            {
                                err = narrow<usize>(0 - lo);
                                break;
                            }
                            at += 4;
                            u32 low = narrow<u32>(lo);
                            if (low >= 0xDC00 && low <= 0xDFFF)
                            {
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                                joined = true;
                            }
                        }
                    }
                    if (!joined)
                    {
                        at = save;
                        cp = 0xFFFD;
                    }
                }
                // As `String::push_code_point`: a surrogate is U+FFFD.
                u32 u = if (cp >= 0xD800 && cp <= 0xDFFF) { 0xFFFD } else { cp };
                if (u < 0x80)
                {
                    Vec::push(&mut out, narrow<u8>(u));
                }
                else if (u < 0x800)
                {
                    Vec::push(&mut out, 0xC0 | narrow<u8>(u >> 6));
                    Vec::push(&mut out, 0x80 | narrow<u8>(u & 0x3F));
                }
                else if (u < 0x10000)
                {
                    Vec::push(&mut out, 0xE0 | narrow<u8>(u >> 12));
                    Vec::push(&mut out, 0x80 | narrow<u8>((u >> 6) & 0x3F));
                    Vec::push(&mut out, 0x80 | narrow<u8>(u & 0x3F));
                }
                else
                {
                    Vec::push(&mut out, 0xF0 | narrow<u8>(u >> 18));
                    Vec::push(&mut out, 0x80 | narrow<u8>((u >> 12) & 0x3F));
                    Vec::push(&mut out, 0x80 | narrow<u8>((u >> 6) & 0x3F));
                    Vec::push(&mut out, 0x80 | narrow<u8>(u & 0x3F));
                }
            }
            else
            {
                err = at;
                break;
            }
        }
        JsonStr { .bytes = out, .pos = at, .err = err }
    }

    fn JsonReader::value(ref<JsonReader, exclusive> r, usize depth) : Result<Json, ParseError>
    {
        JsonReader::skip_space(r);
        u8 b = JsonReader::peek(r);
        if (b == b'{')
        {
            if (depth >= MAX_DEPTH)
            {
                return Err(OutOfRange);
            }
            r.at += 1;
            Vec<JsonMember> members = Vec::new();
            JsonReader::skip_space(r);
            if (JsonReader::peek(r) == b'}')
            {
                r.at += 1;
                return Ok(JsonObject(members));
            }
            while (true)
            {
                JsonReader::skip_space(r);
                if (JsonReader::peek(r) != b'"')
                {
                    return Err(Invalid(r.at));
                }
                r.at += 1;
                String key = JsonReader::text(r)?;
                JsonReader::skip_space(r);
                if (JsonReader::peek(r) != b':')
                {
                    return Err(Invalid(r.at));
                }
                r.at += 1;
                Json v = JsonReader::value(r, depth + 1)?;
                Vec::push(&mut members, JsonMember { .key = key, .value = v });
                JsonReader::skip_space(r);
                u8 c = JsonReader::peek(r);
                r.at += 1;
                if (c == b'}')
                {
                    return Ok(JsonObject(members));
                }
                if (c != b',')
                {
                    return Err(Invalid(r.at - 1));
                }
            }
        }
        if (b == b'[')
        {
            if (depth >= MAX_DEPTH)
            {
                return Err(OutOfRange);
            }
            r.at += 1;
            Vec<Json> items = Vec::new();
            JsonReader::skip_space(r);
            if (JsonReader::peek(r) == b']')
            {
                r.at += 1;
                return Ok(JsonArray(items));
            }
            while (true)
            {
                Json v = JsonReader::value(r, depth + 1)?;
                Vec::push(&mut items, v);
                JsonReader::skip_space(r);
                u8 c = JsonReader::peek(r);
                r.at += 1;
                if (c == b']')
                {
                    return Ok(JsonArray(items));
                }
                if (c != b',')
                {
                    return Err(Invalid(r.at - 1));
                }
            }
        }
        if (b == b'"')
        {
            r.at += 1;
            return Ok(JsonText(JsonReader::text(r)?));
        }
        if (b == b'-' || ascii_is_digit(b))
        {
            return JsonReader::number(r);
        }
        if (JsonReader::word(r, "true"))
        {
            return Ok(JsonBool(true));
        }
        if (JsonReader::word(r, "false"))
        {
            return Ok(JsonBool(false));
        }
        if (JsonReader::word(r, "null"))
        {
            return Ok(JsonNull);
        }
        Err(Invalid(r.at))
    }

    // The value `text` holds: one JSON value with whitespace around it.
    // `Empty` for no value at all; `Invalid(i)` names the first byte that
    // does not fit (the length when the text stops short); `OutOfRange` a
    // number beyond `f64` or nesting deeper than 512.
    export fn Json::parse(StringView text) : Result<Json, ParseError>
    {
        JsonReader r = JsonReader { .text = text, .at = 0 };
        JsonReader::skip_space(&mut r);
        if (r.at >= StringView::len(text))
        {
            return Err(Empty);
        }
        Json v = JsonReader::value(&mut r, 0)?;
        JsonReader::skip_space(&mut r);
        if (r.at < StringView::len(text))
        {
            return Err(Invalid(r.at));
        }
        Ok(v)
    }

    // ---- writing ----

    // `s` as a JSON string: quoted, with `"`, `\` and the control characters
    // escaped (`\n`, `\t`, `\r`, `\b`, `\f`, else `\u00XX`); everything else,
    // non-ASCII included, as it is.
    fn json_quote(ref<String, exclusive> out, StringView s)
    {
        Vec<u8> o = replace(&mut out.bytes, Vec::new());
        overwrite(&mut out.bytes, json_quote_bytes(o, s.bytes));
    }

    // `json_quote`'s work: the output vector passed in and given back, the
    // text's bytes the only reference (read unchecked, handed to nothing).
    // Each byte is copied, or escaped; UTF-8 passes through.
    fn json_quote_bytes(Vec<u8> out, slice<u8, shared> s) : Vec<u8>
    {
        str digits = "0123456789abcdef";
        Vec::push(&mut out, b'"');
        for (usize i = 0; i < slice_len(s); i += 1)
        {
            u8 b = s[i];
            if (b >= 0x20 && b != b'"' && b != b'\\')
            {
                Vec::push(&mut out, b);
                continue;
            }
            Vec::push(&mut out, b'\\');
            if (b == b'"') { Vec::push(&mut out, b'"'); }
            else if (b == b'\\') { Vec::push(&mut out, b'\\'); }
            else if (b == 10) { Vec::push(&mut out, b'n'); }
            else if (b == 13) { Vec::push(&mut out, b'r'); }
            else if (b == 9) { Vec::push(&mut out, b't'); }
            else if (b == 8) { Vec::push(&mut out, b'b'); }
            else if (b == 12) { Vec::push(&mut out, b'f'); }
            else
            {
                Vec::push(&mut out, b'u');
                Vec::push(&mut out, b'0');
                Vec::push(&mut out, b'0');
                Vec::push(&mut out, str_byte(digits, widen<usize>(b >> 4)));
                Vec::push(&mut out, str_byte(digits, widen<usize>(b & 15)));
            }
        }
        Vec::push(&mut out, b'"');
        out
    }

    // A number: a whole value within 2^53 without a fraction, else the
    // shortest text that reads back as it; a NaN or an infinity, which JSON
    // cannot write, as `null`.
    fn json_number(ref<String, exclusive> out, f64 x)
    {
        if (!is_finite(x))
        {
            String::append(out, "null");
            return;
        }
        if (x == trunc(x) && x < 9007199254740992.0 && x > -9007199254740992.0)
        {
            String::append(out, to_int<i64>(x));
            return;
        }
        String::append(out, x);
    }

    fn json_indent(ref<String, exclusive> out, usize depth)
    {
        String::push_ascii(out, 10);
        for (usize i = 0; i < depth; i += 1)
        {
            String::append(out, "  ");
        }
    }

    // `j` written onto `out`: compact when `indent` is false, else on
    // several lines with two spaces per level.
    fn json_write(ref<String, exclusive> out, ref<Json, shared> j, bool indent, usize depth)
    {
        match (j)
        {
            JsonNull            : String::append(out, "null"),
            JsonBool(b)         : String::append(out, if (*b) { "true" } else { "false" }),
            JsonNumber(x)       : json_number(out, *x),
            JsonText(s)         : json_quote(out, String::as_view(s)),
            JsonArray(items)    :
            {
                String::push_ascii(out, b'[');
                for (usize i = 0; i < Vec::len(items); i += 1)
                {
                    if (i > 0)
                    {
                        String::push_ascii(out, b',');
                    }
                    if (indent)
                    {
                        json_indent(out, depth + 1);
                    }
                    json_write(out, &items[i], indent, depth + 1);
                }
                if (indent && Vec::len(items) > 0)
                {
                    json_indent(out, depth);
                }
                String::push_ascii(out, b']');
            },
            JsonObject(members) :
            {
                String::push_ascii(out, b'{');
                for (usize i = 0; i < Vec::len(members); i += 1)
                {
                    if (i > 0)
                    {
                        String::push_ascii(out, b',');
                    }
                    if (indent)
                    {
                        json_indent(out, depth + 1);
                    }
                    json_quote(out, String::as_view(&members[i].key));
                    String::append(out, if (indent) { ": " } else { ":" });
                    json_write(out, &members[i].value, indent, depth + 1);
                }
                if (indent && Vec::len(members) > 0)
                {
                    json_indent(out, depth);
                }
                String::push_ascii(out, b'}');
            },
        }
    }

    // `j` as JSON text, compact: no whitespace, members and elements in
    // order. `Json::parse` of it gives `j` back.
    export fn Json::text(ref<Json, shared> j) : String
    {
        String out = String::new();
        json_write(&mut out, j, false, 0);
        out
    }

    // `j` as JSON text for people: one element or member per line, two
    // spaces per level, `: ` after a key; no newline at the end.
    export fn Json::pretty(ref<Json, shared> j) : String
    {
        String out = String::new();
        json_write(&mut out, j, true, 0);
        out
    }

    // ---- reading a tree ----

    // The value of the first member named `key` of an object; `None` for
    // another kind of value or no such member.
    export fn Json::get(ref<Json, shared> j, StringView key) : Option<ref<Json, shared>>
    {
        match (j)
        {
            JsonObject(members) :
            {
                for (usize i = 0; i < Vec::len(members); i += 1)
                {
                    if (members[i].key == key)
                    {
                        return Some(&members[i].value);
                    }
                }
                None
            },
            _                   : None,
        }
    }

    // Element `i` of an array; `None` for another kind of value or past the
    // end.
    export fn Json::at(ref<Json, shared> j, usize i) : Option<ref<Json, shared>>
    {
        match (j)
        {
            JsonArray(items) : if (i < Vec::len(items)) { Some(&items[i]) } else { None },
            _                : None,
        }
    }

    export fn Json::is_null(ref<Json, shared> j) : bool
    {
        match (j)
        {
            JsonNull : true,
            _        : false,
        }
    }

    export fn Json::as_bool(ref<Json, shared> j) : Option<bool>
    {
        match (j)
        {
            JsonBool(b) : Some(*b),
            _           : None,
        }
    }

    export fn Json::as_f64(ref<Json, shared> j) : Option<f64>
    {
        match (j)
        {
            JsonNumber(x) : Some(*x),
            _             : None,
        }
    }

    // The number as an integer: `Some` only for a whole value within `i64`.
    export fn Json::as_i64(ref<Json, shared> j) : Option<i64>
    {
        match (j)
        {
            JsonNumber(x) : if (*x == trunc(*x) && *x >= -9223372036854775808.0 && *x < 9223372036854775808.0) { Some(to_int<i64>(*x)) } else { None },
            _             : None,
        }
    }

    export fn Json::as_text(ref<Json, shared> j) : Option<StringView>
    {
        match (j)
        {
            JsonText(s) : Some(String::as_view(s)),
            _           : None,
        }
    }

    export fn Json::as_array(ref<Json, shared> j) : Option<ref<Vec<Json>, shared>>
    {
        match (j)
        {
            JsonArray(items) : Some(items),
            _                : None,
        }
    }

    export fn Json::as_object(ref<Json, shared> j) : Option<ref<Vec<JsonMember>, shared>>
    {
        match (j)
        {
            JsonObject(members) : Some(members),
            _                   : None,
        }
    }

    // ---- building a tree ----

    // Sets member `key` of an object to `v`: the first member of that name
    // replaced, or a new member appended. On a value that is not an object:
    // `diag.assert-failed`.
    export fn Json::set(ref<Json, exclusive> j, StringView key, Json v)
    {
        match (j)
        {
            JsonObject(members) :
            {
                for (usize i = 0; i < Vec::len(members); i += 1)
                {
                    if (members[i].key == key)
                    {
                        overwrite(&mut members[i].value, v);
                        return;
                    }
                }
                Vec::push(members, JsonMember { .key = String::from_view(key), .value = v });
            },
            _                   : assert(false, "Json::set on a value that is not an object"),
        }
    }

    // Appends `v` to an array. On a value that is not an array:
    // `diag.assert-failed`.
    export fn Json::push(ref<Json, exclusive> j, Json v)
    {
        match (j)
        {
            JsonArray(items) : Vec::push(items, v),
            _                : assert(false, "Json::push on a value that is not an array"),
        }
    }

**Depends on:** rule.stdlib.text, rule.stdlib.stringview, rule.stdlib.vec, rule.agg.layout, D-0182
**Affects:** none (pure)

## 2s. Compression

In `std::compress` (D-0183).

### `rule.stdlib.compress`
**Status:** ACCEPTED

DEFLATE (RFC 1951) and its two wrappers, zlib (RFC 1950) and gzip (RFC
1952), both ways, and Adler-32. Compression is one fixed strategy — so
every implementation writes the same bytes — and decompression reads
every well-formed stream. Written in CobaltC; nothing is linked.

    [Inflate]          (D-0183)   ⟨inflate(d, max), Σ⟩ → Ok(the bytes d's DEFLATE blocks encode, read from its first byte to
                       its final block: stored, fixed-Huffman and dynamic-Huffman blocks, lengths 3 to 258, distances
                       to 32 768); bytes after the final block are ignored. Err(Truncated) when d ends inside a block;
                       Err(Corrupt(i)) at byte i for a block type 3, a stored length that does not match its
                       complement, an over-subscribed or incomplete code, a length or distance code out of range, a
                       distance before the start; Err(Oversize) as soon as the output would exceed max bytes
    [Deflate]          (D-0183)   ⟨deflate(d), Σ⟩ → a DEFLATE stream s with inflate(s, len(d)) = d, written as the listing
                       says: LZ77 matches found through a hash of three bytes and a chain of at most 64 earlier
                       positions within 32 768 bytes (the longest match, the first of equals), tokens cut into blocks
                       of 32 768, each block written as stored (when that is shortest: its bytes in pieces of at most
                       65 535), fixed or dynamic Huffman (code lengths from a Huffman tree over the block's
                       frequencies, flattened until no code exceeds 15 bits, 7 for the code-length code); empty data
                       is one empty fixed block (03 00)
    [Zlib]             (D-0183)   ⟨zlib_deflate(d), Σ⟩ → 78 9C ‖ deflate(d) ‖ adler32(d) big-endian. ⟨zlib_inflate(s, max), Σ⟩
                       → Ok(the inflated bytes) when s begins with a zlib header for DEFLATE without a preset
                       dictionary and ends with their Adler-32 after the final block; Err(Corrupt(0)) for another
                       header, Err(Corrupt(i)) at the trailer for a wrong checksum, Err(Truncated) for fewer than 6
                       bytes or a missing trailer; inflate's errors otherwise
    [Gzip]             (D-0183)   ⟨gzip(d), Σ⟩ → 1F 8B 08 00, four zero bytes of time, 00, FF (an unknown system) ‖
                       deflate(d) ‖ crc32(d) ‖ len(d) mod 2^32, the last two little-endian. ⟨gunzip(s, max), Σ⟩ →
                       Ok(the inflated bytes of every member of s in turn): each member's header (1F 8B 08, its flags'
                       extra field, name, comment and header CRC skipped), its DEFLATE stream, and a trailer whose
                       CRC-32 and length match the member's bytes (Err(Corrupt(i)) at the trailer otherwise);
                       Err(Corrupt(i)) at a header that is not gzip's; Err(Truncated) for a member cut short
    [Adler32]          (D-0183, RFC 1950 §8.2)   ⟨adler32(d), Σ⟩ → b · 2^16 + a, a = (1 + Σ d[i]) mod 65521, b = Σ of a
                       after each byte, mod 65521; adler32 of nothing is 1, of "Wikipedia" 300286872
    [Compress-Error]   (D-0183)   CompressError: Corrupt(usize), Truncated, Oversize; CompressError::text as D-0117's

The `std` source:

    // `std::compress` (spec/21 §2s, D-0183): DEFLATE (RFC 1951) and its two
    // wrappers, zlib (RFC 1950) and gzip (RFC 1952), compressed and
    // decompressed; Adler-32. Written in CobaltC. Compression is LZ77 over a
    // 32 KiB window with hash chains, each block written in whichever of the
    // three block kinds is smallest (stored, fixed or dynamic Huffman);
    // decompression reads every well-formed stream. `crc32` is `std::crypto`'s.

    // Why a stream could not be decompressed. `Corrupt(i)`: the stream is
    // not DEFLATE (or not the wrapper asked for), or its checksum does not
    // match, first seen at byte `i`; `Truncated`: it ends before its last
    // block; `Oversize`: the output would exceed the `max` given.
    export enum CompressError
    {
        Corrupt(usize),
        Truncated,
        Oversize,
    }

    export fn CompressError::text(ref<CompressError, shared> e) : str
    {
        match (e)
        {
            Corrupt(_) : "corrupt compressed data",
            Truncated  : "compressed data ends early",
            Oversize   : "decompressed data too large",
        }
    }

    // ---- the DEFLATE tables (RFC 1951 §3.2.5) ----

    // The length codes 257 to 285: the base length of each and its extra bits.
    const array<u16, 29> LENGTH_BASE = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
    ];
    const array<u8, 29> LENGTH_EXTRA = [
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        1,
        1,
        1,
        1,
        2,
        2,
        2,
        2,
        3,
        3,
        3,
        3,
        4,
        4,
        4,
        4,
        5,
        5,
        5,
        5,
        0,
    ];
    // The distance codes 0 to 29.
    const array<u16, 30> DIST_BASE = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const array<u8, 30> DIST_EXTRA = [
        0,
        0,
        0,
        0,
        1,
        1,
        2,
        2,
        3,
        3,
        4,
        4,
        5,
        5,
        6,
        6,
        7,
        7,
        8,
        8,
        9,
        9,
        10,
        10,
        11,
        11,
        12,
        12,
        13,
        13,
    ];
    // The order the code-length code lengths are sent in.
    const array<u8, 19> CLEN_ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

    // ---- decompression ----

    struct BitReader
    {
        slice<u8, shared> data;
        usize at;                           // the next byte
        u32 bits;                           // bits not yet taken, least significant first
        u32 count;
    }

    // The next `n` bits (at most 16), least significant first.
    fn BitReader::take(ref<BitReader, exclusive> r, u32 n) : Result<u32, CompressError>
    {
        while (r.count < n)
        {
            if (r.at >= slice_len(r.data))
            {
                return Err(Truncated);
            }
            r.bits = r.bits | (widen<u32>(r.data[r.at]) << r.count);
            r.at += 1;
            r.count += 8;
        }
        u32 v = r.bits & ((1: u32 << n) - 1);
        r.bits = r.bits >> n;
        r.count -= n;
        Ok(v)
    }

    fn BitReader::align(ref<BitReader, exclusive> r)
    {
        r.bits = 0;
        r.count = 0;
    }

    // A canonical Huffman code, as puff.c decodes it: how many codes have
    // each length, and the symbols in code order. Plain data (at most 288
    // symbols, the literal/length alphabet), so the decoder below can reach
    // it through a reference with no check at run time.
    struct Huffman
    {
        array<u16, 16> counts;
        array<u16, 320> symbols;
    }

    // The code for symbols with the given lengths (0: unused); `false` for
    // an over-subscribed set of lengths.
    fn Huffman::build(slice<u8, shared> lengths) : Option<Huffman>
    {
        array<u16, 16> counts = [0; 16];
        for (usize i = 0; i < slice_len(lengths); i += 1)
        {
            counts[widen<usize>(lengths[i])] += 1;
        }
        counts[0] = 0;
        i32 left = 1;
        for (usize len = 1; len < 16; len += 1)
        {
            left = left * 2 - widen<i32>(counts[len]);
            if (left < 0)
            {
                return None;
            }
        }
        array<u16, 16> offs = [0; 16];
        for (usize len = 1; len < 15; len += 1)
        {
            offs[len + 1] = offs[len] + counts[len];
        }
        array<u16, 320> symbols = [0; 320];
        for (usize i = 0; i < slice_len(lengths); i += 1)
        {
            if (lengths[i] != 0)
            {
                symbols[widen<usize>(offs[widen<usize>(lengths[i])])] = narrow<u16>(i);
                offs[widen<usize>(lengths[i])] += 1;
            }
        }
        Some(Huffman { .counts = counts, .symbols = symbols })
    }

    // The next symbol, bit by bit.
    fn Huffman::decode(ref<Huffman, shared> h, ref<BitReader, exclusive> r) : Result<u16, CompressError>
    {
        HuffStep s = huff_step(r.data, h, r.at, r.bits, r.count);
        r.at = s.at;
        r.bits = s.bits;
        r.count = s.count;
        if (s.sym == -1)
        {
            return Err(Truncated);
        }
        if (s.sym < 0)
        {
            return Err(Corrupt(r.at));
        }
        Ok(narrow<u16>(s.sym))
    }

    // The bit reader's state passed by value (as the hashes' blocks are, so
    // that nothing is checked per bit): a symbol decoded from `data` (-1: the
    // data ran out, -2: no code matched), or `need` bits taken.
    struct HuffStep
    {
        i32 sym;
        usize at;
        u32 bits;
        u32 count;
    }

    fn huff_step(slice<u8, shared> data, ref<Huffman, shared> h, usize at, u32 bits, u32 count) : HuffStep
    {
        usize n = slice_len(data);
        usize p = at;
        u32 bb = bits;
        u32 bc = count;
        i32 code = 0;
        i32 first = 0;
        i32 index = 0;
        for (usize len = 1; len < 16; len += 1)
        {
            if (bc == 0)
            {
                if (p >= n)
                {
                    return HuffStep { .sym = -1, .at = p, .bits = bb, .count = bc };
                }
                bb = widen<u32>(data[p]);
                p += 1;
                bc = 8;
            }
            code = code | narrow<i32>(bb & 1);
            bb = bb >> 1;
            bc -= 1;
            i32 c = widen<i32>(h.counts[len]);
            if (code - c < first)
            {
                return HuffStep { .sym = widen<i32>(h.symbols[narrow<usize>(index + (code - first))]), .at = p, .bits = bb, .count = bc };
            }
            index += c;
            first += c;
            first = first << 1;
            code = code << 1;
        }
        HuffStep { .sym = -2, .at = p, .bits = bb, .count = bc }
    }

    struct BitsStep
    {
        bool ok;
        u32 v;
        usize at;
        u32 bits;
        u32 count;
    }

    fn bits_step(slice<u8, shared> data, usize at, u32 bits, u32 count, u32 need) : BitsStep
    {
        usize p = at;
        u32 bb = bits;
        u32 bc = count;
        while (bc < need)
        {
            if (p >= slice_len(data))
            {
                return BitsStep { .ok = false, .v = 0, .at = p, .bits = bb, .count = bc };
            }
            bb = bb | (widen<u32>(data[p]) << bc);
            p += 1;
            bc += 8;
        }
        u32 v = bb & ((1: u32 << need) - 1);
        BitsStep { .ok = true, .v = v, .at = p, .bits = bb >> need, .count = bc - need }
    }

    // The fixed codes of RFC 1951 §3.2.6.
    fn fixed_codes() : Huffman
    {
        Vec<u8> lit = Vec::filled(288, 8: u8);
        for (usize i = 144; i < 256; i += 1)
        {
            lit[i] = 9;
        }
        for (usize i = 256; i < 280; i += 1)
        {
            lit[i] = 7;
        }
        Option::unwrap(Huffman::build(&lit[0..$]))
    }

    fn fixed_distances() : Huffman
    {
        Vec<u8> d = Vec::filled(30, 5: u8);
        Option::unwrap(Huffman::build(&d[0..$]))
    }

    // One compressed block's symbols copied out, under the codes given. The
    // work is `inflate_run`'s, on the output vector and the bit reader's
    // state passed by value: its references are then all shared ones to
    // plain data, and the vector a confined local, none of them checked per
    // symbol.
    fn inflate_codes(
        ref<BitReader, exclusive> r,
        ref<Vec<u8>, exclusive> out,
        ref<Huffman, shared> lit,
        ref<Huffman, shared> dist,
        usize max,
    ) : Result<void, CompressError>
    {
        Vec<u8> o = replace(out, Vec::new());
        InflateRun { bytes, pos, bitbuf, nbits, status } = inflate_run(r.data, lit, dist, o, r.at, r.bits, r.count, max);
        r.at = pos;
        r.bits = bitbuf;
        r.count = nbits;
        overwrite(out, bytes);
        if (status == 1)
        {
            return Err(Truncated);
        }
        if (status == 2)
        {
            return Err(Corrupt(r.at));
        }
        if (status == 3)
        {
            return Err(Oversize);
        }
        Ok(())
    }

    // `inflate_codes`'s result: the output and the reader's state, and how the
    // block ended: 0 at its end code, 1 the data ran out (`Truncated`), 2 a
    // bad code or distance (`Corrupt` at `at`), 3 beyond `max` (`Oversize`).
    struct InflateRun
    {
        Vec<u8> bytes;
        usize pos;
        u32 bitbuf;
        u32 nbits;
        u32 status;
    }

    fn inflate_run(
        slice<u8, shared> data,
        ref<Huffman, shared> lit,
        ref<Huffman, shared> dist,
        Vec<u8> out,
        usize at0,
        u32 bits0,
        u32 count0,
        usize max,
    ) : InflateRun
    {
        usize at = at0;
        u32 bits = bits0;
        u32 count = count0;
        u32 status = 0;
        while (true)
        {
            HuffStep s = huff_step(data, lit, at, bits, count);
            at = s.at;
            bits = s.bits;
            count = s.count;
            if (s.sym < 0)
            {
                status = if (s.sym == -1) { 1 } else { 2 };
                break;
            }
            if (s.sym < 256)
            {
                if (Vec::len(&out) >= max)
                {
                    status = 3;
                    break;
                }
                Vec::push(&mut out, narrow<u8>(s.sym));
                continue;
            }
            if (s.sym == 256)
            {
                break;
            }
            usize li = narrow<usize>(s.sym) - 257;
            if (li >= 29)
            {
                status = 2;
                break;
            }
            BitsStep e = bits_step(data, at, bits, count, widen<u32>(LENGTH_EXTRA[li]));
            at = e.at;
            bits = e.bits;
            count = e.count;
            if (!e.ok)
            {
                status = 1;
                break;
            }
            usize len = widen<usize>(LENGTH_BASE[li]) + narrow<usize>(e.v);
            HuffStep d = huff_step(data, dist, at, bits, count);
            at = d.at;
            bits = d.bits;
            count = d.count;
            if (d.sym < 0)
            {
                status = if (d.sym == -1) { 1 } else { 2 };
                break;
            }
            usize di = narrow<usize>(d.sym);
            if (di >= 30)
            {
                status = 2;
                break;
            }
            BitsStep f = bits_step(data, at, bits, count, widen<u32>(DIST_EXTRA[di]));
            at = f.at;
            bits = f.bits;
            count = f.count;
            if (!f.ok)
            {
                status = 1;
                break;
            }
            usize distance = widen<usize>(DIST_BASE[di]) + narrow<usize>(f.v);
            if (distance > Vec::len(&out))
            {
                status = 2;
                break;
            }
            if (Vec::len(&out) + len > max)
            {
                status = 3;
                break;
            }
            usize from = Vec::len(&out) - distance;
            for (usize k = 0; k < len; k += 1)
            {
                u8 b = out[from + k];
                Vec::push(&mut out, b);
            }
        }
        InflateRun { .bytes = out, .pos = at, .bitbuf = bits, .nbits = count, .status = status }
    }

    // A dynamic block's two codes, read from its header (RFC 1951 §3.2.7).
    fn read_dynamic(
        ref<BitReader, exclusive> r,
        ref<Option<Huffman>, exclusive> lit,
        ref<Option<Huffman>, exclusive> dist,
    ) : Result<void, CompressError>
    {
        usize nlen = narrow<usize>(BitReader::take(r, 5)?) + 257;
        usize ndist = narrow<usize>(BitReader::take(r, 5)?) + 1;
        usize ncode = narrow<usize>(BitReader::take(r, 4)?) + 4;
        if (nlen > 286 || ndist > 30)
        {
            return Err(Corrupt(r.at));
        }
        Vec<u8> clens = Vec::filled(19, 0: u8);
        for (usize i = 0; i < ncode; i += 1)
        {
            clens[widen<usize>(CLEN_ORDER[i])] = narrow<u8>(BitReader::take(r, 3)?);
        }
        Huffman ccode = match (Huffman::build(&clens[0..$]))
        {
            Some(h) : h,
            None    : return Err(Corrupt(r.at)),
        };
        Vec<u8> lengths = Vec::new();
        while (Vec::len(&lengths) < nlen + ndist)
        {
            u16 sym = Huffman::decode(&ccode, r)?;
            if (sym < 16)
            {
                Vec::push(&mut lengths, narrow<u8>(sym));
                continue;
            }
            u8 value = 0;
            usize repeat = 0;
            if (sym == 16)
            {
                if (Vec::len(&lengths) == 0)
                {
                    return Err(Corrupt(r.at));
                }
                value = lengths[Vec::len(&lengths) - 1];
                repeat = 3 + narrow<usize>(BitReader::take(r, 2)?);
            }
            else if (sym == 17)
            {
                repeat = 3 + narrow<usize>(BitReader::take(r, 3)?);
            }
            else
            {
                repeat = 11 + narrow<usize>(BitReader::take(r, 7)?);
            }
            if (Vec::len(&lengths) + repeat > nlen + ndist)
            {
                return Err(Corrupt(r.at));
            }
            for (usize k = 0; k < repeat; k += 1)
            {
                Vec::push(&mut lengths, value);
            }
        }
        if (lengths[256] == 0)
        {
            return Err(Corrupt(r.at));
        }
        match (Huffman::build(&lengths[0..nlen]))
        {
            Some(h) : overwrite(lit, Some(h)),
            None    : return Err(Corrupt(r.at)),
        }
        match (Huffman::build(&lengths[nlen..nlen + ndist]))
        {
            Some(h) : overwrite(dist, Some(h)),
            None    : return Err(Corrupt(r.at)),
        }
        Ok(())
    }

    // The DEFLATE stream in `data`, from `start`, inflated onto `out`; the
    // position after its last block.
    fn inflate_from(
        slice<u8, shared> data,
        usize start,
        ref<Vec<u8>, exclusive> out,
        usize max,
    ) : Result<usize, CompressError>
    {
        BitReader r = BitReader { .data = data, .at = start, .bits = 0, .count = 0 };
        Huffman fixed_lit = fixed_codes();
        Huffman fixed_dist = fixed_distances();
        while (true)
        {
            u32 last = BitReader::take(&mut r, 1)?;
            u32 kind = BitReader::take(&mut r, 2)?;
            if (kind == 0)
            {
                BitReader::align(&mut r);
                if (r.at + 4 > slice_len(data))
                {
                    return Err(Truncated);
                }
                usize len = widen<usize>(read_le<u16>(data, r.at));
                usize nlen = widen<usize>(read_le<u16>(data, r.at + 2));
                if (len != (nlen ^ 0xFFFF))
                {
                    return Err(Corrupt(r.at));
                }
                r.at += 4;
                if (r.at + len > slice_len(data))
                {
                    return Err(Truncated);
                }
                if (Vec::len(out) + len > max)
                {
                    return Err(Oversize);
                }
                Vec::extend_from(out, &data[r.at..r.at + len]);
                r.at += len;
            }
            else if (kind == 1)
            {
                inflate_codes(&mut r, out, &fixed_lit, &fixed_dist, max)?;
            }
            else if (kind == 2)
            {
                Option<Huffman> lit = None;
                Option<Huffman> dist = None;
                read_dynamic(&mut r, &mut lit, &mut dist)?;
                inflate_codes(&mut r, out, &Option::unwrap(lit), &Option::unwrap(dist), max)?;
            }
            else
            {
                return Err(Corrupt(r.at));
            }
            if (last == 1)
            {
                // The bits left in the reader's buffer belong to no block; the
                // position after the block is the next whole byte.
                return Ok(r.at);
            }
        }
    }

    // The bytes a raw DEFLATE stream (RFC 1951, no header) compresses, at
    // most `max` of them (`Oversize` beyond that: a limit against a small
    // stream that expands without bound). Data after the last block is
    // ignored.
    export fn inflate(slice<u8, shared> data, usize max) : Result<Vec<u8>, CompressError>
    {
        Vec<u8> out = Vec::new();
        inflate_from(data, 0, &mut out, max)?;
        Ok(out)
    }

    // ---- compression ----

    struct BitWriter
    {
        Vec<u8> out;
        u64 bits;
        u32 count;
    }

    fn BitWriter::put(ref<BitWriter, exclusive> w, u32 value, u32 n)
    {
        w.bits = w.bits | (widen<u64>(value) << w.count);
        w.count += n;
        while (w.count >= 8)
        {
            Vec::push(&mut w.out, narrow_wrapping<u8>(w.bits));
            w.bits = w.bits >> 8;
            w.count -= 8;
        }
    }

    // A Huffman code's bits are sent most significant first: reversed.
    fn BitWriter::put_code(ref<BitWriter, exclusive> w, u32 code, u32 len)
    {
        u32 rev = 0;
        for (u32 i = 0; i < len; i += 1)
        {
            rev = (rev << 1) | ((code >> i) & 1);
        }
        BitWriter::put(w, rev, len);
    }

    fn BitWriter::flush(ref<BitWriter, exclusive> w)
    {
        if (w.count > 0)
        {
            Vec::push(&mut w.out, narrow_wrapping<u8>(w.bits));
            w.bits = 0;
            w.count = 0;
        }
    }

    // Code lengths for `freq` (0 for an unused symbol), none longer than
    // `limit`: a Huffman tree built by merging the two lightest nodes, and,
    // when it is too deep, rebuilt over flattened frequencies until it fits
    // (a little longer, never invalid). A lone used symbol gets length 1.
    fn code_lengths(slice<u32, shared> freq, usize limit) : Vec<u8>
    {
        usize n = slice_len(freq);
        Vec<u8> lengths = Vec::filled(n, 0: u8);
        Vec<u32> f = Vec::from_slice(freq);
        usize used = 0;
        for (usize i = 0; i < n; i += 1)
        {
            if (f[i] > 0)
            {
                used += 1;
            }
        }
        if (used == 0)
        {
            return lengths;
        }
        if (used == 1)
        {
            for (usize i = 0; i < n; i += 1)
            {
                if (f[i] > 0)
                {
                    lengths[i] = 1;
                }
            }
            return lengths;
        }
        while (true)
        {
            // Nodes 0..n are the symbols, n.. the merges; each node's weight,
            // parent and depth.
            Vec<u64> weight = Vec::new();
            Vec<usize> parent = Vec::new();
            Vec<bool> alive = Vec::new();
            for (usize i = 0; i < n; i += 1)
            {
                Vec::push(&mut weight, widen<u64>(f[i]));
                Vec::push(&mut parent, 0);
                Vec::push(&mut alive, f[i] > 0);
            }
            usize live = used;
            while (live > 1)
            {
                usize a = n * 2;
                usize b = n * 2;
                for (usize i = 0; i < Vec::len(&weight); i += 1)
                {
                    if (!alive[i])
                    {
                        continue;
                    }
                    if (a == n * 2 || weight[i] < weight[a])
                    {
                        b = a;
                        a = i;
                    }
                    else if (b == n * 2 || weight[i] < weight[b])
                    {
                        b = i;
                    }
                }
                usize m = Vec::len(&weight);
                Vec::push(&mut weight, weight[a] + weight[b]);
                Vec::push(&mut parent, 0);
                Vec::push(&mut alive, true);
                alive[a] = false;
                alive[b] = false;
                parent[a] = m;
                parent[b] = m;
                live -= 1;
            }
            usize root = Vec::len(&weight) - 1;
            usize deepest = 0;
            for (usize i = 0; i < n; i += 1)
            {
                if (f[i] == 0)
                {
                    continue;
                }
                usize d = 0;
                usize at = i;
                while (at != root)
                {
                    at = parent[at];
                    d += 1;
                }
                lengths[i] = narrow<u8>(d);
                if (d > deepest)
                {
                    deepest = d;
                }
            }
            if (deepest <= limit)
            {
                return lengths;
            }
            for (usize i = 0; i < n; i += 1)
            {
                if (f[i] > 0)
                {
                    f[i] = (f[i] >> 1) | 1;
                }
            }
        }
    }

    // The canonical codes for `lengths` (RFC 1951 §3.2.2).
    fn canonical_codes(slice<u8, shared> lengths) : Vec<u32>
    {
        array<u32, 16> count = [0; 16];
        for (usize i = 0; i < slice_len(lengths); i += 1)
        {
            count[widen<usize>(lengths[i])] += 1;
        }
        count[0] = 0;
        array<u32, 16> next = [0; 16];
        u32 code = 0;
        for (usize len = 1; len < 16; len += 1)
        {
            code = (code + count[len - 1]) << 1;
            next[len] = code;
        }
        Vec<u32> codes = Vec::filled(slice_len(lengths), 0: u32);
        for (usize i = 0; i < slice_len(lengths); i += 1)
        {
            usize l = widen<usize>(lengths[i]);
            if (l != 0)
            {
                codes[i] = next[l];
                next[l] += 1;
            }
        }
        codes
    }

    // The LZ77 symbols of one block: a literal (< 256), or a match as its
    // length code with extra bits and its distance code with extra bits.
    struct Token
    {
        u16 lit;                            // a literal, or 256 + length code - 257 for a match
        u16 len_extra;
        u8 len_extra_bits;
        u8 dist_code;
        u16 dist_extra;
        u8 dist_extra_bits;
    }

    fn length_code(usize len) : usize
    {
        usize c = 28;
        for (usize i = 0; i < 28; i += 1)
        {
            if (len < widen<usize>(LENGTH_BASE[i + 1]))
            {
                c = i;
                break;
            }
        }
        c
    }

    fn distance_code(usize d) : usize
    {
        usize c = 29;
        for (usize i = 0; i < 29; i += 1)
        {
            if (d < widen<usize>(DIST_BASE[i + 1]))
            {
                c = i;
                break;
            }
        }
        c
    }

    const usize WINDOW = 32768;
    const usize HASH_SIZE = 32768;
    const usize CHAIN_LIMIT = 64;
    const usize MAX_TOKENS = 32768;

    fn hash3(slice<u8, shared> d, usize i) : usize
    {
        ((widen<usize>(d[i]) << 10) ^ (widen<usize>(d[i + 1]) << 5) ^ widen<usize>(d[i + 2])) & (HASH_SIZE - 1)
    }

    // How often each literal/length and distance code occurs among a block's
    // tokens, and their extra bits in all: counted with the tokens the only
    // reference, read unchecked.
    struct TokenCounts
    {
        array<u32, 286> lit;
        array<u32, 30> dist;
        u64 extra;
    }

    fn token_counts(slice<Token, shared> tokens) : TokenCounts
    {
        array<u32, 286> lit = [0; 286];
        array<u32, 30> dist = [0; 30];
        u64 extra = 0;
        for (usize k = 0; k < slice_len(tokens); k += 1)
        {
            u16 l = tokens[k].lit;
            lit[widen<usize>(l)] += 1;
            if (l >= 256)
            {
                dist[widen<usize>(tokens[k].dist_code)] += 1;
                extra += widen<u64>(tokens[k].len_extra_bits) + widen<u64>(tokens[k].dist_extra_bits);
            }
        }
        TokenCounts { .lit = lit, .dist = dist, .extra = extra }
    }

    // One block's tokens written: as stored, fixed or dynamic, whichever is
    // shortest.
    fn write_block(ref<BitWriter, exclusive> w, slice<Token, shared> tokens, slice<u8, shared> raw, bool last)
    {
        // Frequencies, with the end-of-block symbol.
        TokenCounts counts = token_counts(tokens);
        Vec<u32> lf = Vec::from_slice(&counts.lit[0..$]);
        Vec<u32> df = Vec::from_slice(&counts.dist[0..$]);
        lf[256] += 1;
        Vec<u8> ll = code_lengths(&lf[0..$], 15);
        Vec<u8> dl = code_lengths(&df[0..$], 15);
        // The fixed code's lengths, for its cost.
        Vec<u8> fl = Vec::filled(288, 8: u8);
        for (usize i = 144; i < 256; i += 1)
        {
            fl[i] = 9;
        }
        for (usize i = 256; i < 280; i += 1)
        {
            fl[i] = 7;
        }
        u64 extra = counts.extra;
        u64 cost_fixed = extra;
        u64 cost_dyn = extra;
        for (usize i = 0; i < 286; i += 1)
        {
            cost_fixed += widen<u64>(lf[i]) * widen<u64>(fl[i]);
            cost_dyn += widen<u64>(lf[i]) * widen<u64>(ll[i]);
        }
        for (usize i = 0; i < 30; i += 1)
        {
            cost_fixed += widen<u64>(df[i]) * 5;
            cost_dyn += widen<u64>(df[i]) * widen<u64>(dl[i]);
        }
        // The dynamic header: the code lengths run-length coded.
        usize nlen = 286;
        while (nlen > 257 && ll[nlen - 1] == 0)
        {
            nlen -= 1;
        }
        usize ndist = 30;
        while (ndist > 1 && dl[ndist - 1] == 0)
        {
            ndist -= 1;
        }
        Vec<u8> all = Vec::new();
        Vec::extend_from(&mut all, &ll[0..nlen]);
        Vec::extend_from(&mut all, &dl[0..ndist]);
        // Run-length symbols: (symbol, extra value, extra bits).
        Vec<u8> rl_sym = Vec::new();
        Vec<u8> rl_val = Vec::new();
        Vec<u8> rl_bits = Vec::new();
        usize i = 0;
        while (i < Vec::len(&all))
        {
            u8 v = all[i];
            usize run = 1;
            while (i + run < Vec::len(&all) && all[i + run] == v)
            {
                run += 1;
            }
            if (v == 0 && run >= 3)
            {
                usize r = if (run > 138) { 138 } else { run };
                if (r <= 10)
                {
                    Vec::push(&mut rl_sym, 17);
                    Vec::push(&mut rl_val, narrow<u8>(r - 3));
                    Vec::push(&mut rl_bits, 3);
                }
                else
                {
                    Vec::push(&mut rl_sym, 18);
                    Vec::push(&mut rl_val, narrow<u8>(r - 11));
                    Vec::push(&mut rl_bits, 7);
                }
                i += r;
            }
            else if (v != 0 && run >= 4)
            {
                Vec::push(&mut rl_sym, v);
                Vec::push(&mut rl_val, 0);
                Vec::push(&mut rl_bits, 0);
                usize r = if (run - 1 > 6) { 6 } else { run - 1 };
                Vec::push(&mut rl_sym, 16);
                Vec::push(&mut rl_val, narrow<u8>(r - 3));
                Vec::push(&mut rl_bits, 2);
                i += 1 + r;
            }
            else
            {
                Vec::push(&mut rl_sym, v);
                Vec::push(&mut rl_val, 0);
                Vec::push(&mut rl_bits, 0);
                i += 1;
            }
        }
        Vec<u32> cf = Vec::filled(19, 0: u32);
        foreach (s in &rl_sym)
        {
            cf[widen<usize>(*s)] += 1;
        }
        Vec<u8> cl = code_lengths(&cf[0..$], 7);
        usize ncode = 19;
        while (ncode > 4 && cl[widen<usize>(CLEN_ORDER[ncode - 1])] == 0)
        {
            ncode -= 1;
        }
        cost_dyn += 14 + widen<u64>(ncode) * 3;
        for (usize k = 0; k < Vec::len(&rl_sym); k += 1)
        {
            cost_dyn += widen<u64>(cl[widen<usize>(rl_sym[k])]) + widen<u64>(rl_bits[k]);
        }
        u64 cost_stored = widen<u64>(slice_len(raw)) * 8 + 40 * (widen<u64>(slice_len(raw)) / 65535 + 1);
        if (cost_stored <= cost_fixed && cost_stored <= cost_dyn)
        {
            // Stored blocks of at most 65535 bytes, the last marked.
            usize at = 0;
            while (true)
            {
                usize take = if (slice_len(raw) - at > 65535) { 65535 } else { slice_len(raw) - at };
                bool final = last && at + take == slice_len(raw);
                BitWriter::put(w, if (final) { 1 } else { 0 }, 1);
                BitWriter::put(w, 0, 2);
                BitWriter::flush(w);
                Vec::push_le(&mut w.out, narrow<u16>(take));
                Vec::push_le(&mut w.out, narrow<u16>(take ^ 0xFFFF));
                Vec::extend_from(&mut w.out, &raw[at..at + take]);
                at += take;
                if (at >= slice_len(raw))
                {
                    return;
                }
            }
        }
        BitWriter::put(w, if (last) { 1 } else { 0 }, 1);
        Vec<u32> lcodes = Vec::new();
        Vec<u8> llens = Vec::new();
        Vec<u32> dcodes = Vec::new();
        Vec<u8> dlens = Vec::new();
        if (cost_fixed <= cost_dyn)
        {
            BitWriter::put(w, 1, 2);
            overwrite(&mut lcodes, canonical_codes(&fl[0..$]));
            overwrite(&mut llens, fl);
            Vec<u8> fd = Vec::filled(30, 5: u8);
            overwrite(&mut dcodes, canonical_codes(&fd[0..$]));
            overwrite(&mut dlens, fd);
        }
        else
        {
            BitWriter::put(w, 2, 2);
            BitWriter::put(w, narrow<u32>(nlen - 257), 5);
            BitWriter::put(w, narrow<u32>(ndist - 1), 5);
            BitWriter::put(w, narrow<u32>(ncode - 4), 4);
            for (usize k = 0; k < ncode; k += 1)
            {
                BitWriter::put(w, widen<u32>(cl[widen<usize>(CLEN_ORDER[k])]), 3);
            }
            Vec<u32> ccodes = canonical_codes(&cl[0..$]);
            for (usize k = 0; k < Vec::len(&rl_sym); k += 1)
            {
                usize s = widen<usize>(rl_sym[k]);
                BitWriter::put_code(w, ccodes[s], widen<u32>(cl[s]));
                if (rl_bits[k] > 0)
                {
                    BitWriter::put(w, widen<u32>(rl_val[k]), widen<u32>(rl_bits[k]));
                }
            }
            overwrite(&mut lcodes, canonical_codes(&ll[0..$]));
            overwrite(&mut llens, ll);
            overwrite(&mut dcodes, canonical_codes(&dl[0..$]));
            overwrite(&mut dlens, dl);
        }
        Vec<u8> o = replace(&mut w.out, Vec::new());
        BitOut { buf, acc, nacc } = put_tokens(
            o,
            w.bits,
            w.count,
            tokens,
            &lcodes[0..$],
            &llens[0..$],
            &dcodes[0..$],
            &dlens[0..$],
        );
        overwrite(&mut w.out, buf);
        w.bits = acc;
        w.count = nacc;
    }

    // A block's tokens and its end code written under the codes given: the
    // output and the bit buffer passed by value, the rest shared slices of
    // plain data, so nothing here is checked per code.
    struct BitOut
    {
        Vec<u8> buf;
        u64 acc;
        u32 nacc;
    }

    fn put_tokens(
        Vec<u8> out,
        u64 bits0,
        u32 count0,
        slice<Token, shared> tokens,
        slice<u32, shared> lcodes,
        slice<u8, shared> llens,
        slice<u32, shared> dcodes,
        slice<u8, shared> dlens,
    ) : BitOut
    {
        u64 bits = bits0;
        u32 count = count0;
        usize nt = slice_len(tokens);
        for (usize k = 0; k <= nt; k += 1)
        {
            // Up to four fields per token: its code (or, after the last, the
            // end code), then for a match the length's extra bits, the
            // distance's code and its extra bits. A code goes most
            // significant bit first: reversed.
            usize s = if (k < nt) { widen<usize>(tokens[k].lit) } else { 256 };
            for (usize f = 0; f < 4; f += 1)
            {
                u32 v = 0;
                u32 n = 0;
                if (f == 0)
                {
                    u32 code = lcodes[s];
                    n = widen<u32>(llens[s]);
                    for (u32 i = 0; i < n; i += 1)
                    {
                        v = (v << 1) | ((code >> i) & 1);
                    }
                }
                else if (s >= 257)
                {
                    if (f == 1)
                    {
                        v = widen<u32>(tokens[k].len_extra);
                        n = widen<u32>(tokens[k].len_extra_bits);
                    }
                    else if (f == 2)
                    {
                        usize d = widen<usize>(tokens[k].dist_code);
                        u32 code = dcodes[d];
                        n = widen<u32>(dlens[d]);
                        for (u32 i = 0; i < n; i += 1)
                        {
                            v = (v << 1) | ((code >> i) & 1);
                        }
                    }
                    else
                    {
                        v = widen<u32>(tokens[k].dist_extra);
                        n = widen<u32>(tokens[k].dist_extra_bits);
                    }
                }
                if (n == 0)
                {
                    continue;
                }
                bits = bits | (widen<u64>(v) << count);
                count += n;
                while (count >= 8)
                {
                    Vec::push(&mut out, narrow_wrapping<u8>(bits));
                    bits = bits >> 8;
                    count -= 8;
                }
            }
        }
        BitOut { .buf = out, .acc = bits, .nacc = count }
    }

    // `deflate`'s matching for one block: tokens for `data` from `start` until
    // there are `MAX_TOKENS` or the data ends. `data` is the function's only
    // reference, read element by element and handed to nothing, so the
    // compiler reads it unchecked; the hash chains and the tokens are vectors
    // passed in and given back by value.
    struct DeflateScan
    {
        Vec<usize> head;
        Vec<usize> prev;
        Vec<Token> tokens;
        usize end;
    }

    fn deflate_scan(slice<u8, shared> data, Vec<usize> head, Vec<usize> prev, Vec<Token> tokens, usize start) : DeflateScan
    {
        usize n = slice_len(data);
        usize i = start;
        while (i < n && Vec::len(&tokens) < MAX_TOKENS)
        {
            usize best_len = 0;
            usize best_dist = 0;
            if (i + 3 <= n)
            {
                usize h = ((widen<usize>(data[i]) << 10) ^ (widen<usize>(data[i + 1]) << 5) ^ widen<usize>(data[i + 2])) & (HASH_SIZE - 1);
                usize cand = head[h];
                usize chain = 0;
                usize limit = if (n - i > 258) { 258 } else { n - i };
                while (cand != max_value<usize>() && i - cand <= WINDOW && chain < CHAIN_LIMIT)
                {
                    if (data[cand + best_len] == data[i + best_len])
                    {
                        usize l = 0;
                        while (l < limit && data[cand + l] == data[i + l])
                        {
                            l += 1;
                        }
                        if (l > best_len)
                        {
                            best_len = l;
                            best_dist = i - cand;
                            if (l == limit)
                            {
                                break;
                            }
                        }
                    }
                    usize next = prev[cand & (WINDOW - 1)];
                    if (next == max_value<usize>() || next >= cand)
                    {
                        break;
                    }
                    cand = next;
                    chain += 1;
                }
                prev[i & (WINDOW - 1)] = head[h];
                head[h] = i;
            }
            if (best_len >= 3)
            {
                usize lc = length_code(best_len);
                usize dc = distance_code(best_dist);
                Vec::push(&mut tokens, Token {
                    .lit = narrow<u16>(257 + lc),
                    .len_extra = narrow<u16>(best_len - widen<usize>(LENGTH_BASE[lc])),
                    .len_extra_bits = LENGTH_EXTRA[lc],
                    .dist_code = narrow<u8>(dc),
                    .dist_extra = narrow<u16>(best_dist - widen<usize>(DIST_BASE[dc])),
                    .dist_extra_bits = DIST_EXTRA[dc],
                });
                // The bytes inside the match enter the hash chains too.
                for (usize k = 1; k < best_len; k += 1)
                {
                    if (i + k + 3 <= n)
                    {
                        usize h = ((widen<usize>(data[i + k]) << 10) ^ (widen<usize>(data[i + k + 1]) << 5) ^ widen<usize>(data[i + k + 2])) & (HASH_SIZE - 1);
                        prev[(i + k) & (WINDOW - 1)] = head[h];
                        head[h] = i + k;
                    }
                }
                i += best_len;
            }
            else
            {
                Vec::push(
                    &mut tokens,
                    Token { .lit = widen<u16>(data[i]), .len_extra = 0, .len_extra_bits = 0, .dist_code = 0, .dist_extra = 0, .dist_extra_bits = 0 },
                );
                i += 1;
            }
        }
        DeflateScan { .head = head, .prev = prev, .tokens = tokens, .end = i }
    }

    // `data` as a raw DEFLATE stream (RFC 1951, no header or checksum).
    export fn deflate(slice<u8, shared> data) : Vec<u8>
    {
        BitWriter w = BitWriter { .out = Vec::new(), .bits = 0, .count = 0 };
        usize n = slice_len(data);
        if (n == 0)
        {
            // One empty fixed block.
            BitWriter::put(&mut w, 1, 1);
            BitWriter::put(&mut w, 1, 2);
            BitWriter::put(&mut w, 0, 7);
            BitWriter::flush(&mut w);
            BitWriter { out, bits, count } = w;
            return out;
        }
        Vec<usize> heads = Vec::filled(HASH_SIZE, max_value<usize>());
        Vec<usize> chains = Vec::filled(WINDOW, max_value<usize>());
        DeflateScan s = DeflateScan { .head = heads, .prev = chains, .tokens = Vec::new(), .end = 0 };
        while (s.end < n)
        {
            usize block_start = s.end;
            DeflateScan { head, prev, tokens, end } = s;
            s = deflate_scan(data, head, prev, tokens, end);
            write_block(&mut w, &s.tokens[0..$], &data[block_start..s.end], s.end >= n);
            Vec::clear(&mut s.tokens);
        }
        BitWriter::flush(&mut w);
        BitWriter { out, bits, count } = w;
        out
    }

    // ---- the wrappers ----

    // Adler-32 (RFC 1950 §8.2), zlib's checksum; `adler32` of the empty
    // slice is 1.
    export fn adler32(slice<u8, shared> data) : u32
    {
        u32 a = 1;
        u32 b = 0;
        usize i = 0;
        usize n = slice_len(data);
        while (i < n)
        {
            // 5552 bytes at a time fit the sums in a `u32` before reducing.
            usize end = if (n - i > 5552) { i + 5552 } else { n };
            while (i < end)
            {
                a += widen<u32>(data[i]);
                b += a;
                i += 1;
            }
            a = a % 65521;
            b = b % 65521;
        }
        (b << 16) | a
    }

    // `data` as a zlib stream (RFC 1950): the two-byte header, the DEFLATE
    // stream, the Adler-32 of `data`.
    export fn zlib_deflate(slice<u8, shared> data) : Vec<u8>
    {
        Vec<u8> out = Vec::new();
        Vec::push(&mut out, 0x78);
        Vec::push(&mut out, 0x9C);
        Vec::append(&mut out, deflate(data));
        Vec::push_be(&mut out, adler32(data));
        out
    }

    // The bytes of a zlib stream, at most `max` (`Oversize` beyond): its
    // header checked (DEFLATE, no preset dictionary) and its Adler-32
    // verified (`Corrupt` at the checksum when it does not match).
    export fn zlib_inflate(slice<u8, shared> data, usize max) : Result<Vec<u8>, CompressError>
    {
        if (slice_len(data) < 6)
        {
            return Err(Truncated);
        }
        u8 cmf = data[0];
        u8 flg = data[1];
        if ((cmf & 0x0F) != 8 || (widen<u32>(cmf) * 256 + widen<u32>(flg)) % 31 != 0 || (flg & 0x20) != 0)
        {
            return Err(Corrupt(0));
        }
        Vec<u8> out = Vec::new();
        usize end = inflate_from(data, 2, &mut out, max)?;
        if (end + 4 > slice_len(data))
        {
            return Err(Truncated);
        }
        if (read_be<u32>(data, end) != adler32(&out[0..$]))
        {
            return Err(Corrupt(end));
        }
        Ok(out)
    }

    // `data` as a gzip file (RFC 1952): a header with no name, time or
    // extra fields, the DEFLATE stream, the CRC-32 and the length of `data`.
    export fn gzip(slice<u8, shared> data) : Vec<u8>
    {
        Vec<u8> out = Vec::new();
        array<u8, 10> header = [0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0, 255];
        Vec::extend_from(&mut out, &header[0..$]);
        Vec::append(&mut out, deflate(data));
        Vec::push_le(&mut out, crc32(data));
        Vec::push_le(&mut out, narrow_wrapping<u32>(slice_len(data)));
        out
    }

    // The bytes of a gzip file, at most `max` (`Oversize` beyond): every
    // member in turn, each header read (its optional fields skipped) and
    // each CRC-32 and length verified (`Corrupt` at the trailer otherwise).
    export fn gunzip(slice<u8, shared> data, usize max) : Result<Vec<u8>, CompressError>
    {
        Vec<u8> out = Vec::new();
        usize at = 0;
        usize n = slice_len(data);
        while (true)
        {
            if (at + 10 > n)
            {
                return Err(Truncated);
            }
            if (data[at] != 0x1F || data[at + 1] != 0x8B || data[at + 2] != 8)
            {
                return Err(Corrupt(at));
            }
            u8 flags = data[at + 3];
            usize p = at + 10;
            if ((flags & 4) != 0)
            {
                if (p + 2 > n)
                {
                    return Err(Truncated);
                }
                p += 2 + widen<usize>(read_le<u16>(data, p));
            }
            if ((flags & 8) != 0)
            {
                while (p < n && data[p] != 0)
                {
                    p += 1;
                }
                p += 1;
            }
            if ((flags & 16) != 0)
            {
                while (p < n && data[p] != 0)
                {
                    p += 1;
                }
                p += 1;
            }
            if ((flags & 2) != 0)
            {
                p += 2;
            }
            if (p > n)
            {
                return Err(Truncated);
            }
            usize before = Vec::len(&out);
            usize end = inflate_from(data, p, &mut out, max)?;
            if (end + 8 > n)
            {
                return Err(Truncated);
            }
            if (read_le<u32>(
                data,
                end,
            ) != crc32(&out[before..$]) || read_le<u32>(data, end + 4) != narrow_wrapping<u32>(Vec::len(&out) - before))
            {
                return Err(Corrupt(end));
            }
            at = end + 8;
            if (at >= n)
            {
                return Ok(out);
            }
        }
    }

**Depends on:** rule.stdlib.vec, rule.stdlib.crypto (crc32), rule.arith.bitwise, D-0183
**Affects:** none (pure)

## Change Log

- 4.53.1 — `CHG-0245` (D-0207), found by `impl/tools/speccheck.py`:
  `[Push-Ascii-Not-Ascii]` is `disposition: checked` (was the
  non-vocabulary `fault`); `[Local-Offset]`'s outcome is
  `unspecified { the platform's offset }` (was prose); two Depends-on
  lines name `rule.fail.fault-unwind` (where `[Fault-Contain]` lives),
  `rule.arith.checked` and `rule.arith.bitwise` instead of entities that
  do not exist. No rule's meaning changed.
- 4.53.0 — `CHG-0240` (D-0206): §0 "What `std` asserts": the trusted
  library base, every `unsafe` block of the normative bodies in a table
  generated from the bodies' `// trusted:` comments
  (`impl/tools/trusted_table.py`) with the rule(s) it instantiates and
  the fact that discharges them. No rule or body changed.
- 4.52.1 — `ChildOutput::read`'s signature in the `std::process` listing laid
  out as `cobfmt` lays it out (one parameter per line). Layout only.
- 4.52.0 — `CHG-0238` (D-0205): `Result::ok`, `slice_eq` (`[Slice-Eq]`),
  `StringView::from_utf8` (`[View-From-Utf8]`), `UdpSocket::try_clone`
  (`[Udp-Try-Clone]`), `Child::kill_tree` (`[Child-Kill-Tree]`); `std::net`'s
  `try_clone` primitive takes a UDP socket too.
- 4.51.1 — `CHG-0235`: `[Http-Keep-Alive]` sends a request again on a fresh
  connection only when its method is idempotent (RFC 9110 §9.2.2); a POST
  whose kept connection closed unanswered is `ConnectionClosed`. `Json::get`
  and `Json::set` take the key as a `StringView` (D-0134's text parameters):
  a key known only at run time could not be asked for or set.
- 4.51.0 — `CHG-0233` (D-0204): prelude `cancel` and `join_status`;
  `std::sync` `try_join`, `is_finished`, `ThreadFailure`; `std::collections`
  `slice_parts` (§3c′, `rule.stdlib.supervision`).
- 4.50.0 — `CHG-0229` (D-0201): `File` writes are buffered, `[File-Buffer]` (§2e): the bytes reach
  the environment by fixed points (the file's next operation, any other file-system operation, a child's start, the
  program's end); `[Flush]` writes them out first.
- 4.49.0 — `CHG-0226` (D-0198): `to_hex`, `from_hex` move from
  `std::text` to `std::encoding::hex`, and `base64_encode`,
  `base64_decode` to `std::encoding::base64`. `std::encoding` holds data
  formats and encodings of bytes as text. §0's listing, tables and text;
  §2d's `[Hex]` and `[Base64]` name their submodules (the rules stay in
  `rule.stdlib.text`). Names and keys unchanged, so `import std;`
  programs are unchanged; a program that imports `std::text` alone and
  uses them adds `import std::encoding;`. No rule or behavior changed.
- 4.48.0 — `CHG-0225` (D-0197): `std::json` is now `std::encoding::json`.
  §0's table and text: `std::encoding`, the data formats, with one
  submodule per format, re-exported by its parent and by `std`; §2r is
  in `std::encoding::json`. `Json` and `JsonMember` keep their names and
  keys, so `import std;` programs are unchanged; `import std::json;`
  becomes `import std::encoding::json;` (or `import std::encoding;`).
  §0's listing also gains `database`'s, `compress`'s lines it lacked.
  No rule or behavior changed.
- 4.47.0 — `CHG-0223` (D-0195): §2k `[Child-Pipes]`: `ChildInput`, `ChildOutput`,
  `Child::take_input`, `Child::take_output`; §2p `[Http-Keep-Alive]` and
  `HttpClient::keep_alive`. Additive: a client without `keep_alive` behaves as
  before.
- 4.46.0 — `CHG-0221` (D-0193): §1 `Vec::swap_remove` (the element at
  `i` exchanged with the last and popped); §2h `StringView::char_count`,
  `String::char_count`; §2p `Response::text` (the body's bytes as
  `String::from_utf8` takes them). Additive.
- 4.45.1 — Non-normative (2026-10-08): §2m's AES is bitsliced (four
  blocks at once, the S-box Boyar and Peralta's circuit) and GHASH
  multiplies by integer multiplications on bits four apart (BearSSL's
  `ctmul64`), both still constant time; AES-128-GCM 449 → 48 ms a MiB.
  No item, rule or result changed. The header's version, left at 4.44.0
  by 4.45.0, is corrected.
- 4.45.0 — `CHG-0215` (D-0187): nested submodules. §0's listing, table
  and text: `std::crypto` divided into `digest`, `kdf`, `aead`, `pk`;
  `std::http` into `client`, `server`; `std::database` into `postgres`,
  each re-exported by its parent, every item keeping its short name and
  key. `std::extensions`, the implementation-defined area `std` declares
  and does not re-export. §2m, §2p, §2q name their submodules. No item,
  rule or behavior changed.
- 4.44.0 — the `std` completeness round (D-0181 to D-0186, 2026-10-07).
  `CHG-0209` (D-0181): the small holes — §2h `[Strip-Prefix]`, `[Lines]`,
  `[Split-Whitespace]`, `[Code-Point]`; §2d `[Parse-Bool]`; §1 `[Retain]`,
  `[Dedup]`; §1b `[Sort-Float]`; §0 `[Clamp]`, `[Is-Nan]`, `[Is-Finite]`,
  `[Gcd]`, `[Rng-Shuffle]`, `[Uuid-V4]`; §3d `asin`, `acos`, `atan`; §2e
  `[Create-New]`, `[Flush]`, `[Position]`; §2e′ `[Current-Exe]`,
  `[Hostname]`; §2k `[Process-Id]`, `[Child-Terminate]`, `read_error_line`;
  §2j `[Iso-Ms]`, `[Http-Date]`, a fraction in `[From-Iso]`; §2l
  `[Keepalive]`, `[Udp-Connect]`; §2m `[Sha1]` and `HashKind::Sha1`; §2o
  `[Tls-Read-Line]`, `[Tls-Peer-Certificate]`; §2p `[Form]`; §2q
  `[Pg-Cell]`. `CHG-0210` (D-0182): §2r `rule.stdlib.json`. `CHG-0211`
  (D-0183): §2s `rule.stdlib.compress`, §2p `[Content-Encoding]`.
  `CHG-0212` (D-0184): §2c `[Args-Flag]`, `[Args-Option]`, `[Args-Finish]`,
  `[Args-Help]`. `CHG-0213` (D-0185): §3c `[Try-Recv]`, `[Recv-Timeout]`,
  §0 `[Cpu-Count]`, §2l `[Try-Clone]`. `CHG-0214` (D-0186): §2o
  `[Tls-Server]`, §2p `[Https-Bind]`, `HttpServer::accept` gives
  `HttpError`. §0's tables; the listings of §2j, §2k, §2l, §2m, §2n, §2o,
  §2p, §2q regenerated. Additive but for `HashKind`'s new variant, the
  fraction `from_iso` now reads, and `accept`'s error type.

- 4.43.0 — `CHG-0208` (D-0180): `std::database`, a PostgreSQL client over
  `std::net` and `std::tls` with SCRAM-SHA-256 (§2q `rule.stdlib.postgres`:
  `[Pg-Url]`, `[Pg-Connect]`, `[Pg-Auth]`, `[Pg-Query]`, `[Pg-Result]`,
  `[Pg-Execute]`, `[Pg-Script]`, `[Pg-Notice]`, `[Pg-Error]`; §0's tables).
  Additive.

- 4.42.0 — `CHG-0206` (D-0178): standard output is buffered (`[Print-Buffer]`,
  §2a): written out before standard error, standard input, a child sharing
  standard output, the program's end, at `flush_stdout()`, and per line on a
  terminal; `flush_stdout` added to `std::io` (§0's tables, §2f). Additive; a
  program watched through a pipe while it runs sees its output later
  unless it calls `flush_stdout`.
- 4.41.0 — `CHG-0205` (D-0177): `read_le`/`read_be` moved from `std::text`
  to `std::collections`, beside `Vec::push_le`/`push_be`; `crc32` moved to
  `std::crypto`, marked as not cryptographic (§0's table, §2g, §2m). No
  item's meaning changed. Additive for every program that imports `std`.
- 4.40.0 — `CHG-0204` (D-0176): `std::sys` divided: files and paths (with
  `File`, `read_file`, `write_file`, `read_bytes`, `write_bytes` and `FileError` from
  `std::io`) in `std::fs`, the arguments and environment in `std::env`, the clocks in
  `std::time`; `std::io` keeps the standard streams; §0's tables and the rule that
  there is no catch-all submodule. No item's meaning changed. Additive for every
  program that imports `std`.
- 4.39.0 — `CHG-0202` (D-0174): `[Http-Accept]`, `[Http-Request]`, `[Http-Respond]`. Additive.
- 4.38.0 — `CHG-0201` (D-0173): §2p `rule.stdlib.http`: `[Url-Parse]`, `[Percent]`, `[Header-Find]`,
  `[Http-Send]`, `[Http-Body]`, `[Http-Redirect]`, `[Http-Error]`; §0's tables; §2o's introduction
  names D-0172's suites. Additive.
- 4.37.0 — `CHG-0200` (D-0172): `[Tls-Client]` offers TLS_AES_256_GCM_SHA384, P-384 and
  Ed25519 and runs the key schedule under the suite's hash. Additive.
- 4.36.0 — `CHG-0199` (D-0171): `[Key-Der]`, `[Key-Pem]`, `[Sign]`; `[Cert-Parse]` reads
  Ed25519; `CertKey::Ed25519Key`, `SignatureScheme::Ed25519`, `EcPrivateKey`, `PrivateKey`;
  §0's tables. Additive.
- 4.35.0 — `CHG-0198` (D-0170): `[Blake2b]`, `[Pbkdf2]`, `[Argon2id]`, `[Password-Hash]`,
  `[Kdf-Parameters]`; `[Crypto-Length]` covers BLAKE2b's lengths. Additive.
- 4.34.0 — `CHG-0197` (D-0169): `[Rsa-Private-Key]`, `[Rsa-Private-Op]`, `[Rsa-Sign-Pkcs1v15]`,
  `[Rsa-Sign-Pss]`, `[Rsa-Generate]`; `[Big-Mod-Inverse]`, `BigUint::clone`; `[Crypto-Length]`
  covers RSA's lengths. Additive.
- 4.33.0 — `CHG-0196` (D-0168): `[Ec-Keys]`, `[Ecdh]`, `[Ecdsa-Sign]`; `[P256-Ecdh]` is their
  P256 case; `[Crypto-Length]` and `[Constant-Time]` cover them. Additive.
- 4.32.0 — `CHG-0195` (D-0167): `[Ed25519-Sign]`, `[Ed25519-Verify]`; `[Crypto-Length]` and
  `[Constant-Time]` cover them. Additive.
- 4.31.0 — `CHG-0194` (D-0166): `[Hasher]`, `[Hmac]`, `[Hkdf-With]`; §2m's introduction no
  longer excludes signing and passwords. Additive.
- 4.30.0 — `CHG-0193` (D-0165): `[Aes]`, `[Aes-Gcm-Seal]`, `[Aes-Gcm-Open]`,
  `[P256-Ecdh]`; `[Crypto-Length]` and `[Constant-Time]` cover them; the P-256/P-384
  field arithmetic reduces by masks; `[Tls-Client]` offers TLS_AES_128_GCM_SHA256 and
  P-256 and answers a HelloRetryRequest; §0's tables. Additive.
- 4.29.1 — `TlsError::text` names the common alerts (40, 42, 70, 80, 109, 112); found
  by round 7's HTTPS tests (a server without ChaCha20-Poly1305). Clarification.
- 4.29.0 — `CHG-0192` (D-0164): `[Text-Mixed-Eq]`: `==` and `!=` between a `String`
  and a `str` or a `StringView`; `String::eq_view`. Additive.
- 4.28.0 — `CHG-0191` (D-0163): `StringView::as_bytes` (`[View-As-Bytes]`). Additive.
- 4.27.0 — `CHG-0190` (D-0162): `[Str-Literal-String]` reaches an argument whose
  parameter a type parameter fixed as `String`. Additive.
- 4.26.0 — `CHG-0189` (D-0161): §2o (new), `rule.stdlib.tls`: a TLS 1.3
  client in a new submodule `std::tls`; `hkdf_expand_label`
  (`[Hkdf-Expand-Label]`). Additive.
- 4.25.0 — `CHG-0188` (D-0160): §2n (new), `rule.stdlib.x509`: DER,
  certificates and chain verification in a new submodule `std::x509`.
  Additive.
- 4.24.0 — `CHG-0187` (D-0159): `EcCurve`, `ecdsa_verify` (`[Ecdsa]`,
  `[Public-Verify]`). Additive.
- 4.23.0 — `CHG-0186` (D-0158): `HashKind`, `hash`, `RsaPublicKey`,
  `rsa_verify_pkcs1v15`, `rsa_verify_pss`. Additive.
- 4.22.0 — `CHG-0185` (D-0157): §3e (new), `rule.stdlib.bigint`: `BigUint`
  and `BigDivision` in `std::math`. Additive.
- 4.21.0 — `CHG-0184` (D-0156): `Sha512`, `Sha384`, `sha512`, `sha384`
  (`[Sha512]`). Additive.
- 4.20.0 — `CHG-0183` (D-0155): `[Str-Literal-String]` and `[Str-Literal-View]`
  reach a literal that a block, an `if` branch or a `match` arm in the
  position gives. Additive.
- 4.19.0 — `CHG-0182` (D-0154): `x25519`, `x25519_public_key`,
  `x25519_private_key` in `std::crypto` (`[X25519]`); `[Constant-Time]`
  covers them. Additive.
- 4.18.0 — `CHG-0181` (D-0153): `chacha20`, `poly1305`,
  `chacha20_poly1305_seal`, `chacha20_poly1305_open` in `std::crypto`
  (`[ChaCha20]`, `[Poly1305]`, `[Aead-Seal]`, `[Aead-Open]`,
  `[Crypto-Length]`, `[Constant-Time]`); `diag.crypto-length`. Additive.
- 4.17.0 — `CHG-0180` (D-0152): `hkdf_extract`, `hkdf_expand`, `hkdf_sha256` in
  `std::crypto` (`[Hkdf]`, §0's tables); `diag.hkdf-length`. Additive.
- 4.16.0 — `CHG-0179` (D-0151): §2m (new), `rule.stdlib.crypto`: `Sha256`,
  `sha256`, `HmacSha256`, `hmac_sha256`, `digest_eq` in a new submodule
  `std::crypto`; `rule.stdlib.text` gains `[Hex]`, `[Base64]` (`to_hex`,
  `from_hex`, `base64_encode`, `base64_decode`); §0's tables. Additive.
- 4.15.0 — `CHG-0178` (D-0150): `exit(u8) : never` in `std::process` (`[Exit]`,
  §0's tables), realized natively; `rule.fn.program` `[Terminate-Exit]`. Additive.
- 4.14.0 — `CHG-0177` (D-0149): `[Str-Literal-String]` (§2a): a `str` literal
  whose declared position is a `String` (a parameter, a local's type, a struct
  literal's field, a function's result) is `String::from_str` of it, as
  `[Str-Literal-View]` types one for `StringView`. Additive.
- 4.13.0 — `CHG-0176` (D-0148): `min`, `max`, `abs`, `pow` move from `std::core` to
  `std::math` (§0's tables). Their meaning is unchanged; `import std;` sees the
  same names. Breaking (source) only for a program writing the path
  `std::core::min` (none checked in).
- 4.12.0 — `CHG-0175` (D-0147): `local_offset_seconds` and
  `DateTime::to_local` in `std::time` (`rule.stdlib.time` `[Local-Offset]`,
  `[To-Local]`; §0's tables; the listings gain `tz_offset`). Additive.
- 4.11.1 — `[Child-Streams]` says a write to a child that has ended is
  `Err(Io)`, never the program's end (a review of D-0143 found the compiled
  program ended by SIGPIPE where `coby` gave `Err(Io)`). Clarification.
- 4.11.0 — `CHG-0173` (D-0145): `read_all` and `read_all_bytes` in `std::io`
  (`rule.stdlib.read` `[Read-All]`; §0's tables). Additive.
- 4.10.0 — `CHG-0172` (D-0144): §2l (new), `rule.stdlib.net`: `IpAddr`,
  `SocketAddr`, `resolve`, `TcpListener`, `TcpStream` (with
  `TcpStream::printf`), `UdpSocket`, `Datagram` and `NetError` in a new
  submodule `std::net` (`[Addr]`, `[Resolve]`, `[Bind]`, `[Accept]`,
  `[Connect]`, `[Stream-Read]`, `[Stream-Write]`, `[Stream-Timeout]`,
  `[Shutdown]`, `[Udp]`, `[Net-Error]`), over the std-private `net_op`;
  §0's tables. Additive.
- 4.9.0 — `CHG-0171` (D-0143): §2k (new), `rule.stdlib.process`: `Command`,
  `Output`, `Child`, `watch_interrupts` and `interrupt_requested` (failures
  are `FileError`s) in a new submodule `std::process` (`[Command]`,
  `[Output]`, `[Status]`, `[Spawn-Child]`, `[Child-Streams]`, `[Wait]`,
  `[Kill]`, `[Interrupt-Flag]`), over the std-private `proc_op`; §0's
  tables. Additive.
- 4.8.0 — `CHG-0170` (D-0142): `os_random_bytes`, `os_random_u64` and
  `Rng::from_os` in `std::random` (`[Os-Random]`, `[Rng-From-Os]`, §0),
  over the std-private `os_random`; `diag.entropy-unavailable`. Additive.
- 4.7.0 — `CHG-0169` (D-0141): paths and file metadata in `std::sys`
  (`rule.stdlib.fs`): `path_join`, `path_parent`, `path_file_name`,
  `path_stem`, `path_extension`, `path_is_absolute`, `path_normalize`,
  `path_canonical`, `FileInfo`, `file_info`, `copy_file`, `make_dir_all`,
  `remove_dir_all`, `set_current_dir`, `temp_dir`, `home_dir`
  (`[Path-Join]` … `[Home-Dir]`), over the std-private `path_op`; §0's
  tables. Additive.
- 4.6.0 — `CHG-0168` (D-0140): §2j (new), `rule.stdlib.time`: `DateTime`,
  `Weekday` and `unix_ms` in a new submodule `std::time` (`[From-Unix]`,
  `[To-Unix]`, `[Invalid-DateTime]`, `[Weekday]`, `[To-Iso]`,
  `[From-Iso]`, `[Unix-Ms]`); §0's submodule table and a `std::time`
  table; `[Clocks]` names `unix_ms`. Additive.
- 4.5.0 — `CHG-0167` (D-0139): `StringView::contains`, `replace`, `join`
  and `String::contains`, `replace`, `join` (`[Contains]`, `[Replace]`,
  `[Join]`, §2h; §0's `std::text` table). Additive.
- 4.4.0 — `CHG-0166` (D-0138): `HashSet::union`, `HashSet::intersection`
  and `HashSet::difference` (`[Set-Ops]`, §2g; §0's `std::collections`
  table). Additive.
- 4.3.0 — `CHG-0165` (D-0137): §2i, `Queue<T>` (`rule.stdlib.queue`)
  and `PriorityQueue<T>` (`rule.stdlib.priority-queue`,
  `[Priority-Queue-Not-Key]`) in `std::collections`; §0's submodule
  table and `std::collections` table list them. Additive.
- 4.2.0 — `CHG-0164` (D-0136): `std` divided into submodules —
  `std::core`, `std::collections`, `std::text`, `std::io`, `std::sys`,
  `std::memory`, `std::sync`, `std::random`, `std::math` — each re-exported
  by `std`'s root, so `import std;` and every `std::` path are unchanged;
  `std`'s submodules share one privacy; each section names its
  submodule. §3d (new): `rule.arith.float-fns`, moved from `spec/06`:
  `sqrt`, `floor`, `ceil`, `round`, `trunc`, `ln`, `exp`, `log2`,
  `log10`, `sin`, `cos`, `tan`, `atan2` and `powf` are `std::math`'s,
  no longer intrinsics.
- 4.1.0 — `CHG-0162` (D-0135): `[View-Form-Temporary-Argument]`: as a
  call's argument, a `String` that is a call's result, or part of one,
  may be viewed (`&f()[0..$]`), as D-0103 lets it be sliced.
- 4.0.0 — `CHG-0157`–`CHG-0161` (D-0134), the alignment of `std`:
  text a function only reads is a `StringView` (paths, `write_file`'s
  text, the needles of `find`/`starts_with`/`ends_with`),
  `StringView::of`, `[Str-Literal-View]`, `File::write_str` →
  `File::write_text`; `Rng::seed` → `Rng::new`, `map_err` →
  `Result::map_err`, `parse` → `String::parse`, `HashSet::at` →
  `HashSet::key_at`, the externs `write`/`read` → `stdout_write`/`stdin_read`
  (and `std`'s private `write_err` → `stderr_write`); `remove` keeps
  order, `swap_remove` is the constant-time removal, `remove_ordered` is
  gone; `ReadError` is gone, `read_line`'s error is `FileError`;
  `HashSet::clear`; §0's table grouped by what it serves, with a
  conventions paragraph. Breaking: every renamed name, and a
  `ref<String, shared>` where a `StringView` is now taken.
- 3.51.0 — `CHG-0152` (D-0129): `Vec::reserve` (§1, with a paragraph on
  its meaning), `String::reserve` (§2); `String::from_str` reserves its
  length (§2), as `String::clone` and `Vec::from_slice` do inside `std`.
- 3.50.0 — `CHG-0151` (D-0128): `StringView::chars`, `String::chars` (§0,
  §2h `rule.stdlib.stringview`, and its listing).
- 3.49.0 — `CHG-0144`–`CHG-0148` (D-0121–D-0125): `read_volatile`/`write_volatile`,
  `checked_narrow`, the bit-counting and rotation intrinsics, `sleep_ms`,
  `Weak<T>` (the `Rc` source: a weak count and an `Option<T>` value).
- 3.48.0 — `CHG-0143` (D-0120): `Rng` (`[Rng]`: xoshiro256** seeded by
  splitmix64, with check values).
- 3.47.0 — `CHG-0142` (D-0119): `Vec::push_le`, `push_be`, `read_le`, `read_be`
  (`[Bytes-LE]`, `[Bytes-BE]`); `crc32` (`[CRC32]`, CRC-32/IEEE written out).
- 3.46.0 — `CHG-0140` (D-0117): `FileError::text`, `ReadError::text`,
  `ParseError::text`, `Utf8Error::text`.
- 3.45.0 — `CHG-0139` (D-0116): `Vec::clone`, `from_slice`, `filled`, `extend_from`
  take `T: clone` and copy by `clone`; `Box::clone`, `HashMap::clone`,
  `HashSet::clone`.
- 3.44.0 — `CHG-0137` (D-0114): `Option::ok_or`.
- 3.43.0 — `CHG-0135` (D-0113): `String::as_view`, `find`, `starts_with`,
  `ends_with`, `trim`, `trim_start`, `trim_end`, `split`; `HashMap::get_str`,
  `get_mut_str`, `contains_str`, `remove_str`, `HashSet::contains_str`,
  `remove_str`; `[Lookup-Str]`.
- 3.42.0 — `CHG-0133` (D-0112): `Vec::filled`, `from_fn`, `append`,
  `extend_from`, `position`, `index_of`, `contains`, `reverse`.
- 3.41.0 — `CHG-0129` (D-0110): a struct or enum of key types is a key
  type (`[Key-Bytes]`, `[Sort]`).
- 3.40.0 — `CHG-0127` (D-0108): `str` is ordered (`[Cmp-Text]`).
- 3.39.0 — `CHG-0120` (D-0101): `ln`, `exp`, `log2`, `log10`, `sin`,
  `cos`, `tan`, `atan2`, `powf`; the ASCII helpers and
  `String::to_ascii_lower`/`upper`.
- 3.38.0 — `CHG-0119` (D-0100): `*` widths and precisions (a `usize`
  argument); `%v` of an enum writes its variant's name.
- 3.37.0 — `CHG-0107` (D-0092): `read_line` — on standard input and on
  a `File` — consumes one `\r` immediately before the `\n` it
  consumes.
- 3.36.0 — `CHG-0106` (D-0091): `sqrt`, `floor`, `ceil`, `round`, `trunc`;
  `min`, `max`, `abs`, `pow`; `HashMap::clear`.
- 3.35.0 — `CHG-0100` (D-0085): `Vec::from_slice`, `Vec::truncate`.
- 3.34.0 — `CHG-0100`: `Rc::clone` and `Rc::drop` borrow the count only,
  not the whole box, so a live `Rc::get` reference into the value does
  not clash with them.

- 3.33.0 — `CHG-0096` (D-0080): `overwrite`. `CHG-0098` (D-0082):
  `Option::unwrap`, `Option::expect`, `Result::unwrap`, `Result::expect`,
  `Vec::clone`, `Vec::clone_by`. `CHG-0099` (D-0083): `fault` takes any
  run-time diagnostic and an optional message; any other name is rejected.
- 3.32.0 — `CHG-0090` (D-0076): `StringView::less`, `String::less`,
  `String::truncate`.

- 3.31.0 — `CHG-0086` (D-0073): `Option::is_some`/`is_none`,
  `Result::is_ok`/`is_err`, `String::eq`/`eq_str`, `Vec::insert`/`remove`.

- 3.30.0 — `CHG-0085` (D-0072): `checked_neg`.
- 3.29.0 — Non-normative: `Vec::sort`'s source compares keys in place
  (`key_less_at`, `Vec::sorted_order_keys`); `[Sort]` unchanged.

- 3.28.0 — `CHG-0075` (D-0065): §0's table: `assert`.

- 3.27.0 — `CHG-0074` (D-0064): `Channel`'s source reaches the guarded
  state as `g.count` (`[Guard-Auto-Deref]`).

- 3.26.0 — `CHG-0073` (D-0063): §3c (new) `rule.stdlib.channel`; §0's
  table; `fault`'s names.

- 3.25.0 — `CHG-0071`: a read or write against a `File`'s open mode is
  exactly `Err(Io)`.

- 3.24.0 — `CHG-0070` (D-0062): §1b (new) `rule.stdlib.sort`; §0's table.

- 3.23.0 — `CHG-0069` (D-0061): §2e′ (new) `rule.stdlib.fs` and
  `rule.stdlib.env`; `PathKind`; §0's table and scope paragraph.

- 3.22.0 — `CHG-0067` (D-0059): `[File-Printf]` and `File::write_text`;
  `[File-Read]` reads `max` bytes, fewer only at the end.

- 3.21.1 — Non-normative (`CHG-0063`, D-0055): the scope paragraph notes
  that an item of an intrinsic's name does not reach `std`'s code.

- 3.21.0 — `CHG-0062` (D-0054): §2e gains `rule.stdlib.file-handle`
  (`File`, `read_bytes`, `write_bytes`); §0's table and scope paragraph.

- 3.20.0 — `CHG-0061` (D-0053): §2h (new) `rule.stdlib.stringview`; §0's table.

- 3.19.0 — `CHG-0060` (D-0052): §0's table gains `static_assert`.

- 3.18.0 — `CHG-0059` (D-0051): §0's table gives `min_value`/`max_value`
  for `f32` and `f64`; the conversions' new forms are `spec/06`'s.

- 3.17.0 — `CHG-0056` (D-0048): `swap`, `replace`, `Vec::swap`,
  `slice_swap` (`rule.stdlib.swap`, §3b).

- 3.16.0 — `CHG-0055` (D-0047): `slice_len` in §0's table. A `Vec` is
  indexed `v[i]` and sliced `&v[lo .. hi]` (`spec/16` §3a).

- 3.15.0 — `CHG-0053` (D-0045): `Box<T>` (`rule.stdlib.box`, §3a).

- 3.14.0 — `CHG-0052` (D-0044): `String::clone`, `String::push_ascii`
  (`[Push-Ascii-Not-Ascii]`) and `String::clear` in §2d; `Vec::clear`
  among §2g's code.

- 3.13.0 — `CHG-0050` (D-0042): §2g gains the holders a consuming
  `foreach` uses (`Drain`, `HashDrain`, `SetDrain`, `Vec::take_raw`, all
  private to `std`); §2f formats a reference to a printable value as
  the value.

- 3.12.0 — `CHG-0049` (D-0041): hash tables. `std` gains `HashMap<K, V>`
  and `HashSet<K>` (`rule.stdlib.hashmap`, §2g), written in CobaltC over
  the std-only intrinsics `key_hash` and `key_eq`, and the private
  `Vec::replace` and `Vec::remove_at`; §0's type list and table list
  them.

- 3.11.0 — `CHG-0048` (D-0040): `eprintf` writes formatted text to
  standard error (`rule.stdlib.format` `[Eprintf]`); §0's table and
  scope paragraph list it.

- 3.10.0 — `CHG-0047` (D-0039): one way to write output. `print` is no
  longer exported (`rule.stdlib.print` now defines the text `printf`,
  `%v` and `String::append` write); `rule.stdlib.format` gains `%v` and
  `sprintf`; §0's table and scope paragraph follow.

- 3.9.0 — `CHG-0046` (D-0038): formatted output. `std` gains `printf`
  and `String::appendf` (`rule.stdlib.format`, §2f); §0's table and
  scope paragraph list them.

- 3.8.0 — `CHG-0043` (D-0034): files. `std` gains `read_file`,
  `write_file` and `FileError` (`rule.stdlib.file`, §2e); §0's type
  list, table and scope paragraph list them.

- 3.7.0 — `CHG-0042` (D-0033): `Option::unwrap_or` and
  `Result::unwrap_or` in §0's table.

- 3.6.0 — `CHG-0041` (D-0032): text conversions. `std` gains
  `String::new`, `String::as_bytes`, `String::append<T>`, `parse<T>`
  and `ParseError` (`rule.stdlib.text`, §2d); §0's type list, table and
  scope paragraph list them.

- 3.5.0 — `CHG-0040` (D-0031): program arguments. `std` gains the
  `extern` `arg_bytes`, `arg_count` and `arg` (`rule.stdlib.args`,
  §2c); §0's table and the scope paragraph list them.

- 3.4.0 — `CHG-0038` (D-0029): standard input. `std` gains the
  `extern` `read`, `read_line` and `ReadError` (`rule.stdlib.read`,
  §2b); §0's type list and table list them.

- 3.3.0 — `CHG-0037` (D-0028): `print` is `print<T>(T x)`, printing
  `str`, integers, floats, `bool` and `ref<String, shared>`
  (`rule.stdlib.print`, §2a), realized natively; its former body is the
  `str` case.

- 3.2.0 — `CHG-0036`: §0 says an intrinsic's name yields to a local
  binding or item of the same name.

- 3.1.0 — `CHG-0035` (D-0026): intrinsics `min_value<T>()` and
  `max_value<T>()` (`rule.arith.limits`).

- 3.0.0 — `CHG-0033` (D-0024): the prelude's declarations form the
  root-level module `std`, reached by `import std;` or `std::…` paths,
  no longer in scope unqualified in every module. `write` is declared
  `export` (programs call it, `CHG-0020`/`CHG-0021`), and so is
  `Utf8Error`'s `offset` field (it reports where validation failed; it
  had been reachable only because the prelude shared the root module).
  The intrinsics and `str` are unchanged and need no import.
- 2.9.1 — Non-normative (owner-directed, 2026-09-23): §2a states
  `print`'s intended scope — the prelude prints `str` only, and output
  of `u8`, `Vec<u8>` and `String` is program-written through `write`.
  No rule changes.
- 2.9.0 — `CHG-0025` (D-0020, human-directed): §2a added — `type.str`
  (a non-resource text value type whose domain is well-formed UTF-8
  byte sequences) and `rule.stdlib.str` (`[Str-Literal]`, `[Str-Len]`,
  `[Str-Byte]`, `[Str-Byte-Out-Of-Bounds]`, `[Str-Ptr]`, and the
  ordinary function `print`). §0's table gains the three `str_*`
  intrinsics. §2 gains `String::from_str`, a second `String`
  constructor that carries `inv.str.utf8-validity` over into
  `inv.string.utf8-validity` without a validator; the prose no longer
  calls `from_utf8` "the only constructor".
- 2.8.0 — `CHG-0020`: `write(rawptr<u8> buf, usize len) : isize` added
  to §0's prelude as a named `extern fn` instance, so a conformance
  case's outcome can in principle be observed by a real running
  program (`feat.minimal-io-extern-surface`, now `ACCEPTED`). No new
  rule — `rule.trust.extern-call`'s `[Extern-Call]` already governs
  any `FfiType` signature; `isize`, `usize`, and `rawptr<u8>` are all
  in `FfiType` (`spec/20` §3).
- 2.7.0 — `CHG-0019`: `Vec::pop` additionally `release`s the popped
  slot after reading/moving its value out. Under 2.6.0, a plain-typed
  element's reclaimed object (from an earlier `index_shared`) survived
  a `pop` untouched — reading owns nothing, correctly — so a
  subsequent `push` reusing the same slot could silently overwrite the
  value a live shared reference into it observed, with no diagnostic
  (`spec/AUDIT-STATUS.md` finding F-05). A resource `T`'s `pop` is
  unaffected (`[Rawptr-Move-Out]` already ends the reclaimed object;
  the added `release` finds nothing there). `rule.trust.rawptr`
  `[Rawptr-Write]`/`[Rawptr-Read]` tightened to match (no rule any
  longer needs the `reclaimed-at(addrs)` exemption they previously
  carried).
- 2.6.0 — `CHG-0009`: `Rc::drop` restructured so the exclusive borrow
  of the box (`b`) ends before `drop(reclaim<RcBox<T>>(self.ptr))`
  runs; the 2.5.0 body destroyed the box inside the block that still
  held `b`, which `[Destroy-Not-Solitary]` faults
  (`diag.destroy-while-aliased`) on every last drop —
  `conf.rc-last-drop-frees` did not derive. `fault(d)` declared
  `never`-typed in the §0 table (its match-arm and statement uses
  already required it). Observable behavior of every prelude item is
  otherwise unchanged.
- 2.5.0 — Reformatted every CobaltC code sample to Allman brace style
  (human-directed documentation convention, `spec/22` §1; non-
  normative per `spec/02-schema.md` §6, no `CHG-XXXX` record — no
  token sequence's meaning changed, only line breaks and indentation).
  The `[Allocate]`/`[Deallocate]` CFN blocks are metalanguage notation,
  not CobaltC surface syntax, and are unchanged; `map_err`'s inline
  `match` stays single-line since it sits inside a table cell (the
  same structural exemption documented for `spec/conformance.md`).
- 2.4.0 — `CHG-0008`: `spawn` and `join` added to the §0 intrinsics
  table, now that `spec/22` 2.6.0 demotes them from dedicated grammar.
  `rule.conc.spawn`/`rule.conc.join` added to this section's Depends
  on. No behavior changed — this documents where their existing rules
  now live in the classification, nothing more.
- 2.3.0 — `CHG-0006`: every `pub` re-spelled `export` (21 sites: the
  prelude types, `Vec`, `String`, `Rc`, and their public functions),
  matching `spec/22` 2.5.0. No visibility changed — every item that
  was `pub` is now `export` and nothing else moved.
- 2.2.0 — `CHG-0005`: every `match` in `map_err`, `Vec::grow`, and
  `Rc::new` re-spelled with `:` instead of `=>`. No behavior changed.
- 2.1.0 — `CHG-0003`: every `if`/`while`/`match` in `Vec`, `String`,
  `Rc`, and `map_err` re-spelled with mandatory condition parentheses
  (`spec/22` 2.2.0). No behavior changed.
- 2.0.0 — Re-spelled to `spec/22` 2.0.0's C-style declarator syntax
  (`CHG-0001`): every function/associated-function/destructor
  signature flipped to type-first parameters with `:`-introduced
  return types; every struct declaration flipped to type-first,
  semicolon-terminated fields; every struct-literal construction
  (`Vec`, `String`, `Utf8Error`, `RcBox`, `Rc`) changed to `.f = e`
  form; every local declaration flipped to type-first or `auto`. The
  `[Allocate]`/`[Deallocate]` CFN rule blocks are metalanguage
  notation, not CobaltC surface syntax, and are unchanged. No rule's
  behavior, and no conformance case in `spec/conformance.md`, changed.
- 1.0.0 — Rewritten (`spec/AUDIT-2.md` B-05, B-07, B-11, B-20): §0
  prelude declares every intrinsic and prelude type previously used
  without declaration; `Vec`, `String`, `Rc` written in the surface
  language against the 1.0.0 rules (destructors, `reclaim`
  re-attachment, `Rawptr-Move-In/Out`); `from_utf8` given an actual
  validator; `fault`, `reinterpret_ptr` added. All entities
  `ACCEPTED`.
- 0.6.0 and earlier — superseded.
