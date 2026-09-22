from gen import Section, p, h3, h4, ul, ol, table, note, rules, code
from content_a import CHECK, CHECK_STD

# ================================================================ §12

b = ""
b += p("""CobaltC's type system is deliberately plain: every type is exactly what it is declared to
be, nothing converts to anything else, and generics are templates instantiated with concrete types.
What it gives you in return is that the type of every expression is decidable from the text, and
that a function signature is a complete statement of what the function may do with its
arguments.""")

b += h3("The built-in types")
b += table(["Type", "Values", "Resource?"], [
    ["integers, `f32`, `f64`, `bool`", "numbers and truth values (§06)", "no"],
    ["`str`", "text: a valid UTF-8 byte sequence, written `\"…\"`; a whole value like an integer, not a reference and not owned (§21)", "no"],
    ["`void` (type) / `()` (value)", "the unit type; the type of assignments, of `if` without `else`, of loops and of functions with no return type. Written `void` in type position and `()` as a value.", "no"],
    ["`ref<T, shared>`, `ref<T, exclusive>`", "references (§09)", "no"],
    ["`rawptr<T>`", "raw addresses (§20)", "no"],
    ["`array<T, N>`", "N values of `T` in a row, `N` a literal", "if `T` is"],
    ["`fn(T1, T2) : R`", "function values", "no"],
    ["`handle<T>`, `mutex<T>`, `guard<T>`", "threads and locks (§19)", "yes"],
    ["`struct`, `enum`", "user-declared aggregates (§16)", "if marked `resource` or any part is"],
    ["`never`", "the type of `return`, `break` and `continue`: an expression that never yields a value, accepted wherever any type is expected", "n/a"],
])

b += h3("Nominal, exact, no coercions")
b += p("""Two types are the same only if they have the same name and the same type arguments.
`ref<i32, shared>` and `ref<i32, exclusive>` are different types. There is no subtyping and no
implicit conversion anywhere: not between integer widths, not from `T` to `ref<T, m>`, not from
`Option<T>` to `T`. This applies to arguments too.""")
b += code("""
fn takes_i64(i64 x) : i64
{
    x
}

fn main()
{
    i32 y = 5;
    takes_i64(y);                     // ✗ diag.type-mismatch: i32 is not i64; write takes_i64(widen<i64>(y))
}
""", title="No implicit widening, even for arguments", expect="diag.type-mismatch", section="s12", slug="arg-mismatch")
b += p("""A bare integer literal is the one flexible thing: `takes_i64(5)` is fine, because the
literal takes its type from the parameter (§06).""")

b += h3("Equality")
b += p("""`==` and `!=` are defined for numbers, `bool`, `()`, `str` (bytewise), references (identity), raw pointers
(address), and for structs, arrays and enums whose parts are all comparable (element by element;
enums must have the same variant and equal payloads). They are not defined for function values or
for anything that is a resource.""")

b += h3("Generics")
b += p("""Functions, structs and enums can take type parameters. Each use with concrete types is a
separate copy of the declaration, so there is no run-time type information and no cost at call
sites. There is no trait or bound system: inside a generic body a value of bare type `T` can be
stored, passed, returned, referenced and moved, but not added, compared, indexed or called. If you
need arithmetic, take a concrete type.""")
b += code(CHECK_STD + """
struct Box<T>
{
    T value;
}

enum Maybe<T>
{
    Just(T),
    Nothing
}

fn identity<T>(T x) : T
{
    x                                 // all a bare T can do: be moved around
}

fn boxed<T>(T x) : Box<T>
{
    Box { .value = x }                // the field type is T, so this is fine
}

fn main()
{
    check(identity(5) == 5);          // T := i32, from the argument
    check(identity<i64>(5) == 5);     // T given explicitly: name<Type>(...)
    Box<i32> b = Box { .value = 7 };  // Box's T from the declared type
    check(b.value == 7);
    Box<Vec<u8>> c = boxed(Vec::new());   // both T's from the declared type of c
    check(Vec::len(&c.value) == 0);
}
""", title="Generic declarations", expect="ok", section="s12", slug="generics")
b += p("""Inference is <em>local and downward</em>. A type parameter is found from, in order: an
explicit type argument (`identity<i64>(5)`), the type of an argument in the same call, or the
<em>expected type</em> where the call appears (a declared variable type, a parameter type, a field
type). Nothing is ever inferred from how a value is used later, so a call that fixes nothing is
rejected:""")
b += code("""
import std;

fn main()
{
    auto v = Vec::new();              // ✗ diag.cannot-infer-type-parameter: nothing says what T is
    Vec::push(&mut v, 1);             //   (a later use does not count)
}
""", title="Nothing to infer from", expect="diag.cannot-infer-type-parameter", section="s12", slug="uninferable")
b += p("""The same holds when the call is an argument to another generic call. `Mutex::new<T>` is
one: with `auto` nothing reaches it, so nothing reaches the `Vec::new()` inside it either. Give the
outer binding its type and both are fixed.""")
b += code(CHECK_STD + """
fn main()
{
    mutex<Vec<u8>> m = Mutex::new(Vec::new());   // T of Mutex::new, then T of Vec::new, from the declaration
    mutex<i64> big = Mutex::new(5000000000);     // and the literal is an i64 for the same reason
    check(Vec::len(&*lock(&m)) == 0);
}
""", title="Inference through Mutex::new", expect="ok", section="s12", slug="mutex-new-infer")
b += code("""
import std;

fn main()
{
    auto m = Mutex::new(Vec::new());  // ✗ diag.cannot-infer-type-parameter: nothing fixes Vec's T
}
""", title="Nothing to infer from", expect="diag.cannot-infer-type-parameter", section="s12", slug="mutex-new-uninferable")
b += code(CHECK_STD + """
struct Box<T>
{
    T value;
}

fn identity<T>(T x) : T
{
    x
}

fn main()
{
    Vec<i32> a = Vec::new();                  // T from the declaration
    Box<Vec<u8>> c = Box { .value = Vec::new() };   // both T's from the declared type
    Vec::push(&mut c.value, 200);             // the literal becomes u8 from the field type
    check(Vec::len(&c.value) == 1);
    check(Vec::len(&a) == 0);
    i64 w = identity<i64>(3);                 // explicit: name<Type>(...)
    check(w == 3);
}
""", title="Where inferred types come from", expect="ok", section="s12", slug="inference")

