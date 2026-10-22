# Generics and closures

Two features let one piece of code serve many cases. A **generic** function
or type takes a type as a parameter, so that `Vec<i32>` and `Vec<String>`
are one declaration. A **closure** is a function written inline that can use
the variables around it, so that a sorting function can be told how to
compare, or a loop body can be handed to another function. Both are
deliberately plainer in CobaltC than in most languages, and this chapter
says exactly how plain.

## Generic functions and types

```
import std;

struct Pair<T>
{
    T first;
    T second;
}

fn swap_pair<T>(Pair<T> p) : Pair<T>           // any T: a value can be moved around
{
    Pair { first, second } = p;       // take the pair apart (a field cannot be moved out alone)
    Pair { .first = second, .second = first }
}

fn largest<T: ordered>(slice<T, shared> xs) : T   // T: ordered gives it < and >
{
    T best = xs[0];
    foreach (x in xs)
    {
        if (*x > best)
        {
            best = *x;
        }
    }
    best
}

fn total<T: number>(slice<T, shared> xs) : T      // T: number gives it + and a zero
{
    T sum = 0;
    foreach (x in xs)
    {
        sum += *x;
    }
    sum
}

fn main()
{
    Pair<i32> p = swap_pair(Pair { .first = 1, .second = 2 });   // T is i32, from the argument
    printf("%v %v\n", p.first, p.second);
    Pair<String> q = swap_pair(Pair { .first = String::from_str("a"), .second = String::from_str("b") });
    printf("%v %v\n", &q.first, &q.second);

    array<i32, 4> a = [4, -2, 9, 1];
    array<f64, 3> f = [1.5, 2.25, 0.5];
    printf("%v %v %v %v\n", largest(&a[0..$]), largest(&f[0..$]), total(&a[0..$]), total(&f[0..$]));
}
```

```
2 1
b a
9 2.25 12 4.25
```

A type parameter is written in angle brackets after the name, and used like
a type in the body. Each use with a different concrete type is a separate
copy of the function, compiled as if you had written it out: `largest` for
`i32` and `largest` for `f64` are two functions. There is no run-time type
information and no indirect call; the cost is one copy per combination of
types used.

With no bound, a value of type `T` can be stored, moved, passed, returned and
referenced, and nothing else. `swap_pair` needs only that. To do more, the
parameter needs a **bound**, and there are six to choose from, each naming a
fixed set of built-in types:

| Bound | Satisfied by | Allows |
|---|---|---|
| `eq` | numbers, `bool`, `str`, `String`, and any struct or enum built from those (a *key type*) | `==` `!=` |
| `ordered` | the same | also `<` `<=` `>` `>=` |
| `number` | integers and floats | also arithmetic, and a literal like `0` |
| `integer` | integers | also the bitwise operators and shifts |
| `clone` | all of the above; `String`, the containers, and any struct or enum of `clone` parts | `clone(&x)`, an independent copy |
| `plain` | numbers, `bool`, raw pointers, and arrays and structs of those | moving the bytes through raw memory |

A function whose body uses an operator its bound does not give is rejected
where it is written, not where it is called:

```
import std;

fn larger<T>(T a, T b) : T
{
    if (a > b) { a } else { b }       // ✗ a bare T cannot be compared
}

fn main()
{
    printf("%v\n", larger(1, 2));
}
```

```
error[diag.unbounded-type-parameter]: `>` on a `T` needs a bound: declare `<T: ordered>` (D-0090), or pass the operation in as a `fn` parameter
 --> unbounded.cb:5:9
  |
5 |     if (a > b) { a } else { b }       // ✗ a bare T cannot be compared
  |         ^
  = repair: give `T` the bound the operator needs (`<T: ordered>`, D-0090), or pass the operation in as a `fn` parameter
```

That is the whole of bounds. There are no traits, no interfaces, and no way
to declare a new bound. Your own struct can be `clone`, by its fields, and
it is `eq` and `ordered` when it is a key type: built from integers,
`bool`, `str`, `String` and other such types, with no float, reference or
container in it. Its `==` compares field by field, and its `<` is the order
`Vec::sort` gives it, field by field in declaration order (an enum by the
order of its variants, then its payload), so `a < b` holds exactly when
sorting puts `a` first. When generic code needs to combine values of your
own types, or order a struct otherwise than by its fields, it takes the
operation as a function parameter, which is what `Vec::sort_by` does and
what the last example in this chapter shows. The restriction is a
deliberate one. It keeps generic code to plumbing, moving values in and out
of containers, and keeps the behaviour of every operator fixed by the
language: nothing a program writes changes what `<` or `==` means.

## Where a type parameter comes from

Inference is local and flows downward only. A type parameter is found from,
in order: an explicit argument, `identity<i64>(5)`; the type of an argument
in the same call; or the type expected where the call appears, such as the
declared type of the variable being initialised. Nothing is ever inferred
from how a value is used later:

```
import std;

fn main()
{
    Vec<u8> bytes = Vec::new();       // T from the declaration
    Vec::push(&mut bytes, 200);       // the literal is a u8, from the Vec's T
    auto v = Vec::new();              // ✗ nothing says what T is
    Vec::push(&mut v, 1);
}
```

```
error[diag.cannot-infer-type-parameter]: nothing here fixes `T` of `Vec::new`: write `Vec::new<T>(…)`, or use the result where its type is declared
 --> inference.cb:7:14
  |
7 |     auto v = Vec::new();              // ✗ nothing says what T is
  |              ^^^
  = repair: `name<τ>(…)` or annotate the declaration
```

The habit this asks for is simple: write the type on the left for empty
containers and generic constructors, and use `auto` for everything whose
type the right-hand side already shows. In exchange, the type of every
expression can be read from the text around it, without following the
value forward through the function.

## Closures

A closure is a function written where it is used, with a list of the outer
variables it uses in square brackets:

```
import std;

fn apply_twice(fn(i32) : i32 f, i32 x) : i32
{
    f(f(x))
}

fn main()
{
    i32 k = 10;
    auto add_k = [k](i32 x)           // captures k; the body only reads it
    {
        x + k
    };
    printf("%v %v\n", add_k(5), apply_twice(add_k, 0));

    i32 total = 0;
    {
        auto add_to_total = [total](i32 x)      // the body writes total: an exclusive capture
        {
            total += x;
        };
        add_to_total(3);
        add_to_total(4);
    }                                 // the closure ends here and releases total
    printf("total %v\n", total);

    Vec<i32> v = Vec::new();
    Vec::push(&mut v, 7);
    auto first = move [v]()           // move: v now belongs to the closure
    {
        v[0]
    };
    printf("%v %v\n", first(), first());
}                                     // first is destroyed here, and its Vec with it
```

```
15 20
total 7
7 7
```

The bracket is the closure's **capture list**, and it must name exactly the
outer variables the body uses, no more and no fewer. It is documentation
that the compiler checks:

```
import std;

fn main()
{
    i32 a = 1;
    i32 b = 2;
    auto f = [a](i32 x)               // ✗ the body uses b as well
    {
        x + a + b
    };
    printf("%v\n", f(0));
}
```

```
error[diag.capture-list-mismatch]: the capture list does not match the names the closure uses
 --> capture_mismatch.cb:7:14
  |
7 |     auto f = [a](i32 x)               // ✗ the body uses b as well
  |              ^
  = repair: add the missing name(s) or remove the unused one(s); the list documents an already-derived fact, it cannot select a different capture set
```

*How* each variable is captured is decided by the body, not the list. A
variable the body only reads is captured by shared reference; one it writes
is captured by exclusive reference. Those references are ordinary borrows
with the usual consequences: while `add_to_total` exists, `total` is
exclusively borrowed and nothing else may touch it, which is why the example
puts the closure in a block. The `move` keyword captures by value instead,
moving resources into the closure, which then owns them and destroys them
when it is destroyed. A closure that will outlive the current block, or run
on another thread, is always a `move` closure.

A closure's type is a function type, `fn(i32) : i32` for `add_k`, so it can
be passed wherever a function of that signature is expected, stored in a
variable or a struct field, and called from there. A named function is a
value of the same kind. The result type is taken from the body, or from the
`fn` type the closure is passed as; when neither settles it, write it after
the parameters, `[](i64 a) : i64 { 0 }`.

Functions do not nest, so a helper needed only inside one function is
written as a closure. It is also how CobaltC does what other languages do
with iterators and lambdas: the library's `position`, `sort_by`, `retain`
and `from_fn` each take a function value and apply it.

## Functions as values

```
import std;

struct Person
{
    String name;
    u32 age;
}

fn younger(ref<Person, shared> a, ref<Person, shared> b) : bool
{
    a.age < b.age
}

fn main()
{
    Vec<Person> people = Vec::new();
    Vec::push(&mut people, Person { .name = "Ada", .age = 36 });
    Vec::push(&mut people, Person { .name = "Lin", .age = 29 });
    Vec::push(&mut people, Person { .name = "Kay", .age = 41 });

    Vec::sort_by(&mut people, younger);         // a named function as a value
    foreach (p in &people)
    {
        printf("%v %v\n", &p.name, p.age);
    }

    u32 limit = 35;
    Option<usize> i = Vec::position(&people, [limit](ref<Person, shared> p) : bool { p.age > limit });
    printf("first over %v: %v\n", limit, Option::unwrap_or(i, 99));
}
```

```
Lin 29
Ada 36
Kay 41
first over 35: 1
```

`Vec::sort_by` takes a function of two shared references that says whether
the first should come before the second; `Vec::position` takes a test and
returns the index of the first element that passes it. This is the pattern
the language offers in place of traits: the generic container does the
moving, and the caller supplies the one operation it cannot know. It is less
general than a trait system and considerably easier to read, since the
operation is visible at the call.
