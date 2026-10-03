# D-0103 — Parts of temporaries: borrowed or sliced as arguments, a field moved out

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 frictions 1, 13 and 21)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0073, D-0084 (a temporary borrowed as an argument), D-0044 (destructuring)
Affects: spec/09 `rule.ref.form`, spec/16 §1 (`[Temp-Root]`, `[Temp-Field-Move]`)

## Problem

D-0084 let a whole temporary be borrowed as a call's argument
(`total(&make())`), but not a part of one: `String::len(&t("x").name)`
(C's `strlen(get().name)`), `sum(&[1, 2, 3][0..$])` and
`sum(&make().v[0..2])` were rejected, and so was moving a field out of a
function's result (`divmod(a, b).r`), though the temporary ends at the
statement anyway. Each needed a binding first, or a helper that
destructures.

## Candidate mechanisms

1. **Extend D-0084 to projections and slices, and let a field be moved
   out by taking the temporary apart.** Selected.
2. **Give temporaries longer lives** (to the block's end) — a new
   lifetime rule for everything, not just these forms.
3. **Keep the rule and teach binding first.**

## Selected design

- As an argument of a call whose result is not a reference, `&e` where
  `e` is a field or element (any chain) of a temporary, and `&e[lo .. hi]`
  where `e` is a temporary or part of one, are formed as for a place; the
  temporary lives to the end of the statement, after the call.
- `f().x`, where it moves a resource value, is `{ S { x, … } = f(); x }`:
  the field is the value and the temporary's other fields end at once. A
  struct with a destructor is not taken apart (as D-0044 already says).

## Compatibility impact

Additive: programs that were rejected now run. `conf.move-out-of-temporary-field-rejected`
becomes `conf.move-out-of-temporary-field` (a positive row), with the
destructor case keeping the rejection.

## Revisit conditions

None.
