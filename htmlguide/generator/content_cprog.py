"""The section for C programmers: what carries over, what the same words
mean here, the habits to drop, and, at length, how error handling works
in place of return codes, errno, NULL and abort. Every complete program
is checked by `gen.py --check` under both tools, as everywhere in the
guide."""
from gen import code, h3, h4, note, p, rules, table, ul, Section

b = ""
b += p("""If you write C, most of CobaltC will look familiar and a few things will look wrong. This
section is the shortest path from one to the other: it says what you can keep doing, what the same
spelling means here, which three habits the compiler will stop, and, at the greatest length, how a
program reports and handles failure, because that is where C and CobaltC differ most and where the
reference sections take the longest to connect. Everything here is said again, more fully, in the
sections it links to; this is the map.""")

b += h3("What carries over", anchor="cprog-carry")
b += p("""Declarations are type-first, blocks are braces, statements end in `;`, comments start with
`//`, and `printf` is `printf`. `if`, `while`, `for (init; cond; step)`, `break`, `continue` and
`return` are what you expect, and `&&`, `||`, `!`, the arithmetic, bitwise and comparison operators
have C's spellings and precedences. A struct is a struct, written with C99's designated initialisers
and nothing else: `Point { .x = 3, .y = 4 }`. `main` may return a `u8`, which is the process's exit
status, and the program's arguments are `arg_count()` and `arg(i)`.""")
b += code("""
import std;

struct Point
{
    i32 x;
    i32 y;
}

fn main() : u8
{
    Point p = Point { .x = 3, .y = 4 };
    i32 sum = 0;
    for (i32 i = 0; i < 5; i += 1)
    {
        sum += i;
    }
    if (sum == 10 && p.x < p.y)
    {
        printf("sum %d, point (%d, %d)\\n", sum, p.x, p.y);
    }
    0
}
""", title="A C programmer's first program", expect="ok", output="sum 10, point (3, 4)\n",
     section="cprog", slug="carry")
b += p("""Three small things in that program are not C. `fn main() : u8` puts the return type after a
colon, as every function does. `import std;` brings in the standard library, which is where `printf`
lives; without it the name is unknown (<a href="#parts">Core language and std</a>). And the last line
of `main` is `0` with no `return` and no semicolon: a block's final expression is its value, so
`if`, `match` and plain blocks are expressions too, which is how CobaltC does without the `?:`
operator. `return 0;` would also have been fine.""")

