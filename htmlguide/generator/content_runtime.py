"""Appendix B: how a compiled program uses the runtime (`cbrt`): what the
runtime holds, every name of `impl/cbrt/include/cbrt.h`, and why each call
is made. The CobaltC programs are checked by `gen.py --check`; the C is what
`cobc` wrote for them and is shown, not compiled.

The appendix must name every `cb_` name the header declares, and no `cb_`
name it does not: `_check_names` reads the header when the page is
generated and stops with the list of what is missing or unknown, so a call
added to the runtime is added here in the same change."""
import html as _html
import os
import re
import sys

from gen import Section, p, h3, h4, ul, ol, table, note, code

HEADER = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "impl", "cbrt", "include", "cbrt.h")

# Names the generated program defines for itself, which the appendix shows
# in its C: not the header's.
GENERATED = re.compile(r"cb_(s|f|thr|nt|call_fn[a-z0-9_]*|env_[0-9]+|types|type_ids|files|program|main_fn)(_[a-z0-9_]*)?$"
                       r"|cb_rot[lr](8|16|32|64)$")


def plain(text, title):
    """Text shown as it is (generated C, a report): no CobaltC colouring."""
    return '<figure class="code"><div class="code-head"><span class="code-title">%s</span></div><pre><code>%s</code></pre></figure>\n' % (
        _html.escape(title), _html.escape(text.strip("\n") + "\n"))


def calls(rows):
    return table(["Name", "When the generated C uses it", "What it does"], rows)


b = ""

# ---------------------------------------------------------------- why

b += p("""A compiled CobaltC program is two things linked together: the C that `cobc` wrote for the
program, compiled by the system's C compiler, and `cbrt`, the CobaltC runtime, a static library
(`libcbrt.a`). This appendix describes the line between them: what the runtime holds, what the
generated C asks of it and when, and what the generated C does on its own. It is a reference for
reading generated C (`cobc --keep-c`), for reading a profile that names a `cb_` function, and for
working on the compiler or the runtime. Nothing in it is needed to write CobaltC programs, and
nothing in it is a rule of the language: the interface belongs to this implementation and changes
with it. The section on <a href="#perf-costs">where the time goes</a> gives the same picture from
the side of a program's speed.""")

b += h3("Why there is a runtime", anchor="rt-why")
b += p("""CobaltC's safety rules are checked in two places. The static pass proves what it can before
the program runs. What depends on values, such as whether two indexes name the same element, whether
a reference kept in a data structure is still alive when it is used, or whether a binding still owns
its resource on this path, the specification leaves to the run, and says exactly which checks those
are (the diagnostics marked <em>dynamic</em> or <em>both</em> in <a href="#appx">appendix A</a>).
Those checks are made over state that only exists while the program runs: which objects are alive,
which references reach them and in which mode, which frame owns what. Something has to hold that
state and make the checks, and that is the runtime. A conforming compiler is native code for the
program plus a runtime holding the dynamic part of the specification's state.""")
b += p("""The runtime also does what any language's runtime does: it starts the program, ends it, runs
destructors when a fault unwinds the stack, starts threads, and carries the standard library's calls
to the operating system (files, sockets, processes, clocks, random bytes).""")
b += p("""The runtime is written in C, not in CobaltC (D-0326). Every other tool is CobaltC. Three
things a runtime needs are not in the language:""")
b += ul([
    "<strong>Code that calls no runtime.</strong> Compiled CobaltC asks the runtime to record its objects and check its accesses. A runtime written in CobaltC would ask itself: the function that checks a read would need its own reads checked. That takes a way of compiling with no runtime calls at all, which does not exist (D-0256 set a freestanding mode aside for the same reason).",
    "<strong>Mutable statics.</strong> The runtime's state is global: its tables, its allocator's lists, its lock. CobaltC has static atomics, and no general mutable `static`.",
    "<strong>Thread-local storage.</strong> Each thread has a stack of frames and scopes of its own, and the language has no way to declare one.",
])
b += p("""C costs nothing extra here: C is what CobaltC compiles to, so a C compiler is already needed,
and the runtime is not a second toolchain.""")

# ---------------------------------------------------------------- shape

b += h3("The shape of a compiled program", anchor="rt-shape")
b += p("""`cobc prog.cb` checks the program, writes one C file in a directory of its own, and runs the C
compiler on it. `--keep-c` keeps that file beside the executable (`prog.c`), writing over a file there
only when `cobc` wrote it, and no output ever replaces one of the program's own files (its source, its
modules, a C file it names with `extern`); `--emit-c` writes it and compiles nothing,
`--no-line-directives` leaves out the `#line` lines so that the C reads straight through, and `-v`
prints the C compiler's command and which form the program was lowered in. The file has these parts,
in this order:""")
b += ol([
    "<strong>The form's macros</strong>, when the program is lowered in a form that needs them: `CB_PATHLESS`, `CB_DECIDED`, `CB_QUIET_CELLS` (see <a href=\"#rt-forms\">the forms</a>).",
    "<strong>The whole text of `cbrt.h`</strong>, copied in, so the file includes nothing but the C library's headers. Everything the generated C may say to the runtime is declared there, and nothing else is exported by the library.",
    "<strong>The program's types</strong> as C structs, each with a `_Static_assert` on its size and alignment against the layout the specification gives.",
    "<strong>String literals</strong> as byte arrays (`cb_s_…`).",
    "<strong>Prototypes</strong>, then the <strong>call thunks</strong> for `fn` values and the thread bodies for each `spawn`.",
    "<strong>The type descriptors</strong>: `cb_types[]`, one `cb_type` for each type the runtime may have to end or copy without seeing its source, and `cb_type_ids[]`, each one's 32-bit identifier. A call that names a type (`cb_new(addr, ty, …)`) passes that identifier.",
    "<strong>One C function for each function</strong> the program reaches, generic ones once for each set of types (`f_name`, and for a function with reference parameters sometimes two bodies and a dispatcher: `f_name__dir`, `f_name__chk`).",
    "<strong>The start-up text</strong>: two thread-locals the runtime reads through functions the program defines (`cb_at_pos`, the statement now executing, and `cb_stack_floor`), then `cb_program` and C's `main`.",
])
b += plain("""
static void cb_program(void)
{
    cb_init(cb_files, 1, cb_types, 4, cb_type_ids);
    cb_frame_push();
    void (*volatile cb_main_fn)(void) = f_main;
    cb_main_fn();
    cb_frame_pop();
    cb_terminate_ok(0);
}
int main(int argc, char **argv)
{
    cb_set_args(argc, argv);
    cb_run_main(cb_program);
}
""", "the end of every generated program")
b += p("""`cb_run_main` moves the program onto a large stack of its own, whose limit every function's
`cb_stack_check` tests, and calls `cb_program`. `cb_init` hands the runtime the file names, for
reports, and the type descriptors. `main` runs inside an outermost frame, and `cb_terminate_ok`
writes out what is still buffered and ends the process with `main`'s value.""")
b += p("""The link line is `cc … prog.c -L DIR -lcbrt -lpthread -ldl -lm -Wl,--gc-sections`, where `DIR`
is where `cobc` itself is. The library is one relocatable object whose only global names are the
interface's (`impl/cbrt/tools/exports.txt`), so the runtime's own names never meet a program's C at
link time; it is compiled with a section for each function, so a program links only the parts it
reaches. `cobc` refuses a `libcbrt.a` built from other sources than the ones it was built with: the
library carries a stamp, a hash of its sources, and the compiler knows its own.""")

# ---------------------------------------------------------------- state

