from gen import Section, p, h3, h4, ul, ol, table, note, rules, code

CHECK = """
// Fault deliberately when a check fails: a checked division by a
// run-time zero. Used throughout this guide as a tiny assertion.
fn check(bool ok)
{
    i32 z = if (ok)
    {
        1
    }
    else
    {
        0
    };
    1 / z;                            // diag.div-by-zero when ok is false
}
"""

# The same, for a program that also uses the standard library.
CHECK_STD = "\nimport std;\n" + CHECK

# ================================================================ §06

b = ""
b += p("""Numbers are where silent corruption usually starts, so CobaltC's arithmetic is
<em>checked by default</em>: an operation whose mathematical result does not fit its type does not
wrap around, it faults. Every other policy (wrapping, saturating, or returning `None`) is a named
function you call on purpose. Types never convert implicitly; every change of representation is a
named intrinsic with a documented behaviour.""")

b += h3("Numeric types")
b += table(["Type", "Meaning", "Notes"], [
    ["`i8` `i16` `i32` `i64` `i128`", "signed integers of the given width", "two's complement"],
    ["`u8` `u16` `u32` `u64` `u128`", "unsigned integers of the given width", ""],
    ["`isize` `usize`", "signed / unsigned integers of address width", "the types of indices, lengths, sizes and pointer offsets; the width (16, 32, 64 or 128 bits) is fixed and documented by each implementation"],
    ["`f32` `f64`", "IEEE 754 binary32 / binary64", "`±Inf` and `NaN` are ordinary values; float arithmetic never faults"],
    ["`bool`", "`true` or `false`", "produced by comparisons and `&& || !`"],
])
b += p("""There is no implicit promotion: `i32 + i64` is a type error, and a `u8` cannot be passed
where an `i32` is expected. Unary minus applies only to signed and float types.""")

b += h3("Literals")
b += p("""Integer literals are digit strings: decimal, or hexadecimal, octal or binary after `0x`,
`0o` or `0b` (`0xFF`, `0o755`, `0b1010`), with no sign. A `_` between two digits groups them and
is ignored: `1_000_000`, `0xFFFF_0000`, `3.141_592` (anywhere else, a `_` is a syntax error). A literal's
type comes from, in order: an explicit suffix `123: u8`; the <em>expected type</em> at that position
(a declared variable type, a parameter type, a field type, or the other operand of the same
operator); otherwise the default, `i32` for integers and `f64` for floats. An expression made only
of literals and operators takes the expected type as a whole, so `u64 x = 1024 * 1024 * 1024 * 16;`
is computed in `u64`, while `auto y = 65536 * 65536;` has no expected type, is an `i32`, and
overflows. A literal that does not
fit its type is a static error. A minus written directly before an integer literal is checked
with it, as one negative value, so `-2147483648` is the smallest `i32`. A type's limits also
have names, `min_value<i32>()` and `max_value<i32>()` (below). Float literals are `digits . digits`, with an
optional exponent (`1.5e-3`, `2.5E3`, or `1e5` with no fraction), optionally suffixed `: f32`;
one too large for its type (`1e400`) is a static error, not infinity. A byte character `b'a'`
is always a `u8`: one ASCII character or an escape (`b'\\n'`, `b'\\''`, `b'\\x7f'`), for writing
bytes readably in code that parses. A string literal `"…"` is always a `str` and a byte literal
`b"…"` always an `array<u8, N>` (§21, §22); neither takes an expected type.""")
b += code(CHECK + """
fn main()
{
    auto a = 5;                       // i32: no suffix, no expected type
    u8 b = 200;                       // u8 from the declared type; 200 fits
    auto c = 7: u64;                  // u64 from the suffix
    auto d = b + 1;                   // the 1 takes u8 from b; d is u8
    auto e = 2.5;                     // f64
    f32 f = 0.5: f32;
    f64 tiny = 1.5e-3;                // an exponent: 0.0015
    u8 comma = b',';                  // a byte character: 44
    u32 mask = 0xFF00;                // hexadecimal; also 0o17 (octal) and 0b1010 (binary)
    u64 ns = 1_000_000_000;           // _ groups digits: one second in nanoseconds
    u64 sixteen_gib = 16 * 1024 * 1024 * 1024;   // literals only: computed in u64
    i64 g = -1;                       // the expected type reaches through unary minus
    i32 lo = -2147483648;             // min(i32): 2147483648 alone would not fit
    check(d == 201);
    check(g < 0);
    check(lo + 2147483647 == -1);
    check(tiny == 0.0015 && comma == 44 && mask == 65280);
    check(sixteen_gib == 17_179_869_184 && ns == 1000000000);
    check(sizeof<u64>() == 8);        // sizeof and alignof are intrinsics returning usize
    check(alignof<u16>() == 2);
}
""", title="Literal typing", expect="ok", section="s06", slug="literals")
b += code("""
fn main()
{
    256: u8;                          // ✗ diag.literal-out-of-range (static): 256 does not fit u8
}
""", title="A literal that does not fit", expect="diag.literal-out-of-range", section="s06", slug="literal-oor")

b += h3("Checked arithmetic")
b += p("""`+`, `-` and `*` on integers fault with `diag.arith-overflow` when the result leaves the
type's range. `/` and `%` truncate toward zero (the remainder takes the sign of the dividend);
dividing by zero is `diag.div-by-zero`, and the one overflowing division, the minimum value of a
signed type divided by `-1`, is `diag.div-overflow`. Negating the minimum signed value is an
overflow too. When both operands are literals the fault is detected statically and the program is
rejected; otherwise it is a run-time fault that terminates the program (see §18).""")
b += code("""
fn add(i32 a, i32 b) : i32
{
    a + b                             // checked: faults if the sum leaves i32
}

fn main()
{
    add(2147483647, 1);               // ✗ diag.arith-overflow (dynamic): 2^31 does not fit i32
}
""", title="Overflow at run time", expect="diag.arith-overflow", section="s06", slug="overflow-dyn")
b += code("""
fn main()
{
    2147483647 + 1;                   // ✗ diag.arith-overflow (static): both operands are literals
}
""", title="Overflow the checker can see", expect="diag.arith-overflow", section="s06", slug="overflow-static")
b += code("""
fn div(i32 a, i32 b) : i32
{
    a / b
}

fn main()
{
    div(-2147483647 - 1, -1);         // ✗ diag.div-overflow (dynamic): -MIN does not fit
}
""", title="The one division that overflows", expect="diag.div-overflow", section="s06", slug="div-overflow")

