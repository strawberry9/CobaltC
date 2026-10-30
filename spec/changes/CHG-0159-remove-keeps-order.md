# CHG-0159 — `remove` keeps order; `swap_remove` is the constant-time one

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0134)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0134
Affects: `spec/21` §0, §2g (`rule.stdlib.hashmap`, `[Lookup-Str]`); `spec/conformance.md`; the guide; `impl/src/prelude.rs`

## What changed

- **Previous semantics:** `HashMap::remove` / `HashSet::remove` moved the last entry into the removed one's place (constant time); `HashMap::remove_ordered` / `HashSet::remove_ordered` kept the others' order (linear); `remove_str` reordered as `remove` did.
- **New semantics:** `HashMap::remove`, `HashSet::remove` and `remove_str` keep the others' order (linear); `HashMap::swap_remove` and `HashSet::swap_remove` move the last entry into the hole (constant time). `remove_ordered` is gone.
- **Rows:** `conf.hashmap-remove-swaps` is now `conf.hashmap-swap-remove`; `conf.hashmap-remove-ordered` is now `conf.hashmap-remove-keeps-order`; `conf.map-remove-str` keeps order.

## Compatibility classification

Breaking: `remove_ordered` is `diag.unbound-name` (rename to `remove`). Silent for a program calling `remove` or `remove_str`: its later iteration order can differ — the others now keep theirs. Migration that preserves behaviour exactly: `remove` → `swap_remove`, then `remove_ordered` → `remove`.
