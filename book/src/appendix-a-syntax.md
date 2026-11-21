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
fn f(ref<T, shared> a, ref<T, shared> b) : ref<T, shared> from a, b
                                            // a reference result: `from` names the
                                            // parameters it may come from
unsafe extern fn name(rawptr<u8> p) : i32;  // a C function
extern "./file.c";                          // C code to link

struct S { T1 a; export T2 b; }             // fields are private unless exported
resource struct S { ... }                   // a type with a destructor, moved not copied
enum E { A(T), B, C }                       // variants, with or without a payload
enum E { A(T), B { T1 x; T2 y; } }          // a variant with named fields
enum E : u8 { A = 1, B }                    // with numeric codes
bitstruct F : u8 { x : 1; pad : 7; }        // named bits of one integer
const T NAME = e;                           // a value known when the program is compiled
unittest "name" { ... }                     // a test; cobc --unittest --run runs it
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
foreach (x in &c) { ... }                         // also &mut c, c (consumes), lo..hi, lo..=hi
foreach (i, x in &c) { ... }                      // with the position
break;    continue;    return e;    return;
match (e) { A(x) : e1, B : e2, _ : e3, }          // every arm ends with a comma
match (e) { E::B { x, .. } : …, E::A(*p) : …, }   // a variant's fields; `*p` binds through a Box
match (o) { Some(S { a, b: c }) : …, None : …, }  // a struct pattern; `b: c` renames
unsafe { ... }
drop(x);                                          // destroy early
_ = e;                                            // discard a value on purpose
```

## Expressions

```
42   7: u64   2.5   b'a'   "text"   b"bytes"   true   ()
S { .a = 1, .b = 2 }      [1, 2, 3]      [0; 256]      Some(5)      E::A(1)
E::B { .x = 1, .y = 2 }                           // a variant with fields, always with its enum
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

## Threads

```
handle<R> h = spawn(f, a, b);     // runs f(a, b) on a new thread; a, b moved or lent
R r = join(h);                    // waits for it and takes its result
mutex<T> m = Mutex::new(v);       // shared through &m
guard<T> g = lock(&m);            // g.f, *g; the lock ends with g
```

## Running

```
cobc --run prog.cb [args]      compile and run, keeping nothing
cobc -o prog prog.cb           compile to the executable prog
cobc --check prog.cb           only the checks made before a run; every independent error
cobc --explain --run prog.cb   add the registry's entry to a diagnostic (or --explain --check)
cobc --doc Vec::push           an item of the standard library; --doc Vec, --doc std::text
cobc --doc widen               one of the language's own functions; --doc language lists them
cobc --doc --find stdin        the items whose name, signature or summary has every word
cobc --unittest --run prog.cb  run the tests
cobc --explore 50 prog.cb      run a threaded program under 50 schedules
cobc --schedule-seed S prog.cb run once under the schedule --explore named S
cobfmt prog.cb                 format in place
```

## What is not written

A few things the language does are not on the line that does them. These are
all of them; each is one rule, the same everywhere.

| Rule | Example | Where it is |
|---|---|---|
| A field or an index reads through a reference or a guard | `r.f`, `r[i]`, `g.f` with `r` a reference, `g` a guard | `spec/16`, `[Field-Access-Auto-Deref]`, `[Guard-Auto-Deref]` |
| Text fits where it is declared | `String s = "x";`, `&s` where a `StringView` is declared | `spec/12`, `[Coerce-Text]` |
| A whole `Vec` or array fits where a slice is declared | `slice_len(&v)` | `spec/12`, `[Coerce-Slice]` |
| A literal takes its type from where it goes; `auto` from the value | `u64 n = 5;`, `f(5)`, `auto x = f();` | `spec/12`, `rule.type.expected` |
| What a pattern binds through a reference is a reference | `match (&o) { Some(x) : *x, … }`; `E::B { x, .. }` too | `spec/16`, `[Match-By-Ref]`, `[Struct-Pattern-By-Ref]` |
| A plain value is copied, a resource moved | `i32 b = a;` copies, `String t = s;` moves | `spec/12`, `rule.type.is-resource` |
| A reference ends after its last use | `auto r = &mut v; … r …; Vec::push(&mut v, 1);` | `spec/10`, D-0111 |
| A value is destroyed at the end of its scope, last declared first | a `String` local freed at its block's `}` | `spec/07`, `rule.resauth.destroy` |
| A closure borrows what it captures, exclusively when its body writes it; `move` copies or moves | `[n]() { n += 1; }` | `spec/15`, closures |

