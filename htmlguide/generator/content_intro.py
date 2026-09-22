from gen import Section, p, h3, h4, ul, ol, table, note, rules, code

# ---------------------------------------------------------------- Introduction

intro_body = ""

intro_body += code("""
import std;

fn main()
{
    printf("Hello, CobaltC!\\n");
}
""", title="hello.cb", expect="ok", output="Hello, CobaltC!\n", section="intro", slug="hello")

intro_body += h3("What CobaltC is")
intro_body += p("""CobaltC is a small systems programming language with a C-family surface
syntax. It is designed around one idea: <strong>the facts that correct low-level code depends on
should live in the language, not in the programmer's memory.</strong> When you write C, you have
to remember that a pointer is still valid, that an integer cannot overflow here, that a buffer has
been initialised, that nobody else is writing to that struct right now, and that this handle is
closed exactly once. CobaltC's goal is that every such fact is either represented and checked by
the language, or explicitly marked as trusted by you.""")

intro_body += p("""CobaltC is not a safe language obtained by removing low-level capability. You still get
deterministic control over resources, raw storage, foreign functions, threads, and predictable
data layout. What changes is that each of those capabilities comes with a precise account of what
is checked, when it is checked, and what you are asserting when the language cannot check it.""")

intro_body += h3("The objectives")
intro_body += p("""The language design was derived from a short list of objectives, in priority order.
Where two objectives conflict, the earlier one wins.""")
intro_body += ol([
    "<strong>Preserve semantic invariants.</strong> Arithmetic stays in range, references point at live objects, a resource has exactly one owner, an object is initialised before it is read, two threads never race on one location, and external data is treated as a claim until validated.",
    "<strong>Safety of valid safe programs.</strong> A well-formed program that uses no `unsafe` block cannot violate any of those invariants on any conforming implementation. If the language cannot establish a fact, the operation is one of: statically rejected, dynamically checked, explicitly fallible (returns a `Result` or `Option`), explicitly trusted (inside `unsafe`), or simply not provided.",
    "<strong>Consistency and composability.</strong> A handful of mechanisms that combine cleanly, instead of many overlapping features. Every value-placing construct (declarations, arguments, fields, returns) follows one rule; every access (through a name, a field, an index, a reference) follows one rule.",
    "<strong>Formal precision and deterministic diagnostics.</strong> The specification is written as rules, so two implementations must reject exactly the same programs and report the same named diagnostic. Every error you see has a stable identifier such as `diag.aliasing-conflict`.",
    "<strong>A small language.</strong> Every feature starts as absent and must earn its place. The result is a language you can hold in your head: no traits, no inheritance, no exceptions, no implicit conversions, no operator overloading, no macros.",
    "<strong>Low-level capability.</strong> Raw pointers, manual allocation, FFI, threads, and defined representation are all available, gated by `unsafe` where the language cannot check them.",
])

