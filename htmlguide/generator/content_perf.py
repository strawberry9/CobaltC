# Performance: advice measured with cobc (2026-10-05, re-measured twice on
# 2026-10-06 after the two compiler rounds that closed most of the gaps, and
# again on 2026-10-08 after the Vec rounds, D-0188 to D-0191, and on
# 2026-10-10 after the compile-time discharge rounds, D-0199 and D-0200;
# stress/perfguide/FINDINGS.md and results.txt have the figures). The figures come from
# a measurement round of small A/B programs, each variant in its own
# function, built with cobc -O2 under GCC and Clang (which agreed within a
# few per cent unless stated), median of five runs on the benchmark machine
# that bench/RESULTS.md describes. Ratios, not absolute times, are quoted:
# they are what carries over to another machine.

from gen import Section, p, h3, h4, ul, ol, table, note, rules, code
from content_a import CHECK_STD

b = ""
b += p("""This chapter is about making compiled programs fast. Every piece of advice in it was
measured: a small program written two or more ways, each way built by `cobc` at its default `-O2`
under GCC and under Clang, and timed. The ratios quoted are from those runs, on the machine described
in `bench/RESULTS.md`. They describe `cobc` as it is today. The compiler proves more with each
release, so a gap measured here may close; where that is likely, the text says so.""")
b += p("""None of it applies to `coby`. The interpreter tracks every object, reference and access
path and makes every check the specification leaves to run time, so it is many times slower than
compiled code whatever you write, and its relative costs are not `cobc`'s. Use it to run a program
at once and to check what it does, never to time it.""")

b += h3("A workflow by stage", anchor="perf-workflow")
b += table(["Stage", "Command", "What it costs and gives"], [
    ["Writing", "`coby --check prog.cb` or `cobc --check src/`",
     "Every check made before a program starts: syntax, names, types, ownership, exhaustive matches. Nothing runs and no C is compiled, so it takes seconds even for a large program, and a program that reads input, writes files or serves the network is checked without doing so."],
    ["Trying it", "`coby prog.cb`, or `cobc --run -O0 prog.cb`",
     "`-O0` makes the C compiler's work short, at the price of a slow program: right while you only care whether it behaves correctly."],
    ["Measuring and shipping", "`cobc -o prog prog.cb` (`-O2`, the default)",
     "What users run. Measure only this: `-O0` keeps exactly the work `-O2` removes, so a comparison made at `-O0` says nothing about the real build."],
])
b += p("""Beyond `-O2`: `--cc clang` and `--cc gcc` choose the C compiler, and neither wins everywhere
(`bench/RESULTS.md` has the comparison); `-march=native` lets the C compiler use every instruction
your processor has, at the cost of an executable that may not run on an older one. To see where the
time goes, `perf stat ./prog` counts instructions and `perf record ./prog` then `perf report` names
the functions; `cobc --keep-c` keeps the generated C, in which every run-time check is a visible
line (\"Reading the C that `cobc` generates\" in the Introduction).""")

b += h3("Where the time goes", anchor="perf-costs")
b += p("""A compiled CobaltC program makes every check the specification requires, except those the
compiler proves can never fail. Two kinds of check cost very differently:""")
b += ul([
    "<strong>Cheap checks</strong>, a compare and a branch the processor predicts: an index against a length, an addition for overflow. In a tight loop they cost a few per cent to a few tens of per cent.",
    "<strong>Tracking</strong>, which the runtime library does for a value the compiler cannot reason about: it records each reference and access path as it is made, checks each access against the others (§08–§09), and retires them at the end of their statement or scope. This costs hundreds of nanoseconds per access, a hundred times a plain access or more.",
])
b += p("""So the advice that matters most is not about saving checks: it is about writing code whose
values the compiler can see through, so that tracking is left out. The sections below are in order
of how much they saved in the measurements.""")
b += p("""To see where your own program still pays for tracking, run `cobc --report-tracking prog.cb`. It
compiles nothing: it lists the lines whose generated C still calls the runtime's tracking, with the
kinds of call on each, lines inside a loop first. A line inside a loop is the one to look at; a
line that runs once (a binding, a move into a call) costs next to nothing. The sections below say
how to rewrite what it finds.""")

