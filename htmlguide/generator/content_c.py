from gen import Section, p, h3, h4, ul, ol, table, note, rules, code
from content_a import CHECK, CHECK_STD
from std_inventory import inventory_html

# ================================================================ §17

b = ""
b += p("""Modules group declarations and control which names are visible where. They introduce no
new semantics: ownership, borrowing and every other rule apply unchanged across module boundaries.""")

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
name. Nothing can intercept this within a thread. (A fault in a thread you spawned ends that thread
alone, and its failure becomes a value at the thread's boundary: §19, <a href="#s19-supervision">When a
thread fails</a>.) The point is that a fault cannot corrupt state or leak an external
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
error) is evaluated either way, and one that goes unused is destroyed. `Result::ok(r)` turns a
`Result` into an `Option`, dropping the error. Where there must be a value,
`Option::unwrap(o)` and `Result::unwrap(r)` give it or fault with `diag.unwrap-failed`, and
`Option::expect(o, "…")` and `Result::expect(r, "…")` do the same with your message.""")
b += code(CHECK_STD + """
fn main()
{
    Vec<String> names = Vec::new();
    Vec::push(&mut names, String::from_str("ada"));
    Vec<String> copy = Vec::clone(&names);                    // each String copied by its clone (§12)
    String first = Option::expect(Vec::pop(&mut copy), "the copy has one name");
    first = String::from_str("grace");                        // a String is quiet: the old one is destroyed
    u32 n = Result::unwrap(String::parse<u32>(&String::from_str("42")));
    assert(String::eq_str(&first, "grace") && n == 42 && Vec::len(&names) == 1);
    Option<u32> none = None;
    u32 fails = Option::expect(none, "no value where one was promised");
    // ✗ diag.unwrap-failed (dynamic), reporting "no value where one was promised"
}
""", title="unwrap, expect, replacing a String, and clone_by", expect="diag.unwrap-failed", section="s18", slug="unwrap")
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

b += p("""<strong>Without waiting for ever</strong> (D-0185). `Channel::try_recv(&ch)` answers at once:
`Ok(Some(v))` a value, `Ok(None)` nothing there, `Err(())` closed and empty — three answers, because a
loop that also watches a socket or `interrupt_requested()` must tell "nothing yet" from "the
senders are done". `Channel::recv_timeout_ms(&ch, ms)` waits at most `ms` milliseconds for the same
three answers. `cpu_count()` is how many threads the machine runs at once, the size to give a pool.""")

b += h3("How threads run", anchor="s19-how-threads-run")
b += p("""Every CobaltC thread is a thread of the operating system. In a program built by `cobc`
they run at once, on as many cores as the machine has. The checks of §08 and §10 still run as the
program goes, in every thread, and two threads that use the same object meet in them: that is how
a conflict between threads is caught at the access that would have raced.""")
b += p("""To keep threads from slowing each other down, the run-time checks keep each thread's records
apart. What a thread makes, and what is moved into it, are its own; most checks on them involve no
other thread. Where threads do meet:""")
b += ul([
    "<strong>Data reached through a reference to another thread's</strong> (`spawn(f, &v)`) stays the owner's, and each access through it is checked against the owner's records too. It works, and costs a little more on every access than data the thread owns.",
    "<strong>Waiting</strong> — on `join`, on a `lock` someone else holds, on a full or empty `Channel` — stops only the waiting thread, which spins briefly before it sleeps: most waits for a lock are short. Locking, sending and receiving involve no other thread's checks.",
    "<strong>A block that only reads and writes through its guard</strong> (`{ auto g = lock(&m); *g += 1; }`, plain data inside, no early `return`) costs a lock and an unlock and nothing more: `cobc` makes no guard object for it.",
    "<strong>Running code of the program on a value's end</strong> — a destructor you wrote, a `fn` value, a `Box` — briefly pauses the other threads' checks. Everything else the runtime does at a block's end stays within the threads involved.",
])
b += p("""So the program that runs fastest in parallel gives each thread its work: move a chunk of the
data into the thread (an argument it takes by value, which then belongs to it) and hand the result
back through `join` or a `Channel`. The performance chapter's <a href="#perf-threads">Threads</a>
section has the figures.""")
b += code("""
import std;

fn total(Vec<u64> chunk) : u64
{
    u64 t = 0;
    foreach (x in &chunk)
    {
        t += *x;
    }
    t
}

fn main()
{
    Vec<handle<u64>> hs = Vec::new();
    foreach (k in 0..4: u64)
    {
        Vec<u64> chunk = Vec::new();                 // made here, then moved into its thread
        foreach (i in 0..1000: u64)
        {
            Vec::push(&mut chunk, k * 1000 + i);
        }
        Vec::push(&mut hs, spawn(total, chunk));   // `chunk` now belongs to that thread
    }
    u64 sum = 0;
    while (Vec::len(&hs) > 0)
    {
        sum += join(Option::unwrap(Vec::pop(&mut hs)));
    }
    assert(sum == 7998000);                         // 0 + 1 + … + 3999
}
""", title="Each thread its own data", expect="ok", section="s19", slug="own-data")

b += h3("The guarantee", anchor="s19-guarantee")
b += p("""What a program without `unsafe` can rely on, however its threads are timed (`rule.conc.guarantee`):""")
b += ul([
    "<strong>No data races.</strong> Two threads never touch the same storage at once with one of them writing. Either something orders the two accesses (a `join`, a mutex's lock, a value sent and received), or the second access is `diag.aliasing-conflict`.",
    "<strong>One step at a time.</strong> Every run is some interleaving of the threads' steps, each step whole: a thread never sees half of another's write, and there is no reordering to reason about.",
    "<strong>Nothing undefined.</strong> An access that is not allowed is a diagnostic, in every interleaving.",
])
b += p("""A reference passed to `spawn` belongs to the new thread from the moment of the `spawn`, until
the thread ends, even before it has run a single step. Timing can still choose between two permitted
outcomes: a thread that has already finished holds nothing, so an access that would have conflicted
with it while it ran is fine afterwards. Use `join` (or a channel) to make the order part of the
program. Deadlock is not ruled out, but it is reported rather than left to hang (below).""")
b += code("""
import std;

fn bump(ref<i64, exclusive> x, ref<Channel<u8>, shared> go)
{
    _ = Channel::recv(go);                    // waits, holding x
    *x += 1;
}

fn main()
{
    Vec<i64> v = Vec::filled(4, 0: i64);
    Channel<u8> go = Channel::new(1);
    {
        auto h = spawn(bump, &mut v[0], &go);
        i64 other = v[1];                      // fine: another element
        i64 same = v[0];                       // ✗ diag.aliasing-conflict: the thread holds v[0]
        _ = Channel::send(&go, 1);
        join(h);
    }
}
""", title="Caught, whatever the timing", expect="diag.aliasing-conflict", section="s19", slug="no-race")

b += h3("When a thread fails", anchor="s19-supervision")
b += p("""A checked fault in a thread you spawned ends <em>that thread</em>: it unwinds, its destructors
run, the threads it started are stopped and waited for, and its result becomes the failure. The rest
of the program goes on. This is what lets a server give each connection a thread and survive a bug
in one of them. Nothing continues in a state the failed check protected:""")
b += ul([
    "<strong>A mutex the thread held is poisoned.</strong> Its interior may be half-changed, so the next `lock` of it faults with `diag.mutex-poisoned`.",
    "<strong>A thread lent an exclusive reference is not contained.</strong> Given one at `spawn`, or by locking a mutex whose contents can hold one (receiving from a `Channel` of `&mut` references, say), it may be changing data that belongs to a thread still running, so its fault ends the program at once, as a fault always did.",
    "<strong>The main thread's fault</strong> ends the program, as before.",
])
b += p("""Nobody has to ask for this. `join(h)` of a failed thread raises its failure again in the joiner,
and so does the handle's end if nobody joined it: a program that never looks at failures still fails
as a whole, one scope at a time. To take a failure as a value, use `try_join(h)`, which returns
`Result<R, ThreadFailure>`. `ThreadFailure::text` is the report the program would have printed,
`diagnostic` its `diag.…` name. `is_finished(&h)` says, without waiting, whether a thread has ended,
so one loop can accept new work and collect the threads that are done.""")
b += code("""
import std;

fn handler(usize i) : u64
{
    Vec<u64> v = Vec::filled(3, 7: u64);
    v[i]                                               // a bug when i >= 3
}

fn main()
{
    usize served = 0;
    usize failed = 0;
    foreach (i in 0..5)
    {
        match (try_join(spawn(handler, i)))
        {
            Ok(_)  : served += 1,
            Err(f) :
            {
                String d = ThreadFailure::diagnostic(&f);
                assert(String::as_view(&d) == "diag.index-out-of-bounds");
                failed += 1;
            },
        }
    }
    assert(served == 3 && failed == 2);                // the program went on after both
}
""", title="A supervisor takes failures as values", expect="ok", section="s19", slug="try-join")
b += code("""
import std;

fn handler(usize i) : u64
{
    Vec<u64> v = Vec::new();
    v[i]
}

fn main()
{
    auto h = spawn(handler, 0);
    join(h);                          // ✗ diag.index-out-of-bounds (dynamic): raised again here
}
""", title="join raises a failure nobody took", expect="diag.index-out-of-bounds", section="s19", slug="join-failed")
b += code("""
import std;

fn update(ref<mutex<i64>, shared> m, usize i)
{
    auto g = lock(m);
    *g += 1;
    Vec<i64> v = Vec::new();
    *g += v[i];                       // faults while holding the lock
}

fn main()
{
    mutex<i64> m = Mutex::new(0: i64);
    _ = try_join(spawn(update, &m, 0));
    auto g = lock(&m);                // ✗ diag.mutex-poisoned (dynamic): *g may be half-updated
}
""", title="A failed thread's mutex is poisoned", expect="diag.mutex-poisoned", section="s19", slug="poisoned")

b += h3("Stopping a thread: `cancel`")
b += p("""`cancel(&h)` asks a thread to stop. Its current wait, or its next one, ends with the fault
`diag.thread-cancelled`, contained like any other: the thread unwinds and its result is that
failure. The waits are `join`, `lock`, a channel's `send` and `recv`, `sleep_ms`, and every socket
operation, so a handler blocked reading a client that never writes can be stopped. A thread that
never waits finishes its work. `ThreadFailure::is_cancelled` tells a cancellation from a bug, and a
handle whose thread you cancelled raises nothing when it ends. Together with a channel this is
"first answer wins" without a `select`: start one thread per source, all sending to one channel,
take the first value, and cancel the rest.""")
b += code("""
import std;

fn listen(ref<Channel<u8>, shared> c) : u8
{
    Option::unwrap_or(Channel::recv(c), 0)           // nobody ever sends
}

fn main()
{
    Channel<u8> c = Channel::new(1);
    auto h = spawn(listen, &c);
    cancel(&h);                                      // its wait ends at once
    match (try_join(h))
    {
        Ok(_)  : assert(false),
        Err(f) : assert(ThreadFailure::is_cancelled(&f)),
    }
}
""", title="cancel ends a wait", expect="ok", section="s19", slug="cancel")

b += h3("Deadlock")
b += p("""A wait that can never end is `diag.deadlock`, reported where it happens. A thread whose
`join` or `lock` would close a cycle (it waits on a thread that, through holders and joins, waits on
it) faults at once. And when every thread is waiting on another thread, a mutex or a channel, with
none on the clock, a socket or input, the runtime faults one thread of a cycle, or, if there is no
cycle, the threads at the ends of the chains of waits (those waiting on a channel); the threads
waiting on them then get the failure through `join`, or find the mutex poisoned. The result does
not depend on timing. A channel wait while some thread is in a socket call or asleep is never
called a deadlock: that thread may send when its call returns. Give such a wait a limit
(`recv_timeout_ms`) or `cancel` it from a supervisor. Like any fault in a spawned thread it is contained, so two deadlocked handlers in
a server are reported and the server goes on. (The main thread waiting alone on a channel is still
`diag.channel-deadlock`.)""")
b += code("""
import std;

fn waits(ref<Channel<u8>, shared> c)
{
    _ = Channel::recv(c);            // nobody will send
}

fn main()
{
    Channel<u8> c = Channel::new(1);
    auto h = spawn(waits, &c);
    join(h);                          // ✗ diag.deadlock (dynamic): every thread waits, none can go on
}
""", title="Every thread waiting", expect="diag.deadlock", section="s19", slug="deadlock")

b += h3("Exploring schedules: `--explore`")
b += p("""A result that depends on timing is easy to miss: the timing you test is rarely the one that
goes wrong. `coby --explore N prog.cb` runs the program N times, each under a different order of its
threads chosen by a seed, and groups the runs by outcome (output and exit status). One outcome is
what you want; more than one means the program's result depends on timing, and `coby` exits with 1.
Each outcome comes with a seed, and `coby --schedule-seed S prog.cb` replays that run exactly, as
often as you need to find out why. Under a schedule the threads take turns one at a time, so
exploration needs no cores and finds orders a parallel run would rarely produce.""")
b += p("""`cobc --explore N prog.cb` does the same for the compiled program: it compiles once and runs the
executable N times. There a thread keeps its turn between the points where threads interact -- a
`spawn`, a wait, a mutex released, a call outside the program -- and now and then between two of the
program's checks, which is where a program whose checks all pass can end differently.
`cobc --schedule-seed S prog.cb` replays one run, and so does setting `COBALTC_SCHEDULE_SEED=S` in a
compiled program's environment. In either tool, a thread that is asleep or in a socket or input call
leaves the schedule until the call returns, so a program that talks to the network explores only
the orders around those calls.""")
b += code("""
import std;

