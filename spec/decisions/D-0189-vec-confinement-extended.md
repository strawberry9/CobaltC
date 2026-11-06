# D-0189 — `Vec` kept unchecked in more of the places programs keep it

Status: ACCEPTED (2026-10-08; the owner delegated the decision: "I leave all these matters for you to decide. Try to do as much as you reasonably can autonomously", on `private/vec-group2-proposal.md`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0018 (use-time checking), D-0023 (natives), D-0107, D-0188 (dual bodies; `spec/08` §4)
Affects: `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs` (`cb_write` with no path); `spec/conformance.md` (3.182.0); the guide's performance chapter; `CHG-0217`

## Problem

After the `Vec` group-1 natives (2026-10-08), a confined local `Vec`, or one
a confined local is passed to, ran at close to plain C, and every other
`Vec` paid the runtime's tracking: a `Vec` field of a struct given by
reference (the shape of most real code: functions over a struct that owns a
list) up to 300 times slower than the same loop on a local; any std call but
`push`, `pop`, `reserve` and `len` unconfined a local; a borrowed temporary
key (`&narrow<i64>(q)`) made a runtime object per search.

## Candidate mechanisms (`private/vec-group2-proposal.md`)

- **G2a, more confining calls.** Selected. `swap`, `reverse`, `insert`,
  `remove`, `truncate`, `clear`, `contains`, `index_of`, `binary_search`
  and `sort` keep a local (or a confining parameter) confined. Their native
  bodies for plain elements touch elements only as bytes, form no element
  path and keep nothing; on a confined vector their unchecked forms
  (`nc_variant`: the checks through the vector's own path and the
  live-object test removed, both unable to fail there) are called with the
  root path. For an element type without a native body the prelude's body
  is called through the root path.
- **G2b, struct fields through the function's reference.** Selected, by the
  entry test of D-0188 rather than the caller-side variants first proposed:
  a function whose one reference parameter `b` is to a struct with `Vec`
  fields of plain elements, whose other parameters are plain values, whose
  body loops, and which uses such a field only as a confined vector is used
  (`field_confined_uses`) is compiled twice. The fast half treats those
  fields as confined; the dispatcher runs it when `b`'s path is valid,
  over a live object and not lock-derived (`cb_paths_disjoint`) and no
  element of those fields has a live object (`cb_live_in`). The caller-side
  form would cover only structs that are confined locals of their caller;
  the entry test covers a struct held anywhere, at one or two runtime calls
  per call, which a loop amortizes (hence the loop requirement).
- **G2c, the same for structs not provable by the caller.** Subsumed by the
  form of G2b chosen.
- **G2d, `foreach (x in v)` over an exclusive `Vec` parameter.** Partly:
  `each.rs` loops over it as `&mut *v`; it is now lowered as `$each_at_mut`
  is (521 → 29 ns an element). The rest needs the parameter itself to be
  confining, left for later.
- **G2e, temporary keys.** Selected for `contains`, `index_of` and
  `binary_search` over integer and `bool` elements: a temporary key is
  passed as the address of its value, with no object (nothing else can
  reach a temporary; the native reads it once and keeps nothing), lowered
  at the element type.

- **Also, from group 3 where a native form suffices.** `dedup` for integer
  and `bool` elements is a native body (its `==` is C's); `retain` and
  `position` with a capture-free closure literal call the closure's code with
  bare element addresses (as `sort_by`'s literal has since D-0062), each
  element's borrow kept as a `cb_elem_access` check in the body's mode and
  order; `sort_by`'s literal is now also taken through an exclusive
  reference parameter. A capturing closure, and any closure value, still
  takes the prelude's body.

## Soundness

As for a confined vector: in the fast half nothing but `b` reaches the
fields or their elements. `b` is the function's one reference, so no other
argument aliases it; an element object, the one thing a check on `b` does
not see (`spec/08` §4), is ruled out by the entry test; nothing the body
does forms an element path or object (the uses admitted), and another
thread's step cannot change `b`'s standing (`spec/08` §4). Where the test
fails, the checked body runs and faults where it always did
(`conf.vec-struct-field-held-element`). `cb_write` with no path (0) now
returns as `cb_read` always did: a confined field has no path of its own,
and `grow` is handed none.

## Results (ns per element, `stress/vec_survey`)

`b.items` through `ref<Bag, exclusive>`: indexed write 546 → 1.8, read 31
→ 1.1, `pop` 280 → 8.3, `push` 41 → 9.0, `swap` 152 → 1.8; `swap` on a
local 135 → about 2; `binary_search` 650 → 142 per lookup (with the sort);
`foreach` over an exclusive parameter 521 → 29; `retain` with a literal
1,066 → 3.5; `dedup` 400 → 2.2; `sort_by` through a parameter 22,100 → 239.

## Not decided here

Element references handed to function values (`position`, `retain`,
`dedup`, `binary_search_by`, closures over `index_shared`), about 1 µs an
element: a design of its own (cheaper element borrows in `cbrt`, or
inlining a closure literal into the std function it is passed to). A struct
inside a `Vec` takes the checked body (`cb_paths_disjoint` refuses a path
over a reclaimed element object; a single path could be admitted).
