# CHG-0132 — Method-call syntax is a deliberate absence

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-09-30; the owner's decision, final: "I don't want method call syntax like that. Make that final.")
Governed by: `CobaltC_Master_Instructions.md` §13, §18
Depends on: `rule.fn.call` (`spec/15` §2), `spec/17` §1 (associated functions)
Affects: `spec/22` §5 (deliberate absences), the guide's "What CobaltC deliberately leaves out"

## Problem / motivation

A scan of every program in the repository (2026-09-30, `private/findings.md`,
finding F1) measured what the call form costs: more than three thousand
sites spell the function's owner and the borrow, `Vec::push(&mut v, x)`,
`Vec::len(&v)`, `HashMap::get(&m, &k)`; and method-shaped functions on user
types are written as free functions five times more often than as
`fn T::name`, since `T::name` gives only a namespace. The scan proposed a
receiver form, `v.push(x)` standing for `Vec::push(&mut v, x)`, with the
borrow copied from the declared first parameter.

The owner declined it, and declined it finally. The guide's tour already
states the position as a fact — "Library calls are qualified: `Vec::push(&mut v, x)`,
`Vec::len(&v)` … There is no method-call syntax" — but `spec/22` §5, the
list of considered absences, did not carry it, so the question could be
re-raised by the next count of the same sites.

## What changed

- **`spec/22` §5:** the deliberate-absences list names method-call
  syntax in every form — receiver sugar for `fn T::name`, uniform call
  syntax for free functions, an auto-borrowing `.`, and call chaining —
  and gives the reason: a call names the function's owner and writes
  the borrow it takes, so the mode of every access path is visible at
  the point it is formed, and `.` means one thing, field access.
- **The guide:** the same item in "What CobaltC deliberately leaves out".

No rule, production, diagnostic or implementation changes: the grammar
never derived `e.name(args)` as a call of `T::name`, and both tools
already reject it.

## Compatibility classification

Editorial: records a considered "no". Every program accepted before is
accepted, with the same meaning.