b += h3("`auto` and the expected type")
b += p("""`auto x = e;` gives `x` the type of `e`. Since inference is downward only, `auto` works
whenever `e` has a type of its own: a literal (`i32`/`f64` by default), an arithmetic expression, a
call whose type parameters are already fixed, a struct literal with a concrete type. Where `e`
needs help (`Vec::new()`), write the type on the left instead: `auto m = Mutex::new(HashMap::new());`
cannot know the map's types, but `mutex<HashMap<String, i64>> m = Mutex::new(HashMap::new());` can.
The branches of an `if` or `match` share one type, and a branch that is only a literal takes it from
the others: in `auto n = if (big) { count } else { 0 };` with `count : i64`, the `0` is an `i64`.""")
b += code(CHECK_STD + """
fn main()
{
    i64 count = 5000000000;
    auto n = if (count > 10) { count } else { 0 };     // 0 : i64, from the other branch
    Option<i64> o = None;
    auto m = match (o) { None : -1, Some(v) : v };     // -1 : i64, from the Some arm
    check(n + m == 4999999999);
    mutex<HashMap<String, i64>> totals = Mutex::new(HashMap::new());   // the declaration fixes K and V
    check(HashMap::len(&*lock(&totals)) == 0);
}
""", title="Branch types and declared types", expect="ok", section="s12", slug="branch-types")

b += rules([
    "Write the type you mean; the language will not adjust it for you. Convert integers with `widen`/`narrow`, and take references explicitly.",
    "Use `auto` freely for values whose type is obvious from the right-hand side, and a written type for empty containers and generic constructors.",
    "Keep generic code to plumbing (containers, wrappers, moving values around). Anything that computes on `T` needs a concrete type.",
    "A `void` function returns `()`; you may write `: void` explicitly or leave the return type off.",
])
S12 = Section("s12", "§12", "The type system", "12-type-system.md",
              "Nominal types, no coercions, monomorphic generics without bounds, and local inference that only flows downward.", b)

# ================================================================ §13

b = ""
b += p("""CobaltC has a small expression language with a fixed evaluation order and a clear line
between <em>places</em> (things you can assign to or borrow) and <em>values</em>. Almost everything
is an expression, including blocks, `if` and `match`, which is what lets you write
`i32 x = if (c) { 1 } else { 2 };`.""")

b += h3("Places and values")
b += p("""A <strong>place</strong> is a variable name, a field access `e.f`, an index `e[i]`, or a
dereference `*r`. A place can be read (yielding its value), written (`place = e`), borrowed
(`&place`, `&mut place`), or projected further. Everything else, such as a literal, an arithmetic
result, a call result or a struct literal, is a value or a temporary. Reading a place that holds a
resource is not allowed (that would copy the resource, §07); such a place may only be moved,
borrowed, or destructured.""")
b += code(CHECK + """
struct Pair
{
    i32 a;
    array<i32, 3> items;
}

fn main()
{
    Pair p = Pair { .a = 1, .items = [10, 20, 30] };
    p.a = 99;                         // a field is a place
    p.items[1] = 200;                 // so is an element
    {
        auto r = &mut p.items[2];
        *r = 300;                     // and so is *r
    }
    check(p.a == 99);
    check(p.items[1] == 200);
    check(p.items[2] == 300);
}
""", title="Places", expect="ok", section="s13", slug="places")

b += h3("Evaluation order")
b += p("""Every expression is evaluated strictly left to right: the left operand before the right,
the callee before its arguments, arguments in written order, struct-literal fields in written order.
Assignment evaluates the target place first, then the value. `&&` and `||` short-circuit: the right
operand runs only when needed.""")
b += code(CHECK + """
fn bump(ref<i32, exclusive> counter) : i32
{
    *counter = *counter + 1;
    *counter
}

fn main()
{
    i32 n = 0;
    i32 left = bump(&mut n);
    i32 right = bump(&mut n);
    check(left == 1);                 // the first call ran first
    check(right == 2);
    bool touched = false;
    if (true || bump(&mut n) == 0)    // the right side is never evaluated
    {
        touched = true;
    }
    check(touched);
    check(n == 2);
}
""", title="Left to right, and short-circuit", expect="ok", section="s13", slug="order")

b += h3("Expressions that produce values")
b += code(CHECK_STD + """
fn main()
{
    bool c = true;
    i32 x = if (c)                            // if is an expression; both arms must have one type
    {
        1
    }
    else
    {
        2
    };
    i32 y =                                   // a block's value is its trailing expression
    {
        i32 t = x * 3;
        t + 1
    };
    Option<i32> o = Some(x);
    i32 z = match (o)                         // so is match
    {
        Some(v) : v * 10,
        None    : 0,
    };
    check(x == 1);
    check(y == 4);
    check(z == 10);
    i32 w = 0;
    w = 5;                                    // assignment is an expression of type void
    check(w == 5);
}
""", title="if, match and blocks as values", expect="ok", section="s13", slug="value-exprs")
b += note("""A block, `if`, `while` or `match` used as a statement needs no semicolon after its
closing brace. When one of them is the right-hand side of a declaration or assignment, the
statement as a whole still ends with `;`, as in `i32 x = if (c) { 1 } else { 2 };`.""", kind="tip")

