from gen import Section, p, h3, h4, ul, ol, table, note, rules, code
from content_a import CHECK, CHECK_STD

# ================================================================ §17

b = ""
b += p("""Modules group declarations and control which names are visible where. They introduce no
new semantics: ownership, borrowing and every other rule apply unchanged across module boundaries.
The vocabulary is C++20's: `module`, `import`, `export`.""")

b += h3("Declaring modules")
b += p("""`module name { … }` encloses any declarations, including nested modules. Everything is
<em>private</em> by default: usable from its own module and the modules nested inside it. `export`
on a declaration makes it usable from outside. An exported item inside a private module is only
reachable where that module is.""")
b += p("""Each declaration adds one qualified name (`bank::fee`, `bank::Account::open`,
`Shape::Circle`), and a name may be added only once, so declaring `fee` twice in `bank` is
`diag.duplicate-item`. The same simple name in two different modules is two different names. An
associated function `fn T::name` belongs in the module that declares `T`; anywhere else it is
`diag.foreign-associated-fn`, so no program can add to, or replace, the functions of `std`'s
`Vec`.""")
b += code("""
module bank
{
    export fn fee(i32 amount) : i32
    {
        percent(amount, 2)            // a private helper is visible inside its own module
    }

    fn percent(i32 v, i32 pct) : i32
    {
        v * pct / 100
    }

    export module audit                // nested modules; both must be exported to reach `twice`
    {
        export fn twice(i32 v) : i32
        {
            fee(v) * 2                 // the enclosing module's items are in scope
        }
    }
}

fn main()
{
    std::assert(bank::fee(1000) == 20);      // qualified with ::
    std::assert(bank::audit::twice(1000) == 40);
}
""", title="Modules and visibility", expect="ok", section="s17", slug="modules")
b += code("""
module inner
{
    fn helper() : i32                 // not exported
    {
        42
    }
}

fn main()
{
    inner::helper();                  // ✗ diag.name-not-visible
}
""", title="Private by default", expect="diag.name-not-visible", section="s17", slug="not-visible")

b += h3("`import`")
b += p("""`import a::b::name;` lets `name` be used unqualified in the importing module. It creates no
alias and no copy; it only extends name lookup. There is no `import … as` renaming: if two
imports would give one name two meanings, qualify.""")
b += code("""
module maths
{
    export fn square(i32 v) : i32
    {
        v * v
    }
}

import maths::square;

fn main()
{
    std::assert(square(6) == 36);           // resolves through the import
    std::assert(maths::square(7) == 49);    // the qualified form still works
}
""", title="import", expect="ok", section="s17", slug="import")
b += p("""`import m;` naming a module does two things: `m` itself resolves, as with any import, and
every item `m` exports, including the variants of its exported enums, can be used unqualified, except
its submodules, which keep their paths (`std::text`) or are imported themselves. This
is how programs use the standard library: `import std;`. A name is looked up in a fixed order: the
module's own declarations first, then single-name imports, then module imports, then the
enclosing modules. So a module's own `printf` wins over `std::printf`, which stays reachable by its
full name. If two imported modules both export a name, using that name unqualified is
`diag.ambiguous-name`; importing both is fine, so a module that gains an export never breaks a
program that does not use it. An import acts only in the module that writes it: a nested module, or
a module in a file of its own, writes its own `import std;`.""")
b += code(CHECK_STD + """
module shapes
{
    import std;

    export enum Shape
    {
        Square(i32),
        Dot,
    }

    export fn describe(Shape s)
    {
        match (s)
        {
            Square(_) : printf("square\\n"),
            Dot       : printf("dot\\n"),
        }
    }
}

import shapes;

fn printf(str s)                      // the root's own printf is found before std::printf
{
    std::printf("> ");
    std::printf("%v", s);
}

fn main()
{
    describe(Square(2));              // shapes::describe and shapes::Shape::Square, unqualified
    printf("done\\n");
}
""", title="Importing a module", expect="ok", output="square\n> done\n", section="s17", slug="import-module")

b += h3("Re-export: `export import`")
b += p("""An `import` can be exported like any other declaration. `export import p;` in a module `M`
does what `import p;` does, and also passes on what `p` brings in as `M`'s own items: every item
the module `p` exports, or the one item `p` names. A program that imports `M` then finds those names
as if `M` had declared them, and `M::name` works too. They are the same items, not copies: a type
passed on this way is one type under two paths. This is how a library divided into modules is still
used with one import, and how `std` itself is built (§21): `import std;` brings in `std::text`,
`std::io` and the rest because `std` re-exports each of them.""")
b += code(CHECK_STD + """
module geometry
{
    export module shapes
    {
        export struct Square
        {
            export i32 side;
        }

        export fn area(Square s) : i32
        {
            s.side * s.side
        }
    }

    export module units
    {
        export fn cm(i32 n) : i32
        {
            n * 10
        }
    }

    export import geometry::shapes;   // shapes' items become geometry's
    export import geometry::units::cm;     // one item only
}

import geometry;

fn main()
{
    Square s = Square { .side = cm(2) };      // geometry::shapes::Square, through geometry
    printf("%v\\n", area(s));
    geometry::shapes::Square t = geometry::Square { .side = 1 };   // one type, two paths
    printf("%v\\n", area(t));
}
""", title="One import for a library in parts", expect="ok", output="400\n1\n", section="s17", slug="reexport")
b += p("""Only what is exported passes through: a private item never becomes reachable by being
re-exported. A name the module already has for a different item is `diag.duplicate-item`; the same
item arriving twice, or two modules re-exporting each other, is not an error.""")

b += h3("Constants as items")
b += p("""A `const` declared at a module's top level is an item like a function: it is private unless
marked `export`, it can be imported, and other modules name it by its path (`config::LIMIT`).
Constants themselves, and the `static_assert` checks built on them, are described with
declarations (§11) and with assertions (§18). The specification keeps their rules in its modules
chapter (`spec/17` §1a and §1b), because a constant is an item.""")

b += h3("A module in another file")
b += p("""A module's body can be a separate file. `module geo "./geometry.cb";` means exactly
`module geo { … }` with the file's declarations as the body: the same visibility, the same
`import`, the same lookup. The declaring file names the module; the other file's top level is
anonymous, just as an inline body is. The path is relative to the directory of the file that
writes it, so a file can itself declare file-backed modules with paths relative to its own
location, and a library moves as a unit. Use forward slashes and stay inside the tree (no leading
`/`, no `..`); anything beyond that is up to the implementation.""")
GEOMETRY = """
// geometry.cb -- the body of a module. Nothing here names the module;
// the file that declares it does. Only `export` items are visible there.
export struct Rect
{
    export f64 w;
    export f64 h;
}

export fn area(Rect r) : f64
{
    r.w * r.h
}

fn scale_factor() : f64               // private to this module
{
    1.0
}
"""
b += code(GEOMETRY, title="geometry.cb", section="s17", slug="geometry")
b += code("""
// main.cb -- run this one: `coby main.cb`
module geo "./geometry.cb";           // module geo { … } with geometry.cb's items as the body

import geo::Rect;

fn main()
{
    Rect r = Rect { .w = 3.0, .h = 4.0 };
    std::assert(geo::area(r) == 12.0);      // qualified, as with any module
    // geo::scale_factor();           // ✗ diag.name-not-visible: private, exactly as in one file
}
""", title="main.cb", expect="ok", section="s17", slug="two-files", files={"geometry.cb": GEOMETRY})
b += p("""Three things are rejected before anything runs: a path that names no file
(`diag.module-file-not-found`), a file whose body would load itself again through some chain of
declarations (`diag.module-cycle`), and one file named by two declarations
(`diag.module-file-duplicate` — it would be two distinct modules and two distinct `Rect` types).
A `fn main` in a loaded file is just the item `geo::main`; the program's entry is the root file's
`main`, and a program with none is `diag.no-main`. Both implementations report a diagnostic
inside a loaded file against that file and its own line.""")

b += h4("A library in several files")
b += p("""Files and re-export (above) together give a library that is split across files but used with
one `import`. Its root file declares the parts, keeps its helpers in a private module, and passes on
what users should see: here every exported item of `shapes`, and `cm` alone from `units`. The
helpers are usable everywhere inside the library and nowhere outside it, and the files can be
reorganized later without changing any program that imports the root.""")
LIB_ROOT = """
// geometry.cb -- the library's root: what `import geometry;` brings in
export module shapes "shapes.cb";
export module units "units.cb";
module detail "detail.cb";            // private: the library's own helpers

export import geometry::shapes;       // every exported item of shapes
export import geometry::units::cm;    // one item; units::mm keeps its path
"""
LIB_SHAPES = """
// shapes.cb
export struct Square
{
    export i32 side;
}

export fn Square::new(i32 side) : Square
{
    Square { .side = side }
}

export fn area(Square s) : i32
{
    detail::square(s.side)            // geometry's private module, visible inside geometry
}
"""
LIB_UNITS = """
// units.cb
export fn cm(i32 n) : i32
{
    n * 10
}

export fn mm(i32 n) : i32
{
    n
}
"""
LIB_DETAIL = """
// detail.cb -- exported from a private module: reachable only inside geometry
export fn square(i32 v) : i32
{
    v * v
}
"""
b += code(LIB_ROOT, title="geometry.cb", section="s17", slug="lib-root")
b += code(LIB_SHAPES, title="shapes.cb", section="s17", slug="lib-shapes")
b += code(LIB_UNITS, title="units.cb", section="s17", slug="lib-units")
b += code(LIB_DETAIL, title="detail.cb", section="s17", slug="lib-detail")
b += code("""
// main.cb -- run this one: `coby main.cb`
import std;

module geometry "geometry.cb";
import geometry;

fn main()
{
    Square s = Square::new(cm(2));    // shapes and units, through one import
    printf("%v\\n", area(s));
    printf("%v\\n", geometry::units::mm(5));     // not passed on: written in full
    // geometry::detail::square(3);   // ✗ diag.name-not-visible: detail is private
}
""", title="main.cb", expect="ok", output="400\n5\n", section="s17", slug="lib-several-files",
    files={"geometry.cb": LIB_ROOT, "shapes.cb": LIB_SHAPES, "units.cb": LIB_UNITS, "detail.cb": LIB_DETAIL})

b += h3("How a name is found")
b += p("""An unqualified name is first looked up as a local variable or parameter. Failing that, in
this order: an item of the current module; an `import` in the current module whose last segment
matches; an item of an enclosing module, outward to the root; finally a variant: first of the
enums of this module, or else of the nearest enclosing module with one of that name, and only if
there is none, of an imported enum. So your own `enum Shape { …, Empty }` keeps its bare `Empty` even
though `std`'s `ParseError` has an `Empty` too. More than one candidate at a step is
`diag.ambiguous-name`; none at all is
`diag.unbound-name`. A qualified name `a::b::c` starts by resolving `a` the same way and then walks
into it, checking visibility at the end.""")
b += p("""Two special cases: an associated function declared as `fn Type::name(…)` is addressed as
`Type::name` (or `mod::Type::name` from outside), and an enum's variants are addressable as
`Enum::Variant` as well as bare.""")
b += code("""
fn main()
{
    does_not_exist(1);                // ✗ diag.unbound-name
}
""", title="An unbound name", expect="diag.unbound-name", section="s17", slug="unbound")

b += h3("Structs and fields across modules")
b += p("""A struct or enum declared in a module is an item like any other: export it to use it
outside. Each <em>field</em> has its own visibility too; write `export i32 x;` for fields that
callers may read and write directly, and leave the rest private to give the module full control
over its invariants.""")
b += code("""
module shapes
{
    export struct Rect
    {
        export i32 w;                 // visible to users of the module
        export i32 h;
        i32 cached_area;              // private: only this module's functions may touch it
    }
}
""", title="Field visibility")
b += code("""
module shapes
{
    export struct Rect
    {
        export i32 w;
        i32 cached_area;
    }
}

fn main()
{
    auto r = shapes::Rect { .w = 1, .cached_area = 0 };   // ✗ diag.name-not-visible: cached_area is private
}
""", title="A private field", expect="diag.name-not-visible", section="s17", slug="private-field")
b += p("""Naming a private field from outside its module — reading it, writing it, giving it in a
struct literal, or destructuring it — is `diag.name-not-visible`, the same diagnostic as for a
private function. Give the module a constructor function if callers need to build the struct.""")
b += p("""There are no read-only (`const`) fields: in CobaltC `const` means a value known before the
program runs (§11 constants), not "fixed once built". A field that other code may read but not
change is a private field with an exported function that reads it. Only the module can then change
it, which is also what keeps the module's invariant in one place.""")
b += code(CHECK_STD + """
module bank
{
    export struct Account
    {
        export u32 id;                // callers may read and write it
        i64 balance;                  // private: changed only by this module
    }

    export fn Account::open(u32 id, i64 deposit) : Account
    {
        std::assert(deposit >= 0, "an account opens with a deposit, not a debt");
        Account { .id = id, .balance = deposit }
    }

    export fn Account::balance(ref<Account, shared> a) : i64
    {
        a.balance
    }

    export fn Account::withdraw(ref<Account, exclusive> a, i64 amount) : bool
    {
        if (amount > a.balance)
        {
            return false;             // the invariant: the balance never goes below zero
        }
        a.balance -= amount;
        true
    }
}

fn main()
{
    auto acct = bank::Account::open(7, 100);
    assert(bank::Account::balance(&acct) == 100);   // read through the module
    assert(!bank::Account::withdraw(&mut acct, 500));
    assert(bank::Account::withdraw(&mut acct, 30));
    assert(bank::Account::balance(&acct) == 70);
    // acct.balance = 1000000;        // ✗ diag.name-not-visible: only bank can change it
}
""", title="A field others can read but not change", expect="ok", section="s17", slug="read-only-field")

b += rules([
    "Start every module with everything private; export only what callers need.",
    "Prefer qualified calls (`maths::square(x)`) in code that reads like a library; `import` the handful of names you use constantly.",
    "Module boundaries are not safety boundaries: `unsafe` is lexical (§20). To confine trusted code, keep the functions that wrap it un-exported.",
    "Split a program across files with `module m \"./m.cb\";`, one file per module; keep paths relative and let the declaring file do the naming.",
])
S17 = Section("s17", "§17", "Modules and name resolution", "17-modules.md",
              "module, export and import; re-export; private by default; a module's body may be another file; a fixed lookup order for every name.", b)

# ================================================================ §18

b = ""
b += p("""There are exactly two ways for something to go wrong in CobaltC, and they never mix. A
<strong>checked fault</strong> is the language catching a violated invariant (overflow, an index out
of range, a stale reference, a conflicting borrow): it is fatal. An <strong>error value</strong> is
your program reporting an expected failure (bad input, a full buffer) through `Result` or `Option`:
it is data, and the caller decides what to do. There are no exceptions and no catch.""")

b += h3("Checked faults")
b += p("""When a run-time check fails, the current thread unwinds every open scope, running every
destructor exactly as a normal scope exit would, and the program terminates reporting the fault by
name. Nothing can intercept this. The point is that a fault cannot corrupt state or leak an external
resource on the way out, and that a program that reaches its end has never once violated a checked
rule.""")
b += code("""
import std;

resource struct Guarded
{
    i32 tag;
}

fn Guarded::drop(ref<Guarded, exclusive> self)
{
    // runs during unwinding, before the program terminates
}

fn divide(i32 a, i32 b) : i32
{
    a / b
}

fn main()
{
    Guarded g = Guarded { .tag = 1 };
    Vec<i32> v = Vec::new();
    divide(1, 0);                     // ✗ diag.div-by-zero (dynamic): v and g are destroyed, then the program ends
}
""", title="A fault unwinds, then terminates", expect="diag.div-by-zero", section="s18", slug="fault")
b += h3("Assertions: `assert`")
b += p("""`assert(c)` states something the program relies on at that point: when `c` is false, the
program stops with the checked fault `diag.assert-failed`, naming the line, and unwinds like any
other fault. `assert(c, "format", args…)` adds a message, formatted as `printf` formats, so the
failure can say which values were wrong. The condition is always evaluated, exactly once: there is
no build in which assertions are left out. The message, and its arguments, are only evaluated when
the condition is false, so they cost nothing while it holds. Every example in this guide checks its
own results with `assert`; a condition that is constant is checked with `static_assert` (below)
before the program runs.""")
b += code("""
import std;

fn average(ref<Vec<u32>, shared> v) : u32
{
    assert(Vec::len(v) > 0, "average of an empty list");
    u32 sum = 0;
    foreach (x in v)
    {
        sum += *x;
    }
    sum / narrow<u32>(Vec::len(v))
}

fn main()
{
    Vec<u32> v = Vec::new();
    Vec::push(&mut v, 4);
    Vec::push(&mut v, 8);
    assert(average(&v) == 6);
    usize cap = 1;
    assert(Vec::len(&v) <= cap, "%v items exceed the capacity %v", Vec::len(&v), cap);
    // ✗ diag.assert-failed (dynamic), reporting "2 items exceed the capacity 1"
}
""", title="An assertion that fails, with its message", expect="diag.assert-failed", section="s18", slug="assert")
b += p("""Faults are named `diag.…` and listed in the appendix. Both the interpreter and a compiled
program print the name, the rule that fired, what was required and what was observed, and a repair
hint.""")

b += h3("Static assertions")
b += p("""`static_assert(c)` and `static_assert(c, "why")` check a constant condition when the program
is compiled, not when it runs: a false one stops it with `diag.static-assert-failed` and shows the
message. As with `assert`, the message may carry values, `static_assert(D == 201, "D is %v", D)`,
provided they are constants too. The condition is a constant expression (the same kind a `const` has, §11), so it can use
constants, `sizeof`, `alignof`, the limits and the conversions. It is checked in every function,
called or not, and in a generic function once for each type it is used with, so
`static_assert(sizeof<T>() <= 8, "…")` limits what `T` may be. At run time it does nothing. For a
condition only known as the program runs, use `assert` (above): a `static_assert` whose condition
names a local variable, calls a function or is not a `bool` is rejected before the program runs,
with `diag.static-assert-not-constant`, which says so.""")
b += code(CHECK_STD + """
struct Header
{
    u32 magic;
    u16 version;
    u16 flags;
    u64 length;
}

const usize BUF = 4096;

// Stores a value that must fit in one machine word.
fn word<T>(T x) : T
{
    static_assert(sizeof<T>() <= 8, "word<T> needs T to fit in 8 bytes");
    x
}

fn main()
{
    static_assert(sizeof<Header>() == 16, "Header must match the C struct");
    static_assert(BUF & (BUF - 1) == 0, "BUF must be a power of two");
    assert(word(5: i64) == 5 && word(2.5) == 2.5);
}
""", title="Checked before the program runs", expect="ok", section="s18", slug="static-assert")
b += code("""
import std;

fn main()
{
    i32 x = 1;
    static_assert(x == 1);            // ✗ diag.static-assert-not-constant: x is a variable;
                                      //   write assert(x == 1), or const i32 X = 1;
}
""", title="A variable is not a constant", expect="diag.static-assert-not-constant", section="s18", slug="static-assert-variable")
b += code("""
fn word<T>(T x) : T
{
    static_assert(sizeof<T>() <= 8, "word<T> needs T to fit in 8 bytes");
    x
}

fn main()
{
    word(1: i128);                    // ✗ diag.static-assert-failed (static): an i128 is 16 bytes
}
""", title="A failed assertion", expect="diag.static-assert-failed", section="s18", slug="static-assert-fails")

