# D-0185 — The thread-shaped holes: `try_recv`, `recv_timeout_ms`, `cpu_count`, `TcpStream::try_clone`

Status: ACCEPTED (2026-10-07, the owner: "implement C1, C2, C3, C4, C5 and C6", on the completeness assessment of the same day)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0063 (channels), D-0124 (`sleep_ns`), D-0143 (`proc_op`, interrupts), D-0144 (sockets)
Affects: `spec/21` §0, §2l, §3c (4.44.0), `spec/conformance.md` (3.174.0), the guide §19, §21, `impl/std/sync.cb`, `impl/std/net.cb`, `src/procio.rs`, `src/netio.rs`; `CHG-0213`

## Problem

A server written with `spawn` and a `Channel` meets three walls. A
thread that must also watch something else (a socket with a timeout,
`interrupt_requested`) cannot block in `recv`. A worker pool has no way
to ask how many workers the machine can run. And a `TcpStream` is one
`ref<_, exclusive>`, so a relay, a chat server or a pipelined client
cannot read it in one thread while another writes it.

## Candidate mechanisms

1. **`Channel::try_recv`, `Channel::recv_timeout_ms`, `cpu_count`, and
   `TcpStream::try_clone` (a second handle on the same connection).**
   Selected.
2. **`select` over several channels.** A larger design (a wait on many
   counts); the polling forms serve the loops that need it. Deferred.
3. **`TcpStream::split` into a reader and a writer half (two types).**
   Two new types for what one existing one does; `try_clone` is Rust's
   name and shape, and each clone has its own timeouts and read-ahead.
   Not adopted.
4. **A thread pool in `std`.** Composes from `spawn`, a `Channel` and
   `cpu_count` in a dozen lines; the owner put it out of scope
   (2026-10-04). Not adopted.

## Selected design

- `[Try-Recv]` (`spec/21` §3c): `Channel::try_recv(ch) :
  Result<Option<T>, void>` never waits: `Ok(Some(v))` the value at the
  front, `Ok(None)` the channel is empty, `Err(())` it is closed and
  empty. The three answers are distinct because "empty now" and "the
  senders are done" lead a loop to different places, and a bare
  `Option` could not tell them apart. `[Recv-Timeout]`
  `Channel::recv_timeout_ms(ch, ms)`: the same answers, waiting at most
  `ms` milliseconds for a value; the wait polls (every millisecond at
  first, then every five, as `accept`'s timed wait does), so it ends at
  most a few milliseconds late. No new primitive: the event count
  (`event_op`) has no timed wait, and a polled one needs none.
- `[Cpu-Count]` (§0): `cpu_count()`, the threads the machine runs at
  once (`available_parallelism`), at least 1, through `proc_op`.
- `[Try-Clone]` (§2l): `TcpStream::try_clone(s)`, a second `TcpStream`
  on the same connection (the system's `dup`): each has its own
  read-ahead and timeouts, bytes read through one are not seen by the
  other, `shutdown_write` through either ends sending for both, and the
  connection closes when the last handle is dropped. A `TlsStream` has
  no clone: its record keys are one state.

## Compatibility impact

Additive. No new tokens.

## Testing

`conf.channel-try-recv`, `conf.channel-recv-timeout`, `conf.cpu-count`
(`channel_try_recv_timeout_cpu_ok.cb`); `conf.tcp-try-clone`
(`net_try_clone_keepalive_udp_connect_ok.cb`): a clone read in one
thread while the original writes in another, a clone dropped with the
connection kept.

## Revisit conditions

- `select`, if a program must wait on several channels at once.
- A timed wait in the event primitive, if the poll's few milliseconds
  matter.
