# CHG-0172 — `std::net`: TCP, UDP, name resolution

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0144)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0144
Affects: `spec/21` (4.10.0) §0, §2l (new); `spec/conformance.md` (3.134.0); the guide §21; `impl/std/net.cb` (new), `impl/std/std.cb`, `impl/src/netio.rs` (new), `impl/src/prelude.rs`, `impl/src/modres.rs`

## What changed

- **`spec/21` §2l (new):** `rule.stdlib.net` with `[Addr]`, `[Resolve]`, `[Bind]`, `[Accept]`, `[Connect]`, `[Stream-Read]`, `[Stream-Write]`, `[Stream-Timeout]`, `[Shutdown]`, `[Udp]`, `[Net-Error]` and the CobaltC source.
- **`spec/21` §0:** the submodule table and listing gain `std::net`; a `std::net` table, `TcpStream::printf` among the natively realized functions.
- **`std` (`impl/std/net.cb`, new):** the addresses, `resolve`, `TcpListener`, `TcpStream`, `UdpSocket`, `Datagram`, `NetError`, written in CobaltC over the std-private `net_op`.
- **Front end (`impl/src/modres.rs`):** `TcpStream::printf` entered in `std::net` and rewritten to `TcpStream::write_formatted(s, $fmt(…))`, as `File::printf` is.
- **Both tools:** `net_op` in `impl/src/netio.rs`; routed as D-0141's `path_op` is (`cb_net_op` in `cbrt`).
- **Rows:** `conf.net-bind-port`, `conf.net-echo`, `conf.net-read-exact`, `conf.net-shutdown-read-zero`, `conf.net-addr-in-use`, `conf.net-accept-timeout`, `conf.net-refused`, `conf.net-addr-parse`, `conf.net-addr-reject`, `conf.net-addr-constants`, `conf.net-udp`, `conf.net-resolve-localhost`, `conf.std-variant-names-unique` (no two `std` enums share a variant name).

## Compatibility classification

Additive: a new submodule `std::net` and its names, shadowed by a program's own.