b += h3("The same words, a different meaning", anchor="cprog-words")
b += table(["In C", "In CobaltC", "Where"], [
    ["`int`, `long`, `unsigned`, `size_t`", "sized types only: `i8` `i16` `i32` `i64`, `u8` `u16` `u32` `u64`, `usize` `isize`; `f32` `f64`; `bool`. Literals take a suffix or the expected type.", "§06"],
    ["`char`, `'a'`", "`u8` and the byte literal `b'a'`; text is `str`, never an array of `char`", "§06, §21"],
    ["`char *`, `const char *`", "`str` (a literal, read-only), `String` (owned, growable), `StringView` (a borrowed view); UTF-8 bytes with a length, no terminator", "§21 <em>Which text type?</em>"],
    ["`T *p`", "`ref<T, shared>` (`&x`, read-only) or `ref<T, exclusive>` (`&mut x`, read-write); never null, never dangling; `rawptr<T>` only inside `unsafe`", "§08, §09, §20"],
    ["`T a[N]`", "`array<T, N>`, indexed with a bounds check; `Vec<T>` for a growable one; `slice<T, m>` for a view of part of either", "§16, §21"],
    ["`NULL`, `-1`, `EOF` as \"no value\"", "`Option<T>`: `Some(v)` or `None`", "below"],
    ["`int` return code and `errno`", "`Result<T, E>`: `Ok(v)` or `Err(e)`, `E` an enum you declare", "below"],
    ["`enum`", "`enum`, whose variants may carry a payload: `Invalid(usize)`", "§16"],
    ["`switch`", "`match`, exhaustive, no fall-through, arms `pattern : expression,`", "§16"],
    ["`?:`", "`if (c) { a } else { b }` as an expression; `?` means something else (below)", "§14"],
    ["`i++`, `--i`", "`i += 1`, `i -= 1`; there is no `++`", "§13"],
    ["`(long) x`, implicit promotion", "`widen<i64>(x)`, `narrow<u8>(x)` (checked), `to_float<f64>(x)`, `to_int<i32>(f)`; nothing converts on its own", "§06"],
    ["`sizeof x`, `sizeof(T)`", "`sizeof<T>()`, `alignof<T>()`", "§12"],
    ["`#include \"x.h\"`, `static`", "`import std;`, `module m \"./x.cb\";`, `export` on what others may use; no headers, no preprocessor", "§17"],
    ["`typedef`, `#define N 10`", "no aliases; `const usize N = 10;` is a compile-time constant", "§11, §17"],
    ["`malloc`/`realloc`/`free`", "`Vec`, `String`, `Box`, `Rc`; freed when their owner's scope ends", "§07, §21"],
    ["a `struct` with a cleanup function you must remember to call", "`resource struct` with `fn T::drop`, run automatically when the value's scope ends, on every path", "§07"],
    ["`goto cleanup;`", "nothing: destructors run on early `return`, on `?`, and while a fault unwinds", "§07, §18"],
    ["`assert(c)`, `abort()`", "`assert(c, \"fmt\", …)`, never compiled out; `exit(status)`", "§18, below"],
    ["`setjmp`/`longjmp`, signals", "no equivalent: a fault cannot be caught (a thread's can be contained, §19)", "§18"],
    ["`pthread_create`, a shared global", "`spawn(f, args…)` and `join(h)`; shared data only through a `mutex<T>` or an `Atomic<T>`; there are no mutable globals", "§19"],
    ["`_Atomic`, `atomic_fetch_add`", "`Atomic<T>` (an integer of up to 64 bits), `Atomic::fetch_add(&a, n)`; always sequentially consistent, and overflow faults unless you call the `_wrapping` form", "§19"],
    ["`atomic_bool`, `_Atomic(T *)`, `atomic_thread_fence`, C++20 `atomic::wait`/`notify_one`", "`AtomicBool`, `AtomicPtr<T>`, `atomic_fence()`, `Atomic::wait(&a, v)`/`Atomic::notify_one(&a)`; no `memory_order` argument anywhere", "§19"],
    ["GCC/Clang `__attribute__((vector_size(16)))`, `<immintrin.h>` intrinsics", "`simd<f32, 4>` and kin: the same vector types, with `+ - *` lane by lane and `simd_` intrinsics; portable, no per-processor names", "§16"],
    ["`struct` layout, `alignas(64)`", "the same layout as C for the same members; `struct S align(64) { … }` on the type", "§16"],
    ["`extern int f(int);` and the linker", "`unsafe extern fn f(i32 x) : i32;` plus `extern \"./f.c\";` or `extern \"m\";`; `cobc` compiles and links", "§20"],
    ["passing a `struct` or a `const char *` to C, reading a returned `char *`", "a plain struct crosses by value as it is; text goes as `CString::new(s)` and `CString::ptr(&c)`, comes back with `unsafe { String::from_c(p) }`; `is_null(p)` tests for `NULL`", "§20"],
])
b += p("""Two spellings deserve a warning because they parse and mean something else. `?` after an
expression is not the start of a conditional: it <em>propagates an error</em> and returns from the
function (below). And `&x` always makes a reference the compiler tracks, never an address you can
add to; the address is `rawptr_of(&x)`, usable only in `unsafe`.""")