b += h3("Keep hot vectors confined", anchor="perf-confined")
b += p("""`cobc` proves a `Vec` <em>confined</em> when nothing but its own name can ever reach it.
A confined `Vec`'s accesses compile to plain C with bounds and overflow checks and no tracking. A
local `Vec`, or one a function takes by value, stays confined when it is:""")
b += ul([
    "made where it is declared (`Vec::new()`, or a function's result), or received as a parameter `Vec<T> v`;",
    "used through `Vec::push`, `Vec::len`, `Vec::pop`, `Vec::reserve`, `Vec::swap`, `Vec::reverse`, `Vec::insert`, `Vec::remove`, `Vec::truncate`, `Vec::clear`, `Vec::sort`, `Vec::contains`, `Vec::index_of`, `Vec::binary_search`, indexing (`v[i]`, `v[i] = x`, `v[i].f += x`) and `foreach`, also `foreach (p in &mut v)` writing `p.f`;",
    "passed as `&v` or `&mut v` to functions that themselves use it only in those ways (the compiler checks each such function), and in no other argument of the same call;",
    "at most moved out at the end: as the function's result (also inside a struct or `Ok(…)` built as the result), or into a call as its last use, such as `overwrite(&mut h.v, v)`.",
])
b += p("""A <strong>shared</strong> reference to an element, held across the loop
(`ref<i64, shared> first = &v[0];`), leaves the loop's reads as fast as before: a read cannot clash
with a shared reference. Writes to the vector and pushes onto it are then checked, since one of
them could. What loses confinement altogether is an <strong>exclusive</strong> reference to an
element (`&mut v[i]` kept in a binding), `&mut v` handed to a function that may keep a path to it,
a closure that names `v`, or moving `v` to another binding; then every access in the loop is
tracked, about ten times the cost in a summing loop.""")
b += p("""A vector that is a field of a local struct (`h.v`) is confined the same way while the function
uses it only by indexing (`h.v[i]`, `h.v[i] = x`, `h.v[i].f += x`) and `Vec::len(&h.v)`, and never
uses `h` whole (moving it, borrowing it, capturing it); the struct's other fields are used freely.
Where the function also pushes to `h.v`, borrows an element of it or hands it on, indexing it there
is tracked; then hand it to a helper (`sum_helper(&h.v)`), loop over it with
`foreach (x in &h.v)`, or move it out for the loop and back (`replace` then `overwrite`), each of
which is as fast as a local. Confinement is decided per function: one function's use of a vector
does not slow another function down.""")
b += p("""A function that takes a <strong>struct by reference</strong> and loops over the struct's
vectors (the shape of most programs: `fn restock(ref<Inventory, exclusive> inv, …)`) runs as plain
code too, wherever the struct lives, when that reference is the function's only one, its other
parameters are plain values, and it uses each such vector only in the ways listed above (no
`&inv.counts` handed to another function, no element borrowed and kept). The function then
checks once, as it starts, that no element of those vectors is borrowed elsewhere, and otherwise
runs its checked code, which faults where §08 says it must. Its other fields (`inv.owner`,
`inv.total`) are used freely. Over 1,000,000 elements through such a reference, a write
`v[i] = v[i] + 1` went from 546 to 1.8 ns an element, `pop` from 280 to 8, `push` from 41 to 9:
the speed of a local.""")
b += code(CHECK_STD + """
struct Inventory
{
    String owner;
    Vec<i64> counts;
    Vec<u8> flags;
    i64 total;
}

// One reference, to a struct whose vectors it only indexes, pushes,
// swaps and measures: checked once as it starts, then plain code.
fn restock(ref<Inventory, exclusive> inv, i64 amount)
{
    for (usize i = 0; i < Vec::len(&inv.counts); i += 1)
    {
        if (inv.counts[i] < 3)
        {
            inv.counts[i] += amount;
            inv.flags[i] = 1;
        }
        inv.total += inv.counts[i];
    }
    Vec::push(&mut inv.counts, amount);
    Vec::push(&mut inv.flags, 0);
    Vec::swap(&mut inv.counts, 0, Vec::len(&inv.counts) - 1);
}

fn main()
{
    Inventory inv = Inventory { .owner = String::from_str("shop"), .counts = Vec::new(), .flags = Vec::new(), .total = 0 };
    foreach (i in 0..8: i64)
    {
        Vec::push(&mut inv.counts, i % 5);
        Vec::push(&mut inv.flags, 0);
    }
    restock(&mut inv, 10);
    printf("%s: %v items, total %v, first %v\\n", &inv.owner, Vec::len(&inv.counts), inv.total, inv.counts[0]);
}
""", title="Functions over a struct that owns vectors", expect="ok", output="shop: 9 items, total 73, first 10\n", section="perf", slug="perf-struct-vec")
b += table(["The same sum of 20,000,000 elements, ten times", "Time, relative"], [
    ["A local `Vec`, indexed", "1"],
    ["The same `Vec`, passed as `&v` to a helper that indexes it", "1"],
    ["A `Vec` field of a local struct, indexed in the loop (`h.v[i]`)", "1"],
    ["A `Vec` field of a struct passed to that helper (`sum_helper(&h.v)`), or moved out for the loop and back", "1"],
    ["A local `Vec` while a shared reference to one element is held", "1"],
    ["A local `Vec` that is moved to another binding after the loop", "10"],
])

