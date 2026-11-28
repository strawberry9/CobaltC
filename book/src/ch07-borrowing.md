# Borrowing

The last chapter left a gap. A resource has one owner, and giving it to a
function moves it, so how does a function look at a vector without taking it?
The answer is a **reference**: a way to reach a value that someone else owns.
References are the mechanism behind most of what makes CobaltC safe, and
they are also the part of the language that takes longest to feel natural,
so this chapter goes slowly.

The idea underneath is one rule, and it is worth stating before any syntax.
At any moment, a value may have **any number of readers, or one writer, but
never both**. Everything else in the chapter is the working-out of that
sentence.

## Two kinds of reference

```
import std;

fn length(ref<String, shared> s) : usize     // looks: may read, cannot change
{
    StringView::len(s)
}

fn shout(ref<String, exclusive> s)           // changes: may read and write
{
    String::push_ascii(s, b'!');
}

fn main()
{
    String greeting = "hello";
    printf("%v\n", length(&greeting));       // lend it for reading
    shout(&mut greeting);                    // lend it for writing
    shout(&mut greeting);
    printf("%v, %v bytes\n", &greeting, length(&greeting));
}
```

```
5
hello!!, 7 bytes
```

`&x` makes a *shared* reference to `x`, of type `ref<T, shared>`; through it
you may read. `&mut x` makes an *exclusive* reference, `ref<T, exclusive>`;
through it you may read and write. The word for making one is *borrowing*:
the owner lends the value, and the reference is the loan. The owner keeps
owning the value throughout; a reference never destroys what it points at.

The mode is part of the reference's type, so a function's signature says
exactly what it may do. `length` takes `ref<String, shared>`, and the caller
knows the string will come back unchanged. `shout` takes
`ref<String, exclusive>`, and the caller knows it may not. There is no way
for `length` to cheat: writing through a shared reference is rejected before
the program runs.

A reference is used with `*` to reach the value, `*r`; for a field or an
element, `r.field` and `r[i]` work directly, as they would on the value
itself. References are plain values, copied freely and never destroyed;
the thing they point at is what has an owner.

## The one rule

```
import std;

fn main()
{
    i32 x = 1;
    ref<i32, shared> a = &x;
    ref<i32, shared> b = &x;          // any number of readers may exist at once
    printf("%v %v %v\n", x, *a, *b);  // and the owner may still read

    ref<i32, exclusive> w = &mut x;   // an exclusive reference: alone
    *w += 10;
    printf("%v\n", *w);
    printf("%v\n", x);                // w's last use is above, so x is free again
}
```

```
1 1 1
11
11
```

Any number of shared references may coexist, and while they do, the owner
may read as well. An exclusive reference must be the only way to reach the
value while it lives: no other reference, and no use of the owner's own name.
Here is what happens when that is broken:

```
import std;

fn main()
{
    i32 x = 1;
    ref<i32, shared> r = &x;
    x = 2;                            // ✗ r still reads x
    printf("%v\n", *r);
}
```

```
error[diag.aliasing-conflict]: `r` still borrows `x`, so it cannot be written or borrowed `mut` here; end that borrow first
 --> conflict.cb:7:5
  |
7 |     x = 2;                            // ✗ r still reads x
  |     ^
  = repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides
```

The write to `x` conflicts with `r`, a live reader. The program is rejected
before it runs. The same diagnostic covers every case: writing while a
reader lives, making a second reference while an exclusive one lives, using
the owner's name while an exclusive reference lives.

Why is a rule this strict worth having? Because the thing it forbids is the
root of most memory bugs. Here is the same conflict in the form it usually
takes:

```
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 10);
    ref<i32, shared> first = &v[0];   // points into v's buffer
    Vec::push(&mut v, 20);            // ✗ the push may move the buffer
    printf("%v\n", *first);
}
```

```
error[diag.aliasing-conflict]: `first` still borrows `v`, so it cannot be written or borrowed `mut` here; end that borrow first
 --> vec_push_conflict.cb:8:15
  |
8 |     Vec::push(&mut v, 20);            // ✗ the push may move the buffer
  |               ^
  = repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides
```

`first` points into the vector's buffer. The push may have to grow the
buffer, which means allocating a new one and freeing the old, after which
`first` would point at freed memory. In C this is a bug that may not show for
weeks. Here it cannot be written. The rule does not know about buffers or
reallocation; it knows only that `Vec::push` takes `&mut v` while `first`
still borrows `v`, and that is enough.

## When a borrow ends

A reference lives as long as something holds it. That makes "end that borrow
first" a matter of where the reference is used, and the language is generous
about it:

