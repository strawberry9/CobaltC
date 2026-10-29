# CHG-0298 — The model: closures, slices, handles in containers

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; D-0246)
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0246, D-0242–D-0245
Affects: `impl/model` (`ast.rs`, `parse.rs`, `print.rs`, `statics.rs`, `machine.rs`, `enumerate.rs`, README,
`sample/`)

## What changed

- The fragment: `fn() : τ`, closure literals (`move` or not), `c()`, `(*f)()`, `spawn(c)` and
  `spawn(move [..]() {..})`; `slice<i32, m>`, `&_m v[lo .. hi]` of a `Vec` or a slice, `s[k]`;
  `Vec<handle<void>>`, `struct Job { handle<void> h; }`; helpers `call_val`, `call_shared`, `bump_slice`,
  `first`.
- The machine: path steps `F`/`R`/`I` compared by `meet`; `[Index-Vec]`'s element borrow (D-0238) for `v[k]`;
  D-0242's one-access paths; destruction that waits for each held handle in order.
- The static half: capture modes, the closure row (D-0243), closure bodies analysed as bodies of their own,
  slice places with `range` steps, the consumption row (D-0244), closure calls (D-0245).
- The alphabet; hand-written sample programs for each finding; the sample's `hand-r*` headers corrected
  (their `// MODEL:` line numbers were off by the header's length).
- Later rounds: `while` loops (fixed point at the head), `array<i32, 2>` with literal-index places and slices
  of arrays, closures owning a resource (read moves them, an empty one faults when called; `call_twice`),
  guards and slices captured by closures and moved into threads; a lend skips a lock path (D-0240).

## What changed for programs

Nothing: evidence. The findings it led to are CHG-0293–CHG-0297, CHG-0299–CHG-0304.
