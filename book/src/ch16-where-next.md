# Where to go from here

You now know the language. What remains is practice, and knowing where to
look things up. The CobaltC repository has several kinds of documentation,
each for a different need, and this chapter says which is which.

## The language guide

`htmlguide/index.html` is the reference. It has one section per chapter of
the specification, numbered the same way, each with a complete account of
the rules and a worked example of every one, and a listing of every module
and item of the standard library with its signature and a line about what
it does. When this book says a library has a function and does not show
it, the guide shows it. When you want to know exactly what a rule says, not
just what it means, the guide says so and names the rule. Its examples are
tested in the same way as this book's.

The guide also has three chapters this book does not attempt: one on
performance, measured and explained, for a reader who wants to know what a
check costs and when the compiler can leave it out; one for C programmers,
on error handling at length and on reading the C that `cobc` generates; and
one for Rust programmers, on where the two languages agree and where they
part.

## The specification

`spec/` is the authority. It is written as rules over an abstract machine,
in a notation its first chapter defines, and it is what both tools
implement. A rule is terse and complete; the prose around it explains. For
most questions the guide is enough, but when two readings of a sentence in
the guide seem possible, the specification settles it, and `coby --explain`
prints the rule behind any diagnostic. `spec/OVERVIEW.md` is a prose
account of the whole model, the state, how references are formed and end,
the one conflict check, what is decided before a run and what during it,
for a reader who wants the machinery rather than the rules one at a time.

The specification's decision records, under `spec/decisions/`, say *why*.
Every feature that was added, declined or changed has one, with the problem,
the options considered and the reasons. When something in the language
seems odd, there is usually a decision record that explains what the
alternative would have cost.

## The showcases

`showcase/` holds dozens of small, complete programs and one library, each
chosen to show something particular to CobaltC: a bytecode interpreter
whose every failure is a value, a tree in an arena with no pointers, a
parallel sort, a TLS client, a tiny shell. Its README describes them all.
They are tested under both tools, so each one is current, and they are the
best place to see how the language is used at a hundred to five hundred
lines.

## The tools

`coby prog.cb` runs; `cobc -o prog prog.cb` builds; `cobfmt prog.cb`
formats. Beyond those: `--check` runs only the checks made before a run,
`--explain` adds the registry's entry to a diagnostic, `--unittest` runs the
tests, `--explore N` runs a threaded program under many schedules, and
`--warn-shadow` notes names of your program that hide imported ones. For the
compiler, `--keep-c` leaves the generated C beside the executable, `-v`
shows the exact C compiler command, and `-g -O0` makes a build you can step
through in a debugger at the level of your own source lines. Each tool's
`--help` lists the rest.

## A way to proceed

Write something. A program of a few hundred lines that does a job you
actually want done teaches more than any chapter, and the compiler will tell
you, precisely, each time you reach for a habit from another language that
does not fit here. The habits that matter are few: decide what each
function keeps and what it only looks at; keep borrows short and visible;
put failures in return types and leave bugs to the checks; use an enum
wherever a value is one of several things. When a diagnostic surprises you,
read it to the end, then `--explain` it, and the reason is usually on the
page.
