# Hello, CobaltC

This chapter gets the tools onto your machine, runs a first program, and
shows what the tools say when a program is wrong. By the end you will know
the whole cycle: write, run, read the message, fix.

## Getting the tools

CobaltC's two tools are built from the repository with Cargo, Rust's build
tool. With Rust installed from [rustup.rs](https://rustup.rs), and GCC or
Clang installed for the compiler to use, this builds everything:

```
cd impl
cargo build --release --workspace
```

The executables land in `impl/target/release/`: `coby`, the interpreter;
`cobc`, the compiler; and `cobfmt`, the formatter. Put that directory on your
path, or call them by their full path. The repository's `README.md` has the
details, including how to build on Windows and how to compile for it.

## The first program

Save this as `hello.cb`:

```
import std;

fn main()
{
    printf("hello, world\n");
}
```

Run it with the interpreter:

```
$ coby hello.cb
hello, world
```

Or compile it and run the executable:

```
$ cobc -o hello hello.cb
$ ./hello
hello, world
```

`cobc --run hello.cb` does both at once and keeps nothing. For trying things
out, `coby` is quicker; for a program you will run many times, `cobc` makes
it fast. They agree on everything, so use whichever is convenient.

Three lines of this program deserve a word each.

`import std;` brings the standard library into scope. `printf` lives there,
as do `Vec`, `String` and almost everything else a program uses. The language
itself is small, and a program that does not import `std` cannot even print.
You will write this line at the top of every program.

`fn main()` declares the function the program starts in. A function is
declared with `fn`, its name, its parameters in parentheses, and its body in
braces. This one takes no parameters and returns nothing.

`printf("hello, world\n");` writes text to standard output. The function is
C's, and its format string works the same way, with one improvement that you
will meet shortly. The `\n` ends the line; there is no separate function that
adds a newline for you.

## Values in the output

Here is a program with some values in it:

```
import std;

fn main()
{
    str name = "Ada";
    i32 year = 1815;
    i32 age = 2026 - year;
    printf("%v was born in %v, %v years ago.\n", name, year, age);
}
```

```
Ada was born in 1815, 211 years ago.
```

Declarations put the type first, as C does: `i32 year` declares `year` as a
32-bit signed integer, and `str name` declares `name` as text. There are no
implicit conversions anywhere in the language, so types are worth getting
right from the start; chapter 4 lists them.

`%v` in a format string prints whatever value it is given, in the way CobaltC
writes that kind of value. It works for numbers, text and truth values, and
you will use it more than any of C's specifiers. The format string is checked
when the program is compiled: a `%d` given text, or a `%v` with no value to
print, is an error before the program runs, so a `printf` can never crash or
print garbage.

## When a program is wrong

Most of what you learn about CobaltC, you will learn from its messages, so it
is worth reading one carefully now. This program has a spelling mistake:

```
import std;

fn main()
{
    i32 year = 1815;
    printf("%v\n", yaer);             // ✗ a name that was never declared
}
```

Both tools refuse it with the same message:

```
error[diag.unbound-name]: nothing named `yaer` is declared here
 --> mistake.cb:6:20
  |
6 |     printf("%v\n", yaer);             // ✗ a name that was never declared
  |                    ^^^^
  = repair: declare it, fix the spelling, or add an `import` (`import std;` for the standard library, `CHG-0033`)
```

The first line says what is wrong. The word `error` means the program was
rejected before anything ran. In the brackets is the diagnostic's name,
`diag.unbound-name`. Every diagnostic has a name like this, the same in every
version of the tools, and the specification keeps a registry that describes
each one. The arrow line gives the file, the line and the column, in the form
editors turn into a link; the source line follows with carets under the
offending token; and the `repair` line is the registry's standard advice.

Not everything can be caught before a program runs. Here is a program whose
mistake depends on a value:

```
import std;

fn twice(i32 x) : i32
{
    x + x
}

fn main()
{
    printf("%v\n", twice(21));
    printf("%v\n", twice(2000000000));    // ✗ 4000000000 does not fit an i32
}
```

```
42
fault[diag.arith-overflow]: arithmetic overflow
 --> overflow.cb:5:5
  |
5 |     x + x
  |     ^
  = repair: widen, or use `wrapping_`/`saturating_`/`checked_`
```

The first call works and prints 42. The second asks for a sum that does not
fit in a 32-bit integer. In C the result would wrap around to a negative
number and the program would carry on. In CobaltC the program stops at that
line with a *fault*. The word `fault` instead of `error` tells you the
program was accepted and ran until a check failed while it was running. The
rest of the message has the same shape as before.

These two kinds of message, an error before the run and a fault during it,
are the only two ways a CobaltC program can be wrong at the language level.
The book will often say that a mistake is caught "statically", meaning before
the run, or "dynamically", meaning during it. The rule is always the same;
only the moment it is checked differs, and that moment is fixed by the
language, not by how clever the tool is.

Two options help when you are puzzled. `coby --explain mistake.cb` prints the
registry's whole entry for the diagnostic, with the rule of the language that
rejected the program and examples of it. And `coby --check prog.cb` runs the
checks that happen before a run and nothing else, so you can check a program
that would otherwise read input or write files.

## Keeping it tidy

`cobfmt hello.cb` rewrites a file in the style this book uses: four-space
indentation, every brace on its own line, spaces where they belong. It
changes layout and nothing else, and it refuses to write a file whose
tokens would change. The language itself ignores whitespace between tokens,
so any layout is correct; a formatter just saves arguments.

## What you have

You can now write a program, run it two ways, and read what the tools say
about it. The next chapter writes a program that does some real work.