intro_body += h3("Which features were chosen, and why")
intro_body += p("""Each mechanism in CobaltC exists because a specific invariant needed a home. The
table below is the map from objective to feature; the rest of this guide explains each feature in
turn.""")
intro_body += table(
    ["Invariant to protect", "Mechanism CobaltC provides", "Where to read"],
    [
        ["Arithmetic stays in range",
         "Integer `+ - *` and `/ %` are <em>checked</em>: overflow is a fault, never silent wrap. Wrapping, saturating and `checked_` families are explicit opt-ins. There are no implicit numeric conversions; `widen`, `narrow`, `reinterpret` and friends are named calls.",
         "§06"],
        ["A resource is owned once and released once",
         "<em>Resource types</em> (anything that owns something, marked `resource` or containing a resource) have a single holder. Assigning or passing one <em>moves</em> it. The old name becomes stale. Destruction is automatic at scope end, in reverse order, running the type's `drop` destructor.",
         "§07"],
        ["No conflicting simultaneous access",
         "Two reference modes: `ref<T, shared>` (read-only, any number) and `ref<T, exclusive>` (read and write, alone). Conflicts are checked at every use, so a stale or conflicting reference is always caught. There is no `mut` on variables: mutability is a property of the access path, not the binding.",
         "§08, §09"],
        ["A reference never outlives what it points at",
         "References cannot be returned or stored beyond their referent's scope; the obvious cases are rejected at compile time, the rest at run time. Functions with one reference parameter may return a reference without annotations.",
         "§10"],
        ["No read of uninitialised storage",
         "Definite assignment: a variable must be written on every path before it is read. Aggregates are initialised whole.",
         "§11"],
        ["Types mean what they say",
         "Nominal typing, no subtyping, no coercions. Generics are monomorphic and bound-free. Type inference is local: `auto` infers from the initialiser, and generic arguments come from arguments or the expected type.",
         "§12"],
        ["Evaluation order is defined",
         "Strict left-to-right evaluation everywhere. Blocks, `if` and `match` are expressions.",
         "§13, §14"],
        ["Function interfaces carry the whole contract",
         "A by-value resource parameter takes ownership; a shared reference can only read; an exclusive reference can mutate but not destroy. Closures declare their captures and the language checks the list.",
         "§15"],
        ["Aggregates are whole and exhaustive",
         "Structs, fixed arrays and enums with payloads. Array indexing is bounds-checked. `match` must be exhaustive.",
         "§16"],
        ["Names are unambiguous",
         "Modules with `export`/`import`, private by default. Name resolution is a fixed, documented order.",
         "§17"],
        ["Failure is never silent",
         "Two channels only: a <em>checked fault</em> is fatal and unwinds destructors; recoverable errors are `Result` values with the `?` operator. There is no catch.",
         "§18"],
        ["Threads do not race",
         "`spawn`/`join` with owning handles; a `mutex` whose interior is reachable only through a `lock` guard. The same aliasing rules apply across threads.",
         "§19"],
        ["External data is a claim, not a fact",
         "`unsafe` blocks record what you assert. `rawptr<T>`, `reclaim`, `allocate` and `extern fn` are the only doors to the outside, and FFI signatures are restricted to plain types.",
         "§20"],
        ["Text is a value, not a resource",
         "A string literal `\"…\"` is a `str`: a copyable value like an integer, valid UTF-8 by construction, never owned or freed. `printf` writes one. Owned, growable text is `String`, built from a `str` or validated from bytes.",
         "§21"],
        ["The library proves the language is sufficient",
         "`Vec<T>`, `String` and `Rc<T>` are written in CobaltC itself, using nothing the language does not give you.",
         "§21"],
    ])

intro_body += h3("What CobaltC deliberately leaves out")
intro_body += p("""These are considered decisions, not gaps. Each one either duplicated an existing
mechanism or would have hidden a semantic fact the language wants to keep visible.""")
intro_body += ul([
    "<strong>Implicit conversions and subtyping.</strong> `i32` and `i64` never mix silently.",
    "<strong>A trait or bound system.</strong> A generic body can store, pass and return a `T`, but not add, compare or call it.",
    "<strong>Exceptions.</strong> A fault terminates the program after running destructors; expected failure is a `Result`.",
    "<strong>Garbage collection.</strong> Ownership and scope-end destruction are deterministic; `Rc<T>` is a library type.",
    "<strong>Character literals, string indexing and slicing.</strong> A `str` is a whole value; its bytes are reached with `str_byte`, and a single byte is written `b'a'`, a `u8`. Text that arrives from outside is still bytes until `String::from_utf8` validates it.",
    "<strong>A `mut` qualifier on bindings, `switch` and `goto`, `import … as` renaming, partial initialisation, lifetime annotations, atomics.</strong>",
])

