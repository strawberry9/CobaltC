# D-0104 — Destructuring: `..` for the rest, and renaming

Status: ACCEPTED (2026-09-30, owner-delegated: "proceed with all your choices" — round-6 frictions 20 and 24)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9
Depends on: D-0044 (`[Let-Destructure]`)
Affects: spec/11 `[Let-Destructure]`, spec/22 `decl-stmt`

## Problem

D-0044's destructuring names every field of the struct, each bound to
its own name. Two things followed. Closing one `File` held in a
seven-field struct meant naming all seven (`kvlog.cb`'s
`Store { path, out, inp, size, index, records, dead } = st;`). And two
values of one struct type could not both be taken apart in one scope —
the second destructuring would bind the same names — so `huffman.cb`
needed a helper function to take each heap entry apart.

## Candidate mechanisms

1. **`..` for the fields not named, and `field: name` to bind a field to
   another name.** Selected. Both exist in Rust (`S { a, .. }`,
   `S { a: x }`) and JavaScript (`{ a: x, ...rest }`); `..` and `:` are
   tokens already.
2. **Partial moves** (`File::close(st.out)` leaving `st` partly moved) —
   flow facts for every field, and a struct's use after a partial move to
   define.

## Selected design

- An entry is `field` or `field : name`; `..` may end the list. Without
  `..`, every field is named, as before.
- The fields `..` covers are bound to hidden names: each ends as an
  unused binding does, at its block's end, in reverse order with the
  others — so a resource among them is destroyed there, not lost.
- A struct with a destructor still cannot be taken apart (D-0044), and a
  private field is not visible to `..` either.

## Compatibility impact

Additive.

## Revisit conditions

None.
