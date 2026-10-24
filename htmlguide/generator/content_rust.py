"""The section comparing CobaltC with Rust: what the two share, where
they differ and why each chose as it did, without a verdict. Every
complete CobaltC program is checked by `gen.py --check` under both
tools; the Rust fragments are shown, not run."""
from gen import code, h3, note, p, table, ul, Section

b = ""
b += p("""Rust and CobaltC want the same things: no null, no dangling reference, no data race, no
use after free, one owner per resource, every failure a value or a named stop, and all of it
without a garbage collector or a hidden runtime. A Rust programmer will recognise most of CobaltC
on sight, which makes the places where the two part company the interesting ones. This section
goes through them: when a check happens, what a type says, how generic code is written, how
threads share, how failure is reported, what `unsafe` means, and what the surface looks like. It
takes no side. Each choice has a price and a payoff, and the point is to say what they are. Rust is
a mature language with a large ecosystem; CobaltC is a small one with a complete written semantics
and two implementations that share it. Which matters more depends on what you are doing.""")

b += h3("What the two share", anchor="rust-shared")
b += ul([
    "<strong>Ownership and moves.</strong> A resource has one owner; assigning or passing it moves it, and the old name is unusable afterwards. In Rust every non-`Copy` type moves; in CobaltC a type is either plain (copied) or a resource (moved), decided by its declaration: `struct` or `resource struct`, and anything that holds a resource.",
    "<strong>Shared or exclusive access, never both.</strong> Rust's `&T` and `&mut T` are CobaltC's `ref<T, shared>` and `ref<T, exclusive>`, written `&x` and `&mut x` in both. Any number of readers, or one writer, to one object at a time.",
    "<strong>Destruction at scope end.</strong> Rust's `Drop` is CobaltC's `Type::drop`; both run in reverse order of declaration, on every path out, including a panic or a fault.",
    "<strong>`Option`, `Result` and `?`.</strong> The same two types, the same operator, the same habit of making absence and failure part of a function's type. Neither has exceptions.",
    "<strong>Enums with payloads and exhaustive `match`.</strong> A closed set of alternatives, each carrying data, taken apart by patterns the compiler checks for completeness.",
    "<strong>Generics compiled per use.</strong> Both monomorphise: a generic function becomes one copy per set of concrete types, with no run-time type information.",
    "<strong>Explicit conversions.</strong> Neither converts numeric types silently. Rust writes `as` and `From`; CobaltC writes `widen`, `narrow` and `reinterpret`.",
    "<strong>A marked boundary.</strong> Raw pointers and foreign functions live inside `unsafe` in both, and both say that safe code cannot break the language's guarantees.",
])