b += h3("What the runtime holds", anchor="rt-state")
b += p("""The runtime's state is the dynamic part of the state the specification describes, kept over
real addresses. The generated C owns the storage: a local is a C local, a `Vec`'s buffer is a block
the program allocated. The runtime never holds a value, only records about values.""")
b += table(["Record", "What it says", "Made by", "Ended by"], [
    ["<strong>Object</strong>", "A value at an address: its type, whether it is initialised, whether it is a resource, and what holds it: a frame (a binding), a statement scope (a temporary), nothing (in flight across a call, or lying over raw storage). Named by a 64-bit id.", "`cb_new`, `cb_new_uninit`, `cb_reclaim`", "its frame's or scope's end, `cb_destroy`, `cb_consume`, `cb_absorb`, `cb_release`"],
    ["<strong>Access path</strong>", "One way of reaching an object: the object, the steps from it (a field, an index, an enum's payload, a range), shared or exclusive, whether it is still valid, the paths it was formed from, and how many places hold it. A reference's identity. Named by a 64-bit <em>token</em>.", "`cb_bind` (an object's root path), `cb_borrow` and its relatives, `cb_lock`", "the end of what holds it; a conflicting access makes it invalid"],
    ["<strong>Frame</strong>", "An entry on the thread's stack: the objects it owns, in the order they were bound, and the paths it holds.", "`cb_frame_push` and its relatives", "`cb_frame_pop`: its objects end last bound first, destructors run"],
    ["<strong>Statement scope</strong>", "An entry on the same stack for one statement: its temporaries and the paths formed in it that nothing has taken.", "`cb_stmt_push`", "`cb_stmt_pop`"],
    ["<strong>Reference slot</strong>", "For a memory cell that holds a reference: the token of the reference stored there. In memory a reference is its 8-byte address alone; the token is kept here, keyed by the cell's address.", "`cb_store_ref`, `cb_hold_ref`, `cb_copy_datum`", "the end of the object the cell lies in; `cb_deallocate`"],
    ["<strong>In flight</strong>", "A queue for each thread of what is between a caller and a callee: objects, references, values holding references, in argument order.", "`cb_send` and its relatives", "`cb_recv` and its relatives"],
    ["<strong>Objects over raw cells</strong>", "Objects that lie in storage the program allocated (a `Vec`'s elements, a `Box`'s contents), found by address, with a count for each page so that the usual answer, none here, costs a load.", "`cb_reclaim`, `cb_elem_borrow`", "`cb_release`, `cb_deallocate`"],
    ["<strong>Threads and locks</strong>", "Each thread's state and result, each mutex's holder, what a thread was lent, and under `--explore` whose turn it is.", "`cb_spawn`, `cb_lock`", "`cb_join`, the guard's end"],
    ["<strong>Noted locals</strong>", "For a function whose locals have no object: a table of their addresses and types, read only if a fault unwinds through the function, to run their destructors in order.", "`cb_notes_push`", "`cb_notes_pop`"],
])
b += p("""Each running thread has a heap of records of its own, so two threads that share nothing do
not wait for each other; an id carries its heap's number. A program's first thread, while no other
has been started, takes none of the runtime's locks. Once threads run, every access to memory two
threads can reach is checked under the runtime's lock before it is made.""")

b += h4("References and tokens")
b += p("""A reference in a register, as a parameter, a result or a temporary, is a `cb_ref`: the address
and the token of its path. Stored into memory it is the address alone, and `cb_store_ref` and
`cb_load_ref` keep and find its token by the cell's address. That keeps a reference 8 bytes wide in
every struct and array, as the specification's layout says, while every access still knows which
path it is made through. A token is one of four things:""")
b += table(["Token", "Meaning", "What a check through it costs"], [
    ["`0`", "No record. Every token of a program lowered without paths; also a reference the compiler proved needs none.", "nothing: the entries are macros that expand to nothing"],
    ["`CB_LIGHT | CB_ROOT | obj`", "The owner's own binding of object `obj`.", "one byte read (`cb_marks`): no call while no recorded path is on the object"],
    ["`CB_LIGHT | obj`", "A <em>light</em> reference: formed in a function the static pass decided, recorded nowhere.", "a test of the token's bits, no call"],
    ["anything else", "The id of a recorded path.", "a call into the runtime"],
])
b += p("""The test is `cb_vouched`, inline in the header: `cb_read`, `cb_write`, `cb_borrow` and their
relatives are macros over inline functions (`cb_read_i`, …) that call the runtime only for a token
that is not vouched for. A light reference becomes a recorded one where the record is needed: when
it is stored into data (`cb_hold_ref`) or arrives in a function that left a conflict to the run
(`cb_rec_ref`).""")

# ---------------------------------------------------------------- inline

b += h3("What the generated C does itself", anchor="rt-inline")
b += p("""Most of what a program does never reaches the runtime:""")
b += ul([
    "<strong>Loads and stores.</strong> `cb_read` and `cb_write` check that an access may be made; the C statement after them reads or writes the value. The runtime never touches the program's values on an access.",
    "<strong>Arithmetic.</strong> A checked `+` is `__builtin_add_overflow` and a branch; division tests for zero and for `MIN / -1`; a shift tests its amount. Only the failing branch calls the runtime (`cb_fault`).",
    "<strong>Bounds.</strong> An index is compared with the length in line (`cb_index_check`, or the comparison written out), so the C compiler can drop the test where a loop's condition already proves it.",
    "<strong>Layout.</strong> Field offsets, enum tags, array elements and conversions are plain C.",
    "<strong>The stack limit.</strong> `cb_stack_check` compares the frame's address with a thread-local.",
    "<strong>Where the program is.</strong> `cb_at(file, line)` is a store to a thread-local, read by the runtime only when a fault needs a location that was not passed to it.",
    "<strong>Small allocations.</strong> A block of up to 1,024 bytes is popped from, and pushed onto, a list for its size class in line (`cb_bins`), while the program has one thread.",
    "<strong>Formatting and text helpers.</strong> A literal format's common specifiers, byte comparison, hashing, searching, bit counts and rotations are inline functions of the header.",
])

# ---------------------------------------------------------------- worked example