```
import std;

fn main()
{
    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 10);            // this borrow ends at the semicolon
    Vec::push(&mut v, 20);            // so this one is fine

    {
        ref<i32, exclusive> last = &mut v[1];
        *last = 25;
    }                                 // last ends with its block

    ref<i32, shared> first = &v[0];
    printf("%v\n", *first);           // first's last use: it ends here
    Vec::push(&mut v, 30);            // so v is free again
    printf("%v %v %v\n", v[0], v[1], v[2]);
}
```

```
10
10 25 30
```

Three ways a borrow ends. A reference passed straight to a call, `&mut v` in
`Vec::push(&mut v, 10)`, lives for that statement and is gone at the
semicolon; consecutive calls that each borrow the same variable are
therefore always fine. A reference kept in a variable ends when that
variable's block ends. And a reference kept in a variable ends early, after
the *last statement that uses it*, so `first` above is gone before the push
that follows its `printf`. A use inside a loop counts as a use by the whole
loop, so a reference used in a loop body lives through the loop.

When the checker complains, then, the fix is one of two things: move the
conflicting access below the reference's last use, or put the reference in a
block of its own so that its end is visible. Both make the program clearer.

## Borrows are precise

A conflict is about *overlap of storage*, not about which variable a
reference came from. Two exclusive references to different fields of one
struct do not overlap and may live at once:

```
import std;

struct Pair
{
    i32 left;
    i32 right;
}

fn main()
{
    Pair p = Pair { .left = 1, .right = 2 };
    ref<i32, exclusive> l = &mut p.left;
    ref<i32, exclusive> r = &mut p.right;    // a different field: no overlap
    *l += 10;
    *r += 20;
    printf("%v %v\n", p.left, p.right);
}
```

```
11 22
```

The same holds for two slices of different parts of one vector, which
chapter 13 uses to let two threads fill one vector at once.

## What the checker sees, and what the run checks

So far every conflict was found before the program ran. That holds across
calls too. The checker knows what each function does through each reference
parameter (whether it reads or writes, and which fields), so a call that
hands a function two references that meet is rejected where it is written:

```
import std;

fn set_and_read(ref<i32, exclusive> a, ref<i32, shared> b) : i32
{
    *a = 1;
    *b
}

fn main()
{
    i32 x = 0;
    i32 y = 0;
    printf("%v\n", set_and_read(&mut x, &y));    // fine: two different variables
    printf("%v\n", set_and_read(&mut x, &x));    // ✗ rejected: both reach x
}
```

```
error[diag.aliasing-conflict]: `set_and_read` writes `x` through argument 1, which argument 2 also reaches
  --> call_conflict.cb:14:20
   |
14 |     printf("%v\n", set_and_read(&mut x, &x));    // ✗ rejected: both reach x
   |                    ^^^^^^^^^^^^
   = repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides
```

`set_and_read` writes through its first parameter while its second is in
use, and both arguments reach `x`. The program is rejected, and the first call
never runs either.

Only the values can decide some questions. Two elements of one array, chosen
by indexes the program computes, are the same element only when the indexes
are equal:

```
import std;

fn set_and_read(ref<i32, exclusive> a, ref<i32, shared> b) : i32
{
    *a = 1;
    *b
}

fn main()
{
    array<i32, 3> v = [0, 0, 0];
    usize i = 0;
    while (i < 3)
    {
        usize j = 2 - i;
        printf("%v\n", set_and_read(&mut v[i], &v[j]));    // ✗ at run time when i == j
        i = i + 1;
    }
}
```

```
0
fault[diag.aliasing-conflict]: `set_and_read` may write through argument 1, and a live reference reaches the same place: the call is checked when it is made (D-0283)
  --> index_conflict.cb:16:9
   |
16 |         printf("%v\n", set_and_read(&mut v[i], &v[j]));    // ✗ at run time when i == j
   |         ^^^^^^
   = repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides
```

The first call passes elements 0 and 2, which are different. The second passes
element 1 twice: `set_and_read` may write through `a`, and `b` reaches the same
element, so the call faults before its body runs. The rule is the same one as
before; only the moment it is applied differs. The check is made at the call,
from what the function may do with its arguments, so it does not depend on
which way the body would have gone on this run.

This is the pattern for the whole language. Every rule is checked. Where the
text of the program decides the question, it is decided before the run; where
only the values can decide it, such as two indexes or a reference kept inside a
data structure, it is decided during the run. A conforming compiler may prove
more and leave out checks it has proved unnecessary, which is what `cobc` does,
but it may never accept a program the language rejects or run a check the
language does not require.

