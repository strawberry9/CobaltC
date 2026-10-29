# Introduction

CobaltC is a small systems programming language. It gives you what C gives
you: control over memory, predictable layout, threads, direct calls into C
libraries, and programs that compile to native code and run fast. It takes
away one thing, the obligation to remember.

When you write C, your program depends on a long list of facts that appear
nowhere in its text. This pointer is still valid. That integer cannot
overflow here. This buffer was initialised before it was read. Nobody else is
writing to that struct right now. This file handle is closed exactly once. The
compiler checks none of them. You keep them in your head, and when one slips,
the program is wrong in a way that may not show for months.

CobaltC is built around one idea: **the facts that correct low-level code
depends on should live in the language, not in the programmer's memory.**
Every fact of that kind is either represented and checked by the language, or
written down by you, in one place, as something you are asserting. There is no
third option where a fact is simply assumed.

## What that means in practice

Arithmetic is checked. If `a + b` does not fit its type, the program stops
with a named fault instead of wrapping silently. When you want wrapping, you
ask for it by name.

Every value that owns something, a buffer, a file, a thread, has exactly one
owner. Giving it to someone else is a move, and the old name is no longer
usable. When the owner goes out of scope, the thing it owned is released, in a
fixed order, whether or not you remembered to release it.

References come in two kinds, shared and exclusive, and the language checks
at every use that no exclusive reference is live at the same time as any other
reference to the same storage. Most of these checks happen before the program
runs. The ones that cannot be decided from the text happen while it runs, and
a failure there stops the program with a diagnostic rather than reading freed
memory.

A variable is never read before it is written. A reference never outlives the
thing it points to. Two threads never race on one location. A value that came
from outside the program, from a file or a socket, is bytes until you have
validated it, and the language keeps the distinction.

Where the language cannot check a fact, because the fact is about memory it
did not allocate or code it did not compile, you assert it inside an `unsafe`
block. That block is a sentence in your program, visible to every reader, and
a search for the word `unsafe` finds every assumption the program makes.

## What CobaltC leaves out

A language earns its size by what it refuses. CobaltC has no inheritance, no
traits or interfaces of your own, no exceptions, no garbage collector, no
implicit conversions, no operator overloading, no macros, and no method-call
syntax: you write `Vec::push(&mut v, x)`, never `v.push(x)`. Each absence is a
decision, and the ones that matter to a programmer are explained where they
come up. The result is a language you can hold in your head after a few days.

## Two implementations

CobaltC comes with two tools that share one front end, so they accept and
reject exactly the same programs with exactly the same messages.

`coby` is the interpreter. It runs a program straight from its source, doing
every check the language requires on every operation. There is no build step,
so it is the quickest way to run a program or try something out, and when the
book says a program prints something, `coby` is what printed it.

`cobc` is the compiler. It translates a checked program to C and hands that to
GCC or Clang, which produces a native executable. The compiled program keeps
every run-time check the language requires except the ones the compiler
proves can never fail, so it behaves exactly like the interpreted one and runs
many times faster. The project's tests hold the two to that promise on every
program in this book.

## How the book is arranged

Part I is practical. The next chapter installs the tools and runs a first
program. The chapter after it builds a complete small program, a word
counter, and uses most of the language once, lightly, so that the detailed
chapters have something to refer back to.

Part II explains the language one idea at a time. Chapters 4 and 5 cover
values and control flow, which will feel familiar. Chapters 6 and 7, ownership
and borrowing, are the heart of the language and the part most worth reading
slowly. The remaining chapters cover data types, errors, collections and text,
generics and closures, modules, threads, and the boundary with C.

Part III builds a larger program, a command-line to-do list that keeps its
data in a file, and then points you at the rest of the documentation.

You can read the whole book in a few evenings. You do not have to: a reader
in a hurry can read Part I, then chapters 6 and 7, and look the rest up as
they need it.