intro_body += h3("How to read this guide")
intro_body += p("""The next section is a complete program you can read top to bottom to absorb the
syntax. After that there is one section per chapter of the normative specification, numbered the
same way (§06 to §22) so you can cross-reference. If you are new, a good order is: the tour, then
§22 (syntax reference), then §06 onward. Each section ends with a short list of rules of thumb.""")
intro_body += p("""Every code block that is a complete program has been run through both implementations,
the reference interpreter `coby` and the native compiler `cobc`, which agree on every one. A green
<em>runs</em> badge means it executes to completion; a red badge names the
diagnostic the language reports for that program. Diagnostics are labelled <em>static</em> (the
program is rejected before it runs) or <em>dynamic</em> (a checked fault at run time); where a check
is static only when the operands are literal, both labels may appear in the text. Comments inside
examples use &#10003; for accepted lines and &#10007; for rejected ones.""")
intro_body += h3("Running a program")
intro_body += p("""Save a program as a `.cb` file. `coby program.cb` interprets it; `cobc --run
program.cb` compiles it to native code and runs it, and `cobc -o program program.cb` keeps the
executable. Both take the program's entry file (it may name other files as modules, §17), both exit
with status 0 when the program runs to completion, and both print a rejected program's diagnostic
the same way and exit with status 1. The repository's README says how to build them.""")
intro_body += p("""All code in this guide uses Allman brace style (every opening brace on its own
line) and four-space indentation. That is a documentation convention: CobaltC ignores whitespace
between tokens, so any brace placement is equally valid.""")

# ---------------------------------------------------------------- Tour

TOUR = r'''
// ------------------------------------------------------------------
// A first CobaltC program: a temperature log.
//
// One page that touches most of the language: a module, structs, an
// enum with payloads and `match`, a resource with a destructor,
// Vec and foreach, references, Result and `?`, generics, a closure, and a thread.
// ------------------------------------------------------------------

import std;

// A module. Its `export` items are reached as `limits::COLD`; the rest
// stay inside. A `const` is a named value fixed when the program is compiled.
module limits
{
    export const i32 COLD = 150;                          // tenths of a degree
    export const i32 HOT = 300;
}

// A plain struct: copied on assignment, comparable with ==.
struct Reading
{
    i32 tenths;                                           // temperature in tenths of a degree
    u8 sensor;
}

// An enum. Variants may carry a payload.
enum Verdict
{
    Cold(i32),
    Fine,
    Hot(i32)
}

// `ref<Reading, shared>` is a read-only reference. `if` is an expression,
// so the function's value is whichever branch runs.
fn judge(ref<Reading, shared> r) : Verdict
{
    if (r.tenths < limits::COLD)
    {
        Cold(limits::COLD - r.tenths)
    }
    else if (r.tenths > limits::HOT)
    {
        Hot(r.tenths - limits::HOT)
    }
    else
    {
        Fine
    }
}

// A resource: something that owns other things. It is moved, never
// copied, and destroyed exactly once, automatically, at scope end.
resource struct Log
{
    Vec<Reading> entries;
}

// The destructor. It is never called directly; the language runs it.
fn Log::drop(ref<Log, exclusive> self)
{
    printf("L%d\n", Vec::len(&self.entries));             // printf's format is checked when compiled
    // self.entries (a Vec, itself a resource) is destroyed after this body.
}

// A fallible function returns a Result. Err carries the offending byte.
fn digit(u8 b) : Result<i32, u8>
{
    if (b > b'9' || b < b'0')
    {
        return Err(b);
    }
    Ok(widen<i32>(b - b'0'))                              // widening is always safe, still spelled out
}

// `?` unwraps an Ok, or returns the Err from this function at once.
fn parse_two(u8 hi, u8 lo) : Result<i32, u8>
{
    i32 h = digit(hi)?;
    i32 l = digit(lo)?;
    Ok(h * 10 + l)
}

// A generic function. T is inferred from the arguments at each call.
fn first_of<T>(T a, T b) : T
{
    a
}

// A function to run on another thread.
fn work(i32 n) : i32
{
    n * 2
}

fn main()
{
    Log log = Log { .entries = Vec::new() };              // Vec::new's T comes from the field type
    Vec::push(&mut log.entries, Reading { .tenths = 120, .sensor = 1 });
    Vec::push(&mut log.entries, Reading { .tenths = 215, .sensor = 1 });
    Vec::push(&mut log.entries, Reading { .tenths = 340, .sensor = 2 });

    // Loop over the entries through a shared reference to each one.
    foreach (r in &log.entries)                           // r : ref<Reading, shared>
    {
        match (judge(r))
        {
            Cold(by) :            // "C" then the shortfall
            {
                printf("C%d", by);
            },
            Fine :
            {
                printf("F");
            },
            Hot(by) :             // "H" then the excess
            {
                printf("H%d", by);
            },
        }
        printf("\n");
    }

    // Result handling with match. `b"42"` is a byte-array literal:
    // the two u8 values 52 and 50.
    auto digits = b"42";
    match (parse_two(digits[0], digits[1]))
    {
        Ok(v) :
        {
            printf("%d", v);
        },
        Err(_) :
        {
            printf("?");
        },
    }
    printf("\n");

    // A closure. The capture list names every outer variable the body
    // uses; `scale` is only read, so it is captured by shared reference.
    i32 scale = 3;
    auto scaled = [scale](i32 x)
    {
        x * scale
    };
    printf("%d\n", scaled(4));

    printf("%d\n", first_of(9, 8));                      // T := i32

    // A thread: spawn takes a function and its arguments; join waits
    // and returns the result. The handle is a resource too.
    auto h = spawn(work, 21);
    printf("%d\n", join(h));
}   // `log` is destroyed here: its destructor prints "L3", then the Vec frees its buffer.
'''

