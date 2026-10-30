# Unsafe code and C

Everything so far has been checked by the language. But a systems language
has to reach memory it did not allocate and call code it did not compile,
and about those the language can check nothing. CobaltC's answer is not to
pretend. The operations it cannot check are allowed only inside an `unsafe`
block, and the block is a written assertion: you are stating, in a place
every reader can see, that a fact holds which the language cannot verify.
This chapter is about what those operations are, what each one asserts, and
what happens when an assertion is false.

## `unsafe` blocks and raw pointers

```
import std;

fn read_through(rawptr<i32> p) : i32
{
    unsafe
    {
        *p                            // asserted: p addresses a live, initialised i32
    }
}

fn main()
{
    i32 x = 7;
    rawptr<i32> p = rawptr_of(&x);    // taking an address is safe; using it is not
    printf("%v\n", read_through(p));

    array<u8, 3> bytes = [10, 20, 30];
    rawptr<u8> b = reinterpret_ptr<u8>(rawptr_of(&bytes));
    unsafe
    {
        printf("%v\n", *(b + 2));     // pointer offset, in elements
    }
}
```

```
7
30
```

A `rawptr<T>` is an address and nothing more: no mode, no lifetime, no
checks, a plain value copied freely. Getting one is safe, since an address
by itself harms nothing; `rawptr_of(&x)` takes the address of a place, and
`reinterpret_ptr<U>(p)` changes the pointee type without changing the
address. Using one, reading or writing `*p` or offsetting it with `p + n`,
is a trusted operation, and the block around it asserts that `p` addresses
initialised bytes of a `T` that no live reference reaches and that the
offset stays within one allocation.

Outside an `unsafe` block, a trusted operation is rejected before the
program runs:

```
import std;

fn read_through(rawptr<i32> p) : i32
{
    *p                                // ✗ a trusted operation outside unsafe
}

fn main()
{
    i32 x = 7;
    printf("%v\n", read_through(rawptr_of(&x)));
}
```

```
error[diag.trusted-outside-unsafe]: this needs an `unsafe` block
 --> no_unsafe.cb:5:5
  |
5 |     *p                                // ✗ a trusted operation outside unsafe
  |     ^
  = repair: wrap in `unsafe` after verifying the asserted property
```

An `unsafe` block changes nothing else. It is an ordinary block, with a
value and a scope, and every ordinary access inside it is checked as it
would be anywhere. Only the handful of trusted operations become available,
and a search for the word `unsafe` finds every one of them in a program.

## Manual allocation

`allocate(bytes, align)` asks for fresh memory and returns a `Result`, so
calling it is safe: failure is a value. Everything done with the memory
afterwards is trusted, and `deallocate` must be called exactly once with the
same size and alignment:

```
import std;

// A buffer of n zeroed u32s, built from raw memory and handed back as a Vec.
fn zeros(usize n) : Vec<u32>
{
    usize bytes = n * sizeof<u32>();
    rawptr<u8> raw = match (allocate(bytes, alignof<u32>()))   // safe: failure is a value
    {
        Ok(p)  : p,
        Err(_) : { return Vec::new(); },
    };
    rawptr<u32> words = reinterpret_ptr<u32>(raw);
    Vec<u32> out = Vec::new();
    unsafe
    {
        foreach (i in 0..n)
        {
            *(words + narrow<isize>(i)) = 0;     // asserted: inside the allocation
            Vec::push(&mut out, *(words + narrow<isize>(i)));
        }
        deallocate(raw, bytes, alignof<u32>());   // asserted: the allocation above, once
    }
    out
}

fn main()
{
    Vec<u32> v = zeros(4);
    printf("%v zeros, the last is %v\n", Vec::len(&v), v[3]);
}
```

```
4 zeros, the last is 0
```

This is how `Vec` itself is written: a `resource struct` holding a raw
pointer, a length and a capacity, with a destructor that deallocates and a
`push` that writes through the pointer inside `unsafe`. The standard library
is written in CobaltC, and its `unsafe` blocks are the implementation's to
keep true, not yours: a program that writes no `unsafe` of its own has the
language's guarantee without conditions.

Between raw memory and the checked world there is one more operation.
`reclaim<T>(p)` turns the bytes at `p` into an object of type `T` that every
ordinary rule then applies to; you can borrow it, assign to it, or `drop`
it, and two references obtained this way are checked against each other
like any others. The library uses it to hand out references to a vector's
elements. Most programs never need it.

## Calling C

A function provided from outside is declared with `unsafe extern fn`, and
called inside `unsafe`:

```
import std;

unsafe extern fn sqrt(f64 x) : f64;          // from the C maths library
unsafe extern fn toupper(i32 c) : i32;       // from the C library

fn main()
{
    f64 root = unsafe { sqrt(2.0) };
    assert(root > 1.414 && root < 1.415);     // a claim from outside, checked before it is trusted
    printf("%.6f\n", root);
    printf("%v\n", unsafe { toupper(widen<i32>(b'c')) });   // 'c' -> 'C', as a number
}
```

```
1.414214
67
```

The declaration is marked `unsafe` because no tool can check that the C
function really has that name and those types; if it does not, every call is
wrong, so the assertion is made where the declaration is. The call is inside
`unsafe` for what the function does. What comes back is a *claim*: a genuine
value of the declared type whose relationship to anything else, that a
length is right, that a pointer is valid, is not established until your code
checks it. The `assert` above is that check in miniature, and it is the
pattern for everything that crosses the boundary: bytes from a file are a
`Vec<u8>` until `String::from_utf8` validates them, and a count from C is a
number until the program compares it with what it asked for.

The types that may cross are integers, floats, `bool`, `void`, raw pointers,
and plain structs of those, laid out as C lays them out. Anything else is
rejected at the declaration:

```
import std;

unsafe extern fn takes_vec(Vec<i32> v);      // ✗ a Vec cannot cross into C

fn main()
{
}
```

```
error[diag.extern-non-ffi-type]: `takes_vec` cannot cross to C: its signature has `Vec<i32>`, a resource; an extern's types are integers, `f32`, `f64`, `bool`, `rawptr<T>` and plain structs of those (D-0252)
 --> bad_extern.cb:3:1
  |
3 | unsafe extern fn takes_vec(Vec<i32> v);      // ✗ a Vec cannot cross into C
  | ^^^^^^
  = repair: pass `rawptr`/scalars, or a struct without resources, enums or bitstructs
```

A program's own C code is named with `extern "./shapes.c";`, which the
compiler compiles and links, and an installed library with `extern "z";`.
The whole program still builds with `cobc main.cb`. The interpreter's memory
is simulated, so it calls C functions that take and return numbers and
refuses, with a clear message, those that need a pointer; the compiler
passes everything. The language guide shows a complete C file, a CobaltC
library that C programs call, and the layout rules for structs that cross.

## When an assertion is false

An `unsafe` block's assertions are not checked; that is what makes them
assertions. If one is false, because `*p` read bytes that were not a valid
`T`, or a C function wrote past what it was given, the program has left the
language's rules at that step. Everything before it happened exactly as the
rules say. From that step on, nothing is promised: the program may carry on,
print nonsense, fault or crash, and no guarantee in this book covers what
follows.

Two consequences shape how `unsafe` is used. Keep each block small, so that
what it asserts can be read and checked by eye in the place it is written.
And keep the number of blocks small, by putting the trusted operations
behind a safe function, as `zeros` and `read_through` above do, so that the
rest of the program, which is most of it, makes no assertions at all and has
the language's guarantee in full.
