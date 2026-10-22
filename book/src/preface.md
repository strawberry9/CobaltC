# Preface

This book teaches CobaltC to someone who has not written it before. It
assumes you have programmed in some language, so that a loop, a function and a
type are familiar ideas, and it assumes nothing else. If you know C, many
things will look familiar and a few will surprise you; if you know Rust, the
ideas will be familiar and the spelling will not. Either way, the book is
written to be read in order, each chapter resting on the ones before it.

The first part gets a program running and then builds a small, real one, so
that you have seen the whole language once before studying any part of it.
The second part takes the language apart, one idea per chapter: values,
functions, ownership, borrowing, data types, errors, collections, generics,
modules, threads and the boundary with C. The third part builds a larger
program and says where to go next. The appendices are for looking things up.

Every complete program in the book has been run. Each one is kept as a file
beside the book, with the output it prints, and the project's tests run all
of them under the interpreter and under the compiler every time the language
changes. So when the book says a program prints something, or is rejected
with a particular message, that is what the tools do today, not what they did
when the sentence was written.

A few conventions. Code that is a complete program is shown whole and is
followed by what it prints. Code that is only a fragment is marked as such,
and you cannot run it on its own. When a program is meant to be rejected, the
line at fault carries a comment beginning with a cross, and the message the
tools print is shown below the program. All the code uses one brace style,
with every opening brace on a line of its own; the language does not care,
and the formatter that comes with it, `cobfmt`, writes this style for you.

The book is part of the CobaltC repository and changes with the language.
Where it is silent, the language guide and the specification, both in the
same repository, have the answer; the last chapter says how to use them.