b += h3("One function, three forms", anchor="rt-example")
b += p("""The clearest way to see the interface is to read what `cobc` writes for a small program. This
one makes a struct, passes an exclusive reference to it, and reads a field through a shared
reference:""")
b += code("""
import std;

struct Point
{
    i32 x;
    i32 y;
}

fn bump(ref<Point, exclusive> p)
{
    p.x = p.x + 1;
}

fn main()
{
    Point pt = Point { .x = 1, .y = 2 };
    bump(&mut pt);
    ref<Point, shared> r = &pt;
    printf("%v\\n", r.x);
}
""", title="point.cb", expect="ok", output="2\n", section="appb", slug="rt-point")
b += p("""`cobc --bootstrap-form` writes the general form, with every record made and every check
asked for. It is what the rules say, statement by statement, and the other forms are this one with
parts proved away. Here is `bump`, and the first three statements of `main`:""")
b += plain("""
void f_bump(cb_ref p_p)
{
    cb_stack_check();
    cb_frame_push_rec();
    p_p.tok = cb_rec_ref(p_p.tok, p_p.p, 3936939825u, CB_EXCLUSIVE, 1u, CB_NOLOC, 0);
    void *l_p_p = p_p.p;
    uint64_t o1 = cb_new(&l_p_p, 1886034091u, CB_NOLOC, 0);
    cb_hold_ref(&l_p_p, p_p.tok, p_p.p, 3936939825u, CB_EXCLUSIVE, CB_NOLOC, 0);
    uint64_t root2 = cb_bind(o1);
    cb_at(0u, 11u);
    uint64_t tk3 = cb_load_ref(&l_p_p);
    uint64_t tk4 = cb_load_ref(&l_p_p);
    cb_read(tk4, (cb_proj[]){ { CB_FIELD, 0u } }, 1, 0u, 11u);
    int32_t t5 = (*(struct s_Point *)l_p_p).x;
    int32_t t6 = ((int32_t)1ULL);
    int32_t t7 = ({ int32_t _r; if (__builtin_add_overflow(t5, t6, &_r)) cb_fault("diag.arith-overflow", 0u, 11u); _r; });
    cb_write(tk3, (cb_proj[]){ { CB_FIELD, 0u } }, 1, 0u, 0u, 11u);
    (*(struct s_Point *)l_p_p).x = t7;
    cb_frame_pop();
    return;
}

void f_main(void)
{
    cb_stack_check();
    cb_frame_push_rec();
    cb_at(0u, 16u);
    cb_stmt_push_at(0u, 16u);
    int32_t t2 = ((int32_t)1ULL);
    int32_t t3 = ((int32_t)2ULL);
    struct s_Point st4;
    st4.x = t2;
    st4.y = t3;
    struct s_Point l_pt_1;
    l_pt_1 = st4;
    uint64_t o5 = cb_new(&l_pt_1, 3936939825u, 0u, 16u);
    uint64_t root6 = cb_bind(o5);
    cb_stmt_pop();
    cb_at(0u, 17u);
    cb_stmt_push_at(0u, 17u);
    uint64_t tok7 = cb_borrow(root6, NULL, 0, CB_EXCLUSIVE, 0u, 17u);
    cb_ref r8 = ((cb_ref){ (void *)&(l_pt_1), tok7 });
    f_bump(r8);
    cb_stmt_pop();
    cb_at(0u, 18u);
    cb_stmt_push_at(0u, 18u);
    uint64_t tok10 = cb_borrow(root6, NULL, 0, CB_SHARED, 0u, 18u);
    cb_ref r11 = ((cb_ref){ (void *)&(l_pt_1), tok10 });
    void * l_r_9;
    l_r_9 = (r11).p;
    cb_hold_ref(&l_r_9, (r11).tok, (r11).p, 3936939825u, CB_SHARED, CB_NOLOC, 0);
    uint64_t o12 = cb_new(&l_r_9, 3175594122u, 0u, 18u);
    uint64_t root13 = cb_bind(o12);
    cb_stmt_pop();
    …
    cb_frame_pop();
    return;
}
""", "cobc --bootstrap-form --keep-c --no-line-directives point.cb")
b += p("""Read with the table of records beside it:""")
b += ul([
    "<strong>Line 16, `Point pt = …`.</strong> The C builds the struct in a C local. `cb_new` tells the runtime an object of type `Point` now lies at `&l_pt_1`, as a temporary of this statement; `cb_bind` makes the frame adopt it and returns its root path's token, `root6`, through which every later access to `pt` is made. `cb_stmt_pop` ends the statement's scope, which by then holds nothing.",
    "<strong>Line 17, `bump(&mut pt)`.</strong> `cb_borrow` checks that an exclusive reference may be formed from `pt` now (no other live path conflicts) and mints a path; the token and the address make the `cb_ref` passed to `f_bump`. The path is held by the statement, and `cb_stmt_pop` ends it when the call has returned.",
    "<strong>Inside `bump`.</strong> `cb_frame_push_rec` opens a frame that records the references arriving in it, and `cb_rec_ref` records the parameter. The parameter is itself a local that holds a reference: an object (`cb_new`, `cb_bind`) whose one cell holds the token (`cb_hold_ref`). `p.x + 1` loads the token back (`cb_load_ref`), asks `cb_read` whether field 0 may be read through it, and then reads it with a C expression. The sum is checked in line. `cb_write` asks whether field 0 may be written, and a C assignment writes it. `cb_frame_pop` ends the parameter's object and what the frame held.",
    "<strong>Line 18, `ref<Point, shared> r = &pt`.</strong> A shared borrow, stored into the local `r`: the cell gets the address, the runtime gets the token for that cell, and `r` is an object the frame owns.",
])
b += p("""Without the option, `cobc` first asks whether the static pass decided the program: whether
every conflict between references that could arise was proved or refuted before the run. This
program is decided, and `cobc -v` says "lowered pathless and decided". All of the above is then
proved unnecessary, and the same two functions are:""")
b += plain("""
CB_SMALL void f_bump__dir(cb_ref p_p)
{
    enum { cb_lf = 1 };
    void *l_p_p = p_p.p;
    int32_t t1 = (*(struct s_Point *)l_p_p).x;
    int32_t t2 = ((int32_t)1ULL);
    int32_t t3 = ({ int32_t _r; if (__builtin_add_overflow(t1, t2, &_r)) cb_fault("diag.arith-overflow", 0u, 11u); _r; });
    (*(struct s_Point *)l_p_p).x = t3;
    return;
}

void f_main(void)
{
    enum { cb_lf = 1 };
    int32_t t2 = ((int32_t)1ULL);
    int32_t t3 = ((int32_t)2ULL);
    struct s_Point st4;
    st4.x = t2;
    st4.y = t3;
    struct s_Point l_pt_1;
    l_pt_1 = st4;
    cb_at(0u, 17u);
    void *ad5 = (void *)&(l_pt_1);
    f_bump__dir(((cb_ref){ ad5, 0 }));
    cb_at(0u, 18u);
    cb_ref r7 = ((cb_ref){ (void *)&(l_pt_1), 0 });
    void * l_r_6;
    cb_ref r8 = r7;
    l_r_6 = (r8).p;
    uint64_t tk9 = (r8).tok;
    cb_frame_hold(tk9);
    cb_at(0u, 19u);
    cb_str t10 = ((cb_str){ cb_s_f788bc38, 3u });
    uint64_t tk11 = tk9;
    cb_read(tk11, (cb_proj[]){ { CB_FIELD, 0u } }, 1, 0u, 19u);
    int32_t t12 = (*(struct s_Point *)l_r_6).x;
    …
    return;
}
""", "cobc --keep-c --no-line-directives point.cb")
b += p("""No frame, no scope, no object, no path. Every token is `0`, and the three entries still
written, `cb_frame_hold`, `cb_read` and (in other programs) `cb_write`, are macros that expand to
nothing when `CB_PATHLESS` is defined, so the C compiler sees a function that reads a field, adds
with an overflow test and writes the field. What is left of the runtime in this program is the
start-up text, `cb_at`'s store before a call that may fault, the overflow test's `cb_fault`, and
`cb_write_out` under `printf`. The executable is 54 KB; in the bootstrap form it is 225 KB, because
it links the object and path machinery it calls.""")
b += p("""A program the static pass does not decide keeps its paths, but only where they are needed.
This one stores a reference to a local through a parameter, so the local's end leaves a dead
reference behind, which only the run can find:""")
b += code("""
import std;

struct Holder
{
    ref<i32, shared> r;
}

fn stash(ref<Holder, exclusive> h)
{
    i32 local = 5;
    h.r = &local;
}

fn main()
{
    i32 outer = 0;
    auto held = Holder { .r = &outer };
    stash(&mut held);
    printf("%v\\n", *held.r);          // ✗ diag.stale-binding (dynamic)
}
""", title="stash.cb", expect="diag.stale-binding", section="appb", slug="rt-stash")
b += p("""`cobc -v` says why the record is kept: "kept paths: `stash`, stash.cb:11: a reference is
stored where this frame does not hold it (D-0290); lowered with holders (D-0292)". The function
that left the conflict, and the statement of `main` that reads the reference back:""")
b += plain("""
static void f_stash(cb_ref p_h)
{
    cb_frame_push_rec();
    void *l_p_h = p_h.p;
    p_h.tok = cb_rec_ref(p_h.tok, p_h.p, 458631599u, CB_EXCLUSIVE, 1u, CB_NOLOC, 0);
    cb_at(0u, 10u);
    cb_stmt_push_at(0u, 10u);
    int32_t l_local_1;
    l_local_1 = ((int32_t)5ULL);
    uint64_t o2 = cb_new(&l_local_1, 3400004773u, 0u, 10u);
    uint64_t root3 = cb_bind(o2);
    cb_stmt_pop();
    cb_at(0u, 11u);
    cb_stmt_push_at(0u, 11u);
    uint64_t tk4 = p_h.tok;
    uint64_t tok5 = cb_borrow(root3, NULL, 0, CB_SHARED, 0u, 11u);
    cb_ref r6 = ((cb_ref){ (void *)&(l_local_1), tok5 });
    cb_write(tk4, (cb_proj[]){ { CB_FIELD, 0u } }, 1, 0u, 0u, 11u);
    (*(struct s_Holder *)l_p_h).r = (r6).p;
    cb_hold_ref(&(*(struct s_Holder *)l_p_h).r, (r6).tok, (r6).p, 3400004773u, CB_SHARED, CB_NOLOC, 0);
    cb_stmt_pop();
    cb_frame_pop();
    return;
}

    /* in f_main, line 19 */
    cb_read(root10, (cb_proj[]){ { CB_FIELD, 0u } }, 1, 0u, 19u);
    cb_ref r14 = ((cb_ref){ l_held_4.r, cb_load_ref(&l_held_4.r) });
    cb_read(r14.tok, NULL, 0, 0u, 19u);
    int32_t t15 = (*(int32_t *)r14.p);
""", "cobc --keep-c --no-line-directives stash.cb")
b += p("""`cb_hold_ref` records that the cell `held.r` now holds the path to `local`. `cb_frame_pop`
ends `local`, and with it every path to it: the path is marked invalid, and the token in the cell
stays. Back in `main`, `cb_load_ref` finds that token, and `cb_read` finds its path invalid and
calls `cb_fault`. The address in the cell is never followed:""")
b += plain("""
fault[diag.stale-binding]: this value is no longer valid
  --> stash.cb:19:5
   |
19 |     printf("%v\\n", *held.r);          // ✗ diag.stale-binding (dynamic)
   |     ^^^^^^
   = repair: use the current holder after a move, or give the binding a new value first (`x = e`, `spec/11` `[Assign-Reestablish]`); do not keep a reference past its referent's or its holder's scope; re-index after a `Vec` reallocation
""", "standard error; exit status 1")
b += p("""`cobc --report-tracking stash.cb` lists these calls by source line without compiling, which is
the quick way to see what a program of your own still asks of the runtime.""")

