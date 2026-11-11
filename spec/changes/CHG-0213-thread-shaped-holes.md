# CHG-0213 — `try_recv`, `recv_timeout_ms`, `cpu_count`, `TcpStream::try_clone`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-07; D-0185)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0185
Affects: `spec/21` (4.44.0) §0, §2l, §3c; `spec/conformance.md` (3.174.0); the guide; `impl/std/sync.cb`, `impl/std/net.cb`, `src/procio.rs`, `src/netio.rs`

## What changed

- **`spec/21` §3c `rule.stdlib.channel`:** `[Try-Recv]`, `[Recv-Timeout]`.
- **`spec/21` §0:** `cpu_count` under `std::sync`.
- **`spec/21` §2l `rule.stdlib.net`:** `[Try-Clone]`.
- **Rows:** `conf.channel-try-recv`, `conf.channel-recv-timeout`, `conf.cpu-count`, `conf.tcp-try-clone`.

## Compatibility classification

Additive.