b += h3("Operator precedence")
b += p("""From tightest to loosest binding. Comparison is non-associative: `a < b < c` is a
syntax error, and `a < b == c` needs parentheses.""")
b += table(["Level", "Operators", "Notes"], [
    ["postfix", "`e.f`  `e[i]`  `e(args)`  `e?`", "field, index, call, propagate"],
    ["unary", "`-e`  `!e`  `~e`  `&e`  `&mut e`  `*e`", "negate, boolean not, bitwise not, borrow, dereference"],
    ["multiplicative", "`*`  `/`  `%`", "left-associative"],
    ["additive", "`+`  `-`", "left-associative; `p + n` on a `rawptr` is pointer offset (§20)"],
    ["bitwise and", "`&`", ""],
    ["bitwise xor", "`^`", ""],
    ["bitwise or", "`|`", ""],
    ["shift", "`<<`  `>>`", "the right operand is a `u32`"],
    ["comparison", "`==` `!=` `<` `<=` `>` `>=`", "non-associative"],
    ["logical and", "`&&`", "short-circuit"],
    ["logical or", "`||`", "short-circuit"],
    ["assignment", "`=`  `+=`  `-=`  `*=`  `/=`  `%=`  `&=`  `|=`  `^=`  `<<=`  `>>=`", "right-associative; the left side must be a place"],
])
b += p("""`x op= e` is `x = x op e` with `x` evaluated once: the same types, the same overflow
check (`x += 1` can overflow), the same write rules. Evaluating the place once matters only when it
contains a call, as in `*Vec::index_exclusive(&mut v, i) += 1`, which calls `index_exclusive` once.
There is no `++` or `--`; write `+= 1`.""")
b += note("""Unlike C, the bitwise operators bind <em>tighter</em> than shifts and comparisons
here, so `x & 1 == 0` parses as `(x & 1) == 0`. When in doubt, add parentheses.""", kind="warn")

b += rules([
    "Use `if`, `match` and blocks as values instead of declaring a variable and assigning in each branch.",
    "Side effects in one operand are visible to the next: order is defined, so you may rely on it, but readers will thank you for not making them.",
    "Parenthesise mixed comparisons and bitwise expressions.",
])
S13 = Section("s13", "§13", "Expressions", "13-expression-semantics.md",
              "Places versus values, strict left-to-right evaluation, and the operators.", b)

# ================================================================ §14

b = ""
b += p("""Control flow is C's `if`, `while` and `for`, without `switch` and `goto`, plus
expression-valued `if` and `match` and a `foreach` over collections. What is new is how carefully the language ties <em>scope</em> to <em>destruction</em>, and
the precise line between what the compiler proves and what the run time checks.""")

b += h3("Blocks and scopes")
b += p("""A block `{ … }` opens a scope. Variables declared in it die when it ends, in reverse
order; resources are destroyed at that point (§07); references held by those variables end too. A
block is an expression: its value is its trailing expression, or `()` if there is none. Every
statement is a scope of its own for temporaries: a value created during a statement and not stored
anywhere is destroyed at the semicolon.""")

b += h3("`if` and `else`")
b += p("""`if (cond) { … } else { … }` with mandatory parentheses and mandatory braces. `else if`
chains as expected. An `if` without `else` has type `void`; an `if` used as a value needs both arms,
of the same type.""")

b += h3("`while`, `for`, `break`, `continue`")
b += p("""`while (cond) { … }` loops while `cond` holds; `for (init; cond; step) { … }` is C's
counted loop: `init` runs once (a declaration there belongs to the loop), `cond` is tested before
each iteration, and `step` runs after each one. Any of the three may be left out; a missing `cond`
is `true`, so `for (;;)` loops until a `break`. The body is a fresh scope each iteration, so
anything declared inside it is destroyed before the next test. `break` exits the innermost loop;
`continue` goes to its next iteration, running a `for`'s `step` first. Both are typed `never` and
must appear inside a loop. A body is always a block, so C's trap `for (…);` (an empty loop, then a
block that runs once) is a syntax error here, and the message says to remove the `;`.""")
b += code(CHECK_STD + """
fn main()
{
    i32 sum = 0;
    i32 i = 0;
    while (i < 10)
    {
        i = i + 1;
        if (i == 3)
        {
            continue;                 // skip 3
        }
        if (i == 7)
        {
            break;                    // stop before adding 7
        }
        sum = sum + i;
    }
    check(sum == 1 + 2 + 4 + 5 + 6);
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 4);
    Vec::push(&mut v, 9);
    i32 total = 0;
    for (usize k = 0; k < Vec::len(&v); k += 1)   // an index loop; foreach (below) is shorter
    {
        total += *Vec::index_shared(&v, k);
    }
    check(total == 13);
    u32 odd = 0;
    for (u32 j = 0; j < 10; j += 1)
    {
        if (j % 2 == 0)
        {
            continue;                 // still runs j += 1
        }
        odd += j;
    }
    check(odd == 25);
}
""", title="Loops", expect="ok", section="s14", slug="loops")
b += code("""
fn main()
{
    break;                            // ✗ diag.break-outside-loop
}
""", title="break outside a loop", expect="diag.break-outside-loop", section="s14", slug="break-outside")