# ---------------------------------------------------------------- forms

b += h3("The forms a program is lowered in", anchor="rt-forms")
b += p("""The same program can be written as C in five ways. All of them behave the same: the same
output, the same fault at the same place. They differ in how much the runtime is asked, and the
project's tests run the suites in several of them and compare.""")
b += table(["Form", "Chosen when", "`cobc -v` says", "What reaches the runtime"], [
    ["<strong>Pathless and decided</strong>", "the static pass left no conflict to the run (the default for such a program)", "lowered pathless and decided", "`CB_PATHLESS` and `CB_DECIDED` are defined: the path entries are constants, a value holding references is plain data, most locals have no object, and a function has a frame or a table of noted locals only where an unwind would need one. Bounds, arithmetic and the other value checks stay."],
    ["<strong>Pathless</strong>", "the same program with `COBALTC_DECIDED=0` in the compiler's environment", "lowered pathless", "`CB_PATHLESS` alone: no paths, but objects, frames, statement scopes and values in flight as in the general form."],
    ["<strong>Kept paths, with holders</strong>", "some function left a conflict to the run (the default for such a program)", "kept paths: <em>the first conflict, with its file and line</em>; lowered with holders", "A function the static pass decided opens `cb_frame_push_fn` and forms light references (`enum { cb_lf = 1 };` at its start says so to the header's inline tests). A function that left a conflict opens `cb_frame_push_rec` and records what arrives (`cb_rec_ref`). A reference stored into data is recorded where it is stored (`cb_hold_ref`)."],
    ["<strong>Every check</strong>", "`cobc --all-checks`", "kept paths: every check was asked for", "No proof about a function's reference parameters is used: every function records, and every access through a parameter is checked where it is made."],
    ["<strong>Bootstrap form</strong>", "`cobc --bootstrap-form`", "kept paths: every check was asked for", "The general form with no elision: every local an object, every stored reference a slot, a stack check in every function. The reference the code generator is held to; much slower."],
])
b += p("""`CB_QUIET_CELLS` is defined as well, in any form, when no call in the program can put an
object over raw storage or start a second thread: the tests for an object over a `Vec`'s elements
are then constants.""")
b += p("""The header is how the forms differ without the generated C differing much. Under
`CB_PATHLESS` the entries `cb_borrow`, `cb_read`, `cb_valid`, `cb_store_ref`, `cb_load_ref`,
`cb_frame_hold` and their relatives are redefined as macros that evaluate their token argument and
give `0` or nothing; `cb_write` becomes `cb_write_pathless`, which keeps the one check a decided
program still needs at a write through a reference (the target is a resource that still owns
something: `diag.overwrite-of-live-resource`). Under `CB_DECIDED` the entries for references and
reference data in flight are nothing too, and `cb_init` first calls `cb_set_decided`. The C
compiler then deletes the calls and whatever was computed only for them.""")
b += p("""A function with reference parameters is sometimes written twice, when whether its checks
are needed depends on its arguments: `f__dir`, the direct body, with no check on its parameters, and
`f__chk`, the checked one. A small dispatcher chooses: the direct body when the program has one
thread, or the tokens are `0`, or `cb_direct_ok` vouches for the paths (valid, and over fresh storage). `cb_paths_disjoint`, `cb_paths_arrive` and `cb_frozen_ok` are the same kind of
question for other shapes of parameter.""")

# ---------------------------------------------------------------- the calls

b += h3("The interface, name by name", anchor="rt-calls")
b += p("""Every name `cbrt.h` declares is below, grouped by what it is for. "The runtime" in the third
column means code in `libcbrt.a`; a name marked <em>inline</em> is defined in the header and compiled
into the program, and calls the library only where its row says so. The words in square brackets
are the specification's rules, as the header's own comments cite them. Arguments named `file` and
`line` are the source location a fault there is reported at (`CB_NOLOC` for none); `ty` is a type's
identifier; `obj` an object's id; `tok`, `root` and `base` are tokens.""")

b += h4("Types, constants and shared variables")
b += table(["Name", "What it is"], [
    ["`cb_str`", "A `str`: a pointer and a length, 16 bytes."],
    ["`cb_unit`", "The empty value `()`: an empty struct."],
    ["`cb_i128`, `cb_u128`", "The 128-bit integers, as the C compiler's `__int128`."],
    ["`cb_ref`", "A reference outside memory: `{ void *p; uint64_t tok; }`."],
    ["`cb_proj`", "One step of a place from its base: `CB_FIELD` i, `CB_INDEX` i or `CB_PAYLOAD`. `&s.f[i]` passes an array of two."],
    ["`cb_type`, `cb_field`", "A type descriptor and one of its fields: name, size, kind (`CB_K_SCALAR`, `CB_K_STRUCT`, `CB_K_ENUM`, `CB_K_ARRAY`, `CB_K_REF`, `CB_K_FN`, `CB_K_HANDLE`, `CB_K_GUARD`, `CB_K_OTHER`), whether it is a resource, whether it holds references, its destructor's address, its fields or variants or element."],
    ["`cb_fnbox`", "What a `fn` value's 8-byte handle points at: the code, and for a closure the object holding its captures."],
    ["`cb_fmt_arg`", "One argument of a `printf`-family call that the general formatter (`cb_format`) handles."],
    ["`struct cb_pos`, `cb_at_pos`", "The statement now executing, a thread-local the generated program defines."],
    ["`struct cb_note`", "One entry of a function's table of noted locals: address, moved flag, type, whether bound now."],
    ["`CB_SHARED`, `CB_EXCLUSIVE`", "A borrow's mode."],
    ["`CB_LIGHT`, `CB_ROOT`, `cb_lf`", "A token's kind bits; `cb_lf` is 0 in the header and shadowed by 1 at the start of a decided body, where a borrow from a vouched token is light."],
    ["`cb_marks`, `cb_inline_off`", "For each heap one byte for each object, nonzero while a recorded path is on it; and a flag that sends every entry to the runtime (set under `COBALTC_RECORD`)."],
    ["`cb_live_threads`, `cb_threaded`, `cb_sched_flag`", "How many threads run (1: this one is alone); whether a thread was ever started; whether a schedule runs the threads (`--explore`)."],
    ["`cb_reclaimed_entries`, `cb_reclaimed_pages`", "The count of objects over raw cells, in all and by page: the gates of the element fast paths."],
    ["`cb_slots_used`", "1 once any reference was stored into data: until then no cell holds one, and a copy or a deallocation has no slot to look for."],
    ["`cb_bins`, `cb_alloc_gate`, `cb_alloc_traced`", "The free lists of small blocks by size class; the flag that closes the inline allocation paths (a thread was started, an over-aligned block was made, a sanitizer is on, a trace records allocations); whether the replay trace records each allocation."],
    ["`cb_stack_floor`", "This thread's stack limit, a thread-local the generated program defines."],
    ["`CB_SMALL`, `CB_NOTYPE`, `CB_NOLOC`, `CB_OWN_STACK`, `CB_SANITIZED`", "A small function's linkage; no type; no location; whether the program runs on a stack of its own (not under a sanitizer); whether the C is compiled under a sanitizer, in which case every block is `malloc`'s so that the sanitizer sees it."],
])