fn bump(ref<mutex<i64>, shared> m)
{
    i64 seen = 0;
    {
        auto g = lock(m);
        seen = *g;
    }
    auto g = lock(m);
    *g = seen + 1;                    // a lost update when the other thread ran in between
}

fn main()
{
    mutex<i64> m = Mutex::new(0: i64);
    {
        auto a = spawn(bump, &m);
        auto b = spawn(bump, &m);
    }
    auto g = lock(&m);
    printf("total %v\n", *g);
}
""", title="counter.cb: a lost update, in some orders", section="s19", slug="explore-counter")
b += code("""
$ coby --explore 50 counter.cb
explored 50 schedules of counter.cb: 2 distinct outcomes

19 runs, exit 0 -- replay: coby --schedule-seed 1 counter.cb
  | total 1

31 runs, exit 0 -- replay: coby --schedule-seed 2 counter.cb
  | total 2
""", title="Two outcomes: the result depends on timing")

b += h3("Data parallelism: `slice_parts`")
b += p("""`slice_parts(&mut v[0..$], k)` splits a vector (or any exclusive slice) into `k` consecutive
runs as nearly equal as they can be, each an exclusive slice of its own. Give each run to a thread
and the threads write one vector at once, with no lock: the runs are disjoint, and the checker
proves it the way it proves any two ranges apart. The vector is usable again once every thread is
joined. A thread takes a named function here, not a closure: a closure is called through an
exclusive borrow of itself (§15), so one closure cannot run in several threads at once.""")
b += code("""
import std;

fn square_all(slice<u64, exclusive> s)
{
    for (usize i = 0; i < slice_len(s); i += 1)
    {
        s[i] = s[i] * s[i];
    }
}

fn main()
{
    Vec<u64> v = Vec::new();
    foreach (i in 0..1000: u64)
    {
        Vec::push(&mut v, i);
    }
    {
        Vec<slice<u64, exclusive>> parts = slice_parts(&mut v[0..$], cpu_count());
        Vec<handle<void>> hs = Vec::new();
        while (Vec::len(&parts) > 0)
        {
            Vec::push(&mut hs, spawn(square_all, Vec::swap_remove(&mut parts, 0)));
        }
        foreach (h in hs)
        {
            join(h);
        }
    }
    assert(v[999] == 998001);
}
""", title="Each thread one part of a vector", expect="ok", section="s19", slug="slice-parts")

b += h3("What is not provided")
b += p("""No atomics, no memory orderings, no condition variables, no thread-local storage. Steps of different threads interleave at
the granularity of single operations and every operation sees a consistent state; a `join`, a
`lock`, a `send` or a `recv` that cannot proceed simply waits. `Rc<T>` is not thread-safe; to share
across threads, share a `mutex` or pass values through a `Channel`.""")
b += note("""The compiler `cobc` runs threads in parallel on separate cores (above); the interpreter `coby`
runs them one at a time. A program that follows the rules gets the same result from both. Where the
language leaves an outcome to timing — writing a variable while another thread may still be reading
it through a reference is `diag.aliasing-conflict` if the thread is still running, and fine if it has
finished — each is free to show either permitted outcome, and they may differ.""", kind="note", title="Parallel or interleaved")

b += rules([
    "Every thread has an owner: the handle. Join it explicitly to get the result; let scope end join it if you do not care.",
    "Supervise threads that may fail: `try_join` takes the failure as a value; `cancel` the ones you no longer need.",
    "Lend a thread exclusive access to your data only when its failure should end the program; otherwise move the data in.",
    "Share a `mutex` by shared reference; keep each guard in the smallest block that needs it.",
    "To hand values from thread to thread, send them through a `Channel`; `close` it when the last value is sent.",
    "Move data into a thread with a `move` closure or by-value arguments; lend it with references only when the mutex rule allows.",
])
S19 = Section("s19", "§19", "Concurrency", "19-concurrency.md",
              "Threads owned by handles, a fault contained to its thread and handed over as a value, cancellation at any wait, deadlocks reported, mutexes reachable only through guards, channels, and the same aliasing rules everywhere.", b)

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
    ["an `unsafe extern fn` declaration", "the linked code has a function of this name with exactly this signature"],
    ["a call to an `unsafe extern fn`", "the foreign code honours its signature and touches only what the pointer arguments reach"],
])

b += h3("Who asserts what", anchor="s20-who-asserts")
b += p("""The standard library's own bodies use `unsafe`: `Vec::push` writes an element through a raw
pointer, `File::write` calls the operating system, `Rc::clone` reclaims its box. Those blocks are not
yours. They belong to the implementation, which must keep every one of them true to conform, and the
specification lists each of them with the fact that makes it hold (`spec/21` §0, "What `std` asserts":
for `Vec::push`, that `len < cap` after `grow` and that no element object was ever established in the
slot). So a program that writes no `unsafe` of its own has the language's guarantee unconditionally,
and the guarantee is lost only by a false claim in a block the program itself wrote.""")

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

unsafe extern fn area(i32 w, i32 h) : i32;
unsafe extern fn squares(rawptr<i64> out, i64 n) : i64;

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
b += p("""`unsafe extern fn name(params) : ret;` declares a function provided from outside. Parameters and
result may only be integers, floats, `bool`, `void` and `rawptr<T>`; anything else, including a
`ref` or a struct, is rejected (`diag.extern-non-ffi-type`). The declaration itself says `unsafe`:
no tool can check that the C function really has that name and exactly those
types, and if it does not, every call is wrong, so the promise is marked where it is made, where a
search for `unsafe` finds it. Without `unsafe` the declaration is a syntax error. Calling it needs
`unsafe` too, for what the function does. Its result
is a <em>claim</em>: a genuine value of its declared type, but one whose relationship to anything
else (that a returned length is right, that a pointer is valid) is not established until your code
checks it.""")
b += p("""`std` declares two externs: `stdout_write(rawptr<u8> buf, usize len) : isize`, which
`printf` (§21) writes through, with the `unsafe` block inside `std`, and its counterpart
`stdin_read`, which `read_line` reads through. Calling conventions and how code is linked are outside the language
and documented by each implementation; a program names the C code it needs with `extern "…";`
(below).""")
b += p("""Both implementations call real C functions on x86-64 Linux: an `unsafe extern fn` names a
function in the C library or the maths library by its declared name. The compiler passes any
FfiType, pointers included. The interpreter's memory is simulated, so it can pass only integers,
`bool` and floats, and reports a call that needs a pointer as unsupported rather than guessing.""")
b += code(CHECK_STD + """
unsafe extern fn sqrt(f64 x) : f64;          // from the maths library
unsafe extern fn labs(i64 x) : i64;          // from the C library
unsafe extern fn toupper(i32 c) : i32;

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

unsafe extern fn takes_vec(Vec<i32> v);      // ✗ diag.extern-non-ffi-type: a Vec cannot cross the boundary

fn main()
{
}
""", title="Only plain types cross an extern boundary", expect="diag.extern-non-ffi-type", section="s20", slug="extern-nonffi")

b += h3("Your own C code")
b += p("""`extern "…";` names C code for the compiler to link, so that the program's `unsafe extern fn`
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
as `std::collections::Vec`. A program that wants one subject alone may import just that one. Four
submodules divide again, the same way: `std::crypto` into `digest`, `kdf`, `aead` and `pk`,
`std::http` into `client` and `server`, `std::database` into one submodule per database server,
`postgres` today, and `std::encoding` into one submodule per data format or encoding of bytes as
text, `json`, `hex` and `base64`. Each is
re-exported by its parent, so `sha256` is also `std::crypto::digest::sha256`, `PgConnection` is
`std::database::postgres::PgConnection` and `Json` is `std::encoding::json::Json`. One submodule is declared and not
re-exported: `std::extensions`, the area an implementation may fill with modules of its own, which the
specification leaves undefined and conformance never tests. A program reaches one only by naming it
(`import std::extensions::m;`), so its dependence on that implementation shows in its source; the
reference implementation's `std::extensions` is empty.""")
b += h3("Every module and its items")
b += p("""Below is every module of `std` with every item it declares, generated from the
reference implementation's source. Names <code class="priv">in faint red</code> are not exported: they are the
module's own working parts, which a program cannot name or call, listed so that nothing in the source
is left out. Everything else is exported. An item marked <sup class="nat">n</sup> has no CobaltC body: both
tools build it in (`printf`, `sqrt`, `exit`, …). A row headed `T::` lists `T`'s
associated functions. A parent module also re-exports the submodules it lists, so their items are its
own too.""")
b += inventory_html()
b += code("""
import std::text;
import std::io;

fn main()
{
    String s = "only text and output";
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
    ["`Result::ok(r)`", "the `Result` as an `Option`: `Some` of the value, `None` for an error (which is destroyed)", "§18"],
    ["`Option::is_some(&o)`, `is_none`, `Result::is_ok(&r)`, `is_err`", "which variant, without taking the value", "§18"],
    ["`Result::map_err(r, f)`", "convert a `Result`'s error type with `f`", "§18"],
    ["`FileError::text(&e)`, `ParseError::text`, `Utf8Error::text`", "a `std` error as a sentence for a person, a `str`", "§21"],
])
b += h4("Output, input and the program")
b += table(["Function", "Purpose", "See"], [
    ["`printf(\"%v\", x)`", "write a `str`, a number, a `bool`, a `String` (through `&`), a `StringView`, or an enum's variant name to standard output", "§21"],
    ["`printf(fmt, args…)`, `eprintf(fmt, args…)`, `sprintf(fmt, args…)`, `String::appendf(&mut s, fmt, args…)`", "C-style formatted output, to standard output, to standard error, as a new `String` or into one, checked when compiled", "§21"],
    ["`assert(c)`, `assert(c, fmt, args…)`", "fault with `diag.assert-failed` (and the formatted message) when `c` is false", "§18"],
    ["`flush_stdout()`", "write out what `printf` has buffered, for output watched through a pipe while the program runs", "§21"],
    ["`read_line()`", "read the next line of standard input as a `String`", "§21"],
    ["`read_all()`, `read_all_bytes()`", "the rest of standard input at once, as a `String` or as bytes", "§21"],
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
    ["`Vec::swap_remove`", "`(ref<Vec<T>, exclusive>, usize i) : T`", "takes the element at `i` out and moves the last element into its place: no other element moves, the order is not kept"],
    ["`Vec::retain`", "`(ref<Vec<T>, exclusive>, fn(ref<T, shared>) : bool keep)`", "keeps the elements `keep` accepts, in order, and destroys the rest (D-0181)"],
    ["`Vec::dedup`", "`(ref<Vec<T>, exclusive>)`", "removes each element equal to the one before it, so a sorted `Vec` keeps one of each value (`T` must be `eq`, D-0181)"],
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
    ["`slice_eq`", "`(slice<T, shared>, slice<T, shared>) : bool`", "the same length and equal elements in order; `T: eq`. `==` does not compare vectors (they are resources): `slice_eq(&a[0..$], &b[0..$])`"],
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

    String key = "n1";
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
    String line = "  name = Ada, born = 1815  ";
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
hand; all four are in `std::collections`. `crc32`, the CRC-32/IEEE checksum those formats use
(D-0119), is in `std::crypto` beside the hashes, but it only catches accidents: anyone can make
data with a chosen CRC, so use a hash or a MAC against tampering (D-0177). A `bitstruct` (§16)
crosses into bytes the same way, through `Name::bits(v)`.""")
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
    String text = "123456789";
    assert(crc32(&String::as_bytes(&text)[0..$]) == 0xCBF43926);
}
""", title="Bytes in a fixed order, and a checksum", expect="ok", section="s21", slug="bytes-crc32")
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

b += h3("Random numbers")
b += p("""<em>In `std::random`.</em>""")
b += p("""`Rng` is a general-purpose pseudo-random generator — for simulations, games, sampling,
randomised tests, shuffling — whose sequence the specification fixes, so a program that seeds it
with a constant prints the same numbers under both tools and on every machine; seed it with
`Rng::from_os()` for a fresh sequence each run. `below(&mut r, n)` is unbiased, `unit_f64` is in `[0, 1)`.
It is <strong>not suitable for cryptographic purposes</strong>: its output is predictable from a few values, so
never use it for keys, tokens, nonces, passwords, or anything an adversary must not guess (D-0120).""")
b += table(["Function", "Does"], [
    ["`Rng::new(s)`, `Rng::next_u64(&mut r)`, `Rng::below(&mut r, n)`, `Rng::unit_f64(&mut r)`", "a general-purpose pseudo-random generator (xoshiro256**, fixed by the specification): the same seed gives the same numbers everywhere; `below` is unbiased in `0 .. n`. It is not suitable for cryptographic purposes — keys, tokens, nonces, anything an adversary must not predict (D-0120)"],
    ["`Rng::shuffle(&mut r, &mut v[0..$])`, `uuid_v4()`", "a slice put in random order (Fisher-Yates; the same seed, the same order everywhere); a version-4 UUID as text, from the operating system's secure source (D-0181)"],
])
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
b += p("""For anything secret, take bytes from the operating system's cryptographically secure source
instead (`std::random`): `os_random_bytes(&mut buf[0..$])` fills a slice, `os_random_u64()` gives one
number. These are what keys, session tokens and nonces are made of. `Rng::from_os()` seeds an `Rng`
from them, which makes its sequence differ between runs but not secret: the sequence is still
`Rng`'s. Every system both tools run on has such a source; if one did not, the call would be the
fault `diag.entropy-unavailable`, never a quiet fall back to the clock.""")
b += code(CHECK_STD + """
fn main()
{
    Vec<u8> token = Vec::filled(16, 0: u8);
    os_random_bytes(&mut token[0..$]);
    String hex = String::new();
    foreach (byte in &token)
    {
        String::appendf(&mut hex, "%02x", *byte);
    }
    assert(String::len(&hex) == 32);
    Rng shuffle = Rng::from_os();                    // differs each run; not for secrets
    assert(Rng::below(&mut shuffle, 52) < 52);
}
""", title="A session token", expect="ok", section="s21", slug="os-random")
b += h3("`String`")
b += p("""<em>In `std::text`.</em>""")
b += p("""A `String` is a `Vec<u8>` that is known to be valid UTF-8: owned, heap-allocated text,
a resource. `String::new()` makes an empty one, `String::from_str` copies a `str`'s bytes
(already valid, so nothing is checked), and `String::from_utf8` validates bytes that came from
anywhere else. Where the declared type already says `String`, a literal is enough: `String s =
"text";`, a struct literal's `String` field, an argument whose parameter is a `String`, and the
value a function declared `: String` returns are each `String::from_str` of the literal, written
short (D-0149), and so is a literal argument whose generic parameter the other arguments make a
`String`: `Result::unwrap_or(arg(0), "default")`, `Vec::push(&mut names, "x")` for a `Vec<String>`
(D-0162). That is the one place a literal allocates, and the word `String` in a declaration nearby
says so; a `str` <em>variable</em> is never made a `String` without `String::from_str`, and
`auto s = "text";` is a `str`. It changes only through functions that keep it valid UTF-8 — `append` and
`appendf` add text, `push_ascii` an ASCII byte, `truncate` cuts at a character boundary, `clear`
empties it — so the guarantee holds for its whole life. `from_utf8` is the trust-transition pattern of §20 in
miniature.""")
b += table(["Function", "Signature"], [
    ["`String::new`", "`() : String` (empty)"],
    ["`String::from_str`", "`(str) : String` (copies; cannot fail); a literal where a `String` is declared is this call already (D-0149)"],
    ["`String::from_utf8`", "`(Vec<u8>) : Result<String, Utf8Error>`"],
    ["`StringView::from_utf8`", "`(slice<u8, shared>) : Result<StringView, Utf8Error>` -- the same check, and a view of the bytes instead of a copy"],
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
    String owned = "caf\\u{e9}";   // a str's bytes, copied; no validation needed
    assert(String::len(&owned) == 5);                // 5 bytes: é is two of them
    assert(String::char_count(&owned) == 4);         // but 4 characters
    assert(Vec::len(&String::chars(&owned)) == 4);   // each a view of its bytes

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
    ["`StringView::as_bytes(v)`", "the view's bytes as a `slice<u8, shared>`, not a copy: what a hash, a MAC or a socket write takes (D-0163)"],
    ["`StringView::find(v, t)`", "`Option<usize>`: where the text of the view `t` first occurs"],
    ["`StringView::starts_with(v, t)`, `ends_with`", "`bool`"],
    ["`StringView::trim(v)`, `trim_start`, `trim_end`", "the view without ASCII white space at its ends"],
    ["`StringView::split(v, \",\")`", "a `Vec<StringView>` of the parts, each a view of the same `String`"],
    ["`StringView::chars(v)`", "a `Vec<StringView>` of its characters, in order, one view each (D-0128)"],
    ["`StringView::char_count(v)`", "how many characters it holds (`len` counts bytes; D-0193)"],
    ["`StringView::contains(v, t)`", "`bool`: whether the text of `t` occurs in `v` (D-0139)"],
    ["`StringView::replace(v, from, to)`", "a new `String`: `v` with every occurrence of `from` replaced by `to`, left to right, no overlap; an empty `from` occurs nowhere"],
    ["`StringView::join(&parts[0..$], sep)`, `String::join(&strings[0..$], sep)`", "a new `String`: the parts with `sep` between each two; the inverse of `split`"],
    ["`StringView::parse<T>(v)`", "a number, as `String::parse`"],
    ["`String::from_view(v)`, `String::append(&mut s, v)`", "the text copied into a `String`"],
    ["`String::find(&s, t)`, `starts_with`, `ends_with`, `contains`, `replace`, `trim`, `trim_start`, `trim_end`, `split`, `chars`, `char_count`, `as_view`", "the same, straight from a `String`: each is the `StringView::` function applied to `&s[0..$]`, the whole-string view (D-0113)"],
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
    String text = "the cat sat on the mat";
    String word = "sat";
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
    String line = "  set width=80, height = 24  ";
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
    String path = "/usr/local/lib/cobaltc";
    assert(String::contains(&path, "/local/") && !String::contains(&path, "bin"));

    String windows = String::replace(&path, "/", "\\\\");          // a new String; path unchanged
    printf("%s\\n", &windows);
    String aaa = "aaa";
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
    String name = "ada";
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
    String s = "caf\\u{e9} \\u{1F600}!";
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

b += p("""<strong>Lines, words, prefixes and code points</strong> (D-0181). `lines` splits text at
`\n`, dropping a `\r` before it and giving no empty line after a final newline; `split_whitespace`
gives the words between runs of ASCII whitespace, none empty; `strip_prefix(v, t)` and
`strip_suffix` give the rest of the text as `Some(view)` when it begins (or ends) with `t`, and
`None` otherwise, which is how `--name=value` is read. `code_point(v)` is the number of the first
character (`None` for empty text), and `String::push_code_point(&mut s, cp)` appends one as UTF-8,
turning a surrogate or a value above U+10FFFF into U+FFFD so the `String` stays valid. Each exists on
a `String` too (`String::lines(&s)`, …). `String::parse<bool>` reads exactly `true` or `false`.""")
b += code("""
import std;