b += h3("Choosing a different overflow policy")
b += p("""When wrapping or clamping is what you actually want, say so. The `wrapping_` and
`saturating_` families never fault; the `checked_` family returns an `Option` so the caller can
decide. These are ordinary generic calls with the operand type inferred from the arguments.""")
b += table(["Call", "Result", "On overflow"], [
    ["`wrapping_add(a, b)`, `wrapping_sub`, `wrapping_mul`", "same type", "wraps modulo 2<sup>N</sup>"],
    ["`saturating_add(a, b)`, `saturating_sub`, `saturating_mul`", "same type", "clamps to the type's minimum or maximum"],
    ["`checked_add(a, b)`, `checked_sub`, `checked_mul`, `checked_div`, `checked_rem`", "`Option<T>`", "`None` (also for division by zero)"],
])
b += code(CHECK_STD + """
fn main()
{
    u8 top = 255;
    check(wrapping_add(top, 1: u8) == 0);          // 255 + 1 wraps to 0
    check(saturating_add(top, 9: u8) == 255);      // clamps at the maximum
    match (checked_add(top, 1: u8))
    {
        Some(_) :                 // must not happen
        {
            check(false);
        },
        None    : {},                              // overflow reported as None
    }
    match (checked_add(1: u8, 1: u8))
    {
        Some(v) :
        {
            check(v == 2);
        },
        None :
        {
            check(false);
        },
    }
}
""", title="Explicit overflow policies", expect="ok", section="s06", slug="alt-families")

b += h3("Shifts, bitwise operators and comparison")
b += ul([
    "`<<` and `>>` take a `u32` shift amount that must be less than the type's bit width; anything else is `diag.shift-amount-out-of-range`. Left shift discards high bits by definition (it is not an overflow); right shift is arithmetic on signed types.",
    "`&`, `|`, `^` and `~` work on any integer type; `!` is boolean not.",
    "`== != < <= > >=` compare two operands of the <em>same</em> type and yield `bool`. Floats follow IEEE rules: every comparison involving `NaN` is false except `!=`.",
])
b += code(CHECK + """
fn main()
{
    u32 one = 1;
    check(one << 4 == 16);
    check(-8 >> 1 == -4);                          // arithmetic shift keeps the sign
    check((12 & 10) == 8);
    check((12 | 3) == 15);
    check((6 ^ 3) == 5);
    check(~0 == -1);
    f64 zero = 0.0;
    auto nan = zero / zero;                        // floats do not fault
    check(!(nan == nan));
    check(nan != nan);
}
""", title="Bits and comparisons", expect="ok", section="s06", slug="bits")

b += h3("Conversions are explicit")
b += p("""There is exactly one way to change a number's type: call the intrinsic that names the
conversion. Each takes the target type as a type argument.""")
b += table(["Intrinsic", "From → to", "Behaviour"], [
    ["`widen<U>(x)`", "integer to a wider integer whose range contains every value; `f32` to `f64`", "always exact, never faults (`u8 → i32`, `i32 → i64`, `f32 → f64`)"],
    ["`narrow<U>(x)`", "any integer to any integer", "checked: `diag.narrowing-overflow` if `x` does not fit"],
    ["`narrow_wrapping<U>(x)`", "any integer to any integer", "keeps the low bits, never faults"],
    ["`reinterpret<U>(x)`", "between the signed and unsigned types of one width (and `usize` ⇄ `isize`); between a float and an integer of its width (`f32` ⇄ `u32`/`i32`, `f64` ⇄ `u64`/`i64`)", "the same bit pattern read as the other type"],
    ["`to_float<U>(x)`", "any integer or float to `f32`/`f64`", "nearest representable value (ties to even); beyond the target's range an infinity, as float arithmetic gives; `NaN` stays `NaN`"],
    ["`to_int<U>(x)`", "float to integer", "truncates toward zero; checked: infinities, `NaN` and out-of-range values fault"],
])
b += code(CHECK + """
fn main()
{
    u8 small = 200;
    i32 wide = widen<i32>(small);                  // always safe
    check(wide == 200);
    check(narrow<u8>(wide) == 200);                // fits, so it succeeds
    check(narrow_wrapping<u8>(300) == 44);         // 300 mod 256
    check(reinterpret<i32>(4294967295: u32) == -1);
    f64 f = to_float<f64>(wide);
    check(to_int<i32>(f * 1.5) == 300);
    usize n = 12;
    check(narrow<i32>(n) == 12);                   // usize → i32 is a narrowing, so it is checked
}
""", title="Named conversions", expect="ok", section="s06", slug="conversions")
b += p("""Between the two float types: `widen<f64>(x)` takes an `f32` to an `f64` exactly, and
`to_float<f32>(x)` takes an `f64` to the nearest `f32`, rounding as float arithmetic does. There
is no `widen<f32>` of an `f64`, since most `f64` values have no exact `f32`. `reinterpret` gives
a float's IEEE-754 bits as an integer of the same width, and turns such an integer back into a
float, with no `unsafe`.""")
b += code(CHECK + """
fn main()
{
    f32 sample = 0.1;
    f64 d = widen<f64>(sample);                    // exact: 0.100000001490116...
    check(to_float<f32>(d) == sample);             // and back, rounded, to the same f32
    check(to_float<f32>(1.0 / 3.0) == 0.33333334: f32);
    u64 bits = reinterpret<u64>(-2.0);
    check(bits >> 63 == 1);                        // the sign bit
    check(reinterpret<f64>(bits) == -2.0);
    check(reinterpret<u32>(1.0: f32) == 0x3f80_0000: u32);
    check(max_value<f32>() == 3.4028235e38: f32);  // the largest finite f32
}
""", title="Between f32 and f64, and a float's bits", expect="ok", section="s06", slug="float-conversions")
b += code("""
fn to_byte(i32 x) : u8
{
    narrow<u8>(x)                     // checked at run time
}

fn main()
{
    to_byte(300);                     // ✗ diag.narrowing-overflow (dynamic)
}
""", title="A narrowing that does not fit", expect="diag.narrowing-overflow", section="s06", slug="narrow-overflow")
b += p("""`widen` is only accepted where every value of the source fits the target, so it can never
lose one: `widen<u64>` of an `i32` is a static error, because a negative `i32` has no `u64`
value. `narrow` works between any two integer types, and never faults where the value fits.
Use it for `usize` and `isize`: whether `usize → u64` is a widening depends on the address
width, so `narrow<u64>(n)` is the spelling that is accepted by every implementation.""")
b += code("""
fn to_u64(i32 x) : u64
{
    widen<u64>(x)                     // ✗ diag.type-mismatch (static): -1 has no u64 value; use narrow
}

fn main()
{
    to_u64(1);
}
""", title="A widening that could lose a value", expect="diag.type-mismatch", section="s06", slug="widen-direction")

