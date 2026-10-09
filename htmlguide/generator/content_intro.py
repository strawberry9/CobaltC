from gen import Section, p, h3, h4, ul, ol, table, note, rules, code

CHECK_STD = "\nimport std;\n"

# ---------------------------------------------------------------- Introduction

intro_body = ""

intro_body += code("""
import std;

fn main()
{
    printf("hello, world\\n");
}
""", title="hello.cb", expect="ok", output="hello, world\n", section="intro", slug="hello")

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

intro_body += h3("The two implementations: `coby` and `cobc`")
intro_body += p("""CobaltC ships with two implementations, both written in Rust, and they share one
front end — the same parser, name resolution and static checks — so they accept and reject exactly
the same programs, with the same diagnostics.""")
intro_body += p("""<strong>`coby`</strong> is the reference interpreter. It runs a program straight
from its source on the abstract machine the specification defines, performing every check the
specification requires on every operation. It is the specification's meaning made executable:
when this guide says a program prints something or stops with a diagnostic, that is what `coby`
does. There is no build step, so it is the quickest way to run a program or try an idea.""")
intro_body += p("""<strong>`cobc`</strong> is the native compiler. After the shared front end has
checked the program, it translates it to plain C and compiles that with the system's C compiler
(GCC or Clang) against `cbrt`, its run-time library. A compiled program keeps every run-time check
the specification requires <em>except</em> those the compiler proves can never fail — the rest
becomes ordinary C — so it runs many times faster than the interpreter while behaving identically.
`cobc --keep-c` leaves the generated C beside the executable if you want to read what your program
became; <a href="#intro-generated-c">Reading the C that `cobc` generates</a>, below, says what is in
it and what you can do with it.""")
intro_body += p("""The two are kept honest against each other: the test suites require `cobc`'s
executables to match `coby`'s output and diagnostics byte for byte on every program, under both
GCC and Clang — including every complete example in this guide. Use `coby` to run a program at
once; use `cobc` to build an executable, or to see how fast the program can be.""")
intro_body += p("""On Linux the two work side by side, and everything in this guide runs under
either. On Windows, `coby` runs programs today, and the formatter `cobfmt` (below) runs there too;
`cobc` does not yet follow them there.""")
intro_body += note("""CobaltC's interpreter (`coby`) and compiler (`cobc`) are available
<a href="https://github.com/strawberry9/CobaltC">here</a>; the repository's README says how to
build them.""", title="Toolchain")