fn main()
{
    String config = "name = demo\\r\\nlevel = 3\\n\\n# a comment\\nflags = loud fast\\n";
    foreach (line in String::lines(&config))
    {
        if (StringView::starts_with(line, "#") || StringView::len(line) == 0)
        {
            continue;
        }
        Vec<StringView> words = StringView::split_whitespace(line);
        printf("%v -> %v word(s)\\n", words[0], Vec::len(&words) - 2);
    }
    foreach (a in ["--level=7", "--quiet", "file.txt"])
    {
        match (StringView::strip_prefix(StringView::of(a), "--level="))
        {
            Some(v) : printf("level %v\\n", Result::unwrap_or(StringView::parse<u32>(v), 0)),
            None    : printf("other: %s\\n", a),
        }
    }
    String s = "é";
    printf("U+%04X\\n", Option::unwrap_or(String::code_point(&s), 0));
    String t = String::new();
    String::push_code_point(&mut t, 0x1F600);
    printf("%v has %v bytes\\n", &t, String::len(&t));
}
""", title="Lines, words, prefixes and code points", expect="ok",
     output="name -> 1 word(s)\nlevel -> 1 word(s)\nflags -> 2 word(s)\nlevel 7\nother: --quiet\nother: file.txt\nU+00E9\n\U0001F600 has 4 bytes\n",
     section="s21", slug="lines-words")

b += h3("Which text type?")
b += p("""Rust programmers will recognise the pair, and C programmers may wonder why there are two
text types at all. The reason is that a constant should not have to be managed. If `"hi"` were a
`String`, every literal would be a heap allocation with an owner: it would be moved, destroyed at
scope end, and allocated again on every pass through a loop, all for text that never changes. So a
literal is a `str`, which behaves like the number `5`. `String` is kept for text that really does
need an owner.""")
b += table(["", "`str`", "`String`", "`b\"…\"`"], [
    ["What it is", "a text constant", "owned text", "raw bytes"],
    ["Written as", "`\"hello\"`", "`String::from_str(\"hello\")`, or `\"hello\"` where a `String` is declared (D-0149); `String::from_utf8(bytes)`", "`b\"hello\"`"],
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
rule). Going from a `str` value to a `String` is an explicit call that makes a copy, and nothing turns
a `String` back into a `str`: a `str` is always a literal's value, so it cannot refer to bytes that
something else owns. A <em>literal</em> is different in kind: like the `5` in `u64 x = 5;`, it has no
type until its position gives it one, so `String s = "hi";` is the literal typed as a `String`
(`String::from_str` of it, D-0149) and `StringView v = "hi";` as a view, not a value converted. A
`str` variable in the same place is still a type error. A `String` compares with `==` and `!=` to a
`str` or a `StringView`, either way round, by its bytes (`name == "sha256"`, D-0164); the orderings
`<` … `>=` stay within one text type.""")
b += h3("Printing: `printf`")
b += p("""<em>In `std::io` (`printf`, `eprintf`, `flush_stdout`) and `std::text` (`sprintf`, `String::appendf`).</em>""")
b += p("""`printf` is the one way a program writes output. It is C's, with one difference that matters: the format is checked against its
arguments when the program is compiled. The format must be a string literal; each `%` specifier
must match its argument's type and there must be exactly one argument per specifier, or the
program is rejected (`diag.type-mismatch`, or `diag.format-invalid` for a malformed specifier). So
a `printf` can never misprint or crash at run time. `sprintf(fmt, args…)` returns the same text as a
new `String` (C's `sprintf` without a buffer to overflow), and `String::appendf(&mut s, fmt, args…)`
adds it to an existing one. `eprintf(fmt, args…)` writes the same text to standard error, for
messages that are not the program's output. There is no `println`; end a line with `\\n`.""")
b += p("""Standard output is buffered, so printing line by line is fast. The text reaches the
screen, a pipe or a file in pieces, but never later than the next `eprintf`, the next read of
standard input, the start of a child that shares standard output (`Command::status`), and the
program's end, however it ends. On a terminal each line also appears as soon as it is complete. So
the two streams keep the program's order, and a prompt is shown before the program waits for the
answer. Call `flush_stdout()` only when standard output goes to a pipe or a file and someone watches
it while the program runs: a progress line, a log.""")
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
    String item = "widget";
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

    String name = "owned text";
    printf("%v", &name);                     // a String through a reference: name stays usable
    printf("\\n");
    printf("x = %v, y = %v\\n", 3, 4.5);
}
""", title="Printing values", expect="ok", output="-42\n18446744073709551615\ntrue\n0.1\n1.0\n0.3333333333333333\n0.1\n1.0e21\nNaN\nowned text\nx = 3, y = 4.5\n", section="s21", slug="print")
b += code("""
import std;

fn main()
{
    String s = "text";
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
    str t = "hi";
    String s = t;                     // ✗ diag.type-mismatch (static): t is a str value;
                                      // String::from_str(t) makes the String
}
""", title="A str value is not a String", expect="diag.type-mismatch", section="s21", slug="str-not-string")
b += code(CHECK_STD + """
fn main()
{
    String s = "hi";                  // ✓ a literal where a String is declared is String::from_str of it (D-0149)
    str t = "there";
    String u = String::from_str(t);   // ✓ the explicit, allocating bridge for a str value
    assert(String::len(&s) == 2 && String::len(&u) == 5);
}
""", title="The bridge is explicit for a value, implied for a literal", expect="ok", section="s21", slug="str-to-string")

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
        Ok(n)  :
        {
            printf("%v greeted\\n", n);
        },
        Err(e) :
        {
            match (e)
            {
                Io      :
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
b += p("""A program that needs all of its input before it can start, to sort it or parse it as one
document, takes it at once with `read_all()` (a `String`, `Err(Utf8(e))` when it is not text) or
`read_all_bytes()` (a `Vec<u8>`, any bytes). Each gives what is left after any `read_line` calls,
line ends exactly as they came, and an empty result at the end of input.""")
b += code("""
import std;

fn before(ref<StringView, shared> a, ref<StringView, shared> b) : bool
{
    StringView::less(*a, *b)
}

// Sorts the lines of its input: `sort` in a few lines.
fn main() : u8
{
    String all = match (read_all())
    {
        Ok(s)  : s,
        Err(e) :
        {
            eprintf("sort: %s\\n", FileError::text(&e));
            return 1;
        },
    };
    Vec<StringView> lines = String::split(&all, "\\n");
    if (Vec::len(&lines) > 0 && StringView::len(lines[Vec::len(&lines) - 1]) == 0)
    {
        Vec::pop(&mut lines);                       // the empty piece after the last line end
    }
    Vec::sort_by(&mut lines, before);
    foreach (l in &lines)
    {
        printf("%s\\n", *l);
    }
    0
}
""", title="All of the input at once")

b += h3("Program arguments")
b += p("""<em>In `std::env`.</em>""")
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

b += h3("Command-line options: `Args`")
b += p("""<em>In `std::env`.</em>""")
b += p("""A tool declares its options as it reads them (D-0184): `Args::flag(&mut a, "loud", "l", help)`
says whether `--loud` or `-l` was given and takes every such argument; `Args::option(&mut a, "times",
"n", "N", help)` gives the value of `--times=3`, `--times 3` or `-n 3` (the last one, or `None`), and
`Args::options` every value of a repeated option; each call also adds a line to `Args::help(&a)`.
`Args::finish(&mut a)` then gives what is left — the positional arguments, in order, with everything
after `--` among them — or an `Err` naming the first option nobody declared, or an option missing its
value, for the program to print. A lone `-` is an argument. `Args::new(usage)` reads the program's own
arguments; `Args::from(items, usage)` takes any `Vec<String>`.""")
b += code("""
import std;

