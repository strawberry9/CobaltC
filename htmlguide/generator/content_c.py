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
b += code(CHECK + """
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
    check(bank::fee(1000) == 20);      // qualified with ::
    check(bank::audit::twice(1000) == 40);
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
b += code(CHECK + """
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
    check(square(6) == 36);           // resolves through the import
    check(maths::square(7) == 49);    // the qualified form still works
}
""", title="import", expect="ok", section="s17", slug="import")
b += p("""`import m;` naming a module does two things: `m` itself resolves, as with any import, and
every item `m` exports, including the variants of its exported enums, can be used unqualified. This
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
            Dot : printf("dot\\n"),
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

b += h3("Constants")
b += p("""`const τ NAME = e;` names a value, at the top level of any module. `NAME` can be used
wherever its value could, with the visibility, `export`, `import` and qualified paths of any item.
`e` must be a <em>constant expression</em>: literals, other constants, operators, struct, array and
variant literals of constant expressions, a function's name, and the pure intrinsics (`min_value`,
`sizeof`, `widen`, …). A constant cannot be a resource, cannot depend on itself, and cannot be
assigned to or borrowed; an overflow in computing one is reported at the constant, before the
program runs. Integer, float, `bool` and `str` constants cost nothing at run time: each use becomes
the literal.""")
b += code(CHECK_STD + """
const u64 KIB = 1024;
const u64 MIB = KIB * KIB;
const u32 READ = 0x1;
const u32 WRITE = 0x2;
const u32 READ_WRITE = READ | WRITE;
const i32 LOWEST = -2147483648;

struct Size
{
    u32 w;
    u32 h;
}

const Size SCREEN = Size { .w = 640, .h = 480 };

fn main()
{
    check(MIB == 1048576);
    check(READ_WRITE == 3);
    check(LOWEST < 0);
    check(SCREEN.w * SCREEN.h == 307200);
}
""", title="Constants", expect="ok", section="s17", slug="constants")
b += p("""A constant is typed exactly as a declaration `τ x = e;` would be: in
`const u64 MIB = 1024 * 1024;` the literals are `u64`s, and an initializer that overflows its type,
such as `const i32 BIG = 65536 * 65536;`, is rejected at the constant.""")

