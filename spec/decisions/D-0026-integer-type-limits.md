# D-0026 — Integer Type Limits Are the Intrinsics `min_value<T>()` and `max_value<T>()`

Status: ACCEPTED (2026-09-24, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0002, D-0006, D-0010, D-0025, rule.arith.convert, rule.arith.alt, rule.type.kind
Affects: rule.arith.convert, rule.arith.alt, rule.stdlib.prelude (the intrinsics table), rule.type.kind

## Problem

A program had no way to name a type's limits. D-0025 lets it write
them as literals (`-2147483648`), but:
- `isize` and `usize` have an implementation-defined width, so a
  literal limit for them is not portable;
- a generic body cannot write a limit of its `T` at all;
- 24 limits written as literals are easy to get wrong.

The specification already uses `min(τ)` and `max(τ)` (spec/06 §1) in
`[Neg-Overflow]`, `[Div-Overflow]` and `[Saturating]`. Programs could
not reach them.

## Constraints

- No new tokens (the owner's standing preference).
- No bound system (D-0010).
- No constant items: the language has none.

## Candidate mechanisms

1. **Two generic intrinsics, `min_value<T>()` and `max_value<T>()`.**
   **Selected.**
2. **Limits named on the type** (`i32::MIN`). Needs a grammar production
   (a `type-name` cannot head a `path`, `spec/22` §1) and associated
   constants, a new kind of entity.
3. **Library functions** (`std::limits::i32_max()` …). No rule changes,
   but 24 functions, none usable generically.
4. **Constant items.** A new keyword and a whole feature.
5. **Literals only** (D-0025 as it stands).

## Selected design

Candidate 1, with four choices made:

- **(a) Names.** `min_value` and `max_value`. They follow the
  library's lowercase style (`checked_add`, `to_float`) and leave `min`
  and `max` free for programs' own two-argument functions.
- **(b) Integers only.** `T` must be an integer type. A float's
  "minimum" has three candidate meanings (most negative finite, −Inf,
  smallest positive), so floats are left for a later decision.
- **(c) Not constants to flow analysis.** `rule.control.flow-analysis`
  refutes literal operands only, and that stays so:
  `max_value<i32>() + 1` is `diag.arith-overflow (dynamic)`.
- **(d) Checked at each instantiation.** A `T` that is not an integer
  type is `diag.type-mismatch (static)`, also when it arrives through a
  generic body. The same holds for every intrinsic whose rule names an
  integer or float type (`rule.arith.convert`, `rule.arith.alt`),
  whose typing no rule stated before.

The type argument is written explicitly, as for the conversions
(`spec/06` §4): `max_value()` is `diag.cannot-infer-type-parameter`.

    [Min-Value]   τ integer    ⊢ min_value<τ>() : τ    ⟨min_value<τ>(), Σ⟩ → ⟨min(τ), Σ⟩
    [Max-Value]   τ integer    ⊢ max_value<τ>() : τ    ⟨max_value<τ>(), Σ⟩ → ⟨max(τ), Σ⟩

## Rejected alternatives

- **2:** new syntax and a new entity kind for twelve pairs of values.
- **3:** no generic use, and `isize`/`usize` would need computing from
  `sizeof`.
- **4:** far more than this problem needs.
- **5:** leaves the portability and generic gaps.
- **Inferring `T` from the expected type** (`i32 hi = max_value();`),
  which D-0013 would allow for a generic call: the conversions already
  require their target type written, and so does this, so that a limit
  always names its type where it is used. It can be added later without
  breaking anything.

## Semantic rationale

`min(τ)` and `max(τ)` are already defined for every integer type. The
intrinsics expose them. They have no side effects and no failure.

## Usability

    i64 lo = max_value<i64>();              // a sentinel for a running minimum
    usize none = max_value<usize>();        // portable, whatever the address width
    fn or_max<T>(Option<T> r) : T           // generic: a limit of T
    {
        match (r)
        {
            Some(x) : x,
            None : max_value<T>(),
        }
    }

## Explainability

"`max_value<τ>()` is the largest `τ`" needs no further explanation. It
has the same shape as `sizeof<τ>()`.

## Implementation-feasibility

Both implementations already hold every type's limits (the range
checks use them). `cobc` emits a C constant. Making (d) static found
that no intrinsic's operand types were checked before the program ran:
`coby` faulted `diag.type-mismatch` at run time, `cobc` failed with an
internal error, and `wrapping_add(1: i32, 2: i64)` was accepted by
both.

## Compatibility impact

- **Newly accepted:** programs using `min_value`/`max_value`.
- **Newly rejected, statically:** an integer intrinsic applied to a
  non-integer, a conversion from or to the wrong kind of type, and an
  alternative operation on two different integer types. The first two
  already faulted at run time in `coby`; the third was silently
  accepted.

## Prior-art status

- **Rust:** `i32::MIN`, `i32::MAX`; before 1.43, `i32::min_value()` and
  `i32::max_value()`, the spelling this decision takes.
- **C++:** `std::numeric_limits<int>::min()`, a generic function of
  the type, as here.
- **C:** `INT_MIN` and `INT_MAX` macros in `limits.h`, one per type.
- **Zig:** `std.math.maxInt(T)`, a generic function of the type.

## Invariant traceability

`inv.arith.range-validity`: both results are in `represented-domain(τ)`
by definition.

## Revisit conditions

- A need for float limits.
- A need for the limits as constants to flow analysis (choice (c)).
- Named constants or associated items being added to the language.