b += h3("Three habits the compiler stops", anchor="cprog-habits")
b += p("""<strong>Overflow is not silent.</strong> In C, `unsigned` arithmetic wraps and signed overflow
is undefined. Here `+`, `-` and `*` are checked: a result that leaves the type faults the program
with `diag.arith-overflow`, division by zero with `diag.div-by-zero`. When you want wrapping, say so
with `wrapping_add` and friends; when you want to detect it, `checked_add` returns an `Option`
(§06).""")
b += code("""
import std;

fn bump(u8 x) : u8
{
    x + 10                             // checked: faults when the sum leaves u8
}

fn main()
{
    printf("%d\\n", bump(100));
    bump(250);                         // ✗ diag.arith-overflow (dynamic)
}
""", title="No silent wrap-around", expect="diag.arith-overflow", output="110\n", section="cprog", slug="overflow")
b += p("""<strong>Nothing converts implicitly.</strong> `int` to `long`, `char` to `int`, `float` to
`double`, a pointer to `bool`: none of it happens. Every change of representation is a named
intrinsic, and the ones that can lose information check the value (§06, <em>Conversions are
explicit</em>). The error message tells you which one to write.""")
b += code("""
fn main()
{
    i32 a = 5;
    i64 b = a;                         // ✗ diag.type-mismatch: write widen<i64>(a)
}
""", title="No implicit widening", expect="diag.type-mismatch", section="cprog", slug="convert")
b += p("""<strong>Pointers do not escape.</strong> A reference to a local cannot be returned or stored
anywhere that outlives the local; a reference into a `Vec` cannot be kept across a `push` that may
move the elements; a value cannot be used after it was moved. These are rejected before the program
runs where the compiler can see them (`diag.reference-escapes-scope`, `diag.stale-binding`) and
stop the program at the first use otherwise (§09, §10). There is no dangling pointer to find later.""")
b += code("""
fn dangling() : ref<i32, shared>
{
    i32 local = 5;
    &local                             // ✗ diag.reference-escapes-scope
}

fn main()
{
    dangling();
}
""", title="A pointer to a dead local, caught", expect="diag.reference-escapes-scope", section="cprog", slug="dangling")

b += h3("Memory: no malloc, no free", anchor="cprog-memory")
b += p("""Every value has one owner, and when the owner's scope ends the value is destroyed, on every
path out of the scope: a normal end, an early `return`, a `?` that returns an error, or the unwinding
of a fault. A `Vec` frees its buffer, a `String` its bytes, a `File` closes. Your own type joins in by
being declared `resource` and giving `fn T::drop`, the destructor, which runs first; the fields follow.
This replaces the `cleanup:` label, the matching `free` for each `malloc`, and the `fclose` someone
forgot on one error path. Passing or assigning a resource <em>moves</em> it: the new name owns it, the
old name is dead, and using the old name is an error. Plain values (numbers, `bool`, references,
small structs of them) are copied as in C.""")
b += code("""
import std;

resource struct Connection
{
    String name;
}

fn Connection::drop(ref<Connection, exclusive> self)
{
    printf("close %s\\n", &self.name);
}

fn use_it()
{
    Connection c = Connection { .name = String::from_str("primary") };
    printf("using %s\\n", &c.name);
}                                      // c ends here: drop runs, then the String is freed

fn main()
{
    use_it();
    Vec<i32> v = Vec::new();           // grows as needed; freed when v ends
    for (i32 i = 0; i < 5; i += 1)
    {
        Vec::push(&mut v, i * i);
    }
    printf("%d items, last %d\\n", Vec::len(&v), v[4]);
}
""", title="Destruction at scope end", expect="ok", output="using primary\nclose primary\n5 items, last 16\n",
     section="cprog", slug="resource")
b += p("""Library calls name the type they belong to and write the borrow they take: `Vec::push(&mut
v, x)`, `Vec::len(&v)`. There is no `v.push(x)`; the `.` is only field access. The mode of every
access, `&` or `&mut`, is visible at the call (§08).""")

b += h3("Strings", anchor="cprog-strings")
b += p("""A string is bytes with a length, in UTF-8, with no terminating zero. A literal is a `str`:
read-only, indexed by byte with `str_byte(s, i)`, measured with `str_len(s)`. Text you build or own is
a `String` (`String::from_str`, `String::append`, `sprintf`); text you only look at, passed to a
function, is a `StringView` (§21, <em>Which text type?</em>). Comparison is a function
(`String::eq_str`, `StringView::eq`), never `==` on pointers, and `%s` prints any of the three.""")
b += code("""
import std;

fn main()
{
    str lit = "héllo";                 // UTF-8 bytes, no terminator
    printf("%d bytes\\n", str_len(lit));
    String s = String::from_str("abc");
    String::append(&mut s, "def");
    printf("%s has %d bytes; third is %d\\n", &s, String::len(&s), str_byte("abc", 2));
}
""", title="Bytes with a length", expect="ok", output="6 bytes\nabcdef has 6 bytes; third is 99\n",
     section="cprog", slug="strings")

