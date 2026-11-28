# D-0335 — The destruction of deep structures

Status: ACCEPTED (2026-11-21). Decided for the owner, who was away and had asked that no report stay open: the last open
parts of report 022. Implemented by `CHG-0478`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0334, D-0333, D-0204 (stack exhaustion), D-0116 (`clone`)
Affects: `spec/16` §4a and §6, `spec/conformance.md`

## 1. Decisions

1. **Deferred Boxes are kept on a list (022, bootstrap form).** The runtime's destruction of a Box keeps its one
   deferred slot; a second Box in tail position goes on a per-thread heap list that the innermost loop empties down to
   the length it found, so a tree walked or cloned is destroyed without recursion in the bootstrap form too.
2. **A field is in tail position when everything destroyed after it only frees memory (022, default form).** A type
   is *quiet* when its destruction only frees memory (a Box, a plain `Vec`, a struct, enum or array without a `drop` of
   its own, with no `fn` value, thread handle or guard). A link is in tail position when every field destroyed after
   it is quiet, so a node whose `String` is declared before the link no longer recurses. Nothing observable is
   reordered.
3. **Left (022).** The derived `clone` of a type that links to itself only through another type recurses: each type's
   clone is a function of its own declaration, and a loop over the cycle would have to build the other type's values,
   possibly in another module, generic, or with a `clone` written by hand. A field declared before the link that runs
   a destructor of its own (a `Vec<String>`, a type's `drop`) still recurses on destruction: it must be destroyed
   after the whole linked value; declaring it after the link avoids it.

## 2. Consequences

- A destruction touches a per-thread list when two deferred Boxes meet; a chain of `String`-owning nodes costs +1.0%
  instructions (GCC) and takes 18% fewer cycles; the bootstrap form −0.4%.

## 3. Verification

See `CHG-0478`.