fn main() : u8
{
    Args a = Result::unwrap(Args::new("greet [options] NAME..."));
    bool loud = Args::flag(&mut a, "loud", "l", "shout");
    bool help = Args::flag(&mut a, "help", "h", "show this help");
    usize times = Result::unwrap_or(StringView::parse<usize>(String::as_view(&Option::unwrap_or(Args::option(&mut a, "times", "n", "N", "how many times"), String::from_str("1")))), 1);
    if (help)
    {
        printf("%v", &Args::help(&a));
        return 0;
    }
    Vec<String> names = match (Args::finish(&mut a))
    {
        Ok(rest) : rest,
        Err(m)   :
        {
            eprintf("greet: %v\\n", &m);
            return 2;
        },
    };
    foreach (i in 0..times)
    {
        foreach (n in &names)
        {
            printf("%v, %v!\\n", if (loud) { "HELLO" } else { "hello" }, n);
        }
    }
    0
}
""", title="A tool with options", expect="ok", args=["--loud", "-n", "2", "world"],
     output="HELLO, world!\nHELLO, world!\n", section="s21", slug="args")
b += p("""Run with `--help` the same program prints:""")
b += code("""
Usage: greet [options] NAME...

Options:
  -l, --loud     shout
  -h, --help     show this help
  -n, --times N  how many times
""", title="Its help text")

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

b += p("""<strong>Numeric helpers</strong> in `std::math` (D-0181): `clamp(x, lo, hi)` holds a value within
bounds; `gcd(a, b)` is the greatest common divisor, never negative; `is_nan(x)` and `is_finite(x)`
ask about a float (an integer is always finite); and `asin`, `acos` and `atan` join the
floating-point functions. `Vec::sort` orders `f32` and `f64` too, with every NaN last.""")

b += h3("Files")
b += p("""<em>In `std::fs`.</em>""")
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
    String text = "first line\\nsecond line\\n";
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
b += p("""<em>In `std::fs`.</em>""")
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
    ["`File::create_new(path)`", "`Ok(Some(file))` on a new file, `Ok(None)` when something is already there: a lock or pid file that two programs race for (D-0181)"],
    ["`File::flush(&mut f)`, `File::position(&f)`", "make everything written durable (`fsync`) — flush, then rename, for a file that must survive a crash; the byte the next read or write starts at (D-0181)"],
    ["`File::close(f)`", "close it and say whether that worked"],
    ["`read_bytes(path)`, `write_bytes(path, &bytes[i..j])`", "`read_file` and `write_file` for bytes: a `Vec<u8>`"],
])
b += p("""Reading is buffered, so `read_line` is quick, and `File::read` fills up to what was asked for.
Writing is buffered too (D-0201), so writing a file line by line is fast. The program itself always
sees what it wrote: the bytes reach the file before the same file's next operation, before any
other file operation of the program (opening, `read_file`, `file_info`, removing, renaming), before a
child process starts, and when the program ends, however it ends. Another program watching the file
while yours runs sees them at those points; `File::flush` writes them out at once.""")
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
b += p("""<em>Directories, paths and file metadata in `std::fs`; the environment in `std::env`; the clocks in `std::time`.</em>""")
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
    ["`sleep_ms(ms)`", "the calling thread does nothing for at least `ms` milliseconds; other threads run meanwhile (D-0124)"],
    ["`make_dir_all(path)`, `remove_dir_all(path)`", "make a directory and any missing ones above it; remove a directory and everything in it"],
    ["`copy_file(from, to)`", "copy a file, replacing one at `to`; the number of bytes copied"],
    ["`file_info(path)`", "a `FileInfo`: `kind` (as `path_kind`'s), `len`, `modified_unix` (seconds since 1970) and `readonly`"],
    ["`set_current_dir(path)`", "change the working directory, for the whole program"],
    ["`temp_dir()`, `home_dir()`", "the system's directory for temporary files; the user's home directory, if known"],
    ["`current_exe()`, `hostname()`", "the running program's own executable, for the files installed beside it; the machine's name, for a log line (D-0181)"],
])
b += p("""Paths have their own functions, because a path's grammar is the operating system's: `/` on
Unix, `\\` (and `/`) on Windows, where `C:\\` starts an absolute path. They read the text alone and
never touch the file system, except `path_canonical`, which asks the system where a path really
leads (following links; the path must exist). Use `path_join` rather than gluing with `sprintf`:
it puts in the right separator, only one, and an absolute tail replaces the base.""")
b += table(["Function", "Gives"], [
    ["`path_join(base, tail)`", "`base` and `tail` with one separator between, as a `String`"],
    ["`path_parent(p)`, `path_file_name(p)`", "`Some` part, or `None`: `/a/b` and `c.txt` of `/a/b/c.txt`"],
    ["`path_stem(p)`, `path_extension(p)`", "`archive.tar` and `gz` of `archive.tar.gz`; `.bashrc` has a stem and no extension"],
    ["`path_is_absolute(p)`, `path_normalize(p)`", "whether `p` starts from a root; `p` with `.` and `x/..` removed, by the text alone"],
    ["`path_canonical(p)`", "the absolute path the system resolves `p` to"],
])
b += code("""
import std;

fn main() : u8
{
    DateTime day = DateTime::from_unix(1791022620);
    String name = sprintf("app-%d-%02d-%02d.log", day.year, day.month, day.day);
    String dir = path_join(&temp_dir()[0..$], "cobaltc-guide-logs/2026");
    if (Err(_) = make_dir_all(&dir[0..$]))
    {
        return 1;
    }
    String log = path_join(&dir[0..$], &name[0..$]);
    if (Err(_) = write_file(&log[0..$], "started\\n"))
    {
        return 1;
    }
    String stem = Option::unwrap_or(path_stem(&log[0..$]), String::new());
    String ext = Option::unwrap_or(path_extension(&log[0..$]), String::new());
    printf("%s: stem %s, extension %s, absolute %v\\n", &name, &stem, &ext, path_is_absolute(&log[0..$]));
    match (file_info(&log[0..$]))
    {
        Ok(i)  : printf("%u bytes, written in the last minute: %v\\n", i.len, unix_seconds() - i.modified_unix < 60),
        Err(e) : printf("file_info: %s\\n", FileError::text(&e)),
    }
    String top = path_join(&temp_dir()[0..$], "cobaltc-guide-logs");
    _ = remove_dir_all(&top[0..$]);
    String up = path_normalize("logs/2026/../2025/./app.log");
    printf("normalized: %s\\n", &up);
    0
}
""", title="A log file's name and place", expect="ok",
     output="app-2026-10-03.log: stem app-2026-10-03, extension log, absolute true\n8 bytes, written in the last minute: true\nnormalized: logs/2025/app.log\n",
     section="s21", slug="paths")
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

b += h3("Dates and times")
b += p("""<em>In `std::time`.</em>""")
b += p("""A `DateTime` is a moment in UTC to the second: `year` (an `i64`), `month`, `day`, `hour`,
`minute` and `second`, all fields a program can read. It converts to and from the seconds of
`unix_seconds()` and to and from ISO 8601 text, the form logs, file names and protocols use. The
one time zone `std` knows is the machine's own: `DateTime::to_local` shifts a moment into it for
showing to a person; there are no named zones and no other zone's rules.""")
b += table(["Function", "Does"], [
    ["`DateTime::now()`, `DateTime::from_unix(s)`", "the moment now, or `s` seconds after 1970-01-01T00:00:00Z (before it when negative); every `i64` is a moment"],
    ["`DateTime::new(y, mo, d, h, mi, s)`", "`Some` moment, or `None` when the fields name none (a 30 February, an hour 24, a second 60)"],
    ["`DateTime::to_unix(&t)`", "the seconds since 1970; a `DateTime` written by hand with impossible fields is the fault `diag.invalid-datetime`"],
    ["`DateTime::to_iso(&t)`, `DateTime::from_iso(text)`", "`2026-10-03T10:17:00Z` as a `String`, and back; `from_iso` also takes a date alone, as its midnight, and gives a `ParseError` for anything else"],
    ["`DateTime::weekday(&t)`, `Weekday::text(&w)`", "the day of the week, `Monday` … `Sunday`, and its name"],
    ["`DateTime::eq(&a, &b)`, `DateTime::less(&a, &b)`", "the same moment; earlier (fits `Vec::sort_by`)"],
    ["`unix_ms()`", "the wall clock in milliseconds since 1970"],
    ["`DateTime::to_local(&t)`", "the same moment written in the machine's local time, daylight saving included, by the operating system's rule"],
    ["`local_offset_seconds(unix)`", "how many seconds local time is ahead of UTC at that moment (negative when behind)"],
])
b += p("""<strong>Measure elapsed time with `monotonic_ns()`, never with the wall clock.</strong> The wall
clock is whatever the computer's owner or its time service last set it to, and it can jump backwards
while a program runs; `unix_seconds()`, `unix_ms()` and `DateTime::now()` say what time it is,
`monotonic_ns()` how long something took.""")
b += code("""
import std;

fn main() : u8
{
    DateTime launch = DateTime::from_unix(1791022620);
    String text = DateTime::to_iso(&launch);
    Weekday w = DateTime::weekday(&launch);
    printf("%s, a %s\\n", &text, Weekday::text(&w));
    DateTime due = match (DateTime::from_iso("2026-12-24"))
    {
        Ok(t)  : t,
        Err(e) :
        {
            printf("bad date: %s\\n", ParseError::text(&e));
            return 1;
        },
    };
    i64 days = (DateTime::to_unix(&due) - DateTime::to_unix(&launch)) / 86400;
    printf("%d whole days to %d-%02d-%02d\\n", days, due.year, due.month, due.day);
    match (DateTime::from_iso("2026-02-29"))
    {
        Ok(_)  : printf("a leap day?\\n"),
        Err(e) : printf("2026-02-29: %s\\n", ParseError::text(&e)),
    }
    0
}
""", title="Dates as text and back", expect="ok",
     output="2026-10-03T10:17:00Z, a Saturday\n81 whole days to 2026-12-24\n2026-02-29: out of range\n",
     section="s21", slug="dates")

b += p("""<strong>Local time is for showing, not for keeping.</strong> Store and log UTC, convert with
`DateTime::to_local` as the last step before printing, and print a local value by its fields: it is a
plain `DateTime` that knows nothing of its zone, so `to_iso` of it would end in `Z` and `to_unix` of it
is not the moment. The offset depends on the machine and on the moment (clocks change twice a year
in many zones), so this program's output differs from place to place:""")
b += code("""
import std;

fn main() : u8
{
    DateTime now = DateTime::now();
    DateTime here = DateTime::to_local(&now);
    i64 offset = local_offset_seconds(DateTime::to_unix(&now));
    str sign = if (offset < 0) { "-" } else { "+" };
    i64 magnitude = if (offset < 0) { 0 - offset } else { offset };
    printf(
        "%s UTC is %04d-%02d-%02d %02d:%02d:%02d here (UTC%s%02d:%02d)\\n",
        &DateTime::to_iso(&now),
        here.year,
        here.month,
        here.day,
        here.hour,
        here.minute,
        here.second,
        sign,
        magnitude / 3600,
        magnitude % 3600 / 60,
    );
    0
}
""", title="Showing a moment in local time (output depends on the machine)")

b += p("""<strong>Milliseconds and HTTP dates</strong> (D-0181). A log line or a JSON document wants the
moment to the millisecond: `iso_from_unix_ms(unix_ms())` writes `2026-10-07T12:34:56.789Z`, and
`unix_ms_from_iso(text)` reads it back (`from_iso` reads the same text and drops the fraction).
`DateTime::to_http_date(&t)` writes the form HTTP headers carry, `Sun, 06 Nov 1994 08:49:37 GMT`, and
`DateTime::from_http_date(text)` reads it and the two older forms a server may still send.""")

b += h3("Running other programs")
b += p("""<em>In `std::process`.</em>""")
b += p("""A `Command` names a program and its arguments; running it starts a child process. There is
<strong>no shell</strong> in between: each `arg` reaches the program exactly as written, spaces,
quotes, `$` and `;` included, so text from a user can be passed as an argument without any way to
run something else. A program that wants a shell's features runs one by name, `/bin/sh` with `-c`,
and then quoting is its own business.""")
b += table(["Function", "Does"], [
    ["`Command::new(program)`, `Command::arg(&mut c, a)`", "a command; one more argument"],
    ["`Command::current_dir(&mut c, dir)`, `Command::env(&mut c, name, value)`, `Command::clear_env(&mut c)`", "where the child starts; an environment variable for it; start it with no inherited variables"],
    ["`Command::output(&c)`, `Command::output_with_input(&c, input)`", "run to the end; an `Output` with `status`, `stdout` and `stderr` (bytes)"],
    ["`Command::status(&c)`", "run to the end with this program's own input and output; the status"],
    ["`Command::spawn(&c)`", "start it and return a `Child` at once, its three streams piped to this program"],
    ["`Child::write_input`, `close_input`, `read_output`, `read_output_line`, `read_error`", "talk to a running child, as to a `File`"],
    ["`Child::wait(&mut ch)`, `try_wait`, `kill`", "wait for the end (its status), ask without waiting, or end it at once"],
])
b += p("""A status is the program's exit code, or minus the signal that ended it (`-9` after `kill`
on Unix). Failing to start is a `FileError`, as every input and output failure is: `NotFound`
when there is no such program. Dropping a
`Child` closes its pipes but leaves it running, like a job sent to the background; `wait` for it if
it should finish first.""")
b += code("""
import std;

fn main() : u8
{
    Command c = Command::new("/bin/sh");
    Command::arg(&mut c, "-c");
    Command::arg(&mut c, "echo \\"$1 world\\"; echo oops >&2; exit 3");
    Command::arg(&mut c, "sh");
    Command::arg(&mut c, "hello; echo injected");   // one argument, never run as a command
    match (Command::output(&c))
    {
        Ok(o)  :
        {
            String out = Result::unwrap_or(String::from_utf8(Vec::clone(&o.stdout)), String::new());
            printf("status %d, said: %s", o.status, &out);
            printf("and %u bytes on standard error\\n", Vec::len(&o.stderr));
        },
        Err(e) :
        {
            printf("could not run it: %s\\n", FileError::text(&e));
            return 1;
        },
    }
    Command gone = Command::new("no-such-program-anywhere");
    if (Err(NotFound) = Command::output(&gone))
    {
        printf("no-such-program-anywhere: not found\\n");
    }
    0
}
""", title="Capturing a program's output", expect="ok",
     output="status 3, said: hello; echo injected world\nand 5 bytes on standard error\nno-such-program-anywhere: not found\n",
     section="s21", slug="process")