# ---------------------------------------------------------------- errors

b += h3("Error handling, from the beginning", anchor="cprog-errors")
b += p("""C has one mechanism for everything that can go wrong, and it is a convention: a function
returns a value that means failure (`-1`, `NULL`, `EOF`, `false`), perhaps sets `errno`, and the caller
checks, or does not. Bugs and expected failures travel the same road, nothing forces the check, and
cleanup on the way out is the caller's problem. CobaltC separates the two kinds of failure and gives
each its own mechanism, and that separation is the thing to learn first; the syntax follows from
it.""")
b += table(["", "An <strong>error value</strong>", "A <strong>fault</strong>"], [
    ["What it is", "An expected failure the program may recover from: bad input, a missing file, a full queue, a lookup that finds nothing", "A violated rule of the language: overflow, an index out of range, a dead reference, a failed `assert`, an `unwrap` of nothing"],
    ["How it travels", "As a value: `Result<T, E>` or `Option<T>`, returned like any other value", "It does not: the thread unwinds every scope, running destructors, and the program ends with the fault's name and exit status 1"],
    ["Who decides", "The caller, with `match`, `?`, `if (Some(x) = …)`, `unwrap_or`", "Nobody: there is no `catch` (a spawned thread's fault can be contained at its `join`, §19)"],
    ["In C terms", "the return code, done so that it cannot be ignored", "`abort()`, done so that it runs your cleanup first"],
])
b += p("""The rule of thumb: if a failure is part of normal operation, it is an error value; if it can
only happen because the program is wrong, let the fault fire and fix the program. Do not wrap
arithmetic in `Result`, and do not `assert` that a file exists.""")

b += h4("`Option` in place of `NULL` and sentinels")
b += p("""`Option<T>` is `Some(v)` or `None`. It is a plain two-variant enum from `std`, not a pointer,
so a function that may have nothing to return says so in its type and the caller cannot forget to
look: there is no way to use the `T` without taking the `Option` apart. There are four ways to take it
apart, and C programmers tend to reach for the last one first.""")
b += code("""
import std;

fn find(ref<Vec<i32>, shared> v, i32 wanted) : Option<usize>
{
    foreach (i in 0 .. Vec::len(v))
    {
        if ((*v)[i] == wanted)
        {
            return Some(i);
        }
    }
    None
}

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 5);
    Vec::push(&mut v, 8);
    Vec::push(&mut v, 13);
    match (find(&v, 8))                                  // 1. match: both cases, side by side
    {
        Some(i) : printf("found at %d\\n", i),
        None    : printf("absent\\n"),
    }
    if (Some(i) = find(&v, 13))                          // 2. a pattern as the condition
    {
        printf("13 at %d\\n", i);
    }
    usize at = Option::unwrap_or(find(&v, 99), 0);       // 3. a default
    printf("%d\\n", at);
    while (Some(x) = Vec::pop(&mut v))                   // the same, as a loop
    {
        printf("%d ", x);
    }
    printf("\\n");
}
""", title="Option, taken apart four ways", expect="ok", output="found at 1\n13 at 2\n0\n13 8 5 \n",
     section="cprog", slug="option")
b += p("""The fourth way is `Option::unwrap(o)`, or `Option::expect(o, "why it must be there")`: it
gives the value or faults with `diag.unwrap-failed`. That is the right tool exactly when `None` would
mean the program is wrong, as `assert` is; in a library function, or anywhere the caller could
reasonably have passed nothing, it is the wrong one, because it turns the caller's input into your
crash. If you find yourself writing `if (Option::is_some(&o)) { … Option::unwrap(o) … }`, write the
`if (Some(x) = o)` form instead: it binds the value and checks in one step.""")