b += h3("When a check happens", anchor="rust-when")
b += p("""This is the difference everything else follows from. Whether a reference is used after its
object has gone, or while another part of the program writes through a second reference, depends
on what the program does when it runs; no checker can decide it exactly for every program. Each
language turns that into a question it can answer, and they turn it in opposite directions.""")
b += p("""<strong>Rust decides at compile time, by over-approximating.</strong> Every reference type
carries a lifetime, an upper bound on how long the reference may be used, written as `'a` or filled
in by three elision rules. Every function signature states how its result's lifetime relates to its
parameters'. The borrow checker then checks each function body against its own signature, one
function at a time, following the flow of the code to see which borrows are live where. Nothing is
checked at run time, and nothing is paid at run time. The price is that the checker must be
conservative: when it cannot prove a program safe, it rejects it, correct or not, and the programmer
restructures, adds annotations, or steps into `unsafe`. Two exclusive references into one vector at
indices known only at run time, a struct that refers into itself, a reference kept across a call
that might reallocate: all rejected, because the proof is not in the text.""")
b += p("""<strong>CobaltC decides at run time, against the actual state.</strong> A reference carries no
lifetime; its validity is a fact about the running program: which object it reaches, whether that
object still exists, which other references to it are live and in what mode. The language's rules
are written over that state, and the interpreter and the compiled program check them at each
access. The price is a run-time cost and a fault instead of a compile error: a mistake becomes a
named diagnostic at the step that commits it, in the run that reaches it, and a path no test runs
is a path no check has run. The payoff is that no annotation is needed and no correct program is
refused for lack of a proof. Both languages are in fact hybrids. Rust keeps run-time checks for
the cases its static rule cannot express (`RefCell`, `Mutex`, `Rc`, bounds, and overflow in debug
builds); CobaltC rejects at compile time what its checker can see without running the program
(<a href="#s10">§10</a>, <a href="#s14">§14</a>). The difference is the default, and what each asks
of the programmer where the default fails.""")
b += code("""
import std;

fn main()
{
    Vec<i32> v = Vec::filled(4, 0);
    usize i = 1;
    usize j = 3;
    ref<i32, exclusive> a = &mut v[i];   // two exclusive references into one vector,
    ref<i32, exclusive> b = &mut v[j];   // at indices known only now
    *a = 10;
    *b = 20;
    printf("%v %v\\n", v[1], v[3]);
}
""", title="Accepted here, refused by Rust: disjointness decided at run time", expect="ok", output="10 20\n",
     section="rust", slug="runtime-disjoint")
b += p("""Rust refuses that program: with `i` and `j` unknown, it cannot prove the two borrows
disjoint, so the programmer writes `split_at_mut` or indexes twice. CobaltC accepts it and checks
the indices as the borrows are made. Run it with `j = 1` and the second borrow is the fault
`diag.aliasing-conflict`, at that line, in that run. Rust's answer is the same in both cases and
arrives before the program exists; CobaltC's is different in the two cases and arrives when it
runs.""")
b += code("""
import std;

struct Holder
{
    ref<i32, shared> r;
}

fn keep(ref<i32, shared> r) : Holder
{
    Holder { .r = r }
}

fn main()
{
    i32 outer = 1;
    Holder h = keep(&outer);
    {
        i32 inner = 2;
        h = keep(&inner);          // a reference to inner, stored by a function
        printf("%v\\n", *h.r);      // fine: inner is alive
    }
    printf("%v\\n", *h.r);          // ✗ diag.stale-binding: inner ended with its block
}
""", title="Caught at run time here, at compile time in Rust", expect="diag.stale-binding",
     section="rust", slug="stale-through-call")
b += p("""In Rust, `Holder` would be `Holder<'a>`, `keep` would carry the same `'a`, and the second
assignment would not compile, because `h` outlives `inner`. CobaltC has no `'a` to carry, so the
escape through `keep` is invisible to the checker and the program runs until the read, which faults.
The same escape written in one function, with no call between, is caught before the program runs,
because there the checker can see it:""")
b += code("""
import std;

struct Holder
{
    ref<i32, shared> r;
}

fn main()
{
    i32 outer = 1;
    Holder h = Holder { .r = &outer };
    {
        i32 inner = 2;
        h = Holder { .r = &inner };     // ✗ diag.reference-escapes-scope, before the program runs
    }
    printf("%v\\n", *h.r);
}
""", title="The same escape, visible to the checker", expect="diag.reference-escapes-scope",
     section="rust", slug="escape-static")
b += p("""Moving or destroying a value while a reference to it is live, and writing to a vector while
a reference into it is live, are likewise compile-time errors in both languages when the two uses
are in one function (<a href="#s08">§08</a>, <a href="#s10">§10</a>); Rust also catches them across
functions, through signatures; CobaltC catches those at run time.""")
b += code("""
import std;

fn main()
{
    String s = String::from_str("text");
    ref<String, shared> r = &s;
    String t = s;                  // ✗ diag.move-while-aliased: r still borrows s
    printf("%v\\n", String::len(r));
}
""", title="A move under a live borrow: rejected before running, as in Rust", expect="diag.move-while-aliased",
     section="rust", slug="move-while-aliased")

