# The CobaltC Programming Language

A book for learning CobaltC from the beginning: what the language is for, how
to write programs in it, and why its rules are the way they are. It is written
for a reader who has programmed before, in any language, and takes nothing
else for granted.

Read it at **https://strawberry9.github.io/CobaltC/**, where it is rendered by
[mdBook](https://rust-lang.github.io/mdBook/) on every push (the workflow is
`.github/workflows/book.yml`). Or start with [`src/SUMMARY.md`](src/SUMMARY.md),
the table of contents, and read the chapters as plain Markdown; `mdbook build`
in this directory renders them locally, and `mdbook serve` shows the result in
a browser.

## The examples are tested

Every complete program in the book lives twice: in the chapter that explains
it, and as a file under `examples/`, beside the output it is expected to
print. `./check.sh` runs each one under the interpreter `coby` and under the
compiler `cobc` with GCC and with Clang, compares the three outputs with the
expectation, confirms that the file and the chapter show the same text, and
checks that every example is formatted as `cobfmt` would format it. The
project's test battery runs this check, so a change to the language that would
make the book wrong fails the build until the book is corrected.

To add or change an example: edit the program in the chapter and in its file
under `examples/chNN/`, then run `./check.sh --regenerate` to rewrite its
expected output (and read the diff), then `./check.sh`.

## Where the rest of the documentation is

The book teaches. For looking things up there is the
[language guide](../htmlguide/index.html), one section per chapter of the
specification with a complete listing of the standard library; the
[specification](../spec/README.md) itself, which is the authority on every
question; and the [showcase programs](../showcase/README.md), small complete
programs each chosen to show something particular to CobaltC. The
book's last chapter says which to read next.

## Licence

Copyright © 2026 strawberry9.

The book's text, the contents of `src/`, is licensed under the Creative Commons
Attribution-NonCommercial-NoDerivatives 4.0 International License
(CC BY-NC-ND 4.0): you may share it for non-commercial purposes with
attribution, and you may not distribute modified versions. See
`LICENSE-CC-BY-NC-ND`, or https://creativecommons.org/licenses/by-nc-nd/4.0/.

The example programs, the contents of `examples/`, and the scripts `check.sh`
and `embedded.py` are source code, licensed under the BSD 3-Clause License.
See `LICENSE`.

These are the same terms as the CobaltC repository's, for its documentation and
its code respectively.
