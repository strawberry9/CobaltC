# A first program: counting words

Before studying the language piece by piece, it helps to see a whole program
that does something useful. This one reads text from standard input and
prints the words that occur most often. It is sixty lines long and uses a
struct, a hash map, a vector, text handling, error handling, references and
a sorting function. You are not expected to understand every line yet. Read
it for its shape, then read the explanation, and come back to it from later
chapters when the details have names.

## The program

```
import std;

// One row of the final table: a word and how often it appeared.
struct Count
{
    String word;
    u32 n;
}

// The order of the table: most frequent first, then alphabetical.
fn more_frequent(ref<Count, shared> a, ref<Count, shared> b) : bool
{
    a.n > b.n || (a.n == b.n && a.word < b.word)
}

// Reads every line of standard input and counts its words.
fn count_words(ref<HashMap<String, u32>, exclusive> counts) : Result<usize, FileError>
{
    usize lines = 0;
    while (Some(line) = read_line()?)
    {
        lines += 1;
        foreach (word in String::split_whitespace(&line))
        {
            String key = String::to_ascii_lower(&String::from_view(word));
            *HashMap::entry(counts, key, 0) += 1;
        }
    }
    Ok(lines)
}

fn main() : u8
{
    HashMap<String, u32> counts = HashMap::new();
    usize lines = match (count_words(&mut counts))
    {
        Ok(n)  : n,
        Err(e) :
        {
            eprintf("wordcount: %s\n", FileError::text(&e));
            return 1;
        },
    };

    Vec<Count> table = Vec::new();
    foreach (word, n in &counts)
    {
        Vec::push(&mut table, Count { .word = String::clone(word), .n = *n });
    }
    Vec::sort_by(&mut table, more_frequent);

    printf("%v lines, %v distinct words\n", lines, Vec::len(&table));
    foreach (i, c in &table)
    {
        if (i == 5)
        {
            break;
        }
        printf("%5d %s\n", c.n, &c.word);
    }
    0
}
```

Given this input:

```
The quick brown fox jumps over the lazy dog.
The dog sleeps; the fox does not.
A fox is quick and a dog is lazy, or so the story goes.
```

it prints:

```
3 lines, 20 distinct words
    5 the
    3 fox
    2 a
    2 dog
    2 is
```

Run it yourself with `coby wordcount.cb < some-text-file`, or compile it and
pipe anything you like into the executable.

## A struct and a function of two references

The program begins by declaring a type for one row of its output:

```
struct Count
{
    String word;
    u32 n;
}
```

A struct is a group of named fields, each with a type. `String` is owned text,
the kind you build at run time; `u32` is an unsigned 32-bit integer. Nothing
about this struct is special, except that because it contains a `String`, it
*owns* something, and the language will treat it accordingly: a `Count` is
moved from place to place, never silently copied. Chapter 6 is about that.

Then a function that says which of two rows comes first:

```
fn more_frequent(ref<Count, shared> a, ref<Count, shared> b) : bool
{
    a.n > b.n || (a.n == b.n && a.word < b.word)
}
```

The return type comes after a colon. The function's body is a single
expression with no `return` and no semicolon: in CobaltC, the last expression
of a block is the block's value, and a function's body is a block. You will
write `return` only to leave early.

The parameters are *references*. `ref<Count, shared>` means "a way to reach a
`Count` that someone else owns, for reading only". The function could have
taken two `Count` values instead, but then each call would have had to hand
over its rows, and the caller wants to keep them. Taking a shared reference
says, in the signature, that the function looks and does not touch. Field
access works through a reference without any extra syntax: `a.n` is the `n`
of the row `a` refers to.

Comparing two `String`s with `<` compares their text, byte by byte.

## Reading input, and the first error

The next function does the counting:

```
fn count_words(ref<HashMap<String, u32>, exclusive> counts) : Result<usize, FileError>
{
    usize lines = 0;
    while (Some(line) = read_line()?)
    {
        lines += 1;
        foreach (word in String::split_whitespace(&line))
        {
            String key = String::to_ascii_lower(&String::from_view(word));
            *HashMap::entry(counts, key, 0) += 1;
        }
    }
    Ok(lines)
}
```

