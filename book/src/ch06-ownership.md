# Ownership

Some values are just data. An integer, a `bool`, a struct of two integers:
copy it and you have two of it, and nothing in the world has changed. Other
values stand for something that must be given back exactly once. A block of
heap memory, an open file, a lock that is held, a thread that is running: copy
one of these and you have two names for one thing, and whichever name
releases it first leaves the other dangling.

CobaltC draws this line in the type system. Values of the second kind are
called **resources**, and the language keeps three promises about them. A
resource has exactly one owner. It is never copied; it moves. And it is
destroyed exactly once, at a moment you can point to in the source, whether
or not you remembered to do anything. This chapter is about those three
promises. They are the reason there is no garbage collector, no leak, and no
use-after-free.

## Plain types and resource types

A type is a resource if it is declared with the word `resource`, or if any
part of it is a resource. Everything else is *plain*.

```
import std;

struct Point                          // plain: two numbers
{
    i32 x;
    i32 y;
}

struct Path                           // a resource: it contains a Vec
{
    Vec<Point> points;
}

fn main()
{
    Point a = Point { .x = 1, .y = 2 };
    Point b = a;                      // a copy: a and b are two points
    b.x = 10;
    printf("a.x %v, b.x %v\n", a.x, b.x);

    Path p = Path { .points = Vec::new() };
    Vec::push(&mut p.points, a);
    Path q = p;                       // a move: q owns the points now, p is stale
    printf("%v point(s)\n", Vec::len(&q.points));
}
```

```
a.x 1, b.x 10
1 point(s)
```

`Vec<T>` owns a buffer on the heap, so it is a resource, and `Path` is a
resource because it contains one; nobody had to say so. `String`, `HashMap`,
a file, a thread handle and a mutex are resources too. Numbers, `bool`,
`str`, references and plain structs are not.

The same assignment, `b = a`, means two different things for the two kinds.
For the plain `Point` it copies. For the resource `Path` it *moves*: `q`
becomes the owner, and `p` is left *stale*, a name with nothing behind it.
Nothing was copied, because a copy would have made two owners.

## A move leaves the old name stale

Every way of putting a resource somewhere moves it: initialising a variable
from it, passing it to a function by value, storing it in a field, returning
it. After the move the old name cannot be used:

```
import std;

fn total(Vec<i32> v) : i32            // takes the vector: it is destroyed when total ends
{
    i32 t = 0;
    foreach (x in v)
    {
        t += x;
    }
    t
}

fn main()
{
    Vec<i32> nums = Vec::new();
    Vec::push(&mut nums, 4);
    Vec::push(&mut nums, 5);
    printf("%v\n", total(nums));      // nums moves into the call
    printf("%v\n", Vec::len(&nums));  // ✗ nums no longer owns anything
}
```

```
error[diag.stale-binding]: `nums` was moved or dropped before this use, and holds no value here; give it a new value first (`nums = …;`)
  --> use_after_move.cb:19:30
   |
19 |     printf("%v\n", Vec::len(&nums));  // ✗ nums no longer owns anything
   |                              ^^^^
   = repair: use the current holder after a move, or give the binding a new value first (`x = e`, `spec/11` `[Assign-Reestablish]`); do not keep a reference past its referent's or its holder's scope; re-index after a `Vec` reallocation
```

The function `total` took the vector by value, so the call handed it over,
and `total` destroyed it when it finished. That was probably not what the
author wanted. If a function only needs to look at a value, it should take a
*reference* to it, `ref<Vec<i32>, shared>`, and the caller passes `&nums`;
chapter 7 is about exactly this. Taking a resource by value is the right
choice when the function is going to keep it: push it into a container, hand
it to a thread, store it in a struct it returns.

A name that was moved on only some paths, in one branch of an `if` say,
counts as moved. The rule is checked before the program runs wherever the
move is visible in the same function, and at run time in the few cases where
it is not. Either way, there are never two owners.

Because a resource cannot be copied, it cannot appear where a copy would be
needed. Two vectors cannot be compared with `==`, since comparing would copy
them both; chapter 10 has `slice_eq` for that. A resource field cannot be
read out of a struct that lives on, since that would leave the struct
half-owned; you move the whole struct, or take a reference to the field.

## Destruction is automatic and ordered

A resource is destroyed when its owner's scope ends. Locals are destroyed in
the reverse of the order they were declared, a struct's fields after the
struct itself, and a temporary that nothing stored at the end of its
statement. This program watches it happen:

```
import std;

resource struct Tracer
{
    str name;
}

fn Tracer::drop(ref<Tracer, exclusive> self)
{
    printf("  drop %v\n", self.name);
}

fn make(str name) : Tracer
{
    Tracer { .name = name }           // moved out to the caller, not destroyed here
}

fn consume(Tracer t)
{
    printf("  consume has %v\n", t.name);
}                                     // t is destroyed here, by its new owner

fn main()
{
    printf("a block:\n");
    {
        Tracer a = make("a");
        Tracer b = make("b");
        Tracer c = make("c");
    }                                 // c, then b, then a

    printf("an early drop:\n");
    {
        Tracer x = make("x");
        Tracer y = make("y");
        drop(x);                      // now, not at the block's end
        printf("  end of block\n");
    }

    printf("a move:\n");
    {
        Tracer m = make("m");
        consume(m);                   // m's destructor runs inside consume, once
        printf("  back in main\n");
    }

    printf("a temporary:\n");
    make("t");                        // not stored: destroyed at the end of this statement
    printf("done\n");
}
```

```
a block:
  drop c
  drop b
  drop a
an early drop:
  drop x
  end of block
  drop y
a move:
  consume has m
  drop m
  back in main
a temporary:
  drop t
done
```

`Tracer` is declared `resource` even though it holds only a `str`, because
it has a *destructor*: a function named `drop`, taking an exclusive reference
to the value, which the language calls exactly once when the value is
destroyed. You never call it yourself. A destructor is where a type closes
its file or releases its lock; `Vec` and `String` have one that frees their
memory. A plain type cannot have a destructor, since a plain value is copied
freely and is never destroyed.

`drop(x)` destroys a resource early. Afterwards `x` is stale until you give it
a new value, which it can take with an ordinary assignment, `x = make("z")`.

A `return`, a `break` or a fault leaves scopes the same way as reaching the
closing brace, destroying what each scope owns on the way out. That is why
a fault, which stops the program, still closes every file the program had
open, and why you will never write cleanup code for an early exit.

## Overwriting a resource

If `c` owns a connection and you assign `c = another`, what happens to the
first connection? For a type whose destruction only frees memory, a
`String` or a `Vec`, the answer is simple: the old value is destroyed and the
new one stored. The language calls such types *quiet*, and lets them be
overwritten freely. For a resource whose destruction does something you
would want to see, closing a connection, printing a line, the assignment is
rejected, and you must say what you mean:

```
import std;

resource struct Connection
{
    u32 id;
}

fn Connection::drop(ref<Connection, exclusive> self)
{
    printf("closing %v\n", self.id);
}

fn main()
{
    String s = "first";
    s = String::from_str("second");   // fine: a String only frees memory, so it is replaced quietly
    printf("%v\n", &s);

    Connection c = Connection { .id = 1 };
    overwrite(&mut c, Connection { .id = 2 });   // closes 1, then stores 2
    printf("now %v\n", c.id);
}                                     // closes 2
```

```
second
closing 1
now 2
closing 2
```

`overwrite(&mut c, v)` destroys the old value where you wrote it and stores
the new one. `replace(&mut c, v)` stores the new one and gives you the old
one back. Or `drop(c)` first and assign afterwards. The plain assignment is
refused:

```
import std;

resource struct Connection
{
    u32 id;
}

fn Connection::drop(ref<Connection, exclusive> self)
{
    printf("closing %v\n", self.id);
}

fn main()
{
    Connection c = Connection { .id = 1 };
    c = Connection { .id = 2 };       // ✗ 1 would be closed without a trace
}
```

```
error[diag.overwrite-of-live-resource]: `c` still holds a `Connection`, which this assignment would lose
  --> overwrite_rejected.cb:16:5
   |
16 |     c = Connection { .id = 2 };       // ✗ 1 would be closed without a trace
   |     ^
   = repair: `overwrite(&mut place, v)` to put `v` there and destroy the old value (D-0080), `replace(&mut place, v)` to keep the old value, or `drop` the old value first
```

## The shape of the rules

Three promises, then: one owner, moves not copies, destruction once and in
order. They cost you two habits. Decide, for every function, whether it keeps
what it is given or only looks at it, and write the parameter accordingly.
And put a resource in the smallest block that needs it, so that the block's
end releases it.

What they give you is the absence of a whole class of bugs. A resource cannot
leak, because every one has an owner whose scope ends. It cannot be freed
twice, because it has one owner and one destruction. It cannot be used after
it is freed, because the name that owned it is stale the moment it goes.
None of this needs a garbage collector, and all of it happens at a moment you
can read off the page.

The one thing missing is a way to *look* at a resource without taking it.
That is the next chapter.