intro_body += h3("The objectives")
intro_body += p("""The language design was derived from a short list of objectives, in priority order.
Where two objectives conflict, the earlier one wins.""")
intro_body += ol([
    "<strong>Preserve semantic invariants.</strong> Arithmetic stays in range, references point at live objects, a resource has exactly one owner, an object is initialised before it is read, two threads never race on one location, and external data is treated as a claim until validated.",
    "<strong>Safety of valid safe programs.</strong> A well-formed program that writes no `unsafe` block of its own cannot violate any of those invariants on any conforming implementation. The standard library's own bodies do use `unsafe` (a `Vec` writes its elements through a raw pointer); those blocks are the implementation's to keep true, each listed in the specification with the fact that justifies it, so they cost the program nothing (§20, <em>Who asserts what</em>). If the language cannot establish a fact, the operation is one of: statically rejected, dynamically checked, explicitly fallible (returns a `Result` or `Option`), explicitly trusted (inside `unsafe`), or simply not provided.",
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
         "Nominal typing, no subtyping, no coercions. Generics are monomorphic: each use with concrete types becomes its own copy of the function, so there is no run-time type information; a type parameter's operations come from one of five built-in bounds. Type inference is local: `auto` infers from the initialiser, and generic arguments come from arguments or the expected type.",
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
         "`spawn`/`join` with owning handles; a `mutex` whose interior is reachable only through a `lock` guard. The same aliasing rules apply across threads. A fault in a spawned thread ends that thread alone and reaches its supervisor as a value; any wait can be cancelled; deadlocks are reported.",
         "§19"],
        ["External data is a claim, not a fact",
         "`unsafe` blocks record what you assert. `rawptr<T>`, `reclaim`, `allocate` and `unsafe extern fn` are the only doors to the outside, and FFI signatures are restricted to plain types.",
         "§20"],
        ["A text literal is a value; owned text is a resource",
         "A string literal `\"…\"` is a `str`: a copyable value like an integer, valid UTF-8 by construction, never owned or freed. `printf` writes one. Owned, growable text is `String`, a resource: moved, destroyed at scope end, built from a `str` (a literal written where a `String` is declared becomes one there) or validated from bytes.",
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
    "<strong>User-defined traits or bounds.</strong> A type parameter's operations come from five built-in bounds — `eq`, `ordered`, `number`, `integer`, `clone` (§12) — and nothing else: there is no way to declare a new bound. Your own struct or enum satisfies `clone` alone, by its fields or its own `T::clone`; to compare or combine values of your own types in generic code, pass the operation in as a function value.",
    "<strong>Exceptions.</strong> A fault terminates the program after running destructors; expected failure is a `Result`.",
    "<strong>Garbage collection.</strong> Ownership and scope-end destruction are deterministic; `Rc<T>` is a library type.",
    "<strong>Character literals, and indexing or slicing a `str`.</strong> A `str` is a whole value; its bytes are reached with `str_byte`, and a single byte is written `b'a'`: a `u8`, or the wider unsigned type its context asks for (§06). A `String` is sliced into views, `&s[i..j]`, and taken apart by `String::split`, `find` and `chars` (§21). Text that arrives from outside is still bytes until `String::from_utf8` validates it.",
    "<strong>Method-call syntax.</strong> A call names the function's owner and writes the borrow it takes: `Vec::push(&mut v, x)`, never `v.push(x)`. The mode of every access path is visible where the path is formed, and `.` means one thing, field access. This is final (`CHG-0132`): no receiver sugar, no uniform call syntax, no auto-borrowing dot, no chaining.",
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
the same way and exit with status 1. `coby --check program.cb` (or `cobc --check`) runs only
the checks made before a program starts, and prints the same diagnostic for a program they
reject; nothing runs, so a program that reads input, writes files or serves the network can be
checked without doing so. Given a directory, it checks every `.cb` file in it.""")
intro_body += p("""All code in this guide uses Allman brace style (every opening brace on its own
line) and four-space indentation. That is a documentation convention: CobaltC ignores whitespace
between tokens, so any brace placement is equally valid.""")
intro_body += p("""`cobfmt`, the formatter, writes that style for you: `cobfmt program.cb` rewrites the
file in place, `cobfmt --backup src/` does every `.cb` file under `src/` and keeps each changed
file's previous contents as `NAME.cb.format.bak`, and `cobfmt --check src/` only lists the files it
would change (for a test or a commit hook). It changes layout and nothing else: indentation, braces,
spacing, blank lines, the `:` of a `match`'s arms and of a `bitstruct`'s fields aligned in one column, and trailing comments kept
in theirs. Before writing, it checks that the result is the same program token for token, so
formatting can never change what a program does. It keeps your line breaks — a short
`if (n == 0) { return; }` stays on one line, though a function's body always opens on its own line — and a line longer than 120 characters
that holds a list of arguments, parameters or array elements is broken one item per line. `--width N`
sets another width, and `--no-wrap` leaves every line as long as it is. `cobfmt --help` lists the rest. Like `coby`, `cobfmt` runs on Windows as well as
Linux, and it keeps a file's Windows line endings (`\\r\\n`) as they are.""")

intro_body += h3("Reading the C that `cobc` generates", anchor="intro-generated-c")
intro_body += p("""`cobc` does not produce machine code itself. Once the shared front end has checked
your program, `cobc` translates it into one C file and hands that file to GCC or Clang, which
builds the executable against `libcbrt.a`, the run-time library. Normally the C file is deleted
afterwards; `cobc --keep-c -o prog prog.cb` keeps it as `prog.c`, beside `prog` (without `-o`,
`a.out` gets `a.c`). Under `--run` without `-o` it lives in a temporary directory and is deleted
with it. If the C compiler ever rejects the file, `cobc` keeps it whatever you asked and prints
its path.""")
intro_body += p("""The file is the complete translation of your program, not an excerpt: nothing but
`libcbrt.a` is needed to build it. Even a seven-line program comes to a few hundred lines,
because the file carries everything it uses, in this order:""")
intro_body += ul([
    "<strong>The run-time interface</strong>, `cbrt.h`, copied in at the top: the C declarations of every `cb_…` function the program calls.",
    "<strong>`/* ---- types ---- */`</strong>: each struct, enum and generic instantiation as a C struct, each followed by a `_Static_assert` that its size and alignment are the ones the specification fixes. `Vec<i32>` becomes `struct s_std__Vec_Li32_R`.",
    "<strong>`/* ---- string literals ---- */`</strong>: your text literals as byte arrays.",
    "<strong>`/* ---- functions ---- */`</strong>: one C function for each function the program actually uses, `std`'s included, and one copy per instantiation of a generic one (`f_std__Vec__push_Li32_R` is `Vec::push` for `i32`). Your `main` is `f_main`; every other function is `static`, so the C compiler may inline it where it is called.",
    "<strong>Call thunks and type descriptors</strong>: what `fn` values and the run-time checks need to know about each type.",
    "<strong>`int main`</strong>, last: it hands the program's arguments to the runtime and starts `f_main`.",
])
intro_body += p("""Inside a function, the C follows your statements one by one, and every run-time
check the specification requires is an explicit line, so you can see exactly which checks your
code still pays for. This is `printf("%v\\n", v[0]);` with `v` a `Vec<i32>`:""")
intro_body += code("""
cb_at(0u, 7u);                                   // now at line 7 of file 0, for diagnostics
struct s_std__Vec_Li32_R *v9 = &(l_v_1);
if (t8 >= v9->len) cb_fault("diag.index-out-of-bounds", CB_NOLOC, 0);
int32_t *e10 = v9->ptr + (int64_t)t8;            // the element's address
int32_t t11 = (*e10);
""", title="from prog.c")
intro_body += p("""Elsewhere you will meet `cb_borrow`, `cb_read` and `cb_write` (the access and
aliasing checks of §08–§09), `__builtin_add_overflow` and its relatives (checked arithmetic, §06),
and `cb_frame_push`, `cb_stmt_push` and their `_pop` partners (the bookkeeping that runs
destructors at scope end). A check the compiler has proved can never fail is simply absent.""")
intro_body += h4("What you can do with it")
intro_body += ul([
    "<strong>Read it</strong> to see how a construct is compiled, which checks a loop costs, or why one way of writing it is faster than another. <a href=\"#perf\">Performance</a> (chapter P) collects what such reading and measuring found.",
    "<strong>Build it yourself, with your own flags.</strong> `cobc`'s own build of `prog.c` is the command below, and `cobc -v` prints the exact one it runs, with the real path of `libcbrt.a` (and any `extern` C file's compilation), ready to paste and change; `-O2` is the default level and `libcbrt.a` sits beside the `cobc` executable. Add `-g -O0` for a build to step through in a debugger, `-fsanitize=address,undefined` to have the sanitizers watch it, `-march=native` or `-flto`, or leave out `-w` to see the C compiler's warnings, which `cobc` silences.",
    "<strong>Debug and profile it</strong> at the C level: in a `-g` build, `gdb` and `perf annotate` show these C lines, and the `cb_at(file, line)` calls lead back to the line of your `.cb` file.",
    "<strong>Report a compiler bug.</strong> If GCC or Clang rejects the generated C, that kept file and the C compiler's message are the whole report.",
])
intro_body += code("""
gcc -std=gnu11 -O2 -ffp-contract=off -fno-optimize-sibling-calls -w prog.c \\
    -L /path/to/cobc/dir -lcbrt -lpthread -ldl -lm \\
    -Wl,--gc-sections -Wl,--strip-debug -o prog
""", title="building prog.c by hand (Linux; clang takes the same flags)")
intro_body += p("""Keep `-std=gnu11` and the two `-f` flags when you change the rest; each is part of
the program's meaning. `-ffp-contract=off` stops the C compiler fusing `a * b + c` into one
fused multiply-add with a single rounding, since the specification rounds each operation on its
own (§06). `-fno-optimize-sibling-calls` keeps every call a real call, so a recursion too deep for
the stack stops with the same diagnostic it gets from `coby`, instead of being turned into a loop
that runs to completion. A program that links its own C code (`extern "./mine.c";`, §20) also
needs that code compiled and added before `-lcbrt`; `cobc -v` shows both commands. The last two
options only make the executable smaller: they leave out the parts of the runtime the program does
not use, and the runtime's debug information (a hello world is about 0.8 MB with them, 5.9 MB
without); drop them if you link a `-g` build to debug. `cobc -s` also leaves out the symbol table
(`--strip-all` in place of `--strip-debug`, about 0.6 MB), which only profilers and debuggers read.""")
intro_body += note("""The generated C is for reading and experimenting, not for keeping or editing by
hand. Its names are internal and change between versions of `cobc`, and it is only correct with the
`libcbrt.a` built alongside the same `cobc`. `cobc` refuses a mismatched runtime, but a build by hand
skips that check.""", kind="warn")
intro_body += h4("Why compile to C")
intro_body += p("""Compiling through C is a long tradition rather than a shortcut: C++ began as
Cfront, which translated it into C, and Nim, Vala, Cython and several Scheme compilers work this
way today. Few compilers write machine code entirely on their own; most hand the last step to a
shared back end such as LLVM or GCC's. C as the target brings:""")
intro_body += ul([
    "<strong>Portability for free.</strong> Wherever there is a C compiler, the language can run.",
    "<strong>Decades of optimisation for free.</strong> GCC's and Clang's `-O2` do register allocation, inlining, vectorisation and instruction scheduling better than a young compiler could.",
    "<strong>Readable output.</strong> You can see what your program became, which is what `--keep-c` is for; LLVM IR or assembly would be far harder to read.",
    "<strong>Easy C interoperation.</strong> Calling C functions and linking C libraries is natural, which is what `unsafe extern fn` (§20) needs.",
])
intro_body += h4("What it costs")
intro_body += ul([
    "<strong>C's own rules come along.</strong> The generated C must avoid C's undefined behaviour, and it has to keep up as C compilers grow stricter: code an older compiler accepted with a warning, a newer one may reject outright.",
    "<strong>Less control.</strong> Some things C cannot say directly (exact stack layout, particular calling conventions, precise debug information), so they are worked around with flags such as `-ffp-contract=off` above.",
    "<strong>Slower builds.</strong> Every build includes a full C compilation.",
    "<strong>Debuggers show C lines, not CobaltC lines.</strong> `cobc` emits no `#line` directives, so `gdb` steps through the C; the `cb_at(file, line)` calls lead back to your source.",
])
intro_body += p("""For CobaltC the trade is a good one. Its central promise is that `coby` and `cobc`
behave identically, and letting GCC and Clang do the machine-code work leaves `cobc` free to get
the semantics and the checks exactly right.""")

# ---------------------------------------------------------------- Tour

TOUR = r'''
// ------------------------------------------------------------------
// A first CobaltC program: a temperature log.
//
// One page that touches most of what CobaltC is for: structs, an enum
// with payloads and `match`, a resource with a destructor, Vec, String
// and HashMap, references the compiler checks, slices, Option, Result
// and `?`, checked arithmetic, generics, a closure, and two threads
// sharing a counter through a mutex.
// ------------------------------------------------------------------

import std;

// A module. Its `export` items are reached as `limits::COLD`; the rest
// stay inside. A `const` is a value fixed when the program is compiled.
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
    HashMap<String, u32> per_sensor;                      // readings counted by sensor name
}

// The destructor. It is never called directly; the language runs it.
fn Log::drop(ref<Log, exclusive> self)
{
    printf("L%d\n", Vec::len(&self.entries));             // printf's format is checked when compiled
}

fn record(ref<Log, exclusive> log, i32 tenths, u8 sensor)
{
    Vec::push(&mut log.entries, Reading { .tenths = tenths, .sensor = sensor });
    *HashMap::entry(&mut log.per_sensor, sprintf("sensor-%d", sensor), 0) += 1;
}

// A slice is a borrowed run of elements. Arithmetic never wraps
// silently: `checked_add` gives None on overflow, and `?` on an Option
// returns that None from this function at once.
fn average(slice<Reading, shared> rs) : Option<i32>
{
    if (slice_len(rs) == 0)
    {
        return None;
    }
    i32 total = 0;
    foreach (r in rs)
    {
        total = checked_add(total, r.tenths)?;
    }
    Some(total / narrow<i32>(slice_len(rs)))              // `narrow` checks that the value fits
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

// A generic function: T is any integer type, fixed at each call.
fn larger<T: integer>(T a, T b) : T
{
    if (a > b) { a } else { b }
}

// Runs on its own thread: counts the hot readings in its half of the
// log into a counter the two threads share. Only the lock reaches it.
fn count_hot(ref<mutex<u32>, shared> hot, slice<Reading, shared> part) : usize
{
    foreach (r in part)
    {
        if (r.tenths > limits::HOT)
        {
            *lock(hot) += 1;
        }
    }
    slice_len(part)
}

fn main()
{
    Log log = Log { .entries = Vec::new(), .per_sensor = HashMap::new() };
    record(&mut log, 120, 1);
    record(&mut log, 215, 1);
    record(&mut log, 340, 2);
    record(&mut log, 310, 2);

    // Loop over the entries through a shared reference to each one.
    // `match` takes the Verdict apart; it must handle every variant.
    foreach (r in &log.entries)                           // r : ref<Reading, shared>
    {
        match (judge(r))
        {
            Cold(by) : printf("C%d ", by),                // "C" then the shortfall
            Fine     : printf("F "),
            Hot(by)  : printf("H%d ", by),                // "H" then the excess
        }
    }
    printf("\n");

    // A lookup gives an Option; a pattern condition takes it apart.
    if (Some(n) = HashMap::get(&log.per_sensor, &String::from_str("sensor-2")))
    {
        printf("sensor-2: %d\n", *n);
    }

    // A slice of the Vec, and an average that cannot overflow unseen.
    if (Some(avg) = average(&log.entries[0 .. 2]))
    {
        printf("avg %d\n", avg);
    }

    // Sorting, and a closure: `to_f` captures nothing, `offset` is read.
    Vec<i32> temps = Vec::new();
    foreach (r in &log.entries)
    {
        Vec::push(&mut temps, r.tenths);
    }
    Vec::sort(&mut temps);
    i32 offset = 32;
    auto to_f = [offset](i32 tenths)
    {
        tenths * 9 / 50 + offset
    };
    printf("coldest %dF, warmest %dF\n", to_f(temps[0]), to_f(temps[3]));

    // Result handling: `b"42"` is a byte-array literal (52 and 50).
    auto digits = b"42";
    if (Ok(v) = parse_two(digits[0], digits[1]))
    {
        printf("%d %d\n", v, larger(v, 17));             // T := i32
    }

    // Two threads share the counter through a mutex; each gets its own
    // half of the log as a slice. `join` waits and gives the result.
    auto hot = Mutex::new(0: u32);
    auto h1 = spawn(count_hot, &hot, &log.entries[0 .. 2]);
    auto h2 = spawn(count_hot, &hot, &log.entries[2 .. 4]);
    usize seen = join(h1) + join(h2);
    printf("%d of %d readings hot\n", *lock(&hot), seen);
}   // `log` is destroyed here: its destructor prints "L4", then its Vec and HashMap are freed.
'''

tour_body = ""
tour_body += p("""Here is a complete program. Read it once for the shape of the syntax; every
construct it uses has its own section later. It runs under both the interpreter and the compiler
and prints the output shown below the code.""")
tour_body += code(TOUR, title="tour.cb", expect="ok",
                  output="C30 F H40 H10 \nsensor-2: 2\navg 167\ncoldest 53F, warmest 93F\n42 42\n2 of 4 readings hot\nL4\n",
                  section="tour", slug="tour")
tour_body += h3("What the compiler stops")
tour_body += p("""The program above is correct, so it shows what CobaltC lets you write. Just as much of
the language is what it will not let through. Each of these mistakes, a one-line change to the
same kind of code, is rejected <em>before the program runs</em>, by both the interpreter and the
compiler, with a diagnostic that names the rule; what cannot be decided in advance is checked
while the program runs, and stops it at the line where it happens (§08–§10).""")
tour_body += code(CHECK_STD + """
struct Reading
{
    i32 tenths;
    u8 sensor;
}

fn main()
{
    Vec<Reading> entries = Vec::new();
    Vec::push(&mut entries, Reading { .tenths = 120, .sensor = 1 });
    ref<Reading, shared> first = Vec::index_shared(&entries, 0);
    Vec::push(&mut entries, Reading { .tenths = 215, .sensor = 1 });   // ✗ the push may move the elements
    printf("%d\\n", first.tenths);                                     //   while `first` still points into them
}
""", title="A reference kept across a push", expect="diag.aliasing-conflict", section="tour", slug="tour-stop-ref")
tour_body += code(CHECK_STD + """
resource struct Log
{
    Vec<i32> entries;
}

fn archive(Log l)                     // takes the log: it is destroyed when this call ends
{
}

fn main()
{
    Log log = Log { .entries = Vec::new() };
    archive(log);                     // `log` moves into the call
    Vec::push(&mut log.entries, 7);   // ✗ `log` no longer owns anything
}
""", title="A value used after it moved", expect="diag.stale-binding", section="tour", slug="tour-stop-move")
tour_body += code(CHECK_STD + """
fn digit(u8 b) : Result<i32, u8>
{
    if (b > b'9' || b < b'0')
    {
        return Err(b);
    }
    Ok(widen<i32>(b - b'0'))
}

fn main()
{
    digit(b'x');                      // ✗ an error dropped on the floor would go unseen
    // Handle it (`?`, `match`, `if (Err(e) = …)`), or say you mean to drop
    // it: `_ = digit(b'x');` discards the value on purpose.
}
""", title="An error ignored", expect="diag.result-discarded", section="tour", slug="tour-stop-result")
tour_body += code(CHECK_STD + """
fn main()
{
    i32 tenths = 2000000000;
    i32 twice = tenths * 2;           // ✗ the product does not fit in an i32: no silent wrap
    printf("%d\\n", twice);
}
""", title="Arithmetic that would wrap", expect="diag.arith-overflow", section="tour", slug="tour-stop-overflow")
tour_body += code("""
enum Verdict
{
    Cold(i32),
    Fine,
    Hot(i32)
}

fn label(Verdict v) : i32
{
    match (v)                         // ✗ `Hot` is not handled: every variant must be
    {
        Cold(by) : -by,
        Fine     : 0,
    }
}

fn main()
{
    label(Fine);
}
""", title="A case left out", expect="diag.non-exhaustive-match", section="tour", slug="tour-stop-match")
tour_body += h3("What you just saw")
tour_body += ul([
    "<strong>Declarations are type-first</strong>, like C: `i32 x = 5;`, `Vec<Reading> entries;`. `auto` infers the type from the initialiser. Function return types follow a colon: `fn judge(…) : Verdict`.",
    "<strong>Blocks, `if` and `match` are expressions.</strong> The last expression in a block, with no semicolon, is the block's value. `judge` returns whatever branch of the `if` runs.",
    "<strong>Conditions need parentheses</strong> (`if (c)`, `while (c)`, `match (e)`); `match` arms are `Pattern : expression,`, and a `match` must cover every variant (a missing one is rejected, above). A pattern can also be a condition: `if (Some(n) = HashMap::get(…))` runs its block only when the lookup found something, with `n` bound.",
    "<strong>`&x` is a shared (read-only) reference and `&mut x` an exclusive one.</strong> The compiler checks every use: a reference kept into the `Vec` across a `push`, a value used after it moved, are rejected before the program runs (above). `&log.entries[0 .. 2]` is a <em>slice</em>, a borrowed run of elements.",
    "<strong>`Log` is a `resource`</strong>: it owns a `Vec` and a `HashMap`. Its destructor `fn Log::drop` runs at the end of `main`, then its fields are freed. Passing or assigning a resource moves it.",
    "<strong>`String`, `HashMap` and `sprintf`.</strong> `sprintf(\"sensor-%d\", s)` builds a `String`; `*HashMap::entry(&mut m, key, 0) += 1` counts. The standard library lives in the module `std`, imported by `import std;`.",
    "<strong>Errors are values.</strong> A `Result` must be used (dropping one is rejected), `?` returns an `Err` or a `None` early, and `checked_add` turns an overflow into `None`. Plain arithmetic that overflows is never silent: it is rejected when the compiler can see it, and stops the program when it happens.",
    "<strong>No implicit conversions.</strong> `widen<i32>(b - b'0')` and `narrow<i32>(n)` say what happens to the representation; `narrow` checks that the value fits.",
    "<strong>Library calls are qualified</strong>: `Vec::push(&mut v, x)`, `Vec::sort(&mut v)`, `HashMap::get(&m, &k)`. There is no method-call syntax.",
    "<strong>Generics, closures and threads.</strong> `larger<T: integer>` works for any integer type; `to_f` is a closure whose capture list names what it uses; `spawn` starts a thread and `join` waits for its result. The two threads share their counter only through a `mutex`: `lock` is the only way in.",
])
tour_body += p("""Further showcases can be found in the repository's
<a href="https://github.com/strawberry9/CobaltC/tree/main/showcase">showcase directory</a>; its
<a href="https://github.com/strawberry9/CobaltC/blob/main/showcase/README.md">README</a> describes
them all.""")

parts_body = ""
parts_body += p("""CobaltC comes in two layers. The <strong>core language</strong> is always there: its
types, its constructs, and a set of <em>intrinsics</em>, functions defined by a rule of the
specification rather than by a body. The <strong>standard library</strong>, `std`, is a module,
written mostly in CobaltC itself. Nothing of `std` is in scope until a module writes
`import std;`, and each module imports it for itself (<a href="#s17">§17</a>). This page says which
layer each part belongs to.""")
parts_body += code("""// No `import std;`: only the core language.
fn main() : u8
{
    array<i32, 4> a = [3, 4, 5, 6];
    i32 s = 0;
    foreach (x in a)
    {
        s += x;
    }
    u32 m = max_value<u32>();
    s += narrow<i32>(m % 10);            // 4294967295 % 10 is 5
    narrow<u8>(s)                        // the exit status: 23
}
""", title="Core only: no import", expect="ok", status=23, section="parts", slug="core-only")
parts_body += code("""fn main()
{
    printf("hello\\n");                   // ✗ diag.unbound-name (static): printf is std's
}
""", title="printf needs the import", expect="diag.unbound-name", section="parts", slug="no-import-printf")

parts_body += h3("Core: always available, no import")
parts_body += h4("Types built into the language")
parts_body += ul([
    "Integers `i8`–`i128`, `u8`–`u128`, `isize`, `usize`; floats `f32`, `f64`",
    "`bool`, `str` (string literals), `void` / `()`",
    "`array<T, N>`, `slice<T, shared>` and `slice<T, exclusive>`",
    "`ref<T, shared>` and `ref<T, exclusive>`, `rawptr<T>`",
    "`fn(…) : R` values and closures",
    "`handle<T>` (a thread), `mutex<T>`, `guard<T>`",
])
parts_body += h4("Language constructs")
parts_body += p("""`struct`, `bitstruct`, `enum` (with numeric codes), `resource`, destructors (`Type::drop`),
generics with the built-in bounds `eq`, `ordered`, `number`, `integer` and `clone`, `match` with
nested, literal and text patterns, `if` and `while` (either with a pattern as the condition), `for`,
`foreach` (over collections and ranges), `break`, `continue`, `?`, `const`, modules and `import`, `unsafe`,
`unsafe extern fn` and `extern "code";`.""")
parts_body += h4("Intrinsics")
parts_body += table(["Group", "Names"], [
    ["Resources", "`drop`"],
    ["Conversions", "`widen`, `narrow`, `checked_narrow`, `narrow_wrapping`, `reinterpret`, `to_float`, `to_int`"],
    ["Bits", "`count_ones`, `leading_zeros`, `trailing_zeros`, `rotate_left`, `rotate_right`"],
    ["Arithmetic", "`wrapping_add`/`sub`/`mul`, `saturating_add`/`sub`/`mul`, `checked_add`/`sub`/`mul`/`div`/`rem`, `checked_neg`"],
    ["Numeric limits", "`min_value`, `max_value`"],
    ["Layout", "`sizeof`, `alignof`"],
    ["`str` and slices", "`str_len`, `str_byte`, `str_ptr`, `slice_len`"],
    ["Faults and compile-time checks", "`fault`, `static_assert`"],
    ["Threads", "`spawn`, `join`, `cancel`, `join_status`, `Mutex::new`, `lock`"],
    ["Raw memory (most need `unsafe`)", "`rawptr_of`, `reinterpret_ptr`, `dangling`, `allocate`, `deallocate`, `reclaim`, `release`, `copy_raw`, `read_volatile`, `write_volatile`"],
])
parts_body += p("""`allocate` returns a `Result`, which a program can use without the import even though it
cannot <em>name</em> the `Result` type without it.""")
parts_body += h4("Intrinsics private to std")
parts_body += p("""`key_hash`, `key_eq`, `key_less`, `key_less_at`, `swap_places`, `append_native`,
`text_write`, `parse_check`, `parse_value`, `str_slice` (beneath `StringView::of`), and the names
that begin with `$`: the machinery `std`'s own code is built on. A program cannot call them, nor
`std`'s private externs `stderr_write`, which `eprintf` writes through, and `stdout_flush`, beneath
`flush_stdout`.""")

parts_body += h3("std: needs import std;")
parts_body += p("""Mostly written in CobaltC itself; a few parts are provided natively by each
implementation. `std` is divided into submodules by subject, and `import std;` brings in all of
them, so a program never needs to know which holds what; the column says, for a program that wants
one subject alone (`import std::text;`). <a href="#s21">§21</a> covers it in full.""")
parts_body += table(["Area", "Items", "Submodule"], [
    ["Core types", "`Option<T>`, `Result<T, E>`, `AllocError`; `unwrap`, `unwrap_or`, `expect`, `ok_or`, `is_some`/`is_none`/`is_ok`/`is_err`; `map_err`", "`std::core`"],
    ["Collections", "`Vec<T>` (push, pop, index, insert, remove, swap_remove, retain, dedup, clone, from_slice, filled, from_fn, append, contains, position, reverse, truncate, swap, clear, …), `HashMap<K, V>`, `HashSet<K>`, `Queue<T>`, `PriorityQueue<T>`, `slice_parts`", "`std::collections`"],
    ["Sorting", "`Vec::sort`, `sort_by`, `binary_search`, `binary_search_by`", "`std::collections`"],
    ["Ownership helpers", "`Box<T>`, `Rc<T>`, `Weak<T>`", "`std::memory`"],
    ["Values behind references", "`swap`, `replace`, `overwrite`", "`std::core`"],
    ["Numeric helpers", "`min`, `max`, `abs`, `pow`, `clamp`, `gcd`, `is_nan`, `is_finite` (over the built-in bounds)", "`std::math`"],
    ["Floating-point functions", "`sqrt`, `floor`, `ceil`, `round`, `trunc`, `ln`, `exp`, `log2`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `powf`", "`std::math`"],
    ["Random numbers", "`Rng`, a seeded pseudo-random generator (`Rng::from_os` for a fresh seed, `Rng::shuffle`); `os_random_bytes`, `os_random_u64` for secrets; `uuid_v4`", "`std::random`"],
    ["Networking", "`TcpListener` (bind, accept), `TcpStream` (connect, read_line, read, write, printf, shutdown_write, try_clone, timeouts, keepalive), `UdpSocket` (send_to, recv_from, connect, send, recv), `IpAddr`, `SocketAddr`, `resolve`, `NetError`", "`std::net`"],
    ["Other programs", "`Command` (arg, current_dir, env, output, status, spawn), `Child` (read_output_line, read_error_line, write_input, wait, terminate, kill), `process_id`, `watch_interrupts`, `interrupt_requested`, `exit`", "`std::process`"],
    ["Bytes", "`Vec::push_le`, `push_be`, `read_le`, `read_be`", "`std::collections`"],
    ["Checksum", "`crc32` (CRC-32/IEEE; catches accidents, not tampering)", "`std::crypto`"],
    ["Text", "`String`, `StringView` (split, lines, split_whitespace, chars, code_point, find, trim, starts/ends_with, strip_prefix/suffix, parse), `String::parse<T>`, `String::append`, `push_code_point`, `sprintf`, `String::appendf`, the ASCII tests and case changes, `Utf8Error`, `ParseError`", "`std::text`"],
    ["Output", "`printf`, `eprintf`, `flush_stdout`; the extern `stdout_write`", "`std::io`"],
    ["Checks", "`assert`", "`std::core`"],
    ["Input", "`read_line`, `read_all`, `read_all_bytes`; the extern `stdin_read`", "`std::io`"],
    ["Files", "`File` (open, create, create_new, append, open_rw, read, read_line, write, seek, position, flush, len, close, …), `read_file`, `write_file`, `read_bytes`, `write_bytes`, `FileError`", "`std::fs`"],
    ["Directories and paths", "`make_dir`, `remove_file`, `remove_dir`, `rename`, `list_dir`, `path_kind`, `make_dir_all`, `remove_dir_all`, `copy_file`, `file_info`, the `path_` functions (join, parent, file_name, stem, extension, is_absolute, normalize, canonical)", "`std::fs`"],
    ["Arguments and environment", "`arg_count`, `arg` and the extern `arg_bytes`; `Args` (flag, option, options, finish, help); `env_var`, `current_dir`, `set_current_dir`, `temp_dir`, `home_dir`, `current_exe`, `hostname`", "`std::env`"],
    ["Clocks", "`monotonic_ns`, `unix_seconds`, `sleep_ms`; dates and times are `DateTime`", "`std::time`"],
    ["Concurrency", "`Channel<T>` (new, send, recv, try_recv, recv_timeout_ms, close), `cpu_count`, `try_join`, `is_finished`, `ThreadFailure` (text, diagnostic, is_cancelled)", "`std::sync`"],
    ["Cryptography", "`sha256`, `sha384`, `sha512`, `sha1` (legacy), `blake2b`, `hmac_sha256`, `hmac`, `digest_eq`, `hkdf_sha256`, `chacha20_poly1305_seal`/`open`, `aes_gcm_seal`/`open`, `x25519`, `ecdh`, `ed25519_sign`/`verify`, `ecdsa_sign`/`verify`, `rsa_sign_pss`/`verify_pss`, `hash_password`/`verify_password`; `to_hex`, `from_hex`, `base64_encode`, `base64_decode` in `std::text`", "`std::crypto`; `to_hex`, `from_hex`, `base64_encode`, `base64_decode`: `std::encoding`"],
    ["HTTP", "`http_get`, `http_post`, `HttpClient`, `Url`, `form_decode`/`form_encode`; `HttpServer` (`bind`, `bind_tls`), `HttpConnection::request`/`respond`", "`std::http`"],
    ["JSON", "`Json` (parse, text, pretty, get, at, as_*, set, push), `JsonMember`", "`std::encoding::json`"],
    ["Compression", "`deflate`/`inflate`, `zlib_deflate`/`zlib_inflate`, `gzip`/`gunzip`, `adler32`, `CompressError`", "`std::compress`"],
    ["Databases", "`PgConnection::connect`, `query`, `execute`, `run_script`; `PgValue`, `PgResult`, `PgRow::get`, `get_i64`, `get_f64`, `get_bool`, `get_bytes`", "`std::database::postgres`"],
    ["Big integers", "`BigUint` (add, sub, mul, div_rem, mod_pow, shifts, text, parse, bytes)", "`std::math`"],
    ["Certificates and TLS", "`Certificate`, `TrustStore`, `verify_chain`; `TlsStream` (connect, client, server, read, read_line, read_exact, write, peer_certificate, close)", "`std::x509`, `std::tls`"],
    ["Dates and times", "`DateTime` (from_unix, now, new, to_unix, to_iso, from_iso, to_http_date, from_http_date, weekday, less, to_local), `Weekday`, `unix_ms`, `iso_from_unix_ms`, `unix_ms_from_iso`, `local_offset_seconds` — UTC, and the machine's local time", "`std::time`"],
])

parts_body += h3("What may surprise you")
parts_body += ul([
    "`printf` and `assert` are `std`, not core: a program without the import cannot print.",
    "`Vec`, `String` and `Option` are `std`, but `array`, `str`, `slice`, `handle` and `mutex` are core.",
    "`swap`, the one to use, is `std`; `swap_places`, which it is built on, is private to `std`.",
    "`min` and `max` are `std`; `min_value` and `max_value` are core.",
    "`sqrt`, `round` and the other floating-point functions are `std` (`std::math`), not core: they are ordinary library functions, so they need the import, and a function of your own named `round` is an ordinary name.",
])

SECTIONS = [
    Section("intro", "", "Introduction", None,
            "Why CobaltC exists, what it promises, and which features were chosen to keep those promises.",
            intro_body),
    Section("tour", "", "A tour in one program", None,
            "A complete, runnable program that introduces the syntax and the main capabilities, and the mistakes the compiler stops, before the reference sections begin.",
            tour_body),
    Section("parts", "", "Core language and std", None,
            "Which parts of CobaltC are the language itself, always available, and which come from the standard library and need import std;.",
            parts_body),
]
SECTIONS[0].group = "Start here"
