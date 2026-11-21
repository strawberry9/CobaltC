# Errors

There are exactly two ways for something to go wrong in a CobaltC program,
and they never mix. A **fault** is the language catching a broken rule: an
overflow, an index out of range, a reference used after its target ended.
It is fatal. An **error value** is your program reporting that something it
expected might fail did fail: a file was missing, the input was malformed.
It is data, returned to the caller, who decides what to do. There are no
exceptions and nothing to catch. This chapter is about both, and about
telling them apart.

## A fault unwinds, then stops the program

```
import std;

resource struct Session
{
    u32 id;
}

fn Session::drop(ref<Session, exclusive> self)
{
    printf("session %v closed\n", self.id);
}

fn average(ref<Vec<u32>, shared> v) : u32
{
    u32 sum = 0;
    foreach (x in v)
    {
        sum += *x;
    }
    sum / narrow<u32>(Vec::len(v))    // ✗ divides by zero when v is empty
}

fn main()
{
    Session s = Session { .id = 7 };
    Vec<u32> empty = Vec::new();
    printf("average %v\n", average(&empty));
    printf("not reached\n");
}
```

```
session 7 closed
fault[diag.div-by-zero]: division by zero
  --> fault_unwinds.cb:20:5
   |
20 |     sum / narrow<u32>(Vec::len(v))    // ✗ divides by zero when v is empty
   |     ^^^
   = repair: test first, or `checked_div`
```

When a check fails, the thread *unwinds*: every open scope is left in order,
each destroying what it owns, exactly as if the function had returned. The
session's destructor ran, so it was closed, and the program then stopped and
reported the fault. Nothing in the program can intercept this. The point is
that a program which reaches its end has never once broken a checked rule,
and a program that breaks one leaves nothing half-done behind it: files are
closed, locks released, memory freed.

Every fault has a name, `diag.div-by-zero` here, and the registry entry that
`--explain` prints. Appendix B lists the ones you are likely to meet.

A fault in a thread you started ends only that thread, and its failure is
handed to whoever waits for the thread as a value. Chapter 13 covers that;
in a single-threaded program, a fault ends the program.

## An error is a value: `Result` and `?`

When failure is part of the job, a function says so in its return type.
`Result<T, E>` is an enum with two variants: `Ok(value)` and `Err(error)`.
The error type is yours to choose, and an enum that names each way the
function can fail is usually the right one:

```
import std;

enum ParseError
{
    Empty,
    BadDigit(usize),                  // where the bad character was
    TooLarge,
}

fn parse_number(StringView s) : Result<u32, ParseError>
{
    slice<u8, shared> bytes = StringView::as_bytes(s);
    if (slice_len(bytes) == 0)
    {
        return Err(Empty);
    }
    u32 value = 0;
    foreach (i in 0..slice_len(bytes))
    {
        u8 c = bytes[i];
        if (!ascii_is_digit(c))
        {
            return Err(BadDigit(i));
        }
        u32 digit = widen<u32>(c - b'0');
        value = Option::ok_or(checked_mul(value, 10), TooLarge)?;   // None becomes Err(TooLarge)
        value = Option::ok_or(checked_add(value, digit), TooLarge)?;
    }
    Ok(value)
}

// Two parses and a sum: either one's failure is this function's failure.
fn add_numbers(StringView a, StringView b) : Result<u32, ParseError>
{
    u32 x = parse_number(a)?;         // Err returns from add_numbers right here
    u32 y = parse_number(b)?;
    Option::ok_or(checked_add(x, y), TooLarge)
}

fn report(StringView a, StringView b)
{
    match (add_numbers(a, b))
    {
        Ok(v)            : printf("%v + %v = %v\n", a, b, v),
        Err(Empty)       : printf("%v + %v: empty input\n", a, b),
        Err(BadDigit(i)) : printf("%v + %v: not a digit at position %v\n", a, b, i),
        Err(TooLarge)    : printf("%v + %v: too large for a u32\n", a, b),
    }
}

fn main()
{
    report("1200", "34");
    report("12x4", "1");
    report("7", "");
    report("4000000000", "300000000");
}
```

```
1200 + 34 = 1234
12x4 + 1: not a digit at position 2
7 + : empty input
4000000000 + 300000000: too large for a u32
```

The `?` operator is what keeps this readable. Written after an expression of
type `Result`, it takes the `Ok` value out, and if the expression is an `Err`
it returns that `Err` from the enclosing function at once. So
`u32 x = parse_number(a)?;` reads "the number, or give up with its error".
The enclosing function must return a `Result` with the same error type, which
is why `add_numbers` and `parse_number` share `ParseError`; where two error
types meet, `Result::map_err` converts one into the other at the boundary.

The three functions take their text as `StringView`, so they read the
literals `main` passes as readily as a line read from a file (`&line`).
Chapter 10 says when a parameter is a `str` instead, which is seldom.

The standard library's parsers have an error type of their own:
`StringView::parse<T>` returns `Result<T, ParseError>`, where this
`ParseError` is `std`'s (the example above declared an enum of the same name,
which a program's own declaration hides). A function with its own error type
converts it with `map_err`, whose second argument is a closure taking the
error, its parameter written with its type as every closure parameter is:

```
import std;

enum ConfigError
{
    Missing,
    BadPort(String),                  // the text that was not a port number
}

fn port_of(StringView text) : Result<u16, ConfigError>
{
    if (StringView::len(text) == 0)
    {
        return Err(Missing);
    }
    // `StringView::parse` fails with std's own `ParseError`; `map_err`
    // turns it into this function's error, so `?` can return it.
    u16 port = Result::map_err(StringView::parse<u16>(text), [text](ParseError e) : ConfigError { BadPort(String::copy(text)) })?;
    Ok(port)
}

fn main()
{
    foreach (t in ["8080", "", "80x", "70000"])
    {
        match (port_of(t))
        {
            Ok(p)           : printf("%v: port %v\n", t, p),
            Err(Missing)    : printf("(empty): missing\n"),
            Err(BadPort(s)) : printf("%v: not a port\n", &s),
        }
    }
}
```

```
8080: port 8080
(empty): missing
80x: not a port
70000: not a port
```

`?` works on an `Option` too, inside a function that returns an `Option`:
`Some` gives the value, `None` returns `None`. Nothing converts between the
two on its own, so `Option::ok_or(o, e)` is how an `Option` becomes a
`Result`, as the checked arithmetic above shows: `checked_mul` returns
`None` on overflow, `ok_or` turns that into `Err(TooLarge)`, and `?` returns
it.

In `report`, a `match` handles every case, with nested patterns reaching
inside the `Err`. The caller of a fallible function always sees the whole set
of outcomes, and the compiler holds it to handling them.

## An error cannot be dropped by accident

A `Result` that is computed and not used is an error before the program
runs:

```
import std;

fn save(StringView name) : Result<void, str>
{
    if (StringView::len(name) == 0)
    {
        return Err("no name");
    }
    Ok(())
}

fn main()
{
    save("");                         // ✗ the error would vanish unseen
}
```

```
error[diag.result-discarded]: this `Result` is discarded, so an error it carries would be lost: handle it (`?`, `match`), or discard it on purpose with `_ = …;`
  --> discarded.cb:14:10
   |
14 |     save("");                         // ✗ the error would vanish unseen
   |          ^^
   = repair: handle the error (`?`, `match`), or discard the value on purpose with `_ = e;`
```

Handle it with `?` or `match`, or say that you mean to ignore it:
`_ = save("");`. A function that returns nothing but may fail returns
`Result<void, E>`, and `Ok(())` is its success value; `()` is the value of
type `void`.

Three more functions round out the set. `Result::unwrap_or(r, d)` and
`Option::unwrap_or(o, d)` give the value or a default. `Result::unwrap(r)`,
`Option::unwrap(o)`, and `expect`, which takes a message, give the value or
fault. Use the last two only where the other case is impossible by
construction, because what they do with it is turn an error into a fault.

## Assertions

`assert(condition)` states something the program relies on at that point,
and faults if it is false. A message, formatted as `printf` formats, says
which values were wrong:

```
import std;

fn median(ref<Vec<i32>, shared> sorted) : i32
{
    assert(Vec::len(sorted) > 0, "median of an empty list");
    sorted[Vec::len(sorted) / 2]
}

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 1);
    Vec::push(&mut v, 5);
    Vec::push(&mut v, 9);
    printf("%v\n", median(&v));
    usize limit = 2;
    assert(Vec::len(&v) <= limit, "%v values, but the limit is %v", Vec::len(&v), limit);
    printf("not reached\n");
}
```

```
5
fault[diag.assert-failed]: 3 values, but the limit is 2 (3 <= 2)
  --> assert.cb:17:5
   |
17 |     assert(Vec::len(&v) <= limit, "%v values, but the limit is %v", Vec::len(&v), limit);
   |     ^^^^^^
   = repair: the program's own condition names what was assumed; find why it did not hold
```

The condition is always evaluated; there is no build that leaves assertions
out. The message and its arguments are evaluated only when the condition is
false, so a passing assertion costs its test and nothing more. When the
condition compares two numbers, the failure also shows both sides, `(3 <=
2)` here, and an assertion without a message says `assertion failed: 3 <= 2`.
`static_assert`, from chapter 4, is the form for a condition the compiler
can evaluate, and it rejects the program instead of stopping it.

## Which one to use

The two mechanisms answer two different questions, and the choice is made by
asking which question you are answering.

Could this fail when the program is correct? Then the failure is part of the
function's job, and it belongs in the return type. Bad input, a missing
file, a full queue, a connection refused: all `Result`. A value that may be
absent for a good reason: `Option`.

Could this fail only if the program has a bug? Then let the fault happen. An
index you computed yourself, arithmetic on values you already checked, a
state the code's own logic should make impossible: an `assert` where the
assumption lives, and otherwise the language's own checks. Wrapping these in
a `Result` would only pass a bug on to a caller who can do nothing about it.

Two conventions follow. Keep one error type across a chain of calls, so `?`
flows, and convert at the boundaries. And trust the unwinding: a fault runs
your destructors, so a function need not clean up before a check that might
fail.