b += h3("Sizes and limits of types")
b += p("""Four intrinsics describe a type rather than a value. Each takes the type as its type
argument, written explicitly, and no other arguments.""")
b += table(["Intrinsic", "Result", "Meaning", "For"], [
    ["`sizeof<T>()`", "`usize`", "the bytes a `T` occupies, padding included", "any type"],
    ["`alignof<T>()`", "`usize`", "the alignment a `T`'s address must have, in bytes", "any type"],
    ["`min_value<T>()`", "`T`", "the smallest value of `T` (for a float, the most negative finite one)", "integer and float types"],
    ["`max_value<T>()`", "`T`", "the largest value of `T` (for a float, the largest finite one)", "integer and float types"],
])
b += p("""Every fixed-width type has the same size and limits on every implementation:
`sizeof<i32>()` is 4 and `max_value<i32>()` is 2147483647. `isize` and `usize` have the address
width, which each implementation documents, so for them the intrinsics are the portable way to
write a size or limit, where a literal is not. A struct's size and alignment follow from its
fields (§16): each field is aligned, and the struct's alignment is its largest field's.""")
b += code(CHECK + """
struct Pair
{
    u8 tag;
    u32 value;
}

fn main()
{
    check(sizeof<i32>() == 4);
    check(sizeof<u64>() == 8);
    check(alignof<u64>() == 8);                      // a number is aligned to its own size
    check(sizeof<bool>() == 1);
    check(sizeof<array<u16, 10>>() == 20);           // an array has no padding
    check(alignof<Pair>() == 4);                     // its largest field's alignment
    check(sizeof<Pair>() == 8);                      // tag, 3 bytes of padding, value
    check(sizeof<usize>() == sizeof<isize>());       // the address width, whatever it is

    check(min_value<i8>() == -128);
    check(max_value<u8>() == 255);
    check(max_value<i32>() == 2147483647);
    check(min_value<u64>() == 0);
    check(max_value<usize>() == ~(0: usize));        // every bit set, at any address width
    check(wrapping_add(max_value<i32>(), 1) == min_value<i32>());
}
""", title="Sizes and limits", expect="ok", section="s06", slug="sizes-limits")
b += p("""Limits are most often the starting value of a search, a bound to clamp against, or a
value that means "none". Here are all three:""")
b += code(CHECK_STD + """
// The spread of a list: start the minimum at the largest i64 and the
// maximum at the smallest, so the first element replaces both.
fn spread(ref<Vec<i64>, shared> v) : i64
{
    i64 lo = max_value<i64>();
    i64 hi = min_value<i64>();
    foreach (p in v)
    {
        i64 x = *p;
        if (x < lo)
        {
            lo = x;
        }
        if (x > hi)
        {
            hi = x;
        }
    }
    hi - lo
}

// Clamp into i32's range, then narrow: the narrow can no longer fault.
fn clamp_to_i32(i64 x) : i32
{
    if (x > widen<i64>(max_value<i32>()))
    {
        max_value<i32>()
    }
    else if (x < widen<i64>(min_value<i32>()))
    {
        min_value<i32>()
    }
    else
    {
        narrow<i32>(x)
    }
}

// The index of the first b, or usize's maximum for "not found".
fn find(ref<Vec<u8>, shared> v, u8 b) : usize
{
    foreach (i, x in v)
    {
        if (*x == b)
        {
            return i;
        }
    }
    max_value<usize>()
}

fn main()
{
    Vec<i64> v = Vec::new();
    Vec::push(&mut v, 7);
    Vec::push(&mut v, -3);
    Vec::push(&mut v, 12);
    check(spread(&v) == 15);

    check(clamp_to_i32(5000000000) == 2147483647);
    check(clamp_to_i32(-5000000000) == -2147483648);
    check(clamp_to_i32(42) == 42);

    Vec<u8> bytes = Vec::new();
    Vec::push(&mut bytes, 10);
    Vec::push(&mut bytes, 20);
    check(find(&bytes, 20) == 1);
    check(find(&bytes, 99) == max_value<usize>());
}
""", title="Limits as sentinels and bounds", expect="ok", section="s06", slug="limits-uses")
b += p("""A generic function may use the limits of its own type parameter. It cannot do arithmetic
on a `T` (§12: there are no bounds), but it can produce a `T`'s limit and pass it on. Each use is
checked when the function is instantiated, so a `T` that is not an integer type is rejected before
the program runs.""")
b += code(CHECK_STD + """
// The value inside, or T's maximum if there is none.
fn or_max<T>(Option<T> r) : T
{
    match (r)
    {
        Some(x) : x,
        None : max_value<T>(),
    }
}

fn main()
{
    u64 big = 5000000000;
    check(or_max(checked_mul(big, big)) == max_value<u64>());   // the product overflows u64
    check(or_max(checked_add(2: i32, 3)) == 5);
}
""", title="A limit in a generic function", expect="ok", section="s06", slug="limits-generic")
b += code("""
fn main()
{
    max_value<bool>();                // ✗ diag.type-mismatch (static): only number types have these
}
""", title="Limits are for number types", expect="diag.type-mismatch", section="s06", slug="limits-float")
b += code("""
fn main()
{
    max_value<i32>() + 1;             // ✗ diag.arith-overflow (dynamic): a limit is a value, not a literal
}
""", title="Going past a limit", expect="diag.arith-overflow", section="s06", slug="limits-overflow")