b += h3("Static assertions")
b += p("""`static_assert(c)` and `static_assert(c, "why")` check a constant condition when the program
is compiled, not when it runs: a false one stops it with `diag.static-assert-failed` and shows the
message. The condition is a constant expression (the same kind a `const` has), so it can use
constants, `sizeof`, `alignof`, the limits and the conversions. It is checked in every function,
called or not, and in a generic function once for each type it is used with, so
`static_assert(sizeof<T>() <= 8, "…")` limits what `T` may be. At run time it does nothing.""")
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
    check(word(5: i64) == 5 && word(2.5) == 2.5);
}
""", title="Checked before the program runs", expect="ok", section="s17", slug="static-assert")
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
""", title="A failed assertion", expect="diag.static-assert-failed", section="s17", slug="static-assert-fails")

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
b += code(CHECK + """
// main.cb -- run this one: `coby main.cb`
module geo "./geometry.cb";           // module geo { … } with geometry.cb's items as the body

import geo::Rect;

fn main()
{
    Rect r = Rect { .w = 3.0, .h = 4.0 };
    check(geo::area(r) == 12.0);      // qualified, as with any module
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

b += rules([
    "Start every module with everything private; export only what callers need.",
    "Prefer qualified calls (`maths::square(x)`) in code that reads like a library; `import` the handful of names you use constantly.",
    "Module boundaries are not safety boundaries: `unsafe` is lexical (§20). To confine trusted code, keep the functions that wrap it un-exported.",
    "Split a program across files with `module m \"./m.cb\";`, one file per module; keep paths relative and let the declaring file do the naming.",
])
S17 = Section("s17", "§17", "Modules and name resolution", "17-modules.md",
              "module, export and import; private by default; a module's body may be another file; a fixed lookup order for every name.", b)

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
b += p("""Faults are named `diag.…` and listed in the appendix. Both the interpreter and a compiled
program print the name, the rule that fired, what was required and what was observed, and a repair
hint.""")

b += h3("`Result`, `Option` and `?`")
b += p("""`Result<T, E>` is `Ok(T)` or `Err(E)`; `Option<T>` is `Some(T)` or `None`. Both are plain
enums of `std`, taken apart with `match`. The postfix `?` operator unwraps a `Result`: on `Ok(v)` it
yields `v`, on `Err(e)` it returns `Err(e)` from the enclosing function immediately. That function
must therefore return `Result<_, E>` with exactly the same `E`; convert first with
`map_err(r, f)` if the error types differ. Where a default will do, `Option::unwrap_or(o, d)` and
`Result::unwrap_or(r, d)` give the value, or `d` when there is none; the default is evaluated
either way, and one that goes unused is destroyed.""")
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
        Ok(v) :
        {
            check(v == 42);
        },
        Err(_) :
        {
            check(false);
        },
    }
    match (two_digits(b'4', b'x'))
    {
        Ok(_) :
        {
            check(false);
        },
        Err(e) :
        {
            check(e == b'x');
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
              "Fatal checked faults that unwind, and recoverable Result values with the ? operator. Nothing in between.", b)

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
waits for the thread and returns its result, consuming the handle. A handle is a resource: if it is
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
    check(join(h) == 42);             // wait and take the result

    Vec<i32> data = Vec::new();
    Vec::push(&mut data, 5);
    auto h2 = spawn(move [data]()     // a move closure carries the Vec to the other thread
    {
        Vec::len(&data)
    });
    check(join(h2) == 1);

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
b += code(CHECK + """
fn worker(ref<mutex<i32>, shared> m, i32 n) : i32
{
    i32 i = 0;
    while (i < n)
    {
        auto g = lock(m);             // blocks until the mutex is free
        *g = *g + 1;
        i = i + 1;
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
    check(a + b == 7);
    i32 total = *lock(&m);            // a temporary guard: it lives until the semicolon
    check(total == 7);                // whatever the interleaving, every increment was serialised
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

b += h3("What is not provided")
b += p("""No atomics, no memory orderings, no condition variables, no deadlock detection, no
thread-local storage. Steps of different threads interleave at the granularity of single
operations and every operation sees a consistent state; a `join` or a `lock` that cannot proceed
simply waits. `Rc<T>` is not thread-safe; to share across threads, share a `mutex`.""")
b += note("""The compiler `cobc` runs threads in parallel on separate cores; the interpreter `coby`
runs them one at a time. A program that follows the rules gets the same result from both. Where the
language leaves an outcome to timing — writing a variable while another thread may still be reading
it through a reference is `diag.aliasing-conflict` if the thread is still running, and fine if it has
finished — each is free to show either permitted outcome, and they may differ.""", kind="note", title="Parallel or interleaved")

b += rules([
    "Every thread has an owner: the handle. Join it explicitly to get the result; let scope end join it if you do not care.",
    "Share a `mutex` by shared reference; keep each guard in the smallest block that needs it.",
    "Move data into a thread with a `move` closure or by-value arguments; lend it with references only when the mutex rule allows.",
])
S19 = Section("s19", "§19", "Concurrency", "19-concurrency.md",
              "Threads owned by handles, mutexes reachable only through guards, and the same aliasing rules everywhere.", b)

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
b += code(CHECK + """
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
    check(read_through(rawptr_of(&x)) == 7);
    auto bytes = [1: u8, 2: u8, 3: u8];
    rawptr<u8> b = reinterpret_ptr<u8>(rawptr_of(&bytes));
    unsafe
    {
        check(*(b + 2) == 3);         // pointer offset, in units of the pointee size
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
    auto p = rawptr_of(Vec::index_shared(&v, 0));   // the element's address; obtaining it is safe
    unsafe
    {
        auto r = &reclaim<i32>(p);    // a checked reference to the element, via its address
        check(*r == 7);
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
        Ok(p) : p,
        Err(_) :
        {
            return out;
        },
    };
    rawptr<i64> buf = reinterpret_ptr<i64>(raw);
    i64 written = unsafe { squares(buf, widen<i64>(n)) };
    if (written >= 0 && written <= widen<i64>(n))
    {
        usize i = 0;
        while (i < narrow<usize>(written))
        {
            Vec::push(&mut out, unsafe { *(buf + reinterpret<isize>(i)) });
            i = i + 1;
        }
    }
    unsafe
    {
        deallocate(raw, bytes, alignof<i64>());
    }
    out
}
"""

b += h3("Foreign functions")
b += p("""`extern fn name(params) : ret;` declares a function provided from outside. Parameters and
result may only be integers, floats, `bool`, `void` and `rawptr<T>`; anything else, including a
`ref` or a struct, is rejected (`diag.extern-non-ffi-type`). Calling it needs `unsafe`. Its result
is a <em>claim</em>: a genuine value of its declared type, but one whose relationship to anything
else (that a returned length is right, that a pointer is valid) is not established until your code
checks it.""")
b += p("""`std` declares two externs: `write(rawptr<u8> buf, usize len) : isize`, which
`printf` (§21) writes through, with the `unsafe` block inside `std`, and its counterpart `read`,
which `read_line` reads through. Calling conventions and how code is linked are outside the language
and documented by each implementation; a program names the C code it needs with `extern "…";`
(below).""")
b += p("""Both implementations call real C functions on x86-64 Linux: an `extern fn` names a
function in the C library or the maths library by its declared name. The compiler passes any
FfiType, pointers included. The interpreter's memory is simulated, so it can pass only integers,
`bool` and floats, and reports a call that needs a pointer as unsupported rather than guessing.""")
b += code(CHECK + """
extern fn sqrt(f64 x) : f64;          // from the maths library
extern fn labs(i64 x) : i64;          // from the C library
extern fn toupper(i32 c) : i32;

fn main()
{
    f64 root = unsafe
    {
        sqrt(2.0)
    };
    check(root * root > 1.9999 && root * root < 2.0001);   // a claim, checked before it is trusted
    i64 distance = unsafe
    {
        labs(3 - 10)
    };
    check(distance == 7);
    check(unsafe { toupper(99) } == 67);                   // 'c' -> 'C'
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
        write(p, 3)                   // the extern call; its result is a claim about how much was written
    };
}
""", title="Calling the write extern", expect="ok", output="Hi\n", section="s20", slug="extern-write")
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
b += p("""The standard library is the module `std`: the `Option` and `Result` enums, `printf`, `eprintf`, `sprintf`,
`write`, `read`, `read_line`, `arg_count`, `arg`, `arg_bytes`, `parse`, `read_file`, `write_file` and `map_err`, and the library types written in CobaltC itself, `Vec<T>`, `String`,
`HashMap<K, V>`, `HashSet<K>`, `Rc<T>` and `Box<T>`, and `swap` and `replace`. A program uses it with `import std;`, or by qualified paths such as `std::Vec`. Its source
is part of the specification, which is the language's proof that the mechanisms in this guide are
enough to build real containers. Being a module, it keeps its private parts private: no program
can touch `Vec`'s length or buffer, `String`'s bytes, a `HashMap`'s table or `Rc`'s count. The intrinsics and the text
value type `str` belong to the language itself and need no import.""")

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

export enum ReadError                 // why read_line failed
{
    Io,
    Utf8(Utf8Error)
}

export enum FileError                 // why read_file or write_file failed
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

b += h3("Intrinsics")
b += p("""Intrinsics are called like functions; their behaviour is given by rules rather than by a
body. Type arguments are written `name<T>(…)` where the signature needs one. Their names are not
reserved: a function or variable of your own with the same name is what the name means, and the
intrinsic is used only where there is none.""")
b += table(["Intrinsic", "Purpose", "See"], [
    ["`drop(x)`", "destroy a resource now", "§07"],
    ["`wrapping_*`, `saturating_*`, `checked_*`", "alternative overflow policies", "§06"],
    ["`widen<U>`, `narrow<U>`, `narrow_wrapping<U>`, `reinterpret<U>`, `to_float<U>`, `to_int<U>`", "numeric conversions", "§06"],
    ["`sizeof<T>()`, `alignof<T>()`", "layout queries, in bytes", "§06, §16"],
    ["`min_value<T>()`, `max_value<T>()`", "an integer type's smallest and largest value", "§06"],
    ["`spawn(f, args…)`, `join(h)`", "threads", "§19"],
    ["`Mutex::new(v)`, `lock(&m)`", "mutual exclusion", "§19"],
    ["`rawptr_of(&x)`, `reinterpret_ptr<U>(p)`", "obtain and retype raw pointers (safe)", "§20"],
    ["`reclaim<T>(p)`, `release(p, n)`, `copy_raw(dst, src, n)`", "raw memory as objects (unsafe)", "§20"],
    ["`allocate(n, align)`, `deallocate(p, n, align)`", "raw allocation (`deallocate` is unsafe)", "§20"],
    ["`dangling<T>()`", "a well-aligned non-null address that must never be dereferenced; used for empty containers", "§21"],
    ["`write(buf, len)`", "an extern of `std`: write bytes to standard output (unsafe)", "§20"],
    ["`read(buf, len)`", "an extern of `std`: read bytes from standard input (unsafe; `cobc` only)", "§21"],
    ["`str_len(s)`, `str_byte(s, i)`, `str_ptr(s)`", "a `str`'s byte count, one byte (bounds-checked), and the address of its bytes (safe)", "§21"],
    ["`printf(\"%v\", x)`", "write a `str`, a number, a `bool` or a `String` (through `&`) to standard output; a function of `std`", "§21"],
    ["`printf(fmt, args…)`, `eprintf(fmt, args…)`, `sprintf(fmt, args…)`, `String::appendf(&mut s, fmt, args…)`", "C-style formatted output, to standard output, to standard error, as a new `String` or into one, checked when compiled (functions of `std`)", "§21"],
    ["`read_line()`", "read the next line of standard input as a `String` (a function of `std`; `cobc` only)", "§21"],
    ["`arg_count()`, `arg(i)`", "the number of program arguments, and argument `i` as a `String` (functions of `std`)", "§21"],
    ["`arg_bytes(i, buf, len)`", "an extern of `std`: copy argument `i`'s bytes, whatever they are (unsafe)", "§21"],
    ["`parse<T>(&s)`", "read a whole `String` as a number of type `T` (a function of `std`)", "§21"],
    ["`read_file(&path)`, `write_file(&path, &text)`", "read a whole file as a `String`, or replace a file's contents (functions of `std`)", "§21"],
    ["`Option::unwrap_or(o, d)`, `Result::unwrap_or(r, d)`", "the value, or the default `d` when there is none (functions of `std`)", "§18"],
    ["`map_err(r, f)`", "convert a `Result`'s error type with `f` (a function of `std`)", "§18"],
])

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
`\\u{…}` for a Unicode scalar value; a literal must close on its own line.""")
b += p("""`printf("%v", s)` writes a `str` to standard output. It prints numbers, `bool` and `String`
too (see "Printing: `printf`" below). The `unsafe` block that reaches the `write` extern is inside `std`, so a
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
    check(str_len(greeting) == 5);
    check(str_byte(greeting, 0) == 104);
    check(greeting == again);
    check("caf\\u{e9}" != "cafe");     // \\u{e9} is é, two bytes
    check(str_len("caf\\u{e9}") == 5);
    auto named = Named { .name = greeting, .n = 1 };
    auto copy = named;                // a struct of plain values is plain
    check(copy.name == "hello");
    Vec<str> words = Vec::new();
    Vec::push(&mut words, greeting);
    Vec::push(&mut words, "world");
    shout(*Vec::index_shared(&words, 1));
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
otherwise). Use it where bytes are the honest type, such as data handed to `write` or to a
validator.""")

b += h3("`Vec<T>`")
b += p("""A growable array with a raw buffer, a length and a capacity. It is a `resource` whose
destructor destroys each element and frees the buffer. Elements are reached by reference, never
copied out (so `Vec<Vec<i32>>` works without any special support).""")
b += table(["Function", "Signature", "Notes"], [
    ["`Vec::new`", "`() : Vec<T>`", "`T` from the expected type or written `Vec<i32>::new()`; allocates nothing"],
    ["`Vec::len`", "`(ref<Vec<T>, shared>) : usize`", ""],
    ["`Vec::push`", "`(ref<Vec<T>, exclusive>, T)`", "moves a resource element in; grows 0 → 4 → 8 → …, which relocates elements"],
    ["`Vec::pop`", "`(ref<Vec<T>, exclusive>) : Option<T>`", "`None` when empty; moves a resource element out"],
    ["`Vec::index_shared`", "`(ref<Vec<T>, shared>, usize) : ref<T, shared>`", "bounds-checked: `diag.index-out-of-bounds`"],
    ["`Vec::index_exclusive`", "`(ref<Vec<T>, exclusive>, usize) : ref<T, exclusive>`", "as above, for writing"],
    ["`v[i]`, `&v[i..j]`", "indexing and slicing (§16)", "`v[i]` reads through `index_shared`, writes through `index_exclusive`; `$` is `Vec::len(&v)`"],
    ["`Vec::clear`", "`(ref<Vec<T>, exclusive>)`", "destroys every element, first to last; keeps the buffer for reuse"],
])
b += code(CHECK_STD + """
fn main()
{
    Vec<i32> v = Vec::new();
    i32 i = 0;
    while (i < 9)                     // crosses two growth boundaries (4 and 8)
    {
        Vec::push(&mut v, i * i);
        i = i + 1;
    }
    check(Vec::len(&v) == 9);
    check(*Vec::index_shared(&v, 8) == 64);
    {
        auto slot = Vec::index_exclusive(&mut v, 0);
        *slot = 100;                  // write through an exclusive element reference
    }                                 // slot must be gone before v is borrowed again
    check(*Vec::index_shared(&v, 0) == 100);
    match (Vec::pop(&mut v))
    {
        Some(x) :
        {
            check(x == 64);
        },
        None :
        {
            check(false);
        },
    }
    Vec<Vec<i32>> nested = Vec::new();
    Vec::push(&mut nested, v);        // v moves into the outer Vec
    check(Vec::len(Vec::index_shared(&nested, 0)) == 8);
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
    check(**Vec::index_shared(&readers, 0) == 10);

    Vec<ref<i32, exclusive>> writers = Vec::new();
    Vec::push(&mut writers, &mut x);
    **Vec::index_exclusive(&mut writers, 0) = 7;   // write through the stored reference
    Vec::pop(&mut writers);                          // the Vec lets go of it
    check(x == 7);
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
b += p("""A `String` is a `Vec<u8>` that is known to be valid UTF-8: owned, heap-allocated text,
a resource. `String::new()` makes an empty one, `String::from_str` copies a `str`'s bytes
(already valid, so nothing is checked), and `String::from_utf8` validates bytes that came from
anywhere else. The only way to change one is `String::append`, which adds valid UTF-8 (below), so
the guarantee holds for its whole life. `from_utf8` is the trust-transition pattern of §20 in
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
    ["`String::push_ascii`", "`(ref<String, exclusive>, u8)`: adds one ASCII byte; one of 128 or more faults `diag.not-ascii`"],
    ["`String::clear`", "`(ref<String, exclusive>)` (empties it, keeping the buffer)"],
])
b += code(CHECK_STD + """
fn main()
{
    String owned = String::from_str("caf\\u{e9}");   // a str's bytes, copied; no validation needed
    check(String::len(&owned) == 5);

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
    check(String::len(&s) == 3);
    Vec<u8> back = String::into_bytes(s);          // and out again
    check(Vec::len(&back) == 3);

    Vec<u8> bad = Vec::new();
    Vec::push(&mut bad, 226);         // a truncated sequence
    Vec::push(&mut bad, 130);
    match (String::from_utf8(bad))
    {
        Ok(_) :
        {
            check(false);
        },
        Err(e) :
        {
            check(e.offset == 0);
        },
    }
}
""", title="Validating bytes into a String", expect="ok", section="s21", slug="string")

b += h3("`StringView`: part of a `String`, without copying")
b += p("""`&s[i..j]` on a `String` gives a `StringView`: a read-only view of the bytes `i` to `j`,
with no copy. Positions are byte positions, `$` is the length, and both ends must fall on a
character boundary (cutting through a multi-byte character is `diag.not-char-boundary`). A view
prints with `%s`, compares with `==` to another view or a `str`, and can be sliced again,
`&v[1..$]`. It borrows its `String` exactly as a slice borrows a `Vec`: while the view lives, the
`String` cannot be changed, and a view cannot outlive it. `String::from_view(v)` copies it into a
`String` of its own when it must.""")
b += table(["Function", "Result"], [
    ["`StringView::len(v)`", "its length in bytes"],
    ["`StringView::find(v, \"x\")`", "`Option<usize>`: where `x` first occurs"],
    ["`StringView::starts_with(v, \"x\")`, `ends_with`", "`bool`"],
    ["`StringView::trim(v)`, `trim_start`, `trim_end`", "the view without ASCII white space at its ends"],
    ["`StringView::split(v, \",\")`", "a `Vec<StringView>` of the parts, each a view of the same `String`"],
    ["`StringView::parse<T>(v)`", "a number, as `parse`"],
    ["`String::from_view(v)`, `String::append(&mut s, v)`", "the text copied into a `String`"],
])
b += code(CHECK_STD + """
// The first word of a line: a view into the line itself.
fn first_word(ref<String, shared> s) : StringView
{
    StringView t = StringView::trim(&(*s)[0..$]);
    match (StringView::find(t, " "))
    {
        Some(i) : &t[0..i],
        None : t,
    }
}

fn main()
{
    String line = String::from_str("  set width=80, height = 24  ");
    check(first_word(&line) == "set");

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
                    Ok(n) : { area *= n; },
                    Err(_) : { check(false); },
                }
            },
            None : { check(false); },
        }
    }
    check(area == 1920);
    printf("%s -> %d\\n", args, area);
}
""", title="Reading fields out of a line", expect="ok", output="width=80, height = 24 -> 1920\n", section="s21", slug="stringview")
b += code(CHECK_STD + """
fn main()
{
    String name = String::from_str("ada");
    StringView v = &name[0..$];
    String::push_ascii(&mut name, b'!');   // ✗ diag.aliasing-conflict (static): v still views name
    check(v == "ada");
}
""", title="The viewed String cannot change", expect="diag.aliasing-conflict", section="s21", slug="stringview-held")

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
    ["Passed to", "`printf`, `==`, `str_len`, `str_byte`", "`printf` (as `&s`), `String::len`, `String::into_bytes`", "`write` and other raw-byte code"],
])
b += rules([
    "Text written in your source code: use `str`. Pass it, store it in structs and `Vec<str>`, and print it freely.",
    "Text built at run time, or text you need to own: use `String`, created explicitly from a `str` with `String::from_str`.",
    "Part of a `String`, to read, compare, split or parse without copying: a `StringView`, `&s[i..j]`.",
    "A function that takes text from anywhere, a literal or a run-time `String`, takes `ref<String, shared>`. A caller holding a literal copies it first (`String k = String::from_str(\"key\"); f(&k);`): the copy is small, and the call site shows it.",
    "Bytes from outside the program (a file, a socket, FFI): they stay `Vec<u8>` until `String::from_utf8` accepts them.",
    "Bytes that are not text (a protocol header, data for `write`): use `b\"…\"`.",
])
b += p("""The two text types never convert into each other silently (§12's no-implicit-conversion
rule). Going from `str` to `String` is an explicit call that makes a copy, and nothing turns a
`String` back into a `str`: a `str` is always a literal's value, so it cannot refer to bytes that
something else owns.""")
b += h3("Printing: `printf`")
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
    ["`%s`", "`str`, `&String`, `bool`", "the text"],
    ["`%v`", "any of the above", "the value as CobaltC writes it (below)"],
    ["`%%`", "nothing", "a `%`"],
])
b += p("""Flags, a width and a precision work as in C: `%-8s` pads on the right, `%05d` pads with
zeros, `%+d` always shows a sign, `%.2f` rounds to two decimals, `%8.3f` does both. Widths count
characters, so text in any language lines up. There is no `%c`.""")
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
        Err(e) :
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
a struct, a `Vec` or an `Option` is a static type error, so printing a composite value means
printing its parts. It takes a width and `-`, but no precision.""")
b += table(["Argument", "Written as", "Examples"], [
    ["`str`", "its bytes", "`printf(\"%v\", \"hi\")` → `hi`"],
    ["any integer type", "decimal, `-` if negative", "`-42`, `18446744073709551615`"],
    ["`f32`, `f64`", "the shortest digits that read back as the same value", "`0.1`, `1.0`, `0.3333333333333333`, `1.0e21`, `1.0e-7`, `NaN`, `inf`"],
    ["`bool`", "`true` or `false`", ""],
    ["`ref<String, shared>`", "the string's bytes", "`printf(\"%v\", &s)`"],
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
a `u8` prints as a number. To write bytes, use the `write` extern the way `std` does, around an
`unsafe` block. `put` writes one byte and `print_bytes` a whole `Vec<u8>`:""")
b += code(CHECK_STD + """
// Write one byte to standard output.
fn put(u8 b)
{
    auto buf = [b];                   // an array<u8, 1>
    rawptr<u8> p = reinterpret_ptr<u8>(rawptr_of(&buf));
    unsafe
    {
        write(p, 1);                  // trusted: p points at one valid byte
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
    check(String::len(&s) == 2);
}
""", title="The bridge is explicit", expect="ok", section="s21", slug="str-to-string")

b += h3("Reading input")
b += p("""`read_line()` reads the next line of standard input. It returns a
`Result<Option<String>, ReadError>`:""")
b += table(["Result", "Meaning"], [
    ["`Ok(Some(line))`", "the next line, without its `\\n`; a last line with no `\\n` counts too"],
    ["`Ok(None)`", "the input has ended (and every later call says so again)"],
    ["`Err(Utf8(e))`", "the line is not valid UTF-8; `e.offset` is the bad byte's position in the line, and the next call reads the line after it"],
    ["`Err(Io)`", "the input could not be read"],
])
b += p("""Only `\\n` ends a line, so a line typed on Windows keeps its `\\r`. The line is a
`String` that has already been checked, so a program that reads never writes `unsafe`, and `?`
passes a `ReadError` up to the caller. Underneath is the `read` extern, the counterpart of
`write`, which you can call yourself inside `unsafe` to read raw bytes.""")
b += p("""Both `coby` and `cobc` read standard input, from the terminal or from a pipe, for example
`printf 'ada\\nlin\\n' | coby greet.cb`. Standard output is flushed before each read, so a prompt
printed with `printf` appears before the program waits.""")
b += code("""
import std;

// Greets every line of input; blank lines are skipped.
fn greet_all() : Result<usize, ReadError>
{
    usize count = 0;
    while (true)
    {
        auto line = read_line()?;     // an error goes back to main
        match (line)
        {
            Some(name) :
            {
                if (String::len(&name) > 0)
                {
                    printf("hello, %v\\n", &name);
                    count = count + 1;
                }
            },
            None :
            {
                return Ok(count);     // the end of input
            },
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
""", title="Reading lines (cobc only)")

b += h3("Program arguments")
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
    usize i = 0;
    while (i < arg_count())
    {
        match (arg(i))
        {
            Ok(a) :
            {
                printf("%v: %v\\n", i, &a);
            },
            Err(e) :
            {
                printf("%v: not UTF-8 at byte %v\\n", i, e.offset);
            },
        }
        i = i + 1;
    }
}
""", title="Listing the arguments: `coby args.cb one two`", expect="ok", output="0: one\n1: two\n",
     section="s21", slug="list-args", args=["one", "two"])

b += h3("Numbers and text")
b += p("""`String::append(&mut s, x)` adds to `s` exactly what `printf("%v", x)` would write, for the same
types: a `str`, any integer, `f32`/`f64`, `bool`, or another `String` through `&`. So a line can be
built piece by piece and printed, or kept, once. `parse<T>(&s)` goes the other way: it reads the
whole `String` as a number of type `T`, any integer type, `f32` or `f64`, and returns a
`Result<T, ParseError>`.""")
b += table(["Result", "Meaning"], [
    ["`Ok(v)`", "the whole text is a number, and it fits in `T`"],
    ["`Err(Empty)`", "the text is empty"],
    ["`Err(Invalid(k))`", "byte `k` cannot continue a number (`\"12x4\"` → 2), or the text ends too early (`\"-\"` → 1)"],
    ["`Err(OutOfRange)`", "a number, but not one `T` can hold (`\"300\"` as `u8`)"],
])
b += p("""Parsing is strict: an optional `+` or `-`, then digits, and for a float an optional fraction and
exponent (`2.5e-3`), or `inf` or `NaN`. No spaces anywhere, so a line typed on Windows ends in a `\\r`
that makes it `Invalid`. Floats are rounded correctly, and every float `%v` writes parses back to
the same value. To take a line apart first, `String::as_bytes(&s)` gives its bytes to read.""")
b += code("""
import std;

// Adds up its arguments: `coby sum.cb 1.5 2 x` reports the `x`.
fn main() : u8
{
    f64 total = 0.0;
    usize i = 0;
    while (i < arg_count())
    {
        String a = match (arg(i))
        {
            Ok(s) : s,
            Err(_) : String::new(),
        };
        match (parse<f64>(&a))
        {
            Ok(v) :
            {
                total = total + v;
            },
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
        i = i + 1;
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
b += p("""`read_file(&path)` reads a whole file as a `String`; `write_file(&path, &text)` makes the
file hold exactly `text`, creating it or replacing what it held. Both take the path as a `String`
(it usually comes from `arg`) and return a `Result` whose error is a `FileError`: `NotFound`,
`Denied`, `Utf8(e)` for a file that is not valid UTF-8 (with the offset of the first bad byte), or
`Io` for anything else. A relative path is relative to the directory the program runs in. Both
tools read and write files.""")
b += table(["Function", "Signature"], [
    ["`read_file`", "`(ref<String, shared> path) : Result<String, FileError>`"],
    ["`write_file`", "`(ref<String, shared> path, ref<String, shared> text) : Result<void, FileError>`"],
])
b += p("""These two handle the whole file at once; `File`, below, reads and writes one in pieces. `ReadError`
and `FileError` both have variants called `Io` and `Utf8`; a `match` names them plainly, since the
value's type says which enum is meant, but building one by hand is written `FileError::Io`.""")
b += code("""
import std;

fn main() : u8
{
    String path = String::from_str("notes.txt");
    String text = String::from_str("first line\\nsecond line\\n");
    match (write_file(&path, &text))
    {
        Ok(_) :
        {
        },
        Err(_) :
        {
            printf("cannot write notes.txt\\n");
            return 1;
        },
    }
    String back = Result::unwrap_or(read_file(&path), String::new());
    printf("%v", &back);
    String gone = String::from_str("no-such-file.txt");
    match (read_file(&gone))
    {
        Ok(_) :
        {
            printf("?\\n");
        },
        Err(e) : match (e)
        {
            NotFound :
            {
                printf("no-such-file.txt: not found\\n");
            },
            _ :
            {
                printf("no-such-file.txt: cannot read it\\n");
            },
        },
    }
    0
}
""", title="Writing a file and reading it back", expect="ok", output="first line\nsecond line\nno-such-file.txt: not found\n",
     section="s21", slug="files")

b += h3("Open files: `File`")
b += p("""A `File` is an open file, for what `read_file` cannot do: bytes that are not text, files
too large to hold, appending to a log. It is a resource, like `Vec`: it has one owner, can move to
another thread, and its destructor closes it. `close` closes it too, and reports whether that
worked (for a written file, whether the bytes were stored); after `close` the `File` is gone,
so it cannot be used again. Every operation returns a `Result` whose error is a `FileError`.""")
b += table(["Function", "Does"], [
    ["`File::open(&path)`", "open for reading; the file must exist"],
    ["`File::create(&path)`", "open for writing, empty: created, or its contents discarded"],
    ["`File::append(&path)`", "open for writing at the end, created if missing"],
    ["`File::open_rw(&path)`", "open for reading and writing, created if missing, contents kept"],
    ["`File::read_line(&mut f)`", "the next line without its `\\n`, `None` at the end (as `read_line`)"],
    ["`File::read(&mut f, &mut buf, max)`", "append up to `max` bytes to a `Vec<u8>`; how many, 0 at the end"],
    ["`File::read_to_end(&mut f, &mut buf)`", "append the rest of the file to a `Vec<u8>`"],
    ["`File::write(&mut f, &bytes[i..j])`, `File::write_str(&mut f, &text)`", "write all of a slice of bytes, or a `String`'s text"],
    ["`File::seek(&mut f, pos)`, `File::len(&f)`", "move to byte `pos` (a `u64`); the length in bytes"],
    ["`File::close(f)`", "close it and say whether that worked"],
    ["`read_bytes(&path)`, `write_bytes(&path, &bytes[i..j])`", "`read_file` and `write_file` for bytes: a `Vec<u8>`"],
])
b += p("""Reading is buffered, so `read_line` is quick; each write reaches the file before it returns,
so gather many small pieces in a `String` or `Vec<u8>` first when speed matters.""")
b += code("""
import std;

fn opened(Result<File, FileError> r) : File
{
    match (r)
    {
        Ok(f) : f,
        Err(_) : fault(assertion_failed),
    }
}

fn main()
{
    String path = String::from_str("app.log");
    File log = opened(File::create(&path));
    usize i = 1;
    while (i <= 3)
    {
        String line = sprintf("event %v\\n", i);
        Result::unwrap_or(File::write_str(&mut log, &line), ());
        i = i + 1;
    }
    Result::unwrap_or(File::close(log), ());

    File input = opened(File::open(&path));
    printf("%v bytes\\n", Result::unwrap_or(File::len(&input), 0));
    while (true)
    {
        match (File::read_line(&mut input))
        {
            Ok(o) : match (o)
            {
                Some(line) :
                {
                    printf("read: %s\\n", &line);
                },
                None :
                {
                    break;
                },
            },
            Err(_) :
            {
                break;
            },
        }
    }
    Result::unwrap_or(File::seek(&mut input, 6), ());
    Vec<u8> bytes = Vec::new();
    Result::unwrap_or(File::read(&mut input, &mut bytes, 1), 0);
    printf("byte 6 is %v\\n", bytes[0]);
}
""", title="Writing a log, reading it line by line, and seeking", expect="ok",
     output="24 bytes\nread: event 1\nread: event 2\nread: event 3\nbyte 6 is 49\n",
     section="s21", slug="file-handle")

b += h3("`HashMap<K, V>` and `HashSet<K>`")
b += p("""`HashMap<K, V>` maps keys to values; `HashSet<K>` holds a set of keys. A key is an integer,
a `bool`, a `str` or a `String`; any other key type (a float, a struct) is a static type error, since
CobaltC has no way to ask an arbitrary type for a hash. The map owns its keys and values. Lookups
take the key by reference, so they copy nothing.""")
b += table(["Function", "Does"], [
    ["`HashMap::new()`, `HashMap::len(&m)`", "an empty map; the number of entries"],
    ["`HashMap::insert(&mut m, k, v)`", "adds `k`; if it was present, replaces its value and returns the old one as `Some`"],
    ["`HashMap::entry(&mut m, k, fresh)`", "the value for `k` as an exclusive reference, adding `k` with `fresh` first if absent"],
    ["`HashMap::get(&m, &k)`, `HashMap::get_mut(&mut m, &k)`", "`Some` reference to the value, or `None`"],
    ["`HashMap::contains(&m, &k)`", "whether `k` is present"],
    ["`HashMap::remove(&mut m, &k)`", "removes `k` and returns its value; fast: the last entry moves into its place"],
    ["`HashMap::remove_ordered(&mut m, &k)`", "the same, keeping every other entry's place; takes time in proportion to the map's size"],
    ["`HashMap::key_at(&m, i)`, `HashMap::value_at(&m, i)`", "entry `i`, for `i < len`, in insertion order"],
    ["`HashSet::new`, `len`, `insert`, `contains`, `remove`, `remove_ordered`, `at`", "the same for a set; `insert` and `remove` say whether anything changed"],
])
b += p("""Entries are kept in the order they were added, so `foreach (k, v in &m)` (§14) visits
them in that order, the same in the interpreter and the compiler, whatever the keys hash to.
`key_at` and `value_at` reach entry `i` directly. `remove` is quick
because it fills the gap with the last entry; use `remove_ordered` when the order matters.""")
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
        None : printf("no cat\\n"),
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
b += code("""
import std;

fn main()
{
    HashMap<f64, str> m = HashMap::new();   // ✗ diag.type-mismatch (static): f64 is not a key type
}
""", title="A float is not a key", expect="diag.type-mismatch", section="s21", slug="hashmap-float-key")

b += h3("`Rc<T>`")
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
    check(Vec::len(Rc::get(&b)) == 1);
    drop(a);                          // count = 1; the box survives
    check(*Vec::index_shared(Rc::get(&b), 0) == 42);
    drop(b);                          // count = 0: the Vec is destroyed, then the box is freed
}
""", title="Reference counting", expect="ok", section="s21", slug="rc")

b += h3("`Box<T>`")
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
    check(*Box::get(&counter) == 42);
    i32 n = Box::into_inner(counter);  // the value back; the Box's memory is freed
    check(n == 42);
}                                     // list: both nodes destroyed, the outer one first
""", title="A list that owns its nodes", expect="ok", section="s21", slug="box")
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
b += p("""A value cannot be moved out of something you hold only by reference, since that would leave
a hole. So two values behind references are exchanged by `swap(&mut a, &mut b)`, and one is
exchanged for a new value by `replace(&mut r, v)`, which returns the old one. Two elements of the
same `Vec` or slice are exchanged by `Vec::swap(&mut v, i, j)` or `slice_swap(s, i, j)`, which also
handle `i == j`. Nothing is copied or destroyed: each value just changes place.""")
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
    check(String::len(&words[0]) == 3 && String::len(&words[$ - 1]) == 6);

    Pair p = Pair { .left = String::from_str("L"), .right = String::from_str("RR") };
    swap(&mut p.left, &mut p.right);
    String old = replace(&mut p.left, String::from_str("new"));   // old is "RR"
    check(String::len(&old) == 2 && String::len(&p.left) == 3);
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
            None : { break; },
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
            None : { break; },
        }
    }
    check(sum == 3);
}
""", title="Filling an empty link", expect="ok", section="s21", slug="replace-fill")

b += rules([
    "`Vec<T>` for sequences, `Option<T>` for maybe-absent values, `Result<T, E>` for fallible calls, `Box<T>` for one value on the heap (a recursive type's link), `Rc<T>` when two owners are genuinely needed.",
    "Library functions take `&v` or `&mut v`; match the mode to what you need.",
    "Hold element references briefly; any `push` or `pop` may invalidate them (§10).",
])
S21 = Section("s21", "§21", "The standard library, std", "21-standard-library-semantics.md",
              "import std; Option, Result, the intrinsics, str and printf, and Vec, String, HashMap, HashSet, Rc and Box written in CobaltC.", b)

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
    "<strong>String literals:</strong> `\"…\"`, type `str`. Escapes `\\n \\r \\t \\0 \\\\ \\\"` and `\\u{hex}`; any other source character stands for itself. A literal must close on its own line. <strong>Byte literals:</strong> `b\"…\"`, type `array<u8, N>`; the same escapes except `\\xNN` in place of `\\u{…}`, ASCII characters only, at least one byte. <strong>Byte character literals:</strong> `b'a'`, type `u8`, one ASCII character or escape. <strong>No character literals</strong> (a `char` type).",
    "<strong>Keywords:</strong> `fn struct enum resource match if else while return auto module import export unsafe extern break continue move mut true false as void`.",
    "<strong>Reserved type names</strong> (cannot be identifiers): `i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 isize usize f32 f64 bool str ref rawptr array handle mutex guard`. The words `shared` and `exclusive` are ordinary identifiers that only have meaning inside `ref<…, …>`.",
    "Whitespace and newlines are insignificant between tokens; brace placement is a matter of style.",
])

b += h3("Types")
b += table(["Type syntax", "Meaning"], [
    ["`i32`, `u8`, `f64`, `bool`, …", "built-in scalars"],
    ["`str`", "text value (§21)"],
    ["`void`", "the unit type (its value is written `()`)"],
    ["`ref<T, shared>`, `ref<T, exclusive>`", "references"],
    ["`rawptr<T>`", "raw pointer"],
    ["`array<T, N>`", "fixed array, `N` a literal"],
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
    ["array literal", "`[1, 2, 3]`"],
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
    "<strong>`Name { f1, f2 } = e;`</strong> (bare identifiers in braces) is destructuring; `Name { .f = e }` is a literal.",
    "<strong>`&mut`</strong> is two tokens, one operator. `x = y = z` assigns right to left.",
])

b += h3("Restrictions and deliberate absences")
b += p("""Things the grammar does not provide, each a considered decision recorded in the
specification:""")
b += ul([
    "no character literals (`b'x'` is a byte, a `u8`), no indexing or slicing of a `str`; no `switch` (a `match` on an integer, §16, is the checked form); no `goto`",
    "no `import … as` renaming; no method-call syntax (`v.push(x)`); no operator overloading",
    "patterns in `match` are `_`, a literal, a variant, and a variant with a pattern for its payload, nothing else (no ranges, alternatives, struct patterns or guards); destructuring takes a struct apart whole, every field at once",
    "no partial initialisation of aggregates",
    "no lifetime annotations: a function returning a reference must have exactly one reference parameter",
    "no bounds on generic type parameters",
    "no `mut` qualifier on bindings (the keyword is reserved for `&mut`)",
    "no moving a resource out of a field of a live struct; move the whole struct",
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
    ["`diag.unbound-name`", "static", "a name resolves to nothing", "declare it, fix the spelling, or `import` it"],
    ["`diag.ambiguous-name`", "static", "two imports or two enums supply the same name", "qualify it"],
    ["`diag.name-not-visible`", "static", "the item is private to another module", "mark it `export` or move the use"],
    ["`diag.module-file-not-found`", "static", "`module m \"…\";` names no readable file", "fix the path (relative to the declaring file) or create the file"],
    ["`diag.module-cycle`", "static", "a file's module body loads itself again, directly or through other files", "move the shared items into a third file both name"],
    ["`diag.module-file-duplicate`", "static", "one file is named by two `module` declarations", "declare it once; reach it by qualified path or `import`"],
    ["`diag.duplicate-item`", "static", "two declarations add the same qualified name", "rename one, or move it into a module"],
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
    ["`diag.overwrite-of-live-resource`", "both", "assigning over a resource that is still owned", "`drop` first"],
    ["`diag.read-of-resource`", "static", "a resource used where a copy would be made (`==`, a value read)", "move it or take a reference"],
    ["`diag.bad-destructor-signature`, `diag.direct-destructor-call`", "static", "`drop` declared wrongly or called by hand", "`fn T::drop(ref<T, exclusive> self)`; use `drop(x)`"],
    ["`diag.arith-overflow`", "both", "`+ - *` or negation left the type's range", "widen, or use `wrapping_`/`saturating_`/`checked_`"],
    ["`diag.div-by-zero`, `diag.div-overflow`", "both", "division by zero; `MIN / -1`", "test first or `checked_div`"],
    ["`diag.shift-amount-out-of-range`", "both", "shift by the bit width or more", "mask the amount"],
    ["`diag.narrowing-overflow`", "both", "`narrow` or `to_int` of a value that does not fit", "`narrow_wrapping`, or widen the target"],
    ["`diag.literal-out-of-range`", "static", "a literal that does not fit its type", "fix the literal or its type"],
    ["`diag.index-out-of-bounds`", "both", "array or `Vec` index, or `str_byte` position, at or past the length", "check the index first"],
    ["`diag.non-exhaustive-match`", "static", "a variant with no arm and no `_`", "add arms"],
    ["`diag.propagate-outside-fallible-context`", "static", "`?` in a function whose return type is not `Result<_, E>` with the same `E`", "change the return type or `map_err`"],
    ["`diag.spawn-borrow-closure`", "static", "`spawn` given a closure that borrows", "add `move`"],
    ["`diag.mutex-reentrant-lock`", "dynamic", "locking a mutex this thread already holds", "release the guard first"],
    ["`diag.trusted-outside-unsafe`", "static", "a raw or extern operation outside `unsafe`", "wrap it in `unsafe` after checking the precondition"],
    ["`diag.extern-non-ffi-type`", "static", "an `extern fn` with a non-FFI parameter or result", "pass scalars or `rawptr`"],
    ["`diag.alloc-failure`", "dynamic", "`Vec` or `Rc` could not allocate", "call `allocate` yourself if you need to recover"],
])
APPX = Section("appx", "A", "Appendix: diagnostics", None,
               "Every named diagnostic, what it means, and the usual fix.", b)
APPX.group = "Appendix"

SECTIONS = [S17, S18, S19, S20, S21, S22, APPX]
