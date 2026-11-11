# D-0298 — A `Box` is one pointer, and `Option<Box<T>>` is one pointer too

Status: ACCEPTED (2026-10-31; the owner: "the aim needs to be to meet Rust on its own ground with idiomatic
code"). Implemented: step 1 as `CHG-0394`, step 2 as `CHG-0395`.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0045 (`Box`), D-0250 (struct layout is C's), D-0206 (the trusted library base), D-0278
Affects: `impl/std/memory.cb`; `spec/21` §3a and its trusted-claims table; `spec/16` `[Layout-Enum]`; `spec/06`
`[Sizeof-Enum]`, `[Repr-Enum]`; `impl/cobc/src/lower/layout.rs` and every enum discriminant the generated C
reads; `impl/cbrt/src/lib.rs` (`box_drop`, `tree_*`, `owns`, the enum arms of the descriptor walks);
`impl/src` (`sizeof`, the interpreter's Box destruction); `spec/conformance.md`; `impl/selfhost` in the same Update

## Problem

`bench/tree`'s node `{ i64 key; Option<Box<Node>> left; Option<Box<Node>> right; }` was 56 bytes under both
tools; Rust's is 24. A `Box<T>` was `{ rawptr<T> ptr; bool full; }`, 16 bytes, and `Option<Box<T>>` put a
4-byte discriminant at offset 0 (`[Layout-Enum]`, DW = 4) and the payload at 8: 24 bytes. Rust's `Option<Box<T>>`
is 8: its `None` is the null pointer.

Measured on 2026-10-31 (`perf`, 15 runs), Rust's `tree.rs` with its node padded to each size, and with a real call
per level as `[Call-Stack-Exhausted]` requires of CobaltC (`conf.tail-recursion-stack-exhausted`):

| node | Rust, loop | Rust, a call per level |
|---|---|---|
| 24 B | 59.1 M cycles | 73.0 M |
| 40 B | 70.1 M | — |
| 56 B | 78.2 M | 95.8 M |

After `CHG-0393` `cobc` stood at 114.1 M cycles, so the node's size accounted for about 23 M of the 53 M cycles
left to Rust; the calls staying calls for about 14 M; the compiler's own checks for the rest. Every Box-linked
structure (lists, trees, `Json`, the self-hosted compiler's own trees) pays the same in cache footprint, and no
compiler work can take it back: it is set by the layout rules.

## Decision

1. **A `Box<T>` is one pointer.** `Box::new` never gives the null pointer (a zero-sized `T` still gets one
   byte), so null marks a `Box` whose value `into_inner` took: `into_inner` moves the value out, releases and
   deallocates the cells and nulls the pointer; `drop` does nothing for a null pointer and otherwise reclaims,
   destroys and deallocates as before. `sizeof<Box<T>>()` is `sizeof<rawptr<T>>()`; `Option<Box<T>>` is 16
   bytes under DW = 4, the tree's node 40. No rule changes: the std definition, spec/21's text and the three
   trusted claims (`Box::get`, `Box::into_inner`, `Box::drop`). Implemented as `CHG-0394`.

2. **`Option<Box<T>>` stores no discriminant.** `[Layout-Enum]` gains a case: an enum of exactly two variants,
   one with no payload and one whose payload is a `Box<T>`, is laid out as the `Box` alone, and its no-payload
   variant is the null pointer. `sizeof(Option<Box<T>>) = sizeof(rawptr<T>)`; the tree's node is 24 bytes, as in
   Rust. The rule is stated for the one payload type whose representation has a value it can never take and
   whose every use already goes through `std` (`Box::get`, `Box::get_mut`, `Box::into_inner`, `Box::drop`); a
   later decision may admit other such types (`ref`, whose address is never null, is the candidate) under the
   same wording. To a C programmer reading the layout, `Option<Box<T>>` is a nullable `T *`, which is C's own
   idiom for "maybe a pointer", and it crosses an `extern` boundary as one. Both tools make the same layout, and
   a `match`, an `if`-pattern, `Option::is_some` and the rest see it as the enum it is; only its bytes change. To
   be implemented as `CHG-0395`, with the rule in `spec/16`, `[Sizeof-Enum]` and `[Repr-Enum]` in `spec/06`,
   conformance rows for the size and the bytes, and the self-hosted toolchain at its next port (D-0278).
   Implemented as `CHG-0395`: `bench/tree` at 71.9 M instructions and 90.0 M cycles against Rust's 30.1 M and
   57.5 M, the node 24 bytes.

## Why these, and not the alternatives

- **Keep the layout.** Rejected by the aim: the footprint gap is the one part of the gap to Rust that no amount
  of compiler work can close, and it is visible in every program with pointers in it.
- **Step 1 alone.** Contained and principled (`full` duplicated what the pointer already knew), but it leaves the
  node at 40 bytes against 24, and the same objection stands.
- **A general niche rule now** (any payload type with a forbidden value, `ref` and `slice` included). The gain
  elsewhere is real (`Option<ref<T>>` is what `HashMap::get` returns), but a `ref`'s representation in `cobc` is
  an address and a token, and the rule's interaction with light references (D-0292) and the record a token
  names wants its own decision. Step 2 is written so that it can be widened without being rewritten.

The cost of step 2 is accepted knowingly: layout now depends on the payload type in one case, which is a step
away from "the layout is what the C struct says" (D-0250). The case is narrow, stated as a rule, and reads as C's
own convention.

## Consequences

- `sizeof<Box<T>>()` 16 → 8 and `sizeof<Option<Box<T>>>()` 24 → 16 (step 1), → 8 (step 2); every Box-linked
  node shrinks accordingly. `bench/tree` after step 1: 73.9 M instructions and 95.6 M cycles against Rust's
  30.1 M and 61.4 M (`CHG-0394`).
- Step 1 changes no observable behaviour: `into_inner` frees the memory it always freed, one call earlier.
- Step 2 changes `sizeof`, the bytes a `rawptr` into an `Option<Box<T>>` finds, and `[Repr-Enum]`'s FFI
  condition for that one shape, which `spec/06` §7 already names as the implementation's committed choice.