b += rules([
    "Default arithmetic is checked. If you want wrapping, write `wrapping_add`; if you want to handle overflow, write `checked_add` and match on the `Option`.",
    "Never expect a conversion to happen for you. Reach for `widen` first; use `narrow` (checked) or `narrow_wrapping` (deliberate truncation) when going the other way.",
    "Use `usize` for lengths and indices, `isize` for pointer offsets, and suffix a literal (`5: u8`) when the context does not already fix its type.",
    "Floats never fault. Remember that `NaN` is unequal to itself.",
    "Write a type's limits as `min_value<T>()` and `max_value<T>()`. For `isize` and `usize` they are the only portable spelling.",
])
S06 = Section("s06", "§06", "Arithmetic and numeric types", "06-arithmetic.md",
              "Checked integer arithmetic, explicit conversions, how literals get their types, and the sizes and limits of types.", b)

# ================================================================ §07

b = ""
b += p("""Some values are just data: an `i32`, a `bool`, a struct of numbers. Copying them is
harmless. Other values <em>own</em> something that must be released exactly once: a heap buffer, a
file handle, a lock, a thread. CobaltC calls the second kind <strong>resources</strong>, and the
rules in this section exist so that a resource always has exactly one owner, is never copied, and
is destroyed exactly once, at a predictable moment, whether or not you remember to do it.""")

b += h3("Plain types and resource types")
b += p("""A type is a resource if it is declared with the `resource` keyword, or if any of its
fields, elements or payloads is a resource. `Vec<T>`, `String`, `Rc<T>`, a thread `handle`, a
`mutex` and a lock `guard` are all resources. Numbers, `bool`, references, raw pointers and function
values are plain. A struct of plain fields is plain; a struct containing a `Vec` is a resource by
derivation, with no keyword needed.""")
b += code("""
struct Point                          // plain: two numbers
{
    i32 x;
    i32 y;
}

struct Path                           // a resource by derivation: it contains a Vec
{
    Vec<Point> points;
}

resource struct Handle                // a resource by declaration: it manages something
{                                     // beyond its fields (an OS handle, say)
    i32 fd;
}
""", title="Three kinds of struct")

b += h3("One owner: moves, not copies")
b += p("""A resource lives in exactly one place. When you initialise a variable from it, pass it by
value to a function, store it in a field, or return it, the resource <em>moves</em>: the new place
becomes the owner and the old name becomes <em>stale</em>. Using a stale name is an error
(`diag.stale-binding`), caught at compile time when the move is visible in the same function and at
run time otherwise. Nothing is ever copied silently, so there can never be two owners.""")
b += code("""
resource struct Token
{
    i32 id;
}

fn consume(Token t)                   // a by-value resource parameter takes ownership
{
}                                     // t is destroyed here, at the callee's scope exit

fn main()
{
    Token t = Token { .id = 1 };
    consume(t);                       // ✓ t moves into consume
    consume(t);                       // ✗ diag.stale-binding: t no longer owns anything
}
""", title="Use after move", expect="diag.stale-binding", section="s07", slug="use-after-move")
b += code("""
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    auto w = v;                       // moves the Vec into w; v is now stale
    Vec::push(&mut w, 1);             // ✓ use the new owner
}
""", title="Moving into a new name", expect="ok", section="s07", slug="move-ok")
b += p("""Because a resource is never copied, it cannot appear where a copy would be made: you
cannot compare two `Vec`s with `==`, and you cannot read one out of a field of a live struct. Take
a reference instead (§09), or move the whole container.""")
b += code("""
import std;

fn main()
{
    Vec<i32> a = Vec::new();
    Vec<i32> b = Vec::new();
    auto same = a == b;               // ✗ diag.read-of-resource: comparison would copy both
}
""", title="No value semantics for resources", expect="diag.read-of-resource", section="s07", slug="read-of-resource")

b += h3("Automatic destruction")
b += p("""Every resource is destroyed automatically: a named resource when the block that declared
it ends, and a temporary (an unnamed call result or literal) at the end of the statement that
created it. Locals are destroyed in reverse order of creation. A struct's resource fields are
destroyed after the struct's own destructor, innermost first, in reverse declaration order. A
`return`, `break`, `continue` or run-time fault unwinds the same way. There is no way to leak a
resource, so there is no leak diagnostic.""")
b += p("""You can also destroy a resource early with `drop(x)`. Afterwards `x` is stale until you give
it a new value: once `x`'s value has gone, moved away or dropped, `x = e` gives it a new one, which
is destroyed at the end of `x`'s own block like its first value. Assigning over a value that is
still there is an error for a resource (`diag.overwrite-of-live-resource`): drop it first, so a
destruction is always visible. What counts is whether the value there owns anything: a `None` of an
`Option<String>` owns nothing, so `o = Some(s);` over it is fine, while a `Some` must be dropped
first. A type declared `resource`, or with a destructor, always owns. Declaring the name again
(shadowing) works too.""")
b += code("""
import std;

fn make() : Vec<i32>
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 1);
    v                                 // moved out to the caller, not destroyed here
}

fn main()
{
    {
        auto result = make();         // result owns the Vec now
        Vec::push(&mut result, 2);
    }                                 // destroyed here, at the caller's block exit
    make();                           // an unstored temporary: destroyed at this `;`
    Vec<i32> v = Vec::new();
    drop(v);                          // ✓ destroyed early; v is now stale
    v = Vec::new();                   // ✓ v's value has gone, so v takes a new one
    Vec::push(&mut v, 3);
}
""", title="Where destruction happens", expect="ok", section="s07", slug="destruction")

