# CHG-0165 — `Queue<T>` and `PriorityQueue<T>`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-04; D-0137)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0137
Affects: `spec/21` (4.3.0) §0, §2i (new); `spec/conformance.md` (3.127.0); the guide §21; `impl/std/collections.cb`, `impl/src/typecheck.rs`, `impl/cobc/src/lower.rs`

## What changed

- **`spec/21` §2i (new):** `rule.stdlib.queue` (`[Queue]`) and `rule.stdlib.priority-queue` (`[Priority-Queue]`, `[Priority-Queue-Not-Key]`), with their CobaltC bodies.
- **`spec/21` §0:** the submodule table lists `Queue` and `PriorityQueue` under `std::collections`; the `std::collections` table gains their row; the scope paragraph lists them among the library types.
- **`std` (`impl/std/collections.cb`):** `Queue` and `PriorityQueue`, written in CobaltC; both tools run them as written.
- **Front end (`impl/src/typecheck.rs`):** `[Priority-Queue-Not-Key]` at `PriorityQueue::new`'s call, as `[Sort-Not-Key]` at `Vec::sort`'s.
- **`cobc` (`impl/cobc/src/lower.rs`):** `key_less_at` of an element type with no key order, on a path `std` does not take for it, lowers to `diag.type-mismatch` at run time rather than failing to compile; `Queue::front` and `back` (element types without references or `fn` values) and `Queue::pop_front` and `pop_back` (plain element types) are realized natively, as `HashMap::get` and `Vec::pop` are.
- **Rows:** `conf.queue-fifo`, `conf.queue-empty`, `conf.queue-both-ends`, `conf.queue-destroy`, `conf.queue-front-borrows`, `conf.priority-queue-key-order`, `conf.priority-queue-by`, `conf.priority-queue-by-any-type`, `conf.priority-queue-ties`, `conf.priority-queue-destroy`, `conf.priority-queue-not-key`, `conf.priority-queue-generic-key-checked`.

## Compatibility classification

Additive: two new names in `std`, shadowed by a program's own.
