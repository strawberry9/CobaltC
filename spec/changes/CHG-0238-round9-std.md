# CHG-0238 — `Result::ok`, `slice_eq`, `StringView::from_utf8`, `UdpSocket::try_clone`, `Child::kill_tree`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; D-0205 (2)-(6))
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0205
Affects: `spec/21` 4.52.0, `spec/conformance.md` 3.199.0; `impl/std/{core,collections,text,net,process}.cb`,
`impl/src/netio.rs` (`TRY_CLONE` of a UDP socket), `impl/src/procio.rs` (`KILL_TREE`), `impl/tests/common/mod.rs`
(`platform: linux`)

## What changed

- The five functions, with their rows and listings in `spec/21`.
- Conformance: `conf.result-ok`, `conf.slice-eq`, `conf.view-from-utf8`, `conf.udp-try-clone`,
  `conf.child-kill-tree` (`platform: linux`, a header value the runners now accept).

## What did not change

Every existing function keeps its meaning; `Child::kill` still signals the child alone.
