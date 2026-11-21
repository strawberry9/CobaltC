# D-0028 — `print` Takes Any Printable Type: `print<T>(T x)`

Status: ACCEPTED (2026-09-24, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0010, D-0020, D-0024, D-0026, rule.stdlib.str, rule.type.kind
Affects: rule.stdlib.str (its `print`), rule.stdlib.print (new), rule.stdlib.prelude

## Problem

`print` took only `str`. Printing a number, a `bool` or a `String` meant
writing a formatting helper over `write` in `unsafe` code: the
showcases and benchmarks each carry an `out.cb` module for this. On
2026-09-23 the owner left this to programs; this decision revisits it.

A generic `print<T>(T x)` cannot be written in CobaltC: without bounds
(D-0010) a body can do nothing with a `T` but store and move it.

## Constraints

- No new tokens, and one output function named `print` (the owner's
  2026-09-23 note: rename only if a second output function arrives).
- `coby` and `cobc` must write the same bytes for every value.
- Existing programs keep working.

## Candidate mechanisms

1. **`std::print<T>(T x)`, realized natively**, for a fixed set of
   printable types, checked at each instantiation as D-0026's intrinsics
   are. **Selected.**
2. **A family of ordinary functions** (`print_int`, `print_float`,
   `print_string`, …) written in CobaltC.
3. **Conversions to text** (`String::from_int`, …) with `print` taking
   `str` or `&String`.
4. **No change.**

## Selected design

Candidate 1, with four choices made:

- **(a) Floats print their shortest round-trip digits**: the fewest
  decimal digits that read back as the value in its own type, nearest
  the value and ties to even (JavaScript's and Python's rule).
  Positional for a decimal exponent from −6 to 20, always with a
  fraction (`1.0`, `0.1`, `100000000000000000000.0`), and `1.0e21`,
  `1.0e-7` outside it; `NaN`, `inf`, `-inf`, `-0.0`.
- **(b) `String` by shared reference only**: `print(&s)`. By value it
  would be moved into `print` and destroyed.
- **(c) Printable types**: `str`, every integer type, `f32`, `f64`,
  `bool`, `ref<String, shared>`. Composite values and single-byte
  characters are not printable; a `u8` prints as a number.
- **(d) No `println`**: `print("\n")` stays the way to end a line.

`print` stays an exported function of `std`, reached as before, and a
program's own `print` still takes precedence (D-0024). `print` of a
`str` behaves exactly as before.

## Rejected alternatives

- **2:** conversions at nearly every call site, no generic use, and a
  family of output names.
- **3:** more library than this problem needs; it can be added later
  beside this design.
- **4:** every program that prints a number repeats the same helper.
- **A bound system** (`T: Printable`): a far larger feature (D-0010's
  revisit condition). This design would become one instance of it.
- **Fixed decimal places for floats:** loses information by default;
  fixed-precision output can be a separate function later.
- **Rust's positional-only layout** (`1e300` as 301 digits): unreadable
  for large and small magnitudes.

## Semantic rationale

`print` stays an ordinary `std` function in every respect a program can
observe, except that its argument type ranges over a fixed set, as
`max_value<T>()`'s does. Its `str` and `String` cases are written in
CobaltC inside `std`; only number formatting is native.

## Usability

    print(n);              // an integer
    print(x);              // a float: 0.1, 1.0, 1.0e21
    print(&name);          // a String
    fn show<T>(T x)        // generic code can print its T
    {
        print(x);
    }

## Explainability

"`print` prints text, numbers and `bool`; a `String` through `&`" is
one sentence. The float rule is the one programmers see in JavaScript
and Python.

## Implementation-feasibility

The float formatter is one Rust function, the same in `coby` and in
`cobc`'s runtime (`cbrt`), built on Rust's shortest (`{:e}`) and
correctly rounded (`{:.*e}`) conversions. It matched an independent
reference on 9,000 `f64` and `f32` values, and the two implementations
agreed on all of them.

## Compatibility impact

Extension. Every program that printed a `str` prints the same bytes.
`out.cb`-style helpers keep working; they are no longer needed for
plain numbers.

## Prior-art status

- **Rust:** `print!("{}", x)` through the `Display` trait; shortest
  round-trip floats, positional only.
- **JavaScript:** `Number.prototype.toString`: shortest round-trip,
  exponent form outside 1e-7 ≤ |x| < 1e21 (the thresholds taken here).
- **Python:** `print(x)` via `repr`: shortest round-trip, ties to even.
- **Go:** `fmt.Println(x)`, shortest round-trip (`%v`).

## Invariant traceability

`inv.str.utf8-validity`: every byte `print` writes is valid UTF-8,
since the number, `bool` and float texts are ASCII and `str` and
`String` are valid by their own invariants.

## Revisit conditions

- A need for fixed-precision or other formatted output.
- A need to print composite values (would suggest a bound system).
- A second output function (then name the family consistently).