b += h3("What a type says", anchor="rust-types")
b += p("""Because Rust's checks are static, its types must carry what the checker needs; because
CobaltC's are dynamic, its types carry less and its run-time state carries the rest. That shows up
in four places.""")
b += table(["", "Rust", "CobaltC"], [
    ["A reference", "`&'a T`, `&'a mut T`: a lifetime (usually elided) bounds every use", "`ref<T, shared>`, `ref<T, exclusive>`: no lifetime; validity is checked at each use (<a href=\"#s09\">§09</a>)"],
    ["A struct holding a reference", "`struct Holder<'a> { r: &'a i32 }`: the lifetime is a parameter of the type", "`struct Holder { ref<i32, shared> r; }`: the reference is tied to the storage holding it and the object it reaches, at run time (<a href=\"#s10\">§10</a>)"],
    ["A function returning a reference", "the result's lifetime is related to a parameter's by elision or by writing `'a`; any relation can be stated", "elided when exactly one parameter is a reference; with two or more, returning a reference is refused outright (`diag.lifetime-elision-ambiguous`): return a copy, an index or an owned value instead"],
    ["What may cross to another thread", "`Send` and `Sync` marker traits, checked at compile time; `spawn` wants `'static` data, scoped threads borrow", "any value moves; a reference may be lent for the thread's life and is checked at run time (<a href=\"#s19\">§19</a>)"],
])
b += code("""
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}
""", title="Rust: a lifetime relates two parameters to the result")
b += code("""
import std;

fn longer(ref<String, shared> a, ref<String, shared> b) : ref<String, shared>
{
    if (String::len(a) >= String::len(b)) { a } else { b }    // ✗ which one does the result borrow from?
}

fn main()
{
    String x = String::from_str("abc");
    String y = String::from_str("de");
    printf("%v\\n", String::len(longer(&x, &y)));
}
""", title="CobaltC: with two reference parameters, a returned reference is refused", expect="diag.lifetime-elision-ambiguous",
     section="rust", slug="two-refs")
b += p("""Rust lets `longest` be written by naming the relation; CobaltC, having no names for
lifetimes (that choice is final, D-0011 and CHG-0022), refuses the shape and asks for a function
that returns which one, or a copy. In exchange, a CobaltC signature never needs a lifetime, and a
reference stored in a `Vec`, a `HashMap` or a closure needs no type parameter to say how long it
lives.""")

b += h3("Generics: traits against bounds", anchor="rust-generics")
b += p("""Rust's generics are organised by traits: a type implements a trait, a generic function
requires one (`T: Ord`), and traits also serve as interfaces, as operator overloading, as the
mechanism for iteration, formatting, conversion, and, through `dyn Trait`, as run-time
polymorphism. Most of Rust's expressiveness and most of its learning curve are there. CobaltC has
no traits. A generic parameter takes one of five built-in bounds, `ordered`, `number`, `integer`,
`clone` or `plain`, which name what the language itself knows about a type
(<a href="#s12">§12</a>); a type joins `clone` by defining `T::clone`, and nothing else is
user-extensible. There is no operator overloading, no interface a user type can implement, and no
run-time dispatch except through function values and closures. Where a Rust program would define a
trait and implement it for several types, a CobaltC program writes an enum of those types and a
`match`, or passes the behaviour as a function value. That is a real limit on what can be
abstracted over, chosen so that a call always names the function it calls
(<a href="#s15">§15</a>).""")
b += p("""Neither language has inheritance. Both monomorphise, so generic code costs nothing at run
time in either. Rust infers types across a whole function, forwards and backwards; CobaltC's
inference flows downward only, from a declaration or an expected type into an expression, which is
why its variables are usually declared with a type and `auto` is the exception.""")