tour_body = ""
tour_body += p("""Here is a complete program. Read it once for the shape of the syntax; every
construct it uses has its own section later. It runs under both the interpreter and the compiler
and prints the output shown below the code.""")
tour_body += code(TOUR, title="tour.cb", expect="ok",
                  output="C30\nF\nH40\n42\n12\n9\n42\nL3\n",
                  section="tour", slug="tour")
tour_body += h3("What you just saw")
tour_body += ul([
    "<strong>Declarations are type-first</strong>, like C: `i32 x = 5;`, `usize i = 0;`, `Vec<Reading> entries;`. `auto` infers the type from the initialiser. Function return types follow a colon: `fn work(i32 n) : i32`.",
    "<strong>Blocks, `if` and `match` are expressions.</strong> The last expression in a block, with no semicolon, is the block's value. `judge` returns whatever branch of the `if` runs.",
    "<strong>Conditions need parentheses</strong> (`if (c)`, `while (c)`, `match (e)`), and `match` arms are `Pattern : expression,` with a comma after each arm.",
    "<strong>`&x` is a shared (read-only) reference and `&mut x` an exclusive one.</strong> `*r` reads or writes through a reference; `r.field` works directly on a reference to a struct.",
    "<strong>`Log` is a `resource`</strong>: it owns a `Vec`. It has a destructor `fn Log::drop`, which the language runs at the end of `main`. Passing or assigning a resource moves it.",
    "<strong>No implicit conversions.</strong> `widen<i32>(b - b'0')` says exactly what happens to the representation; its opposite, `narrow`, is checked at run time. `b'0'` is the byte 48.",
    "<strong>`foreach (r in &log.entries)`</strong> visits each element through a shared reference; `in c` would take the elements and `in &mut c` change them in place.",
    "<strong>Library calls are qualified</strong>: `Vec::push(&mut v, x)`, `Vec::len(&v)`, `Vec::index_shared(&v, i)`. There is no method-call syntax.",
    "<strong>Text is a `str` value; `printf` writes it.</strong> `\"F\"` is a `str` literal, a copyable value with no owner. `printf` is C's, but its format is checked against its arguments when the program is compiled; `b\"42\"` is a byte-array literal, `array<u8, 2>`.",
    "<strong>Modules and constants.</strong> `limits` exports two `const` values, used as `limits::COLD` and `limits::HOT`.",
])
tour_body += p("""Further showcases can be found in the repository's
<a href="https://github.com/strawberry9/CobaltC/tree/main/showcase">showcase directory</a>; its
<a href="https://github.com/strawberry9/CobaltC/blob/main/showcase/README.md">README</a> describes
them all.""")

SECTIONS = [
    Section("intro", "", "Introduction", None,
            "Why CobaltC exists, what it promises, and which features were chosen to keep those promises.",
            intro_body),
    Section("tour", "", "A tour in one program", None,
            "A complete, runnable program that introduces the syntax before the reference sections begin.",
            tour_body),
]
SECTIONS[0].group = "Start here"