b += h4("Starting, ending, faulting")
b += calls([
    ["`cb_set_args`", "first thing in C's `main`", "keeps `argc` and `argv` for `arg`, `arg_count` and `Args`"],
    ["`cb_run_main` (inline), `cb_run_program`", "C's `main`", "runs `cb_program` on a large stack of its own, or on a thread under a sanitizer; does not return"],
    ["`cb_init`, `cb_set_decided`", "start of `cb_program`", "[Program]: takes the file names and type descriptors and sets up the state; `cb_set_decided` first, in a decided program"],
    ["`cb_terminate_ok`", "end of `cb_program`", "writes out buffered output and ends the process with `main`'s value"],
    ["`cb_exit`", "`std::exit`", "unwinds this thread as a fault does, reports nothing, ends with the status"],
    ["`cb_unittest_runner`", "`cobc --unittest`'s runner, before its first test", "an `exit` reached during a test then reports the test FAILED and ends the run with status 1 (report 374)"],
    ["`cb_fault`, `cb_fault_msg`, `cb_fault_index`, `cb_fault_range`, `cb_fault_arith`, `cb_fault_arith_r`, `cb_fault_arith_c`, `cb_fault_op`, `cb_fault_narrow`, `cb_fault_to_int`", "a check written in line has failed (overflow, bounds, `unwrap`, `assert`)", "[Fault-Unwind]: see <a href=\"#rt-fault\">a fault from call to exit</a>; `cb_fault_msg` carries `assert`'s formatted message; the others the values the check compared: the index and the length, the range and the length, the operands and the type (of `+ - *`, of `-a`, of a division and of a shift), the value and the type it does not fit"],
    ["`cb_at` (inline)", "before a statement that may fault without a location of its own", "stores the file and line in `cb_at_pos`"],
    ["`cb_where`, `cb_set_where`, `cb_set_stack_floor`", "defined by the generated program, called by the runtime", "read and set the two thread-locals, which live in the program so that its stores to them are plain"],
    ["`cb_stack_check` (inline), `cb_stack_exhausted`", "a function's entry", "[Call]'s stack limit: below the floor is `diag.stack-exhausted`, raised as a fault"],
    ["`cb_index_check` (inline)", "an index into an array, slice or `Vec`", "the comparison, and `cb_fault(\"diag.index-out-of-bounds\")`"],
])

b += h4("Frames and statement scopes")
b += calls([
    ["`cb_frame_push`, `cb_frame_push_at`", "a block or a function body begins", "[Block-Enter], [Call]: pushes a frame; `_at` keeps the block's location for a fault in a destructor at its end"],
    ["`cb_frame_push_fn`", "a decided function's body, in a program that keeps paths", "a frame whose references are light"],
    ["`cb_frame_push_rec`", "a body the static pass left a conflict in", "a frame that records every reference formed or arriving in it"],
    ["`cb_frame_pop`", "the block or body ends, on every way out", "[Block-Exit]: ends the frame's objects last bound first ([Destroy] for a resource: its destructor, then its parts; [Object-End] for the rest) and drops the paths it held"],
    ["`cb_stmt_push`, `cb_stmt_push_at`", "a statement that makes temporaries or paths begins", "[Stmt-Enter]: a scope for them"],
    ["`cb_stmt_pop`", "the statement ends", "[Stmt-Exit]: ends the scope's temporaries and the paths nothing took"],
    ["`cb_frame_here`", "a binding that may be given a new value after a move", "the current frame's id, kept for `cb_rebind`"],
    ["`cb_frame_hold`, `cb_frame_unhold`", "a reference local that has no object of its own is bound, or ends early at its last use", "the frame holds the path until [Block-Exit], or stops holding it ([Ref-Binding-Last-Use])"],
])

b += h4("Objects")
b += calls([
    ["`cb_new`", "a value now lies in a C local or temporary", "[Object-Establish]: an initialised object at that address, a temporary of this statement; returns its id"],
    ["`cb_new_uninit`", "`T x;` with no value", "[Let-Uninit]: the same, not yet initialised"],
    ["`cb_bind`", "a temporary becomes a binding", "[Binding-Form]: the current frame adopts it; returns the root path's token"],
    ["`cb_rebind`", "`x = e` after `x` was moved from", "a new object at the address, bound in the binding's own frame"],
    ["`cb_move_to`", "the C copied a value's bytes to another local", "the record follows: the same object, now at the new address"],
    ["`cb_temp_path`", "a temporary closure is called", "[Temp-Root]: an owner's path to a temporary"],
    ["`cb_take`", "a binding is moved from", "[Authority-Transfer]: checks that nothing else reaches it and that the binding still owns it; the root path ends and the object is in flight"],
    ["`cb_absorb`, `cb_absorb_refs`", "a value is moved into a struct, an enum or a container", "[Relocate-In]: its record ends without destruction, since it is now part of the container; `_refs` for plain data holding references, whose slots stay"],
    ["`cb_result`", "a block's or statement's value leaves it", "[Stmt-Exit]'s re-stamp: the object now belongs to the enclosing scope"],
    ["`cb_destroy`, `cb_destroy_via`", "`drop(x)`, `drop(*r)`", "[Destroy]: valid, nothing else reaching it, authority held; then the destructor, the parts, the object's end"],
    ["`cb_consume`", "a temporary is destroyed now, not at the statement's end", "[Destroy] of the temporary"],
    ["`cb_end_moved_out`", "a value was taken apart by a pattern", "[Relocate-Out]: the container ends without destroying the parts that moved out"],
    ["`cb_end_binding`", "a reference binding's last use", "[Ref-Binding-Last-Use]: its object and path end there"],
    ["`cb_forget_obj`", "a callee keeps a received value in a local with no object", "the record ends, the value stays with the C"],
    ["`cb_drop_local`, `cb_drop_value`", "a local with no object ends; a plain temporary holding `fn` values ends", "destroys the value there as its binding's end would, from its type descriptor"],
    ["`cb_owns`", "an assignment over a place whose type may own something", "whether the value at the address owns a resource now, for [Write]'s test"],
])

b += h4("Access paths and checked accesses")
b += calls([
    ["`cb_borrow`", "`&x`, `&mut x`, `&s.f`, `&a[i]`", "[Borrow]: the source is valid, the mode is no stronger than the source's, no live path conflicts, the thread may reach it; mints a path held by this statement and returns its token"],
    ["`cb_borrow_range`", "`&a[lo..hi]`", "[Slice-Form]: a borrow whose last step is a range"],
    ["`cb_borrow_unstamped`, `cb_call_self`, `cb_call_self_fn`, `cb_call_done`", "a call through a closure", "a borrow in no statement scope: the closure's `self` for the call (shared for a closure whose copies share its box), ended by `cb_call_done` when nothing took it"],
    ["`cb_borrow_unminted`, `cb_borrow_range_unminted`, `cb_borrow_check`", "a reference passed straight to a parameter the callee only uses directly, or `&v` straight into `Vec::index`", "[Borrow]'s checks without minting a path"],
    ["`cb_read`", "before a load from a place that a path reaches", "[Read]'s premises: valid, no conflicting exclusive path, the thread may reach it. The C does the load."],
    ["`cb_write`", "before a store to such a place", "[Write]'s premises: valid, exclusive, no conflicting path, and no live resource overwritten when the target is one. The C does the store."],
    ["`cb_write_quiet`", "a store over a value whose end only frees memory", "[Write-Quiet-Replace]: destroys the old value, then allows the write"],
    ["`cb_point_check`", "a call whose callee is known to access something the caller also lent", "the access the callee's summary declares, checked at the call, with the call's argument paths counted as held"],
    ["`cb_valid`", "`swap` of two places", "[Swap-Places]: valid, with no conflict check"],
    ["`cb_vouched`, `cb_read_i`, `cb_write_i`, `cb_borrow_i`, `cb_borrow_unstamped_i`, `cb_borrow_check_i`, `cb_borrow_unminted_i`, `cb_borrow_range_unminted_i` (all inline)", "every use of the entries above, which are macros over these", "the token's kind tested in line; the runtime is called only for a recorded path, a marked owner, or under `COBALTC_RECORD`"],
    ["`cb_write_pathless` (inline)", "`cb_write` in a pathless program", "the live-resource test alone"],
    ["`cb_direct_ok`, `cb_paths_disjoint`, `cb_paths_arrive`, `cb_frozen_ok`, `cb_peek_datum_tok`", "a dispatcher choosing between a function's direct and checked bodies", "whether the arguments' paths are valid, pairwise disjoint and free of conflict as they arrive, so that the direct body may run; `cb_peek_datum_tok` reads the token of a value in flight without taking it"],
])