b += h3("Mutability", anchor="rust-mut")
b += p("""Rust marks the binding: `let x` is immutable, `let mut x` may be assigned and borrowed
exclusively, and a `&mut` can only be taken from a `mut` binding. CobaltC marks nothing on a
variable: any variable may be assigned, and the question is only ever about references, which are
shared or exclusive (<a href="#s08">§08</a>). What Rust's `mut` buys is documentation and a
compile-time refusal to change what was declared unchanging; what CobaltC's absence of it buys is
one fewer word on every declaration and no second meaning for `mut`. Both make a `const` a
compile-time constant only.""")

b += h3("Integers", anchor="rust-ints")
b += p("""Both have the same sized integer types and the same `wrapping_`, `saturating_` and
`checked_` operations. They differ on what plain `+` does when it overflows. In Rust it panics in
a debug build and wraps in a release build unless overflow checks are turned on; a shipped program
usually wraps. In CobaltC it is a fault, always, in both the interpreter and the compiled program
(<a href="#s06">§06</a>): there is no build in which an overflow is silent. The compiled cost of that
choice is measurable and modest (<a href="#perf">P</a>): on the repository's benchmarks a CobaltC
program compiled through C runs between about the same time as the Rust program and about twice
it, and much of the gap is the arithmetic checks that a Rust release build leaves out.""")

b += h3("Threads", anchor="rust-threads")
b += p("""Rust makes a data race a type error: a value crosses to another thread only if it is
`Send`, is shared only if it is `Sync`, and `spawn` requires what it captures to live for the whole
program, so sharing goes through `Arc<Mutex<T>>` or a scoped thread whose borrows the checker can
see end. CobaltC's `spawn` moves its arguments or lends a reference for the thread's life, and the
check that two threads never touch one object incompatibly is the same run-time alias check as
within one thread; a `mutex` guard, an `Atomic`, a `Channel` or an `Arc` are the ways to share on
purpose (<a href="#s19">§19</a>). What that gives CobaltC, and Rust does not set out to give, is an
account of every schedule: a program's threads interleave at the granularity of single operations
under rules the specification states, so `--explore` can run a program under many schedules and
report every distinct outcome, and a wait that no thread can end is a diagnostic, `diag.deadlock`,
rather than a hang. Rust has no equivalent in the language; it has tools outside it. Both unwind a
failed thread and hand the failure to whoever joins it; both poison a mutex the failed thread held.
CobaltC has no condition variables and no memory orderings weaker than sequential consistency; Rust
has both.""")

b += h3("Failure", anchor="rust-failure")
b += p("""Both languages separate expected failure, a `Result` or an `Option` the caller handles, from
a bug, which stops the program. Rust's stop is a panic: it unwinds the thread, running destructors,
can be caught with `catch_unwind`, and in a release build an overflow or an out-of-bounds index may
be a panic or, for overflow, nothing at all. CobaltC's stop is a fault: every fault has a name in a
registry (<a href="#appx">the appendix</a>) and a location, it unwinds running destructors, it is
the same in the interpreter and the compiled program, and it is contained only at the two
boundaries the language names, a supervised thread and a C-callable library entry
(<a href="#s18">§18</a>). Rust's error enums are usually hand-written or generated by a library;
CobaltC's are hand-written, with `map_err` and `ok_or` at the edges. `?` means the same thing.""")

b += h3("Unsafe", anchor="rust-unsafe")
b += p("""In both, `unsafe` marks where the programmer, not the language, vouches for a claim. In Rust
the claims are about the absence of undefined behaviour: a raw pointer is valid, aliasing rules are
respected, a foreign function's contract holds; a wrong claim is undefined behaviour, and the rules
for what unsafe code may assume are still being settled (Stacked Borrows, Tree Borrows). In CobaltC
the claims are stated as side-conditions of named trusted rules (<a href="#s20">§20</a>): a raw
read or write, a reclaim of raw memory, a call across the boundary, each with what it asserts. The
standard library's own `unsafe` blocks are listed with their claims. The interpreter simulates raw
memory and checks even trusted operations against it, so a wrong claim is a fault there; the
compiled program trusts the claim, as Rust does. Both languages hold that safe code cannot break
the invariants; neither can promise anything once a trusted claim is false.""")

