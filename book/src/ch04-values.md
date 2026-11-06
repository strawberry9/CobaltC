# Values, types and variables

This chapter covers the ground every program stands on: how a variable is
declared, what types there are, how a number gets its type, what happens when
arithmetic does not fit, and how a value is converted from one type to
another. Most of it will look familiar from C. The differences are small and
deliberate, and they are the ones to attend to.

## Declaring a variable

A declaration puts the type first, then the name, then an initialiser:

```
import std;

fn main()
{
    i32 count = 3;                    // the type first, then the name
    f64 ratio = 0.75;
    bool ready = count > 2;
    str label = "items";
    auto doubled = count * 2;         // auto: the type of the initialiser, i32 here

    count = count + 1;                // any variable can be assigned
    printf("%v %v: %v, ratio %v, ready %v\n", doubled, label, count, ratio, ready);

    i32 sign;                         // declared without a value...
    if (ratio < 0.5)
    {
        sign = -1;
    }
    else
    {
        sign = 1;
    }
    printf("sign %v\n", sign);        // ...and assigned on every path before it is read
}
```

```
6 items: 4, ratio 0.75, ready true
sign 1
```

`auto` asks the language to take the type from the initialiser. It works
whenever the initialiser has a type of its own, which is almost always; the
exception, an empty container whose element type nothing has fixed yet, comes
up in chapter 10.

Every variable can be assigned. There is no `mut`, no `const` qualifier on a
local, no read-only binding. That may seem a loss if you come from Rust, but
CobaltC does not need the distinction for safety: what it controls is not
whether a variable *may* change but who can reach it, and that is the subject
of chapter 7. To see whether `x` can change, look for `x =` and `&mut x` in
its function, and nowhere else.

A variable declared without a value must be given one on every path before
it is read. The check is made before the program runs, from the shape of the
code alone:

```
import std;

fn main()
{
    i32 sign;
    bool negative = false;
    if (negative)
    {
        sign = -1;
    }
    printf("%v\n", sign);             // ✗ no value when `negative` is false
}
```

```
error[diag.use-of-uninitialized]: `sign` is read here, but on some path to here it was never given a value; give it one where it is declared (`… sign = …;`) or on every path before this
  --> uninitialised.cb:11:20
   |
11 |     printf("%v\n", sign);             // ✗ no value when `negative` is false
   |                    ^^^^
   = repair: initialize on every path; initialize aggregates whole
```

The checker does not know that `negative` is false; it knows only that one
path writes `sign` and one does not. Reading uninitialised storage, which is
undefined behaviour in C, cannot happen in a CobaltC program that the tools
accept.

A name is declared once per block. A second `i32 count` further down the same
block is an error; to change the variable, assign to it. A block *inside*
may declare its own `count`, which hides the outer one until the inner block
ends, as in C.

## The types

| Type | Values |
|---|---|
| `i8` `i16` `i32` `i64` `i128` | signed integers, two's complement |
| `u8` `u16` `u32` `u64` `u128` | unsigned integers |
| `isize` `usize` | integers the width of an address; `usize` is the type of every length and index |
| `f32` `f64` | IEEE 754 floating point |
| `bool` | `true` or `false` |
| `str` | text written in the program, valid UTF-8, copied like a number |
| `void` | the type of nothing; a function with no return type returns it |

There is no `char` type and no `int`: every integer names its width, and a
single byte is written `b'a'`, which is a `u8` holding the ASCII code of `a`.
Structs, enums, arrays, vectors, references and the rest are built from these
and come in later chapters.

Types never convert into each other on their own. `i32` and `i64` do not mix
in an expression, a `u8` cannot be passed where an `i32` is expected, and an
integer is never silently a float. This is the first of the small differences
from C, and it removes a whole family of surprises: a value's type is whatever
it was declared to be, everywhere it goes.

## How a literal gets its type

A number written in the program has no type of its own until its position
gives it one:

```
import std;

fn main()
{
    auto a = 5;                       // i32: nothing says otherwise
    u8 b = 200;                       // u8: from the declared type
    auto c = 7: u64;                  // u64: from the suffix
    auto d = b + 1;                   // u8: the 1 takes b's type
    u64 ns = 1_000_000_000;           // underscores group digits
    u32 mask = 0xFF00;                // hexadecimal; 0o17 and 0b1010 also work
    auto e = 2.5;                     // f64
    f32 f = 0.5;                      // f32: from the declared type
    u8 comma = b',';                  // a byte: the ASCII code of a comma, 44
    printf("%v %v %v %v %v %v %v %v %v\n", a, b, c, d, ns, mask, e, f, comma);
}
```

```
5 200 7 201 1000000000 65280 2.5 0.5 44
```

The rule, in order: a suffix such as `7: u64` wins; otherwise the *expected
type* at that position, which may be a declared variable type, a parameter
type, a field type, or the type of the other operand of the same operator;
otherwise the default, `i32` for an integer and `f64` for a float. A literal
that does not fit the type it is given, such as `256` where a `u8` is
expected, is an error before the program runs.

So the one flexible thing in the type system is a bare literal, and it is
flexible only once, at the place it is written. `takes_i64(5)` is fine; with
`i32 x = 5;` beforehand, `takes_i64(x)` is not, because `x` is an `i32` for
good.

## Arithmetic is checked

