# D-0042 — `foreach`

Status: ACCEPTED (2026-09-25, owner-proposed and owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0035, D-0038, D-0041, rule.control.for, rule.stdlib.vec, rule.stdlib.hashmap
Affects: rule.control.foreach (new), rule.stdlib.format, rule.stdlib.hashmap, spec/22 grammar

## Problem

Visiting every element of a collection took a counted loop, an index
call and a length call:
`for (usize i = 0; i < Vec::len(&v); i += 1) { auto x = Vec::index_shared(&v, i); … }`.
The loop said nothing about whether it only read the elements, changed
them in place, or took them. Taking them (moving each element out) had
no spelling at all: a `Vec` gives its elements up only through `pop`,
last first.

## Constraints

- The three ways CobaltC hands over a value — move, shared borrow,
  exclusive borrow — are the three forms, written at the loop.
- No new borrowing or ownership rule: the loop's guarantees come from
  the existing ones.
- The same order and the same output in `coby` and `cobc`.

## Candidate mechanisms

Spelling: **`foreach (x in c)`** (selected, the owner's), `for (x in c)`
(no reserved word), `for (x : c)` (C++). Collections: **`Vec`, arrays,
`HashSet`, `HashMap`** (selected). Element of a borrowed loop: **a
reference, with `printf` formatting a reference to a printable value as
the value** (selected), or a reference with `*x` always written. Index:
**`foreach (i, x in c)`** (selected), or none.

## Selected design

- `foreach (x in c)` consumes `c`; `foreach (x in &c)` gives
  `ref<T, shared>`; `foreach (x in &mut c)` gives `ref<T, exclusive>`.
  A reference `c` is looped over as the borrow it is.
- `foreach (k, x in c)`: `k` is the position (`usize`) for a `Vec`,
  array or set, the key for a map; a map needs two names; `&mut` of a
  set is refused.
- `foreach` is reserved; `in` is recognised only in its place in a
  `foreach`, so `in` stays usable as a name.
- The parser writes the loop as a counted `for` over hidden bindings;
  what depends on the collection's type is expanded by each tool once
  it knows the type (`impl/src/each.rs`). A consuming loop holds the
  collection in a `std`-private holder that moves element `i` out when
  the loop takes it and destroys the rest when the loop ends early.
- `printf`, `sprintf`, `eprintf`: a reference to a number, `bool` or
  `str` is formatted as its value.

## Rejected alternatives

- **`for (x in c)`:** no reserved word; the owner preferred a loop kind
  visible at a glance.
- **`for (x : c)`:** `:` already means a return type, a match arm and a
  type ascription.
- **An iterator protocol** (a user type that can be looped over): needs
  a way to name "a type with `next`", which is a bound system (D-0010).
  The four `std` collections cover the programs written so far.
- **Copying elements in a borrowed loop over plain types:** `x` would be
  a value for `Vec<i32>` and a reference for `Vec<String>`; one rule is
  simpler.

## Semantic rationale

`[Foreach-Borrowed]` and `[Foreach-Consumed]` are equivalences with a
`for` loop over existing functions (spec/14). The borrow of `c` lasts
the loop, so a body that changes `c` is the aliasing conflict it would
be anywhere; a consuming loop moves `c` into its holder, so a use of
`c` afterwards is `diag.stale-binding`; `break`/`return`/faults destroy
the holder, and with it the elements not yet taken, exactly once.

## Usability

    foreach (name in &names)
    {
        printf("Name: %s\n", name);
    }
    foreach (n in &mut numbers)
    {
        *n *= 2;
    }
    foreach (word, count in &counts)
    {
        printf("%-10s %d\n", word, count);
    }

## Explainability

"`foreach (x in c)` takes the elements, `in &c` reads them, `in &mut c`
changes them; a second name is the index, or a map's key."

## Implementation-feasibility

The parser's rewrite; `each::expand` shared by the typechecker (which
checks the expansion with `std`'s field visibility), `coby` and `cobc`;
three private holders in `std`.

## Compatibility impact

`foreach` becomes a keyword: a program using it as a name no longer
parses. None in the repository did.

## Prior-art status

C# `foreach (var x in c)`; Rust `for x in c` / `&c` / `&mut c` (the
three forms); Python `for x in c` and `enumerate`; C++ range-`for`.

## Invariant traceability

None new; `inv.alias.*` and `inv.resource.*` hold through the existing
rules the expansion uses.

## Revisit conditions

- Looping over user types (would need a bound system or a naming
  convention).
- Ranges (`foreach (i in 0..n)`).
- Looping over a `String`'s bytes or characters.