b += h4("Stopping cleanly")
b += p("""Pressing Ctrl-C, or the system asking a service to stop (SIGTERM), ends a program on the
spot. A server that should finish the request in hand and close its files first calls
`watch_interrupts()`: from then on an interrupt only sets a flag, and `interrupt_requested()` reads
it. There is no callback; the program's own loop looks at the flag, often enough. A blocking call
an interrupt arrives in may return early (a listener's `accept` and a socket's `read` give
`Err(TimedOut)`), and `TcpListener::set_accept_timeout_ms` bounds how long a quiet server waits
before looking again.""")
b += p("""To end the program from anywhere, `exit(status)` (D-0150): the calling thread's scopes and
frames are unwound and every destructor runs, as they do when a fault ends the program, standard
output is flushed, and the process ends with `status`, with no diagnostic. Other threads take no
further steps. `exit` has type `never`, so it can stand in for any value: `None : exit(2)` in a
`match` arm, or `exit(1)` as a statement after an error message. Returning from `main` is still the
ordinary way to end; `exit` is for the place that knows the program is over and is not `main`.""")
b += code("""
import std;

resource struct Lock
{
    i32 id;
}

fn Lock::drop(ref<Lock, exclusive> self)
{
    printf("lock %d released\\n", self.id);
}

fn check(i32 value) : i32
{
    Lock inner = Lock { .id = 2 };
    if (value < 0)
    {
        eprintf("negative input\\n");
        exit(3);                             // inner and outer are released first
    }
    value * 2
}

fn main() : u8
{
    Lock outer = Lock { .id = 1 };
    i32 r = check(-5);
    printf("never printed %d\\n", r);
    0
}
""", title="Ending the program with its destructors run", expect="ok", status=3, output="lock 2 released\nlock 1 released\n", section="s21", slug="exit")
b += code("""
import std;

fn main()
{
    watch_interrupts();
    u64 handled = 0;
    while (!interrupt_requested() && handled < 3)
    {
        handled += 1;                       // one unit of work: a request, a job, a file
    }
    printf("handled %u, stopping cleanly\\n", handled);
}
""", title="A loop that stops when asked", expect="ok", output="handled 3, stopping cleanly\n",
     section="s21", slug="interrupts")

b += p("""<strong>Also</strong> (D-0181): `Child::read_error_line(&mut ch)` reads the child's standard error a
line at a time, as `read_output_line` does its output; `Child::terminate(&mut ch)` asks it to stop
(SIGTERM on Unix, which a program may catch to finish cleanly) where `kill` ends it at once; and
`process_id()` is this program's own number, for a pid file. `kill` and `terminate` signal the child
alone: a `sh -c "…"` killed leaves the commands it started running. `Child::kill_tree(&mut ch)` (D-0205)
ends the child and every process it started, and theirs (on Linux, found from `/proc`; on Windows with
`taskkill /T`).""")
b += p("""<strong>Streaming between processes</strong> (D-0195). A `Child`'s pipes all belong to it, and
every call borrows it exclusively, so one thread cannot write a child's input while another reads
its output. `Child::take_input(&mut ch)` and `Child::take_output(&mut ch)` move a pipe out into a
`ChildInput` or `ChildOutput` of its own (`None` if it was already taken): give one to a thread
(`spawn(pump, out_a, in_b)`) and keep reading the other child here. `ChildInput::write`,
`ChildOutput::read` and `read_line` work as the `Child`'s own calls do; dropping an end closes it, and
an end outlives its `Child`.""")

b += h3("Cryptography, and bytes as text")
b += p("""<em>In `std::crypto`, with hexadecimal and base64 in `std::encoding::hex` and `std::encoding::base64`.</em>""")
b += p("""`sha256(data)` is the SHA-256 digest of some bytes, 32 of them as an `array<u8, 32>`;
`hmac_sha256(key, data)` is a message authentication code under a key, the way a webhook body,
a session cookie or a signed request is protected. Both come in a streaming form for a message that
arrives in pieces: `Sha256::new()`, `Sha256::update(&mut h, piece)` as often as needed,
`Sha256::finish(h)`; `HmacSha256` the same. A `Sha256` is a plain value, so a copy of a half-done
computation continues on its own. Everything is written in CobaltC (the spec lists it), so the two
tools compute exactly the same bytes; for a message of a few kilobytes the interpreter's speed does
not matter.""")
b += p("""<strong>Check a MAC with `digest_eq`, never with `==`.</strong> `digest_eq(a, b)` compares every byte
whatever the first difference, so how long the check takes says nothing about how many leading bytes
a sender guessed right. A digest is shown with `to_hex` and read back with `from_hex`; bytes that
must travel as text go through `base64_encode` and `base64_decode` (RFC 4648, padded). Random keys
come from `os_random_bytes`. A secret is rarely used as it arrives: `hkdf_sha256(salt, secret, info, len)`
derives `len` bytes of key material from it, bound to what they are for by `info`, so one secret
never keys two purposes (RFC 5869; TLS 1.3's key schedule is this function). When a protocol or
a file format names its hash, `hash(kind, data)`, `Hasher`, `hmac(kind, key, data)`, `Hmac` and the
`hkdf_*_with(kind, …)` forms take a `HashKind` (`Sha2_256`, `Sha2_384` or `Sha2_512`) at run time;
the SHA-256 forms are the same functions with `Sha2_256`.""")
b += p("""<strong>Encrypting.</strong> `chacha20_poly1305_seal(key, nonce, aad, plaintext)` encrypts and
authenticates (RFC 8439): a 32-byte key, a 12-byte nonce that must never repeat under one key, and
`aad`, data sent in the clear but protected from change; the result is the ciphertext and a 16-byte
tag. `chacha20_poly1305_open` gives the plaintext back, or `None` if a single bit of the message, the
tag or the `aad` was changed, and never any part of a refused message. <strong>Agreeing on a
key.</strong> Each side makes `x25519_private_key()`, sends `x25519_public_key(&private[0..$])`, and
computes `x25519(&private[0..$], &their_public[0..$])`: both get the same 32 bytes, which nobody who
saw only the public keys can (RFC 7748); `None` means the other side sent a key that must be refused.
Feed the shared secret to `hkdf_sha256`, never use it as a key directly. <strong>The NIST
alternatives.</strong> `aes_gcm_seal` and `aes_gcm_open` are the same pair with AES-GCM (NIST SP
800-38D), for formats and protocols that require it: the key is 16, 24 or 32 bytes (AES-128, AES-192
or AES-256), the nonce 12. `p256_private_key`, `p256_public_key` and `p256_ecdh` agree on a key over
the P-256 curve; the public key is 65 bytes, and `None` means a key that must be refused. Where you
choose, X25519 agrees on a key in about half the time P-256 takes; the two AEADs run at about the
same speed (AES-128-GCM and ChaCha20-Poly1305 each about 50 ms a MiB here). All of these take the same steps
whatever the secret bytes are, so their timing reveals nothing but lengths; a key or nonce of the
wrong length is the fault `diag.crypto-length`.""")
b += code("""
import std;

fn main()
{
    array<u8, 32> alice = x25519_private_key();
    array<u8, 32> bob = x25519_private_key();
    array<u8, 32> alice_public = x25519_public_key(&alice[0..$]);
    array<u8, 32> bob_public = x25519_public_key(&bob[0..$]);
    array<u8, 32> shared = Option::unwrap(x25519(&alice[0..$], &bob_public[0..$]));
    Vec<u8> nothing = Vec::new();
    Vec<u8> key = hkdf_sha256(&nothing[0..$], &shared[0..$], &b"example key"[0..$], 32);
    Vec<u8> nonce = Vec::filled(12, 0: u8);
    Vec<u8> sealed = chacha20_poly1305_seal(&key[0..$], &nonce[0..$], &b"header"[0..$], &b"attack at dawn"[0..$]);
    // Bob derives the same key from his side of the exchange.
    array<u8, 32> bobs = Option::unwrap(x25519(&bob[0..$], &alice_public[0..$]));
    Vec<u8> bob_key = hkdf_sha256(&nothing[0..$], &bobs[0..$], &b"example key"[0..$], 32);
    match (chacha20_poly1305_open(&bob_key[0..$], &nonce[0..$], &b"header"[0..$], &sealed[0..$]))
    {
        Some(p) : printf("bob reads: %s\\n", &Result::unwrap_or(String::from_utf8(p), String::new())),
        None    : printf("refused\\n"),
    }
    sealed[0] = sealed[0] ^ 1;
    printf(
        "changed in transit: %s\\n",
        if (Option::is_none(&chacha20_poly1305_open(
            &bob_key[0..$],
            &nonce[0..$],
            &b"header"[0..$],
            &sealed[0..$],
        ))) { "refused" } else { "accepted" },
    );
}
""", title="A key exchange and an encrypted message", expect="ok",
     output="bob reads: attack at dawn\nchanged in transit: refused\n", section="s21", slug="aead")
b += table(["Function", "Does"], [
    ["`sha256(data)`, `Sha256::new()`, `Sha256::update(&mut h, data)`, `Sha256::finish(h)`", "the SHA-256 digest, in one call or in pieces: an `array<u8, 32>`"],
    ["`hmac_sha256(key, data)`, `HmacSha256::new(key)`, `update`, `finish`", "HMAC-SHA-256 under `key` (any length; a long key is hashed first)"],
    ["`digest_eq(a, b)`", "the same bytes, compared in a time that depends only on the lengths"],
    ["`hkdf_sha256(salt, ikm, info, len)`, `hkdf_extract(salt, ikm)`, `hkdf_expand(prk, info, len)`", "HKDF (RFC 5869): `len` bytes of key material derived from a secret, bound to `info`; at most 8160 bytes from one extracted key"],
    ["`chacha20_poly1305_seal(key, nonce, aad, plaintext)`, `chacha20_poly1305_open(key, nonce, aad, sealed)`", "authenticated encryption (RFC 8439): ciphertext and tag; the plaintext back, or `None` for anything changed"],
    ["`chacha20(key, nonce, counter, data)`, `poly1305(key, message)`", "the stream cipher and the one-time MAC the AEAD is built from"],
    ["`aes_gcm_seal(key, nonce, aad, plaintext)`, `aes_gcm_open(key, nonce, aad, sealed)`, `aes_encrypt_block(key, block)`", "AES-GCM (SP 800-38D) with a 16-, 24- or 32-byte key, as the ChaCha20-Poly1305 pair; the bare AES block cipher (FIPS 197)"],
    ["`x25519_private_key()`, `x25519_public_key(private)`, `x25519(private, their_public)`", "key agreement (RFC 7748): a key pair, and the shared secret, `None` for a public key that must be refused"],
    ["`p256_private_key()`, `p256_public_key(private)`, `p256_ecdh(private, their_public)`", "key agreement over P-256: a 65-byte public key, and the 32-byte shared secret, `None` for a key that must be refused"],
    ["`sha512(data)`, `sha384(data)`, `Sha512`, `Sha384`", "the 64- and 48-byte SHA-2 digests, one-shot or streaming"],
    ["`hash(kind, data)`", "the digest by a `HashKind`: `Sha2_256`, `Sha2_384` or `Sha2_512`"],
    ["`rsa_verify_pkcs1v15(&key, kind, digest, sig)`, `rsa_verify_pss(&key, kind, digest, sig)`", "whether an RSA signature (PKCS #1 v1.5, or PSS with a digest-length salt) is the key's over that digest"],
    ["`ecdsa_verify(curve, public_key, digest, sig)`", "whether a DER ECDSA signature on `P256` or `P384` is the key's over that digest"],
    ["`hkdf_expand_label(secret, label, context, len)`", "TLS 1.3's key-schedule step (RFC 8446 §7.1)"],
    ["`Hasher::new(kind)`, `hmac(kind, key, data)`, `Hmac::new(kind, key)`, `hkdf_with(kind, salt, ikm, info, len)`, `hkdf_extract_with`, `hkdf_expand_with`, `hkdf_expand_label_with`", "the same over a `HashKind` chosen at run time (SHA-256, SHA-384 or SHA-512); results are `Vec<u8>`"],
    ["`ed25519_private_key()`, `ed25519_public_key(private)`, `ed25519_sign(private, message)`, `ed25519_verify(public, message, sig)`", "Ed25519 (RFC 8032): 32-byte keys, 64-byte deterministic signatures over the whole message"],
    ["`ec_private_key(curve)`, `ec_public_key(curve, private)`, `ecdh(curve, private, their_public)`, `ecdsa_sign(curve, private, digest)`", "P-256 or P-384 keys, key agreement, and a DER ECDSA signature over a digest with an RFC 6979 nonce"],
    ["`rsa_generate_key(bits)`, `RsaPrivateKey::new(n, e, d, p, q)`, `RsaPrivateKey::public_key(&key)`, `rsa_sign_pkcs1v15(&key, kind, digest)`, `rsa_sign_pss(&key, kind, digest)`", "an RSA key (1024 to 8192 bits), and signatures in the two encodings the verify functions check"],
    ["`hash_password(password)`, `verify_password(stored, password)`", "Argon2id password storage as a PHC string, and its check"],
    ["`argon2id(password, salt, passes, memory_kib, lanes, len)`, `pbkdf2(kind, password, salt, iterations, len)`", "the raw key-derivation functions (RFC 9106, RFC 8018)"],
    ["`blake2b(data, len)`, `Blake2b::new(len)`, `Blake2b::new_keyed(key, len)`, `update`, `finish`", "BLAKE2b (RFC 7693): a digest of 1 to 64 bytes, optionally keyed"],
    ["`to_hex(bytes)`, `from_hex(text)`", "lowercase hexadecimal and back (`ParseError` for a non-digit or an odd count)"],
    ["`base64_encode(bytes)`, `base64_decode(text)`", "base64 with `=` padding and back (`ParseError` for a bad character or length)"],
])
b += code("""
import std;

fn main()
{
    array<u8, 32> d = sha256(&b"abc"[0..$]);
    printf("%s\\n", &to_hex(&d[0..$]));
    array<u8, 32> mac = hmac_sha256(&b"Jefe"[0..$], &b"what do ya want for nothing?"[0..$]);
    printf("%s\\n", &to_hex(&mac[0..$]));
    array<u8, 32> again = hmac_sha256(&b"Jefe"[0..$], &b"what do ya want for nothing?"[0..$]);
    printf("verified: %v\\n", digest_eq(&mac[0..$], &again[0..$]));
    printf("%s\\n", &base64_encode(&b"foobar"[0..$]));
}
""", title="A digest, a MAC, and their text forms", expect="ok",
     output="ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\n5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843\nverified: true\nZm9vYmFy\n",
     section="s21", slug="crypto")