b += h3("`foreach`")
b += p("""`foreach (x in c) { … }` visits every element of a `Vec`, an array, a `HashSet` or a
`HashMap`, in order: by index for a `Vec` or an array, in insertion order for a set or a map. How
the loop holds the collection is written at the loop, the same three ways a function takes a
value:""")
b += table(["Written", "Each element is", "Afterwards"], [
    ["`foreach (x in c)`", "the element itself, moved out of `c`", "`c` is gone: the loop consumed it"],
    ["`foreach (x in &c)`", "a shared reference, `ref<T, shared>`: read it", "`c` is unchanged and usable"],
    ["`foreach (x in &mut c)`", "an exclusive reference: `*x = …` changes the element", "`c` is usable, with its changes"],
])
b += p("""Use `&c` most of the time. A second name gives the position: `foreach (i, x in &v)` has
`i : usize`. A `HashMap` is always looped over with two names, the key and the value:
`foreach (k, v in &m)`, or three to have the position too, `foreach (i, k, v in &m)`. `printf` prints what a reference refers to, so `printf("%d", x)` works
with `x` a `ref<i32, shared>`; elsewhere write `*x` for the number.""")
b += code("""
import std;

fn shout(String s)                    // takes ownership
{
    printf("%s!\\n", &s);
}

fn main()
{
    Vec<String> names = Vec::new();
    Vec::push(&mut names, String::from_str("ada"));
    Vec::push(&mut names, String::from_str("lin"));
    foreach (i, name in &names)       // read: names is still usable afterwards
    {
        printf("%d: %s\\n", i, name);
    }

    Vec<i32> numbers = Vec::new();
    Vec::push(&mut numbers, 3);
    Vec::push(&mut numbers, 5);
    foreach (n in &mut numbers)       // change in place
    {
        *n *= 2;
    }
    foreach (n in &numbers)
    {
        printf("%d ", n);             // printf reads through the reference
    }
    printf("\\n");

    HashMap<str, u32> stock = HashMap::new();
    HashMap::insert(&mut stock, "pens", 12);
    HashMap::insert(&mut stock, "ink", 3);
    foreach (item, count in &stock)
    {
        printf("%-5s %2d\\n", item, count);
    }

    foreach (name in names)           // consume: each String moves into shout
    {
        shout(name);
    }
    // names is gone here; using it would be diag.stale-binding
}
""", title="The three forms", expect="ok",
     output="0: ada\n1: lin\n6 10 \npens  12\nink    3\nada!\nlin!\n", section="s14", slug="foreach")
b += p("""The loop's borrow of the collection lasts the whole loop, so a body that tries to change
the collection itself (push to it, clear it) is rejected before the program runs, the same
aliasing rule as anywhere else. There is no iterator invalidation to debug. A consuming loop
left early, by `break`, `return` or a fault, destroys the elements it had not reached, exactly
once.""")
b += code("""
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 1);
    foreach (x in &v)
    {
        Vec::push(&mut v, *x);        // ✗ diag.aliasing-conflict (static): v is borrowed by the loop
    }
}
""", title="The collection cannot change under the loop", expect="diag.aliasing-conflict", section="s14", slug="foreach-change")
b += p("""An array of plain values, such as `array<i32, 5>`, is copied by the by-value form:
`foreach (i, e in arr)` walks a copy, so `arr` is neither borrowed nor used up, a write to
`arr[j]` in the body is allowed (and not seen by `e`), and `arr` is still there afterwards. To change
the elements themselves, loop over `&mut arr` and write through the reference: `*e = …`. While
that loop runs, `arr` is borrowed by it, so reach the elements through `e`, not through `arr[j]`.""")
b += code("""
import std;

fn main()
{
    array<i32, 5> arr = [0, 0, 0, 0, 0];

    foreach (i, e in &mut arr)
    {
        *e = narrow<i32>(i) * 10;       // write through the element's reference
    }
    arr[4] += 1;                        // direct indexing works too, outside the loop

    foreach (i, e in &arr)
    {
        printf("[%v] = %v\\n", i, *e);
    }
}
""", title="Changing an array in place", expect="ok",
     output="[0] = 0\n[1] = 10\n[2] = 20\n[3] = 30\n[4] = 41\n", section="s14", slug="foreach-mut-array")

b += h3("Early exit and unwinding")
b += p("""`return`, `break`, `continue`, and a run-time fault all leave scopes the same way: every
statement scope and block between here and the target is exited in order, destroying what each one
owns. A `return e` first moves `e` out so it survives the unwinding. You never have to write
cleanup code on an early exit.""")

b += h3("What is checked when")
b += p("""Many rules in this guide say a program is "rejected" in one example and "faults at run
time" in another. The boundary is fixed by the language, not left to the compiler's cleverness, so
that every implementation accepts and rejects the same programs. Within one function body, the
checker tracks a few facts about each variable: whether it has been moved from, whether it has been
initialised, which references were visibly borrowed from it, and whether a reference to it has been
handed somewhere the checker cannot follow (a call, a field, a thread). At each checked operation
the outcome is one of three:""")
b += table(["Outcome", "When", "Effect"], [
    ["proven", "the tracked facts guarantee the condition", "no run-time check"],
    ["refuted", "the tracked facts guarantee it fails", "the program is rejected (a <em>static</em> diagnostic)"],
    ["unknown", "the facts do not decide (the value came from a call, a loop merged two states, a reference passed through a parameter)", "the run-time check runs, and faults if it fails (a <em>dynamic</em> diagnostic)"],
])
b += p("""The analysis looks only at the current function and only at literals for value ranges.
So `2147483647 + 1` is refuted statically, `add(2147483647, 1)` is checked when `add` runs, and
borrowing `x` twice exclusively in one function is refuted while doing so through two reference
parameters is checked at the use. Two exceptions are stricter: definite assignment (§11) and
reference escape (§10) reject on "unknown" rather than deferring.""")