b += p("""<strong>Predicates.</strong> `Vec::retain`, `Vec::position` and `Vec::binary_search_by` run
as plain loops when the function they are given is a closure literal written at the call, a named
function, or a `fn` value holding a named function, provided it only reads the elements it is
given (`Vec::sort_by` does so for a closure literal): 1,000,000 elements through `retain` took 3 to
9 ns each. A closure stored in a
variable first (`fn(…) : bool p = [lim](…) { … };`) is tested when the call starts: one that only
reads what it is given is called with bare element addresses, about 150 ns an element (the cost of
calling through the variable); one that may keep what it is given is lent each element through the
runtime, about a microsecond. Writing the closure at the call is fastest.""")

b += h3("Updating elements", anchor="perf-elements")
b += p("""Every way of updating the elements of a `Vec` of structs compiles to the element's address
and a few plain instructions, as long as the vector is confined: `v[i].x += v[i].vx`,
`foreach (p in &mut v) { p.x += p.vx; }`, a call that receives one element, `step(&mut v[i])`, and
an element named for a block, `ref<P, exclusive> p = &mut v[i];` (provided the rest of that block
uses `p` and not `v`). Over 100,000 elements updated 100 times, all four took the same time. Choose
whichever reads best:""")
b += code(CHECK_STD + """
struct Particle
{
    i64 x;
    i64 vx;
}

fn main()
{
    Vec<Particle> ps = Vec::new();
    foreach (k in 0..1000: i64)
    {
        Vec::push(&mut ps, Particle { .x = k, .vx = k % 7 - 3 });
    }
    foreach (t in 0..100)
    {
        foreach (p in &mut ps)              // plain code: the loop holds ps for its length
        {
            p.x += p.vx;
            if (p.x < 0 || p.x > 1000)
            {
                p.vx = -p.vx;
            }
        }
    }
    printf("%v %v\\n", ps[0].x, ps[999].x);
}
""", title="Updating a Vec of structs", expect="ok", output="294 803\n", section="perf", slug="perf-step")

b += h3("Functions that take references", anchor="perf-refs")
b += p("""A function that takes a reference (`ref<T, …>`) or a slice runs as plain code, with no
tracking inside it, when `cobc` can see that nothing else can clash with that reference while the
call runs. It can see it in three cases: the reference is the function's only one; all its
references are `shared`; or, the usual shape of code that builds something from its input, some
are `exclusive` and the rest `shared` and every one is to plain data. In the third case the function
checks once, as it starts, that its arguments do not overlap, and then runs unchecked; where they
do overlap (`count(&mut c, &c.buf[0..4])`), it runs the checked code instead, which faults where
§08 says it must. Plain data is numbers, `bool` and `char`, and arrays, structs and enums of them;
not a `String`, a `Vec`, a `Box` or a reference. And in every case the body must use each reference
only directly:""")
b += ul([
    "through it: reads and writes of the value, its fields and its elements, and `slice_len`;",
    "never handed to another function, kept in a variable or a struct, or borrowed from (an only reference may still be handed whole to a function that returns plain data or nothing);",
    "beside parameters that are plain values, in the third case.",
])
b += table(["10,000,000 bytes summed into a struct of two counters", "Time, relative"], [
    ["`fn feed(ref<Acc, exclusive> a, slice<u8, shared> data)`", "1"],
    ["`fn feed(Acc a, slice<u8, shared> data) : Acc`, the counters passed in and returned", "0.7"],
    ["The first, each byte read through a helper, `a.sum += byte_at(data, i)`", "80"],
    ["The first, with `Acc` also holding a `Vec`", "90"],
])
b += p("""So keep the loop over the input in the function itself, and keep the state it updates in a
struct of plain fields. Where that state belongs to something larger (a struct that also holds a
`String` or a `Vec`), pass the plain part alone, as `count(&mut r.counts, …)` does here:""")
b += code(CHECK_STD + """
struct Counts
{
    u64 bytes;
    u64 newlines;
    u64 sum;
}

struct Report
{
    String name;
    Counts counts;
}

// One exclusive reference to plain data beside a shared slice, each used
// only directly: checked once as the call starts, then plain code.
fn count(ref<Counts, exclusive> c, slice<u8, shared> data)
{
    for (usize i = 0; i < slice_len(data); i += 1)
    {
        c.bytes += 1;
        c.sum = wrapping_add(c.sum, widen<u64>(data[i]));
        if (data[i] == 10)
        {
            c.newlines += 1;
        }
    }
}

fn main()
{
    Vec<u8> data = Vec::new();
    foreach (k in 0..1000)
    {
        Vec::push(&mut data, narrow_wrapping<u8>(k * 7));
    }
    Report r = Report { .name = String::from_str("input"), .counts = Counts { .bytes = 0, .newlines = 0, .sum = 0 } };
    count(&mut r.counts, &data[0..$]);    // the struct holds a String: pass only its plain part
    count(&mut r.counts, &data[0..10]);
    printf("%v: %v bytes, %v newlines, sum %v\\n", &r.name, r.counts.bytes, r.counts.newlines, r.counts.sum);
}
""", title="State and input", expect="ok", output="input: 1010 bytes, 4 newlines, sum 126831\n", section="perf", slug="perf-refs")