b += h3("Destructors")
b += p("""A type gets a destructor by declaring an associated function named `drop` that takes an
exclusive reference to the value and returns nothing. The language calls it exactly once, when the
resource is destroyed; you may not call it yourself. Inside it you can mutate the value but not
move out of it. Field resources are destroyed after the body runs.""")
b += code("""
import std;

resource struct Tagged
{
    str tag;
}

fn Tagged::drop(ref<Tagged, exclusive> self)
{
    printf("%s", self.tag);          // runs once, when this Tagged is destroyed
}

resource struct Holder
{
    Tagged a;
    Tagged b;
}

fn main()
{
    Tagged x = Tagged { .tag = "1" };
    Tagged y = Tagged { .tag = "2" };
    Holder h = Holder { .a = Tagged { .tag = "3" }, .b = Tagged { .tag = "4" } };
    printf("\\n");
}   // h first (its fields in reverse order: 4 then 3), then y, then x
""", title="Destruction order", expect="ok", output="\n4321", section="s07", slug="destructor-order")
b += note("""Destructors run in reverse declaration order, last declared first, and a struct's fields
in reverse field order, so what was built last is torn down first. The output above is a newline
followed by `4321`.""", kind="note")

b += h3("Things the rules forbid")
b += code("""
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    v = Vec::new();                   // ✗ diag.overwrite-of-live-resource: the old Vec would be lost
}
""", title="Overwriting a live resource", expect="diag.overwrite-of-live-resource", section="s07", slug="overwrite")
b += p("""The fix is `drop(v);` first, then declare a fresh `v`. Two more rejections you will meet:
`drop(v)` twice is `diag.stale-binding` (the first drop ended `v`); and moving a resource out of a
field of a live struct (`auto inner = holder.field;`) is `diag.move-out-of-field`, because the struct
would be left half-owned. Move the whole struct, or take a reference to the field.""")

b += rules([
    "If a type owns something, it is a resource: mark it `resource` and give it a `drop`. Types that merely contain resources become resources automatically.",
    "Passing a resource by value hands it over. To let a callee look without taking, pass `&x`; to let it modify, pass `&mut x`.",
    "Do not fight scope-end destruction. Put a resource in the smallest block that needs it and let the block end release it.",
    "`drop(x)` is for early release. After it, `x` is gone; redeclare if you need the name.",
])
S07 = Section("s07", "§07", "Resources, ownership and destruction", "07-resource-authority.md",
              "Resource types have exactly one owner, move instead of copying, and are destroyed automatically and exactly once.", b)

# ================================================================ §08

b = ""
b += p("""Two pieces of code touching one object at the same time is the root of most memory bugs:
an iterator invalidated by a push, a struct half-updated by two threads, a buffer freed while
something still points into it. CobaltC's answer is a discipline on <em>access paths</em>: every
way of reaching an object (its name, a reference, a field projection) has a <strong>mode</strong>,
and the modes that may coexist are fixed.""")

b += h3("Two modes")
b += table(["Mode", "Written", "May", "Coexists with"], [
    ["`shared`", "`&x`, type `ref<T, shared>`", "read", "any number of other shared paths"],
    ["`exclusive`", "`&mut x`, type `ref<T, exclusive>`", "read and write; and, for the owner, move or destroy", "nothing else that overlaps"],
])
b += p("""A variable's own name is always an exclusive path: there is no `mut` keyword on bindings,
because whether you may write is decided by what references currently exist, not by how the variable
was declared. Restricting access is what a borrow does.""")

b += h3("The one rule")
b += p("""At every read and every write, the language checks: <em>is there another live path to
the same storage whose mode conflicts with this access?</em> Shared and shared never conflict.
Anything involving an exclusive path conflicts, unless the other path is an <em>ancestor</em> of
this one (the thing it was borrowed from). Forming a conflicting borrow is reported at once as
`diag.aliasing-conflict`; so is using the owner while a conflicting borrow lives.""")
b += code("""
fn main()
{
    i32 x = 1;
    auto r1 = &x;                     // ✓ shared
    auto r2 = &x;                     // ✓ any number of shared readers; the owner may still read x
    x = 2;                            // ✗ diag.aliasing-conflict: writing while r1, r2 are live
    i32 sum = *r1 + *r2;
}
""", title="Shared readers block writes", expect="diag.aliasing-conflict", section="s08", slug="shared-blocks-write")
b += code("""
fn main()
{
    i32 x = 1;
    auto r1 = &mut x;                 // ✓ exclusive
    auto r2 = &x;                     // ✗ diag.aliasing-conflict: r1 is exclusive and still live
    *r1 = 2;
}
""", title="An exclusive borrow excludes everything", expect="diag.aliasing-conflict", section="s08", slug="excl-blocks-shared")
b += p("""Borrows in one argument list are all formed before the call holds any of them, so passing
two conflicting references to the same storage is accepted at the call. The conflict is reported at
the first access inside the function that meets both, the point where it actually matters.""")
b += code("""
fn set_then_read(ref<i32, exclusive> a, ref<i32, shared> b) : i32
{
    *a = 1;                           // ✗ diag.aliasing-conflict (dynamic): b reaches the same i32
    *b
}

fn main()
{
    i32 x = 0;
    set_then_read(&mut x, &x);        // accepted here; each borrow formed while the other was not yet held
}
""", title="Two references to one variable in a call", expect="diag.aliasing-conflict", section="s08", slug="overlapping-args")

