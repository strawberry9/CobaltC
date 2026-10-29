# Appendix C: For readers who know C or Rust

The language guide has a full chapter for each. This appendix is the short
version: the habits to keep and the ones to drop.

## Coming from C

Keep: type-first declarations, `if`/`while`/`for` with parentheses and
braces, `printf` with its format strings, structs with designated
initialisers, `//` and `/* */` comments, and the feel of knowing where your
memory is.

Drop these habits, because the compiler will refuse them:

- **Implicit conversion.** `int` and `long` do not mix here, and neither do
  `i32` and `i64`. Every conversion is a named call: `widen`, `narrow`,
  `to_float`, `to_int`. There is no `int`; every integer names its width.
- **Wrapping arithmetic.** `a + b` that does not fit is a fault, not a wrap.
  Ask for wrapping by name when a hash function wants it.
- **Null.** There is no null pointer in safe code. A value that may be
  absent is an `Option<T>`, and the compiler makes you check it.
- **Pointers for everything.** A pointer that only lets a callee look is
  `ref<T, shared>`, one that lets it change is `ref<T, exclusive>`, and a
  pointer that hands over ownership is just passing the value. A
  `rawptr<T>` exists, inside `unsafe`, for the few places that need an
  address and nothing else.
- **`free`.** Nothing is freed by hand. A value that owns memory is a
  resource, and its owner's scope end frees it. There is no leak and no
  double free because there is no way to write either.
- **Error codes.** A function that can fail returns `Result<T, E>`, and the
  caller cannot forget to check it; a dropped `Result` is a compile error.
- **`switch`.** A `match` on an integer or text does the job, with a `_`
  arm, and a `match` on an enum must cover every variant.
- **Uninitialised variables.** A variable must be assigned on every path
  before it is read; the check is static.
- **`x & 1 == 0`.** In CobaltC this means `(x & 1) == 0`, as you meant.

What you gain is the compiler keeping the facts you used to keep in your
head. And `cobc --keep-c` shows you the C your program became, with every
check as an explicit line.

## Coming from Rust

Keep: ownership, moves, borrowing with two modes, enums with payloads,
`match`, `Option` and `Result` with `?`, generics with bounds, closures with
captures, `unsafe` as the marked boundary, threads whose data is moved or
lent, and the overall conviction that the compiler should prove things.

The differences are mostly about how far the checking is pushed to compile
time, and about spelling:

- **Borrow checking is static where it can be and dynamic where it cannot.**
  Rust rejects every program it cannot prove safe. CobaltC rejects every
  program whose text shows a conflict, and checks the rest while it runs,
  faulting at the first conflicting access. The accepted set is fixed by
  the language, not by the cleverness of the checker, and there are no
  lifetime annotations: a function returning a reference must have exactly
  one reference parameter.
- **No traits.** There are six built-in bounds and no way to declare
  another. Generic code that needs an operation on your type takes it as a
  function parameter.
- **No method-call syntax.** `Vec::push(&mut v, x)`, never `v.push(x)`.
  The mode of every borrow is written where the borrow is taken.
- **No `mut` on bindings.** Every local can be assigned. What is controlled
  is who can reach a value, not whether it may change.
- **No iterators.** `foreach` walks the collections the language knows, and
  library functions such as `position`, `retain` and `sort_by` take
  function values. There is no iterator protocol to implement.
- **Three text types, spelled differently.** `str` is a copyable text
  constant, not a borrowed slice; `String` is owned text; `StringView` is
  Rust's `&str`, a borrowed view of a `String`.
- **Declarations are type-first.** `i32 x = 5;` and `auto y = x;`. Blocks,
  `if` and `match` are expressions, as in Rust, but conditions take
  parentheses and `match` arms are `pattern : expression,`.
- **Overflow always checks.** In Rust it depends on the build profile. Here
  it is a fault in every build, and the wrapping and checked families are
  the only way to get another behaviour.
- **A closure's capture list is written and checked.** `[a, b](i32 x) { ... }`
  must name exactly the outer variables the body uses; how each is captured
  is decided by the body, and `move` captures by value.
- **A fault in a thread is contained.** It ends that thread, poisons its
  mutexes, and arrives at `try_join` as a value; a plain `join` re-raises
  it. Much like a panic, with the policy fixed by the language rather than
  the library.
- **Interior mutability is a mutex or an atomic, and nothing else.** There
  is no `Cell` or `RefCell`; `Rc<T>` gives shared read access and a mutex
  inside it gives mutation.

Rust programmers tend to find CobaltC easy to read and briefly annoying to
write, until the qualified calls and the written borrows become
automatic. The guide's Rust chapter goes through each of these at length,
with the reasons.
