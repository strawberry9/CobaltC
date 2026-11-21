# D-0314 — A literal where a shared slice is declared is a slice of static data; a place rooted at a constant is borrowed from it

Status: ACCEPTED (2026-11-07, under the owner's delegation of 2026-11-07: "go ahead with … and you make the decisions
for all of this"; D-0312 §2's deferred piece); implemented by `CHG-0415`
Kind: Decision (`spec/02-schema.md`); a coercion and a reference form, both the expression a writer could have spelled
Governed by: `CobaltC_Master_Instructions.md` §23
Depends on: D-0006 (no implicit conversion but those the rules write out), D-0036 (constants), D-0047 (slices),
D-0258 (`[Ref-Form-Const]`), D-0312 (`[Coerce-Slice]`, its rewrites)
Affects: `spec/12` (`rule.type.coerce-slice`), `spec/09` (`rule.ref.form`), `spec/17` §1a, `spec/22` §1, `spec/08` §2's
prose; `spec/conformance.md` (seven rows); `coby` (`consts.rs`, `typecheck.rs`, `views.rs`, `lib.rs`), `cbc` (`consts.cb`,
`typecheck.cb`, `check.cb`, `items.cb`); `cobc` through `coby`'s front end

## 1. What was missing

A table of bytes, the most common constant data a systems program has, could not be a slice. `b"\x7fELF"` is an
`array<u8, 4>` value, so `slice<u8, shared> magic = b"\x7fELF";` was a type mismatch, and a struct field
`slice<u8, shared> magic` could not be given a literal. A named constant did not help: `&K` of a literal constant is its
one object (`[Ref-Form-Const]`, D-0258), but `&K[0..2]` was `diag.borrow-of-non-place`, because the place's root `K` is
a call of the constant, a value. The one spelling that worked, `&(*&K)[0..2]`, was not one anyone would write. D-0312
deferred the question (§2) as the static `String` of D-0308 went.

## 2. Decision

1. **`[Ref-Form-Const-Place]`** (`spec/09`): a borrow of a place rooted at a constant written out as literals,
   `&K[lo..hi]`, `&K[i]`, `&K.f`, `&K.f[i]`, is the same borrow of the place with its root written `(*&K)`: through
   the path to `K`'s one object. The front end writes it so (`consts`, before the static pass), so every tool sees a
   form it already has. The place is reached through a shared reference: `&mut K[0..2]` is
   `[Borrow-Exceeds-Source]`.
2. **`[Coerce-Slice-Literal]`** (`spec/12`): an array literal (`[e1, …, eN]` or `[e; N]`) whose elements are written out
   as literals, at a position whose declared type is `slice<τ, shared>` with `τ` plain, is `&(*&K)[0..$]` of an unnamed
   constant `K` of its value. Its elements are typed with `τ` as their expected type, so `[1, 2]` is of `u8` where
   `slice<u8, shared>` is declared. A `byte-literal` is such a literal. The front end adds the constant (named
   `__slice_literal_H`, `H` a hash of its type and value, so equal literals share one object, which no program can
   tell, and both front ends name it alike) and writes the slice, then checks the program again, as for
   `[Coerce-Text]`.
3. **Generic bodies**: a literal in a generic body is one node and so one constant. If two instantiations give it two
   array types, the program is refused (`[Coerce-Slice-Literal-Generic-Disagree]`), as `[Coerce-Text-Generic-Disagree]`
   refuses text.
4. **A repair found on the way**: `crosses_shared`, which decides that a place is reached through a shared
   reference, did not look through `*&x`: `&mut (*&a)[0..2]` was accepted, and the slice wrote `a` (on a constant,
   `cobc`'s build wrote its read-only table and stopped with a signal). `*&x` is now a shared crossing, `*&mut x` what
   `x` is, in both front ends.

## 3. Not decided, and why

- **An exclusive slice of a literal**: refused. A literal's object is no one's to write; a writer who wants to write
  binds the array.
- **A result of static data with no reference parameter** (`fn magic() : slice<u8, shared> { b"…" }`): still
  `diag.lifetime-elision-ambiguous`. A result's references come from a parameter (`rule.temporal.elision`); a
  `'static`-like source would be a new kind of reference, for a case a constant already covers (`&K[0..$]` where the
  slice is used).
- **A literal with elements that are not literals** (`[x, 2]`): no constant; bind the array.
- **Text**: a `str-literal` is already static (`str`, a view where one is declared); nothing here changes it.