b += h3("When does a borrow end?")
b += p("""A reference lives as long as something holds it. A reference stored in a variable lives
until that variable's block ends; a reference passed straight to a call dies when the statement
ends. This is why consecutive calls that each borrow the same variable are fine, and why wrapping a
borrow in a block is the usual way to end it early.""")
b += code(CHECK_STD + """
fn main()
{
    Vec<i32> nums = Vec::new();
    Vec::push(&mut nums, 10);         // the borrow dies at this `;`
    Vec::push(&mut nums, 20);         // ✓ a fresh borrow; nothing conflicts
    i32 x = 1;
    {
        auto r = &mut x;              // r lives until the end of this block
        *r = 5;
    }
    x = x + 1;                        // ✓ r is gone
    check(x == 6);
}
""", title="Borrows end with their holder", expect="ok", section="s08", slug="sequential-borrows")

b += h3("Reborrowing and ancestors")
b += p("""A path derived from another (a reborrow `&*r`, or a field projection `r.a`) has that path
as its ancestor. Ancestors never conflict with their descendants, but the ancestor cannot be used in
a conflicting way while a descendant lives. Once the descendants are gone, the ancestor is usable
again.""")
b += code(CHECK + """
fn main()
{
    i32 x = 1;
    {
        auto r1 = &mut x;
        {
            auto r2 = &*r1;           // ✓ a shared reborrow, child of r1
            check(*r2 == 1);          // writing through r1 here would be diag.aliasing-conflict
        }
        *r1 = 3;                      // ✓ r2 is gone, the parent is usable again
    }
    check(x == 3);                    // ✓ r1 is gone, the owner is usable again
}
""", title="Reborrow", expect="ok", section="s08", slug="reborrow")

b += h3("Disjoint fields")
b += p("""Conflicts are decided by <em>overlap of storage</em>, not by which variable the paths
come from. Two exclusive borrows of different fields of the same struct do not overlap and may
coexist.""")
b += code(CHECK + """
struct Pair
{
    i32 a;
    i32 b;
}

fn main()
{
    Pair p = Pair { .a = 1, .b = 2 };
    auto ra = &mut p.a;
    auto rb = &mut p.b;               // ✓ disjoint from p.a
    p.b = 30;                         // ✗ diag.aliasing-conflict: rb is a live exclusive path to p.b
    *ra = 10;
    *rb = 20;
}
""", title="Disjoint fields (with a mistake)", expect="diag.aliasing-conflict", section="s08", slug="disjoint-mistake")
b += p("""The write to `p.b` above is rejected because `rb`, a live exclusive path to exactly that
storage, still exists until the block ends. The same applies to reading `p.b` through the owner: an
exclusive child excludes every non-ancestor access, reads included, until it is gone. Confine the
borrows to a block and the owner is free again:""")
b += code(CHECK + """
struct Pair
{
    i32 a;
    i32 b;
}

fn main()
{
    Pair p = Pair { .a = 1, .b = 2 };
    {
        auto ra = &mut p.a;
        auto rb = &mut p.b;
        *ra = 10;
        *rb = 20;                     // reading p.a or p.b here would conflict with the live exclusive borrows
    }
    check(p.a == 10);                 // ✓ both borrows ended with the block
    check(p.b == 20);
}
""", title="Disjoint fields", expect="ok", section="s08", slug="disjoint-ok")

b += h3("Shared means read-only, all the way down")
b += p("""An exclusive path can only be derived from an exclusive path. Given a `ref<T, shared>`
you cannot write through it and cannot reborrow it exclusively: the mode is part of the reference's
type, so a function's signature tells the caller exactly what it may do. The other direction is
always allowed: an exclusive reference can be passed to a function that asks for a shared one, and
the function gets a shared reborrow of it (as if you had written `&*r`). With `v : ref<Vec<T>,
exclusive>`, `Vec::len(v)` just works; likewise a `slice<T, exclusive>` for a `slice<T, shared>`.""")
b += code("""
struct P
{
    i32 a;
}

fn bump(ref<P, shared> p)
{
    p.a = 1;                          // ✗ diag.write-through-shared
}

fn main()
{
    P v = P { .a = 0 };
    bump(&v);
}
""", title="Writing through a shared reference", expect="diag.write-through-shared", section="s08", slug="write-through-shared")

b += h3("Moving or destroying needs solitude")
b += p("""Moving a resource, or destroying it (with `drop` or at scope end), requires that no other
path to it exists at all. Otherwise a reference would be left pointing at nothing.""")
b += code("""
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    auto r = &v;
    drop(v);                          // ✗ diag.destroy-while-aliased: r is still live
}
""", title="Destroying while borrowed", expect="diag.destroy-while-aliased", section="s08", slug="destroy-aliased")
b += code("""
import std;

fn take(Vec<i32> v)
{
}

fn main()
{
    Vec<i32> v = Vec::new();
    auto r = &v;
    take(v);                          // ✗ diag.move-while-aliased: r still reaches v
}
""", title="Moving while borrowed", expect="diag.move-while-aliased", section="s08", slug="move-aliased")

b += rules([
    "Borrow shared (`&x`) by default; borrow exclusively (`&mut x`) only for the duration of the change.",
    "Keep references short-lived. Passing `&mut v` directly as an argument creates a borrow that ends at the semicolon.",
    "If the checker complains about a conflict, end the earlier borrow: put it in its own block, or reorder the code so the write happens after the last use of the reference.",
    "Field-level borrows are precise: borrowing `p.a` exclusively leaves `p.b` free.",
])
S08 = Section("s08", "§08", "Aliasing: shared and exclusive access", "08-alias-validity.md",
              "Any number of readers, or one writer, never both. The rule is checked at every access, so a conflict cannot go unnoticed.", b)

# ================================================================ §09

b = ""
b += p("""A reference is the value form of an access path: a way to reach an object without owning
it. References are plain values (copyable, no destroy authority) and carry their mode in their
type. Raw pointers are a different thing entirely and live in §20.""")