b += rules([
    "Let scopes do your cleanup. Open a block to bound a borrow or a lock; close it to release.",
    "A `while` body is re-entered fresh each iteration; declare per-iteration variables inside it.",
    "Prefer trailing expressions to `return` at the end of a function; use `return` for early exits.",
    "If a diagnostic says <em>static</em>, the checker saw the problem in the text; if <em>dynamic</em>, it saw it when the values arrived. Both are the same rule.",
])
S14 = Section("s14", "§14", "Control flow and scopes", "14-control-flow.md",
              "Blocks, if, while, for, foreach, break and continue; scope exit as the moment of destruction; and the fixed line between compile-time and run-time checks.", b)

# ================================================================ §15

b = ""
b += p("""Functions in CobaltC carry their whole contract in the signature. A parameter's type says
whether the function takes ownership, may only look, or may modify. Closures are ordinary structs
that capture what they use, and the capture list is checked, not trusted.""")

b += h3("Declaring and calling")
b += code(CHECK + """
// Parameters are type-first; the return type follows a colon.
fn area(i32 w, i32 h) : i32
{
    w * h                             // the trailing expression is the result
}

// No return type means void; `return;` leaves early.
fn clamp_in_place(ref<i32, exclusive> x, i32 hi)
{
    if (*x <= hi)
    {
        return;
    }
    *x = hi;
}

// An associated function: a plain function whose name is qualified by a type.
struct Rect
{
    i32 w;
    i32 h;
}

fn Rect::square(i32 side) : Rect
{
    Rect { .w = side, .h = side }
}

fn Rect::area(ref<Rect, shared> r) : i32
{
    area(r.w, r.h)
}

fn fact(i32 n) : i32
{
    if (n <= 1)                       // recursion needs nothing special
    {
        1
    }
    else
    {
        n * fact(n - 1)
    }
}

fn main()
{
    i32 v = 50;
    clamp_in_place(&mut v, 10);
    check(v == 10);
    auto sq = Rect::square(4);
    check(Rect::area(&sq) == 16);     // called by qualified name; there is no method syntax
    check(fact(5) == 120);
}
""", title="Functions", expect="ok", section="s15", slug="functions")

b += h3("What a parameter type promises")
b += table(["Parameter type", "The caller writes", "The callee may", "After the call"], [
    ["plain `T` (numbers, plain structs)", "`f(x)`", "use its own copy", "the caller's `x` is unchanged"],
    ["resource `T` (`Vec<i32>`, `handle<T>`, …)", "`f(v)`", "own it: mutate, move on, or let it be destroyed at its scope end", "the caller's `v` is stale"],
    ["`ref<T, shared>`", "`f(&x)`", "read through it", "the borrow is released; `x` is unchanged"],
    ["`ref<T, exclusive>`", "`f(&mut x)`", "read and write through it, but not destroy or move `x`", "the borrow is released; `x` may have changed"],
])

b += h3("Function values")
b += p("""A function type is written `fn(T1, T2) : R` (or `fn(T1)` for a `void` result). A
parameter of that type accepts a function with that signature, or a closure with the same
parameters and result. Function values are plain, copyable, and cannot be compared.""")

b += h3("Closures")
b += p("""A closure is written `[captures](params) { body }`, in the style of a C++ lambda. The
bracket must list <em>exactly</em> the outer variables the body uses, no more and no fewer; it is
documentation that the compiler checks (`diag.capture-list-mismatch`). How each variable is captured
is decided by the body, not the list: a variable the body only reads is captured by shared
reference; one the body writes, borrows exclusively, or moves is captured by exclusive reference.
Prefix `move` to capture by value instead, moving resources into the closure.""")
b += code(CHECK_STD + """
fn apply(fn(i32) : i32 f, i32 x) : i32
{
    f(x)
}

fn main()
{
    i32 k = 10;
    auto add_k = [k](i32 x)           // k is read only: captured as ref<i32, shared>
    {
        x + k
    };
    check(add_k(5) == 15);
    check(apply(add_k, 4) == 14);     // a closure where a fn(i32) : i32 is expected

    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 7);
    auto reader = move [v]()          // v moves into the closure; the closure owns it now
    {
        *Vec::index_shared(&v, 0)
    };
    check(reader() == 7);
    check(reader() == 7);             // calling twice is fine: nothing is consumed
}                                     // reader is destroyed here, and its Vec with it
""", title="Capturing", expect="ok", section="s15", slug="closures")
b += code("""
fn main()
{
    i32 x = 1;
    i32 unused = 2;
    auto f = [x, unused]()            // ✗ diag.capture-list-mismatch: unused is not used by the body
    {
        x
    };
    f();
}
""", title="The capture list must match", expect="diag.capture-list-mismatch", section="s15", slug="capture-mismatch")
b += p("""A closure that captures by reference holds those borrows for as long as the closure
object lives, so the captured variables are subject to the usual aliasing rules until it goes out
of scope: a closure that writes `total` blocks every other use of `total` while it exists. Put such
closures in a block, or use `move`. Calling a closure borrows it exclusively for the call, so a
closure cannot be called re-entrantly.""")
b += code("""
fn main()
{
    i32 total = 0;
    {
        auto add = [total](i32 x)     // the body writes total: captured as ref<i32, exclusive>
        {
            total = total + x;
        };
        add(5);
        add(7);
    }                                 // add ends here and releases its borrow of total
    i32 seen = total;                 // ✓ 12: no borrow remains
}
""", title="An exclusive capture, bounded by a block")

b += h3("Generic functions")
b += p("""`fn name<T>(...)` declares type parameters; see §12 for what a body may do with `T` and
how `T` is inferred at each call.""")

