# D-0098 — A trailing comma in calls and parameter lists

Status: ACCEPTED (2026-09-30, owner-delegated: "go with _ = e; and proceed with all your choices" — proposal P4 of the round-6 stress notes)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: spec/22 §2–§3
Affects: spec/22 `postfix` (calls), `fn-decl`, `extern-decl`, `closure`, `type` (`fn(…)`)

## Problem

A trailing comma was accepted in array and struct literals but not in a
call's arguments or a parameter list: `f(1, 2,)` was a syntax error. One
comma-separated list in a language behaving unlike another is a rule to
remember, and a list written one element per line cannot gain or lose
its last element without touching the line before.

## Candidate mechanisms

1. **Allow an optional trailing comma before `)`** in call arguments,
   parameter lists (functions, `extern` functions, closures) and `fn(…)`
   types. Selected.
2. **Keep C's rule** (trailing commas only in initializers).

Prior art: Python, Rust, JavaScript (ES2017), Kotlin 1.4, PHP 7.3/8.0,
Julia, Dart and Zig allow it; Go requires it before a `)` on the next
line; C, C++, Java and C# do not.

## Selected design

`','?` before the closing `)` of `args`, `params` and a `fn(…)` type's
parameter types. Nothing else changes: an empty list takes no comma
(`f(,)` stays an error), and a comma is never the whole list.

## Compatibility impact

Additive: every program that parsed still parses the same way.

## Revisit conditions

None.