b += h4("`Result` in place of return codes and `errno`")
b += p("""`Result<T, E>` is `Ok(v)` or `Err(e)`. `E` is any type, and the idiom is an enum you declare
for the module, one variant per way things fail, carrying what the caller needs to say something
useful (`Invalid(usize)`: at which position). Compared with an `int` and `errno`: the value and the
failure are one object, so they cannot get out of step; the error carries data, not a number to look
up; and a `Result` that is not looked at is rejected by the compiler (`diag.result-discarded`), so the
one failure mode C cannot prevent, the ignored return code, does not compile. If you really mean to
drop it, say so: `_ = save(x);`.""")
b += code("""
import std;

fn save(i32 n) : Result<void, str>
{
    if (n < 0) { Err("negative") } else { Ok(()) }
}

fn main()
{
    save(-1);                          // ✗ diag.result-discarded: in C this error would vanish; here it cannot
}
""", title="An ignored return code does not compile", expect="diag.result-discarded", section="cprog", slug="discard")
b += p("""`std`'s own functions follow the same shape. `File::open` is `Result<File, FileError>`;
`String::parse<u32>` is `Result<u32, ParseError>`; `HashMap::get` is `Option<ref<V, shared>>`. Every
`std` error enum has a `text` function that gives a short sentence for a message
(`FileError::text(&e)`), which is what a program prints when it gives up (§21).""")

b += h4("`?`: the early return, in one character")
b += p("""Most C error handling is the same four lines repeated: call, test the code, clean up, return
the code to the caller. `?` is those four lines. Written after an expression of type `Result<T, E>`, in
a function whose own return type is `Result<_, E>`, it yields the `T` when the value is `Ok` and
otherwise returns the `Err` from the enclosing function at once, running the destructors of
everything in scope on the way, as any `return` does. Inside a function returning `Option`, `?` does
the same for `None`. Three things follow, and they are the three things that confuse newcomers:""")
b += ul([
    "<strong>The function must be able to return the error.</strong> `?` on a `Result<T, E>` needs an enclosing return type of `Result<_, E>`, with the <em>same</em> `E`. In a function returning `u8` or nothing, `?` is rejected: `diag.propagate-outside-fallible-context`. So `main`, which returns `u8` or nothing, cannot use `?`; the pattern below deals with that.",
    "<strong>Different error types do not mix on their own.</strong> A function that calls into two libraries with two error enums must convert each to its own: `Result::map_err(r, f)` applies `f` to the `Err` and leaves an `Ok` alone, and a closure `[](ParseError e) : ConfigError { BadNumber(e) }` is the usual `f`, wrapping the inner error in a variant of the outer. The conversion is explicit at each boundary, and that is where you decide what the caller is told.",
    "<strong>`Option` and `Result` do not mix either.</strong> `Option::ok_or(o, e)` turns `None` into `Err(e)` so that `?` can return it: `u32 n = Option::ok_or(checked_add(a, b), TooLarge)?;` reads \"the sum, or report TooLarge\". `Result::ok(r)` goes the other way, dropping the error.",
])