b += h3("`Result`, `Option` and `?`")
b += p("""`Result<T, E>` is `Ok(T)` or `Err(E)`; `Option<T>` is `Some(T)` or `None`. Both are plain
enums of `std`, taken apart with `match`. The postfix `?` operator unwraps a `Result`: on `Ok(v)` it
yields `v`, on `Err(e)` it returns `Err(e)` from the enclosing function immediately. That function
must therefore return `Result<_, E>` with exactly the same `E`; convert first with
`Result::map_err(r, f)` if the error types differ. On an `Option`, `?` works the same way inside a
function that itself returns an `Option`: `Some(v)` gives `v`, `None` returns `None` at once, so
`usize end = checked_add(start, n)?;` reads "the sum, or give up" (D-0130). Nothing converts between
the two: in a function returning a `Result`, `Option::ok_or(o, e)` turns the `Option` into a
`Result` — `Ok(v)` from `Some(v)`, `Err(e)` from `None` — so
`u8 c = Option::ok_or(peek(p), fail(p, "expected a value"))?;` reads "the byte, or return that
error". Where a default will do, `Option::unwrap_or(o, d)` and
`Result::unwrap_or(r, d)` give the value, or `d` when there is none; the default (and `ok_or`'s
error) is evaluated either way, and one that goes unused is destroyed. Where there must be a value,
`Option::unwrap(o)` and `Result::unwrap(r)` give it or fault with `diag.unwrap-failed`, and
`Option::expect(o, "…")` and `Result::expect(r, "…")` do the same with your message.""")
b += code(CHECK_STD + """
fn main()
{
    Vec<String> names = Vec::new();
    Vec::push(&mut names, String::from_str("ada"));
    Vec<String> copy = Vec::clone(&names);                    // each String copied by its clone (§12)
    String first = Option::expect(Vec::pop(&mut copy), "the copy has one name");
    overwrite(&mut first, String::from_str("grace"));         // the old String is destroyed here
    u32 n = Result::unwrap(String::parse<u32>(&String::from_str("42")));
    assert(String::eq_str(&first, "grace") && n == 42 && Vec::len(&names) == 1);
    Option<u32> none = None;
    u32 fails = Option::expect(none, "no value where one was promised");
    // ✗ diag.unwrap-failed (dynamic), reporting "no value where one was promised"
}
""", title="unwrap, expect, overwrite and clone_by", expect="diag.unwrap-failed", section="s18", slug="unwrap")
b += code(CHECK_STD + """
fn digit(u8 b) : Result<i32, u8>
{
    if (b > b'9' || b < b'0')         // not a digit
    {
        return Err(b);
    }
    Ok(widen<i32>(b - b'0'))
}

fn two_digits(u8 hi, u8 lo) : Result<i32, u8>
{
    i32 h = digit(hi)?;               // Err returns from two_digits right here
    i32 l = digit(lo)?;
    Ok(h * 10 + l)
}

fn main()
{
    match (two_digits(b'4', b'2'))
    {
        Ok(v)  :
        {
            assert(v == 42);
        },
        Err(_) :
        {
            assert(false, "unreachable");
        },
    }
    match (two_digits(b'4', b'x'))
    {
        Ok(_)  :
        {
            assert(false, "unreachable");
        },
        Err(e) :
        {
            assert(e == b'x');
        },
    }
}
""", title="Result and ?", expect="ok", section="s18", slug="propagate")
b += code("""
import std;

fn f() : i32
{
    Result<i32, i32> r = Ok(1);
    r?                                // ✗ diag.propagate-outside-fallible-context: f does not return Result<_, i32>
}

fn main()
{
    f();
}
""", title="? needs a matching return type", expect="diag.propagate-outside-fallible-context", section="s18", slug="propagate-ctx")
b += p("""Because `?` is just a `match` in disguise, it consumes a resource-bearing `Result` the
same way a `match` would, and the early `return` runs destructors like any other.""")
b += p("""A `Result` is never dropped silently: an expression statement whose value is a
`Result`, such as a bare call `write_all(&f, &data);`, is `diag.result-discarded`, since the error it
may carry would vanish. Handle it (`?`, `match`, `unwrap`), or say that you mean to ignore it with
`_ = e;`, which evaluates `e` and destroys the value at once. `_ = e;` works for any value.""")
b += code(CHECK_STD + """
fn check(i32 n) : Result<i32, str>
{
    if (n < 0) { Err("negative") } else { Ok(n) }
}

fn main()
{
    _ = check(-1);                    // ignored on purpose
    assert(Result::unwrap(check(2)) == 2);
}
""", title="Ignoring a Result on purpose", expect="ok", section="s18", slug="discard")
b += code("""
import std;

fn check(i32 n) : Result<i32, str>
{
    if (n < 0) { Err("negative") } else { Ok(n) }
}

fn main()
{
    check(-1);                        // ✗ diag.result-discarded: the error would vanish unseen
}
""", title="A discarded Result", expect="diag.result-discarded", section="s18", slug="result-discarded")

b += h3("Choosing a channel")
b += table(["Situation", "Use"], [
    ["Input that may legitimately be malformed, a container that may be empty, an allocation that may fail", "`Result` / `Option`; let the caller decide"],
    ["A condition that can only be false if the program is wrong (an index you computed, arithmetic on trusted values)", "let the language's checked fault fire; do not wrap it"],
    ["Recovering from overflow rather than faulting", "`checked_add` and friends return `Option` (§06)"],
    ["Recovering from allocation failure", "call `allocate` yourself; `Vec` and `Rc` treat failure as a fault"],
])

b += rules([
    "Reserve faults for bugs. If a failure is part of normal operation, return a `Result`.",
    "Use `?` to keep the happy path readable; keep error types consistent within a call chain and `map_err` at the boundaries.",
    "Trust the unwinding: a fault runs your destructors, so external handles are released even on the way down.",
])
S18 = Section("s18", "§18", "Errors and failure", "18-error-failure-semantics.md",
              "Fatal checked faults that unwind, assertions checked at run time or before the program runs, and recoverable Result values with the ? operator. Nothing in between.", b)

# ================================================================ §19

b = ""
b += p("""CobaltC's concurrency is small: threads you start and must join, and a mutex whose
contents can only be reached by locking it. The aliasing rules of §08 already forbid two threads
from touching one object in conflicting ways, so most of the work is done before this section
starts. What the primitives add is a way to <em>share mutable state on purpose</em>, safely.""")

b += h3("Threads: `spawn` and `join`")
b += p("""`spawn(f, args…)` runs `f(args…)` on a new thread and returns a `handle<R>`, where `R` is
`f`'s return type. `f` must be a named function or a `move` closure (a closure borrowing locals of
the spawning thread is rejected: `diag.spawn-borrow-closure`). Arguments are passed exactly as for a
call: plain values are copied, resources move to the new thread, references are lent. `join(h)`
waits for the thread and returns its result, consuming the handle. The result may itself be a
reference (or carry one, in a struct or an `Option`): the handle then holds the borrow until the
`join` hands it over, so the source cannot be written meanwhile. A handle is a resource: if it is
still alive when its scope ends, the thread is joined then and its result discarded, so a thread
never outlives its handle. What a thread borrows is still checked at every use, as in any call: if
the referent ends first (a handle kept in a variable declared outside the borrowed local's block,
or an element released by a reallocating `Vec::push`), the thread's next use of it is
`diag.stale-binding`.""")
b += code(CHECK_STD + """
fn work(i32 n) : i32
{
    n * 2
}

fn main()
{
    auto h = spawn(work, 21);         // the handle owns the thread
    assert(join(h) == 42);             // wait and take the result

    Vec<i32> data = Vec::new();
    Vec::push(&mut data, 5);
    auto h2 = spawn(move [data]()     // a move closure carries the Vec to the other thread
    {
        Vec::len(&data)
    });
    assert(join(h2) == 1);

    {
        auto h3 = spawn(work, 1);
    }                                 // unjoined: joined automatically here, result discarded
}
""", title="spawn and join", expect="ok", section="s19", slug="spawn-join")
b += code("""
fn main()
{
    i32 x = 5;
    auto h = spawn([x](i32 n)         // ✗ diag.spawn-borrow-closure: the closure borrows x
    {
        n + x
    }, 1);
    join(h);
}
""", title="A borrowing closure cannot be spawned", expect="diag.spawn-borrow-closure", section="s19", slug="spawn-borrow")
b += p("""A reference passed to `spawn` is held by the new thread until it finishes, so the
spawning thread may not use the referent in a conflicting way before `join(h)`. Whether the thread
happened to finish first would otherwise decide the outcome; the checker rejects the program
instead.""")
b += code("""
fn read(ref<i32, shared> r) : i32
{
    *r
}

fn main()
{
    i32 x = 1;
    auto h = spawn(read, &x);         // the thread borrows x until join(h)
    i32 y = x;                        // ✓ reading alongside a shared borrow
    x = 2;                            // ✗ diag.aliasing-conflict (static): the thread may still be reading x
    join(h);
}
""", title="A thread's borrows last until join", expect="diag.aliasing-conflict", section="s19", slug="spawn-borrow-until-join")

b += h3("Sharing state: `mutex` and `guard`")
b += p("""`Mutex::new(v)` wraps a value in a `mutex<T>`. The value inside is reachable only through
`lock(&m)`, which returns a `guard<T>`: an exclusive path into the interior that exists for as long
as the guard does. `*g` is the place inside. The guard is a resource; dropping it, or letting its
scope end, unlocks the mutex. Only one guard can exist at a time; another thread's `lock` waits.
Locking a mutex your own thread already holds is a fault, `diag.mutex-reentrant-lock`, not a
deadlock.""")
b += p("""A mutex is shared between threads by passing `&m`, a `ref<mutex<T>, shared>`. Any number
of threads may hold that shared reference; it is the one case where an exclusive path (the lock
guard) is allowed to coexist with shared references to the same object, because the lock itself
serialises access.""")
b += code(CHECK_STD + """
fn worker(ref<mutex<i32>, shared> m, i32 n) : i32
{
    foreach (i in 0..n)
    {
        auto g = lock(m);             // blocks until the mutex is free
        *g += 1;
    }                                 // each iteration's guard is destroyed here, unlocking
    n
}

fn main()
{
    auto m = Mutex::new(0);
    auto h1 = spawn(worker, &m, 3);   // both threads hold a shared reference to m
    auto h2 = spawn(worker, &m, 4);
    i32 a = join(h1);
    i32 b = join(h2);
    assert(a + b == 7);
    i32 total = *lock(&m);            // a temporary guard: it lives until the semicolon
    assert(total == 7);                // whatever the interleaving, every increment was serialised
}
""", title="A counter shared by two threads", expect="ok", section="s19", slug="mutex")
b += code("""
fn main()
{
    auto m = Mutex::new(0);
    auto g1 = lock(&m);
    auto g2 = lock(&m);               // ✗ diag.mutex-reentrant-lock: this thread already holds it
}
""", title="Locking twice on one thread", expect="diag.mutex-reentrant-lock", section="s19", slug="reentrant")

b += p("""Through a guard, a field or element of the locked value is reached directly, as through a
reference: `g.count`, `g.items[0]`, `Vec::push(&mut g.items, x)` mean `(*g).count` and so on. `*g`
is how you name the whole value.""")
b += p("""A `mutex` may also be a field: a struct holding one or more mutexes is shared by
reference like a bare one, each thread locking the field it needs (`lock(&board.scores)`). Each
mutex field is its own lock.""")
b += code(CHECK_STD + """
struct Stats
{
    u32 hits;
    Vec<u32> seen;
}

struct Board
{
    mutex<Stats> stats;
    mutex<u32> rounds;
}

fn play(ref<Board, shared> b, u32 me) : u32
{
    {
        auto g = lock(&b.stats);          // one mutex field...
        g.hits += 1;                      // g.hits is (*g).hits
        Vec::push(&mut g.seen, me);
    }
    auto r = lock(&b.rounds);             // ...and another, locked independently
    *r += 1;
    me
}

fn main()
{
    Board b = Board { .stats = Mutex::new(Stats { .hits = 0, .seen = Vec::new() }), .rounds = Mutex::new(0: u32) };
    {
        auto x = spawn(play, &b, 1);
        auto y = spawn(play, &b, 2);
        assert(join(x) + join(y) == 3);
    }
    auto g = lock(&b.stats);
    assert(g.hits == 2 && Vec::len(&g.seen) == 2 && *lock(&b.rounds) == 2);
}
""", title="A struct of mutexes shared by two threads", expect="ok", section="s19", slug="mutex-fields")

b += h3("Passing values: `Channel`")
b += p("""A `Channel<T>` from `std` (`std::sync`) is a queue between threads, shared by reference as a mutex is.
`Channel::new(cap)` holds at most `cap` values (at least 1). `Channel::send(&ch, v)` moves `v` in,
waiting while the channel is full; `Channel::recv(&ch)` moves the oldest value out as `Some(v)`,
waiting while it is empty. A value in a channel has one owner, the channel, so a `String` or a
`Vec` travels between threads without being shared or copied. A waiting thread uses no processor
time.""")
b += p("""`Channel::close(&ch)` says nothing more is coming: the values already sent are still
received, and then `recv` gives `None`, which is what ends a receiver's loop. A `send` after
`close` does not lose its value: it gives it back as `Err(v)`. Destroying a channel destroys the
values left in it, oldest first. The usual shape is a pool of workers: the main thread sends the
jobs, closes the channel, and joins the workers, each of which stops at `None`.""")
b += code(CHECK_STD + """
fn worker(ref<Channel<u64>, shared> jobs, ref<Channel<u64>, shared> results)
{
    while (true)
    {
        match (Channel::recv(jobs))
        {
            Some(n) : Channel::send(results, n * n),
            None    : break,                     // closed and empty: this worker is done
        }
    }
}

fn main()
{
    Channel<u64> jobs = Channel::new(4);        // at most 4 jobs waiting at once
    Channel<u64> results = Channel::new(16);
    {
        auto a = spawn(worker, &jobs, &results);
        auto b = spawn(worker, &jobs, &results);
        foreach (i in 1..11: u64)
        {
            _ = Channel::send(&jobs, i);            // waits while the channel is full
        }
        Channel::close(&jobs);                  // the workers stop once it is empty
        join(a);
        join(b);
    }
    Channel::close(&results);
    u64 total = 0;
    while (Some(sq) = Channel::recv(&results))   // None once it is closed and empty
    {
        total += sq;
    }
    assert(total == 385);                        // 1 + 4 + … + 100, whichever worker did which

    Channel<String> names = Channel::new(1);
    Channel::close(&names);
    match (Channel::send(&names, String::from_str("late")))
    {
        Ok(_)  : assert(false, "unreachable"),
        Err(s) : assert(String::len(&s) == 4),   // closed: the value comes back
    }
}
""", title="A pool of workers", expect="ok", section="s19", slug="channel")
b += p("""A wait that can never end because no other thread exists — the main thread receiving
from an empty channel, or sending to a full one, with no spawned thread running — is a fault,
`diag.channel-deadlock`. Other deadlocks are not detected.""")
b += code("""
import std;

fn main()
{
    Channel<i32> ch = Channel::new(1);
    _ = Channel::send(&ch, 1);
    _ = Channel::send(&ch, 2);            // ✗ diag.channel-deadlock: full, and no one to receive
}
""", title="Waiting alone", expect="diag.channel-deadlock", section="s19", slug="channel-deadlock")

b += h3("What is not provided")
b += p("""No atomics, no memory orderings, no condition variables, no thread-local storage, and
deadlocks are detected only in the channel case above. Steps of different threads interleave at
the granularity of single operations and every operation sees a consistent state; a `join`, a
`lock`, a `send` or a `recv` that cannot proceed simply waits. `Rc<T>` is not thread-safe; to share
across threads, share a `mutex` or pass values through a `Channel`.""")
b += note("""The compiler `cobc` runs threads in parallel on separate cores; the interpreter `coby`
runs them one at a time. A program that follows the rules gets the same result from both. Where the
language leaves an outcome to timing — writing a variable while another thread may still be reading
it through a reference is `diag.aliasing-conflict` if the thread is still running, and fine if it has
finished — each is free to show either permitted outcome, and they may differ.""", kind="note", title="Parallel or interleaved")

b += rules([
    "Every thread has an owner: the handle. Join it explicitly to get the result; let scope end join it if you do not care.",
    "Share a `mutex` by shared reference; keep each guard in the smallest block that needs it.",
    "To hand values from thread to thread, send them through a `Channel`; `close` it when the last value is sent.",
    "Move data into a thread with a `move` closure or by-value arguments; lend it with references only when the mutex rule allows.",
])
S19 = Section("s19", "§19", "Concurrency", "19-concurrency.md",
              "Threads owned by handles, mutexes reachable only through guards, channels that move values between threads, and the same aliasing rules everywhere.", b)

# ================================================================ §20

b = ""
b += p("""Everything so far is checked. This section is about the doors to the outside world:
raw addresses, manual allocation and foreign functions. CobaltC does not pretend those can be
checked; instead it makes the boundary explicit. Each trusted operation is only legal inside an
`unsafe { }` block, and the block is your signed statement that the operation's preconditions
hold.""")

b += h3("`unsafe` blocks")
b += p("""`unsafe { … }` is an ordinary block (it has a value, it opens a scope) with one extra
permission: the trusted operations listed below may appear inside it. Outside, they are rejected
with `diag.trusted-outside-unsafe`. Nothing else changes: the aliasing, ownership and initialisation
rules still apply to every ordinary access inside an `unsafe` block.""")
b += table(["Operation", "What you assert"], [
    ["`*p` (read) / `*p = v` (write) for `p : rawptr<T>`", "`p` addresses `sizeof<T>()` initialised bytes of a `T`, no live reference reaches them, and no other thread is writing them"],
    ["`p + n` (pointer offset, `n : isize`)", "the result stays within, or one past the end of, one allocation or object"],
    ["`reclaim<T>(p)`", "the bytes at `p` hold a valid `T` that no ordinary object owns; you now treat them as an object"],
    ["`release(p, n)`, `copy_raw(dst, src, n)`, `deallocate(p, n, align)`", "the ranges are valid and nothing live overlaps them; `deallocate` matches one earlier `allocate`"],
    ["a call to an `extern fn`", "the foreign code honours its signature and touches only what the pointer arguments reach"],
])

b += h3("Raw pointers")
b += p("""`rawptr<T>` is an address. It has no mode, no lifetime, and no checks; it is plain and
copyable. Getting one is safe: `rawptr_of(&x)` (or `rawptr_of(&mut x)`) takes the address of a
place, and `reinterpret_ptr<U>(p)` changes the pointee type without changing the address. Using one
to reach memory is trusted.""")
b += code(CHECK_STD + """
fn read_through(rawptr<i32> p) : i32
{
    unsafe
    {
        *p                            // trusted: the caller passed a pointer to a live i32
    }
}

fn main()
{
    i32 x = 7;
    assert(read_through(rawptr_of(&x)) == 7);
    auto bytes = [1: u8, 2: u8, 3: u8];
    rawptr<u8> b = reinterpret_ptr<u8>(rawptr_of(&bytes));
    unsafe
    {
        assert(*(b + 2) == 3);         // pointer offset, in units of the pointee size
    }
}
""", title="Raw reads, writes and offsets", expect="ok", section="s20", slug="rawptr")
b += code("""
fn bad(rawptr<i32> p) : i32
{
    *p                                // ✗ diag.trusted-outside-unsafe
}

fn main()
{
    i32 x = 1;
    bad(rawptr_of(&x));
}
""", title="No unsafe block", expect="diag.trusted-outside-unsafe", section="s20", slug="no-unsafe")

b += h3("From raw bytes back to objects: `reclaim`")
b += p("""Raw memory is not an object until you say it is. `reclaim<T>(p)` is a <em>place</em>
expression: it establishes (or re-attaches to) an object of type `T` over the bytes at `p`, and
from then on every ordinary rule applies to it. You can borrow it (`&reclaim<T>(p)`), assign to
it, or `drop` it. Reclaiming the same address twice gives the same object, so two references
obtained that way are checked against each other like any two references. This is how `Vec`
hands out references to its elements.""")
b += code(CHECK_STD + """
fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 7);
    auto p = rawptr_of(&v[0]);                      // the element's address; obtaining it is safe
    unsafe
    {
        auto r = &reclaim<i32>(p);    // a checked reference to the element, via its address
        assert(*r == 7);
    }
}
""", title="reclaim", expect="ok", section="s20", slug="reclaim")