b += p("""<strong>Signatures.</strong> Three families, each with a key pair, a signing function and a
verifying one. <em>Ed25519</em> (RFC 8032) is the simplest and the one to choose where you can:
`ed25519_private_key()`, `ed25519_public_key(&private[0..$])`, `ed25519_sign(&private[0..$],
message)` gives 64 bytes, `ed25519_verify(&public[0..$], message, &sig[0..$])` says whether they are
that key's signature of that message. Signing is deterministic: the same key and message give the
same bytes, and no random number is involved that could repeat. <em>ECDSA</em> on `P256` or `P384`
(what certificates and TLS mostly carry): `ec_private_key(curve)`, `ec_public_key(curve, &k[0..$])`
(the uncompressed point, 65 or 97 bytes), `ecdsa_sign(curve, &k[0..$], &digest[0..$])` (a DER
signature over the digest made with `hash`; the nonce is derived from the key and the digest as
RFC 6979 says, so it too is deterministic) and `ecdsa_verify(curve, public, digest, sig)`; the same
keys do key agreement with `ecdh(curve, &k[0..$], &their_public[0..$])`, of which the `p256_*`
functions are the P-256 case. <em>RSA</em>: `rsa_generate_key(2048)` makes an `RsaPrivateKey`
(minutes under the interpreter, seconds compiled), or `RsaPrivateKey::new(n, e, d, p, q)` builds one
from numbers read elsewhere (`None` if they do not fit together); `rsa_sign_pkcs1v15(&key, kind,
&digest[0..$])` and `rsa_sign_pss(&key, kind, &digest[0..$])` sign the digest in the two standard
encodings, `RsaPrivateKey::public_key(&key)` gives the `RsaPublicKey` that `rsa_verify_pkcs1v15` and
`rsa_verify_pss` check with. Signing with any of them takes the same steps whatever the key; the
RSA operation is blinded and checked against the public key before its result leaves. Verification
uses only public values and answers `false` for anything malformed.""")
b += p("""<strong>Keys in files.</strong> Keys travel in the encodings every tool writes, in
`std::x509`: a public key as a SubjectPublicKeyInfo (`CertKey::from_der`, `from_pem`, `to_der`,
`to_pem`), a private key as PKCS #8 (`PrivateKey::from_der`, `from_pem`, `to_der`, `to_pem`), where
`from_pem` also reads the older `RSA PRIVATE KEY` and `EC PRIVATE KEY` blocks. A `PrivateKey` is
`RsaPrivate`, `EcPrivate` or `Ed25519Private`; `PrivateKey::public_key(&key)` is its `CertKey`, and
`sign(&key, &scheme, message)` signs a whole message under a `SignatureScheme` the way
`verify_signature` checks it, hashing first where the scheme says so. A certificate's key is a
`CertKey` too, so a program can check a signature against a certificate it parsed.""")
b += code("""
import std;

fn main()
{
    array<u8, 32> private = ed25519_private_key();
    array<u8, 32> public = ed25519_public_key(&private[0..$]);
    array<u8, 64> sig = ed25519_sign(&private[0..$], &b"release 1.4.0"[0..$]);
    printf("genuine: %v\\n", ed25519_verify(&public[0..$], &b"release 1.4.0"[0..$], &sig[0..$]));
    printf("altered: %v\\n", ed25519_verify(&public[0..$], &b"release 1.4.1"[0..$], &sig[0..$]));
    // The same key, written the way openssl would.
    PrivateKey key = Ed25519Private(private);
    String pem = PrivateKey::to_pem(&key);
    printf("%s", StringView::sub(String::as_view(&pem), 0, 27));
    printf("\\nread back: %v\\n", Result::is_ok(&PrivateKey::from_pem(String::as_view(&pem))));
}
""", title="Signing a release note", expect="ok",
     output="genuine: true\naltered: false\n-----BEGIN PRIVATE KEY-----\nread back: true\n", section="s21", slug="sign")
b += p("""<strong>Passwords.</strong> Store `hash_password(&password[0..$])`, a text like
`$argon2id$v=19$m=19456,t=2,p=1$…$…` that records the Argon2id parameters and the random salt beside
the result; check a login with `verify_password(stored, &attempt[0..$])`. Argon2id (RFC 9106) costs
memory as well as time (19 MiB and two passes here, the OWASP recommendation), so a stolen table
cannot be searched at hardware speed; the parameters can be raised later and old entries still
verify, since each carries its own. Never store a plain `sha256` of a password, salted or not. For
the formats that predate Argon2, `pbkdf2(kind, password, salt, iterations, len)` is PBKDF2 over
HMAC, and `argon2id(password, salt, passes, memory_kib, lanes, len)` is the raw function.
`blake2b(data, len)` (RFC 7693, up to 64 bytes, `Blake2b::new_keyed` for a keyed MAC) is the fast
hash Argon2 is built on. Under the interpreter `hash_password` takes many minutes (19 MiB of
memory touched twice, word by word); compiled, well under a second.""")

b += p("""<strong>SHA-1</strong> is there too (`Sha1`, `sha1`, and `Sha1` as a `HashKind` for `hmac`,
`pbkdf2` and `Hasher`, D-0181), because older protocols still require it: RFC 6238's one-time
passwords default to HMAC-SHA-1, and git names its objects by it. It is legacy — collisions have been
made — so it is for checking what others produce, never for a new signature.""")

b += h3("Big integers")
b += p("""<em>In `std::math`.</em>""")
b += p("""A `BigUint` is an unsigned integer of any size: `add`, `sub`, `mul`, `div_rem` (a `BigDivision`
of `quotient` and `remainder`), `rem`, `mod_pow`, `mod_inverse`, `clone`, shifts, `eq` and `less`, and conversions to and from
big-endian bytes and decimal text. It owns its digits, so it is a resource: reassign one with
`overwrite`, and take a division apart by destructuring. Its operations take time that depends on the
numbers, so it is for public values, never for a secret key.""")
b += code("""
import std;

fn main()
{
    BigUint f = BigUint::from_u64(1);
    for (u64 i = 2; i <= 30; i += 1)
    {
        overwrite(&mut f, BigUint::mul(&f, &BigUint::from_u64(i)));
    }
    printf("30! = %s\\n", &BigUint::text(&f));
    BigDivision { quotient, remainder } = BigUint::div_rem(&f, &BigUint::from_u64(1000000007));
    printf("mod 1000000007 = %s\\n", &BigUint::text(&remainder));
}
""", title="Thirty factorial", expect="ok", output="30! = 265252859812191058636308480000000\nmod 1000000007 = 109361473\n", section="s21", slug="biguint")

b += h3("Certificates")
b += p("""<em>In `std::x509`.</em>""")
b += p("""`Certificate::parse(der)` reads an X.509 certificate; `certificates_from_pem(text)` reads every
certificate in PEM text. A `TrustStore` holds the roots a chain may end at: `TrustStore::system()` reads
the operating system's bundle on Unix (on Windows, pass a bundle to `TrustStore::from_pem_file`).
`verify_chain(&leaf, &intermediates[0..$], &roots, host, &now)` decides whether a server's certificates
prove it is `host`: the leaf names the host (a DNS name, a one-label wildcard, or an IP address; never
the common name), every certificate is in date and signed by the next, every issuer is a CA, and the
chain ends at a root. The error says why not: `HostMismatch`, `Expired`, `UnknownAuthority` and so on.
Revocation (CRL, OCSP) is not checked.""")

b += h3("TLS")
b += p("""<em>In `std::tls`.</em>""")
b += p("""`TlsStream::connect(addr, server_name, &roots)` opens a TLS 1.3 connection: it checks the
server's certificate chain against `roots` and `server_name`, and then `read`, `read_exact`, `write`
and `close` carry data encrypted and authenticated. `TlsStream::client(tcp, …)` does the same over a
`TcpStream` the program connected and configured itself (its timeouts apply). The client speaks only
TLS 1.3, with ChaCha20-Poly1305, AES-128-GCM or AES-256-GCM, X25519, P-256 or P-384, and ECDSA,
RSA-PSS or Ed25519 signatures: the set every TLS 1.3 server must support and the common additions; a
failure is a `TlsError` saying whether it was the network, the certificate, the handshake or an
alert. The example is shown, not run: it needs the network.""")
b += note("""Run TLS clients compiled. Checking a server's certificate chain is several public-key
signature checks, a few milliseconds compiled but tens of seconds in the interpreter (`TrustStore::system()`
alone parses every root certificate). Many servers close a connection whose handshake stalls for
about ten seconds, and the interpreter's client then fails with the network error `closed`; under
`cobc` the same program completes in well under a second.""")
b += code("""
import std;

fn main() : u8
{
    TrustStore roots = match (TrustStore::system())
    {
        Ok(t)  : t,
        Err(_) : return 1,
    };
    IpAddr ip = Result::unwrap(IpAddr::parse("1.1.1.1"));
    TlsStream s = match (TlsStream::connect(SocketAddr::new(ip, 853), "cloudflare-dns.com", &roots))
    {
        Ok(s)  : s,
        Err(e) :
        {
            eprintf("tls: %s\\n", TlsError::text(&e));
            return 1;
        },
    };
    // … a DNS-over-TLS query: a 2-byte length, then the DNS message …
    _ = TlsStream::close(&mut s);
    0
}
""", title="Connecting to a DNS-over-TLS resolver (needs the network)")

b += p("""<strong>Serving TLS</strong> (D-0186). `TlsStream::server(tcp, &key, &chain[0..$])` performs the
handshake as the server, with a `PrivateKey` (RSA, P-256, P-384 or Ed25519, read with
`PrivateKey::from_pem`) and the server's certificate chain (`certificates_from_pem`, the server's
own certificate first); the client's first suite, key share and the key's signature scheme are
chosen, and a client that cannot agree is answered with an alert. Afterwards `read_line`, `read`,
`write` and `close` work as on the client side, and `peer_certificate` (`None` on a server, the
server's certificate on a client) is there for pinning. `TcpStream::try_clone` has no counterpart
on a `TlsStream`: its record keys are one state. The example is shown, not run: it needs a key and
a certificate (`openssl req -x509 -newkey ed25519 -nodes -subj /CN=localhost -addext
subjectAltName=DNS:localhost` makes a self-signed pair for development).""")
b += code("""
import std;

fn main() : u8
{
    PrivateKey key = Result::unwrap(PrivateKey::from_pem(String::as_view(&Result::unwrap(read_file("server.key")))));
    Vec<Certificate> chain = certificates_from_pem(String::as_view(&Result::unwrap(read_file("server.crt"))));
    TcpListener l = Result::unwrap(TcpListener::bind(Result::unwrap(SocketAddr::parse("0.0.0.0:8443"))));
    while (true)
    {
        TcpStream tcp = Result::unwrap(TcpListener::accept(&mut l));
        TlsStream s = match (TlsStream::server(tcp, &key, &chain[0..$]))
        {
            Ok(s)  : s,
            Err(e) :
            {
                eprintf("handshake: %s\\n", TlsError::text(&e));
                continue;
            },
        };
        while (Some(line) = Result::unwrap_or(TlsStream::read_line(&mut s), None))
        {
            _ = TlsStream::write_text(&mut s, "echo: ");
            _ = TlsStream::write_text(&mut s, String::as_view(&line));
            _ = TlsStream::write_text(&mut s, "\\n");
        }
        _ = TlsStream::close(&mut s);
    }
    0
}
""", title="A TLS echo server (needs a key and a certificate)")