b += h3("The surface", anchor="rust-surface")
b += table(["", "Rust", "CobaltC"], [
    ["Declarations", "`let x: T = …;`, `let mut`", "`T x = …;`, `auto x = …;`; no `mut`"],
    ["Functions", "`fn f(x: T) -> U { … }`", "`fn f(T x) : U { … }`"],
    ["Methods", "`impl T { fn m(&self) }`, called `x.m()`", "`fn T::m(ref<T, shared> x)`, called `T::m(&x)`; there is no method-call syntax, by decision (<a href=\"#s22\">§22</a>)"],
    ["Patterns", "`match v { Some(x) => …, None => … }`", "`match (v) { Some(x) : …, None : …, }`"],
    ["Closures", "`|x| x + k`, captures inferred", "`[k](i32 x) : i32 { x + k }`, captures listed (<a href=\"#s15\">§15</a>)"],
    ["Loops", "`loop`, `while`, `for x in iter`", "`while`, `for (init; cond; step)`, `foreach (x in &v)`"],
    ["Text", "`&str`, `String`", "`str`, `String`, `StringView` (<a href=\"#s21\">§21</a>)"],
    ["Conversions", "`x as T`, `T::from(x)`, `x.into()`", "`widen<T>(x)`, `narrow<T>(x)`, `reinterpret<T>(x)`; `as` is reserved"],
    ["Formatting", "`println!(\"{x} {y:?}\")`, a macro", "`printf(\"%v %v\\\\n\", x, y)`, a built-in whose format is checked"],
    ["Modules", "`mod`, `use`, crates, `pub`", "`module`, `import`, `export`; one standard library, no package manager (<a href=\"#s17\">§17</a>)"],
    ["Macros", "declarative and procedural", "none"],
    ["Traits", "`trait`, `impl … for`, `dyn`", "five built-in bounds; `T::clone` and `T::drop` by name"],
])

b += h3("Semantics and tooling", anchor="rust-tooling")
b += p("""Rust's meaning is given by its reference, its compiler and its test suite, with a formal
specification (Ferrocene) written after the fact and a model of unsafe code still in research. It
has one compiler, `cargo`, `crates.io`, `rustdoc`, `rust-analyzer`, `clippy` and `Miri`, a stable
release since 2015, and targets from microcontrollers to every major operating system. CobaltC's
meaning is its specification, written as rules before either implementation; the interpreter runs
those rules and the compiler must match it byte for byte on every conformance case, every showcase
and every example in this guide; a model checker enumerates the small programs' outcomes. It has an
interpreter, a compiler through C, a formatter, a standard library with no third-party code, and two
targets, x86-64 Linux and Windows. One is a production language with an ecosystem; the other is a
language whose whole behaviour can be read, run and argued about.""")
b += note("""Neither approach is free. A static guarantee is paid for in annotations and in programs
refused for want of a proof; a dynamic one is paid for at run time and in bugs found by running
rather than by compiling. Rust puts the proof in the types and makes the exceptions dynamic;
CobaltC puts the check in the run and makes the obvious cases static. Which is right depends on
whether the cost of a check at run time or the cost of a proof at compile time is the one you can
afford, and on whether you need a language's behaviour to be fully stated or its ecosystem to be
fully grown.""", title="The trade, in one place")

RUST = Section("rust", "", "CobaltC and Rust", None,
               "What the two languages share, where they part company, and the price and payoff of each choice: when a check happens, what a type says, generics, threads, failure, unsafe and the surface.",
               b)
SECTIONS = [RUST]
