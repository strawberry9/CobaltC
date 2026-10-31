# D-0044 — Small Gaps Found by Tier 6

Status: ACCEPTED (2026-09-26, delegated by the owner: "you can resolve the rough edges for me")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0019, D-0032, D-0041, D-0042, rule.stdlib.string, rule.stdlib.text, rule.init.let, rule.control.foreach
Affects: rule.stdlib.text, rule.stdlib.hashmap (`Vec::clear`), rule.init.let, rule.control.foreach, spec/22, diag.not-ascii (new)

## Problem

Writing the Tier 6 showcases (`showcase/tier6`) met five gaps, each
with a workaround that obscured what the program meant:

1. No way to copy a `String`: `sprintf("%s", &s)`.
2. No way to add one byte to a `String`: text built in a `Vec<u8>` and
   checked with `String::from_utf8`, even for ASCII.
3. No way to empty a `Vec` or `String`: `drop(v); v = Vec::new();`.
4. A resource field could be moved out only of a single-field struct,
   so a function could not return a `String` beside other results in a
   struct: it wrote through an out-parameter instead.
5. A `foreach` over a map could not also give the entry's position.

## Constraints

No new tokens; `String`'s UTF-8 guarantee kept; no new ownership rule.

## Selected design

1. **`String::clone(&s) : String`**, named as `Rc::clone`.
2. **`String::push_ascii(&mut s, b)`**: a byte below 128; any other
   faults `diag.not-ascii` (dynamic), since a byte of 128 or more is not
   UTF-8 on its own. Arbitrary bytes still go through `Vec<u8>` and
   `String::from_utf8`, which reports where they stop being UTF-8.
3. **`Vec::clear(&mut v)`** destroys the elements, first to last as the
   Vec's destructor would, and keeps the buffer; **`String::clear`**.
4. **`Name { f1, …, fn } = e;`** takes every field of a struct apart,
   each field named once (`[Let-Destructure-Fields]`,
   `diag.type-mismatch`); the struct is consumed. A struct with its own
   destructor cannot be destructured (`diag.move-out-of-field`): its
   destructor would run on a struct its fields had left.
5. **`foreach (i, k, v in m)`** over a `HashMap`: `i` is the entry's
   position. Three names are refused for any other collection.

## Rejected alternatives

- **`String::push(&mut s, b)` accepting any byte:** would break
  `inv.string.utf8-validity`.
- **A general `clone` for `Vec<T>`:** needs `T` to be copyable, which
  is a bound (D-0010). `String` covers the case met.
- **Partial moves (`auto x = s.f;` leaving the rest):** a struct with
  some fields moved out is a state every rule would have to consider;
  taking all fields apart at once keeps the struct whole or gone.
- **`..` for "the remaining fields":** a new token, and silently
  destroying unnamed resource fields.

## Semantic rationale

1–3 are ordinary CobaltC in `std` (`spec/21`). 4 extends
`[Let-Destructure]`'s single `relocate-out` to one per field; the
container ends with nothing left in it. 5 binds the loop's own counter.

## Usability

    String copy = String::clone(&name);
    String::push_ascii(&mut line, b'\n');
    Vec::clear(&mut buffer);
    Entry { item, change } = parse_line(&line)?;
    foreach (i, word, count in &counts) { … }

## Implementation-feasibility

`std` functions; the destructuring statement carries a list of fields
in all three tools (`coby` ends the container without destroying what
moved out, as `cobc`'s `cb_end_moved_out`); the parser's `foreach`
binds the position for a third name.

## Compatibility impact

Extension. Destructuring was possible only for a single-field struct,
which still works.

## Prior-art status

Rust `String::clone`, `Vec::clear`, struct destructuring `let S { a, b } = s;`
(which also forbids moving out of `Drop` types); Python
`enumerate(d.items())`.

## Invariant traceability

`inv.string.utf8-validity`: `push_ascii` adds only ASCII.
`inv.resource-authority`: each field has one owner after
destructuring; the container's destructor never runs on a moved-from
struct.

## Revisit conditions

- Copying other collections (needs bounds or a native per-type clone).
- Characters in `String`s (deferred by the owner, 2026-09-26).