b += h3("Manual allocation")
b += p("""`allocate(n, align)` returns `Result<rawptr<u8>, AllocError>`: fresh uninitialised bytes,
or `Err`. It is safe to call (failure is a value). `deallocate(p, n, align)` gives them back and is
trusted. A resource written into raw memory with `*p = v` <em>moves</em> there and is tracked; reading
`*p` of a resource type moves it back out. Plain data that holds references is tracked too: the cells
it is written to hold those references, exactly as a variable would, until they are released. The
library's `Vec::grow` in §21 is the reference example of the whole pattern.""")

SHAPES_C = """
#include <stdint.h>

int32_t area(int32_t w, int32_t h)
{
    return w * h;
}

// Fills out[0..n) with the first n squares; returns how many it wrote.
int64_t squares(int64_t *out, int64_t n)
{
    for (int64_t i = 0; i < n; i++)
    {
        out[i] = i * i;
    }
    return n;
}
"""
GEOMETRY_CB = """
import std;

extern "./shapes.c";                  // relative to this file

extern fn area(i32 w, i32 h) : i32;
extern fn squares(rawptr<i64> out, i64 n) : i64;

// u8 sides: the product always fits the C function's int32_t.
export fn rect_area(u8 w, u8 h) : i32
{
    unsafe { area(widen<i32>(w), widen<i32>(h)) }
}

// C fills storage from allocate; the count it returns is checked,
// then the values are copied out.
export fn first_squares(u8 n) : Vec<i64>
{
    Vec<i64> out = Vec::new();
    usize bytes = widen<usize>(n) * sizeof<i64>();
    rawptr<u8> raw = match (allocate(bytes, alignof<i64>()))
    {
        Ok(p)  : p,
        Err(_) :
        {
            return out;
        },
    };
    rawptr<i64> buf = reinterpret_ptr<i64>(raw);
    i64 written = unsafe { squares(buf, widen<i64>(n)) };
    if (written >= 0 && written <= widen<i64>(n))
    {
        foreach (i in 0..narrow<usize>(written))
        {
            Vec::push(&mut out, unsafe { *(buf + reinterpret<isize>(i)) });
        }
    }
    unsafe
    {
        deallocate(raw, bytes, alignof<i64>());
    }
    out
}
"""

b += h3("When a promise is false")
b += p("""An `unsafe` block's promises are not checked; that is what makes them promises. If one is
false — `*p` reads bytes that are not a valid `T`, `reclaim` claims cells another object owns, a
foreign function writes past what it was given — the program has left the language's rules at that
step. Everything it did before that step happened exactly as the rules say. From that step on, no
rule says what happens: the program may carry on, print nonsense, fault, or crash, and every one of
those is allowed. None of the guarantees in this guide — no use after free, no data race, no
out-of-bounds access — covers what comes after. A program with no `unsafe` block makes no promises,
so they cover all of it.""")
b += code("""
struct Holder
{
    Vec<i32> v;
}

fn main()
{
    Holder h = Holder { .v = Vec::new() };
    rawptr<Vec<i32>> p = reinterpret_ptr<Vec<i32>>(rawptr_of(&h));
    unsafe
    {
        auto taken = *p;              // false promise: h still owns these bytes
    }
    h.v = Vec::new();                 // from the line above on, no rule applies
}
""")
b += p("""The tools do not have to notice a broken promise, and in general they cannot. When one does,
it is encouraged to stop with a diagnostic rather than crash, since a report names the problem and a
crash does not; but that is a courtesy, not a rule. Keep `unsafe` blocks small, so that what each one
promises can be read and checked by eye.""")

b += h3("Foreign functions")
b += p("""`extern fn name(params) : ret;` declares a function provided from outside. Parameters and
result may only be integers, floats, `bool`, `void` and `rawptr<T>`; anything else, including a
`ref` or a struct, is rejected (`diag.extern-non-ffi-type`). Calling it needs `unsafe`. Its result
is a <em>claim</em>: a genuine value of its declared type, but one whose relationship to anything
else (that a returned length is right, that a pointer is valid) is not established until your code
checks it.""")
b += p("""`std` declares two externs: `stdout_write(rawptr<u8> buf, usize len) : isize`, which
`printf` (§21) writes through, with the `unsafe` block inside `std`, and its counterpart
`stdin_read`, which `read_line` reads through. Calling conventions and how code is linked are outside the language
and documented by each implementation; a program names the C code it needs with `extern "…";`
(below).""")
b += p("""Both implementations call real C functions on x86-64 Linux: an `extern fn` names a
function in the C library or the maths library by its declared name. The compiler passes any
FfiType, pointers included. The interpreter's memory is simulated, so it can pass only integers,
`bool` and floats, and reports a call that needs a pointer as unsupported rather than guessing.""")
b += code(CHECK_STD + """
extern fn sqrt(f64 x) : f64;          // from the maths library
extern fn labs(i64 x) : i64;          // from the C library
extern fn toupper(i32 c) : i32;

fn main()
{
    f64 root = unsafe
    {
        sqrt(2.0)
    };
    assert(root * root > 1.9999 && root * root < 2.0001);   // a claim, checked before it is trusted
    i64 distance = unsafe
    {
        labs(3 - 10)
    };
    assert(distance == 7);
    assert(unsafe { toupper(99) } == 67);                   // 'c' -> 'C'
}
""", title="Calling C functions", expect="ok", section="s20", slug="extern-libc")
b += code("""
import std;

fn main()
{
    auto msg = b"Hi\\n";                          // a byte-array literal: array<u8, 3>
    rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&msg));
    isize written = unsafe
    {
        stdout_write(p, 3)            // the extern call; its result is a claim about how much was written
    };
}
""", title="Calling the stdout_write extern", expect="ok", output="Hi\n", section="s20", slug="extern-write")
b += code("""
import std;

extern fn takes_vec(Vec<i32> v);      // ✗ diag.extern-non-ffi-type: a Vec cannot cross the boundary

fn main()
{
}
""", title="Only plain types cross an extern boundary", expect="diag.extern-non-ffi-type", section="s20", slug="extern-nonffi")

b += h3("Your own C code")
b += p("""`extern "…";` names C code for the compiler to link, so that the program's `extern fn`
declarations can be found in it. It can go anywhere an item can, declares no name, and can be
repeated; each piece of code is linked once. The program still builds with nothing but
`cobc main.cb`.""")
b += table(["Written", "Links", "Notes"], [
    ["`extern \"./shapes.c\";`", "a C source file", "compiled on its own, with the C compiler's warnings shown"],
    ["`extern \"./lib/fast.o\";`, `extern \"./lib/libfast.a\";`", "an object file or static library", "linked as it is"],
    ["`extern \"./lib/libplugin.so\";`", "a shared library", "found wherever the program is run from"],
    ["`extern \"z\";`", "an installed library", "as `cc -lz`"],
])
b += p("""A string beginning `./`, `../` or `/` is a file, and a relative path is relative to the
file it is written in, exactly as in `module m "./m.cb";`. So a module that wraps some C code can
name it next to itself, and every program that uses the module gets it linked. Anything else is a
library name. A missing file or library is reported at the line that names it.""")
b += note("""Only the compiler links C code. The interpreter accepts `extern "…";` and ignores it;
the first call into that code stops with `unsupported: no C function … which only cobc links`
(exit status 3), and everything before it runs as usual.""", kind="warn")
b += p("""The usual shape is a module that keeps the `unsafe` calls to itself and exports safe
functions. Two things make the wrappers below safe. First, `rect_area` takes `u8` sides, so the
product always fits in the C function's `int32_t`. Second, `first_squares` lets the C function fill
storage that comes from `allocate`, because a C function may write only outside the program's live
objects (§20's rule for extern calls); the count it returns is a claim, checked before it is used,
and the values are copied into a `Vec`.""")
b += code("""
import std;

module geometry "./geometry.cb";

fn main()
{
    printf("%v", geometry::rect_area(6, 7));
    printf("\\n");
    Vec<i64> sq = geometry::first_squares(5);
    foreach (x in &sq)
    {
        printf("%v ", x);
    }
    printf("\\n");
}
""", title="main.cb", expect="ok", output="42\n0 1 4 9 16 \n", section="s20", slug="extern-code", cobc_only=True,
    files={"geometry.cb": GEOMETRY_CB, "shapes.c": SHAPES_C})
b += code(GEOMETRY_CB, title="geometry.cb")
b += code(SHAPES_C, title="shapes.c")

b += h3("From claim to fact")
b += p("""Data from outside (bytes read by an extern, a pointer handed back by a library, a length
in a header) starts as a claim. The pattern for turning it into a fact is a validator that returns
`Result`: the library's `String::from_utf8(bytes)` (§21) is the canonical example. Once the
validator has accepted the bytes, the resulting `String` is guaranteed valid UTF-8 for the rest of
its life, because there is no operation that could break it.""")

b += rules([
    "Keep `unsafe` blocks tiny and wrap them in a safe function whose signature makes the precondition impossible to violate from outside.",
    "Never let a raw pointer stand in for a reference in an API. Return `ref<T, m>` (from `reclaim`) so callers get checked access.",
    "Validate external data once, at the boundary, into a type that cannot represent the invalid state.",
])
S20 = Section("s20", "§20", "Unsafe, raw pointers and FFI", "20-trust-boundaries.md",
              "unsafe blocks record what you assert; rawptr, reclaim, allocate and extern fn are the only doors to the outside.", b)

# ================================================================ §21

b = ""
b += p("""The standard library is the module `std`. It holds the `Option` and `Result` enums and
the library's error types; the containers and text types, written in CobaltC itself: `Vec<T>`,
`String`, `StringView`, `HashMap<K, V>`, `HashSet<K>`, `Queue<T>`, `PriorityQueue<T>`, `Box<T>`, `Rc<T>`, `Weak<T>` and `Channel<T>`
(§19); output, input and files: `printf`, `eprintf`, `sprintf`, `read_line`, `read_file`, `write_file`,
`File` and the directory functions; the program's arguments, `arg_count` and `arg`; and smaller helpers
such as `swap`, `replace`, `min`, `max`, `crc32` and `Rng`. A program uses it with `import std;`, or
by qualified paths such as `std::Vec`. Its source is part of the specification, which is the
language's proof that the mechanisms in this guide are enough to build real containers. A function
joins `std` when it names one very common intent, so that its name says what a loop or a `match`
would otherwise have to be read to discover; one that is only shorter, not clearer, stays out. Being
a module, it keeps its private parts private: no program can touch `Vec`'s length or buffer,
`String`'s bytes, a `HashMap`'s table or `Rc`'s count.""")
b += p("""`std` is divided into submodules by subject, and its root passes each one's items on as its
own (`export import`, §17), so `import std;` brings in everything and `std::Vec` names the same type
as `std::collections::Vec`. A program that wants one subject alone may import just that one:""")
b += table(["Submodule", "What it holds"], [
    ["`std::core`", "`Option`, `Result`, `AllocError`, `assert`, `swap`, `replace`, `overwrite`, `min`, `max`, `abs`, `pow`"],
    ["`std::collections`", "`Vec` (with sorting), `HashMap`, `HashSet`, `Queue`, `PriorityQueue`"],
    ["`std::text`", "`String`, `StringView`, `sprintf`, `String::appendf`, the ASCII functions, `Utf8Error`, `ParseError`, `read_le`, `read_be`, `crc32`"],
    ["`std::io`", "`printf`, `eprintf`, `read_line`, `File`, `FileError`, `read_file`, `write_file`, `read_bytes`, `write_bytes`"],
    ["`std::sys`", "`arg_count`, `arg`, the directory functions, `env_var`, `current_dir`, the clocks, `sleep_ms`"],
    ["`std::memory`", "`Box`, `Rc`, `Weak`"],
    ["`std::sync`", "`Channel`"],
    ["`std::random`", "`Rng`"],
    ["`std::math`", "`sqrt`, `floor`, `ceil`, `round`, `trunc`, `ln`, `exp`, `log2`, `log10`, `sin`, `cos`, `tan`, `atan2`, `powf`"],
])
b += code("""
import std::text;
import std::io;

fn main()
{
    String s = String::from_str("only text and output");
    printf("%s\\n", &s);
}
""", title="One subject at a time", expect="ok", output="only text and output\n", section="s21", slug="std-one-subject")
b += p("""This guide writes `import std;` and the short names throughout; each part below says which
submodule it lives in.""")
b += p("""<strong>The intrinsics and the text value type `str` are not part of `std`.</strong> They belong
to the language itself and need no import. This section keeps the two apart, as the specification's
tables do: the intrinsics first, then `std` at a glance, then each part of `std` in turn.""")

b += h3("Types in `std`")
b += code("""
export enum Option<T>
{
    Some(T),
    None
}

export enum Result<T, E>
{
    Ok(T),
    Err(E)
}

export struct AllocError
{
}

export struct Utf8Error
{
    export usize offset;              // where validation failed
}

export enum FileError                 // why reading or writing failed: a file, or standard input
{
    NotFound,
    Denied,
    Io,
    Utf8(Utf8Error)
}

export enum ParseError                // why parse failed
{
    Empty,
    Invalid(usize),                   // the offset where the number stops
    OutOfRange
}
""", title="Declared in std")

b += h3("Intrinsics: part of the language")
b += p("""Intrinsics are called like functions; their behaviour is given by rules rather than by a
body, and they need no import. Type arguments are written `name<T>(…)` where the signature needs
one. Their names are not reserved: a function or variable of your own with the same name is what the
name means, and the intrinsic is used only where there is none.""")
b += h4("Values and arithmetic")
b += table(["Intrinsic", "Purpose", "See"], [
    ["`drop(x)`", "destroy a resource now", "§07"],
    ["`wrapping_*`, `saturating_*`, `checked_*`", "alternative overflow policies", "§06"],
    ["`widen<U>`, `narrow<U>`, `checked_narrow<U>`, `narrow_wrapping<U>`, `reinterpret<U>`, `to_float<U>`, `to_int<U>`", "numeric conversions", "§06"],
    ["`count_ones(x)`, `leading_zeros(x)`, `trailing_zeros(x)`, `rotate_left(x, k)`, `rotate_right(x, k)`", "an integer's bits", "§06"],
    ["`sizeof<T>()`, `alignof<T>()`", "layout queries, in bytes", "§06, §16"],
    ["`min_value<T>()`, `max_value<T>()`", "a number type's smallest and largest value (a float's finite extremes)", "§06"],
])
b += h4("Text, slices, checks and faults")
b += table(["Intrinsic", "Purpose", "See"], [
    ["`str_len(s)`, `str_byte(s, i)`, `str_ptr(s)`", "a `str`'s byte count, one byte (bounds-checked), and the address of its bytes (safe)", "§21"],
    ["`slice_len(s)`", "a slice's number of elements (also through a reference)", "§16"],
    ["`static_assert(c)`, `static_assert(c, \"why\")`", "reject the program when the constant `c` is false", "§18"],
    ["`fault(name)`, `fault(name, msg)`", "stop with the run-time fault `diag.name` (a dynamic diagnostic, `_` for `-`, such as `index_out_of_bounds`), with `msg` if given; for code that implements containers as `std` does", "§18"],
])
b += h4("Raw memory and threads")
b += table(["Intrinsic", "Purpose", "See"], [
    ["`dangling<T>()`", "a well-aligned non-null address that must never be dereferenced; used for empty containers", "§21"],
    ["`rawptr_of(&x)`, `reinterpret_ptr<U>(p)`", "obtain and retype raw pointers (safe)", "§20"],
    ["`reclaim<T>(p)`, `release(p, n)`, `copy_raw(dst, src, n)`", "raw memory as objects (unsafe)", "§20"],
    ["`allocate(n, align)`, `deallocate(p, n, align)`", "raw allocation (`deallocate` is unsafe)", "§20"],
    ["`read_volatile(p)`, `write_volatile(p, v)`", "raw accesses performed exactly as written (unsafe; below)", "§20"],
    ["`Mutex::new(v)`, `lock(&m)`", "mutual exclusion", "§19"],
    ["`spawn(f, args…)`, `join(h)`", "threads", "§19"],
])
b += p("""`read_volatile(p)` and `write_volatile(p, v)` are raw accesses that the compiler must perform
exactly as written — never merged, dropped or reordered against another volatile access or a call —
for storage that changes on its own, such as a device register (D-0121). They take a `rawptr<T>` to
a plain `T`, inside `unsafe`; a `bitstruct` (§16) names the register's fields and
`Name::from_bits(read_volatile(p))` reads them.""")

b += h3("`std` at a glance")
b += p("""Everything here is an item of `std`, most of it written in CobaltC, and needs `import std;`
(or a qualified path). The containers and text types have their own parts of this section below; this
is the rest, for finding a name quickly.""")
b += h4("`Option`, `Result` and the errors")
b += table(["Function", "Purpose", "See"], [
    ["`Option::unwrap(o)`, `Option::expect(o, msg)`, `Result::unwrap(r)`, `Result::expect(r, msg)`", "the value, or the fault `diag.unwrap-failed` (with `msg`) when there is none", "§18"],
    ["`Option::unwrap_or(o, d)`, `Result::unwrap_or(r, d)`", "the value, or the default `d` when there is none", "§18"],
    ["`Option::ok_or(o, e)`", "the `Option` as a `Result`, `Err(e)` for `None`, so `?` applies to it", "§18"],
    ["`Option::is_some(&o)`, `is_none`, `Result::is_ok(&r)`, `is_err`", "which variant, without taking the value", "§18"],
    ["`Result::map_err(r, f)`", "convert a `Result`'s error type with `f`", "§18"],
    ["`FileError::text(&e)`, `ParseError::text`, `Utf8Error::text`", "a `std` error as a sentence for a person, a `str`", "§21"],
])
b += h4("Output, input and the program")
b += table(["Function", "Purpose", "See"], [
    ["`printf(\"%v\", x)`", "write a `str`, a number, a `bool`, a `String` (through `&`), a `StringView`, or an enum's variant name to standard output", "§21"],
    ["`printf(fmt, args…)`, `eprintf(fmt, args…)`, `sprintf(fmt, args…)`, `String::appendf(&mut s, fmt, args…)`", "C-style formatted output, to standard output, to standard error, as a new `String` or into one, checked when compiled", "§21"],
    ["`assert(c)`, `assert(c, fmt, args…)`", "fault with `diag.assert-failed` (and the formatted message) when `c` is false", "§18"],
    ["`read_line()`", "read the next line of standard input as a `String`", "§21"],
    ["`arg_count()`, `arg(i)`", "the number of program arguments, and argument `i` as a `String`", "§21"],
    ["`read_file(path)`, `write_file(path, text)`", "read a whole file as a `String`, or replace a file's contents; `path` and `text` are `StringView`s: a literal, or `&s[0..$]` of a `String`", "§21"],
    ["`stdout_write(buf, len)`, `stdin_read(buf, len)`, `arg_bytes(i, buf, len)`", "the externs beneath them: raw bytes to standard output, from standard input, and of argument `i` (unsafe)", "§20"],
])
b += h4("Numbers")
b += table(["Function", "Purpose", "See"], [
    ["`String::parse<T>(&s)`", "read a whole `String` as a number of type `T`", "§21"],
    ["`min(a, b)`, `max(a, b)`", "the smaller or larger of two values of an `ordered` type (§12)", "§21"],
    ["`abs(x)`, `pow(x, n)`", "absolute value and power (`n` a `u32`) of a `number` type (§12), checked as the operators they use", "§21"],
    ["`sqrt(x)`, `floor(x)`, `ceil(x)`, `round(x)`, `trunc(x)`", "an `f32` or `f64`'s square root and its roundings to an integral value (`round`: halfway cases away from zero), exact or correctly rounded, so the same bits everywhere", "§21"],
    ["`ln(x)`, `exp(x)`, `log2(x)`, `log10(x)`, `sin(x)`, `cos(x)`, `tan(x)`, `atan2(y, x)`, `powf(x, y)`", "an `f32` or `f64`'s logarithms, exponential, trigonometry and power, from the platform's maths library (within one unit in the last place; the two operands of `atan2` and `powf` of one type)", "§21"],
])
b += p("""`abs` and `pow` are checked like the operators they use: `abs` of a signed type's minimum, or a
power that does not fit, is `diag.arith-overflow`. The floating-point functions (`std::math`) take
an `f32` or an `f64` and give the same type; an integer is converted first, `sqrt(to_float<f64>(n))`.
The logarithms, the exponential, the trigonometric functions and `powf` come from the platform's
maths library, so their last bit may differ between platforms; `sqrt` and the roundings are exact
everywhere.""")

