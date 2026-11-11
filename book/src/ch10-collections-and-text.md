# Collections and text

Most programs spend their time in a few data structures: a growable list,
a run of elements borrowed from one, text, and a table keyed by something.
This chapter covers CobaltC's versions of each, which live in the standard
library, and the `printf` family that writes them out. All of it follows
the ownership and borrowing rules of chapters 6 and 7; what is new is only
what the types are and what they are called.

## `Vec<T>`: a growable list

```
import std;

fn main()
{
    Vec<i32> v = Vec::new();          // empty; the element type comes from the declaration
    Vec::push(&mut v, 30);
    Vec::push(&mut v, 10);
    Vec::push(&mut v, 20);
    printf("%v elements, first %v, last %v\n", Vec::len(&v), v[0], v[$ - 1]);

    v[1] += 5;                        // an element is a place: read it, assign to it
    Vec::sort(&mut v);
    foreach (x in &v)                 // borrow it: x is a reference to each element
    {
        printf("%v ", x);
    }
    printf("\n");

    while (Some(x) = Vec::pop(&mut v))          // take the elements out, last first
    {
        printf("popped %v\n", x);
    }
    printf("%v left\n", Vec::len(&v));
}
```

```
3 elements, first 30, last 20
15 20 30 
popped 30
popped 20
popped 15
0 left
```

A `Vec<T>` owns a buffer on the heap that grows as elements are pushed. It is
a resource: it has one owner, moves rather than copies, and frees its buffer
and destroys its elements when it goes out of scope. `Vec::new()` takes its
element type from the declaration on the left, which is why `auto v =
Vec::new();` is rejected: nothing says what `T` is.

Indexing, `v[i]`, is bounds-checked, and the index is a `usize`. Inside the
brackets, `$` stands for the length, so `v[$ - 1]` is the last element. A
function that reads the vector takes `ref<Vec<T>, shared>` and is passed
`&v`; one that changes it takes an exclusive reference and is passed `&mut
v`. Chapter 7's rules apply: a reference to an element lives as long as it
is used, and `push` while one lives is a conflict.

`Vec::pop` returns an `Option<T>`, `None` when the vector is empty, so the
loop above takes elements out until there are none. For a vector of
resources, `pop` and `remove` move the element out; nothing is ever copied.

The library has the rest of what a list needs: `insert`, `remove`,
`swap_remove`, `clear`, `truncate`, `reverse`, `contains`, `index_of`,
`position` with a test function, `sort_by` with a comparison function,
`binary_search`, `clone` for a vector of copyable elements, `append` to
join two, `filled` and `from_fn` to build one, and `reserve` to make room
in advance. The language guide lists them all with their signatures.

## Three ways to walk a collection

`foreach` holds its collection in one of three ways, written the same way a
function takes a value:

```
import std;

fn main()
{
    Vec<String> names = Vec::new();
    Vec::push(&mut names, "ada");     // a literal where a String is expected becomes one
    Vec::push(&mut names, "lin");

    foreach (n in &mut names)         // exclusive: change each element in place
    {
        String::push_ascii(n, b'!');
    }
    foreach (i, n in &names)          // shared: read each one, with its position
    {
        printf("%v: %v\n", i, n);
    }
    foreach (n in names)              // by value: each String moves out; names is consumed
    {
        printf("took %v\n", &n);
    }                                 // n is destroyed here, each time round
}
```

```
0: ada!
1: lin!
took ada!
took lin!
```

`foreach (x in &c)` is the usual form: it borrows the collection for
reading, gives each element as a shared reference, and leaves the collection
as it was. `&mut c` gives exclusive references, through which elements are
changed in place. The bare form, `foreach (x in c)`, consumes the collection,
moving each element out; afterwards `c` is stale. The loop borrows the whole
collection for its whole duration, so a body that pushes to the vector it is
walking is rejected, and there is no iterator invalidation to debug.

`printf` prints what a reference refers to, so `printf("%v", n)` works with
`n` a reference; anywhere else, `*n` is the value.

## Slices: a run of elements

A **slice** is a borrowed run of consecutive elements of an array or a
vector. It is how a function takes "some numbers" without caring whether
they came from an array of 5, a vector of 5000, or the middle of either:

```
import std;

// Works on an array, a Vec, or any part of either.
fn sum(slice<i64, shared> s) : i64
{
    i64 t = 0;
    foreach (x in s)
    {
        t += *x;
    }
    t
}

fn double_all(slice<i64, exclusive> s)
{
    foreach (x in s)
    {
        *x *= 2;
    }
}

fn main()
{
    array<i64, 5> a = [1, 2, 3, 4, 5];
    printf("%v %v %v\n", sum(&a), sum(&a[0..2]), sum(&a[$ - 2..$]));   // all, the first two, the last two

    Vec<i64> v = Vec::from_slice(&a[1..4]);     // a Vec holding 2, 3, 4
    double_all(&mut v);
    printf("%v %v\n", sum(&v), slice_len(&v));
}
```

