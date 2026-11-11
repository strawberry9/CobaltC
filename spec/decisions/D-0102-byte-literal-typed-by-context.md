# D-0102 — A byte literal takes an unsigned type from its context

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 friction 16)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0033 (`b'x'`), D-0037 (literal expressions)
Affects: spec/06 `rule.arith.literal`, spec/22 `byte-char-literal`

## Problem

There is no character literal (spec/22 §5), and `b'x'` was fixed at
`u8`. A program working on code points (`u32`) — a regex engine, a
tokenizer over decoded text — compared with `widen<u32>(b'|')` at every
turn (`regex.cb` wrapped it in a helper `cp`).

## Candidate mechanisms

1. **`b'x'` takes the unsigned integer type its context expects, `u8`
   otherwise.** Selected.
2. **Any integer type from context** — a byte in a signed context reads
   oddly, and `i8` cannot hold `b'\xff'`.
3. **A character literal `'x'` of type `u32`** — a new literal form, and
   the spec's deliberate absence of character literals (a byte is not a
   character in UTF-8) would have to be reopened.

## Selected design

A byte literal is typed like an unsuffixed integer literal whose context
may only be an unsigned integer type: `u8`, `u16`, `u32`, `u64`, `u128`,
`usize`. With no such context it is a `u8`, as before. In a pattern, it
takes the scrutinee's type.

## Compatibility impact

Additive: every program that type-checked still does, with the same
types (`u8` where nothing else is expected).

## Revisit conditions

None.
