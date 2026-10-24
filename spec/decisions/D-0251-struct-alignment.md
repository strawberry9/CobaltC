# D-0251 — `struct S align(N)`

Status: ACCEPTED (2026-10-16, the owner chose "`struct S align(64)`" of three spellings)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0250, D-0249 (a counter on its own cache line), `rule.agg.layout`, `[Allocate]`
Affects: `spec/22` §3 `struct-align` (2.51.0), `spec/16` §1 (1.30.0), `spec/conformance.md`; `impl/src/parser.rs`, `impl/src/ast.rs`, `impl/src/interp/layout.rs`, `impl/cobc/src/lower/layout.rs`; `CHG-0308`

## Problem

A type's alignment was always its fields' largest. Systems code needs more: a counter that threads update on its
own cache line (otherwise two counters on one line make the cores fight over it, "false sharing"), a buffer a
device or a vector instruction requires aligned, a C struct declared with `_Alignas`.

## Options

1. `struct S align(64) { … }`: a contextual word after the name and type parameters, not reserved. Per type
   only; a field gets an alignment by wrapping it in an aligned struct, as in Rust.
2. `align(64) struct S { … }`: the same, in front.
3. A built-in `aligned<τ, N>` type usable on fields directly: a new reserved type-name and a const parameter
   only `array` has.

Selected: 1, the owner's choice.

## Decision

`struct-align ::= 'align' '(' int-literal ')'` after the type parameters (`spec/22`). `N` is unsuffixed, a power
of two from 1 to 4096, else a syntax error. `[Layout-Struct]`: `alignof(S) = max(N, the fields' alignments)`, so
`align` raises and never lowers; offsets are unchanged and `sizeof` is a multiple of the alignment, so each array
or `Vec` element starts on an `N` boundary. Not for a `bitstruct`, which is its backing integer, or an enum.

## Results

Both tools compute the layout; `cobc` writes `__attribute__((aligned(N)))` on the C struct, and over-aligned
storage from `allocate` was already supported in both runtimes. `conf.struct-align` (sizes, a generic struct,
nesting, `align` still an identifier elsewhere), `conf.struct-align-invalid` (syntax error) and
`conf.struct-align-addresses` (`cobc` only: locals, fields, `Vec`, array, `Box`, mutex, enum payload and a
thread's argument all on their boundary, under GCC and Clang).
