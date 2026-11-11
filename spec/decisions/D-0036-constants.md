# D-0036 — Constants: `const`

Status: ACCEPTED (2026-09-25, owner-directed: "for 2, const only"; the details delegated)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0006, D-0012, D-0024, rule.module.resolve, rule.arith.literal
Affects: rule.module.const (new), spec/22 §3, diag.const-not-constant (new)

## Problem

A program could declare functions, types and modules but no named
values: a limit, a mask or a size was a literal repeated, or a
function called for it. C has `#define`, `const` and `static`; the
owner chose constants only, not mutable globals (which would interact
with the aliasing and thread rules).

## Constraints

- One new keyword at most.
- A constant is a value: nothing to alias, destroy or share between
  threads.
- A constant costs what the literal costs.
- Checked failures in a constant are found before the program runs.

## Selected design

- **`const τ N = e;`**, an item: exported, imported and qualified as
  any item.
- **A use is the value** of `e`, typed as `τ x = e;` types it
  (`[Const-Use]`); no place, no borrow, a copy per use.
- **`e` is a constant expression:** literals, other constants,
  operators, struct/array/variant literals, a function's name, and the
  pure intrinsics (limits, sizes, conversions, wrapping and saturating
  arithmetic). No calls of functions, no resources
  (`diag.const-not-constant`), no cycles.
- **Checked failures are static** (`[Const-Checked-Failure]`): an
  overflow in `const i32 N = M + 1;` is reported at `N`.
- **Implementation:** a constant is a function of no parameters, and a
  use is a call of it; the front end computes every integer, float,
  `bool` or `str` constant and replaces each use with a literal of its
  type, so it costs nothing at run time. Other constants (a struct
  value) stay calls.

## Rejected alternatives

- **Mutable globals (`static`):** excluded by the owner; they need a
  rule for aliasing and for threads.
- **Untyped constants (`const N = 5;`, Go-style):** a second typing
  rule for literals.
- **Constants in `array<τ, N>` lengths:** types would depend on values
  computed by the front end; possible later.
- **`#define`-style text substitution:** no macros (`spec/22` §5).
- **Letting a constant's literals take its type through operators**
  (`const u64 MB = 1024 * 1024;`): convenient, but then a constant would
  type differently from `u64 x = 1024 * 1024;`. That is a question for
  literal typing in general.

## Semantic rationale

A constant is a named constant expression; with no effects and no
state, its value is the same whenever it is computed. `[Const-Use]`
therefore needs no new state.

## Usability

    const usize BUFFER = 4096;
    const u32 FLAG_READ = 0x1;
    const u32 FLAG_WRITE = 0x2;
    const u32 FLAG_BOTH = FLAG_READ | FLAG_WRITE;
    const Point ORIGIN = Point { .x = 0, .y = 0 };

## Explainability

"`const u32 MAX = 100;` names a value; it can be used anywhere the
value could" — one sentence.

## Implementation-feasibility

`modres` rewrites a resolved use to a call; `impl/src/consts.rs` checks
cycles and folds values with the checked arithmetic of `spec/06`
(overflow, division by zero, shift range); the static pass checks the
constant-expression shape. `cobc`'s generated C holds only literals for
folded constants. Assigning to a constant, like assigning to any
value, is now rejected statically (it was a run-time type mismatch).

## Compatibility impact

Extension; `const` could not begin an item before.

## Prior-art status

- **C:** `#define`, `const` (not a constant expression), `enum`
  constants; **C23** `constexpr`.
- **Rust:** `const N: u64 = 1024;` typed, const-evaluated; `static`.
- **Go:** untyped constants. **Zig:** `const` with comptime values.

## Invariant traceability

None changed: a constant is a value, with no object behind its name.

## Revisit conditions

- Constants in array lengths.
- Mutable globals (`static`), with an aliasing and thread rule.
- Literal typing through operators (`1024 * 1024` taking `u64`).