b += h4("References kept in memory")
b += calls([
    ["`cb_store_ref`", "a reference is stored into a cell", "[Write] of a reference value: the cell's slot gets the token, and the path gains a holder"],
    ["`cb_hold_ref`", "the same, where the compiler knows the referent's place", "a light reference becomes a recorded path as it is stored; a recorded one is stored as `cb_store_ref` stores it"],
    ["`cb_rec_ref`", "a reference arrives in a recording function, as a parameter or a call's result", "a light token becomes a recorded path, held by the frame or the statement; a recorded one is returned as it is"],
    ["`cb_load_ref`", "a reference is read out of a cell", "the token the cell's slot holds"],
    ["`cb_copy_datum`", "a value that holds references is copied", "copies each slot's hold with the bytes"],
    ["`cb_lend`, `cb_lend_fn`", "`spawn` with reference arguments, or a closure with reference captures", "[Thread-Lend]: every exclusive path in the argument block is lent to the thread that receives it"],
])

b += h4("Across a call")
b += p("""A C call passes the bytes. The records of what is passed travel beside them, in the thread's
queue of what is in flight, in argument order: the caller sends, the callee receives, and the same
for a result on the way back.""")
b += calls([
    ["`cb_send`, `cb_recv`", "a resource or other object passed or returned by value", "[Call]: the object is in flight; the receiver gets it, now at its own address, and binds it"],
    ["`cb_send_ref`, `cb_recv_ref`", "a reference argument", "the path is pending until the callee binds it, so it is not ended by the caller's statement"],
    ["`cb_result_ref`", "a statement's or call's reference result", "re-stamps the path into the enclosing scope"],
    ["`cb_send_datum`, `cb_recv_datum`", "a value holding references (a view, an `Option` of a reference, a struct of them)", "its slots' holds move with it"],
    ["`cb_recv_datum_tok`, `cb_send_tok_datum`, `cb_send_datum_none`", "a native body that returns one reference as data, or `None`, and its caller", "the token of such a value, taken or sent without a slot"],
])

b += h4("`fn` values and closures")
b += calls([
    ["`cb_fn_item`, `cb_fn_item_pure`", "a function's name used as a value", "the one box kept for that function; `_pure` when its reference parameters are only used directly"],
    ["`cb_fn_box`", "a closure expression", "a heap box that owns the closure's object of captures"],
    ["`cb_fn_pure`, `cb_fn_mark_pure`", "a native body deciding whether it may pass element references with no path", "whether the `fn` value keeps nothing of its arguments; marks a closure's code as such"],
    ["`cb_fn_copy`, `cb_fn_read`, `cb_fn_split`, `cb_fn_moved_out`", "a `fn` value is read from a place", "[Read]: copies the handle, cloning a closure's box (or sharing it, for a closure no copy can tell apart); or moves it out when the closure owns a resource, for one slot or for each `fn` slot of a value; `cb_fn_moved_out` empties the fn slots a raw read moved out"],
    ["`cb_fn_moved`", "a call through a slot that was emptied", "the fault for it"],
    ["`cb_fn_shared`, `cb_fn_shared_value`", "a `fn` value read through a shared reference", "checks that it holds no exclusive capture"],
    ["`cb_drop_fn`", "a `fn` value ends", "[Destroy] of a closure's box and the object it owns"],
])

b += h4("Raw storage, allocation and container elements")
b += p("""`Vec`, `Box`, `String` and the other containers are written in CobaltC, in `std`, over raw
storage (§20). Their elements are not objects while nobody refers to them: an element becomes an
object when it is reclaimed, usually because a reference to it is formed, and stops being one when
it is released. So the common case, an element read or written at once, is checked with a load of
a counter and nothing more. The specification lets an implementation realise a library function
natively (`spec/21` §0), and for some `std` functions that hot loops call (`Vec`'s element access,
its length and its end, sorts and searches over slices) the compiler writes the body directly as C
in place of compiling the CobaltC one: a <em>native body</em>. It makes only the checks that can
fail there, and takes the general path when an object lies over the cells it would touch.""")
b += calls([
    ["`cb_allocate`, `cb_allocate_at_once` (inline)", "`allocate`, a container growing", "[Allocate]: a small block from its class's list in line, else the runtime; 0 on failure"],
    ["`cb_deallocate`, `cb_deallocate_at_once` (inline), `cb_deallocate_plain`, `cb_deallocate_plain_at_once` (inline)", "`deallocate`, a container's end", "[Deallocate]: releases the objects over the cells and their slots, then frees the block; `_plain` for cells that never held a reference; in line when there is nothing to release"],
    ["`cb_reallocate_plain`, `cb_reallocate_plain_fast` (inline)", "a `Vec` of plain elements grows", "allocate, copy, deallocate as one step: `realloc`, or nothing when both sizes are of one class"],
    ["`cb_free_block`, `cb_block_free` (inline)", "the last step of a deallocation", "hands the block back, to its class's list in line when small"],
    ["`cb_drop_plain_buf` (inline)", "a `String` or `Vec` of plain elements with no object ends", "frees its buffer at once when no object lies over it, else `cb_drop_local`"],
    ["`cb_vec_drop_plain`", "`Vec::drop` for plain elements", "ends every element object still over the buffer"],
    ["`cb_copy_raw`, `cb_copy_raw_at_once` (inline)", "`copy_raw`", "[Copy-Raw]: `memmove`, and the slots of any references among the bytes move with them"],
    ["`cb_reclaim`", "`reclaim`", "[Reclaim]: an object over the raw cells at an address; its root token"],
    ["`cb_release`, `cb_release_plain`", "`release`", "[Release]: ends the reclaimed objects that start in the range"],
    ["`cb_raw_move_in`, `cb_raw_move_out`", "`*p = v` and a read of `*p` where the value is a resource", "[Rawptr-Move-In], [Rawptr-Move-Out]: the object's record goes into, or comes out of, raw storage"],
    ["`cb_elem_borrow`, `cb_elem_borrow_here`, `cb_elem_borrow_datum`", "`Vec::index` and its relatives return a reference to an element", "[Reclaim] and [Borrow] together: an object over the element and a path to it, in flight, recorded in the current scope, or sent as reference data"],
    ["`cb_elem_access` (inline), `cb_elem_access_if` (inline), `cb_elem_access_slow`", "`v[i]` read or written at once", "nothing while no object lies near the element (`cb_reclaimed_any`, `cb_reclaimed_near`); otherwise [Read]'s or [Write]'s premises on the object there"],
    ["`cb_reclaimed_any`, `cb_reclaimed_near`, `cb_cells_busy` (all inline), `cb_cells_held`", "the gates of the paths above", "whether any object lies over raw cells at all, on these pages, over exactly these cells"],
    ["`cb_live_at`, `cb_live_in`, `cb_live_in2`, `cb_live_in_q` (inline), `cb_live_in2_q` (inline)", "the entry of a native body", "whether a live object lies over the cells: if so the body takes its general, checked path"],
    ["`cb_gate`, `cb_slots`, `cb_blocks_from_malloc`", "inside the inline paths above", "read `cb_alloc_gate` and `cb_slots_used` (atomically under a sanitizer); `cb_blocks_from_malloc` is a constructor that closes the gate in a sanitized build"],
])

b += h4("Locals with no object")
b += p("""In a decided program most locals have no object, so `cb_frame_pop` has nothing to end, and
the generated C ends each local itself where its scope ends. If a fault unwinds through the
function, those C statements never run, so a function whose locals have destructors, or are freed,
notes them in a table on its C stack.""")
b += calls([
    ["`cb_notes_push`", "such a function is entered, or binds such a local", "registers the table for the unwind"],
    ["`cb_notes_pop`, `cb_notes_drop`", "the function returns; one temporary with a destructor ends", "the table, or the one entry, leaves"],
])