b += h4("The whole shape of a program")
b += p("""Put together, a CobaltC program that can fail has three layers, and the layering is what to
copy. At the bottom, functions return `Result` with the module's error enum and use `?` to pass
failures up. In the middle, a function converts foreign errors into that enum with `map_err` and
`ok_or`, so the layer above sees one type. At the top, `main` cannot use `?` and should not try: it
calls one function that does the work and returns a `Result`, matches once, prints the error's text
to standard error and returns a non-zero status. That `match` is the only place in the program that
decides how a failure is reported, which is where a C programmer expects `perror` and `exit(1)` to
be.""")
b += code("""
import std;

enum ConfigError
{
    MissingValue,
    BadNumber(ParseError),
    OutOfRange(u32),
}

fn ConfigError::text(ref<ConfigError, shared> e) : String
{
    match (e)
    {
        MissingValue  : String::from_str("no value given"),
        BadNumber(pe) : sprintf("not a number (%s)", ParseError::text(pe)),
        OutOfRange(n) : sprintf("%d is not a port", *n),
    }
}

fn port(str text) : Result<u16, ConfigError>
{
    if (str_len(text) == 0)
    {
        return Err(MissingValue);
    }
    u32 n = Result::map_err(StringView::parse<u32>(StringView::of(text)), [](ParseError e) : ConfigError { BadNumber(e) })?;
    if (n > 65535)
    {
        return Err(OutOfRange(n));
    }
    Ok(narrow<u16>(n))
}

fn run() : Result<void, ConfigError>
{
    u16 p = port("8080")?;             // Err returns from run here, if it is one
    printf("port %d\\n", p);
    u16 q = port("99999")?;
    printf("port %d\\n", q);
    Ok(())
}

fn main() : u8
{
    match (run())
    {
        Ok(_)  : 0,
        Err(e) :
        {
            eprintf("config: %s\\n", &ConfigError::text(&e));
            1
        },
    }
}
""", title="Errors travel up; main reports once", expect="ok", output="port 8080\n", status=1,
     section="cprog", slug="config")
b += p("""Read `port` against the C version you would write: the `if (!s[0]) return -1;`, the
`strtoul` with its `endptr` and `errno` check, the range test, each returning a different negative
number the caller has to know. Here each failure is a named variant, the parse error from `std` is
kept inside `BadNumber` rather than flattened to a code, and `run` has no error handling at all: two
`?` and the happy path. The exit status is 1 and the message is on standard error, so the program
behaves as a shell expects, and the `output` above shows only what reached standard output before the
second `port` failed.""")
b += code("""
import std;

fn port(str text) : Result<u16, ParseError>
{
    StringView::parse<u16>(StringView::of(text))
}

fn main() : u8
{
    u16 p = port("8080")?;             // ✗ diag.propagate-outside-fallible-context: main returns u8, not a Result
    0
}
""", title="The mistake every C programmer makes once", expect="diag.propagate-outside-fallible-context",
     section="cprog", slug="main-propagate")
b += note("""`?` is not a `switch` default and not the ternary operator; it is `return Err(e)` with the
cleanup done for you. When it is rejected, the message says the function cannot return the error:
either give the function a `Result` return type and let the caller decide, or handle the value here
with `match`.""", "note", "Reading the `?` error")

b += h4("Faults: what `abort()` should have been")
b += p("""A fault is the language refusing to continue: an overflow, an index out of range, a division by
zero, an `unwrap` of `None`, a false `assert`, a reference used after its referent ended. Nothing in
the program can catch it, which is the point: the code after the failed check would have run on a
state the check protected. What a fault does do, unlike `abort()`, is unwind. The faulting thread
leaves every open scope in order, running every destructor, so files are closed and locks released,
and only then does the program end, printing the fault's name, its location and the source line, with
exit status 1.""")
b += code("""
import std;

resource struct Connection
{
    str name;
}

fn Connection::drop(ref<Connection, exclusive> self)
{
    printf("closed %s\\n", self.name);  // runs while the fault unwinds
}

fn third(ref<array<i32, 3>, shared> a, usize i) : i32
{
    (*a)[i]
}

fn main()
{
    Connection c = Connection { .name = "db" };
    array<i32, 3> a = [1, 2, 3];
    printf("%d\\n", third(&a, 2));
    printf("%d\\n", third(&a, 3));      // ✗ diag.index-out-of-bounds (dynamic): c is closed first
}
""", title="A fault unwinds before it ends the program", expect="diag.index-out-of-bounds", output="3\nclosed db\n",
     section="cprog", slug="fault")