This time the parameter is `ref<HashMap<String, u32>, exclusive>`: a reference
through which the map may be changed. The two kinds of reference are the
whole story of chapter 7, and the short version is that any number of shared
references may exist at once, but an exclusive one must be alone.

The return type, `Result<usize, FileError>`, says the function either
succeeds with a count of lines or fails with a file error. Reading input can
fail, and CobaltC has no exceptions, so a function that can fail says so in
its type, and its caller has to deal with it. The last line, `Ok(lines)`,
is the success case.

The loop's condition deserves a slow look. `read_line()` returns the next
line as `Ok(Some(line))`, the end of input as `Ok(None)`, or a failure as
`Err(e)`. The `?` after the call takes the `Ok` apart: if the result is an
error, `?` returns that error from `count_words` at once, and the caller gets
it; otherwise the expression's value is the `Option` inside. Then
`Some(line) = ...` is a *pattern*: when the option holds a line, the loop
body runs with `line` bound to it, and when it is `None` the loop ends.
One line expresses "for each line until the input ends, and give up on an
error", and nothing is silently ignored.

Inside, `String::split_whitespace(&line)` gives the words of the line as a
vector of *views*: pieces of the line's own text, not copies. The loop
`foreach (word in ...)` visits each one. Then two calls turn a view into a
lower-case `String` of its own, and the last line does the counting:

```
*HashMap::entry(counts, key, 0) += 1;
```

`HashMap::entry` finds the value stored under `key`, inserting `0` first if
the key is new, and returns an exclusive reference to it. The `*` reaches the
value the reference points at, and `+= 1` adds one. Library functions are
called by their qualified name, `HashMap::entry`, `String::from_view`,
always; CobaltC has no method-call syntax, so you can always see which
function runs and what it is given.

## Handling the result

In `main`, the call to `count_words` can fail, so its result is taken apart
with `match`:

```
usize lines = match (count_words(&mut counts))
{
    Ok(n)  : n,
    Err(e) :
    {
        eprintf("wordcount: %s\n", FileError::text(&e));
        return 1;
    },
};
```

`&mut counts` lends the map to the function exclusively for the duration of
the call. A `match` lists one arm per case, each `pattern : expression,`,
and must cover every case; forgetting one is an error before the program
runs. Because `match` is an expression, the whole thing is the initialiser of
`lines`: the `Ok` arm gives the count, and the `Err` arm prints a message to
standard error and returns from `main` with exit status 1. `main` is declared
`: u8` precisely so that it can return an exit status; the `0` at its end is
the success status.

## From a map to a sorted table

A hash map keeps its entries in the order they were added, which is not the
order we want to print them in, so the program copies the entries into a
vector and sorts that:

```
Vec<Count> table = Vec::new();
foreach (word, n in &counts)
{
    Vec::push(&mut table, Count { .word = String::clone(word), .n = *n });
}
Vec::sort_by(&mut table, more_frequent);
```

Looping over `&counts` borrows the map for reading and gives each key and
value as references. A struct is built with a literal that names every field,
`Count { .word = ..., .n = ... }`. The key is copied with `String::clone`
because the map still owns the original and a `String` is never copied
without being asked; `*n` reads the number through its reference.
`Vec::sort_by` takes the function written earlier and sorts the rows by it.
A function is a value you can pass like any other.

The printing loop asks for the position as well as the element, `foreach (i,
c in &table)`, so that it can stop after five rows. `%5d` is C's: the number
right-aligned in five columns. A `String` is printed through a reference,
`&c.word`, which is the language's way of saying that `printf` reads it and
does not take it.

## What you have seen

In sixty lines: a struct, two kinds of reference, a function that returns an
error as a value and a `?` that passes one up, a pattern in a loop condition,
a `match` that must be complete, a hash map, a vector, owned text and views
into it, a function passed as a value, and formatted output. Every one of
these has a chapter of its own.

One flaw is worth noticing before moving on. The input's `dog.` and `dog`
were counted as different words, because the program splits only at spaces.
Chapter 10 has the text functions to fix that, and it is a good first
exercise once you get there.
