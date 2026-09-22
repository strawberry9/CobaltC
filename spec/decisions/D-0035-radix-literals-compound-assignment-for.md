# D-0035 — Radix Literals, Compound Assignment, `for`

Status: ACCEPTED (2026-09-25, owner-directed: "do 1"; the details delegated)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0007, D-0025, rule.arith.literal, rule.control.while, rule.value-object.write
Affects: spec/22 §1–§3, rule.expr.compound-assign (new), rule.control.for (new)

## Problem

Compared with C (the owner asked, 2026-09-25, how far CobaltC is from
C), everyday code was longer than it needed to be: every integer
literal was decimal (`0xFF` could not be written), every update was
`x = x + 1`, and every counted loop was a `while` with its counter
declared outside and stepped by hand at every `continue`.

## Constraints

- Nothing new in the semantics: each form means something the language
  already has.
- Few new tokens.
- No slowdown: `i += 1` in a hot loop must cost what `i = i + 1` does.

## Selected design

- **Integer literals in bases 16, 8 and 2:** `0xFF`, `0o755`,
  `0b1010`, with the same suffix and typing as a decimal literal.
  Octal is spelled `0o`, not C's leading `0`, so `010` stays ten.
- **Compound assignment:** `+= -= *= /= %= &= |= ^= <<= >>=`. `p op= e`
  is `p = p op e` with `p` evaluated once (`[Compound-Assign]`); its
  type, checks and overflow are `p = p op e`'s. A place without a call
  (`i`, `p.x`, `a[i]`) is written out twice, which evaluates to the same
  place; one containing a call is reached once, through an exclusive
  reference.
- **`for (init; cond; step) body`**, C's loop: `{ init; while (cond)
  body }` with `step` run after the body and on `continue`. Each part
  may be left out; a missing `cond` is `true`. The initializer's
  binding belongs to the loop.
- **Not added:** `++`/`--` (`+= 1` says it), `do … while`, a range
  loop (`for x in …` needs an iteration protocol, which is a larger
  decision), C's octal `0…`, digit separators.

## Rejected alternatives

- **Compound assignment as `p = p op e` always:** `p` evaluated twice
  breaks `*Vec::index_exclusive(&mut v, i) += 1` (two exclusive
  borrows of `v`).
- **Compound assignment always through a reference:** makes every
  `i += 1` borrow `i`, which the compiler must then check at run time.
- **`for` as a syntactic rewrite of `continue`:** a body that declares
  the counter's name again would change what the copied `step` refers
  to.

## Semantic rationale

Each form is defined by the rules that already exist: literal typing,
`[Write]`, `[While-*]`, `[Continue]`. `[For-Continue]` is `[Continue]`
with the step in front.

## Usability

    for (usize i = 0; i < Vec::len(&v); i += 1)
    {
        if (*Vec::index_shared(&v, i) & 0x80 != 0)
        {
            continue;
        }
        total += widen<u64>(*Vec::index_shared(&v, i));
    }

## Explainability

"`for` is C's; `x += e` is `x = x + e`; `0x`, `0o`, `0b` literals" —
one sentence.

## Implementation-feasibility

The shared lexer and parser: compound assignment and `for`'s
initializer become existing forms; a `while` node gained an optional
step (the interpreter runs it after the body and after `continue`; the
compiler jumps to a label before it on `continue`; the static pass
joins the body's end and every `continue` before it).

## Compatibility impact

Extension; `for` and the new operator tokens could not appear before.

## Prior-art status

C, C++, Java, JavaScript, Go (`for` without parentheses); `0o` from
Python, Rust, Swift; Rust and Swift dropped `++`.

## Invariant traceability

None changed: every form reduces to existing rules.

## Revisit conditions

- An iteration protocol (`for x in v`), with a trait-like mechanism or
  per-type intrinsics.
- Digit separators (`1_000_000`).
