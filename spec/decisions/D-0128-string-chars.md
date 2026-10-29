# D-0128 — The characters of a text: `String::chars`, `StringView::chars`

Status: ACCEPTED (2026-10-01, the owner: "yes proceed to draft and implement, and ensure the note explains why a zero-allocation form is not currently possible and why")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0053 (`StringView`, `[View-Form]`), D-0113 (functions straight from a `String`), D-0042 (`foreach`), D-0127 (text patterns)
Affects: `spec/21` §0 and §2h `rule.stdlib.stringview` (3.50.0), `spec/conformance.md` (3.119.0), the guide §21, `impl/src/prelude.rs`

## Problem

Going through a text's characters is one of the commonest things a
program does with text, and CobaltC made the programmer do it by hand.
`foreach` visits a `Vec`, an array, a slice, a set or a map, not a
`String`; there is no `char` type; so walking the characters meant
walking the bytes, finding where each character starts (every byte that
is not a UTF-8 continuation byte, `0b10xxxxxx`), and cutting each
character out with `String::view`:

    ref<Vec<u8>, shared> bytes = String::as_bytes(&s);
    usize start = 0;
    foreach (i in 1..Vec::len(bytes) + 1)
    {
        if (i == Vec::len(bytes) || bytes[i] & 0xC0 != 0x80)
        {
            StringView c = String::view(&s, start, i);
            …
            start = i;
        }
    }

Eight lines, a bit mask and an off-by-one hazard for "for each
character". The owner: "this is asking too much from the programmer,
for such a simple operation."

## Candidate mechanisms

1. **Leave it.** The loop above works and is written once per program.
2. **`String::chars(&s)` and `StringView::chars(v)` : `Vec<StringView>`**,
   one view per character, in order, written in CobaltC in `std`.
   `foreach (c in String::chars(&s))` is the whole loop. Selected.
3. **A `char` type** (a code point, as in Rust or C#) and a character
   literal. A new type, a new literal form, conversions to and from
   integers and text, and printing rules — a great deal of language for
   what candidate 2 gives with none. Not adopted; a code point, when a
   program needs one, can be a later `std` function over a view (see
   *Revisit*).
4. **A zero-allocation form** — the characters visited without first
   building a `Vec`. Not currently possible as a `foreach`; see the next
   section.

## Why there is no zero-allocation form

The selected functions build a `Vec` holding one `StringView` per
character: one allocation, and 24 bytes per character on a 64-bit
target (a view is a slice, three words: its source, a start and a length,
`spec/16` §3a), freed when the `Vec` ends. A form that allocates nothing is not possible in CobaltC as
it stands, for these reasons:

- **`foreach` knows a fixed set of collections.** `rule.control.foreach`
  visits a `Vec`, an array, a slice, a `HashSet` or a `HashMap`, and
  `rule.control.foreach-range` a range of integers. Each is the language's
  own: the elements already exist in memory (or, for a range, are
  numbers the loop counts itself), and the loop reads them in order. The
  characters of a text are not stored anywhere as views: each exists only
  once something computes where it starts and ends. To visit them one at
  a time without storing them, `foreach` would have to call a function
  the library supplies — "give me the next character after this one" —
  on each step.
- **There is no way for a library type to supply that function.** Other
  languages do it with an *iterator protocol*: an interface (Rust's
  `Iterator` trait, C#'s `IEnumerable`, Python's `__next__`) that any
  type implements and the loop calls. CobaltC deliberately has no traits
  or interfaces: a generic's bounds are a fixed, built-in set (`number`,
  `integer`, `ordered`, `eq`, `clone`, …, `spec/12`), a program cannot
  declare new ones, and method-call syntax was declined, finally, by the
  owner (CHG-0132). Adding an iterator protocol would be adding exactly
  that machinery, for every kind of sequence, not a function for text;
  it is a language decision of its own, not part of this one.
- **A slice cannot stand in for the `Vec`.** A slice (`slice<T, m>`)
  borrows elements that are already stored somewhere; there is no
  storage of views to borrow until something builds it.
- **The other allocation-free shapes are not a loop over characters.**
  A function that takes a closure and calls it per character
  (`StringView::each_char(v, [](StringView c) { … })`) allocates nothing,
  but its body is a closure, not a loop body: `break`, `continue` and
  `return` do not reach the enclosing function, and a closure that writes
  an outer variable holds it exclusively for its whole life
  (`rule.fn.closure`). A cursor function
  (`StringView::char_at(v, at) : Option<StringView>`, stepped by the
  caller's own `at += StringView::len(c)`) also allocates nothing and
  works with `while (Some(c) = …)`, but it hands back the bookkeeping —
  the position — that this decision exists to remove.

So the `Vec` is the price of a plain `foreach` today. For ordinary text
it is small; a program that must not allocate (a very large text, a hot
loop) can still walk `String::as_bytes(&s)` as before.

## Selected design

    StringView::chars(StringView v) : Vec<StringView>
    String::chars(ref<String, shared> s) : Vec<StringView>     // ≡ StringView::chars(&s[0..$])

- The result holds, in order, one view per character of the text: a
  character starts at byte 0 and at every byte that is not a UTF-8
  continuation byte (`0b10xxxxxx`), and runs to the next start or the
  end. Each view is one to four bytes, and the views together cover the
  text exactly once. Empty text gives an empty `Vec`.
- Every view borrows what the argument borrows (the `String`), as any
  view does (`[View-Form]`): while one is in use the `String` cannot be
  changed (`diag.aliasing-conflict`). The `Vec` itself is new and owned
  by the caller.
- The views are ordinary `StringView`s: they print (`%v`), compare with
  `==` against a view or a `str` (`[View-Eq]`), are matched by text
  patterns (D-0127), and append to a `String`.
- "Character" means a Unicode scalar value, the unit UTF-8 encodes, not
  a user-perceived character: `"e\u{301}"` (e and a combining accent) is
  two characters, as it is two code points.
- Both functions are ordinary exported CobaltC in `std`, the same in
  every implementation; `String::chars` follows D-0113's pattern of a
  `String::` function applying the `StringView::` one to `&s[0..$]`.

## Compatibility impact

Additive. Two new names in `std`, shadowed by a program's own (D-0024).
No rule, grammar, diagnostic or native code changes.

## Revisit conditions

- If an iteration protocol is ever added to the language, a lazy form
  can be added beside these under a new name; these keep returning a
  `Vec`, since programs index it and take its length.
- If programs need code points, `StringView::code(c) : u32` (the scalar
  value of a one-character view) is the natural companion; it was not
  needed for the owner's case and is left out.