b += h4("Threads, mutexes and atomics")
b += calls([
    ["`cb_env_alloc`, `cb_hold`", "`spawn(f, a, b)`: the arguments are put in a block", "storage for the block, outside every object; a resource argument waits in it until the new thread binds it"],
    ["`cb_spawn`", "`spawn`", "[Spawn]: starts a thread on the body the compiler wrote for this `spawn`; the thread's id, which is the `handle`'s value"],
    ["`cb_thread_jmp`, `cb_thread_start`, `cb_thread_done`, `cb_env_forget`", "that body, in the new thread", "sets where a contained fault's unwind ends; registers the thread and binds its parameters from the block; [Thread-Body-Done]: the result is in the block; the block's reference cells stop counting as holders"],
    ["`cb_thread_failed`, `cb_thread_lent`", "the body after a contained fault; a thread lent an exclusive reference", "records the failure ([Fault-Contain]); marks the thread as one whose fault is never contained"],
    ["`cb_join`, `cb_join_plain`", "`join(h)`", "[Join]: waits, copies the result out, and returns its object (`_plain`: a result with no object)"],
    ["`cb_join_status`, `cb_cancel`, `cb_task_op`", "`std::try_join`, `is_finished`, `cancel`, a failure's text", "the supervision operations of §19"],
    ["`cb_lock`, `cb_lock_pathless`", "`lock(&m)`", "[Lock]: waits for the mutex (`diag.mutex-reentrant-lock` if this thread holds it) and returns the lock path's token, which the guard holds; `_pathless` in a program with no paths"],
    ["`cb_lock_bare`, `cb_unlock_bare`, `cb_unlock_signal`", "`std`'s own bodies, which lock with no guard object", "lock and unlock; `_signal` wakes waiters and unlocks in one step"],
    ["`cb_event_op`", "`Channel`'s waits and wake-ups", "an event's count: read, wait for a change, signal"],
    ["`cb_atomic_wait`, `cb_atomic_point`, `cb_spin_hint`", "`Atomic`'s wait and notify; any atomic operation under a schedule; a spin loop", "[Atomic-Wait], [Atomic-Notify]; a point where `--explore` may pass the turn; the processor's pause"],
])

b += h4("Library mode")
b += p("""`cobc --lib` builds the program's `extern fn` definitions as a C library (§20). There is no
`main`: each entry point is a stub that makes sure the runtime is running and that the calling C
thread is known to it.""")
b += calls([
    ["`cb_lib_enter`, `cb_lib_leave`", "each entry point's stub", "starts the runtime once and registers the calling thread; writes out what the call printed"],
    ["`cb_lib_jmp`, `cb_lib_fault_code`, `cb_lib_last_fault`", "an entry point's `_try` form", "[Fault-Contain]: a fault inside the call unwinds to the stub, which returns its status; the report is kept for `cobalt_last_fault`"],
    ["`cb_lib_shutdown`", "`cobalt_shutdown`", "ends the runtime"],
])
b += p("""A program with a `main` may have entry points too, and its own C may call them from threads it
started. Its `main` makes the runtime ready for such threads before it runs anything, and each entry
point's stub registers a thread the runtime does not know on its first call.""")
b += calls([
    ["`cb_entries_on`", "`main`, in a program with entry points", "each thread allocates from its own heap and locking stays on for the whole run, so a thread can arrive at any moment"],
    ["`cb_entry_enter`", "each entry point's stub", "a thread the runtime did not start is registered on its first call and released when it ends; any other goes on"],
])

b += h4("The standard library's way out")
b += p("""`std` is CobaltC. Where it needs the operating system or a routine better written in C, it
declares an `unsafe extern fn` (`stdout_write`, `sleep_ns`, …), and the compiler calls the runtime's
function for it. None of these touches the records above: they take addresses and lengths and
return a count or an error code.""")
b += calls([
    ["`cb_write_out`, `cb_write_err`, `cb_flush_out`", "`printf`, `print`, `eprintf`, `flush_stdout`", "buffered standard output, standard error"],
    ["`cb_read_in`, `cb_read_line_in`, `cb_arg_bytes`", "`read`, `read_line`, `arg`", "standard input; the program's arguments"],
    ["`cb_print_int`, `cb_print_f64`, `cb_print_f32`, `cb_print_bool`", "`print` of a number or a `bool`", "writes its text"],
    ["`cb_text_int`, `cb_text_f64`, `cb_text_f32`, `cb_text_bool`", "`String::append` of a number", "its text into a buffer"],
    ["`cb_parse_int`, `cb_parse_int_q` (inline), `cb_parse_float`, `cb_parse_bool`", "`parse`", "the value, or which byte stopped it; an integer of up to 64 bits and 19 digits is parsed in line"],
    ["`cb_format`, `cb_fmt_space`, `cb_fmt_u64`, `cb_fmt_i64`, `cb_fmt_hex`, `cb_fmt_pad` (the last four inline)", "`printf`, `sprintf`", "a literal format of the common specifiers is written in place by the generated C into `cb_fmt_space`'s buffer; anything else goes to `cb_format`"],
    ["`cb_file_read`, `cb_file_write`, `cb_file_op`, `cb_file_at`", "`read_file`, `write_file`, `File`", "whole files; an open file's read, write, close, seek and length"],
    ["`cb_fs_op`, `cb_fs_query`, `cb_path_op`", "the filesystem and path functions (`make_dir`, `list_dir`, `path_canonical`, …)", "the filesystem"],
    ["`cb_proc_op`, `cb_net_op`", "`Command`, `TcpStream`, `UdpSocket`, `resolve`", "processes and sockets"],
    ["`cb_thread_id` (inline), `cb_thread_here`", "`Rc` and `Weak` (`thread_id`)", "the running thread's number (1 for the main thread), one load while no thread has been spawned; the box of an `Rc` records its maker's, and a use from another thread is `diag.not-thread-safe`"],
    ["`cb_clock_read`, `cb_tz_offset`, `cb_sleep_ns`, `cb_os_random`", "`monotonic_ns`, `unix_ms`, local time, `sleep_ms`, `os_random_bytes`", "clocks, the time zone, sleeping, the system's random bytes"],
    ["`cb_sort_plain`, `cb_sort_text`, `cb_bytes_less`", "`Vec::sort` of integers, bools or text", "a native sort; text in byte order"],
])

b += h4("Inline helpers with no state")
b += table(["Name", "What it computes"], [
    ["`cb_popcount64`, `cb_popcount128`, `cb_clz`, `cb_clz128`, `cb_ctz`, `cb_ctz128`", "the bit-counting built-ins, at a given width"],
    ["`cb_rotl`, `cb_rotr`, `cb_rotl128`, `cb_rotr128`, and `cb_rotl8` to `cb_rotr64`", "rotations: at a width passed as an argument, and at each fixed width in the form a C compiler turns into one instruction"],
    ["`cb_round`, `cb_roundf`", "`round`, half away from zero, computed from `trunc` so that it is the same on every C library"],
    ["`cb_str_eq`, `cb_bytes_eq`, `cb_bytes_cmp`, `cb_bytes_lt`", "equality and byte order of text"],
    ["`cb_hash_fold`", "one step of `HashMap`'s hash: two 64-bit words multiplied to 128 bits, and the halves of the product exclusive-ored"],
    ["`cb_find_bytes`, `cb_char_boundary`", "`StringView::find`; whether a position is on a UTF-8 character's first byte"],
])

b += h4("The hash tables' hash")
b += p("""`HashMap` and `HashSet` are written in CobaltC, in the standard library; the one thing they ask of
the runtime is the hash of a key's bytes. It starts from a seed that the runtime draws once in each run, from
the system's randomness, the first time a program hashes a key: a program with no hash table draws nothing. No
program can read a hash, and a map keeps its entries in the order they were added, so the seed changes nothing
a program computes; it keeps someone who chooses a program's keys from choosing keys that all share a hash.""")
b += table(["Name", "When the generated C uses it, and what it does"], [
    ["`cb_hash_bytes`", "The hash of an integer, `bool`, `str` or `String` key: the seed, the key's length, then its bytes eight at a time (`cb_hash_more`). Inline; one multiplication for a `u64` key."],
    ["`cb_hash_more`", "The hash so far, taken over more bytes: the parts of a struct or enum key go through it one after another, in the function the compiler writes for that key type."],
    ["`cb_hash_bytes_drawn`", "`cb_hash_bytes` with the seed as it stands, with no test of it: the hash in the bodies the compiler writes itself for `HashMap`'s lookups, insertions and removals with integer, `bool` and `String` keys. The ones that place an entry (`insert`, `entry`) call `cb_hash_start` first; a lookup or a removal does not, because until the seed is drawn no map in the program has an entry, and an empty table gives the same answer for any hash."],
    ["`cb_hash_start`", "The seed, where a struct or enum key's hash starts, and what a compiler-written `insert` or `entry` calls before it hashes. Inline: it loads `cb_hash_seed_cell`, and calls `cb_hash_seed_make` only while that is still zero."],
    ["`cb_hash_seed_cell`", "The seed: zero until the first hash of the run."],
    ["`cb_hash_seed_make`", "Draws the seed and stores it, once for all threads: from `COBALTC_HASH_SEED` when that is set, else eight bytes from the system (as `os_random_bytes` gets them), else, on a system with none, from the clock and an address."],
])