b += h3("`str`")
b += p("""CobaltC has two text types, and the short version is: <strong>`str` is a text constant,
`String` is text you own.</strong> Fixed text you write in your program is a `str`; text you build,
receive or keep ownership of is a `String` (below). The end of this section compares them side by
side.""")
b += p("""A string literal `"…"` is a value of type `str`: a sequence of bytes that is valid UTF-8
by construction, because the literal syntax has no way to write an arbitrary byte. A `str` is not a
reference and not a resource. It behaves exactly like an integer: copying it is free, nothing owns
it, nothing frees it, and you can pass it, store it in a struct or a `Vec<str>`, compare it with
`==`, and hand it to another thread without any annotation. Its bytes are reached with `str_len`
and `str_byte`; there is no indexing or slicing syntax. Escapes are `\\n \\r \\t \\0 \\\\ \\"` and
`\\u{…}` for a Unicode scalar value; a literal must close on its own line. Adjacent literals are
one literal, as in C, so longer text is written a line at a time: `"one\\n" "two\\n"` is the
single `str` `"one\\ntwo\\n"`, and a long `printf` format can be split the same way.""")
b += p("""`printf("%v", s)` writes a `str` to standard output. It prints numbers, `bool` and `String`
too (see "Printing: `printf`" below). The `unsafe` block that reaches the `stdout_write` extern is inside `std`, so a
program that only prints never writes `unsafe` itself.""")
b += code(CHECK_STD + """
struct Named
{
    str name;
    i32 n;
}

fn shout(str s)
{
    printf("%v!\\n", s);
}

fn main()
{
    str greeting = "hello";
    str again = greeting;             // copied, not moved: greeting stays valid
    assert(str_len(greeting) == 5);
    assert(str_byte(greeting, 0) == 104);
    assert(greeting == again);
    static_assert("caf\\u{e9}" != "cafe");     // \\u{e9} is é, two bytes
    assert(str_len("caf\\u{e9}") == 5);
    auto named = Named { .name = greeting, .n = 1 };
    auto copy = named;                // a struct of plain values is plain
    assert(copy.name == "hello");
    Vec<str> words = Vec::new();
    Vec::push(&mut words, greeting);
    Vec::push(&mut words, "world");
    shout(words[1]);
    printf("%v\\n", greeting);
}
""", title="str values", expect="ok", output="world!\nhello\n", section="s21", slug="str")
b += code("""
fn main()
{
    str s = "hi";
    u8 b = str_byte(s, 2);            // ✗ diag.index-out-of-bounds (static): a literal index, refuted ahead of time
}
""", title="str_byte is bounds-checked", expect="diag.index-out-of-bounds", section="s21", slug="str-oob")
b += p("""A byte literal `b"…"` is different: it is an `array<u8, N>`, exactly the array you
would have written by hand, and admits `\\xNN` for any byte value (and only ASCII characters
otherwise). Use it where bytes are the honest type, such as data handed to `stdout_write` or to a
validator.""")

b += h3("`Vec<T>`")
b += p("""<em>In `std::collections`.</em>""")
b += p("""A growable array with a raw buffer, a length and a capacity. It is a `resource` whose
destructor destroys each element and frees the buffer. A plain element is copied when read
(`v[i]`); a resource element is only borrowed, or moved out by `pop` or `remove`, never copied (so
`Vec<Vec<i32>>` works without any special support).""")
b += table(["Function", "Signature", "Notes"], [
    ["`Vec::new`", "`() : Vec<T>`", "`T` from the expected type or written `Vec<i32>::new()`; allocates nothing"],
    ["`Vec::len`", "`(ref<Vec<T>, shared>) : usize`", ""],
    ["`Vec::push`", "`(ref<Vec<T>, exclusive>, T)`", "moves a resource element in; grows 0 → 4 → 8 → …, which relocates elements"],
    ["`Vec::reserve`", "`(ref<Vec<T>, exclusive>, usize n)`", "room for `n` more elements, so the next `n` pushes do not grow it; the elements are unchanged (D-0129)"],
    ["`Vec::pop`", "`(ref<Vec<T>, exclusive>) : Option<T>`", "`None` when empty; moves a resource element out"],
    ["`Vec::insert`", "`(ref<Vec<T>, exclusive>, usize i, T)`", "puts the value at `i` (`i ≤ len`), moving the later elements up one"],
    ["`Vec::remove`", "`(ref<Vec<T>, exclusive>, usize i) : T`", "takes the element at `i` out, moving the later elements down one"],
    ["`Vec::index_shared`", "`(ref<Vec<T>, shared>, usize) : ref<T, shared>`", "bounds-checked: `diag.index-out-of-bounds`"],
    ["`Vec::index_exclusive`", "`(ref<Vec<T>, exclusive>, usize) : ref<T, exclusive>`", "as above, for writing"],
    ["`v[i]`, `&v[i..j]`", "indexing and slicing (§16)", "`v[i]` reads through `index_shared`, writes through `index_exclusive`; `$` is `Vec::len(&v)`"],
    ["`Vec::clear`", "`(ref<Vec<T>, exclusive>)`", "destroys every element, first to last; keeps the buffer for reuse"],
    ["`Vec::truncate`", "`(ref<Vec<T>, exclusive>, usize n)`", "keeps the first `n` elements and destroys the rest, last first; `n` past the length changes nothing"],
    ["`Vec::clone`", "`(ref<Vec<T: clone>, shared>) : Vec<T>`", "a copy, element by element, each by `clone` (§12): `Vec<String>`, `Vec<Vec<u8>>`, …"],
    ["`Vec::clone_by`", "`(ref<Vec<T>, shared>, fn(ref<T, shared>) : T) : Vec<T>`", "a copy made by the function, one element at a time, when the copy wanted is not the type's own `clone`"],
    ["`Vec::from_slice`", "`(slice<T: clone, shared>) : Vec<T>`", "a new `Vec` of the slice's elements, each by `clone`: `Vec::from_slice(&bytes[4..$])`"],
    ["`Vec::filled`", "`(usize n, T x) : Vec<T>`", "`n` copies of `x` by `clone`: `Vec::filled(80, b' ')`, `Vec::filled(3, Vec::filled(4, 0: u8))`; `T: clone`"],
    ["`Vec::from_fn`", "`(usize n, fn(usize) : T) : Vec<T>`", "`f(0)`, `f(1)`, …, `f(n - 1)`, any `T`: a grid is `Vec::from_fn(rows, [](usize r) : Vec<u8> { Vec::filled(cols, 0) })`"],
    ["`Vec::append`", "`(ref<Vec<T>, exclusive>, Vec<T> b)`", "moves `b`'s elements onto the end, in order; `b` is consumed"],
    ["`Vec::extend_from`", "`(ref<Vec<T>, exclusive>, slice<T, shared>)`", "copies a slice's elements onto the end by `clone`: `Vec::extend_from(&mut out, &bytes[0..$])`; `T: clone`"],
    ["`Vec::position`", "`(ref<Vec<T>, shared>, fn(ref<T, shared>) : bool) : Option<usize>`", "the first index whose element satisfies the test, or `None`; a closure may capture the key it looks for"],
    ["`Vec::index_of`, `Vec::contains`", "`(ref<Vec<T>, shared>, ref<T, shared>) : Option<usize>` / `: bool`", "the first index equal to the value, or whether there is one; `T: eq` — a number, `bool`, `str` or `String` (a struct takes `position`)"],
    ["`Vec::reverse`", "`(ref<Vec<T>, exclusive>)`", "the order inverted in place, by swaps; nothing is destroyed"],
    ["`Vec::push_le(&mut v, x)`, `Vec::push_be`", "`(ref<Vec<u8>, exclusive>, T: integer)`", "`x`'s bytes onto a byte `Vec`, least (`le`) or most (`be`) significant first; any width, signed as two's complement (D-0119)"],
    ["`read_le<T>(&v[0..$], at)`, `read_be<T>`", "`(slice<u8, shared>, usize) : T`", "the `T` whose bytes start at `at`; `diag.index-out-of-bounds` when fewer than `sizeof<T>()` remain"],
    ["`crc32(&v[0..$])`", "`(slice<u8, shared>) : u32`", "CRC-32/IEEE, the checksum of zlib, gzip and PNG; `crc32` of `\"123456789\"`'s bytes is `0xCBF43926` (§21 of the specification writes the algorithm out)"],
])
b += p("""When you know how many elements are coming, say so first: `Vec::reserve(&mut v, n)`
makes room for `n` more at once, so filling the `Vec` allocates and copies once instead of every
time it doubles. `String::reserve(&mut s, n)` does the same for bytes. Neither changes what the
program computes: no function reports a capacity, the length and the elements stay as they were,
and reserving room that is already there does nothing. Like a `push` that grows, a `reserve` that
grows moves the elements, so a reference into the old cells is stale afterwards. `std`'s own
builders (`String::from_str`, `String::clone`, `Vec::from_slice`) already reserve what they
need.""")
b += code(CHECK_STD + """
fn main()
{
    Vec<u32> squares = Vec::new();
    Vec::reserve(&mut squares, 1000);         // one allocation for all of them
    foreach (i in 0..1000: u32)
    {
        Vec::push(&mut squares, i * i);
    }
    assert(Vec::len(&squares) == 1000 && squares[999] == 998001);

    String line = String::new();
    String::reserve(&mut line, 80);
    foreach (i in 0..10)
    {
        String::append(&mut line, "-=-=-=-=");
    }
    assert(String::len(&line) == 80);
}
""", title="Room in advance", expect="ok", section="s21", slug="reserve")
b += code(CHECK_STD + """
fn main()
{
    Vec<i32> v = Vec::new();
    foreach (i in 0..9: i32)          // crosses two growth boundaries (4 and 8)
    {
        Vec::push(&mut v, i * i);
    }
    assert(Vec::len(&v) == 9);
    assert(*Vec::index_shared(&v, 8) == 64);
    {
        auto slot = Vec::index_exclusive(&mut v, 0);
        *slot = 100;                  // write through an exclusive element reference
    }                                 // slot must be gone before v is borrowed again
    assert(*Vec::index_shared(&v, 0) == 100);
    match (Vec::pop(&mut v))
    {
        Some(x) :
        {
            assert(x == 64);
        },
        None    :
        {
            assert(false, "unreachable");
        },
    }
    Vec<Vec<i32>> nested = Vec::new();
    Vec::push(&mut nested, v);        // v moves into the outer Vec
    assert(Vec::len(Vec::index_shared(&nested, 0)) == 8);
}                                     // nested is destroyed: each inner Vec, then the outer buffer
""", title="Using Vec", expect="ok", section="s21", slug="vec")
b += code("""
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 1);
    Vec::index_shared(&v, 5);         // ✗ diag.index-out-of-bounds (dynamic)
}
""", title="Vec bounds", expect="diag.index-out-of-bounds", section="s21", slug="vec-oob")
b += p("""The helpers of D-0112 name the loops programs kept writing: filling, building by index,
joining, searching and reversing. `filled` and `extend_from` copy plain elements; `from_fn` and
`append` are the forms for resources.""")
b += code(CHECK_STD + """
fn main()
{
    Vec<u8> row = Vec::filled(4, b'.');                                   // "...."
    Vec<Vec<u8>> grid = Vec::from_fn(3, [](usize r) : Vec<u8> { Vec::filled(4, b'.') });
    grid[1][2] = b'#';
    Vec<String> names = Vec::from_fn(3, [](usize i) : String { sprintf("n%d", i) });

    Vec<String> more = Vec::new();
    Vec::push(&mut more, String::from_str("x"));
    Vec::append(&mut names, more);                                        // more is consumed
    Vec::extend_from(&mut row, &grid[1][0..$]);                           // copies bytes
    assert(Vec::len(&names) == 4 && Vec::len(&row) == 8 && row[6] == b'#');

    String key = String::from_str("n1");
    assert(Vec::index_of(&names, &key) == Some(1));
    assert(Vec::contains(&row, &b'#') && !Vec::contains(&row, &b'?'));
    assert(Vec::position(&names, [key](ref<String, shared> s) : bool { *s == key }) == Some(1));
    Vec::reverse(&mut names);
    assert(String::eq_str(&names[0], "x"));
}
""", title="Filling, joining, searching, reversing", expect="ok", section="s21", slug="vec-helpers")
b += p("""A `String` in hand can be searched and split directly: `String::find`, `starts_with`,
`ends_with`, `contains`, `replace`, `trim` and `split` are the `StringView` functions applied to the whole-string view
`&s[0..$]`. And a map keyed by `String` can be asked with a literal: `HashMap::get_str`,
`contains_str` and `remove_str` find the entry a `String` of the same text would, without building
one (D-0113).""")
b += code(CHECK_STD + """
fn main()
{
    String line = String::from_str("  name = Ada, born = 1815  ");
    assert(String::trim(&line) == "name = Ada, born = 1815");
    assert(String::find(&line, "born") == Some(14) && String::ends_with(&line, "  "));

    HashMap<String, i64> fields = HashMap::new();
    foreach (part in String::split(&line, ","))
    {
        Vec<StringView> kv = StringView::split(part, "=");
        match (StringView::parse<i64>(StringView::trim(kv[1])))
        {
            Ok(n)  : { HashMap::insert(&mut fields, String::from_view(StringView::trim(kv[0])), n); },
            Err(_) : {},
        }
    }
    assert(HashMap::contains_str(&fields, "born") && !HashMap::contains_str(&fields, "name"));
    match (HashMap::get_str(&fields, "born"))
    {
        Some(y) : assert(*y == 1815),
        None    : assert(false),
    }
    assert(HashMap::remove_str(&mut fields, "born") == Some(1815) && HashMap::len(&fields) == 0);
}
""", title="Text and maps without extra Strings", expect="ok", section="s21", slug="str-lookups")
b += p("""Binary data: `Vec::push_le` and `Vec::push_be` write an integer's bytes in a fixed order and
`read_le`/`read_be` read them back, so a file header or a network message never spells a shift by
hand; `crc32` is the CRC-32/IEEE checksum those formats use (D-0119). A `bitstruct` (§16) crosses
into bytes the same way, through `Name::bits(v)`.""")
b += code(CHECK_STD + """
fn main()
{
    Vec<u8> header = Vec::new();
    Vec::push_be(&mut header, 0x89504E47: u32);          // a PNG signature's first four bytes, big-endian
    Vec::push_le(&mut header, 640: u16);                  // a little-endian width
    Vec::push_le(&mut header, -1: i8);
    assert(Vec::len(&header) == 7 && header[0] == 0x89 && header[4] == 0x80 && header[5] == 2);
    assert(read_be<u32>(&header[0..$], 0) == 0x89504E47 && read_le<u16>(&header[0..$], 4) == 640);
    assert(read_le<i8>(&header[0..$], 6) == -1);
    String text = String::from_str("123456789");
    assert(crc32(&String::as_bytes(&text)[0..$]) == 0xCBF43926);
}
""", title="Bytes in a fixed order, and a checksum", expect="ok", section="s21", slug="bytes-crc32")
b += p("""`Rng` is a general-purpose pseudo-random generator — for simulations, games, sampling,
randomised tests, shuffling — whose sequence the specification fixes, so a program that seeds it
with a constant prints the same numbers under both tools and on every machine; seed it from
`monotonic_ns()` for a fresh sequence. `below(&mut r, n)` is unbiased, `unit_f64` is in `[0, 1)`.
It is **not suitable for cryptographic purposes**: its output is predictable from a few values, so
never use it for keys, tokens, nonces, passwords, or anything an adversary must not guess (D-0120).""")
b += code(CHECK_STD + """
fn main()
{
    Rng r = Rng::new(42);
    assert(Rng::next_u64(&mut r) == 1546998764402558742);      // the specification's check value
    Rng dice = Rng::new(7);
    u64 total = 0;
    foreach (k in 0..100)
    {
        total += Rng::below(&mut dice, 6) + 1;                  // a die: 1 .. 6
    }
    assert(total >= 100 && total <= 600);
    f64 x = Rng::unit_f64(&mut dice);
    assert(x >= 0.0 && x < 1.0);
}
""", title="A reproducible random sequence", expect="ok", section="s21", slug="rng")
b += p("""A `Vec` can hold references. While it does, they count as live borrows: the referent
cannot be written past them, and a stored `&mut` is the only way in until the element is popped or
the `Vec` is destroyed.""")
b += code(CHECK_STD + """
fn main()
{
    i32 a = 10;
    i32 x = 1;
    Vec<ref<i32, shared>> readers = Vec::new();
    Vec::push(&mut readers, &a);
    assert(*readers[0] == 10);

    Vec<ref<i32, exclusive>> writers = Vec::new();
    Vec::push(&mut writers, &mut x);
    *writers[0] = 7;                                 // write through the stored reference
    Vec::pop(&mut writers);                          // the Vec lets go of it
    assert(x == 7);
    x = 8;                                           // x is free again
}
""", title="A Vec of references", expect="ok", section="s21", slug="vec-refs")
b += code("""
import std;

fn main()
{
    i32 x = 1;
    Vec<ref<i32, exclusive>> writers = Vec::new();
    Vec::push(&mut writers, &mut x);
    x = 5;                            // ✗ diag.aliasing-conflict: the Vec still holds &mut x
}
""", title="Writing past a stored borrow", expect="diag.aliasing-conflict", section="s21", slug="vec-refs-conflict")
b += note("""`Vec::push` is written in CobaltC: a `resource struct` with a `rawptr<T>`, an
`unsafe` write of the new element at `ptr + len`, and a private `grow` that calls `allocate`,
`copy_raw` and `deallocate`. Reading it in the specification is the best way to see how the
pieces of §07 and §20 fit together.""", kind="tip")