b += h3("Forming and using references")
b += table(["Syntax", "Meaning"], [
    ["`&e`", "a `ref<T, shared>` to the place `e`"],
    ["`&mut e`", "a `ref<T, exclusive>` to the place `e`"],
    ["`*r`", "the place `r` refers to: read it in an expression, assign to it, borrow it again, or project a field or element"],
    ["`r.f`, `r[i]`", "field access and indexing look through a reference automatically"],
    ["`r1 == r2`", "identity: true only if both refer to the same storage of the same object"],
])
b += p("""The operand of `&` must be a <em>place</em>: a variable, a field, an element, or `*r`.
You cannot borrow a temporary such as a call result or a literal (`&make()`); bind it to a variable
first. This is rejected rather than given a lifetime because the temporary would end at the
semicolon and take the reference with it.""")
b += code(CHECK + """
struct Pair
{
    i32 a;
    i32 b;
}

fn main()
{
    i32 x = 5;
    {
        auto r = &mut x;
        *r = *r + 1;                  // read and write through the same reference
    }
    check(x == 6);

    Pair p = Pair { .a = 1, .b = 2 };
    auto rp = &p;
    check(rp.a == 1);                 // auto-deref: rp.a means (*rp).a
    auto ra = &p.a;
    auto rb = &p.b;
    check(!(ra == rb));               // different fields, so different identities
    auto ra2 = &p.a;
    check(ra == ra2);                 // same field of the same object

    ref<i32, shared> copy = ra;       // references are plain values: copying is fine
    check(*copy == 1);
}
""", title="References in use", expect="ok", section="s09", slug="refs")
b += code("""
fn make() : i32
{
    5
}

fn main()
{
    auto r = &make();                 // ✗ diag.borrow-of-non-place: a call result is not a place
}
""", title="Borrowing a temporary", expect="diag.borrow-of-non-place", section="s09", slug="borrow-temp")

b += h3("References inside structs")
b += p("""A reference may be stored in a field. It then lives as long as the struct that holds it,
which in turn may not outlive the referent (§10).""")
b += code(CHECK + """
struct View
{
    ref<i32, shared> r;
}

fn main()
{
    i32 x = 1;
    auto v = View { .r = &x };        // the borrow is held by v's object
    check(*v.r == 1);                 // ✓ valid as long as v lives
}
""", title="A reference field", expect="ok", section="s09", slug="ref-field")

b += h3("References as parameters")
b += p("""Passing `&x` or `&mut x` to a function creates a borrow that the parameter holds for the
duration of the call and releases when the call returns. Inside the function the parameter is an
ordinary reference. A parameter of type `ref<T, exclusive>` can be passed on to another call
directly, without a new `&mut`, because it already is an exclusive reference.""")
b += code(CHECK + """
fn bump(ref<i32, exclusive> counter) : i32
{
    *counter = *counter + 1;
    *counter
}

fn bump_twice(ref<i32, exclusive> c) : i32
{
    bump(c);                          // pass the reference on as a value
    bump(c)
}

fn main()
{
    i32 n = 0;
    check(bump(&mut n) == 1);         // the borrow lives for the call only
    check(bump_twice(&mut n) == 3);
    check(n == 3);
}
""", title="Reference parameters", expect="ok", section="s09", slug="ref-params")

b += rules([
    "Use `&x` to lend, `&mut x` to lend for writing. The callee's signature tells you which it needs.",
    "`*r` is a place, so `*r = v`, `&*r`, and `(*r).f` all work; for fields, `r.f` is the same as `(*r).f`.",
    "Bind a call result before borrowing it.",
    "`==` on references asks whether they point at the same thing, not whether the values are equal; compare `*a == *b` for values.",
])
S09 = Section("s09", "§09", "References", "09-identity-origin-extent.md",
              "References are copyable values that name a place; dereferencing gives you that place back with every check intact.", b)

# ================================================================ §10

b = ""
b += p("""A reference must never outlive the object it refers to. CobaltC enforces this with two
layers: a lexical check that rejects the obvious escapes before the program runs, and a use-time
check that catches everything else as `diag.stale-binding` at the first use of a dead reference. No
lifetime annotations exist; the language decides validity from where things are declared.""")

b += h3("A reference cannot leave its referent's scope")
b += p("""If a reference to a local is returned from the function, or stored into a variable
declared in an enclosing block, it is rejected statically as `diag.reference-escapes-scope`.""")
b += code("""
fn dangling() : ref<i32, shared>
{
    i32 x = 1;
    &x                                // ✗ diag.reference-escapes-scope: x dies with this call
}

fn main()
{
    auto r = dangling();
    *r;
}
""", title="Returning a reference to a local", expect="diag.reference-escapes-scope", section="s10", slug="ref-escape")
b += p("""The corrected form returns the value itself: `fn ok() : i32 { i32 x = 1; x }`.""")
b += p("""When the escape is not lexically visible (the reference is stored through another
reference, into an aggregate the checker cannot see the owner of, or through a call), the static
rule does not fire and the run-time check takes over. The program is still rejected, just later.""")
b += code("""
struct Holder
{
    ref<i32, shared> r;
}

fn stash(ref<Holder, exclusive> h)
{
    i32 local = 5;
    h.r = &local;                     // a store through a reference: not lexically visible as an escape
}                                     // local ends here; whatever h points at now holds a dead reference

fn main()
{
    i32 outer = 0;
    auto held = Holder { .r = &outer };
    stash(&mut held);
    *held.r;                          // ✗ diag.stale-binding (dynamic): first use of the dead reference
}
""", title="Caught at use time", expect="diag.stale-binding", section="s10", slug="stale-use")

b += h3("Returning references from functions")
b += p("""A function may return a reference when it has <em>exactly one</em> reference-typed
parameter: the result is understood to point into whatever that argument pointed at, and callers are
checked accordingly. This is the shape of every library accessor (`Vec::index_shared`,
`Rc::get`). A function with zero or several reference parameters may not return a reference
(`diag.lifetime-elision-ambiguous`); compose single-reference helpers instead, or return an owned
value.""")
b += code(CHECK + """
struct Pair
{
    i32 a;
    i32 b;
}

fn first(ref<Pair, shared> p) : ref<i32, shared>
{
    &p.a                              // ✓ one reference parameter: the result borrows from it
}

fn main()
{
    Pair pr = Pair { .a = 1, .b = 2 };
    auto y = first(&pr);              // y is a shared borrow of pr.a, held by y
    check(*y == 1);
}
""", title="Single-parameter elision", expect="ok", section="s10", slug="elision-ok")
b += code("""
fn pick(ref<i32, shared> p1, ref<i32, shared> p2) : ref<i32, shared>
{
    p1                                // ✗ diag.lifetime-elision-ambiguous: which argument does the
}                                     //   result depend on? The language will not guess.
""", title="Two reference parameters, one reference result")
b += p("""Because `y` above is known to borrow from `pr`, a later `pr.a = 3;` while `y` is still in
use is an aliasing conflict (§08), exactly as if you had written `auto y = &pr.a;`.""")

