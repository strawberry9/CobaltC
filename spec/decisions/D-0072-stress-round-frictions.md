# D-0072 — Frictions from the stress-test round

Status: ACCEPTED (2026-09-27, owner-delegated: "fix the frictions using your leanings")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8 (items 7, 9, 10), §9
Depends on: D-0049, CHG-0082, rule.agg.array-construct, rule.arith.alt
Affects: `spec/16` `[Array-Repeat]`; `spec/22` array literals; `spec/21`
`checked_neg`; `spec/registry/diagnostics.md`

## Problem

Writing ten larger programs (a ledger, a calculator, an LRU cache, a
CSV report, an event simulation, a directory tree, a worker pool, a
Markdown converter, and others) turned up these frictions:

1. **Diagnostics named nothing:**
   - `diag.unbound-name` and `diag.name-not-visible` did not say which
     name;
   - `diag.type-mismatch` did not say which types met, or that `*` was
     applied to something that is not a reference;
   - `i32[4] xs;` gave only "expected an expression".
2. **No way to fill an array with one value.** `array<u64, 200>` needed
   200 written elements.
3. **An array literal rejected a trailing comma**, which a `match`
   accepts.
4. **No `checked_neg`** beside the other `checked_*`.
5. **Assigning over a field that holds a live resource**
   (`r.best = String::clone(…)`) is a fault. Its repair said only
   "`drop` first".

## Constraints

No new tokens (Master Instructions §9, the owner's standing
preference). No change to what any existing program means.

## Candidate mechanisms and selection

1. **Messages.** A diagnostic carries a message after its location, as
   `static_assert`'s and a syntax error's already do. Selected.
   - `unbound-name`: "nothing named `zz` is declared here".
   - `name-not-visible`: "`HashMap::drain` is not exported from its
     module".
   - `type-mismatch`, at its common sites:
     - declared type against its initializer;
     - operand types;
     - argument count and argument types;
     - a missing field;
     - `*` on a non-reference;
     - an assignment;
     - the return type;
     - `match` arms that differ;
     - `!` and `-` operands;
     - format arguments.
   - `T[N] x`: a syntax error whose message names `array<T, N>`.
2. **`[v; N]`**, Rust's spelling, with `N` an integer literal as an
   array type's length is. Selected. The alternatives were a function
   `array_fill<T, N>(v)`, which needs a length type parameter the
   language does not have, and nothing at all.
   - `v` is read once and copied, so its type must be plain: not a
     resource, and holding no reference, slice or function value.
   - A copy of those would be a second owner, a second exclusive path,
     or a copied closure box.
3. **A trailing comma** in an array literal, as in `match`. Selected.
4. **`checked_neg<T>(T a) : Option<T>`**, defined as
   `checked_sub(0, a)`: the `0` takes `a`'s type. It is `None` for the
   most negative value of a signed type and for every non-zero unsigned
   value. Selected over leaving `checked_sub(0, a)` to programs: the
   family should be complete.
5. **The resource overwrite stays a fault.** Silently destroying the old
   value is what D-0049 and `ex.no-silent-resource-overwrite` exclude.
   The repair now names `drop(replace(&mut place, v))`. Selected over
   making `=` destroy the old value.

## Semantic rationale

1, 3 and 5 change no meaning. 2 is `[Array-Construct]` with every
element the same value, restricted to types whose values may be copied.
4 is a definition in terms of an existing operation.

## Usability

    array<u64, 256> counts = [0; 256];
    array<str, 3> names = ["ann", "bo", "cy",];
    Option<i32> neg = checked_neg(x);

## Implementation-feasibility

- **Front end, shared by both tools:** the parser reads `[e; N]` into
  `ArrayRepeat` and allows the trailing comma. `modres` rewrites
  `checked_neg`. The checker types `ArrayRepeat`, attaches the
  messages, and prints types as a program spells them (`Display for
  Type`).
- **`coby`:** evaluates `v` once and fills the array.
- **`cobc`:** evaluates `v` into a temporary, then fills the array with
  a C loop.

## Compatibility impact

Extension. Programs that were rejected (`[v; N]`, a trailing comma,
`checked_neg`) are accepted. The rejection messages carry more text;
no outcome or phase changes.

## Prior-art status

- **Rust:** `[v; N]` for `Copy` values, `checked_neg`, and diagnostics
  that name the expected and found types.
- **C:** `T x[N] = {0}` zero-fills, but has no general repeat.

## Revisit conditions

- A constant expression as a length, in both array types and `[v; N]`.