b += p("""`assert` is the one fault you raise yourself. Unlike C's, it is never compiled out, so it is
a statement about the program that holds in every build; and its message is a `printf` format whose
arguments are evaluated only when the condition is false, so a precise message costs nothing while
things are right.""")
b += code("""
import std;

fn average(ref<Vec<i32>, shared> v) : i32
{
    assert(Vec::len(v) > 0, "average of %d values", Vec::len(v));
    i32 sum = 0;
    foreach (x in v)
    {
        sum += *x;
    }
    sum / narrow<i32>(Vec::len(v))
}

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 4);
    Vec::push(&mut v, 6);
    printf("%d\\n", average(&v));
    Vec::clear(&mut v);
    average(&v);                       // ✗ diag.assert-failed (dynamic): "average of 0 values"
}
""", title="assert with a message", expect="diag.assert-failed", output="5\n", section="cprog", slug="assert")
b += p("""Whether something should be an `assert` or an `Err` is the same question as before: can a
correct program, given legitimate input, reach it? An empty `Vec` passed to `average` by a caller who
read it from a file can, so in a real program `average` would return `Option<i32>`; the `assert` is
right only if the caller has already guaranteed the length. When a fault happens in a thread you
`spawn`ed, it ends that thread alone and becomes the thread's result, which `join` raises and
`try_join` hands over as a value (§19, <a href="#s19-supervision">When a thread fails</a>); the main
thread's fault always ends the program.""")

b += h4("The idioms, side by side")
b += table(["C", "CobaltC"], [
    ["`if (p == NULL) return -1;`", "`?` on an `Option` in a function returning `Option`; or `Option::ok_or(o, MyError)?` in one returning `Result`"],
    ["`if (rc < 0) { cleanup(); return rc; }`", "`T v = call()?;` with the module's error type; destructors do the cleanup"],
    ["`if (rc < 0) { cleanup(); return MY_ERR; }`", "`Result::map_err(call(), [](TheirError e) : MyError { Wrapped(e) })?`"],
    ["`switch (rc) { case E_A: … case E_B: … }`", "`match (e) { A : …, B(detail) : …, }`, every variant covered"],
    ["`perror(\"open\"); exit(1);`", "in `main`: `Err(e) : { eprintf(\"open: %s\\n\", FileError::text(&e)); 1 }`"],
    ["`x = f(); /* ignore */`", "`_ = f();`, the only way to drop a `Result`"],
    ["`assert(n > 0);`", "`assert(n > 0, \"n was %d\", n);`"],
    ["`abort()`, `exit(EXIT_FAILURE)`", "`exit(1)`, after unwinding; or `return 1` from `fn main() : u8`"],
    ["a flag `ok` threaded through a loop", "`while (Some(item) = next(&mut it)) { … }` or `foreach`, with `?` inside a `Result`-returning function"],
    ["`errno` read after the call", "the payload of the `Err` variant, or `E::text(&e)` for a sentence"],
])

b += h3("Building and calling C", anchor="cprog-build")
b += p("""There is no header, no object file to manage and no Makefile. `cobc prog.cb` compiles the
program and the files it names with `module m "./m.cb";`, then links; `coby prog.cb` runs it directly;
`cobc --check prog.cb` only checks. Calling C is a declaration and a file: `unsafe extern fn area(i32
w, i32 h) : i32;` promises the signature, `extern "./shapes.c";` names the source (or `extern "m";` a
library), and every call sits in an `unsafe` block, because the compiler cannot see what the C does
(§20). The types at the boundary are the C-compatible ones: sized integers, floats, `bool`,
`rawptr<T>`. Everything you have written in C stays callable; what changes is that the boundary is
marked, and the safe side of it keeps every guarantee of this section.""")
b += rules([
    "Reach for `Option` where you reached for `NULL` or `-1`, for `Result` where you reached for a return code, and for a fault (`assert`) only where the program would be wrong.",
    "Give each module one error enum, convert foreign errors into it with `map_err` and `ok_or` at the boundary, and use `?` everywhere else.",
    "Let `main` match once: print `E::text(&e)` to standard error and return a non-zero `u8`.",
    "Trust the destructors: there is no cleanup label to jump to, because every path out of a scope runs them.",
])

CPROG = Section("cprog", "", "CobaltC for C programmers", None,
                "What carries over from C, what the same words mean here, the habits the compiler stops, and how error handling works in place of return codes, errno, NULL and abort.",
                b)
SECTIONS = [CPROG]
