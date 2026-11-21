# D-0032 — Text Conversions: `String::append` and `parse`

Status: ACCEPTED (2026-09-25, owner-chosen; the two `String` additions and (f) owner-delegated)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0010, D-0024, D-0026, D-0028, D-0029, D-0031, rule.stdlib.print, rule.stdlib.string
Affects: rule.stdlib.text (new), rule.stdlib.prelude, rule.module.resolve

## Problem

A program can now read lines (D-0029) and arguments (D-0031), but it
cannot turn the text `42` into a number, and it cannot build text from
numbers: `print` writes a number but nothing puts one into a `String`,
and a `String` cannot be added to at all. Every program that reads a
number, or builds a line before writing it, writes that code itself.

## Constraints

- No new tokens.
- No bound or trait system (D-0010): a function whose behaviour depends
  on its type argument is realized natively, per instantiation, as
  `print<T>` is (D-0028).
- A number built into text is exactly what `print` writes for it, and
  text that `print` wrote parses back to the same value.
- Existing programs keep working, including those declaring their own
  `parse` or `ParseError`.

- A program's own names keep their meaning when `std` grows. For
  items that already held (D-0024); for enum variants it did not (see
  (f)).

## Candidate mechanisms

To text:

1. **`String::append<T>(ref<String, exclusive> s, T x)`**, for exactly
   `print`'s types. **Selected.**
2. **`to_string<T>(T x) : String`** and `String::push_str`.
3. **Functions per type**: `String::from_int`, `String::from_float`, …

From text:

4. **`parse<T>(ref<String, shared> s) : Result<T, ParseError>`** for
   every integer type, `f32` and `f64`. **Selected.**
5. The same, integers only.
6. **Functions per type**: `parse_i64`, `parse_f64`, …

## Selected design

Candidates 1 and 4, with six choices made:

- **(a) `append` adds `text(x)`**, the bytes `print(x)` writes
  (`rule.stdlib.print`): `str`, every integer type, `f32`, `f64`,
  `bool`, `ref<String, shared>`. Another `T` is `diag.type-mismatch`,
  checked at the call. Appending a `String` to itself is the aliasing
  conflict it always was.
- **(b) `parse` is strict.** Text is accepted only when it is entirely
  a number: an optional `+` or `-` (not `-` for an unsigned type), then
  digits; for a float also an optional fraction (`.` and digits) and
  exponent (`e` or `E`, an optional sign, digits), or `inf`, or `NaN`.
  No spaces, no `_`, no hexadecimal. Leading zeros are allowed.
- **(c) `ParseError` is `Empty`, `Invalid(usize)` or `OutOfRange`.**
  `Invalid(k)` gives the offset of the first byte that cannot continue
  a number, or the length when the text ends too early; an invalid
  byte is reported before a value out of range.
- **(d) A float is correctly rounded** (to nearest, ties to even), in
  its own type; a finite value that rounds beyond the type's range is
  `OutOfRange`. Every text `print` writes for a float parses back to
  that float.
- **(e) `String::new()` and `String::as_bytes`** come with them
  (delegated to the design): `append` needs an empty `String` to start
  from, and `as_bytes(&s) : ref<Vec<u8>, shared>` lets a program read
  a `String`'s bytes, to split a line into the pieces it then parses.
- **(f) A program's own variants are found before imported ones**
  (`spec/17` `[Resolve-Unqualified]` clauses (4) and (4b); delegated to
  the design). `ParseError`'s `Empty` met programs' own `Empty`
  variants (the showcase, example 06, a conformance case, two spec rows
  and a guide example), and clause (4) counted own and imported enums
  alike, so each became `diag.ambiguous-name`. Now clause (4) looks in
  the module and then outward to the nearest module that has the
  variant, as clause (3) finds items, and clause (4b) looks among
  imported enums only when (4) finds none. Two imported enums with the
  same variant are still ambiguous where the name is used.

## Rejected alternatives

- **2:** two names where one does both, and joining makes a temporary
  `String` per piece. `to_string(x)` is `String::new()` plus one
  `append`, and can be added later.
- **3, 6:** many names, each repeating part of one rule; a program
  still converts between widths itself.
- **5:** leaves floats to programs, and correct float parsing is the
  hard part of the two.