b += h3("Growth invalidates element references")
b += p("""A `Vec` may move its elements to a new buffer when it grows. A reference to an element
obtained before a `push` that reallocates is stale afterwards. Where the checker can see both the
borrow and the push in one function, it rejects the program; where it cannot (the push happens
through a reference parameter, say), the stale reference faults at its next use. Either way it does
not read freed memory.""")
b += code("""
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 10);
    auto first = Vec::index_shared(&v, 0);   // a shared borrow into v, held by first
    Vec::push(&mut v, 20);                   // ✗ diag.aliasing-conflict: an exclusive borrow of v
    *first;                                  //   while first (derived from v) is still live
}
""", title="Holding an element reference across a push", expect="diag.aliasing-conflict", section="s10", slug="vec-ref-push")

b += rules([
    "Return values, not references, unless the function has exactly one reference parameter and the result points into it.",
    "Do not store a reference in a variable that outlives the block where the referent was declared.",
    "Re-fetch element references after any call that may grow or shrink a `Vec`.",
    "If you see `diag.stale-binding`, ask what ended the referent: a scope exit, a move, a `drop`, or a reallocation.",
])
S10 = Section("s10", "§10", "Lifetimes and temporal validity", "10-temporal-validity.md",
              "A reference is checked statically where the escape is visible and dynamically everywhere else; either way it never outlives its target.", b)

# ================================================================ §11

b = ""
b += p("""Reading storage that has never been written is undefined behaviour in C. In CobaltC it is
impossible in a well-formed program: a variable declared without an initialiser must be assigned on
<em>every</em> path before it is read, and the check is static. Aggregates are always initialised
whole.""")

b += h3("Declarations")
b += table(["Form", "Meaning"], [
    ["`T x = e;`", "declare `x` of type `T`, initialised from `e` (which is checked against `T`)"],
    ["`auto x = e;`", "declare `x` with the type of `e`"],
    ["`T x;`", "declare `x` uninitialised; the type is required because there is nothing to infer from"],
    ["`Name { f1, f2 } = e;`", "destructure a struct: bind each field to its name and consume the struct"],
])
b += p("""Initialising from a plain value copies it; from a resource, moves it; from a temporary
such as a struct literal or a call result, adopts it. Redeclaring a name in the same block shadows
the earlier binding.""")

b += h3("Definite assignment")
b += code(CHECK + """
fn main()
{
    i32 x;                            // uninitialised
    bool c = true;
    if (c)
    {
        x = 1;
    }
    else
    {
        x = 2;
    }
    check(x == 1);                    // ✓ assigned on both paths
}
""", title="Assigned on every path", expect="ok", section="s11", slug="definite-ok")
b += code("""
fn main()
{
    i32 x;
    bool c = false;
    if (c)
    {
        x = 1;
    }
    i32 y = x + 1;                    // ✗ diag.use-of-uninitialized: no write on the else path
}
""", title="A path with no write", expect="diag.use-of-uninitialized", section="s11", slug="definite-bad")
b += p("""The analysis is purely about the function's own control flow: it does not know that `c`
happens to be `true`. If it cannot prove a write on some path, the program is rejected (this is one
of the few checks where "unknown" means rejected rather than "check at run time").""")

b += h3("Aggregates are initialised whole")
b += p("""You cannot declare a struct uninitialised and then fill its fields one by one. A struct
literal supplies every field at once; after that, individual fields can be assigned freely.""")
b += code(CHECK + """
struct Pair
{
    i32 a;
    i32 b;
}

fn main()
{
    Pair p = Pair { .a = 1, .b = 2 };  // every field, exactly once, in any order
    p.a = 10;                          // ✓ a field write on an initialised object
    check(p.a == 10);
    check(p.b == 2);
}
""", title="Whole initialisation", expect="ok", section="s11", slug="whole-init")

b += h3("Destructuring a struct")
b += code(CHECK_STD + """
struct Entry
{
    String item;
    i64 change;
}

fn main()
{
    Entry e = Entry { .item = String::from_str("pens"), .change = -3 };
    Entry { item, change } = e;       // binds `item` and `change`; e is consumed
    check(String::len(&item) == 4 && change == -3);
}
""", title="Destructuring", expect="ok", section="s11", slug="destructure")
b += p("""This is the only pattern form beyond `match` on enums. It names every field of the
struct, each once, in any order, and consumes the struct: a resource field moves out to its new
binding, a plain one is copied. It is how a struct gives up a resource it holds (the library's
`String::into_bytes` uses it to hand back the `Vec<u8>`), since a resource field cannot otherwise
be moved out of a struct that lives on. Naming a field twice or leaving one out is a type error,
and a struct with its own destructor cannot be destructured, since the destructor would then run
on a struct whose fields had left it (`diag.move-out-of-field`).""")

b += rules([
    "Prefer `T x = e;` or `auto x = e;`. Declare uninitialised only when the value genuinely comes from a branch, and then assign in every branch.",
    "Build structs with a literal that names every field. Assign fields afterwards if needed.",
    "`auto` needs an initialiser it can learn a type from; `auto v = Vec::new();` is rejected because `T` is unknown (§12).",
])
S11 = Section("s11", "§11", "Initialisation", "11-initialization.md",
              "Every variable is written before it is read, on every path, and every aggregate is built whole.", b)

SECTIONS = [S06, S07, S08, S09, S10, S11]
S06.group = "Reference, one section per specification chapter"
