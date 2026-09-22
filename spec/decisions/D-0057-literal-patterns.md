# D-0057 — Literal patterns; the pattern language is closed

Status: ACCEPTED (2026-09-26, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0056, D-0025, rule.agg.match, rule.arith.literal
Affects: rule.agg.match, `spec/22` `pattern`

## Problem

CobaltC has no `switch` (`spec/22` §5), and after D-0056 a pattern could
test variants but not values. A choice among integer values was an
`if`/`else if` chain — `showcase/tier3/tinyos/cpu.cb` decodes opcodes
with fourteen — in which a repeated case is silently dead; and a
payload's value took a binder and an `if` (`Ok(n) : { if (n == 0) … }`
for the end of a file, twice in Tier 7).

## Constraints

- The owner's: the syntax must not grow into Rust's pattern language
  (ranges, `@`, `ref`, slices, struct patterns with `..`, guards,
  or-patterns, binding modes).
- No new tokens.
- `match` stays checked: exhaustive, every arm reachable.

## Candidate mechanisms

1. **Literals where a variant can stand**: an integer (negated or not,
   suffixed or not), a byte, `true`, `false`; at the top level (a
   `match` on an integer or `bool`) and at the end of a nested pattern.
   **Selected.**
2. As 1, with or-patterns (`b' ' | b'\t' : …`).
3. As 1, with ranges (`b'0' .. b'9'`).
4. Nothing: `if` chains remain the way.

## Selected design

Candidate 1, and the pattern language is closed at its four forms:

    pattern ::= '_' | literal | Variant | Variant '(' (pattern | binder) ')'

- **Typing.** A literal is typed as any literal is (`rule.arith.literal`)
  with its level's type expected — the payload's, or the scrutinee's
  for a pattern that is only a literal — and must have exactly that
  type, an integer type or `bool`: `300` for a `u8` is
  `diag.literal-out-of-range`, `1` for a `bool`, `-1` for a `u32` and
  `b'a'` for an `i32` are `diag.type-mismatch`, as they are anywhere.
  A reference to an integer is not matched by literals (`match (*r)`).
  Floats are not patterns (a parse error that says so): `==` on NaN and
  ±0 would make them mislead.
- **Coverage.** A `bool` level is covered by `true` and `false`; an
  integer level only by an arm that stops above it (`_`, a binder), so a
  `match` on an integer needs `_`. The missing pattern is named (`_`,
  `Ok(_)`, `Some(false)`); a repeated literal is an unreachable arm.
- **No binders at the top level**, as before: the scrutinee is already
  named where it matters.

## Rejected alternatives

- **2 and 3:** each is a new pattern form with its own rules (binders
  in alternatives; empty or overlapping ranges), the path the owner
  named as the reason not to use Rust. One arm per value, or an `if`,
  does what they do.
- **4:** leaves the multi-way branch unchecked, where a checked one
  costs no new form.

## Semantic rationale

A literal pattern is `==` against a constant, decided at compile time
for coverage and reachability; it adds no binding, no move and no
borrow, so the resource rules of D-0056 are untouched.

## Usability

    match (op)
    {
        1 : load(cpu),
        2 : store(cpu),
        0x10 : jump(cpu),
        _ : illegal(cpu),
    }

    match (File::read(&mut f, &mut buf, 16))
    {
        Ok(0) : { break; },
        Ok(_) : { … },
        Err(_) : { … },
    }

## Explainability

"A pattern is `_`, a literal, a variant, or a variant with a pattern
for its payload. That is all of them."

## Implementation-feasibility

`Arm` gains `lit` (the literal expression); the parser reads one where
a variant name could start; the checker types it with the level's type
expected and extends coverage (`pattern_elems`: a literal is an element
`=value`; a `bool` level splits into `=true`/`=false`); `coby` compares
(`lit_equals`) and has a scalar path for an integer or `bool`
scrutinee; `cobc` adds `slot == literal` to an arm's condition and
lowers a scalar `match` to an `if`/`else if` chain
(`lower_match_scalar`).

## Compatibility impact

Extension.

## Prior-art status

- **C:** `switch` on integer constants, unchecked (fall-through,
  duplicate cases rejected, no exhaustiveness).
- **Rust / OCaml / Swift:** literal patterns, and much more (ranges,
  or-patterns, guards), which this decision declines.
- **Go:** `switch` with constant cases; duplicates rejected.

## Invariant traceability

None changed.

## Revisit conditions

- Only with an explicit argument against this record's closed list:
  or-patterns and ranges were considered and declined.
