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
    String::len(s)
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

So far every conflict was found before the program ran. That is the common
case: wherever a function's own body shows a conflicting reference to be
live, the program is rejected. But a function cannot see what its caller
passed in:

```
import std;

fn set_and_read(ref<i32, exclusive> a, ref<i32, shared> b) : i32
{
    *a = 1;                           // ✗ at run time: b reaches the same i32
    *b
}

fn main()
{
    i32 x = 0;
    i32 y = 0;
    printf("%v\n", set_and_read(&mut x, &y));    // fine: two different variables
    printf("%v\n", set_and_read(&mut x, &x));    // accepted here; found at the first write
}
```

```
0
fault[diag.aliasing-conflict]: this access conflicts with a live reference
 --> dynamic_conflict.cb:5:5
  |
5 |     *a = 1;                           // ✗ at run time: b reaches the same i32
  |     ^
  = repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides
```

The first call is fine. The second passes two references to the same
variable, one exclusive. The call itself is accepted: the references are
formed one after another and neither is in use yet. Inside the function, the
checker sees two parameters and cannot know whether they overlap, so the
write through `a` is checked when it runs, and it faults, because `b` reaches
the same storage. The rule is the same one as before; only the moment it is
applied differs.

This is the pattern for the whole language. Every rule is checked. Where the
text of the function decides the question, it is decided before the run;
where only the values can decide it, it is decided during the run. A
conforming compiler may prove more and leave out checks it has proved
unnecessary, which is what `cobc` does, but it may never accept a program
the language rejects or run a check the language does not require.

## Returning a reference

A function may return a reference when it has exactly one reference
parameter. The result is understood to point into whatever that parameter
pointed into, and the caller is checked accordingly:

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
conflict. This is the shape of every accessor in the standard library, and it
needs no annotation because there is only one parameter the result could
come from. A function with two reference parameters may not return a
reference at all; the language does not guess, and CobaltC has no lifetime
annotations with which to tell it. Return an owned value, or split the
function.

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