When nothing is left to the run, because every question of a program is
decided by its text, the program is *decided*, and it keeps no record of its
references while it runs: there is nothing such a record could find. `cobc`
builds every decided program that way, and `cobc -v` says whether it did or
names the first line that left a question to the run (D-0284).

## Returning a reference

A function may return a reference, a view, or a value that holds one, such
as an `Option<ref<T, shared>>`. The checker must know what the result points
into, so that it can check the caller's use of it. With one reference
parameter there is only one answer: the result is understood to point into
whatever that parameter pointed into, and the caller is checked accordingly.
With two or more, the function says which with a `from` clause, as the next
section shows.

```
import std;

struct Pair
{
    i32 left;
    i32 right;
}

// One reference parameter: the result is understood to point into it.
fn larger(ref<Pair, shared> p) : ref<i32, shared>
{
    if (p.left > p.right) { &p.left } else { &p.right }
}

fn main()
{
    Pair p = Pair { .left = 3, .right = 7 };
    ref<i32, shared> big = larger(&p);
    printf("%v\n", *big);
}
```

```
7
```

After that call, `big` counts as a borrow of `p`, exactly as if `main` had
written `&p.right` itself, and a write to `p` while `big` is in use is a
conflict. This is the shape of most accessors in the standard library, and it
needs nothing written, because there is only one parameter the result could
come from.

### Saying where a result comes from

With two or more reference parameters the language does not guess which one
the result points into. The function says so itself, with a `from` clause after
the result type, naming the parameters the result may come from. The clause is
a promise, and both sides of the call are held to it:

- In the function's body, a parameter the clause names may be returned or
  kept in the result. A reference parameter it leaves out may only be *looked
  at*: compared, measured, searched for, passed to something that keeps
  nothing. Returning it, storing it or capturing it is rejected
  (`diag.result-source-kept`).
- At each call, every argument the clause names stays borrowed for as long as
  the result is in use, and the others are free again as soon as the call
  returns.

Most functions name one source. Here the key is only searched for, so `main`
may change it while it still uses the result:

```
import std;

// The text after `key` in `line`. The result is part of `line`; `key` is
// only searched for and measured.
fn after(StringView line, StringView key) : StringView from line
{
    match (StringView::find(line, key))
    {
        Some(i) : &line[i + StringView::len(key)..$],
        None    : &line[0..0],
    }
}

fn main()
{
    String line = "name=ada";
    String key = "name=";
    StringView v = after(&line, &key);
    String::append(&mut key, "!");    // fine: key was only looked at
    printf("%s\n", v);
}
```

```
ada
```

The promise binds the body as well. A version of `after` that, on an empty
line, hands back `key` instead breaks it, and is rejected where it does:

```
import std;

// The clause names only `line`, so `key` may only be looked at.
fn after(StringView line, StringView key) : StringView from line
{
    if (StringView::len(line) == 0)
    {
        return key;                       // ✗ key is not a source of the result
    }
    &line[1..$]
}

fn main()
{
    String line = "name=ada";
    printf("%s\n", after(&line, "name="));
}
```

```
error[diag.result-source-kept]: `after` keeps `key` here, but `from line` says its result comes from `line` alone, so `key` may only be probed: compare it, measure it, search it, or pass it where nothing can keep it; or name `key` in the clause
 --> returning_kept.cb:8:16
  |
8 |         return key;                       // ✗ key is not a source of the result
  |                ^^^
  = repair: read only what is needed from it (its length, a comparison, a search's position), or name it in the clause instead
```

The repair is the one the message gives: name `key` in the clause too, if the
result really may be it (and accept that callers then keep `key` borrowed), or
return part of `line` on that path as well.

The standard library declares its own functions the same way:
`StringView::split(v, sep)` is `from v`, and `HashMap::get(m, key)` is `from m`,
so the key you look something up with is never held by the answer.

When the result may be any of several parameters, depending on what happens
when the program runs, name all of them. A lookup with a default is the common
case:

```
import std;

// The value for `key`, or `fallback` when there is none.
fn setting(ref<HashMap<String, String>, shared> config, StringView key, ref<String, shared> fallback) : ref<String, shared> from config, fallback
{
    match (HashMap::get(config, key))
    {
        Some(v) : v,
        None    : fallback,
    }
}

fn main()
{
    HashMap<String, String> config = HashMap::new();
    HashMap::insert(&mut config, "colour", "blue");
    String plain = "none";
    ref<String, shared> c = setting(&config, "colour", &plain);
    ref<String, shared> s = setting(&config, "size", &plain);
    printf("%s %s\n", c, s);
}
```

