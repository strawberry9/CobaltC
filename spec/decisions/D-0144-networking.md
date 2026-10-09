# D-0144 — `std::net`: TCP, UDP and name resolution

Status: ACCEPTED (2026-10-04, the owner, through `private/std-deployable-plan.md` §7 and §10.6: "Sockets are blocking with timeouts; `TcpStream` mirrors `File`'s reading and writing API including `printf`; UDP returns a `Datagram` struct because there are no tuples")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0054 (`File`), D-0059 (`File::printf`), D-0063 (channels), D-0117 (`text` of an error), D-0134 (`std`'s naming conventions), D-0136 (`std`'s submodules), D-0141 (the shared primitive shape), D-0143 (the interrupt flag)
Affects: `spec/21` §0, §2l (new) (4.10.0), `spec/conformance.md` (3.134.0), the guide §21, `impl/std/net.cb` (new), `impl/std/std.cb`, `impl/src/netio.rs` (new), `impl/src/prelude.rs`, `impl/src/modres.rs`

## Problem

A deployed program talks to other programs over the network: it serves
requests, calls services, sends metrics. `std` could not open a socket,
so CobaltC programs could not be servers or clients of anything.

## Candidate mechanisms

1. **Non-blocking sockets and an event loop** (`poll`/`epoll`, futures).
   CobaltC has no async functions, and an event loop written in CobaltC
   would make every program a state machine. Out of scope (the plan's
   list).
2. **Blocking sockets with per-socket timeouts, a thread per
   connection.** The one model the language's threads support well:
   `spawn` with a `move` closure (or a function and the moved stream)
   per connection, channels to coordinate (D-0063). Selected.
3. **Addresses as strings** (`"127.0.0.1:80"` everywhere). Every call
   would parse, and an error in the text would surface at `connect`.
   Addresses are values (`IpAddr`, `SocketAddr`) with `parse` and `text`.

## Selected design

`spec/21` §2l, `rule.stdlib.net`:

- `IpAddr` (`V4(array<u8, 4>)`, `V6(array<u8, 16>)`), `SocketAddr {
  ip, port }`, `parse`/`text` for both, the loopback and unspecified
  constants, `resolve(host, port)` (the system's resolver; `localhost`
  works offline).
- `TcpListener` (`bind`, port 0 for a free port, `local_addr`,
  `set_accept_timeout_ms`, `accept`), `TcpStream` (`connect`,
  `connect_timeout`, `peer_addr`, `local_addr`, read and write timeouts,
  `set_nodelay`, `read`, `read_exact`, `read_line`, `write`,
  `write_text`, `printf`, `shutdown_write`), `UdpSocket` (`bind`,
  `local_addr`, `set_read_timeout_ms`, `send_to`, `recv_from` giving a
  `Datagram { data, from }` — there are no tuples), all three `resource
  struct`s closed when dropped. `NetError` with `text`.
- **`NetError`'s variants share no name with `FileError`'s:**
  `Refused`, `Reset`, `TimedOut`, `Unreachable`, `AddrInUse`,
  `AddrNotAvailable`, `UnknownHost` (a name that resolves to nothing),
  `NotPermitted`, `Closed`, `Failed` (any other), `NotUtf8(Utf8Error)`.
  The plan's `NotFound`, `Denied`, `Io` and `Utf8` would have made every
  unqualified `Err(NotFound)` written as a value in a program importing
  `std` ambiguous (`[Resolve-Ambiguous]`: two imported enums with the
  variant), which the battery's showcase run found in the first version.
  Network failures are not `FileError`s (D-0134's convention) because
  most of them (refused, reset, timed out) have no `FileError`.
- **`TcpStream` reads and writes as `File` does**: `read` appends up to
  `max` (at most a MiB) and gives 0 only at the end; `read_line` drops
  `\n` and a `\r` before it, through a read-ahead buffer; `printf` is
  `File::printf`'s mechanism (`modres` rewrites it to
  `TcpStream::write_formatted(s, $fmt(…))`).
- **A timeout is `Err(TimedOut)` and the socket stays usable.** An
  accept timeout lets a server loop look at `interrupt_requested()`
  (D-0143); while interrupts are watched, an `accept` without a timeout
  polls too, and gives `Err(TimedOut)` once an interrupt has arrived, and
  a `read` the interrupt arrives in gives `Err(TimedOut)` rather than
  resuming.
- **Parsing addresses uses the platform library's grammar** (Rust's,
  in both tools): `SocketAddr::parse` splits the port off in CobaltC (so
  a bad port is located, and 70000 is `OutOfRange`) and hands the
  address text to the library; an address the library refuses is
  `Invalid(i)` at its first byte. IPv6 in a `SocketAddr` must be
  bracketed.
- **Realization:** `impl/src/netio.rs`, shared by both tools, over
  Rust's `std::net` (so Windows needs only `ws2_32`, already linked).
  Listeners, streams and UDP sockets share one process-wide handle table;
  each entry is locked on its own and the table only to find it, so a
  thread blocked in `accept` holds no other thread up. `net_op` has
  D-0141's shape; an address crosses as a 19-byte record (family, 16
  bytes, port). `coby` releases its interpreter lock for every `net_op`,
  so a server and its clients run as threads of one interpreted program
  (the conformance cases do exactly that; any path that forgot would
  deadlock them).
- **Conformance is hermetic:** loopback, port 0, server and client in one
  program. A failing resolution is not a case (offline it may take long);
  `localhost` is the one name resolved.

Not adopted: TLS (needs a cryptographic library in both tools; a later
decision), non-blocking mode, `peek`, socket options beyond `nodelay`,
`TcpStream::try_clone` (one stream read in one thread and written in
another; the ownership model needs a design for it), Unix domain sockets,
multicast.

## Compatibility impact

Additive: a new submodule `std::net` and the names `IpAddr`,
`SocketAddr`, `resolve`, `TcpListener`, `TcpStream`, `UdpSocket`,
`Datagram`, `NetError` in `std`, shadowed by a program's own items of
those names. `V4`, `V6` and the `NetError` variants are new variant
names; none is the name of another `std` enum's variant (case
`std_variant_names_unique_ok.cb`), and a program's own enum with one of
them wins over `std`'s.

## Revisit conditions

- TLS, when a portable implementation can be shared by both tools.
- Reading and writing one stream from two threads (`try_clone` or a
  split into halves), if servers measured to need it.
- More socket options (keep-alive, buffer sizes, `SO_REUSEADDR` control)
  and Unix domain sockets, if programs ask.