```
15 3 9
18 3
```

`&a[i..j]` is the elements from `i` up to, not including, `j`, as a
`slice<T, shared>`; `&mut a[i..j]` is the same for writing. Where a slice
is declared, `&a` is the whole thing, `&a[0..$]`, and `&mut a` the whole
thing for writing. A slice is a borrow, so while it lives its source cannot be
changed except through it, and it cannot outlive the source. Two slices of
one vector conflict only if their ranges overlap, which lets a vector be
split between threads.

A literal can be a slice too. Where a `slice<u8, shared>` is declared, a
byte literal `b"…"` is a slice of bytes that live as long as the program,
and so is any array literal written out as literals, `[1, 2, 3]`, for a
slice of its element type. That is how a table of magic numbers is written
without naming each one:

```
import std;

struct Format
{
    str name;
    slice<u8, shared> magic;
}

fn main()
{
    array<Format, 2> known = [
        Format { .name = "png", .magic = b"\x89PNG" },
        Format { .name = "zip", .magic = b"PK\x03\x04" },
    ];
    foreach (f in &known)
    {
        printf("%v: %v bytes, first %v\n", f.name, slice_len(f.magic), f.magic[0]);
    }
}
```

```
png: 4 bytes, first 137
zip: 4 bytes, first 80
```

A named constant's parts are borrowed the same way: with
`const array<u8, 4> ELF = b"\x7fELF";`, `&ELF[0..2]` is a slice of it.

An `array<T, N>` is a fixed-size run of `N` elements, with `N` written in the
type. It lives wherever it is declared, on the stack for a local, and an
array of plain elements is plain: assigned by copying, passed by copying.
For a list whose size is not known in advance, use a `Vec`.

## Statistics over a slice

Because a slice is how a run of numbers is handed to a function, the
library's statistics take one. `std::stats` has `sum`, `mean`, `variance`
and `stddev`, their sample forms, `median`, `percentile`, `minimum`,
`maximum` and `describe`, over a slice of any numeric type:

```
import std;

fn main()
{
    array<u32, 8> ms = [12, 15, 11, 40, 13, 14, 12, 90];      // response times in milliseconds
    printf("%v requests, %v ms in all\n", slice_len(&ms), sum(&ms));
    printf("mean %.1f, median %.1f, p95 %.1f\n",
           Option::unwrap(mean(&ms)),
           Option::unwrap(median(&ms)),
           Option::unwrap(percentile(&ms, 95.0)));
    match (describe(&ms[0..4]))                               // the first four only
    {
        Some(s) : printf("first four: %v to %v, stddev %.2f\n", s.minimum, s.maximum, s.stddev),
        None    : printf("no data\n"),
    }
    Vec<f64> empty = Vec::new();
    printf("%v\n", Option::is_none(&mean(&empty)));
}
```

```
8 requests, 207 ms in all
mean 25.9, median 13.5, p95 72.5
first four: 11.0 to 40.0, stddev 11.93
true
```

`sum` adds in the slice's own type and is checked like `+`. The others
convert each value to `f64` and return an `Option<f64>`, `None` for an
empty slice, because having no data is ordinary input and not a bug; the
`unwrap` calls above are safe only because the array is not empty.
`describe` gives the common figures at once as a `Summary` struct.

## Text: `str`, `String` and `StringView`

CobaltC has three text types, and knowing which is which saves most of the
confusion a newcomer meets:

```
import std;

fn main()
{
    str fixed = "hello";              // a text constant: copied like a number, never freed
    String owned = "hello";           // owned text, built at run time, destroyed at scope end
    String::append(&mut owned, ", world");
    String::append(&mut owned, 42);   // append takes anything %v prints
    printf("%v / %v / %v bytes\n", fixed, &owned, StringView::len(&owned));

    StringView part = &owned[7..12];  // a view: part of the String, without a copy
    printf("[%v] %v\n", part, part == "world");

    String built = sprintf("%v-%v", fixed, 7);          // printf into a new String
    printf("%v\n", &built);
}
```

```
hello / hello, world42 / 14 bytes
[world] true
hello-7
```

A `str` is text written in the program. It is a plain value, like an integer:
copy it, store it in a struct, pass it anywhere, and never think about who
owns it. It is valid UTF-8 by construction, because the literal syntax has
no way to write anything else.

