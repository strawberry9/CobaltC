# D-0146 — `unsafe extern fn`: the declaration's claim marked where it is made

Status: ACCEPTED (2026-10-04, the owner: "I want to adopt Rust 2024 requirements of having all 'extern' declarations be 'unsafe extern'", then "accept all recommendations and implement" of `private/unsafe-extern-proposal.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1
Depends on: D-0019 (trust boundaries), D-0030 (`extern "…";`)
Affects: `spec/22` §3 (2.46.0), `spec/20` §3 (1.12.0), `spec/21` listings, `spec/conformance.md` (3.136.0), `spec/examples.md`, the guide §20 and every example declaring an extern, `impl/src/parser.rs`, `impl/std/*.cb`, conformance cases, showcase Tier 4

## Problem

`extern fn name(…) : τ;` claims, unchecked, that the linked code has a
function of that name with exactly that FFI signature. A wrong
declaration makes every call undefined, although each call sits inside
`unsafe { }` — and the declaration, the one line whose mistake made them
all wrong, carried no `unsafe`, so an audit of `unsafe` passed it by.
Rust 2024 made the same observation and requires `unsafe extern` blocks.

## Candidate mechanisms

A. `unsafe extern fn` required, plain `extern fn` rejected. **Selected.**
B. Both spellings accepted: two forms, and the audit holds only for
   programs that opt in; CobaltC has no warnings to nudge with.
C. Leave it: calls are already `unsafe`.
D. A plus Rust 2024's `safe fn` (an extern callable without `unsafe`):
   a new contextual keyword and a second kind of extern call. Not now.

## Selected design (the owner accepted every recommendation)

- `extern-decl ::= vis 'unsafe' 'extern' 'fn' …` — `export unsafe extern
  fn` with a visibility (Q3). Plain `extern fn` is `diag.syntax-error`
  with the repair "write `unsafe extern fn`" (Q1, Q4: no new diagnostic).
- `extern "…";` (`rule.trust.extern-code`) is unchanged and takes no
  `unsafe`: it names code to link and declares no signature; `unsafe`
  before it is a syntax error (Q2).
- `std`'s own externs (`stdout_write`, `stdin_read`, `arg_bytes` and the
  private primitives) are written `unsafe extern fn` too (Q5).
- `spec/20` §3 `[Extern-Decl]`, disposition `trusted-unchecked`, states
  the signature's claim at the declaration; `[Extern-Call]` is unchanged
  and calls still need `unsafe { }` (as in Rust 2024).

## Compatibility impact

Breaking (source): every `extern fn` declaration gains `unsafe`. The
repair is one word, and the error message gives it. Meaning is
unchanged.

## Revisit conditions

- `safe fn` declarations callable without `unsafe` (option D), if
  programs measured wrap many pure C functions (`abs`, `sqrt`) in
  `unsafe` blocks only to call them.
