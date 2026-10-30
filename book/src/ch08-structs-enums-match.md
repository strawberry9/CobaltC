# Structs, enums and match

CobaltC has two ways to build a type from others. A **struct** holds several
values at once, each under a name. An **enum** holds one of several
alternatives, each of which may carry a value. Together they describe almost
any data a program has, and `match`, which takes an enum apart, is the
language's way of making sure every alternative is handled.

## Structs

```
import std;

struct Point
{
    f64 x;
    f64 y;
}

struct Segment
{
    Point from;                       // structs nest by value
    Point to;
}

// A function that belongs with the type: called as Segment::length(...).
fn Segment::length(ref<Segment, shared> s) : f64
{
    f64 dx = s.to.x - s.from.x;
    f64 dy = s.to.y - s.from.y;
    sqrt(dx * dx + dy * dy)
}

fn Point::origin() : Point
{
    Point { .x = 0.0, .y = 0.0 }
}

fn main()
{
    Point a = Point { .x = 3.0, .y = 4.0 };     // every field, once, in any order
    Segment s = Segment { .from = Point::origin(), .to = a };
    s.to.x = 6.0;                                 // nested fields are places
    printf("%v\n", Segment::length(&s));
    printf("%v\n", a == Point { .y = 4.0, .x = 3.0 });   // plain structs compare field by field
}
```

```
7.211102550927978
true
```

A struct literal names every field, with a dot before each name, in any
order. There is no way to build a struct with some fields missing: an
aggregate is initialised whole, and after that its fields are ordinary
places you can assign to. A struct of plain fields is plain, copied on
assignment and comparable with `==`; a struct with a resource inside is a
resource, as chapter 6 said.

A function written `fn Type::name(...)` is an *associated function* of the
type. It is an ordinary function with a qualified name, called as
`Segment::length(&s)`, and nothing about it is special except where it
lives. There is no `this`, no implicit receiver, and no method-call syntax:
`s.length()` does not exist. A function that reads the struct takes
`ref<Segment, shared> s`; one that changes it takes an exclusive reference;
one that builds it, like `Point::origin`, takes whatever it needs and
returns the struct.

A struct's fields are private to the module that declares it unless marked
`export`, which chapter 12 covers. Inside one file, everything can see
everything.

## Enums

```
import std;

enum Shape
{
    Circle(f64),                      // a payload: the radius
    Square(f64),                      // the side
    Dot,                              // no payload
}

fn area(Shape s) : f64
{
    match (s)
    {
        Circle(r) : 3.14159 * r * r,  // the payload is bound to r for this arm
        Square(w) : w * w,
        Dot       : 0.0,
    }
}

fn describe(Shape s) : str
{
    match (s)
    {
        Circle(_) : "round",          // _ ignores the payload
        _         : "not round",      // a catch-all arm
    }
}

fn main()
{
    array<Shape, 3> shapes = [Circle(1.0), Square(2.0), Dot];
    foreach (s in shapes)
    {
        printf("%v: %v, area %v\n", s, describe(s), area(s));
    }
}
```

```
Circle: round, area 3.14159
Square: not round, area 4.0
Dot: not round, area 0.0
```

An enum lists its *variants*. A variant may carry one value, its *payload*,
of any type; `Circle(f64)` carries a radius, `Dot` carries nothing. A value
of the enum is exactly one variant with its payload, built by naming the
variant: `Circle(1.0)`, `Dot`. Variant names can be used bare, as here, or
qualified as `Shape::Dot` when two enums in scope share a name. Printed with
`%v`, an enum shows its variant's name.

This is C's `enum` and `union` folded into one safe type, and it is the way
to say "one of these" in CobaltC: a token in a parser, a message from a
thread, a cell that is empty or full, a result that is a value or an error.

## `match`

`match` is how an enum is taken apart. It compares the value with each arm's
*pattern* in order and runs the first that fits. Every arm ends with a
comma; an arm's body is an expression, or a block when it needs statements.
Because `match` is an expression, `area` above simply is its `match`.

A `match` must handle every variant, or end with the catch-all `_`:

```
import std;

enum Light
{
    Red,
    Amber,
    Green,
}

fn go(Light l) : bool
{
    match (l)                         // ✗ Amber is not handled
    {
        Red   : false,
        Green : true,
    }
}

fn main()
{
    printf("%v\n", go(Green));
}
```

```
error[diag.non-exhaustive-match]: not covered: `Amber`
  --> non_exhaustive.cb:12:5
   |
12 |     match (l)                         // ✗ Amber is not handled
   |     ^^^^^
   = repair: add an arm for the pattern named, or a `_` arm
```

This check is the reason to prefer an enum over a set of integer codes. Add a
variant to the enum, and every `match` that fails to handle it stops
compiling until you decide what it should do there. Use `_` sparingly for the
same reason: a catch-all arm silently absorbs the variants you add later.

## `Option`: a value or nothing

The standard library's most used enum has two variants, `Some(value)` and
`None`. It is how CobaltC says "maybe": there is no null, so a function that
may have nothing to return returns an `Option`.

```
import std;

// The position of the first negative number, if there is one.
fn first_negative(ref<Vec<i32>, shared> v) : Option<usize>
{
    foreach (i, x in v)
    {
        if (*x < 0)
        {
            return Some(i);
        }
    }
    None
}

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 4);
    Vec::push(&mut v, -2);
    Vec::push(&mut v, 7);

    match (first_negative(&v))
    {
        Some(i) : printf("first negative at %v\n", i),
        None    : printf("none negative\n"),
    }

    if (Some(i) = first_negative(&v))         // a pattern as a condition
    {
        printf("found one: v[%v] is %v\n", i, v[i]);
    }

    Vec<i32> empty = Vec::new();
    printf("%v\n", Option::unwrap_or(first_negative(&empty), 999));   // a default instead
}
```

