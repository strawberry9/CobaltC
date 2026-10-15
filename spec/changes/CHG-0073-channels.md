# CHG-0073 — Channels

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-27, owner-chosen)
Governed by: `CobaltC_Master_Instructions.md` §1, §17, §19, §21
Depends on: D-0063, CHG-0072
Affects: rule.conc.channel (new), rule.stdlib.channel (new),
rule.stdlib.prelude

## Problem / motivation

D-0063: threads had no way to wait for a value another thread
produces.

## Decision

D-0063: `Channel<T>` in `std` — `Channel::new(cap)`, `send`, `recv`,
`close`; bounded; closed explicitly; blocking only.

## What changed

- **`spec/19` 1.7.0:** §3 (new) `rule.conc.channel` (`[Channel-New]`,
  `[Channel-Zero-Capacity]`, `[Send]`, `[Recv]`, `[Close]`,
  `[Channel-Destroy]`, `[Channel-Deadlock]`).
- **`spec/21` 3.26.0:** §3c (new) `rule.stdlib.channel` (`[Event-Op]`,
  the `std` source); §0's table; `fault`'s names.
- **`spec/registry/diagnostics.md` 1.27.0:** `diag.channel-zero-capacity`,
  `diag.channel-deadlock`.
- **`spec/conformance.md` 3.55.0:** the cases below.
- **`spec/02-schema.md` 1.0.46:** §5's "in use" ranges.
- **Implementations:** `std` gains `Channel`, `ChannelState` and the
  primitive `event_op`: in `coby`, counts in `Shared` beside the lock
  table, a wait through `block_until`; in `cbrt`, `cb_event_op` over
  `wait_until`/`signal`; `cobc` lowers `std::event_op` to it. Found on
  the way and fixed in `coby`: the lock table was keyed by object, so
  two `mutex` fields of one struct were one lock (a false
  `diag.mutex-reentrant-lock`, and needless serialization); and an
  object whose type was replaced by a binding's written type
  (`Option<Noisy> x = None;`) kept the resource flag of its first type,
  so a resource later stored in it was never destroyed. The guide
  (§19); `showcase/tier3/pipeline.cb`.

## Affected entities

`rule.conc.channel` (new); `rule.stdlib.channel` (new);
`rule.stdlib.prelude`.

## Compatibility classification

Extension.

## Conformance changes

**Added:** `conf.channel-close`, `conf.channel-destroy`,
`conf.channel-threads`, `conf.channel-fifo-bounded`,
`conf.channel-zero-capacity`, `conf.channel-deadlock`,
`conf.resource-in-none-binding-destroyed`.

## Prior-art status

See D-0063.

## Revisit conditions

See D-0063.