b += h3("`String`")
b += p("""<em>In `std::text`.</em>""")
b += p("""A `String` is a `Vec<u8>` that is known to be valid UTF-8: owned, heap-allocated text,
a resource. `String::new()` makes an empty one, `String::from_str` copies a `str`'s bytes
(already valid, so nothing is checked), and `String::from_utf8` validates bytes that came from
anywhere else. It changes only through functions that keep it valid UTF-8 — `append` and
`appendf` add text, `push_ascii` an ASCII byte, `truncate` cuts at a character boundary, `clear`
empties it — so the guarantee holds for its whole life. `from_utf8` is the trust-transition pattern of §20 in
miniature.""")
b += table(["Function", "Signature"], [
    ["`String::new`", "`() : String` (empty)"],
    ["`String::from_str`", "`(str) : String` (copies; cannot fail)"],
    ["`String::from_utf8`", "`(Vec<u8>) : Result<String, Utf8Error>`"],
    ["`String::len`", "`(ref<String, shared>) : usize` (bytes)"],
    ["`String::into_bytes`", "`(String) : Vec<u8>` (gives the bytes back, consuming the string)"],
    ["`String::as_bytes`", "`(ref<String, shared>) : ref<Vec<u8>, shared>` (read the bytes in place)"],
    ["`String::append`", "`(ref<String, exclusive>, T)` for any `T` `%v` takes: adds what `printf(\"%v\", x)` would write"],
    ["`String::clone`", "`(ref<String, shared>) : String` (a copy with the same bytes)"],
    ["`String::reserve`", "`(ref<String, exclusive>, usize n)`: room for `n` more bytes; the text is unchanged (D-0129)"],
    ["`String::eq`, `String::eq_str`", "`(ref<String, shared>, ref<String, shared>) : bool`, `(ref<String, shared>, str) : bool`: the same text? (`a == b` does the same for two `String`s, §12)"],
    ["`String::less`, `StringView::less`", "`(ref<String, shared>, ref<String, shared>) : bool`: byte order, a prefix first, as `a < b` (§12); `Vec::sort(&mut v)` sorts text"],
    ["`String::to_ascii_lower`, `String::to_ascii_upper`", "`(ref<String, shared>) : String`: a copy with ASCII letters changed; other bytes as they are"],
    ["`String::truncate`", "`(ref<String, exclusive>, usize n)`: keeps the first `n` bytes; `n` inside a character faults `diag.not-char-boundary`"],
    ["`String::push_ascii`", "`(ref<String, exclusive>, u8)`: adds one ASCII byte; one of 128 or more faults `diag.not-ascii`"],
    ["`String::clear`", "`(ref<String, exclusive>)` (empties it, keeping the buffer)"],
])
b += p("""For bytes, `std` has the ASCII tests and conversions a parser needs: `ascii_is_digit(b)`,
`ascii_is_alpha`, `ascii_is_alnum`, `ascii_is_upper`, `ascii_is_lower`, `ascii_is_space` (a space,
tab, newline, carriage return, vertical tab or form feed) take a `u8` and give a `bool`;
`ascii_to_lower(b)` and `ascii_to_upper(b)` give the `u8` with an ASCII letter's case changed.
Unicode case is not provided.""")
b += code(CHECK_STD + """
fn main()
{
    assert(ascii_is_digit(b'7') && !ascii_is_alpha(b'7') && ascii_is_space(b'\\t'));
    assert(ascii_to_upper(b'q') == b'Q');
    String shout = String::to_ascii_upper(&String::from_str("caf\\u{e9} au lait"));
    assert(String::eq_str(&shout, "CAF\\u{e9} AU LAIT"));
}
""", title="ASCII helpers", expect="ok", section="s21", slug="ascii")
b += code(CHECK_STD + """
fn main()
{
    String owned = String::from_str("caf\\u{e9}");   // a str's bytes, copied; no validation needed
    assert(String::len(&owned) == 5);                // 5 bytes: é is two of them
    assert(Vec::len(&String::chars(&owned)) == 4);   // but 4 characters

    Vec<u8> good = Vec::new();
    Vec::push(&mut good, 226);        // the three bytes of U+20AC, the euro sign
    Vec::push(&mut good, 130);
    Vec::push(&mut good, 172);
    String s = match (String::from_utf8(good))     // good moves in
    {
        Ok(v)  : v,
        Err(_) :
        {
            return;
        },
    };
    assert(String::len(&s) == 3);
    Vec<u8> back = String::into_bytes(s);          // and out again
    assert(Vec::len(&back) == 3);

    Vec<u8> bad = Vec::new();
    Vec::push(&mut bad, 226);         // a truncated sequence
    Vec::push(&mut bad, 130);
    match (String::from_utf8(bad))
    {
        Ok(_)  :
        {
            assert(false, "unreachable");
        },
        Err(e) :
        {
            assert(e.offset == 0);
        },
    }
}
""", title="Validating bytes into a String", expect="ok", section="s21", slug="string")

b += h3("`StringView`: part of a `String`, without copying")
b += p("""<em>In `std::text`.</em>""")
b += p("""`&s[i..j]` on a `String` gives a `StringView`: a read-only view of the bytes `i` to `j`,
with no copy. Positions are byte positions, `$` is the length, and both ends must fall on a
character boundary (cutting through a multi-byte character is `diag.not-char-boundary`). A view
prints with `%s`, compares with `==` to another view or a `str`, and can be sliced again,
`&v[1..$]`. It borrows its `String` exactly as a slice borrows a `Vec`: while the view lives, the
`String` cannot be changed, and a view cannot outlive it. `String::from_view(v)` copies it into a
`String` of its own when it must.""")
b += table(["Function", "Result"], [
    ["`StringView::len(v)`", "its length in bytes"],
    ["`StringView::of(t)`", "a view of a `str` `t`; a literal where a view is expected is one already (D-0134)"],
    ["`StringView::find(v, t)`", "`Option<usize>`: where the text of the view `t` first occurs"],
    ["`StringView::starts_with(v, t)`, `ends_with`", "`bool`"],
    ["`StringView::trim(v)`, `trim_start`, `trim_end`", "the view without ASCII white space at its ends"],
    ["`StringView::split(v, \",\")`", "a `Vec<StringView>` of the parts, each a view of the same `String`"],
    ["`StringView::chars(v)`", "a `Vec<StringView>` of its characters, in order, one view each (D-0128)"],
    ["`StringView::contains(v, t)`", "`bool`: whether the text of `t` occurs in `v` (D-0139)"],
    ["`StringView::replace(v, from, to)`", "a new `String`: `v` with every occurrence of `from` replaced by `to`, left to right, no overlap; an empty `from` occurs nowhere"],
    ["`StringView::join(&parts[0..$], sep)`, `String::join(&strings[0..$], sep)`", "a new `String`: the parts with `sep` between each two; the inverse of `split`"],
    ["`StringView::parse<T>(v)`", "a number, as `String::parse`"],
    ["`String::from_view(v)`, `String::append(&mut s, v)`", "the text copied into a `String`"],
    ["`String::find(&s, t)`, `starts_with`, `ends_with`, `contains`, `replace`, `trim`, `trim_start`, `trim_end`, `split`, `chars`, `as_view`", "the same, straight from a `String`: each is the `StringView::` function applied to `&s[0..$]`, the whole-string view (D-0113)"],
])
b += p("""Text that a function only reads is a `StringView` throughout `std`: a file's path, the text
to search for, the text to write (D-0134). A `String` passes its whole-string view, `&s[0..$]`. A
literal passes itself: a `str` literal where a `StringView` is expected is a view of it, and since a
`str` never ends, neither does the view. A `str` held in a variable is not a literal; view it with
`StringView::of(t)`. (`split`'s separator is the one `str`: `split` returns views of its first
argument, and a function returning views may borrow only one thing, §10.)""")
b += code(CHECK_STD + """
fn main()
{
    String text = String::from_str("the cat sat on the mat");
    String word = String::from_str("sat");
    assert(String::find(&text, &word[0..$]) == Some(8));   // a String to search for
    assert(String::find(&text, "mat") == Some(19));        // a literal: already a view
    str w = "on";
    assert(String::find(&text, StringView::of(w)) == Some(12));
    StringView label = "fixed";                            // a view of a literal, for the whole run
    printf("%s %d\\n", label, StringView::len(label));
}
""", title="Views of a String, of a literal, of a str", expect="ok", output="fixed 5\n", section="s21", slug="views-of-text")
b += code(CHECK_STD + """
// The first word of a line: a view into the line itself.
fn first_word(ref<String, shared> s) : StringView
{
    StringView t = StringView::trim(&(*s)[0..$]);
    match (StringView::find(t, " "))
    {
        Some(i) : &t[0..i],
        None    : t,
    }
}

fn main()
{
    String line = String::from_str("  set width=80, height = 24  ");
    assert(first_word(&line) == "set");

    StringView args = StringView::trim(&line[6..$]);     // "width=80, height = 24"
    i64 area = 1;
    foreach (field in StringView::split(args, ","))
    {
        match (StringView::find(field, "="))
        {
            Some(eq) :
            {
                StringView value = StringView::trim(&field[eq + 1..$]);
                match (StringView::parse<i64>(value))
                {
                    Ok(n)  : { area *= n; },
                    Err(_) : { assert(false, "unreachable"); },
                }
            },
            None     : { assert(false, "unreachable"); },
        }
    }
    assert(area == 1920);
    printf("%s -> %d\\n", args, area);
}
""", title="Reading fields out of a line", expect="ok", output="width=80, height = 24 -> 1920\n", section="s21", slug="stringview")
b += p("""Three questions come up so often that they have their own names (D-0139). `contains` is
`find` with a yes-or-no answer. `replace` makes a new `String` with every occurrence of one text
replaced by another, scanning left to right so occurrences never overlap; the empty text occurs
nowhere, so replacing it changes nothing. `join` is the inverse of `split`: the parts with a
separator between each two, from a slice of views (what `split` gives) or a slice of `String`s
(what a program builds). Each takes its texts as `StringView`s, so a literal, a `String`'s
`&s[0..$]` and another view all serve.""")
b += code(CHECK_STD + """
fn main()
{
    String path = String::from_str("/usr/local/lib/cobaltc");
    assert(String::contains(&path, "/local/") && !String::contains(&path, "bin"));

    String windows = String::replace(&path, "/", "\\\\");          // a new String; path unchanged
    printf("%s\\n", &windows);
    String aaa = String::from_str("aaa");
    printf("%s\\n", &String::replace(&aaa, "aa", "b"));            // left to right, no overlap: "ba"

    Vec<StringView> parts = String::split(&path, "/");          // "", "usr", "local", "lib", "cobaltc"
    printf("%s\\n", &StringView::join(&parts[1..$], " > "));
    assert(StringView::join(&parts[0..$], "/") == path);        // join undoes split

    Vec<String> names = Vec::new();
    Vec::push(&mut names, String::from_str("ada"));
    Vec::push(&mut names, String::from_str("grace"));
    printf("%s\\n", &String::join(&names[0..$], ", "));
}
""", title="contains, replace and join", expect="ok", output="\\usr\\local\\lib\\cobaltc\nba\nusr > local > lib > cobaltc\nada, grace\n", section="s21", slug="string-replace-join")
b += code(CHECK_STD + """
fn main()
{
    String name = String::from_str("ada");
    StringView v = &name[0..$];
    String::push_ascii(&mut name, b'!');   // ✗ diag.aliasing-conflict (static): v still views name
    assert(v == "ada");
}
""", title="The viewed String cannot change", expect="diag.aliasing-conflict", section="s21", slug="stringview-held")
b += h4("Going through the characters")
b += p("""`foreach` does not take a `String` itself, and there is no `char` type; `String::chars(&s)`
gives the characters instead: a `Vec<StringView>`, one view per character, in order. A character is
one to four bytes of UTF-8 (`é` is two, most emoji four), so `Vec::len` of it counts characters where
`String::len` counts bytes. Each view is text like any other: it prints, compares with `==` to a
`str`, is matched by text patterns, and appends to a `String`; and like any view it borrows the
`String` while it is in use. `StringView::chars(v)` does the same for a view, such as a word from
`split`. Building the `Vec` is one allocation; there is no form that visits the characters without
it, because `foreach` walks only the collections the language defines and there is no iterator
protocol for a library to plug into (D-0128 explains). Where that matters, walk
`String::as_bytes(&s)` instead.""")
b += code(CHECK_STD + """
fn main()
{
    String s = String::from_str("caf\\u{e9} \\u{1F600}!");
    foreach (c in String::chars(&s))
    {
        printf("[%v]", c);
    }
    printf("\\n%v characters in %v bytes\\n", Vec::len(&String::chars(&s)), String::len(&s));

    // Reversed by characters, so the two-byte é and the four-byte emoji stay whole.
    Vec<StringView> cs = String::chars(&s);
    Vec::reverse(&mut cs);
    String back = String::new();
    foreach (c in &cs)
    {
        String::append(&mut back, *c);
    }
    printf("%v\\n", &back);

    // Positions and text patterns work on each character.
    foreach (i, c in String::chars(&s))
    {
        match (c)
        {
            "\\u{e9}" : printf("\\u{e9} is character %v\\n", i),
            "!"      : printf("! is character %v\\n", i),
            _        : {},
        }
    }
}
""", title="The characters of a String", expect="ok",
     output="[c][a][f][\u00e9][ ][\U0001F600][!]\n7 characters in 11 bytes\n!\U0001F600 \u00e9fac\n\u00e9 is character 3\n! is character 6\n",
     section="s21", slug="string-chars")

b += h3("Which text type?")
b += p("""Rust programmers will recognise the pair, and C programmers may wonder why there are two
text types at all. The reason is that a constant should not have to be managed. If `"hi"` were a
`String`, every literal would be a heap allocation with an owner: it would be moved, destroyed at
scope end, and allocated again on every pass through a loop, all for text that never changes. So a
literal is a `str`, which behaves like the number `5`. `String` is kept for text that really does
need an owner.""")
b += table(["", "`str`", "`String`", "`b\"…\"`"], [
    ["What it is", "a text constant", "owned text", "raw bytes"],
    ["Written as", "`\"hello\"`", "`String::from_str(\"hello\")`, `String::from_utf8(bytes)`", "`b\"hello\"`"],
    ["Type", "`str`", "`String`", "`array<u8, N>`"],
    ["Valid UTF-8", "yes, by the literal grammar", "yes, by construction or validation", "not guaranteed"],
    ["Resource (moved, destroyed)", "no: copied like an integer", "yes", "no"],
    ["Allocates", "never", "yes", "no (an ordinary array)"],
    ["Passed to", "`printf`, `==`, `str_len`, `str_byte`; any `StringView` parameter", "`printf` (as `&s`), `String::len`, `String::into_bytes`; a `StringView` parameter as `&s[0..$]`", "`stdout_write` and other raw-byte code"],
])
b += rules([
    "Text written in your source code: use `str`. Pass it, store it in structs and `Vec<str>`, and print it freely.",
    "Text built at run time, or text you need to own: use `String`, created explicitly from a `str` with `String::from_str`.",
    "Part of a `String`, to read, compare, split or parse without copying: a `StringView`, `&s[i..j]`.",
    "A function that only reads text, from anywhere, takes a `StringView`, as `std`'s do (D-0134): a caller passes a literal as it is, `f(\"key\")`, and a `String` as `f(&s[0..$])`; nothing is copied.",
    "Bytes from outside the program (a file, a socket, FFI): they stay `Vec<u8>` until `String::from_utf8` accepts them.",
    "Bytes that are not text (a protocol header, data for `stdout_write`): use `b\"…\"`.",
])
b += p("""The two text types never convert into each other silently (§12's no-implicit-conversion
rule). Going from `str` to `String` is an explicit call that makes a copy, and nothing turns a
`String` back into a `str`: a `str` is always a literal's value, so it cannot refer to bytes that
something else owns.""")
b += h3("Printing: `printf`")
b += p("""<em>In `std::io` (`printf`, `eprintf`) and `std::text` (`sprintf`, `String::appendf`).</em>""")
b += p("""`printf` is the one way a program writes output. It is C's, with one difference that matters: the format is checked against its
arguments when the program is compiled. The format must be a string literal; each `%` specifier
must match its argument's type and there must be exactly one argument per specifier, or the
program is rejected (`diag.type-mismatch`, or `diag.format-invalid` for a malformed specifier). So
a `printf` can never misprint or crash at run time. `sprintf(fmt, args…)` returns the same text as a
new `String` (C's `sprintf` without a buffer to overflow), and `String::appendf(&mut s, fmt, args…)`
adds it to an existing one. `eprintf(fmt, args…)` writes the same text to standard error, for
messages that are not the program's output. There is no `println`; end a line with `\\n`.""")
b += table(["Specifier", "Takes", "Prints"], [
    ["`%d` `%i` `%u`", "any integer type", "decimal; no `%ld`/`%lld`/`%zu` needed, the type is known"],
    ["`%x` `%X` `%o` `%b`", "any integer type", "hex, octal, binary (`#` adds `0x`, `0X`, `0o`, `0b`)"],
    ["`%f` `%e` `%g` (and `%F` `%E` `%G`)", "`f32`, `f64`", "fixed, exponent, or shortest of the two, 6 digits by default"],
    ["`%s`", "`str`, `&String`, `StringView`, `bool`", "the text"],
    ["`%v`", "any of the above", "the value as CobaltC writes it (below)"],
    ["`%%`", "nothing", "a `%`"],
])
b += p("""Flags, a width and a precision work as in C: `%-8s` pads on the right, `%05d` pads with
zeros, `%+d` always shows a sign, `%.2f` rounds to two decimals, `%8.3f` does both. Widths count
characters, so text in any language lines up; to size a column to its text, count the characters
too, `Vec::len(&String::chars(&s))`, since `String::len` counts bytes. There is no `%c`. A `*` in place of the width or
the precision takes it from the next argument, which must be a `usize`: `printf("%*d", w, n)`,
`printf("%.*s", n, &s)`.""")
b += code("""
import std;

fn main()
{
    usize w = 6;
    printf("[%*d]\\n", w, 42);
    printf("[%-*s]\\n", w, "ab");
    printf("[%.*f]\\n", 2: usize, 3.14159);
}
""", title="Computed widths", expect="ok", output="[    42]\n[ab    ]\n[3.14]\n", section="s21", slug="printf-star")
b += code("""
import std;

fn main()
{
    String item = String::from_str("widget");
    u32 qty = 12;
    f64 price = 3.5;
    printf("%-8s|%5d|%8.2f|%#06x\\n", &item, qty, price, 255);
    printf("%.1f%% done, %e left\\n", 99.5, 0.00042);
    String line = sprintf("%s x %d", &item, qty);
    String::appendf(&mut line, " = %.2f", price * 12.0);
    printf("%v\\n", &line);
}
""", title="printf, sprintf and appendf", expect="ok",
     output="widget  |   12|    3.50|0x00ff\n99.5% done, 4.200000e-04 left\nwidget x 12 = 42.00\n",
     section="s21", slug="printf")
b += code("""
import std;

fn main()
{
    printf("%d items\\n", "twelve");   // ✗ diag.type-mismatch: %d takes an integer
}
""", title="Checked when compiled", expect="diag.type-mismatch", section="s21", slug="printf-mismatch")
b += code("""
import std;

fn main() : u8
{
    if (arg_count() != 1)
    {
        eprintf("usage: greet NAME\\n");   // to standard error, not mixed into the output
        return 2;
    }
    match (arg(0))
    {
        Ok(name) :
        {
            printf("hello, %s\\n", &name);
        },
        Err(e)   :
        {
            eprintf("greet: the name is not UTF-8 (byte %d)\\n", e.offset);
            return 1;
        },
    }
    0
}
""", title="Errors go to standard error: `coby greet.cb` with no argument", expect="ok", output="",
     section="s21", slug="eprintf", args=[], status=2)