```
first negative at 1
found one: v[1] is -2
999
```

The `if` with a pattern, `if (Some(i) = e)`, is a `match` with one
interesting arm: when the pattern fits, the block runs with `i` bound, and
otherwise it is skipped (or an `else` runs). The same works in a `while`,
as chapter 3's input loop showed. These are the forms to use when only one
case needs work; `match` is the form that makes you handle every case.

`Option::unwrap_or(o, d)` gives the value inside, or `d` if there is none.
`Option::unwrap(o)` gives the value and faults if there is none; use it only
where `None` would be a bug. The next chapter covers these and the `?`
operator together with `Result`, the enum for errors.

## Patterns

A pattern can be a variant with a pattern for its payload, as deep as needed;
a literal; a name, which binds; or `_`, which ignores:

```
import std;

fn classify(Result<Option<i32>, str> r) : str
{
    match (r)
    {
        Ok(Some(0)) : "zero",                     // a literal inside a pattern
        Ok(Some(n)) : if (n > 0) { "positive" } else { "negative" },
        Ok(None)    : "nothing",
        Err(_)      : "an error",
    }
}

fn opcode(u8 b) : str
{
    match (b)                         // an integer: literal arms and a catch-all
    {
        0x01 : "load",
        0x02 : "store",
        _    : "unknown",
    }
}

fn is_quit(str command) : bool
{
    match (command)                   // text: literal arms too
    {
        "quit" : true,
        "exit" : true,
        _      : false,
    }
}

fn main()
{
    printf("%v %v %v %v\n", classify(Ok(Some(0))), classify(Ok(Some(-5))), classify(Ok(None)), classify(Err("bad")));
    printf("%v %v\n", opcode(2), opcode(9));
    printf("%v %v\n", is_quit("exit"), is_quit("help"));
}
```

```
zero negative nothing an error
store unknown
true false
```

A `match` on an integer, a byte or text is CobaltC's `switch`. Since
literals can never list every integer, such a `match` needs a `_` arm; a
`bool` is covered by `true` and `false`. An arm that could never run, such
as a literal repeated from an earlier arm or a variant already fully
covered, is an error, so a `match` never has dead arms.

There are no range patterns and no `a | b` alternatives. For a range, match
on the test: `match (v >= 10 && v < 15) { true : ..., false : ... }`. For
several values with one action, give each its own arm and put the action in
a function. The language keeps patterns to the four forms above, so there is
nothing more to learn about them.

## Matching a resource, by value or by reference

When an enum's payload is a resource, an arm that binds the payload *moves*
it out, and the enum is consumed. To look inside without taking it apart,
match a reference to the enum:

```
import std;

enum Message
{
    Text(String),
    Ping,
}

fn length(ref<Message, shared> m) : usize
{
    match (m)                         // through a reference: the payload is borrowed, not moved
    {
        Text(s) : String::len(s),
        Ping    : 0,
    }
}

fn main()
{
    Message m = Text(String::from_str("hello"));
    printf("%v\n", length(&m));
    printf("%v\n", length(&m));       // m is still here

    match (&mut m)                    // an exclusive reference: change the payload in place
    {
        Text(s) : String::push_ascii(s, b'!'),
        Ping    : {},
    }
    match (m)                         // by value: the Text arm takes the String, and m is consumed
    {
        Text(s) : printf("%v\n", &s),
        Ping    : printf("ping\n"),
    }
}
```

```
5
5
hello!
```

Matching `&m` binds each payload as a shared reference, and matching
`&mut m` binds it as an exclusive one, through which it can be changed. A
parameter that is already a reference, like `m` in `length`, binds in its
own mode without any `&`. Matching the value itself, in the last `match`,
moves the `String` into `s` for the arm's duration, after which `m` is
stale. These are the same rules as chapter 7's, applied to a payload: look
through a reference, take by value.

## Taking a struct apart

A struct can be taken apart into its fields with the same brace syntax,
without dots:

```
import std;

struct Entry
{
    String key;
    i64 value;
}

fn main()
{
    Entry e = Entry { .key = "answer", .value = 42 };
    Entry { key, value } = e;         // take it apart: key and value are now locals, e is consumed
    printf("%v = %v\n", &key, value);
}
```

```
answer = 42
```

Every field is named, or `..` stands for the rest, and `field : name` binds a
field under another name. This is how a struct gives up a resource it holds,
since a resource field cannot be moved out of a struct that lives on. The
same form works inside a `match` arm, `Some(Entry { key, value })`, to take
apart a struct that is an enum's payload.

## Two more kinds of aggregate

A fixed-size **array**, `array<T, N>`, holds exactly `N` elements, with `N`
known when the program is compiled; `[1, 2, 3]` is a literal and `[0; 256]`
is 256 zeros. Indexing is bounds-checked. An array of plain elements is
plain, and copied on assignment. Chapter 10 covers arrays alongside
vectors and slices, which is where they are mostly used.

A **bitstruct** packs named runs of bits into one unsigned integer,
`bitstruct Flags : u8 { carry : 1; zero : 1; pad : 6; }`, for file formats,
instruction encodings and hardware registers. Its fields read and write like
any struct's, with a check that the value fits the field's width, and
`Flags::bits(f)` is the whole integer. The language guide has the details;
most programs never need one.