b += h3("HTTP")
b += p("""<em>In `std::http`.</em>""")
b += p("""`http_get(url)` fetches a URL and `http_post(url, content_type, body)` posts to one; each gives
a `Response` with its `status`, `reason`, `headers` and whole `body`, or an `HttpError` saying what
went wrong (the network, TLS, a malformed URL or message, a limit). Both follow redirects, decode
chunked bodies and speak `https` through `std::tls` with the system roots. A program that makes more
than one request, or wants a timeout, a header on every request or a smaller body limit, keeps an
`HttpClient` and calls `HttpClient::send(&mut client, &request)`: the roots are loaded once, at the
first `https` request. `Url::parse` takes a URL apart (`scheme`, `host`, `port`, `path`, `query`),
`percent_encode` and `percent_decode` handle the escaping, `find_header` (or
`Response::header`) finds a header whatever its case, and `Response::text` gives the body as a `String`
(an error when it is not UTF-8). HTTP/1.1 only and one connection per request, unless the
client's `keep_alive` is set (D-0195): then the connection stays open while the server allows, and
the next request to the same host and port goes on it (a server for such a client must not count
on one connection per request). If that connection turns out closed before any answer, a GET, HEAD,
PUT, DELETE, OPTIONS or TRACE is sent again on a fresh one; a POST is not -- the server may have acted
on it -- and is `ConnectionClosed` for the program to decide. No compression and no cookies; what a server sends compressed it sends only when asked, so plain
requests get plain bodies.""")
b += p("""<strong>Serving.</strong> `HttpServer::bind(addr)` listens; `HttpServer::accept(&mut server)`
gives each client's `HttpConnection`, on which `HttpConnection::request(&mut c)` parses the next
request (`Ok(None)` once the client is done) and `HttpConnection::respond(&mut c, status, &headers,
body)` writes the answer with the right `Content-Length`, reason phrase and `Connection` header,
keeping the connection open when the client asked for that. A malformed request is an `Err` the
program answers with 400 (or 413 for one over `max_body`); the connection then closes. Handle each
connection in its own thread with `spawn`, as the example does; the server speaks plain HTTP (a TLS
server is a later decision). The example is hermetic: its client and server are the same program.""")
b += code("""
import std;

fn serve(HttpConnection c) : u8
{
    while (true)
    {
        Vec<Header> headers = Vec::new();
        Vec::push(&mut headers, Header::new("Content-Type", "text/plain"));
        match (HttpConnection::request(&mut c))
        {
            Ok(Some(req)) :
            {
                if (req.url == "/health")
                {
                    _ = HttpConnection::respond(&mut c, 200, &headers[0..$], &b"ok\\n"[0..$]);
                }
                else if (req.method == "POST" && req.url == "/echo")
                {
                    _ = HttpConnection::respond(&mut c, 200, &headers[0..$], &req.body[0..$]);
                }
                else
                {
                    _ = HttpConnection::respond(&mut c, 404, &headers[0..$], &b"no such page\\n"[0..$]);
                }
            },
            Ok(None) : return 0,
            Err(_)   :
            {
                _ = HttpConnection::respond(&mut c, 400, &headers[0..$], &b"bad request\\n"[0..$]);
                return 1;
            },
        }
    }
    0
}

fn accept_two(HttpServer s) : u8
{
    for (usize i = 0; i < 2; i += 1)
    {
        match (HttpServer::accept(&mut s))
        {
            Ok(c)  : { _ = join(spawn(serve, c)); },
            Err(_) : return 1,
        }
    }
    0
}

fn main()
{
    HttpServer s = Result::unwrap(HttpServer::bind(SocketAddr::new(IpAddr::localhost_v4(), 0)));
    String base = sprintf("http://127.0.0.1:%u", HttpServer::local_addr(&s).port);
    auto server = spawn(accept_two, s);
    match (http_get(String::as_view(&sprintf("%s/health", &base))))
    {
        Ok(r)  : printf("%u %s: %s", r.status, &r.reason, &Result::unwrap_or(Response::text(&r), String::new())),
        Err(e) : printf("error: %s\\n", HttpError::text(&e)),
    }
    match (http_post(String::as_view(&sprintf("%s/echo", &base)), "text/plain", &b"hello there"[0..$]))
    {
        Ok(r)  : printf("%u, %s echoed %v bytes: %s\\n", r.status, Option::unwrap_or(Response::header(&r, "content-type"), StringView::of("?")), Vec::len(&r.body), &Result::unwrap_or(Response::text(&r), String::new())),
        Err(e) : printf("error: %s\\n", HttpError::text(&e)),
    }
    printf("server thread: %u\\n", join(server));
}
""", title="A health check and an echo, served and fetched", expect="ok",
     output="200 OK: ok\n200, text/plain echoed 11 bytes: hello there\nserver thread: 0\n", section="s21", slug="http")

b += p("""<strong>https, forms and compressed bodies.</strong> `HttpServer::bind_tls(addr, key, chain)`
makes the same server speak https (D-0186): `accept` completes each client's TLS handshake first,
and gives `Err(TlsFailure(_))` for a client that fails it, so `accept`'s error is an `HttpError`.
A request's query string and a form body are read with `Url::query_pairs(&url)` and
`form_decode(text)`, each a `Vec<Param>` (`name`, `value`, compared exactly) searched with
`find_param`; `form_encode` writes one (D-0181). The client offers `Accept-Encoding: gzip, deflate`
and decodes a body a server sends that way, dropping the header, so `r.body` is always the plain
bytes (D-0183).""")
b += code("""
import std;

fn main()
{
    Vec<Param> form = form_decode("name=Ada+Lovelace&note=100%25&tag=x&tag=y");
    foreach (p in &form)
    {
        printf("%v = %v\\n", &p.name, &p.value);
    }
    printf("%v\\n", Option::unwrap_or(find_param(&form[0..$], "note"), StringView::of("-")));
    printf("%v\\n", &form_encode(&form[0..2]));
    Url u = Result::unwrap(Url::parse("https://example.org/search?q=cobalt%20c&page=2"));
    printf("%v\\n", Option::unwrap_or(find_param(&Url::query_pairs(&u)[0..$], "q"), StringView::of("-")));
}
""", title="Query strings and form bodies", expect="ok",
     output="name = Ada Lovelace\nnote = 100%\ntag = x\ntag = y\n100%\nname=Ada+Lovelace&note=100%25\ncobalt c\n",
     section="s21", slug="forms")

b += h3("JSON")
b += p("""<em>In `std::encoding::json`.</em>""")
b += p("""`Json::parse(text)` reads a JSON document (RFC 8259) into a tree of `Json` values — `JsonNull`,
`JsonBool`, `JsonNumber` (an `f64`), `JsonText`, `JsonArray` and `JsonObject` (its members in order) —
and `Json::text(&j)` writes one back compact, `Json::pretty(&j)` indented. Reading a tree:
`Json::get(&j, "key")` and `Json::at(&j, i)` give a reference to a member or an element, or `None`,
and `as_text`, `as_i64`, `as_f64`, `as_bool`, `as_array`, `as_object` and `is_null` ask what a value is.
Building one: `Json::set(&mut obj, "key", value)` and `Json::push(&mut arr, value)`. A document that is
not JSON is `Err(Invalid(byte))`, nothing at all `Err(Empty)`, a number beyond `f64` or nesting past
512 levels `Err(OutOfRange)`. Every escape is handled, surrogate pairs included; a whole number is
written without a fraction; a NaN or an infinity, which JSON cannot carry, is written `null`
(D-0182).""")
b += code("""
import std;

fn main()
{
    Json j = Result::unwrap(Json::parse("{\\"name\\": \\"Ada\\", \\"born\\": 1815, \\"langs\\": [\\"Analytical\\", \\"Engine\\"]}"));
    StringView name = Option::unwrap_or(Json::as_text(Option::unwrap(Json::get(&j, "name"))), StringView::of("?"));
    i64 born = Option::unwrap_or(Json::as_i64(Option::unwrap(Json::get(&j, "born"))), 0);
    printf("%v, born %v, knew", name, born);
    foreach (l in Option::unwrap(Json::as_array(Option::unwrap(Json::get(&j, "langs")))))
    {
        printf(" %v", Option::unwrap_or(Json::as_text(l), StringView::of("?")));
    }
    printf("\\n");
    Json out = JsonObject(Vec::new());
    Json::set(&mut out, "name", JsonText(String::from_str("Ada")));
    Json langs = JsonArray(Vec::new());
    Json::push(&mut langs, JsonText(String::from_str("Analytical")));
    Json::set(&mut out, "langs", langs);
    Json::set(&mut out, "ok", JsonBool(true));
    printf("%v\\n%v\\n", &Json::text(&out), &Json::pretty(&out));
    match (Json::parse("{\\"a\\": [1, 2,]}"))
    {
        Err(Invalid(at)) : printf("not JSON at byte %v\\n", at),
        _                : {},
    }
}
""", title="Reading and writing JSON", expect="ok",
     output="Ada, born 1815, knew Analytical Engine\n{\"name\":\"Ada\",\"langs\":[\"Analytical\"],\"ok\":true}\n{\n  \"name\": \"Ada\",\n  \"langs\": [\n    \"Analytical\"\n  ],\n  \"ok\": true\n}\nnot JSON at byte 12\n",
     section="s21", slug="json")

b += h3("Compression")
b += p("""<em>In `std::compress`.</em>""")
b += p("""`deflate(data)` and `inflate(data, max)` are DEFLATE (RFC 1951), the format under zip, PNG and
git; `zlib_deflate`/`zlib_inflate` wrap it with zlib's header and Adler-32 (RFC 1950), and
`gzip`/`gunzip` with gzip's header and CRC-32 (RFC 1952), so a `.gz` file or an HTTP body is one call
away. Decompression takes a `max`, the most it may produce, and answers `Err(Oversize)` beyond it — a
small stream can expand without bound, so a limit is always there; `Truncated` is a stream cut short
and `Corrupt(byte)` one that is not what it claims, a wrong checksum included. Compression uses one
fixed strategy, so every implementation writes the same bytes, and reads everything other tools
write; `adler32` is there beside `crc32` (D-0183).""")
b += code("""
import std;

fn main()
{
    String text = String::new();
    foreach (i in 0..20)
    {
        String::append(&mut text, "the same line again and again\\n");
    }
    Vec<u8> packed = gzip(StringView::as_bytes(String::as_view(&text)));
    printf("%v bytes became %v\\n", String::len(&text), Vec::len(&packed));
    Vec<u8> back = Result::unwrap(gunzip(&packed[0..$], 1 << 20));
    printf("round trip: %v\\n", Vec::len(&back) == String::len(&text) && digest_eq(&back[0..$], StringView::as_bytes(String::as_view(&text))));
    match (gunzip(&packed[0..10], 1 << 20))
    {
        Err(e) : printf("cut short: %s\\n", CompressError::text(&e)),
        Ok(_)  : {},
    }
}
""", title="gzip and back", expect="ok", output="600 bytes became 52\nround trip: true\ncut short: compressed data ends early\n",
     section="s21", slug="compress")