b += h4("`%v`: any value")
b += p("""`%v` is not C's: it writes a value the way CobaltC writes it, whatever its type, so
`printf("%v\\n", x)` needs no thought about `x`. It takes any of the types below, and nothing else:
a struct or a `Vec` is a static type error, so printing a composite value means printing its
parts; an enum prints its variant's name. It takes a width and `-`, but no precision.""")
b += table(["Argument", "Written as", "Examples"], [
    ["`str`", "its bytes", "`printf(\"%v\", \"hi\")` → `hi`"],
    ["any integer type", "decimal, `-` if negative", "`-42`, `18446744073709551615`"],
    ["`f32`, `f64`", "the shortest digits that read back as the same value", "`0.1`, `1.0`, `0.3333333333333333`, `1.0e21`, `1.0e-7`, `NaN`, `inf`"],
    ["`bool`", "`true` or `false`", ""],
    ["`ref<String, shared>`", "the string's bytes", "`printf(\"%v\", &s)`"],
    ["`StringView`", "its text", "`printf(\"%v\", word)`"],
    ["an enum, or a reference to one", "its variant's name (not its payload)", "`Some`, `Mul`, `NotFound`"],
])
b += p("""A float prints as few digits as identify it exactly: `0.1` prints as `0.1`, even though
the stored value is slightly more than a tenth, and reading the printed text back gives the same
value. A whole number keeps a `.0`, so floats and integers look different. Between 0.000001 and
10<sup>21</sup> the number is written out in full; beyond that range it gets an exponent. An `f32` chooses
its digits as an `f32`, so `0.1: f32` also prints `0.1`. This is the rule JavaScript and Python
use.""")
b += p("""`%v`'s argument type is checked where the `printf` is, and a generic function can print
its own `T`. The check happens for each type the function is used with, so `show` below
accepts numbers and text, and would be rejected for a struct.""")
b += code("""
import std;

fn show<T>(T x)                       // T is checked where show is used
{
    printf("%v\\n", x);
}

fn main()
{
    show(-42);
    show(max_value<u64>());
    show(true);
    show(0.1);
    show(1.0);
    show(1.0 / 3.0);
    show(0.1: f32);
    show(1000000000000000000000.0);
    f64 zero = 0.0;
    show(zero / zero);

    String name = String::from_str("owned text");
    printf("%v", &name);                     // a String through a reference: name stays usable
    printf("\\n");
    printf("x = %v, y = %v\\n", 3, 4.5);
}
""", title="Printing values", expect="ok", output="-42\n18446744073709551615\ntrue\n0.1\n1.0\n0.3333333333333333\n0.1\n1.0e21\nNaN\nowned text\nx = 3, y = 4.5\n", section="s21", slug="print")
b += code("""
import std;

fn main()
{
    String s = String::from_str("text");
    printf("%v", s);                         // ✗ diag.type-mismatch (static): a String prints through &s
}
""", title="A String prints through a reference", expect="diag.type-mismatch", section="s21", slug="print-string-value")
b += code("""
import std;

struct Point
{
    i32 x;
    i32 y;
}

fn main()
{
    printf("%v", Point { .x = 1, .y = 2 });  // ✗ diag.type-mismatch (static): print the fields instead
}
""", title="Composite values are not printable", expect="diag.type-mismatch", section="s21", slug="print-struct")
b += p("""Raw bytes are not text, so `printf` does not take a `u8` as a character or a `Vec<u8>`:
a `u8` prints as a number. To write bytes, use the `stdout_write` extern the way `std` does, around an
`unsafe` block. `put` writes one byte and `print_bytes` a whole `Vec<u8>`:""")
b += code(CHECK_STD + """
// Write one byte to standard output.
fn put(u8 b)
{
    auto buf = [b];                   // an array<u8, 1>
    rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&buf));
    unsafe
    {
        stdout_write(p, 1);           // trusted: p points at one valid byte
    }
}

fn print_bytes(ref<Vec<u8>, shared> v)
{
    foreach (b in v)
    {
        put(*b);
    }
}

fn main()
{
    put(b'a');                        // the byte of `a`
    printf("%v", b'a');                      // a u8 prints as a number: 97
    put(b'\\n');
    Vec<u8> v = Vec::new();
    Vec::push(&mut v, b'o');
    Vec::push(&mut v, b'k');
    Vec::push(&mut v, b'\\n');
    print_bytes(&v);
}
""", title="Printing bytes", expect="ok", output="a97\nok\n", section="s21", slug="print-bytes")
b += code("""
import std;

fn main()
{
    String s = "hi";                  // ✗ diag.type-mismatch (static): "hi" is a str
}
""", title="A literal is not a String", expect="diag.type-mismatch", section="s21", slug="str-not-string")
b += code(CHECK_STD + """
fn main()
{
    String s = String::from_str("hi");   // ✓ the explicit, allocating bridge
    assert(String::len(&s) == 2);
}
""", title="The bridge is explicit", expect="ok", section="s21", slug="str-to-string")

b += h3("Reading input")
b += p("""<em>In `std::io`.</em>""")
b += p("""`read_line()` reads the next line of standard input. It returns a
`Result<Option<String>, FileError>`:""")
b += table(["Result", "Meaning"], [
    ["`Ok(Some(line))`", "the next line, without its `\\n` (a final `\\r\\n` is removed whole); a last line with no `\\n` counts too"],
    ["`Ok(None)`", "the input has ended (and every later call says so again)"],
    ["`Err(Utf8(e))`", "the line is not valid UTF-8; `e.offset` is the bad byte's position in the line, and the next call reads the line after it"],
    ["`Err(Io)`", "the input could not be read"],
])
b += p("""A line ends at `\\n`, and a `\\r` just before it is removed with it, so a line typed
on Windows reads the same as one typed on Linux. The line is a
`String` that has already been checked, so a program that reads never writes `unsafe`, and `?`
passes a `FileError` up to the caller: the same error a file gives (D-0134), whose `NotFound` and
`Denied` never come from standard input, so a `match` that names `Io` and `Utf8` ends with `_`.
Underneath is the `stdin_read` extern, the counterpart of `stdout_write`, which you can call
yourself inside `unsafe` to read raw bytes.""")
b += p("""Both `coby` and `cobc` read standard input, from the terminal or from a pipe, for example
`printf 'ada\\nlin\\n' | coby greet.cb`. Standard output is flushed before each read, so a prompt
printed with `printf` appears before the program waits.""")
b += code("""
import std;

// Greets every line of input; blank lines are skipped.
fn greet_all() : Result<usize, FileError>
{
    usize count = 0;
    while (Some(name) = read_line()?) // None: the end of input; an error goes back to main
    {
        if (String::len(&name) > 0)
        {
            printf("hello, %v\\n", &name);
            count += 1;
        }
    }
    Ok(count)
}

fn main()
{
    match (greet_all())
    {
        Ok(n) :
        {
            printf("%v greeted\\n", n);
        },
        Err(e) :
        {
            match (e)
            {
                Io :
                {
                    printf("could not read input\\n");
                },
                Utf8(u) :
                {
                    printf("not UTF-8 at byte %v\\n", u.offset);
                },
            }
        },
    }
}
""", title="Reading lines")

b += h3("Program arguments")
b += p("""<em>In `std::sys`.</em>""")
b += p("""`arg_count()` is the number of arguments the program was started with, and `arg(i)`
is argument `i` as a `Result<String, Utf8Error>`. Argument 0 is the first one after the program:
`coby prog.cb a b` and `cobc --run prog.cb a b` both give `a` and `b`, and the program's own name
is not an argument. Each argument is checked on its own, so one that is not UTF-8 is an `Err`
without affecting the others. `arg(i)` with `i ≥ arg_count()` faults `diag.index-out-of-bounds`,
as indexing a `Vec` does. Both tools pass arguments. Underneath is the `arg_bytes` extern, which
you can call inside `unsafe` for an argument's raw bytes.""")
b += code("""
import std;

fn main()
{
    foreach (i in 0..arg_count())
    {
        match (arg(i))
        {
            Ok(a)  : printf("%v: %v\\n", i, &a),
            Err(e) : printf("%v: not UTF-8 at byte %v\\n", i, e.offset),
        }
    }
}
""", title="Listing the arguments: `coby args.cb one two`", expect="ok", output="0: one\n1: two\n",
     section="s21", slug="list-args", args=["one", "two"])

b += h3("Numbers and text")
b += p("""<em>In `std::text`.</em>""")
b += p("""`String::append(&mut s, x)` adds to `s` exactly what `printf("%v", x)` would write, for the same
types: a `str`, any integer, `f32`/`f64`, `bool`, a `StringView`, or another `String` through `&`. So a line can be
built piece by piece and printed, or kept, once. `String::parse<T>(&s)` goes the other way: it reads the
whole `String` as a number of type `T`, any integer type, `f32` or `f64`, and returns a
`Result<T, ParseError>`.""")
b += table(["Result", "Meaning"], [
    ["`Ok(v)`", "the whole text is a number, and it fits in `T`"],
    ["`Err(Empty)`", "the text is empty"],
    ["`Err(Invalid(k))`", "byte `k` cannot continue a number (`\"12x4\"` → 2), or the text ends too early (`\"-\"` → 1)"],
    ["`Err(OutOfRange)`", "a number, but not one `T` can hold (`\"300\"` as `u8`)"],
])
b += p("""Parsing is strict: an optional `+` or `-`, then digits, and for a float an optional fraction and
exponent (`2.5e-3`), or `inf` or `NaN`. No spaces anywhere; `read_line` removes a Windows line’s
`\\r\\n` whole, so its lines parse, but a `\\r` from any other source is `Invalid`. Floats are rounded correctly, and every float `%v` writes parses back to
the same value. To take a line apart first, split, trim or search it as views (`String::split`,
`trim`, `find`, the `StringView` functions above) and parse a piece with `StringView::parse<T>`.""")
b += code("""
import std;

// Adds up its arguments: `coby sum.cb 1.5 2 x` reports the `x`.
fn main() : u8
{
    f64 total = 0.0;
    foreach (i in 0..arg_count())
    {
        String a = Result::unwrap_or(arg(i), String::new());
        match (String::parse<f64>(&a))
        {
            Ok(v)  : total += v,
            Err(_) :
            {
                String msg = String::new();
                String::append(&mut msg, "argument ");
                String::append(&mut msg, i);
                String::append(&mut msg, " is not a number: ");
                String::append(&mut msg, &a);
                String::append(&mut msg, "\\n");
                printf("%v", &msg);
                return 1;
            },
        }
    }
    String line = String::new();
    String::append(&mut line, "total ");
    String::append(&mut line, total);
    String::append(&mut line, "\\n");
    printf("%v", &line);
    0
}
""", title="Parsing arguments, building a line: `coby sum.cb 1.5 2 0.25`", expect="ok", output="total 3.75\n",
     section="s21", slug="sum-args", args=["1.5", "2", "0.25"])

b += h3("Files")
b += p("""<em>In `std::io`.</em>""")
b += p("""`read_file(path)` reads a whole file as a `String`; `write_file(path, text)` makes the
file hold exactly `text`, creating it or replacing what it held. Both take the path, and the text,
as a `StringView` (D-0134): a literal, `read_file("notes.txt")`, or a `String`'s whole-string view,
`read_file(&path[0..$])` for a path that came from `arg`. They return a `Result` whose error is a
`FileError`: `NotFound`,
`Denied`, `Utf8(e)` for a file that is not valid UTF-8 (with the offset of the first bad byte), or
`Io` for anything else. A relative path is relative to the directory the program runs in. Both
tools read and write files. `FileError::text(&e)` is the error as a sentence for a person
("not found", "permission denied", …), as `ParseError::text` and `Utf8Error::text` are for
theirs (D-0117); `%v` of an error prints its variant's name instead.""")
b += table(["Function", "Signature"], [
    ["`read_file`", "`(StringView path) : Result<String, FileError>`"],
    ["`write_file`", "`(StringView path, StringView text) : Result<void, FileError>`"],
])
b += p("""These two handle the whole file at once; `File`, below, reads and writes one in pieces.""")
b += code("""
import std;

fn main() : u8
{
    String text = String::from_str("first line\\nsecond line\\n");
    if (Err(_) = write_file("notes.txt", &text[0..$]))
    {
        printf("cannot write notes.txt\\n");
        return 1;
    }
    String back = Result::unwrap_or(read_file("notes.txt"), String::new());
    printf("%v", &back);
    match (read_file("no-such-file.txt"))
    {
        Ok(_)         : printf("?\\n"),
        Err(NotFound) : printf("no-such-file.txt: not found\\n"),   // a pattern inside a pattern
        Err(_)        : printf("no-such-file.txt: cannot read it\\n"),
    }
    0
}
""", title="Writing a file and reading it back", expect="ok", output="first line\nsecond line\nno-such-file.txt: not found\n",
     section="s21", slug="files")

b += h3("Open files: `File`")
b += p("""<em>In `std::io`.</em>""")
b += p("""A `File` is an open file, for what `read_file` cannot do: bytes that are not text, files
too large to hold, appending to a log. It is a resource, like `Vec`: it has one owner, can move to
another thread, and its destructor closes it. `close` closes it too, and reports whether that
worked (for a written file, whether the bytes were stored); after `close` the `File` is gone,
so it cannot be used again. Every operation returns a `Result` whose error is a `FileError`.""")
b += table(["Function", "Does"], [
    ["`File::open(path)`", "open for reading; the file must exist (`path` a `StringView`, as for `read_file`)"],
    ["`File::create(path)`", "open for writing, empty: created, or its contents discarded"],
    ["`File::append(path)`", "open for writing at the end, created if missing"],
    ["`File::open_rw(path)`", "open for reading and writing, created if missing, contents kept"],
    ["`File::read_line(&mut f)`", "the next line without its `\\n` (nor a `\\r` before it), `None` at the end (as `read_line`)"],
    ["`File::read(&mut f, &mut buf, max)`", "append up to `max` bytes to a `Vec<u8>`; how many, 0 at the end"],
    ["`File::read_to_end(&mut f, &mut buf)`", "append the rest of the file to a `Vec<u8>`"],
    ["`File::write(&mut f, &bytes[i..j])`, `File::write_text(&mut f, text)`", "write all of a slice of bytes, or text (a `StringView`: a literal, or `&s[0..$]`)"],
    ["`File::printf(&mut f, \"format\", …)`", "write formatted text, as `printf` formats it"],
    ["`File::seek(&mut f, pos)`, `File::len(&f)`", "move to byte `pos` (a `u64`); the length in bytes"],
    ["`File::close(f)`", "close it and say whether that worked"],
    ["`read_bytes(path)`, `write_bytes(path, &bytes[i..j])`", "`read_file` and `write_file` for bytes: a `Vec<u8>`"],
])
b += p("""Reading is buffered, so `read_line` is quick, and `File::read` fills up to what was asked for; each
write reaches the file before it returns, so gather many small pieces in a `String` or `Vec<u8>`
first when speed matters.""")
b += code("""
import std;

fn main()
{
    File log = Result::expect(File::create("app.log"), "cannot create the log");
    foreach (i in 1..4)
    {
        _ = File::printf(&mut log, "event %v\\n", i);   // `_ =`: the result is not needed
    }
    _ = File::close(log);

    File input = Result::expect(File::open("app.log"), "cannot open the log");
    printf("%v bytes\\n", Result::unwrap_or(File::len(&input), 0));
    while (Ok(Some(line)) = File::read_line(&mut input))   // stops at the end, or an error
    {
        printf("read: %s\\n", &line);
    }
    _ = File::seek(&mut input, 6);
    Vec<u8> bytes = Vec::new();
    _ = File::read(&mut input, &mut bytes, 1);
    printf("byte 6 is %v\\n", bytes[0]);
}
""", title="Writing a log, reading it line by line, and seeking", expect="ok",
     output="24 bytes\nread: event 1\nread: event 2\nread: event 3\nbyte 6 is 49\n",
     section="s21", slug="file-handle")

b += h3("Directories, the environment and the clocks")
b += p("""<em>In `std::sys`.</em>""")
b += p("""Files live in directories, which a program can make, list and clean up; it can also read its
environment and measure time. Paths are `StringView`s, as for files, and failures are `FileError`s.""")
b += table(["Function", "Does"], [
    ["`make_dir(path)`", "make a directory: `Ok(true)` made, `Ok(false)` one was already there"],
    ["`remove_file(path)`, `remove_dir(path)`", "remove a file, or an empty directory"],
    ["`rename(from, to)`", "move a file or directory, replacing a file at `to`"],
    ["`list_dir(path)`", "the names in a directory, sorted: a `Vec<String>`"],
    ["`path_kind(path)`", "`File(len)`, `Dir` or `Other`; `Err(NotFound)` when nothing is there"],
    ["`env_var(name)`", "`Ok(Some(value))`, or `Ok(None)` when the variable is not set"],
    ["`current_dir()`", "the working directory, which relative paths start from"],
    ["`monotonic_ns()`, `unix_seconds()`", "a clock for measuring time (never goes back), and the date"],
])
b += code("""
import std;

fn kind(StringView p) : str
{
    match (path_kind(p))
    {
        Ok(File(_))   : "file",
        Ok(Dir)       : "directory",
        Ok(Other)     : "other",
        Err(NotFound) : "missing",
        Err(_)        : "error",
    }
}

fn main() : u8
{
    if (Err(_) = make_dir("work"))
    {
        return 1;
    }
    _ = write_file("work/notes.txt", "remember\\n");
    _ = rename("work/notes.txt", "work/old.txt");
    match (list_dir("work"))
    {
        Ok(names) :
        {
            foreach (n in &names)
            {
                printf("%s\\n", n);
            }
        },
        Err(_)    : return 1,
    }
    printf("notes.txt: %s, old.txt: %s\\n", kind("work/notes.txt"), kind("work/old.txt"));
    _ = remove_file("work/old.txt");
    _ = remove_dir("work");
    printf("work: %s\\n", kind("work"));
    u64 t0 = monotonic_ns();
    printf("time goes forward: %v\\n", monotonic_ns() >= t0);
    0
}
""", title="Making, listing and removing", expect="ok",
     output="old.txt\nnotes.txt: missing, old.txt: file\nwork: missing\ntime goes forward: true\n",
     section="s21", slug="directories")

b += h3("Sorting and searching")
b += p("""<em>In `std::collections`.</em>""")
b += p("""`Vec::sort(&mut v)` puts a `Vec` in order: integers by value, `false` before `true`, text by
its bytes, and a struct or enum of these field by field in declaration order (an enum by the order
its variants are declared in, then by payload). For any other order, or any other element type, `Vec::sort_by(&mut v, less)` takes a
function that says whether its first argument goes before its second. Both are stable: elements
that are equal keep the order they had. In a sorted `Vec`, `Vec::binary_search(&v, &key)` gives
`Ok(i)`, the first position holding the key, or `Err(i)`, where it would go
(`binary_search_by` takes the same `less`).""")
b += code(CHECK_STD + """
struct Player
{
    String name;
    u32 score;
}

fn higher(ref<Player, shared> a, ref<Player, shared> b) : bool
{
    a.score > b.score                 // higher scores first
}

fn main()
{
    Vec<String> names = Vec::new();
    Vec::push(&mut names, String::from_str("mira"));
    Vec::push(&mut names, String::from_str("ada"));
    Vec::push(&mut names, String::from_str("lin"));
    Vec::sort(&mut names);
    assert(&names[0][0..$] == "ada" && &names[2][0..$] == "mira");
    String who = String::from_str("lin");
    assert(Result::unwrap_or(Vec::binary_search(&names, &who), 99) == 1);
    String bob = String::from_str("bob");
    assert(match (Vec::binary_search(&names, &bob))
    {
        Ok(_)  : false,
        Err(i) : i == 1,              // between "ada" and "lin"
    });

    Vec<Player> ps = Vec::new();
    Vec::push(&mut ps, Player { .name = String::from_str("a"), .score = 3 });
    Vec::push(&mut ps, Player { .name = String::from_str("b"), .score = 9 });
    Vec::push(&mut ps, Player { .name = String::from_str("c"), .score = 3 });
    Vec::sort_by(&mut ps, higher);
    assert(ps[0].score == 9);
    assert(&ps[1].name[0..$] == "a" && &ps[2].name[0..$] == "c");   // equal scores keep their order
}
""", title="Sorting and searching", expect="ok", section="s21", slug="sorting")

