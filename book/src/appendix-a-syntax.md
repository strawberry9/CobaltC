# Appendix A: The language on one page

A reference card for the syntax the book has used. The language guide's
syntax chapter has the complete grammar and its tie-break rules.

## Declarations

```
import std;                                 // the standard library's names
import m::item;                             // one name, unqualified
module m { items }                          // a module; items are private unless exported
module m "path.cb";                         // the same, with a file's items as the body
export import m::sub;                       // pass on what sub exports as m's own

fn name(T1 a, T2 b) : R { body }            // a function; the last expression is the result
fn name(T1 a) { body }                      // returns nothing
fn name<T: bound>(T x) : T { body }         // generic; bounds: eq ordered number integer clone plain
fn Type::name(ref<Type, shared> self) : R   // associated: called as Type::name(&v)
fn Type::drop(ref<Type, exclusive> self)    // the destructor; the language calls it
unsafe extern fn name(rawptr<u8> p) : i32;  // a C function
extern "./file.c";                          // C code to link

struct S { T1 a; export T2 b; }             // fields are private unless exported
resource struct S { ... }                   // a type with a destructor, moved not copied
enum E { A(T), B, C }                       // variants, with or without a payload
enum E : u8 { A = 1, B }                    // with numeric codes
bitstruct F : u8 { x : 1; pad : 7; }        // named bits of one integer
const T NAME = e;                           // a value known when the program is compiled
unittest "name" { ... }                     // a test; coby --unittest runs it
```

## Types

| Written | Meaning |
|---|---|
| `i8` … `i128`, `u8` … `u128`, `isize`, `usize` | integers; `usize` for lengths and indices |
| `f32`, `f64`, `bool`, `str`, `void` | floats, truth, text constants, nothing |
| `ref<T, shared>`, `ref<T, exclusive>` | references, made with `&x` and `&mut x` |
| `array<T, N>` | `N` elements, `N` fixed when compiled |
| `slice<T, shared>`, `slice<T, exclusive>` | a borrowed run, `&a[i..j]`, `&mut a[i..j]` |
| `Vec<T>`, `String`, `StringView`, `HashMap<K, V>`, `HashSet<K>` | the library's collections and text |
| `Option<T>`, `Result<T, E>` | `Some(v)` / `None`; `Ok(v)` / `Err(e)` |
| `fn(T1, T2) : R` | a function or closure value |
| `handle<T>`, `mutex<T>`, `guard<T>`, `Channel<T>`, `Atomic<T>` | threads and what they share |
| `rawptr<T>` | an address, used only inside `unsafe` |

## Statements

```
T x = e;        auto x = e;        T x;           // declare; `auto` takes e's type
x = e;          x += e;                           // assign; also -= *= /= %= &= |= ^= <<= >>=
S { a, b } = e;                                   // take a struct apart into locals
if (c) { ... } else if (c2) { ... } else { ... }
if (Some(x) = e) { ... }                          // a pattern as a condition; also while
while (c) { ... }       for (init; c; step) { ... }
foreach (x in &c) { ... }                         // also &mut c, c (consumes), and lo..hi
foreach (i, x in &c) { ... }                      // with the position
break;    continue;    return e;    return;
match (e) { A(x) : e1, B : e2, _ : e3, }          // every arm ends with a comma
unsafe { ... }
drop(x);                                          // destroy early
_ = e;                                            // discard a value on purpose
```

## Expressions

```
42   7: u64   2.5   b'a'   "text"   b"bytes"   true   ()
S { .a = 1, .b = 2 }      [1, 2, 3]      [0; 256]      Some(5)      E::A(1)
x.f      a[i]      a[$ - 1]      f(x)      Type::name(&v)      name<T>(x)
&x      &mut x      *r
e?                                                // Ok(v) gives v; Err(e) returns it
if (c) { 1 } else { 2 }      { stmts; value }      match (e) { ... }
[a, b](i32 x) { a + b + x }      move [v]() { Vec::len(&v) }
```

## Operators, tightest first

| Operators | Notes |
|---|---|
| `e.f` `e[i]` `e(args)` `e?` | postfix |
| `-e` `!e` `~e` `&e` `&mut e` `*e` | unary |
| `*` `/` `%` | checked; division by zero faults |
| `+` `-` | checked |
| `<<` `>>` | the amount is a `u32` below the width |
| `&` | bitwise and |
| `^` | bitwise xor |
| `|` | bitwise or |
| `==` `!=` `<` `<=` `>` `>=` | not chainable; bind looser than `&` `|` `^` |
| `&&` | short-circuit |
| `||` | short-circuit |
| `=` `+=` … | right-associative; the left side is a place |

## Conversions and arithmetic policies

```
widen<U>(x)          never fails; only to a type that holds every value
narrow<U>(x)         checked; faults when x does not fit
narrow_wrapping<U>(x)   keeps the low bits
checked_narrow<U>(x)    Option<U>
to_float<U>(x)       to_int<U>(x)       reinterpret<U>(x)
wrapping_add  wrapping_sub  wrapping_mul
saturating_add  saturating_sub  saturating_mul
checked_add  checked_sub  checked_mul  checked_div  checked_rem  checked_neg   -> Option
min_value<T>()  max_value<T>()  sizeof<T>()  alignof<T>()
```

## Running

```
coby prog.cb [args]            run with the interpreter
cobc -o prog prog.cb           compile; cobc --run prog.cb compiles and runs
coby --check prog.cb           only the checks made before a run
coby --explain prog.cb         add the registry's entry to a diagnostic
coby --unittest prog.cb        run the tests
coby --explore 50 prog.cb      run a threaded program under 50 schedules
cobfmt prog.cb                 format in place
```