# ---------------------------------------------------------------- fault

b += h3("A fault, from call to exit", anchor="rt-fault")
b += p("""Every failed check ends in `cb_fault(diag, file, line)`, whether the generated C found it
(an overflow, an index) or the runtime did (a stale path inside `cb_read`). The generated C never
`longjmp`s and has no cleanup code of its own for this: the runtime holds the stack of frames and
scopes, so it can do what the rules say.""")
b += ol([
    "The location is the one passed, or the one `cb_at` last stored when none was (a fault found while a frame was ending, or in a destructor).",
    "[Fault-Unwind]: every open statement scope and frame of this thread is ended, innermost first, exactly as `cb_stmt_pop` and `cb_frame_pop` would end them. Destructors run: the runtime calls back into the program through each type descriptor's `drop`. Noted locals are ended from their tables at the depth they were registered.",
    "The report is written to standard error: `fault[diag.name]: message`, the file, line and column, the source line, and the registry's repair advice. With `COBALTC_EXPLAIN=1` in the environment the registry's whole entry follows.",
    "Buffered output is written out and the process ends with status 1.",
])
b += p("""Three things change the last step. In a thread started with the supervision operations, the
unwind ends at the `setjmp` in the thread's body (`cb_thread_jmp`), the thread is recorded as
failed, and `try_join` gives the failure to whoever joins it. In a library entry point's `_try`
form it ends at the stub's `setjmp` and the stub returns the status to its C caller. And a
thread that was lent an exclusive reference is never contained: its fault ends the program.""")
b += p("""A state the runtime's own tables should never reach is not a fault of the program: the
runtime prints `cbrt: internal: …` and exits with status 4, as `cobc` does for an error of its
own.""")

# ---------------------------------------------------------------- callbacks

b += h3("What the runtime calls in the program", anchor="rt-callbacks")
b += p("""The calls go the other way in four places only:""")
b += ul([
    "<strong>Destructors.</strong> A `cb_type` with a user `drop`, or generated drop glue, carries its address. `cb_frame_pop`, `cb_stmt_pop`, `cb_destroy`, `cb_drop_local` and the unwind call it with a `cb_ref` to the value.",
    "<strong>Thread bodies.</strong> `cb_spawn` is given the function the compiler wrote for that `spawn`, which unpacks the argument block and calls the program's function.",
    "<strong>`cb_program`.</strong> Given to `cb_run_main`, or the initialiser given to `cb_lib_enter`.",
    "<strong>`cb_where`, `cb_set_where`, `cb_set_stack_floor`.</strong> The accessors of the two thread-locals the program defines.",
])
b += p("""The runtime never reads the program's source, its names or its values except through these
and through the type descriptors: what it knows about a type is its size, its fields' offsets and
types, where its references are, and how to end it.""")

# ---------------------------------------------------------------- environment

b += h3("What the runtime reads from the environment", anchor="rt-env")
b += p("""A compiled program checks these variables when it starts. They change how the run is
observed, never what the program computes.""")
b += table(["Variable", "Effect"], [
    ["`COBALTC_EXPLAIN=1`", "A fault's report is followed by the diagnostic's whole registry entry, as `cobc --explain --run` shows it."],
    ["`COBALTC_SCHEDULE_SEED=S`", "The threads run one at a time, in the order seed `S` gives, as `cobc --explore` ran them: a run that ended differently can be replayed."],
    ["`COBALTC_VERIFY=1` (or `=N`)", "At every statement's end (or every Nth), the runtime re-derives that its object and path tables agree with each other, and stops naming the defect if they do not. A violation is a defect of the tool, never of the program."],
    ["`COBALTC_TRACE_EVENTS=1`", "One `EV` line on standard error for each event the compiler's elisions must not disturb: each user destructor run, the fault, an `exit`, the end."],
    ["`COBALTC_RECORD=full` or `check`", "Every entry goes to the runtime (`cb_inline_off`): `full` keeps the full record of references, for comparison with the light one; `check` stops where a light reference reaches something that should have recorded it."],
    ["`COBALTC_REPLAY_TRACE=FILE`", "Writes the run's steps in the format of the proved machine in `proofs/`, for replaying against it."],
    ["`COBALTC_RT_STATS=1`", "A test hook: counts of what the runtime recorded, and what it still holds when the program ends."],
    ["`COBALTC_HASH_SEED=N`", "Read at the first hash rather than at the start: the hash tables' seed is made from the number `N`, not drawn from the system, so two runs place their entries alike and execute the same instructions. For measuring."],
])
b += p("""Without `COBALTC_HASH_SEED`, the instructions a program with a hash table executes differ a little
from run to run: its output does not.""")

# ---------------------------------------------------------------- reading

b += h3("Where to read further", anchor="rt-read")
b += table(["File", "What is there"], [
    ["`impl/cbrt/include/cbrt.h`", "the interface itself; each declaration's comment names the rule it implements"],
    ["`impl/cbrt/src/rt.h`", "the runtime's data structures, with the design in its first comment"],
    ["`impl/cbrt/src/rt_objs.c`", "objects, frames and statement scopes, destruction"],
    ["`impl/cbrt/src/rt_refs.c`", "borrows, checked reads and writes, reference slots, values in flight, `fn` values, raw storage, noted locals"],
    ["`impl/cbrt/src/rt_tables.c`", "the tables: slots by address, an object's paths, projections and their comparison, the marks, objects over raw cells"],
    ["`impl/cbrt/src/rt_threads.c`, `rt_lock.c`", "threads, the schedule, mutexes and the runtime's lock"],
    ["`impl/cbrt/src/rt_init.c`, `rt_alloc.c`, `diagnostics.c`", "start-up, ending and faults; the allocator; the reports"],
    ["`impl/cbrt/src/plat_posix.c`, `plat_win.c`", "everything that differs between Linux and Windows"],
    ["`impl/COBC-PLAN.md`", "the compiler's plan and its history: why the interface has the shape it has, and each step that made it cheaper, with measurements"],
    ["`spec/04`, `spec/08`, `spec/10`, `spec/14`", "the state, aliasing, temporal validity and the scopes the records above implement"],
    ["`spec/ASSURANCE.md`", "what is proved, checked and tested about the runtime"],
])


def _check_names(body):
    """Every `cb_` name of the header is named in the appendix, and the
    appendix names no `cb_` name that is neither the header's nor one the
    generated program defines."""
    in_header = set(re.findall(r"\bcb_[a-z0-9_]+", open(HEADER, encoding="utf-8").read()))
    in_page = set(re.findall(r"\bcb_[a-z0-9_]+", body))
    missing = sorted(in_header - in_page)
    unknown = sorted(n for n in in_page - in_header if not GENERATED.match(n))
    if missing or unknown:
        msg = "content_runtime.py: appendix B and impl/cbrt/include/cbrt.h disagree\n"
        if missing:
            msg += "  declared in the header, not described in the appendix: %s\n" % " ".join(missing)
        if unknown:
            msg += "  named in the appendix, not in the header: %s\n" % " ".join(unknown)
        sys.exit(msg)


_check_names(b)

RUNTIME = Section("appb", "B", "Appendix: the runtime interface", None,
                  "How compiled CobaltC uses `cbrt`, the runtime: what it holds, every call the generated C makes, and why.", b)
RUNTIME.group = "Appendix"

SECTIONS = [RUNTIME]