In C, `a + b` on integers that overflow wraps round or, for signed types, is
undefined. In CobaltC an overflow is a fault: the program stops, as you saw in
chapter 2. The same holds for `-` and `*`, for division by zero, and for the
one division that overflows, the most negative value divided by `-1`. When
both operands are literals the tools catch it before the run; otherwise the
check happens as the operation runs.

When you *want* another behaviour, you ask for it by name:

```
import std;

fn main()
{
    u8 level = 250;
    printf("%v\n", wrapping_add(level, 10));      // 4: wraps round, on purpose
    printf("%v\n", saturating_add(level, 10));    // 255: stops at the top

    match (checked_add(level, 10))                // None: the sum does not fit
    {
        Some(v) : printf("sum %v\n", v),
        None    : printf("too big for a u8\n"),
    }

    i32 total = 0;
    foreach (i in 1..101: i32)
    {
        total += i;                               // checked: a fault if it ever overflowed
    }
    printf("%v\n", total);
}
```

```
4
255
too big for a u8
5050
```

`wrapping_add`, `wrapping_sub` and `wrapping_mul` wrap round. The
`saturating_` family stops at the type's limits. The `checked_` family,
which also has `checked_div`, `checked_rem` and `checked_neg`, returns an
`Option`: `Some(result)` when the operation fits and `None` when it does not,
so the caller decides. Chapter 8 explains `Option` and `match`; for now, read
the `match` as "if there was a sum, print it, otherwise say so".

The choice is deliberate. A hash function wants wrapping; a volume control
wants saturation; a parser wants to report a number that is too large; and
ordinary code, where overflow is a bug, wants to be stopped. Each of these is
written differently, so the reader can tell them apart.

Floating-point arithmetic never faults. Division by zero gives an infinity,
and the undefined cases give `NaN`, as IEEE 754 says; `NaN` compares unequal
to everything, itself included.

Shifts, the bitwise operators `& | ^ ~`, and the comparisons work as in C,
with two small exceptions. A shift by the type's width or more is a fault,
not a silent zero. And the bitwise operators bind tighter than comparison, so
`x & 1 == 0` means `(x & 1) == 0`, which is what the author almost always
intended; in C it means the opposite.

## Conversions are explicit

Since nothing converts on its own, there are named functions for each kind of
conversion, and each says what it does when the value does not fit:

```
import std;

fn main()
{
    u8 small = 200;
    i32 wide = widen<i32>(small);                 // always fits: no check needed
    i64 wider = widen<i64>(wide) * 100_000_000;
    printf("%v %v\n", wide, wider);

    usize count = 12;
    i32 n = narrow<i32>(count);                   // checked: faults if it did not fit
    printf("%v\n", n);

    f64 half = to_float<f64>(n) / 2.0;
    i32 back = to_int<i32>(half);                 // toward zero: 6
    printf("%v %v\n", half, back);

    u32 bits = reinterpret<u32>(-1: i32);         // the same bit pattern, read as unsigned
    printf("%v\n", bits);

    u8 byte = narrow<u8>(wider);                  // ✗ 20000000000 does not fit a u8
    printf("%v\n", byte);
}
```

```
200 20000000000
12
6.0 6
4294967295
fault[diag.narrowing-overflow]: the value does not fit the narrower type
  --> conversions.cb:21:5
   |
21 |     u8 byte = narrow<u8>(wider);                  // ✗ 20000000000 does not fit a u8
   |     ^^
   = repair: `narrow_wrapping`, or widen the target
```

The target type is written in angle brackets. `widen` is accepted only where
every value of the source fits the target, so it can never lose anything and
never checks; `narrow` works between any two integer types and faults when
the value does not fit; `narrow_wrapping` keeps the low bits and never
faults; `checked_narrow` returns an `Option`. `to_float` and `to_int` go
between integers and floats, `to_int` truncating toward zero and faulting on
an infinity, a `NaN` or a value out of range. `reinterpret` keeps the bits
and changes their reading, between the signed and unsigned type of one width
or between a float and the integer of its width.

Two habits make this painless. Reach for `widen` first, since a conversion
that cannot fail is the common case. And for lengths and indices use `usize`
throughout, so that nothing needs converting until a value meets the outside
world.

## Constants

A `const` names a value known before the program runs:

```
import std;

const u32 WIDTH = 80;
const u32 HEIGHT = 24;
const u32 CELLS = WIDTH * HEIGHT;                 // computed before the program runs

fn main()
{
    static_assert(CELLS == 1920, "the screen has 1920 cells");
    static_assert(sizeof<u32>() == 4);

    u32 used = CELLS / 3;                         // a copy of the value: an ordinary variable
    used += 1;
    printf("%v of %v cells\n", used, CELLS);
}
```

```
641 of 1920 cells
```

A constant's initialiser may use literals, other constants, operators and a
few built-in functions such as `sizeof`. It is computed when the program is
compiled, and an overflow in computing it is an error at the constant.
`static_assert` checks a condition at the same moment, so a program with a
false one never runs at all; its condition must be constant too. Both can
also be written inside a function, where the constant's name is scoped like a
local's.

This is `const` as in Rust or `constexpr` in C++: a value fixed before the
run, not a variable set once. A value that only becomes known while the
program runs is an ordinary variable.

## Where this leaves you

Declare with the type first, or `auto` when the initialiser says enough.
Expect no conversions and write the one you mean. Expect arithmetic to stop
the program rather than wrap, and name the policy when you want another. Use
`usize` for sizes. With that, the next chapter can turn to functions and
control flow.
