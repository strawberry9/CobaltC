# CHG-0277 — The std reference round's questions, decided and carried out

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-14)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0232, CHG-0276
Affects: `spec/21` (4.61.0), `spec/conformance.md` (3.223.0), `spec/02-schema.md` (1.0.83);
`impl/std/http/client.cb`, `encoding/base64.cb`, `env.cb`, `sync.cb`, `tls.cb`; `impl/src/typecheck.rs`;
`impl/cobc/src/lower/intrinsics.rs`; seven cases in `impl/conformance`; `std_error_variants_text_ok.cb`'s
expected output

## What changed

D-0232's seven decisions, each with a case that fails on the tools before this change:

| Decision | Where | Case |
|---|---|---|
| Credentials dropped once a redirect leaves the origin | `http/client.cb` (`is_credential`, `send`), `[Http-Redirect]` | `conf.http-redirect-credentials` |
| Canonical base64 | `encoding/base64.cb`, `[Base64]` | `conf.base64-canonical-padding` |
| A taken item is no option's value | `env.cb` (`Args::options`), `[Args-Option]` | a `std` test |
| A very long wait saturates | `sync.cb` (`Channel::recv_timeout_ms`), `[Recv-Timeout]` | `conf.channel-recv-timeout-long` |
| `ResultTooLarge` kept for too many parameters | no change; the documentation names both causes | — |
| TLS alert texts name the peer | `tls.cb` (`TlsError::text`) | `conf.std-error-variants-text` (its output) |
| `spawn`'s arguments typed as a call's | `typecheck.rs` (the `spawn` intrinsic, `check_arg_exact_types`) | `conf.spawn-argument-type`, `conf.spawn-argument-count` |
| A value is no reference argument | `typecheck.rs` (`check_arg_exact_types`, `plainly_a_value`) | `conf.value-for-reference-parameter` |
| `cobc` infers a spawned generic from a constructor | `lower/intrinsics.rs` (`spawn_probe_safe`) | `conf.spawn-generic-from-constructor` |

`spec/21`'s transcriptions of `Args::options`, `base64_decode_bytes`, `Channel::recv_timeout_ms`, the
HTTP client's `request_bytes`, `exchange`, `exchange_on`, `send` and `is_credential`, and `TlsError::text`
are brought up to the source.

## What changed for programs

As D-0232 says: `spawn` with arguments of the wrong type or count, and a value where a reference is taken,
are now rejected statically. A cross-origin redirect no longer carries credentials. `base64_decode`
refuses non-canonical padding. A huge channel timeout waits. The TLS alert texts are worded for either
side.