```
blue none
```

The checker does not follow which source a particular call returns; it knows
only that the result is one of them. So while `c` is in use, neither `config`
nor `plain` may be changed, even though `c` is in fact one of `config`'s
values. The same holds for the classic example of a function that returns
whichever of its two arguments is longer:

```
import std;

// Either argument: the clause names both, and a call borrows both.
fn longer(ref<String, shared> a, ref<String, shared> b) : ref<String, shared> from a, b
{
    if (StringView::len(a) >= StringView::len(b)) { a } else { b }
}

fn main()
{
    String x = "borrow";
    String y = "own";
    ref<String, shared> l = longer(&x, &y);
    printf("%s\n", l);
}
```

```
borrow
```

The run gave `x`, but adding `String::append(&mut y, "ership");` before the
`printf` is rejected with `diag.aliasing-conflict`: as far as the checker
knows, `l` may be `y`. For the same reason the result may not outlive the
shortest-lived argument it may come from.

**When to use it.** Write `from` when a function with two or more reference
parameters returns a reference, a view, or a value holding them (such as
`Option<ref<T, shared>>` or `Vec<StringView>`). Name only the parameters the
result can really come from; leave out the ones the body only looks at. With
one reference parameter, leave it off: the source is implied. Without the
clause, such a function is rejected (`diag.lifetime-elision-ambiguous`).

**What it costs.** Nothing when the program runs: the clause is checked
before it runs and adds no instructions or checks to the compiled program.
If anything it helps: knowing a left-out view argument is only looked at,
the compiler can pass it without a tracked copy of its own. The cost is to the
callers' freedom: each named argument stays borrowed while the
result is used, so naming a parameter the result never comes from is allowed
but needlessly ties up the caller's value. And a parameter the clause leaves
out must be looked at directly in the body: copying it into a local first,
even only to measure the local, is rejected, because the checker does not
follow where the copy goes.

**What it cannot say.** One clause covers the whole result. A result whose
parts come from different arguments, such as a pair holding one reference
from each, cannot be described; CobaltC has no lifetime annotations with which
to say it. Nor can it say that one argument outlives another. Return positions
(indices) or owned values instead.

A reference to a local cannot leave the function that owns the local:

```
import std;

fn dangling() : ref<i32, shared>
{
    i32 local = 5;
    &local                            // ✗ local ends when this call returns
}

fn main()
{
    printf("%v\n", *dangling());
}
```

```
error[diag.reference-escapes-scope]: the result refers to `local`, which ends when this function returns; a returned reference must come from a reference parameter -- return an owned value instead
 --> escape.cb:6:5
  |
6 |     &local                            // ✗ local ends when this call returns
  |     ^
  = repair: return an owned value, or restructure
```

The same rule stops a reference from being stored into a variable of an
enclosing block. Where an escape is not visible in the text, because the
reference was stored through another reference, say, the run-time check
takes over: the first use of a reference whose target has ended is a fault.
One way or another, a reference never outlives what it points at.

## Reading through a shared reference is read-only all the way down

A shared reference gives read access to everything it reaches. If the struct
it points at holds an exclusive reference to something else, that reference
comes out shared when read through the shared one. There is no path from a
shared reference to a write, however many steps it takes. The one exception
is a mutex, whose purpose is exactly to allow a change through a shared
handle, under a lock; chapter 13 has it.

## Choosing a parameter type

Every function parameter is one of four things, and the choice is the
function's contract with its caller:

| Parameter | The caller writes | The function may | Afterwards |
|---|---|---|---|
| plain `T` | `f(x)` | use its own copy | the caller's `x` is unchanged |
| resource `T` | `f(v)` | own it: keep it, or let it be destroyed | the caller's `v` is stale |
| `ref<T, shared>` | `f(&x)` | read through it | `x` is unchanged |
| `ref<T, exclusive>` | `f(&mut x)` | read and write through it | `x` may have changed |

Choose the one that promises the least. A function that only reads takes a
shared reference. One that changes a value in place takes an exclusive
reference. One that keeps the value, pushing it into a container or handing
it to a thread, takes it by value. That decision, made once per function, is
most of what there is to borrowing in practice; the checker does the rest.

Text follows the same rule with its own types: a function that only reads
text takes a `StringView`, which a literal, a `str`, a view and `&s` of a
`String` all are, and one that keeps owned text takes a `String`. Chapter 10
explains the few places a `str` parameter is right.