b += h3("`main`")
b += p("""A program is a set of declarations with exactly one `main` at the top level, taking no
parameters: `fn main()`, or `fn main() : u8`. Execution is the evaluation of `main()`; the program's
observable behaviour is its termination outcome (success or a named fault) plus the sequence of
`extern` calls it makes.""")
b += p("""A `main` that returns a `u8` sets the program's <strong>exit status</strong>; `fn main()`
exits with 0. Returning from `main` runs every destructor still pending and waits for every
thread, as leaving any block does, so the status is reported only after all of that. A fault
still ends the program with its diagnostic instead. The arguments the program was started with
come from `arg_count()` and `arg(i)` (§21); argument 0 is the first one after the program, not the
program's name.""")
b += code("""
import std;

fn main() : u8
{
    if (arg_count() != 1)
    {
        printf("usage: greet NAME\\n");
        return 2;                     // exit status 2
    }
    match (arg(0))
    {
        Ok(name) :
        {
            printf("hello, %v\\n", &name);
            0
        },
        Err(_) :
        {
            printf("the name is not UTF-8\\n");
            1
        },
    }
}
""", title="An exit status and an argument: `coby greet.cb Ada`", expect="ok", output="hello, Ada\n",
     section="s15", slug="main-exit-status", args=["Ada"])
b += p("""Run with no argument, the same program prints its usage line and exits with status 2.
A `main` with parameters, or returning anything other than `u8`, is rejected with
`diag.no-main`.""")

b += rules([
    "Choose the parameter kind that says the least: a shared reference if the function only reads, an exclusive reference if it modifies, by value only if it needs to keep the thing.",
    "Return values rather than out-parameters; a struct or an enum is cheap to return.",
    "List every captured variable in the closure brackets and let the compiler tell you if you got it wrong.",
    "Use `move` closures for anything that outlives the current block or runs on another thread.",
])
S15 = Section("s15", "§15", "Functions and closures", "15-function-semantics.md",
              "Signatures that state the whole contract, associated functions, function values, and closures with checked capture lists.", b)

# ================================================================ §16

b = ""
b += p("""CobaltC has three aggregate kinds: structs (named fields), arrays (a fixed number of
elements) and enums (one of several variants, each with an optional payload). They compose freely
and generically. `match` is how you take an enum apart, and it must handle every variant.""")

b += h3("Structs")
b += code(CHECK + """
struct Point
{
    i32 x;
    i32 y;
}

struct Segment
{
    Point from;                       // structs nest by value
    Point to;
}

fn main()
{
    auto a = Point { .x = 1, .y = 2 };           // every field, once, in any order
    Point b = Point { .y = 2, .x = 1 };
    check(a == b);                                // plain structs compare field by field
    auto s = Segment { .from = a, .to = Point { .x = 4, .y = 6 } };
    s.to.y = 7;                                   // nested fields are places
    check(s.to.y == 7);
    Point c = a;                                  // plain structs copy
    c.x = 100;
    check(a.x == 1);
}
""", title="Struct declaration, literal, access", expect="ok", section="s16", slug="structs")
b += p("""Field initialisers use C99's designated form `.name = value`. Fields are private to the
module that declares the struct unless marked `export` (§17). A struct with no fields is allowed.
Mark a struct `resource` if it manages something beyond its fields (§07).""")

b += h3("Arrays")
b += p("""`array<T, N>` holds exactly `N` elements; `N` is a literal. The literal form is `[a, b,
c]`. Indexing takes a `usize` and is bounds-checked: a literal index out of range is rejected
statically, a computed one faults at run time with `diag.index-out-of-bounds`. Arrays of plain
elements are plain (copied on assignment); an array of resources is a resource.""")
b += code(CHECK + """
fn sum(ref<array<i32, 4>, shared> a) : i32
{
    i32 t = 0;
    usize i = 0;
    while (i < 4)
    {
        t = t + a[i];                 // indexing looks through the reference
        i = i + 1;
    }
    t
}

fn main()
{
    array<i32, 4> a = [10, 20, 30, 40];
    a[2] = 33;
    check(sum(&a) == 103);
    auto grid = [[1, 2], [3, 4]];     // array<array<i32, 2>, 2>
    check(grid[1][0] == 3);
}
""", title="Fixed-size arrays", expect="ok", section="s16", slug="arrays")
b += code("""
fn get(array<i32, 3> a, usize i) : i32
{
    a[i]                              // checked against 3 when the index arrives
}

fn main()
{
    array<i32, 3> a = [1, 2, 3];
    get(a, 3);                        // ✗ diag.index-out-of-bounds (dynamic)
}
""", title="Out of bounds", expect="diag.index-out-of-bounds", section="s16", slug="oob")
b += p("""For a growable sequence use `Vec<T>` (§21). A `Vec` is indexed the same way, `v[i]`: it
reads through `Vec::index_shared`, and writes (an assignment, `&mut v[i]`) through
`Vec::index_exclusive`, so every rule about borrowing the `Vec` applies.""")