b += h3("Text in hot code", anchor="perf-text")
b += p("""`std` takes text it only reads as a `StringView` (D-0134), and for most code that is the
right choice. In compiled code a view is not quite free, though: making one from a `String`
(`String::as_view`) costs about 60 nanoseconds for its checks, and every piece
`split` returns holds its own borrow of the text. For a function of your own that a hot loop calls
millions of times:""")
b += table(["1,000,000 calls of a function that reads a 60-byte text", "Time, relative"], [
    ["`fn f(StringView s)`, one view made before the loop and passed each time", "0.2"],
    ["`fn f(ref<String, shared> s)`, called `f(&text)`", "1"],
    ["`fn f(StringView s)`, called `f(String::as_view(&text))`", "3"],
    ["`fn f(String s)`, called `f(String::clone(&text))`", "24"],
])
b += ul([
    "A function that only reads its `StringView` (indexes it, takes its length, hands it to `std`'s readers) costs nothing to call: the view is passed as its address and length. Make the view once, outside a hot loop: each `String::as_view` costs about 60 nanoseconds.",
    "To process input line by line, use a `read_line` loop: `read_all` followed by `String::split` took about nine times as long over the same 200,000 lines, the price of keeping every line's view at once.",
    "Calls to `std` that take a view cost per call, not per byte (a 620-byte text cost the same as a 60-byte one).",
])

b += h3("Building and writing text", anchor="perf-output")
b += table(["Building one String from 2,000,000 numbers", "Time, relative"], [
    ["`String::append(&mut out, i)` then `String::append(&mut out, \",\")`", "1"],
    ["`sprintf(\"%v,\", i)`, then append that", "9"],
    ["A `Vec<String>` of pieces (each made by `sprintf`), then `String::join`", "18"],
])
b += p("""`String::append` formats a number or a `bool` straight into the `String`, with no
intermediate text, so build output with it; `sprintf` makes a new `String` each time, about
two-thirds of a microsecond a call. Standard output is buffered (§21), so `printf` per line is a
fast way to write many lines: 2,000,000 lines to a file took 0.38 s with one `printf` per line and
0.24 s building one `String` with `String::append` and printing it once. Printing as you go is
fast enough for most programs and keeps no text in memory; building one `String` saves a little
more where all the text fits.""")

b += h3("Choose the collection by what you do most", anchor="perf-collections")
b += table(["Operation", "Fast", "Slow", "Measured"], [
    ["Take elements from the front, in order", "`Queue::pop_front`, or `Vec::reverse` once then `Vec::pop`", "`Vec::remove(&mut v, 0)`, which moves every remaining element: the cost grows with the square of the length", "10,000 elements: 2.4 times; 100,000: 51 times"],
    ["Test membership in a large set", "`Vec::sort` once then `Vec::binary_search`, or `HashSet::contains`", "`Vec::contains`, which compares element by element", "20,000 keys, 5,000 queries: `binary_search` 5 to 6 times as fast, `HashSet` 1.3 times, growing with the size"],
    ["Fill a `Vec` whose size you know", "`Vec::filled(n, x)`, then write the elements", "pushing one by one (with or without `Vec::reserve` first, which no longer saves anything measurable)", "50,000,000 elements: 5 to 6 times"],
])
b += p("""Counting with a `HashMap<String, …>`: `*HashMap::entry(&mut counts, String::clone(w), 0) += 1`
clones the key on every call; looking the key up first,
`match (HashMap::get_mut(&mut counts, w)) { Some(c) : *c += 1, None : … }`, clones only for a new
key. Over 1,000,000 words the second was about 8% faster, but both are a few hundred nanoseconds a
word: write whichever reads better.""")