- **Trimming whitespace:** `" 42"`, `"4 2"` and `"42\r"` would each
  need a rule, and `Invalid`'s offset would no longer point into the
  text as written. A program trims what it wants to.
- **`parse` from a `str`:** makes the argument generic too, for
  literals, which a program rarely parses. `String::from_str` makes one.
- **Other bases, `_` separators, `0x`:** can be added as separate
  functions if needed.
- **For (f): renaming `ParseError`'s variants:** any name can collide
  with some program, and each future `std` enum would meet the same
  problem.
- **For (f): always qualifying variants:** removes ambiguity, but
  `Some`, `None`, `Ok` and `Err` would need `Option::`/`Result::` or an
  exception, and every program changes. A separate decision if wanted.

## Semantic rationale

`append` is `Vec::push` of each byte of `text(x)`: no new state, and
`text` is `print`'s. `parse` reads a `String`'s bytes and produces a
value; its result is a checked value of `T`, never a claim, since the
bytes are the program's own (already UTF-8, `inv.string.utf8-validity`).

## Usability

    import std;

    fn main() : u8
    {
        if (arg_count() != 2)
        {
            print("usage: add A B\n");
            return 2;
        }
        auto a = match (arg(0))
        {
            Ok(s) : s,
            Err(_) : String::new(),
        };
        auto b = match (arg(1))
        {
            Ok(s) : s,
            Err(_) : String::new(),
        };
        match (parse<i64>(&a))
        {
            Ok(x) : match (parse<i64>(&b))
            {
                Ok(y) :
                {
                    String line = String::new();
                    String::append(&mut line, x);
                    String::append(&mut line, " + ");
                    String::append(&mut line, y);
                    String::append(&mut line, " = ");
                    String::append(&mut line, x + y);
                    String::append(&mut line, "\n");
                    print(&line);
                    0
                },
                Err(_) : 1,
            },
            Err(_) : 1,
        }
    }

## Explainability

"`String::append(&mut s, x)` adds what `print(x)` would write;
`parse<T>(&s)` reads a whole number of type `T`, or says why not" is
one sentence.

## Implementation-feasibility

Two resolver bugs surfaced and were fixed with (f): a qualified
`E::V` whose `V` another enum also has was dispatched by the bare name,
and a generic associated function of a non-generic type
(`fn W::put<T>(ref<W, exclusive> w, T x)`) could not infer `T`.

Both implementations realize `append` as `print` is realized: `str`
and `String` through code in `std`, numbers and `bool` through the
formatting `print` already uses. `parse` scans the bytes and converts
them with the host's correctly rounded conversion; the scanner is one
Rust source file shared by the interpreter and the compiler's runtime.

## Compatibility impact

Extension. A program's own `parse` or `ParseError` takes precedence
over `std`'s (D-0024), and its own `Empty`, `Invalid` or `OutOfRange`
variants over `ParseError`'s, by (f). (f) changes no program that was
accepted before: it only resolves names that were ambiguous.

## Prior-art status

- **Rust:** `str::parse::<T>()` → `Result<T, ParseIntError>` (kinds
  `Empty`, `InvalidDigit`, `PosOverflow`, `NegOverflow`); no whitespace;
  `+` allowed; `write!(s, "{}", x)` / `s.push_str(&x.to_string())`.
- **Go:** `strconv.Atoi`, `ParseInt(s, base, bits)`, `ParseFloat`;
  errors `ErrSyntax`, `ErrRange`; `strconv.AppendInt(buf, x, 10)`.
- **Zig:** `std.fmt.parseInt(T, s, base)`, `parseFloat`; errors
  `InvalidCharacter`, `Overflow`.
- **C:** `strtol`/`strtod` skip leading whitespace and stop at the
  first bad byte, reporting where; `snprintf` to build text.

## Invariant traceability

`inv.string.utf8-validity`: `append` adds only UTF-8 (`text` is ASCII,
or a `str`'s or `String`'s bytes), so the `String` stays valid.
`parse` does not change any invariant.

## Revisit conditions

- Other bases, digit separators, or leading/trailing whitespace.
- Parsing from a `str` or from bytes.
- Fixed-precision or padded formatting (still left to programs,
  D-0028).
- A `to_string` convenience.