b += h3("Slices")
b += p("""A slice is a borrowed run of consecutive elements of an array, a `Vec`, or another slice:
`&a[i..j]` is elements `i` up to (not including) `j`, and `&mut a[i..j]` the same for writing. Inside
the brackets, `$` stands for the length of what is being indexed, so `&a[0..$]` is all of it and
`&a[$ - 3 .. $]` the last three, and `v[$ - 1]` is a `Vec`'s last element. `$` is a `usize` like any
index, so `v[$ - 1]` of an empty `v` is an overflow fault, never a wrapped-around index.""")
b += p("""The type is `slice<T, shared>` or `slice<T, exclusive>`. One parameter of that type takes any
array, any `Vec`, or part of either, so a function need not be written once per container or per
array length. `slice_len(s)` is its length; `s[i]` is bounds-checked; `foreach` works over it. A
slice is a borrow like a reference: while it lives, what it views cannot be pushed to, popped from,
or otherwise changed except through it, and a slice of a local cannot be returned.""")
b += code(CHECK_STD + """
// Sorts any run of i32 in place: an array, a Vec, or part of one.
fn sort(slice<i32, exclusive> s)
{
    for (usize i = 1; i < slice_len(s); i += 1)
    {
        usize j = i;
        while (j > 0 && s[j - 1] > s[j])
        {
            i32 t = s[j - 1];
            s[j - 1] = s[j];
            s[j] = t;
            j -= 1;
        }
    }
}

fn sum(slice<i32, shared> s) : i32
{
    i32 total = 0;
    foreach (x in s)
    {
        total += *x;
    }
    total
}

fn main()
{
    auto a = [5, 3, 9, 1, 7, 2];
    sort(&mut a[0..3]);                     // only the first three: [3, 5, 9, 1, 7, 2]
    check(a[0] == 3 && a[3] == 1);
    check(sum(&a[$ - 3 .. $]) == 10);       // the last three

    Vec<i32> v = Vec::new();
    foreach (x in [40, 10, 30, 20])
    {
        Vec::push(&mut v, x);
    }
    sort(&mut v[0..$]);                     // the same function, on a Vec
    check(v[0] == 10 && v[$ - 1] == 40);
    v[0] += 1;                              // indexing a Vec, written
    check(sum(&v[0..$]) == 101);
}
""", title="One function for arrays, Vecs and parts of them", expect="ok", section="s16", slug="slices")
b += code("""
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 1);
    auto s = &v[0..1];
    Vec::push(&mut v, 2);             // ✗ diag.aliasing-conflict: s still borrows v
}
""", title="A slice keeps its source borrowed", expect="diag.aliasing-conflict", section="s16", slug="slice-borrow")

b += h3("Enums and `match`")
b += p("""An enum lists its variants; each may carry one payload type. Variant names are used
bare (`Cold(3)`, `Fine`) or qualified (`Verdict::Fine`) when two visible enums share a name. A
variant with no payload behaves as if it carried `()`.""")
b += p("""`match (e) { … }` compares the discriminant and runs the arm for that variant. Each arm is
`Variant(binder) : expression,` for a payload, `Variant : expression,` for none, or
`_ : expression,` as a catch-all; every arm ends with a comma. The arms must cover every variant
or include `_`. Arms are expressions, so a `match` produces a value; use a block as the arm body
for statements. Arms are tried in order and the first that fits is taken.""")
b += code(CHECK + """
enum Shape
{
    Circle(i32),                      // radius
    Square(i32),                      // side
    Dot
}

fn area(Shape s) : i32
{
    match (s)
    {
        Circle(r) : 3 * r * r,        // the payload is bound to r for this arm
        Square(w) : w * w,
        Dot       : 0,
    }
}

fn is_round(Shape s) : bool
{
    match (s)
    {
        Circle(_) : true,             // _ as a binder ignores the payload
        _         : false,            // a wildcard arm covers the rest
    }
}

fn main()
{
    check(area(Circle(2)) == 12);
    check(area(Shape::Dot) == 0);
    check(is_round(Square(1)) == false);
    array<Shape, 2> shapes = [Circle(1), Square(3)];
    check(area(shapes[1]) == 9);
}
""", title="Enums", expect="ok", section="s16", slug="enums")
b += code("""
enum Sign
{
    Pos,
    Neg,
    Zero
}

fn describe(Sign s) : i32
{
    match (s)
    {
        Pos : 1,
        Neg : -1,                     // ✗ diag.non-exhaustive-match: Zero is not covered
    }
}

fn main()
{
    describe(Zero);
}
""", title="A missing arm", expect="diag.non-exhaustive-match", section="s16", slug="non-exhaustive")
b += h4("Nested patterns")
b += p("""Inside the parentheses, a pattern can say what the payload itself must be, as deep as
needed: `Ok(Some(line))` matches an `Ok` whose payload is a `Some`, and binds the `Some`'s
payload. A name there is a binder, unless it is a variant's name (`Some(None)`), and `_` ignores
that level. Since a variant has one payload, an arm binds at most one name, the innermost payload
it reaches. Every value must reach some arm (the error names one that does not, such as
`Ok(None)`), and every arm must be reachable: an arm whose values all go to earlier arms is
rejected, since it could never run.""")
b += code(CHECK_STD + """
fn classify(Result<Option<i32>, ParseError> r) : i32
{
    match (r)
    {
        Ok(Some(n)) : n,              // two levels tested, the inner payload bound
        Ok(None) : 0,
        Err(Invalid(at)) : -100 - narrow<i32>(at),
        Err(_) : -1,
    }
}

fn main()
{
    check(classify(Ok(Some(7))) == 7);
    check(classify(Ok(None)) == 0);
    check(classify(Err(Invalid(3))) == -103);
    check(classify(Err(Empty)) == -1);
}
""", title="Nested patterns", expect="ok", section="s16", slug="nested-patterns")
b += code(CHECK_STD + """
fn main()
{
    Option<Option<i32>> o = Some(None);
    i32 v = match (o)
    {
        Some(Some(x)) : x,
        Some(None) : 1,
        Some(_) : 2,                  // ✗ diag.unreachable-arm: every Some went to the arms above
        None : 0,
    };
}
""", title="An arm that can never run", expect="diag.unreachable-arm", section="s16", slug="unreachable-arm")
b += h4("Literal patterns")
b += p("""A pattern can also be a literal: an integer (`3`, `-1`, `0x10`), a byte (`b'x'`), `true` or
`false`. It can stand alone, when the value being matched is an integer or a `bool`, or at the end
of a nested pattern: `Ok(0)`, `Some(b'\\n')`. This is CobaltC's multi-way branch in place of `switch`,
and it is checked: a literal must be of the type it is compared with, a repeated case is an arm that
can never run, and an integer `match` needs a `_` arm, since literals never list every value
(`true` and `false` do cover a `bool`). Floats are not patterns. With that, patterns are `_`, a
literal, a variant, and a variant with a pattern for its payload; there is nothing else to learn.""")
b += code(CHECK_STD + """
fn step(u8 op, i64 acc) : i64
{
    match (op)
    {
        0x01 : acc + 1,               // a byte from an instruction stream
        0x02 : acc * 2,
        0x10 : 0,
        _ : acc,                      // any other opcode: no effect
    }
}

fn is_end(Result<usize, FileError> r) : bool
{
    match (r)
    {
        Ok(0) : true,                 // a read of 0 bytes: the end of the file
        Ok(_) : false,
        Err(_) : true,
    }
}

fn main()
{
    array<u8, 5> program = [0x01, 0x02, 0x02, 0x7f, 0x01];
    i64 acc = 0;
    foreach (op in program)
    {
        acc = step(op, acc);
    }
    check(acc == 5);
    check(is_end(Ok(0)) && !is_end(Ok(16)));
}
""", title="Literal patterns", expect="ok", section="s16", slug="literal-patterns")
b += code(CHECK_STD + """
fn main()
{
    u8 op = 2;
    i32 cycles = match (op)
    {
        1 : 3,
        2 : 5,
        1 : 4,                        // ✗ diag.unreachable-arm: 1 is taken by the first arm
        _ : 1,
    };
}
""", title="A repeated case", expect="diag.unreachable-arm", section="s16", slug="repeated-case")
b += h4("Matching through a reference")
b += p("""`match (e)` binds each payload by value: a plain one is copied, and a resource one (a
`String`, a `Vec`, a `Box`) is moved out, which consumes the enum. To look at an enum without
taking it apart, match a reference to it: `match (&e)` binds each payload as a shared reference,
`match (&mut e)` as an exclusive one, through which the payload can be changed. A scrutinee that
already is a reference, such as a function's `ref<Expr, shared>` parameter, binds in its own mode.
The enum is not consumed, and the usual borrow rules apply to what is bound: nothing can replace
the enum while a payload reference is still in use. This is how a tree built from `Box`es (§21)
is walked.""")
b += code(CHECK_STD + """
enum Expr
{
    Num(i64),
    Neg(Box<Expr>),
}

fn eval(ref<Expr, shared> e) : i64
{
    match (e)                         // e is a reference: n and x are references too
    {
        Num(n) : *n,
        Neg(x) : -eval(Box::get(x)),
    }
}

fn main()
{
    Expr e = Neg(Box::new(Neg(Box::new(Num(7)))));
    check(eval(&e) == 7);

    Option<String> name = Some(String::from_str("ada"));
    match (&mut name)                 // change the payload in place
    {
        Some(s) : String::push_ascii(s, b'!'),
        None : {},
    }
    usize len = match (&name)         // look without consuming
    {
        Some(s) : String::len(s),
        None : 0,
    };
    check(len == 4);
    String kept = Option::unwrap_or(name, String::new());   // name still owns its String
    check(String::len(&kept) == 4);
}
""", title="Looking inside without taking apart", expect="ok", section="s16", slug="match-by-ref")