b += h3("Threads", anchor="perf-threads")
b += p("""Each running thread keeps its own share of the compiled program's runtime checks, so threads
doing their own work run in parallel (§19). Measured with four threads each counting the words of
the same text: 0.30 s, against 0.16 s for one thread doing a quarter of the work, and 0.24 s for
four separate programs. To keep a thread's work its own:""")
b += ul([
    "Move what a thread works on into it (`spawn(count, chunk)` with `chunk` a `Vec` you no longer need): it then belongs to the thread. A thread that reads the spawner's data through a reference shares it. Read it through a shared reference *parameter* the function never rebinds or borrows (`fn band(ref<Vec<Row>, shared> rows, …)`), and hand its fields and elements on only to functions that keep nothing of them: `cobc` then reads through it with bounds checks alone, since nothing can change what it reaches while the call runs, and several threads read one structure at once (four threads aggregating 20,000 rows: 30 ms, against 48 ms for one). A reference kept past the call, or stored in a struct, is tracked as usual, and threads that form such references into one structure take turns on its records.",
    "Hand results back as the thread's result, through `join`, or through a `Channel`, rather than writing into a structure every thread locks.",
    "Keep a lock's block to reads and writes through its guard (`{ auto g = lock(&m); *g += 1; }`): such a block is a lock and an unlock and nothing more. Four threads each taking one shared mutex 200,000 times, with a little work between, took 0.6 s; giving the guard to a function, or returning from inside the block, makes each lock cost about ten times as much.",
    "A thread per connection is a sound design for a server. Each thread is an operating-system thread with a reserved, mostly uncommitted stack, and a finished thread is reused by the next `spawn`. A test server held 10,000 connections at once, each its own supervised thread answering two queries, in 250 MB, about 25 kB per idle connection, with no failures, and a round trip cost the same 0.13–0.16 ms with 1,000 connections open as with 10,000. An idle connection costs nothing while it waits: on Linux its thread sleeps in the kernel until data arrives or it is cancelled.",
])

b += h3("Arithmetic and plain values", anchor="perf-arith")
b += ul([
    "Every `+`, `-` and `*` on integers is checked for overflow (§06). Where wrapping around is what you mean (hashes, checksums, random number generators), say so with `wrapping_add`, `wrapping_mul` and their relatives: a tight sum took a third less time, and Clang executed 40% of the instructions. Never use them to avoid a check on arithmetic that should not overflow: the check is what turns a wrong result into a diagnostic.",
    "Pass a struct by whichever reads best. A 64-byte struct of eight `i64` passed to a function that reads it took the same time copied as through a `ref<…, shared>`.",
])

b += h3("When nothing else is enough", anchor="perf-c")
b += p("""A kernel that must run at the speed of hand-written C can be hand-written C:
`extern \"./kernel.c\";` links it and `unsafe extern fn` declares it (§20). That code is outside every
guarantee CobaltC makes: its memory safety, its aliasing and its arithmetic are yours to get right,
and its declaration is an unchecked claim. Measure first.""")

b += rules([
    "Measure the `-O2` build, never `coby` or `-O0`.",
    "Keep a hot `Vec` a local (or a parameter it owns) that only its own name reaches; across a loop, hold shared element references only, never an exclusive one.",
    "Index a struct's `Vec` field (`h.v[i]`) where the function only indexes it; where it also pushes to it or hands it on, loop with `foreach` or a helper.",
    "Give a function over a struct's vectors the struct as its only reference, and keep its uses of the vectors to indexing and the `Vec` calls listed above.",
    "In a function that takes references, use them only directly: loop over a slice in the function itself, and update plain state through one `exclusive` reference (or pass it by value).",
    "Make a `StringView` once, outside a hot loop; passing it on is free.",
    "Build text with `String::append`; `printf` per line is fine.",
    "Write a child process's input in large pieces (`ChildInput::write`, `Child::write_input`): each write goes straight to the pipe and wakes the child, so a write per line of 100,000 short lines cost 11 s of system time, and 64 KB at a time 0.06 s. Nothing is buffered, so a program that writes a line and then waits for the child's answer works as written.",
    "Never `Vec::remove(&mut v, 0)` in a loop over a long `Vec`; for membership in a large set, sort once and `Vec::binary_search`, or use a `HashSet`.",
    "Make a `Vec` of a known size with `Vec::filled`, then write its elements.",
    "Say `wrapping_` only where wrapping is the meaning.",
])

PERF = Section("perf", "P", "Performance", None,
               "Measured advice for fast compiled programs: what costs time in cobc's output, and how to write code the compiler can see through.",
               b)
PERF.group = "Practice"

SECTIONS = [PERF]