b += h3("Databases")
b += p("""<em>In `std::database::postgres`, re-exported by `std::database`.</em>""")
b += p("""A PostgreSQL client, speaking the server's own wire protocol over `std::net` and `std::tls`, with
SCRAM-SHA-256 from `std::crypto`; nothing is linked, so a program that talks to a database still
needs only a C compiler. `PgConnection::connect(url)` opens a connection from a
`postgres://user:password@host:port/database` URL (the port 5432 and the database the user's name
when left out), encrypting with TLS and verifying the server's certificate against the system roots
unless the URL says `?sslmode=disable`; `connect_with(url, &roots, timeout_ms)` takes its own
`TrustStore` (a self-signed server's issuer, say) and timeout. There is no unverified TLS.""")
b += p("""`PgConnection::query(&mut c, sql, &params[0..$])` runs one statement with `$1`, `$2`, … bound to
the `PgValue`s given (`Null`, `Text`, `Int`, `Float`, `Bool`, `Bytes`): a parameter is sent beside
the statement, never spliced into it, so no quoting is ever needed. The `PgResult` has the
`columns` (name and type OID), the `rows` (each cell `Some(text)` as the server prints the value,
or `None` for NULL; read it with `PgRow::get(&row, i)` and parse it with `StringView::parse<T>`),
the `affected` count and the command `tag`. `execute` is `query` for a statement without rows,
giving the count; `run_script` runs a file of `;`-separated statements without parameters, for
schemas and migrations. Transactions are statements: `execute(&mut c, "BEGIN", &none[0..$])` and
`"COMMIT"` or `"ROLLBACK"`.""")
b += p("""An error the server reports is `Err(Server(e))` with its `severity`, SQLSTATE `code`, `message`,
`detail`, `hint` and `position`; the connection stays usable after it, as it does after a result
over `max_result` (16 MiB by default, `ResultTooLarge`). A network or protocol failure closes the
connection, and every later call gives `Disconnected`. Notices the server sends collect in
`c.notices` for the program to read and clear. `PgError::text(&e)` is a `String` saying what
went wrong, the server's message included. One connection serves one thread; a program that
queries from several keeps one per thread.""")
b += table(["Function", "Does"], [
    ["`PgConnection::connect(url)`, `connect_with(url, &roots, timeout_ms)`", "open a connection (30 s timeout; the system roots), or with the roots and timeout given"],
    ["`PgConnection::query(&mut c, sql, &params[0..$])`", "one statement with `$n` bound to the `PgValue`s: its `PgResult` (`columns`, `rows`, `affected`, `tag`)"],
    ["`PgConnection::execute(&mut c, sql, &params[0..$])`", "the same, giving only the affected count"],
    ["`PgConnection::run_script(&mut c, sql)`", "several `;`-separated statements, no parameters, results dropped, the first error given"],
    ["`PgConnection::parameter(&c, name)`, `c.notices`, `c.max_result`", "a setting the server reported at startup (`server_version`, …); the notices received; the most a result may take"],
    ["`PgRow::get(&row, i)`, `PgResult::column(&res, name)`", "a cell's text (`None` for NULL or past the end); a column's index by name"],
    ["`PgConnection::close(&mut c)`", "tell the server goodbye and close (a dropped connection is just closed)"],
    ["`PgUrl::parse(text)`, `PgValue::text(&v)`", "the URL taken apart; the text a parameter is sent as"],
])
b += code("""
import std;

fn main() : u8
{
    PgConnection c = match (PgConnection::connect("postgres://app:secret@db.internal/shop"))
    {
        Ok(c)  : c,
        Err(e) :
        {
            eprintf("connect: %s\\n", &PgError::text(&e));
            return 1;
        },
    };
    Vec<PgValue> none = Vec::new();
    if (Err(e) = PgConnection::run_script(&mut c, "CREATE TABLE IF NOT EXISTS orders (id serial PRIMARY KEY, item text, qty int)"))
    {
        eprintf("schema: %s\\n", &PgError::text(&e));
        return 1;
    }
    Vec<PgValue> order = Vec::new();
    Vec::push(&mut order, Text(String::from_str("widget")));
    Vec::push(&mut order, Int(3));
    match (PgConnection::execute(&mut c, "INSERT INTO orders (item, qty) VALUES ($1, $2)", &order[0..$]))
    {
        Ok(n)  : printf("inserted %v\\n", n),
        Err(e) : printf("insert: %s\\n", &PgError::text(&e)),
    }
    Vec<PgValue> at_least = Vec::new();
    Vec::push(&mut at_least, Int(2));
    match (PgConnection::query(&mut c, "SELECT id, item, qty FROM orders WHERE qty >= $1 ORDER BY id", &at_least[0..$]))
    {
        Ok(res) :
        {
            foreach (row in &res.rows)
            {
                StringView item = Option::unwrap_or(PgRow::get(row, 1), StringView::of("?"));
                i64 qty = Result::unwrap_or(StringView::parse<i64>(Option::unwrap_or(PgRow::get(row, 2), StringView::of("0"))), 0);
                printf("%s x %v\\n", item, qty);
            }
        },
        Err(Server(e)) : printf("%s: %s\\n", &e.code, &e.message),
        Err(e)         : printf("query: %s\\n", &PgError::text(&e)),
    }
    PgConnection::close(&mut c);
    0
}
""", title="Orders in a PostgreSQL table (needs a server)")
b += p("""Without a server the pieces can still be seen: the URL's defaults and the text each parameter
is sent as.""")
b += code("""
import std;

fn main()
{
    PgUrl u = Result::unwrap(PgUrl::parse("postgresql://app@db.internal"));
    printf("%s@%s:%u/%s tls=%v\\n", &u.user, &u.host, u.port, &u.database, match (u.sslmode) { Disable : false, VerifyFull : true });
    Vec<PgValue> params = Vec::new();
    Vec::push(&mut params, Int(-7));
    Vec::push(&mut params, Float(0.5));
    Vec::push(&mut params, Bool(true));
    Vec::push(&mut params, Bytes(Vec::from_slice(&b"\\x00\\xff"[0..$])));
    Vec::push(&mut params, Null);
    foreach (v in &params)
    {
        printf("%s\\n", &Option::unwrap_or(PgValue::text(v), String::from_str("NULL")));
    }
}
""", title="A URL's defaults and the parameters' text", expect="ok",
     output="app@db.internal:5432/app tls=true\n-7\n0.5\nt\n\\x00ff\nNULL\n", section="s21", slug="postgres")

b += p("""A cell read as a value: `PgRow::get_i64(&row, i)`, `get_f64`, `get_bool` and `get_bytes` (D-0181)
give `None` for NULL, for a column past the end, and for text of another form; `get` still gives the
text.""")

b += h3("Networking")
b += p("""<em>In `std::net`.</em>""")
b += p("""TCP connections, UDP datagrams and name resolution, over IPv4 and IPv6. Addresses are values:
an `IpAddr` (`V4` or `V6`) and a port make a `SocketAddr`, read from text with `SocketAddr::parse`
(`"127.0.0.1:8080"`, `"[::1]:8080"`) or built with `SocketAddr::new(IpAddr::localhost_v4(), 8080)`;
`resolve(host, port)` asks the system's resolver.""")
b += p("""<strong>Sockets block.</strong> `accept` waits for a connection, `read` for bytes, `connect`
for the other side. A server handles many connections at once by giving each its own thread:
`spawn` a function with the accepted `TcpStream` moved into it, and use a `Channel` (§19) where
threads must talk. Every socket can have timeouts; a call that waits longer gives
`Err(TimedOut)`, and the socket is still usable. There is no TLS, and nothing is added to what you
send: the bytes that arrive are exactly the bytes written, in order, in pieces of any size, so a
protocol needs its own framing — lines, read with `read_line`, are the simplest.""")
b += table(["Function", "Does"], [
    ["`TcpListener::bind(addr)`, `TcpListener::local_addr(&l)`", "listen at `addr` (port 0: any free port); where it listens"],
    ["`TcpListener::accept(&mut l)`, `set_accept_timeout_ms(&mut l, ms)`", "the next connection, a `TcpStream`; give up after `ms` with `Err(TimedOut)`"],
    ["`TcpStream::connect(addr)`, `connect_timeout(addr, ms)`", "connect to a listener"],
    ["`TcpStream::read_line(&mut s)`, `read(&mut s, &mut buf, max)`, `read_exact(&mut s, &mut buf, n)`", "as `File`'s: a line, what has arrived (0: the peer has finished), exactly `n` bytes"],
    ["`TcpStream::write(&mut s, data)`, `write_text(&mut s, text)`, `printf(&mut s, fmt, …)`", "send all of it"],
    ["`TcpStream::shutdown_write(&mut s)`", "tell the peer nothing more is coming (it reads 0), and keep reading"],
    ["`set_read_timeout_ms`, `set_write_timeout_ms`, `set_nodelay`", "per-stream timeouts (0: none); send small writes at once"],
    ["`UdpSocket::bind(addr)`, `send_to(&mut u, data, to)`, `recv_from(&mut u, max)`", "datagrams: one message each, with its sender in the `Datagram`"],
    ["`UdpSocket::try_clone(&u)`", "a second handle on the same socket: one thread waits in `recv_from` while another sends on the clone"],
    ["`UdpSocket::connect(&mut u, to)`, `send(&mut u, data)`, `recv(&mut u, max)`", "a socket talking to one peer: `recv` drops datagrams from anyone else (D-0181)"],
    ["`TcpStream::try_clone(&s)`", "a second handle on the same connection, for one thread to read while another writes; the connection closes when the last handle is dropped (D-0185)"],
    ["`TcpStream::set_keepalive(&mut s, on)`", "have the system probe an idle connection, so a vanished peer is noticed (D-0181)"],
])
b += code("""
import std;

// One connection: each line back in capitals, until the client finishes.
fn serve(TcpStream s) : u32
{
    u32 lines = 0;
    while (Ok(Some(line)) = TcpStream::read_line(&mut s))
    {
        String up = String::to_ascii_upper(&line);
        if (Err(_) = TcpStream::printf(&mut s, "%s\\n", &up))
        {
            break;
        }
        lines += 1;
    }
    lines
}

// Accepts `n` connections, each served by its own thread.
fn listen(TcpListener l, usize n) : u32
{
    Vec<handle<u32>> served = Vec::new();
    for (usize i = 0; i < n; i += 1)
    {
        if (Ok(s) = TcpListener::accept(&mut l))
        {
            Vec::push(&mut served, spawn(serve, s));
        }
    }
    u32 total = 0;
    while (Some(h) = Vec::pop(&mut served))
    {
        total += join(h);
    }
    total
}

fn ask(SocketAddr at, str word) : String
{
    match (TcpStream::connect(at))
    {
        Ok(c)  :
        {
            _ = TcpStream::printf(&mut c, "%s\\n", word);
            match (TcpStream::read_line(&mut c))
            {
                Ok(Some(answer)) : answer,
                _                : String::from_str("(no answer)"),
            }
        },
        Err(e) : String::from_str(NetError::text(&e)),
    }
}

fn main() : u8
{
    TcpListener l = match (TcpListener::bind(SocketAddr::new(IpAddr::localhost_v4(), 0)))
    {
        Ok(l)  : l,
        Err(e) :
        {
            printf("bind: %s\\n", NetError::text(&e));
            return 1;
        },
    };
    SocketAddr at = TcpListener::local_addr(&l);
    auto server = spawn(listen, l, 3);
    foreach (word in ["alpha", "beta", "gamma"])
    {
        String answer = ask(at, word);
        printf("%s -> %s\\n", word, &answer);
    }
    printf("the server answered %u lines\\n", join(server));
    0
}
""", title="An echo server and its clients in one program", expect="ok",
     output="alpha -> ALPHA\nbeta -> BETA\ngamma -> GAMMA\nthe server answered 3 lines\n",
     section="s21", slug="echo-server")
b += p("""A server that runs until it is stopped combines this with `watch_interrupts` (above): give
the listener an accept timeout, and the accept loop looks at the flag between waits.""")
b += code("""
import std;

fn main() : u8
{
    watch_interrupts();
    TcpListener l = match (TcpListener::bind(SocketAddr::new(IpAddr::localhost_v4(), 0)))
    {
        Ok(l)  : l,
        Err(_) : return 1,
    };
    TcpListener::set_accept_timeout_ms(&mut l, 50);
    u32 waits = 0;
    while (!interrupt_requested() && waits < 3)      // a real server: while (!interrupt_requested())
    {
        match (TcpListener::accept(&mut l))
        {
            Ok(s)         : printf("a client\\n"),    // spawn a thread for it here
            Err(TimedOut) : waits += 1,               // nobody came: look at the flag again
            Err(e)        :
            {
                printf("accept: %s\\n", NetError::text(&e));
                return 1;
            },
        }
    }
    printf("closing the listener and stopping\\n");
    0
}
""", title="A server loop that can be stopped", expect="ok", output="closing the listener and stopping\n",
     section="s21", slug="server-loop")

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
    String who = "lin";
    assert(Result::unwrap_or(Vec::binary_search(&names, &who), 99) == 1);
    String bob = "bob";
    assert(match (Vec::binary_search(&names, &bob))
    {
        Ok(_)  : false,
        Err(i) : i == 1,              // between "ada" and "lin"
    });

    Vec<Player> ps = Vec::new();
    Vec::push(&mut ps, Player { .name = "a", .score = 3 });
    Vec::push(&mut ps, Player { .name = "b", .score = 9 });
    Vec::push(&mut ps, Player { .name = "c", .score = 3 });
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
    ["`HashMap::clone(&m)`, `HashSet::clone(&s)`, `clone(&m)`", "a copy of a map or set whose contents are `clone` (§12); `Rc::clone` shares instead"],
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

    String cat = "cat";
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
shared reference); put a `mutex` inside if you need mutation. `Rc` is not thread-safe.
A `Weak<T>`, from `Rc::downgrade`, keeps the box but not the value: `Weak::upgrade` is
`Some(Rc)` while a strong handle exists, else `None`. It is for a parent pointer, a cache or an
observer list, where an `Rc` would make a cycle (D-0125).""")
b += table(["Function", "Signature"], [
    ["`Rc::new`", "`(T) : Rc<T>`"],
    ["`Rc::clone`", "`(ref<Rc<T>, shared>) : Rc<T>`"],
    ["`Rc::get`", "`(ref<Rc<T>, shared>) : ref<T, shared>`"],
    ["`Rc::downgrade`", "`(ref<Rc<T>, shared>) : Weak<T>`"],
    ["`Weak::upgrade`", "`(ref<Weak<T>, shared>) : Option<Rc<T>>`"],
    ["`Weak::clone`", "`(ref<Weak<T>, shared>) : Weak<T>`"],
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
in; `Box::get(&b)` and `Box::get_mut(&mut b)` borrow it; `Box::into_inner(b)` moves it back out; `Box::clone(&b)` copies a box whose value is `clone`;
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

    Pair p = Pair { .left = "L", .right = "RR" };
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
unsafe extern fn name(rawptr<u8> buf, usize len) : isize;

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
    ["`diag.overwrite-of-live-resource`", "both", "assigning over a resource that is still owned and is not quiet (its end runs a destructor or releases something)", "`overwrite(&mut x, v)`, or `drop` first"],
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
    ["`diag.extern-non-ffi-type`", "static", "an `unsafe extern fn` with a non-FFI parameter or result", "pass scalars or `rawptr`"],
    ["`diag.alloc-failure`", "dynamic", "`Vec` or `Rc` could not allocate", "call `allocate` yourself if you need to recover"],
])
APPX = Section("appx", "A", "Appendix: diagnostics", None,
               "Every named diagnostic, what it means, and the usual fix.", b)
APPX.group = "Appendix"

SECTIONS = [S17, S18, S19, S20, S21, S22, APPX]
