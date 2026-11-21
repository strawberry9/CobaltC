# CHG-0416 — Text v2 completed: the whole-sequence `[0..$]` migrated, a `str` sliced, the hints, the spec stated by what it denotes

Status: implemented (2026-11-08), D-0312 (completing it; D-0314 for the literal slices it leans on)
Affects: `spec/21` 4.81.0 (`[View-Form]`, the Text table, §1/§2e/§2h prose), `spec/22` 2.62.0 (§1), `registry/diagnostics.md`
1.64.0 (`diag.slice-of-whole`); `coby` and `cbc`: `typecheck` (the lint `whole_slice_lint`, `place_text`, the `str` slice
rewrite `Rewrite::StrView`, `view_base` through `*e` of a call's result and through `*&x`, a probe's resolved type in a
generic body's call, `slice_len`'s argument under `[Coerce-Slice]`, the hints), `views`/`rw_expr` (`StringView::of(t)`
around a sliced `str`; a column-0 marker on the `0` of every slice the front end writes), `each` (the `foreach`-over-text
hint), `modres` (the not-exported text functions' hint), the drivers (`COBALTC_TEXT_LINTS=all`), `lib.rs`/`check.cb`
(a note's column); the corpus (book, showcases, guide, `std`, the self-hosted toolchain, the fuzz programs, the
examples, the conformance cases); `stress/text_v2/tools/fixwhole.py` (the migration, driven by the lint)

## What changes

1. **The idiom goes** (D-0312 §3.2, §9 step 4, §10). `&x[0..$]` where a slice or a `StringView` is declared is the whole
   of `x`, which `[Coerce-Slice]` and `[Coerce-Text]` make `&x` there since CHG-0413; the corpus still wrote the suffix
   at some 1,000 sites. `--check` now says so (`diag.slice-of-whole`, with the column of the slice): for a `Vec`, an
   array or a `String` place "write it without `[0..$]`"; for a view, a slice, a `str` or a reference to a sequence,
   "write `x`, without `&` and `[0..$]`" (`&(*r)[0..$]` is `r`); for a call's result and an array literal, the plain
   borrow; for an array literal of literals where a shared slice is declared, the literal itself
   (`[Coerce-Slice-Literal]`, D-0314). The lint judges every position `rule.type.expected` knows, `slice_len`'s
   argument, and the operands of a text comparison; not a slice the front end wrote itself (its `0` has column 0),
   and not `std`'s bodies unless `COBALTC_TEXT_LINTS=all` (the migration of `std` itself). `fixwhole.py` rewrites what
   the lint names and checks again until none is left; the book, the showcases, the guide's examples and prose, `std`,
   the self-hosted toolchain, the fuzz programs, the examples and the conformance corpus (fifteen cases that test the
   form itself left alone) are migrated. `grep -c '\[0\.\.\$\]'` over the showcases and the book is zero (§10 bullet 3).
2. **A `str` is sliced** (D-0312 §2): `&t[lo..hi]` is `&StringView::of(t)[lo..hi]`, a view of static text, which never
   ends; `[View-Form]` says so, and says what every view denotes with no private function named (R7).
3. **Two gaps of `[Coerce-Text]` closed.** `&(*h(&s))[0..$]`, the form the rule writes for a `ref<String, _>` a call
   gives, was "`String` is not sliced" (`view_base` did not see through `*e` of a call's result, nor through `*&x`, which
   `[Cmp-Through-Shared-Ref]` writes for `&owned < "tea"`); and in a generic body a `String`-keyed lookup by a literal
   (`HashMap::get(m, "k")` with `V` open) was not coerced, because the argument was judged against the declared `probe<K>`
   rather than its resolved `StringView`.
4. **`slice_len(&v)`** for `&v` a reference to a `Vec` or an array (an array element included) gets `[Coerce-Slice]`'s
   rewrite, as any slice position does; before, the interpreter faulted at run time on `slice_len(&rows[0])`.
5. **The hints** (v1 audit C6/R6, D-0312 §9 step 5): a literal, a `str` or a view where a `ref<String, _>` is declared
   (an argument, a field, a local, an array element, a variant's payload, a function's or a `return`'s result) says what
   can stand there (`str`, `StringView`, or a `String` made by `String::copy`) instead of "pass `&…`"; `printf("%v", s)`
   of a `String` says "write `&s`"; `foreach (c in &s)` over text names `StringView::chars` and `as_bytes`; the hints
   that said `StringView::of(x)` and `&x[0..$]` quote the literal or the place and say why the rule did not reach (a
   type parameter); `String::eq`, `less`, `StringView::eq`, `less` and `String::as_bytes` "not exported" name the
   operators and `StringView::as_bytes(&s)`; a `str` where a `String` is assigned, returned or given as a result gets
   `String::copy(…)`.
6. **`coby`'s nested array literals** (found by the ordinary-programs pass over the ten sessions' programs, two of
   whose tests failed under `coby` alone): with no element type from outside, the interpreter gave the later elements
   the first element's type only when that type was a number; for `[[20: usize, 1], [3, 0x7a6d1e854c3c40af]]` the
   second row's literals were evaluated as `i32` and faulted `diag.literal-out-of-range`, where `cobc` was right. An
   array's type propagates too (`eval_array_lit`). `conf.array-literal-rows-take-the-first-type`.
7. **`coby`'s frame records on a transfer** (found by the same pass: one session's `sed` under `--unittest` stopped
   with "stale object"): a resource moved out of a block's binding into an enclosing block's binding stayed in the
   first block's list of what it destroys at its end, so `Vec<u8> t = ps; ps = hold; hold = t;` across three blocks
   ended `hold`'s object with `ps`'s block (`invalidate_other_bindings` now takes the object off the frames that no
   longer bind it). `conf.transfer-out-of-inner-block-binding`.