A `String` is owned text: a buffer on the heap, grown by `append`, destroyed
when its owner's scope ends. It is a resource. Where the declared type says
`String`, a literal is enough, `String owned = "hello";`, and the same holds
where one is assigned or passed in that place; a `str` value is copied with
`String::copy(t)`, and a whole `String` with `String::clone(&s)`. A
literal in a `String` place is the one place text is copied without being
asked. `String::from_utf8` makes one
from bytes that came from outside, checking that they are valid UTF-8, and
returns a `Result`.

A `StringView` is part of a `String`, borrowed, with no copy: `&s[i..j]`,
with byte positions that must fall on character boundaries. It is to a
`String` what a slice is to a vector, and the same borrowing rules apply.
Functions that only read text take a `StringView` throughout the library: a
literal is passed as it is, and so is a `str` or a view; a `String` is passed
as `&s`, which where a `StringView` is expected means its whole text,
`&s[0..$]`. Text fits its declaration, and the rule is the same for
sequences: write `&` on the thing you own (`&s`, `&v`) and nowhere else. These
two are the language's only coercions, and each is exactly the view or slice
you could have written out by hand; nothing else converts implicitly.

All three text types compare with `==` and `<` by their bytes, in any
combination: `s == "ink"`, `part < s`, `t != v`.

A `String` prints through a reference, `printf("%v", &s)`, which says that
`printf` reads it and does not take it. `sprintf` is `printf` into a new
`String`, and `String::appendf` adds formatted text to one that exists.

## When to use `str`

A Rust programmer may take `str` for a borrowed slice and expect it to be
`StringView` under another name; a C programmer may wonder why constant
text needs a type of its own. It has one because CobaltC has no lifetime
parameters. Rust says "this text lives for the whole program" in a
signature, `&'static str`; CobaltC says it with a type.

A `StringView` always views something that ends: a `String`, a buffer, a
parameter. The checker follows every view back to what it views (chapter
7). A struct with a `StringView` field therefore holds a reference and must
end no later than the text it views, and a function that takes two
references and returns a view, or a struct holding one, must say with `from`
which of them it came from. A `str` views nothing that can end, so text in a
`str` is a plain value wherever it goes: in a `const`, in a table of names,
in a struct field (the struct stays a plain value, copied like a number and
kept as long as you like), and in a result, where `fn name(Color c) : str`
needs no `from` clause whatever the function takes.
Without `str`, each of those would be a `String`, allocated and owned for
text that never changes, or a view tied to something that must outlive it.

The price is that nothing becomes a `str`: not a `String`, not a view. Text
read from a file or built at run time can never be passed where a `str` is
declared. That is what decides the type of a parameter:

| Type | Use it for |
|---|---|
| `str` | constants and tables; fields and results that only ever hold text written in the program: a name, a label, a keyword, an error's description |
| `StringView` | every parameter that only reads text while the call runs, to parse, search, compare, print or copy it; a field or result that refers to text owned elsewhere |
| `String` | text built or received at run time that has to be kept |

A `str` parameter is the right choice in two cases. One is a function that
keeps the text after it returns: `Args::flag(&mut args, "verbose", "v",
"print each step")` stores the names in `args`, and if it took views, `args`
would hold references into its caller's text. The other is a message or a
label that every caller writes as a literal, such as `Option::expect(o, "no
config file")`. Anywhere else, take a `StringView`. A `parse_number(str s)`
could not parse a line read from standard input; `parse_number(StringView
s)`, as in chapter 9, accepts `"42"`, a `str`, `&line` and `&line[3..5]`
alike, and costs nothing more.

## Working with text

The functions a program needs to take text apart are on both `String` and
`StringView`, and each returns views into the original rather than copies:

```
import std;

fn main()
{
    String line = "  name = Ada Lovelace ; born = 1815  ";
    StringView trimmed = StringView::trim(&line);
    printf("[%v]\n", trimmed);

    foreach (field in StringView::split(trimmed, ";"))
    {
        Vec<StringView> kv = StringView::split(field, "=");
        StringView key = StringView::trim(kv[0]);
        StringView value = StringView::trim(kv[1]);
        match (StringView::parse<u32>(value))
        {
            Ok(n)  : printf("%v is the number %v\n", key, n),
            Err(_) : printf("%v is the text \"%v\"\n", key, value),
        }
    }

    printf("%v %v\n", StringView::contains(&line, "Ada"), StringView::starts_with(&line, "name"));
    String shouted = StringView::to_ascii_upper(&line);
    printf("%v\n", StringView::trim(&shouted));
    printf("%v\n", &StringView::replace(&line, "Ada", "A."));

    String word = "café";
    printf("%v bytes, %v characters\n", StringView::len(&word), StringView::char_count(&word));
}
```

```
[name = Ada Lovelace ; born = 1815]
name is the text "Ada Lovelace"
born is the number 1815
true false
NAME = ADA LOVELACE ; BORN = 1815
  name = A. Lovelace ; born = 1815  
5 bytes, 4 characters
```

