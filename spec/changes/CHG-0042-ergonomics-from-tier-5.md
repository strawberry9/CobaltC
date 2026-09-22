# CHG-0042 — Re-assignment after a move, `b'x'`, float exponents, `unwrap_or`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-25, owner-delegated)
Governed by: `CobaltC_Master_Instructions.md` §1, §19, §21
Depends on: D-0033, rule.init.let, rule.value-object.write, rule.arith.literal
Affects: rule.init.let, rule.value-object.write, rule.control.flow-analysis, rule.arith.literal, rule.stdlib.prelude, diag.literal-out-of-range, diag.stale-binding, the cases listed below

## Problem / motivation

D-0033 has the details: six awkward spots found writing the Tier 5
showcases, four of which change.

## Decision

D-0033 (1), (2), (4), (6); (3) and (5) unchanged.

## What changed

- **`spec/11` 1.2.0:** `rule.init.let` gains `[Assign-Reestablish]`.
- **`spec/05` 1.2.0:** `[Write-Stale]` excludes a whole-binding
  assignment.
- **`spec/14` 1.4.0:** a whole-object write sets `valid(x) := T`.
- **`spec/22` 2.12.0:** `float-literal` with `exponent`;
  `byte-char-literal`; §5's absence names `b'x'`.
- **`spec/06` 1.8.0:** `rule.arith.literal` on float values and range,
  and on `b'x'`.
- **`spec/21` 3.7.0:** `Option::unwrap_or`, `Result::unwrap_or`.
- **`spec/examples.md` 3.13.0:** `ex.no-silent-resource-overwrite`.
- **`spec/registry/diagnostics.md` 1.13.0:** `diag.literal-out-of-range`,
  `diag.stale-binding`.
- **`spec/conformance.md` 3.28.0:** the cases below.
- **`spec/02-schema.md` 1.0.18:** §5's "in use" ranges.
- **Implementations:**
  - typecheck: a whole-binding assignment target is not a stale
    lookup and re-validates the binding; float literals are
    range-checked;
  - `coby`: `reinit_binding` (a new object in the declaring frame;
    frames record binding types), checked after the right side so
    `x = f(x)` works; a float literal typed `f32` by context is
    rounded to `f32` (it was left as an `f64`);
  - `cobc`: `cb_frame_here` at each binding of a name the function
    assigns as a whole, `cb_rebind` before its write;
  - the lexer: `b'x'`, exponents;
  - `std`: `Option::unwrap_or`, `Result::unwrap_or`.

## Previous semantics

A whole-binding write to a moved-from or dropped binding was
`[Write-Stale]`. Float literals had no exponent, and one that rounded
to infinity was accepted. No `b'x'`, no `unwrap_or`.

## New semantics

    [Assign-Reestablish]   x = e, x's value gone (before or during e)  ⇒  new object in x's frame, then store

plus the literal grammar and values of `spec/22` §1 and
`rule.arith.literal`.

## Affected invariants

None changed; see D-0033's traceability.

## Compatibility classification

Extension, except that a float literal rounding to infinity is now
rejected (none in the repository). One conformance case's outcome
changes from rejected to accepted.

## Migration implications

None.

## Conformance changes

**Removed:** `conf.reassign-after-drop-rejected` (outcome changed;
replaced by `conf.reassign-after-drop-ok`).

**Added:**
- `conf.reassign-after-drop-ok`
- `conf.reassign-after-move-ok`
- `conf.reassign-from-own-move-ok`
- `conf.reassign-in-inner-block-ok`
- `conf.reassign-maybe-live-dynamic`
- `conf.float-exponent-literal`
- `conf.float-literal-out-of-range`
- `conf.float-literal-f32-out-of-range`
- `conf.float-literal-f32-rounded`
- `conf.byte-char-literal`
- `conf.byte-char-escape`
- `conf.byte-char-is-u8`
- `conf.option-unwrap-or`
- `conf.result-unwrap-or-resource`

File cases under `impl/conformance/22-surface-syntax/`:
`byte_char_and_exponent_ok.cb`, `byte_char_empty_parse_error.cb`,
`byte_char_two_bytes_parse_error.cb`,
`byte_char_non_ascii_parse_error.cb`.

## Future implementation implications

- A float literal is lexed to an `f64` and then rounded to `f32` when
  typed so; a literal whose correctly rounded `f32` differs from its
  `f64`-then-`f32` rounding (double rounding) would get the latter.
  No realistic literal does; exact rounding needs the literal's text
  kept to typing.

## Prior-art status

See D-0033.

## Revisit conditions

See D-0033.