8. **The guide** has the one-screen text page (D-0312 §5): three types, "text fits its declaration", "write `&` on the
   thing you own and nowhere else", one program; no "except", no decision number. The guide, the book and `spec/12`
   say the same thing about conversions everywhere: none between types, with two coercions, text and sequences fitting
   their declaration (the guide had said "no coercions" in one place and "two coercions" in another; `spec/12` "the one
   coercion"). `spec/21`'s prose names
   `[Coerce-Text]` and `[Coerce-Slice]` where it still named the clauses they replaced.
9. **Found by the release battery** (both front ends): `&K` of a constant array where a slice is declared was refused
   (`diag.borrow-of-non-place`: `[Coerce-Slice]` made it `&K[0..$]`, which only `[Ref-Form-Const-Place]`'s front
   rewrite accepts); it is `&(*&K)[0..$]`, `[Coerce-Slice]`'s form for a reference (`slice_len(&K)` too). The lint
   said `&f()` for `&f()[0..$]` where `f` already returns the slice or the view (the guide's `crc32` example did not
   compile); it says the call, without `&` and `[0..$]`, and `&K[0..$]` of a constant is `&K`.
   `diag.text-copy-literal` warned on `&String::copy("…")` where a `ref<String, shared>` is declared, where no
   literal fits; it no longer does, and the 20 sites it rightly names in the selfhost compiler are migrated
   (`stress/text_v2/tools/fixcopy.py`). `conf.coerce-slice-of-constant`.

## Not changed, and why

- The tombstones of D-0306/D-0308/D-0312 stay (D-0312 §7 says they *can* go): the ordinary-programs pass of 2026-11-07
  met 88 uses of removed names across ten sessions' programs, each repaired by the tombstone's message; they go once
  the stress corpus and the sessions are migrated and nothing names them.
- `[Str-Literal-View]`, `[Str-Literal-String]`, `[Ref-String-View]`, `[Str-Value-View]`, `[View-Eq]` and
  `[Text-Mixed-Eq]` stay as named clauses of `[Coerce-Text]` and `[Cmp-Text]` (no text of their own, no position
  list): 22 conformance rows and the model cite them by those names.

## Conformance

`conf.str-sliced`, `conf.ref-string-call-result-view`, `conf.probe-literal-in-generic-body`, `conf.slice-len-of-reference`,
`conf.array-literal-rows-take-the-first-type`, `conf.transfer-out-of-inner-block-binding`, `conf.coerce-slice-of-constant` (new); the migrated cases keep their rows. `--check`'s lint has no case (a warning on an accepted program); its text is
`cobc`'s and `cbc`'s alike, checked on the probes of `private/text-audit-probes`.