`split` gives a vector of views; `trim` strips ASCII whitespace from the
ends; `find`, `contains`, `starts_with` and `ends_with` search; `replace`
and `join` build new strings; `lines` and `split_whitespace` do what their
names say; `parse<T>` reads a number of any numeric type, strictly, and
returns a `Result` whose error says what was wrong. `chars` gives the
characters of a string as a vector of one-character views, for the rare
program that needs them.

Lengths and positions count bytes, not characters. Most text work never
notices, because it splits and searches by whole substrings; when the
distinction matters, `char_count` and `chars` are there, and the library
makes no claim beyond UTF-8: there are no Unicode case tables or
normalisation, and ASCII case changes are the only ones provided.

## `HashMap<K, V>` and `HashSet<K>`

```
import std;

fn main()
{
    HashMap<String, u32> stock = HashMap::new();
    HashMap::insert(&mut stock, "pens", 12);
    HashMap::insert(&mut stock, "ink", 3);
    *HashMap::entry(&mut stock, "paper", 0) += 500;   // insert 0 if new, then add

    match (HashMap::get(&stock, "ink"))       // look up with a literal: no String is built
    {
        Some(n) : printf("ink: %v\n", *n),
        None    : printf("no ink\n"),
    }
    printf("paper? %v  glue? %v\n", HashMap::contains(&stock, "paper"), HashMap::contains(&stock, "glue"));

    foreach (item, count in &stock)   // in the order they were added
    {
        printf("%-6s %4d\n", item, count);
    }

    HashSet<i32> seen = HashSet::new();
    foreach (x in [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5])
    {
        HashSet::insert(&mut seen, x);          // false when x was already there
    }
    printf("%v distinct\n", HashSet::len(&seen));
}
```

```
ink: 3
paper? true  glue? false
pens     12
ink       3
paper   500
7 distinct
```

A map owns its keys and values. A key may be an integer, a `bool`, a `str`, a
`String`, or a struct or enum built from those; a float is not a key.
`get` returns `Some` of a reference to the value, or `None`; `entry` returns
an exclusive reference to the value, inserting a default first if the key is
new, which is the one-line way to count things. A lookup takes the
key's *probe*: for a map keyed by `String`, any text, so
`HashMap::get(&stock, "ink")`, a view or `&s` asks without building a
`String`, and `entry` makes one only for a key that is not there yet; for an
integer key, the integer itself, `HashMap::get(&counts, 7)`; for a struct
key, a reference to one.

Entries keep the order they were added, in both tools, so a `foreach` over a
map is deterministic. There is no ordered map; to visit keys in sorted
order, copy them into a vector and sort it.

## `printf`

`printf` is C's, checked. The format must be a literal, each specifier must
match its argument's type, and the count must agree, or the program is
rejected before it runs:

```
import std;

fn main()
{
    String item = "widget";
    u32 qty = 12;
    f64 price = 3.5;
    printf("%-8s|%5d|%8.2f|%#06x\n", &item, qty, price, 255);
    printf("%v %v %v %v\n", 1.0 / 3.0, 1.0, 1.0e21, true);
    printf("%5.1f%%\n", 99.5);
    eprintf("a message for standard error\n");
}
```

```
widget  |   12|    3.50|0x00ff
0.3333333333333333 1.0 1.0e21 true
 99.5%
a message for standard error
```

`%d` takes any integer type, with no `%ld` or `%zu` to get wrong; `%x`,
`%o` and `%b` write hexadecimal, octal and binary; `%f`, `%e` and `%g` take
floats; `%s` takes a `str`, a `&String` or a `StringView`. Flags, widths and
precisions are C's. `%v` takes any of these and a `bool` or an enum and
writes it the way CobaltC does: a float with as few digits as read back to
the same value, and a whole number with `.0` so that it still looks like a
float. There is no `%c`; a byte prints as its number.

Standard output is buffered and flushed at every point where it could
matter, so printing line by line is fast and a prompt still appears before
the program waits for input. `eprintf` writes to standard error for messages
that are not the program's output.

## Reading input and files

Chapter 3 used `read_line()`, which returns `Result<Option<String>,
FileError>`: a line without its newline, `None` at the end of input, or an
error. `read_all()` takes the rest of the input as one `String`. For files,
`read_file(path)` and `write_file(path, text)` handle a whole file at once,
and `File::open`, `File::create` and their functions read and write one in
pieces; a `File` is a resource whose destructor closes it. Chapter 15 uses
them in a program. The library also has directories, paths, the clock, the
environment, random numbers, sorting, networking, and more; the guide's
chapter on the standard library is the reference for all of it.