b += h3("`HashMap<K, V>` and `HashSet<K>`")
b += p("""<em>In `std::collections`.</em>""")
b += p("""`HashMap<K, V>` maps keys to values; `HashSet<K>` holds a set of keys. A key is an integer,
a `bool`, a `str`, a `String`, or a struct whose fields (an enum whose payloads) are all keys:
`struct Pos { i32 x; i32 y; }` keys a grid. Any other key type (a float, an array, a struct with a
float field) is a static type error, and the message names the field that is not a key. The map
owns its keys and values. Lookups
take the key by reference, so they copy nothing.""")
b += table(["Function", "Does"], [
    ["`HashMap::new()`, `HashMap::len(&m)`", "an empty map; the number of entries"],
    ["`HashMap::insert(&mut m, k, v)`", "adds `k`; if it was present, replaces its value and returns the old one as `Some`"],
    ["`HashMap::entry(&mut m, k, fresh)`", "the value for `k` as an exclusive reference, adding `k` with `fresh` first if absent"],
    ["`HashMap::get(&m, &k)`, `HashMap::get_mut(&mut m, &k)`", "`Some` reference to the value, or `None`"],
    ["`HashMap::contains(&m, &k)`", "whether `k` is present"],
    ["`HashMap::remove(&mut m, &k)`", "removes `k` and returns its value; every other entry keeps its place; takes time in proportion to the map's size"],
    ["`HashMap::swap_remove(&mut m, &k)`", "the same, fast: the last entry moves into the removed one's place (D-0134)"],
    ["`HashMap::key_at(&m, i)`, `HashMap::value_at(&m, i)`", "entry `i`, for `i < len`, in insertion order"],
    ["`HashMap::clear(&mut m)`", "removes every entry, destroying keys and values; the map stays usable"],
    ["`HashMap::clone(&m)`, `HashSet::clone(&s)`, `Box::clone(&b)`, `clone(&m)`", "a copy of a map, set or box whose contents are `clone` (§12); `Rc::clone` shares instead"],
    ["`sleep_ms(ms)`", "the calling thread does nothing for at least `ms` milliseconds; other threads run meanwhile (D-0124)"],
    ["`Rc::downgrade(&r)`, `Weak::upgrade(&w)`, `Weak::clone(&w)`", "a `Weak<T>` keeps the box, not the value: `upgrade` is `Some(Rc)` while a strong handle exists, else `None` — for a parent pointer, a cache, an observer list, where an `Rc` would make a cycle (D-0125)"],
    ["`Rng::new(s)`, `Rng::next_u64(&mut r)`, `Rng::below(&mut r, n)`, `Rng::unit_f64(&mut r)`", "a general-purpose pseudo-random generator (xoshiro256**, fixed by the specification): the same seed gives the same numbers everywhere; `below` is unbiased in `0 .. n`. It is not suitable for cryptographic purposes — keys, tokens, nonces, anything an adversary must not predict (D-0120)"],
    ["`HashMap::get_str(&m, \"k\")`, `get_mut_str`, `contains_str`, `remove_str`", "for a map keyed by `String`: the same lookups with a `str`, so no `String` is built to ask (D-0113); `HashSet::contains_str`, `remove_str` likewise"],
    ["`HashSet::new`, `len`, `insert`, `contains`, `remove`, `swap_remove`, `key_at`, `clear`", "the same for a set; `insert` and the removals say whether anything changed"],
    ["`HashSet::union(&a, &b)`, `HashSet::intersection(&a, &b)`, `HashSet::difference(&a, &b)`", "a new set of the keys in either, in both, or in `a` but not `b`, copied with `clone`; `a` and `b` are unchanged. The result keeps `a`'s order, and `union` adds `b`'s new keys after it (D-0138)"],
])
b += p("""Entries are kept in the order they were added, so `foreach (k, v in &m)` (§14) visits
them in that order, the same in the interpreter and the compiler, whatever the keys hash to.
`key_at` and `value_at` reach entry `i` directly. `remove` keeps that order, so it moves every
later entry down one; `swap_remove` is quick because it fills the gap with the last entry instead,
for a map whose order does not matter.""")
b += code("""
import std;

fn main()
{
    array<str, 9> words = ["the", "cat", "and", "the", "hat", "and", "the", "bat", "sat"];
    HashMap<String, u32> counts = HashMap::new();
    foreach (w in words)
    {
        *HashMap::entry(&mut counts, String::from_str(w), 0) += 1;   // 0 if new, then + 1
    }
    foreach (word, n in &counts)                        // insertion order
    {
        printf("%-4s %d\\n", word, n);
    }

    String cat = String::from_str("cat");
    match (HashMap::get(&counts, &cat))
    {
        Some(n) : printf("cat: %d\\n", *n),
        None    : printf("no cat\\n"),
    }

    HashSet<u32> seen = HashSet::new();
    u32 x = 7;
    while (HashSet::insert(&mut seen, x))                 // false once x comes round again
    {
        x = x * x % 10;
    }
    printf("%d distinct, back to %d\\n", HashSet::len(&seen), x);
}
""", title="Counting words, and a set", expect="ok",
     output="the  3\ncat  1\nand  2\nhat  1\nbat  1\nsat  1\ncat: 1\n3 distinct, back to 1\n",
     section="s21", slug="hashmap")
b += code(CHECK_STD + """
struct Pos
{
    i32 x;
    i32 y;
}

enum Tile
{
    Wall,
    Door(u8),
}

fn main()
{
    HashMap<Pos, Tile> grid = HashMap::new();
    HashMap::insert(&mut grid, Pos { .x = 0, .y = -1 }, Wall);
    HashMap::insert(&mut grid, Pos { .x = 2, .y = 3 }, Door(7));
    Pos at = Pos { .x = 2, .y = 3 };
    match (HashMap::get(&grid, &at))
    {
        Some(Door(k)) : assert(*k == 7),
        _             : assert(false),
    }
    HashSet<Tile> seen = HashSet::new();      // an enum of keys is a key too
    HashSet::insert(&mut seen, Door(7));
    assert(HashSet::contains(&seen, &Door(7)) && !HashSet::contains(&seen, &Wall));
}
""", title="A struct as a key", expect="ok", section="s21", slug="hashmap-struct-key")
b += code("""
import std;

fn main()
{
    HashMap<f64, str> m = HashMap::new();   // ✗ diag.type-mismatch (static): f64 is not a key type
}
""", title="A float is not a key", expect="diag.type-mismatch", section="s21", slug="hashmap-float-key")
b += code("""
import std;

fn show(str name, ref<HashSet<str>, shared> s)
{
    printf("%-13s", name);
    foreach (i in 0..HashSet::len(s))
    {
        printf(" %s", *HashSet::key_at(s, i));
    }
    printf("\\n");
}

fn main()
{
    HashSet<str> monday = HashSet::new();
    HashSet<str> tuesday = HashSet::new();
    foreach (n in ["Ann", "Bo", "Cy", "Di"])
    {
        HashSet::insert(&mut monday, n);
    }
    foreach (n in ["Cy", "Ed", "Ann"])
    {
        HashSet::insert(&mut tuesday, n);
    }
    show("either day:", &HashSet::union(&monday, &tuesday));
    show("both days:", &HashSet::intersection(&monday, &tuesday));
    show("monday only:", &HashSet::difference(&monday, &tuesday));
}
""", title="Set operations", expect="ok",
     output="either day:   Ann Bo Cy Di Ed\nboth days:    Ann Cy\nmonday only:  Bo Di\n",
     section="s21", slug="hashset-set-ops")

b += h3("`Queue<T>` and `PriorityQueue<T>`")
b += p("""<em>In `std::collections`.</em>""")
b += p("""A `Queue<T>` hands elements back in the order they arrived: `push_back` adds at the back,
`pop_front` takes from the front, each in constant time. It works at the other ends too
(`push_front`, `pop_back`), and `front` and `back` look without taking. A `Vec` with a growing
`head` index does the same job, but its storage never shrinks, and `Vec::remove(&mut v, 0)` moves
every other element on each call.""")
b += p("""A `PriorityQueue<T>` hands back the least element first, whatever order they arrived in.
`PriorityQueue::new()` orders by the elements' own order, the one `Vec::sort` uses (numbers,
`bool`, text, and structs of them field by field), so a struct whose first field is the
priority needs no comparison function. `PriorityQueue::new_by(less)` takes the order as a
function, as `Vec::sort_by` does; for the largest first, pass one that answers "greater".
Elements the order calls equal come out in the order they went in, so equally urgent work is
first come, first served.""")
b += table(["Function", "Does"], [
    ["`Queue::new()`, `Queue::len(&q)`", "an empty queue; the number of elements"],
    ["`Queue::push_back(&mut q, x)`, `Queue::push_front(&mut q, x)`", "adds `x` at the back, or before the front"],
    ["`Queue::pop_front(&mut q)`, `Queue::pop_back(&mut q)`", "takes the front (or back) element as `Some`, or `None` when empty"],
    ["`Queue::front(&q)`, `Queue::back(&q)`", "`Some` reference to the front (or back) element, or `None`; the queue stays as it is"],
    ["`Queue::clear(&mut q)`", "destroys every element, front to back; the queue stays usable"],
    ["`PriorityQueue::new()`, `PriorityQueue::new_by(less)`", "an empty queue in the elements' own order, or in `less`'s"],
    ["`PriorityQueue::push(&mut q, x)`, `PriorityQueue::len(&q)`", "adds `x`; the number of elements"],
    ["`PriorityQueue::pop(&mut q)`", "takes the least element as `Some` (of equals, the earliest pushed), or `None` when empty"],
    ["`PriorityQueue::peek(&q)`, `PriorityQueue::clear(&mut q)`", "`Some` reference to the element `pop` would take; destroys every element"],
])
b += p("""Both own their elements and destroy what is left when they end. A reference from `front`,
`back` or `peek` borrows the queue, so the queue cannot change while it is in use. Neither has
`foreach` or `q[i]`: a queue is read at its ends.""")
b += code("""
import std;

fn link(ref<Vec<Vec<usize>>, exclusive> adj, usize a, usize b)
{
    Vec::push(&mut (*adj)[a], b);
    Vec::push(&mut (*adj)[b], a);
}

fn main()
{
    Vec<Vec<usize>> adj = Vec::new();
    foreach (_ in 0..7)
    {
        Vec::push(&mut adj, Vec::new());
    }
    link(&mut adj, 0, 1);
    link(&mut adj, 0, 2);
    link(&mut adj, 1, 3);
    link(&mut adj, 2, 3);
    link(&mut adj, 3, 4);
    link(&mut adj, 4, 5);

    // Breadth first: everyone one step from 0, then two steps, ...
    Vec<u32> steps = Vec::filled(7, max_value<u32>());
    Queue<usize> todo = Queue::new();
    steps[0] = 0;
    Queue::push_back(&mut todo, 0);
    while (Some(u) = Queue::pop_front(&mut todo))
    {
        foreach (v in &adj[u])
        {
            if (steps[*v] == max_value<u32>())
            {
                steps[*v] = steps[u] + 1;
                Queue::push_back(&mut todo, *v);
            }
        }
    }
    foreach (i, s in &steps)
    {
        if (*s == max_value<u32>())
        {
            printf("%d: unreachable\\n", i);
        }
        else
        {
            printf("%d: %d steps\\n", i, *s);
        }
    }
}
""", title="Breadth-first search with a Queue", expect="ok",
     output="0: 0 steps\n1: 1 steps\n2: 1 steps\n3: 2 steps\n4: 3 steps\n5: 4 steps\n6: unreachable\n",
     section="s21", slug="queue-bfs")
b += code("""
import std;

struct Road
{
    usize to;
    u32 km;
}

struct Visit                                   // ordered by dist, then node: a key
{
    u32 dist;
    usize node;
}

fn road(ref<Vec<Vec<Road>>, exclusive> map, usize a, usize b, u32 km)
{
    Vec::push(&mut (*map)[a], Road { .to = b, .km = km });
    Vec::push(&mut (*map)[b], Road { .to = a, .km = km });
}

fn main()
{
    Vec<Vec<Road>> map = Vec::new();
    foreach (_ in 0..5)
    {
        Vec::push(&mut map, Vec::new());
    }
    road(&mut map, 0, 1, 7);
    road(&mut map, 0, 2, 2);
    road(&mut map, 2, 1, 3);
    road(&mut map, 1, 3, 4);
    road(&mut map, 2, 3, 9);
    road(&mut map, 3, 4, 1);

    // Dijkstra: always settle the nearest place not yet settled.
    Vec<u32> best = Vec::filled(5, max_value<u32>());
    PriorityQueue<Visit> frontier = PriorityQueue::new();     // least first
    PriorityQueue::push(&mut frontier, Visit { .dist = 0, .node = 0 });
    while (Some(at) = PriorityQueue::pop(&mut frontier))
    {
        if (at.dist >= best[at.node])
        {
            continue;                                         // reached sooner already
        }
        best[at.node] = at.dist;
        foreach (r in &map[at.node])
        {
            PriorityQueue::push(&mut frontier, Visit { .dist = at.dist + r.km, .node = r.to });
        }
    }
    foreach (i, d in &best)
    {
        printf("to %d: %d km\\n", i, *d);
    }
}
""", title="Shortest routes with a PriorityQueue", expect="ok",
     output="to 0: 0 km\nto 1: 5 km\nto 2: 2 km\nto 3: 9 km\nto 4: 10 km\n",
     section="s21", slug="priority-queue-dijkstra")
b += code("""
import std;

struct Task
{
    u8 urgency;
    str what;
}

fn more_urgent(ref<Task, shared> a, ref<Task, shared> b) : bool
{
    a.urgency > b.urgency
}

fn main()
{
    PriorityQueue<Task> inbox = PriorityQueue::new_by(more_urgent);   // largest first
    PriorityQueue::push(&mut inbox, Task { .urgency = 1, .what = "tidy desk" });
    PriorityQueue::push(&mut inbox, Task { .urgency = 5, .what = "server down" });
    PriorityQueue::push(&mut inbox, Task { .urgency = 3, .what = "review" });
    PriorityQueue::push(&mut inbox, Task { .urgency = 5, .what = "disk full" });
    while (Some(t) = PriorityQueue::pop(&mut inbox))
    {
        printf("%d %s\\n", t.urgency, t.what);                  // equal urgency: first come, first served
    }
}
""", title="Largest first, and ties in arrival order", expect="ok",
     output="5 server down\n5 disk full\n3 review\n1 tidy desk\n",
     section="s21", slug="priority-queue-by")

b += h3("`Rc<T>`")
b += p("""<em>In `std::memory`.</em>""")
b += p("""Shared ownership as a library: a reference-counted box. `Rc::clone` makes another handle
to the same box; the value is destroyed when the last handle is dropped. Access is read-only (a
shared reference); put a `mutex` inside if you need mutation. `Rc` is not thread-safe.""")
b += table(["Function", "Signature"], [
    ["`Rc::new`", "`(T) : Rc<T>`"],
    ["`Rc::clone`", "`(ref<Rc<T>, shared>) : Rc<T>`"],
    ["`Rc::get`", "`(ref<Rc<T>, shared>) : ref<T, shared>`"],
])
b += code(CHECK_STD + """
fn main()
{
    Vec<i32> payload = Vec::new();
    Vec::push(&mut payload, 42);
    auto a = Rc::new(payload);        // payload moves into the box; count = 1
    auto b = Rc::clone(&a);           // count = 2
    assert(Vec::len(Rc::get(&b)) == 1);
    drop(a);                          // count = 1; the box survives
    assert(Rc::get(&b)[0] == 42);
    drop(b);                          // count = 0: the Vec is destroyed, then the box is freed
}
""", title="Reference counting", expect="ok", section="s21", slug="rc")

b += h3("`Box<T>`")
b += p("""<em>In `std::memory`.</em>""")
b += p("""A value on the heap with exactly one owner: `Rc` without the count. `Box::new(v)` moves `v`
in; `Box::get(&b)` and `Box::get_mut(&mut b)` borrow it; `Box::into_inner(b)` moves it back out;
destroying the `Box` destroys the value. Its main use is a type that contains itself, which needs
something holding a pointer between it and itself: a struct that contains itself directly has no
size, and is rejected with `diag.recursive-type`.""")
b += code(CHECK_STD + """
struct Node
{
    i32 value;
    Option<Box<Node>> next;           // Option<Node> here would be diag.recursive-type
}

fn main()
{
    Option<Box<Node>> list = None;
    list = Some(Box::new(Node { .value = 1, .next = list }));   // each node owns the next
    list = Some(Box::new(Node { .value = 2, .next = list }));

    Box<i32> counter = Box::new(41);
    *Box::get_mut(&mut counter) += 1;
    assert(*Box::get(&counter) == 42);
    i32 n = Box::into_inner(counter);  // the value back; the Box's memory is freed
    assert(n == 42);
}                                     // list: both nodes destroyed, the outer one first
""", title="A list that owns its nodes", expect="ok", section="s21", slug="box")
b += p("""Destroying a chain of `Box`es does not use a stack frame per node when each `Box` is the
last thing its node destroys (its first-declared resource field, as `next` above): the next node is
destroyed by a loop, in the same order. So a list of a million nodes goes out of scope like any
other value.""")
b += code("""
import std;

struct Node
{
    i32 value;
    Option<Node> next;                // ✗ diag.recursive-type (static): Node would contain itself
}

fn main()
{
}
""", title="A type cannot contain itself", expect="diag.recursive-type", section="s21", slug="box-recursive")

b += h3("`swap` and `replace`")
b += p("""<em>In `std::core`.</em>""")
b += p("""A value cannot be moved out of something you hold only by reference, since that would leave
a hole. So two values behind references are exchanged by `swap(&mut a, &mut b)`, and one is
exchanged for a new value by `replace(&mut r, v)`, which returns the old one. Two elements of the
same `Vec` or slice are exchanged by `Vec::swap(&mut v, i, j)` or `slice_swap(s, i, j)`, which also
handle `i == j`. Nothing is copied or destroyed: each value just changes place.
`overwrite(&mut r, v)` is `replace` for when the old value is not wanted: it stores `v` and
destroys what was there.""")
b += code(CHECK_STD + """
// Sorts any run of Strings by length: the elements are resources, so
// they are exchanged, never copied out.
fn sort_by_len(slice<String, exclusive> s)
{
    for (usize i = 1; i < slice_len(s); i += 1)
    {
        usize j = i;
        while (j > 0 && String::len(&s[j - 1]) > String::len(&s[j]))
        {
            slice_swap(s, j - 1, j);
            j -= 1;
        }
    }
}

struct Pair
{
    String left;
    String right;
}

fn main()
{
    Vec<String> words = Vec::new();
    foreach (w in ["pear", "fig", "banana", "kiwi"])
    {
        Vec::push(&mut words, String::from_str(w));
    }
    sort_by_len(&mut words[0..$]);
    assert(String::len(&words[0]) == 3 && String::len(&words[$ - 1]) == 6);

    Pair p = Pair { .left = String::from_str("L"), .right = String::from_str("RR") };
    swap(&mut p.left, &mut p.right);
    String old = replace(&mut p.left, String::from_str("new"));   // old is "RR"
    assert(String::len(&old) == 2 && String::len(&p.left) == 3);
}
""", title="Sorting Strings, and exchanging fields", expect="ok", section="s21", slug="swap")

b += p("""An empty slot is filled by plain assignment: a `None` owns nothing, so `*slot = Some(...)`
writes over it (§07). To put a value where one may already be, use `replace`, which hands the old
one back to you.""")
b += code(CHECK_STD + """
struct Node
{
    i64 val;
    Option<Box<Node>> next;
}

// Appends at the tail: walk to the empty link, then fill it.
fn push_back(ref<Option<Box<Node>>, exclusive> head, i64 v)
{
    ref<Option<Box<Node>>, exclusive> slot = head;
    while (true)
    {
        match (&mut *slot)
        {
            Some(b) : { slot = &mut Box::get_mut(b).next; },
            None    : { break; },
        }
    }
    *slot = Some(Box::new(Node { .val = v, .next = None }));   // slot holds None: nothing to lose
}

fn main()
{
    Option<Box<Node>> list = None;
    push_back(&mut list, 1);
    push_back(&mut list, 2);
    i64 sum = 0;
    ref<Option<Box<Node>>, shared> cur = &list;
    while (true)
    {
        match (cur)
        {
            Some(b) : { sum += Box::get(b).val; cur = &Box::get(b).next; },
            None    : { break; },
        }
    }
    assert(sum == 3);
}
""", title="Filling an empty link", expect="ok", section="s21", slug="replace-fill")

