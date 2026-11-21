# CHG-0322 — `Arc<T>` and `ArcWeak<T>`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-17)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0259
Affects: `spec/19` §2 (1.20.0); `spec/21` §0 (the `clone` table), §3′ (4.68.0); `spec/conformance.md` (four
rows); `impl/std/memory.cb`

## What changed

`std::memory` gains `Arc<T>` and `ArcWeak<T>` (`rule.stdlib.arc`): `Rc` and `Weak` with atomic counts, whose
handles are cloned into threads, moved and dropped there; the value ends once, on whichever thread drops the
last strong handle, and the box is freed by the last handle of either kind. `spec/19` §2 says how a thread
outlives the frame of what it shares: through an `Arc`. Rows `conf.arc-shared-by-threads`,
`conf.arc-last-drop-frees`, `conf.arc-weak`, `conf.arc-thread-outlives-frame`.

## What changed for programs

A new type; nothing that was accepted changes.

## Found by

D-0259's investigation of what the new atomics make possible in `std` (2026-10-17).