b += h4("An arm that takes the payload consumes the enum")
b += p("""If the enum carries a resource payload, an arm that binds that payload by value
<em>moves</em> it into the binder and consumes the enum: if that arm ran, the scrutinee is stale
afterwards. This is what makes `match (Vec::pop(&mut v)) { Some(x) : …, None : … }` hand you the
popped element. An arm that takes nothing out — `_`, a payload-less variant, or `Some(_)` — leaves
the scrutinee alone, so `match (o) { None : …, _ : use(o) }` is fine. To look inside and keep the
enum whatever the arm, match on a reference to it (above).""")
b += code(CHECK_STD + """
enum Slot
{
    Full(Vec<i32>),
    Empty
}

fn main()
{
    Vec<i32> payload = Vec::new();
    Vec::push(&mut payload, 4);
    auto s = Slot::Full(payload);     // payload moves into the enum
    usize n = match (s)               // the Full arm consumes s
    {
        Full(v) : Vec::len(&v),       // v owns the Vec for the arm, and destroys it at the arm's end
        Empty   : 0,
    };
    check(n == 1);

    Option<String> o = Some(String::from_str("kept"));
    match (o)
    {
        None : {},
        _ :                           // takes nothing out: o is still here
        {
            String t = Option::unwrap_or(o, String::new());
            check(String::len(&t) == 4);
        },
    }
}
""", title="A resource payload moves out", expect="ok", section="s16", slug="resource-payload")

b += h3("Generic aggregates")
b += p("""`struct Box<T> { T value; }` and `enum Maybe<T> { Just(T), Nothing }` declare families of
types. `std`'s `Option<T>` and `Result<T, E>` are ordinary enums of exactly this kind (§21).
Type arguments may be left off a literal when the declared type of the variable, parameter or field
fixes them (§12).""")

b += h3("Layout")
b += p("""Struct fields are laid out in declaration order with implementation-documented padding;
arrays are contiguous with no padding; an enum is a discriminant of documented width followed by
the largest payload, aligned. `sizeof<T>()` and `alignof<T>()` report the result (examples in §06,
"Sizes and limits of types"). You need this mostly when talking to foreign code or managing raw
memory (§20).""")

b += rules([
    "Reach for an enum whenever a value is one of several cases; `match` forces you to handle each one.",
    "Use `_` sparingly: a wildcard arm silently absorbs variants you add later.",
    "Give match arms a block body when they need statements, and remember the comma after each arm.",
    "Build aggregates whole with a literal, then mutate fields as places.",
])
S16 = Section("s16", "§16", "Structs, arrays, enums and match", "16-aggregates.md",
              "The three aggregate kinds, how they nest, and exhaustive matching over enums with payloads.", b)

SECTIONS = [S12, S13, S14, S15, S16]