b += rules([
    "`Vec<T>` for sequences, `Option<T>` for maybe-absent values, `Result<T, E>` for fallible calls, `Box<T>` for one value on the heap (a recursive type's link), `Rc<T>` when two owners are genuinely needed.",
    "Library functions take `&v` or `&mut v`; match the mode to what you need.",
    "Hold element references briefly; any `push` or `pop` may invalidate them (§10).",
])
S21 = Section("s21", "§21", "The standard library, std", "21-standard-library-semantics.md",
              "import std; Option, Result, the intrinsics, str and printf, and Vec, String, HashMap, HashSet, Queue, PriorityQueue, Rc and Box written in CobaltC.", b)

# ================================================================ §22

b = ""
b += p("""This section is the syntax in one place: the tokens, the declarations, the statements,
the expressions, and the handful of places where the grammar needs a tie-break rule. It is meant
for looking things up; the semantics are in the earlier sections.""")

b += h3("Lexical structure")
b += ul([
    "<strong>Comments:</strong> `// to end of line` and `/* block */` (block comments do not nest).",
    "<strong>Identifiers:</strong> ASCII letters, digits and `_`, not starting with a digit.",
    "<strong>Integer literals:</strong> decimal digits, or hexadecimal, octal or binary after `0x`, `0o`, `0b`, optionally `: type` (`255: u8`, `0xFF: u8`). No sign: `-` is an operator. <strong>Float literals:</strong> `1.5`, `0.25: f32`, `1.0e-7`. <strong>Digit separators:</strong> a `_` between two digits is ignored (`1_000_000`, `0xFFFF_0000`). <strong>Booleans:</strong> `true`, `false`. <strong>Unit value:</strong> `()`.",
    "<strong>String literals:</strong> `\"…\"`, type `str`. Escapes `\\n \\r \\t \\0 \\\\ \\\"` and `\\u{hex}`; any other source character stands for itself. A literal must close on its own line. <strong>Byte literals:</strong> `b\"…\"`, type `array<u8, N>`; the same escapes except `\\xNN` in place of `\\u{…}`, ASCII characters only, at least one byte. <strong>Byte character literals:</strong> `b'a'`, one ASCII character or escape: an unsigned integer literal, `u8` unless its context gives another unsigned type. <strong>No character literals</strong> (a `char` type).",
    "<strong>Keywords:</strong> `fn struct bitstruct enum resource match if else while for foreach return auto const module import export unsafe extern break continue move mut true false as void` (`as` is reserved and unused).",
    "<strong>Reserved type names</strong> (cannot be identifiers): `i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 isize usize f32 f64 bool str ref rawptr array slice handle mutex guard`. The words `shared` and `exclusive` are ordinary identifiers that only have meaning inside `ref<…, …>`.",
    "Whitespace and newlines are insignificant between tokens; brace placement is a matter of style.",
])

b += h3("Types")
b += table(["Type syntax", "Meaning"], [
    ["`i32`, `u8`, `f64`, `bool`, …", "built-in scalars"],
    ["`str`", "text value (§21)"],
    ["`void`", "the unit type (its value is written `()`)"],
    ["`ref<T, shared>`, `ref<T, exclusive>`", "references"],
    ["`rawptr<T>`", "raw pointer"],
    ["`array<T, N>`", "fixed array, `N` a constant expression (§16)"],
    ["`slice<T, shared>`, `slice<T, exclusive>`", "a borrowed run of elements (§16)"],
    ["`fn(T1, T2) : R`, `fn(T1)`", "function type (result `void` if omitted)"],
    ["`handle<T>`, `mutex<T>`, `guard<T>`", "concurrency types"],
    ["`Name`, `Name<T1, T2>`, `mod::Name`", "a declared struct or enum, optionally instantiated"],
])

b += h3("Declarations")
b += code("""
// Functions. `export` makes an item visible outside its module.
fn name(T1 p1, T2 p2) : R                   // return type after a colon
{
    body
}

fn name(T1 p1)                              // no return type: returns void
{
    body
}

export fn name<T, U>(T x) : U               // generic, exported
{
    body
}

fn Type::name(ref<Type, shared> self)       // associated function, called as Type::name(...)
{
    body
}

fn Type::drop(ref<Type, exclusive> self)    // the destructor of Type
{
    body
}

// Foreign functions: parameters and result must be scalars, void or rawptr.
extern fn name(rawptr<u8> buf, usize len) : isize;

// Structs and enums. `resource` marks a type that manages something.
struct Name<T>
{
    T field;                                // fields end with ;
    export i32 visible;                     // field visibility is per field
}

resource struct Name
{
    i32 field;
}

enum Name<T>
{
    Variant(T),                             // with a payload
    Other                                   // without
}

// Modules and imports.
module name
{
    items
}

export module name
{
    items
}

module name "./path/to/file.cb";            // the same, with that file's items as the body (§17);
                                            // `export module name "…";` as for the inline form

import path::to::item;
""", title="Declaration forms")

b += h3("Statements and blocks")
b += code("""
T x = e;                 // typed local
auto x = e;              // inferred local
T x;                     // uninitialised local (must be assigned before use)
Name { f1, f2 } = e;     // destructuring: every field
e;                       // expression statement

{                        // a block; no ; needed after it
    statements
}

if (c)
{
    statements
}
else if (c2)
{
    statements
}
else
{
    statements
}

while (c)
{
    statements
}

match (e)                // a comma after every arm
{
    A(x) : e1,
    B : e2,
    _ : e3,
}

unsafe
{
    statements
}
""", title="Statement forms")
b += p("""A block is `{ statement* expr? }`: zero or more statements, then an optional trailing
expression that is the block's value. `if`, `while`, `match`, `unsafe` and plain blocks may stand
as statements without a semicolon, and their value is discarded. When one appears as the initialiser
of a declaration or the right side of an assignment, the enclosing statement still ends with `;`.""")

b += h3("Expressions")
b += table(["Form", "Example"], [
    ["literals and names", "`42`, `3.5`, `true`, `()`, `x`, `mod::item`"],
    ["string and byte literals", "`\"hello\\n\"` (a `str`), `b\"\\x00\\xff\"` (an `array<u8, 2>`)"],
    ["struct literal", "`Point { .x = 1, .y = 2 }`, `Box<i32> { .value = 5 }`"],
    ["array literal", "`[1, 2, 3]`, `[0; 256]` (256 copies of `0`)"],
    ["enum variant", "`Some(5)`, `None`, `Shape::Dot`"],
    ["field, index, call", "`p.x`, `a[i]`, `f(x, y)`, `Vec::len(&v)`, `identity<i64>(5)`"],
    ["borrow, deref", "`&x`, `&mut x`, `*r`"],
    ["operators", "see the precedence table in §13"],
    ["propagate", "`e?`"],
    ["block, if, match, unsafe", "as values: `i32 x = if (c) { 1 } else { 2 };`"],
    ["closure", "`[a, b](i32 x) { a + b + x }`, `move [v]() { Vec::len(&v) }`"],
    ["control", "`return e`, `return`, `break`, `continue`"],
])

b += h3("Reading the grammar in tricky spots")
b += ul([
    "<strong>Declaration or expression?</strong> A statement that begins with a type name, `fn`, `auto`, or the name of a struct or enum is a declaration; otherwise it is an expression. `Point p = …` declares; `p.x = 1;` assigns.",
    "<strong>`[` starts a closure or an array.</strong> It is a closure only if the closing `]` is immediately followed by `( … ) {`. Otherwise it is an array literal.",
    "<strong>Generic call or comparison?</strong> `name<T>(…)` with a type inside the angle brackets is always a generic instantiation. Comparisons are non-associative, so `a < b > c` is never a comparison.",
    "<strong>Conditions are always parenthesised</strong>, which is why `if (Point { .x = 1 } == p) { … }` needs no special rule.",
    "<strong>A pattern as a condition.</strong> `if (Some(x) = e)` and `while (Some(x) = e)` are read as a pattern followed by one `=` when the condition begins `Name(`, `m::Name` or a literal; `Some(x) == e` is a comparison, and `x = e` (a bare name) the assignment it is everywhere, which is not a `bool`.",
    "<strong>`Name { f1, f2 } = e;`</strong> (bare identifiers in braces) is destructuring; `Name { .f = e }` is a literal.",
    "<strong>`bitstruct Name : u32 { f : 6; … }`</strong> declares a struct of bit fields (§16): the widths fill the backing type exactly, low bit first.",
    "<strong>`&mut`</strong> is two tokens, one operator. `x = y = z` assigns right to left.",
])

b += h3("Restrictions and deliberate absences")
b += p("""Things the grammar does not provide, each a considered decision recorded in the
specification:""")
b += ul([
    "no character literals (`b'x'` is a byte: a `u8`, or the unsigned type its context asks for), no indexing or slicing of a `str`; no `switch` (a `match` on an integer, §16, is the checked form); no `goto`",
    "no `import … as` renaming; no method-call syntax (`v.push(x)` — final, `CHG-0132`); no operator overloading",
    "patterns in `match` are `_`, a name (a binder or a constant), a literal, a variant, and a variant with a pattern for its payload, nothing else (no ranges, alternatives, struct patterns or guards); destructuring takes a struct apart whole: every field named, or `..` for the rest (destroyed as usual), and `field : name` renames one",
    "no partial initialisation of aggregates",
    "no lifetime annotations: a function returning a reference must have exactly one reference parameter",
    "no user-declared bounds on generic type parameters: only the five built-in ones (`eq`, `ordered`, `number`, `integer`, `clone`), on a function's parameters; a struct or enum satisfies `clone` alone",
    "no `mut` qualifier on bindings (the keyword is reserved for `&mut`)",
    "no moving a resource out of a field of a live struct; move the whole struct, or take it apart (a field of a temporary may be moved out; the rest ends with the statement)",
    "no spawning a closure that borrows; use `move`",
])

b += h3("What an implementation must document")
b += p("""The width of `isize`/`usize`; byte order; enum discriminant width; struct padding; the
value of `dangling<T>()`; the encoding of function values; the address `str_ptr` gives for a
`str`'s bytes and whether equal values share one; and the form in which faults and diagnostics are
reported. Everything else is fixed by the language.""")
S22 = Section("s22", "§22", "Surface syntax reference", "22-surface-syntax.md",
              "Tokens, declarations, statements and expressions on one page, with the tie-break rules a reader needs.", b)

# ================================================================ Appendix

b = ""
b += p("""Every diagnostic has a stable name. <em>Static</em> means the program is rejected before
it runs; <em>dynamic</em> means a checked fault terminates the program; <em>both</em> means the same
rule is applied statically where the checker can see the problem and dynamically otherwise.""")
b += table(["Diagnostic", "Phase", "Meaning", "Usual fix"], [
    ["`diag.syntax-error`", "static", "the text is not a CobaltC program by the grammar", "correct the text there; the message names what was expected and what was found"],
    ["`diag.unbound-name`", "static", "a name resolves to nothing", "declare it, fix the spelling, or `import` it"],
    ["`diag.ambiguous-name`", "static", "two imports or two enums supply the same name", "qualify it"],
    ["`diag.name-not-visible`", "static", "the item is private to another module", "mark it `export` or move the use"],
    ["`diag.module-file-not-found`", "static", "`module m \"…\";` names no readable file", "fix the path (relative to the declaring file) or create the file"],
    ["`diag.module-cycle`", "static", "a file's module body loads itself again, directly or through other files", "move the shared items into a third file both name"],
    ["`diag.module-file-duplicate`", "static", "one file is named by two `module` declarations", "declare it once; reach it by qualified path or `import`"],
    ["`diag.const-not-constant`", "static", "a `const`'s initializer is not a constant expression, or a constant depends on itself", "use only constants in it (a local `const` helps), or compute the value in a function"],
    ["`diag.static-assert-failed`", "static", "a `static_assert` condition is false, for the program or for one instantiation", "fix what the assertion guards, or the assertion"],
    ["`diag.static-assert-not-constant`", "static", "a `static_assert` condition (or message argument) is not a `bool` constant expression", "check run-time values with `assert`; name compile-time values with `const`"],
    ["`diag.duplicate-item`", "static", "two declarations add the same qualified name", "rename one, or move it into a module"],
    ["`diag.duplicate-local`", "static", "a name declared twice in one block (a function's parameters and a loop's or pattern's names count as declared in the block they govern)", "assign to the existing variable, or choose another name; an inner block may shadow it"],
    ["`diag.foreign-associated-fn`", "static", "`fn T::name` in a module that does not declare `T` (for example `fn Vec::first_or` outside `std`)", "declare a plain function, or put it in `T`'s module"],
    ["`diag.no-main`", "static", "no `fn main()` at the root module (a `main` inside a module is just `m::main`)", "declare `fn main()` at the root, or run the file that has one"],
    ["`diag.div-overflow`", "both", "`min / -1` or `min % -1` for a signed type", "`checked_div`, or widen first"],
    ["`diag.transfer-without-authority`", "both", "a move from a binding that no longer holds its resource (already moved or destroyed)", "move from the current holder"],
    ["`diag.type-mismatch`", "static", "an operand or argument has the wrong type", "convert explicitly; fix the types"],
    ["`diag.unbounded-type-parameter`", "static", "arithmetic, comparison, field access or a call on a bare `T`", "use a concrete type"],
    ["`diag.cannot-infer-type-parameter`", "static", "a generic argument is fixed by nothing", "write `name<T>(…)` or declare the variable's type"],
    ["`diag.break-outside-loop`, `diag.return-outside-fn`", "static", "misplaced control statement", "move it"],
    ["`diag.capture-list-mismatch`", "static", "the closure's `[…]` does not equal its free variables", "add or remove names"],
    ["`diag.stale-binding`", "both", "use of a moved-from, dropped, or dead binding or reference", "use the current owner; re-fetch after a move or reallocation"],
    ["`diag.use-of-uninitialized`", "static", "a read on a path with no prior write", "initialise on every path; initialise aggregates whole"],
    ["`diag.reference-escapes-scope`", "static", "a reference stored or returned beyond its referent's block", "return an owned value"],
    ["`diag.lifetime-elision-ambiguous`", "static", "a reference returned from a function with 0 or 2+ reference parameters", "one reference parameter, or return owned"],
    ["`diag.aliasing-conflict`", "both", "a read or write while a conflicting reference is live", "end the earlier borrow first"],
    ["`diag.borrow-exceeds-source`", "static", "`&mut` taken through a shared reference", "take `ref<T, exclusive>`"],
    ["`diag.write-through-shared`", "static", "assignment through a shared reference", "as above"],
    ["`diag.destroy-while-aliased`", "both", "`drop` or scope exit while a reference or guard is live", "let the reference's holder end first"],
    ["`diag.move-while-aliased`", "both", "a move while a reference is live", "end the reference before moving"],
    ["`diag.move-out-of-field`", "static", "moving a resource out of a live struct's field or through a reference", "move the whole container, or match a reference (`match (&e)`)"],
    ["`diag.borrow-of-non-place`, `diag.borrow-of-temporary`", "static", "`&` applied to a value or temporary", "bind it first"],
    ["`diag.no-destroy-authority`, `diag.transfer-without-authority`", "both", "destroying or moving something this thread does not own", "destroy in the owning thread, once"],
    ["`diag.overwrite-of-live-resource`", "both", "assigning over a resource that is still owned", "`overwrite(&mut x, v)`, or `drop` first"],
    ["`diag.unwrap-failed`", "dynamic", "`unwrap` or `expect` of a `None` or an `Err`", "`match`, or `unwrap_or` with a default"],
    ["`diag.assert-failed`", "dynamic", "an `assert` condition is false (the formatted message, if any, is shown)", "the condition names what was assumed; find why it did not hold"],
    ["`diag.read-of-resource`", "static", "a resource used where a copy would be made (`==`, a value read)", "move it or take a reference"],
    ["`diag.bad-destructor-signature`, `diag.direct-destructor-call`", "static", "`drop` declared wrongly or called by hand", "`fn T::drop(ref<T, exclusive> self)`; use `drop(x)`"],
    ["`diag.destructor-on-plain-type`", "static", "`T::drop` declared for a type that is not a resource", "declare the type `resource struct T` (or `resource enum T`)"],
    ["`diag.arith-overflow`", "both", "`+ - *` or negation left the type's range", "widen, or use `wrapping_`/`saturating_`/`checked_`"],
    ["`diag.div-by-zero`, `diag.div-overflow`", "both", "division by zero; `MIN / -1`", "test first or `checked_div`"],
    ["`diag.shift-amount-out-of-range`", "both", "shift by the bit width or more", "mask the amount"],
    ["`diag.narrowing-overflow`", "both", "`narrow` or `to_int` of a value that does not fit, or a value too wide for a `bitstruct` field", "`checked_narrow` or `narrow_wrapping`, or widen the target"],
    ["`diag.result-discarded`", "static", "an expression statement whose value is a `Result`, so its error would vanish", "handle it (`?`, `match`, `unwrap`), or write `_ = e;` to ignore it on purpose"],
    ["`diag.literal-out-of-range`", "static", "a literal that does not fit its type", "fix the literal or its type"],
    ["`diag.recursive-type`", "static", "a struct or enum contains itself by value", "put the recursive part behind `Box<T>`, `Rc<T>` or `Vec<T>`"],
    ["`diag.index-out-of-bounds`", "both", "array or `Vec` index, or `str_byte` position, at or past the length", "check the index first"],
    ["`diag.not-ascii`", "dynamic", "`String::push_ascii` given a byte of 128 or more", "build the bytes in a `Vec<u8>` and check them with `String::from_utf8`"],
    ["`diag.not-char-boundary`", "dynamic", "a `StringView` bound inside a character's UTF-8 encoding", "move the bound to a character's start, e.g. with `StringView::find`"],
    ["`diag.non-exhaustive-match`", "static", "a variant with no arm and no `_`", "add arms"],
    ["`diag.unreachable-arm`", "static", "an arm whose every value an earlier arm already takes (arms are tried in order)", "remove it, or move it before the arm that shadows it"],
    ["`diag.propagate-outside-fallible-context`", "static", "`?` in a function whose return type is not `Result<_, E>` with the same `E`", "change the return type or `map_err`"],
    ["`diag.format-invalid`", "static", "a `printf`-family format that is not a string literal, or a malformed `%` specifier", "fix the specifier; write `%%` for a literal `%`"],
    ["`diag.spawn-borrow-closure`", "static", "`spawn` given a closure that borrows", "add `move`"],
    ["`diag.mutex-reentrant-lock`", "dynamic", "locking a mutex this thread already holds", "release the guard first"],
    ["`diag.channel-zero-capacity`", "dynamic", "`Channel::new` given a capacity of 0", "give the number of values it may hold; 1 for a hand-over"],
    ["`diag.channel-deadlock`", "dynamic", "the program's only running thread waits on a full or empty channel that is not closed", "send or receive from another thread, or `close` the channel first"],
    ["`diag.stack-exhausted`", "dynamic", "calls nested deeper than the stack the implementation provides", "recurse less deeply: a loop, or an explicit stack in a `Vec`"],
    ["`diag.trusted-outside-unsafe`", "static", "a raw or extern operation outside `unsafe`", "wrap it in `unsafe` after checking the precondition"],
    ["`diag.extern-non-ffi-type`", "static", "an `extern fn` with a non-FFI parameter or result", "pass scalars or `rawptr`"],
    ["`diag.alloc-failure`", "dynamic", "`Vec` or `Rc` could not allocate", "call `allocate` yourself if you need to recover"],
])
APPX = Section("appx", "A", "Appendix: diagnostics", None,
               "Every named diagnostic, what it means, and the usual fix.", b)
APPX.group = "Appendix"

SECTIONS = [S17, S18, S19, S20, S21, S22, APPX]
