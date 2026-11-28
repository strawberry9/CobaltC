# CHG-0408 — A text literal is the `String` reference where one is declared; the `_str` lookups and the last text twins deprecated (D-0308)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-03)
Implements: D-0308 (its rule, and the deprecation step of its retirement, `spec/STABILITY.md` §3a)
Affects: `spec/21` 4.77.0 (§1, §2a `[Str-Literal-String-Ref]` and `type.str`, §2d `[Parse]`, §2g, §2h's rows and
rules, a table of deprecated names, the listings); `spec/09` 1.8.0 (`[Ref-Form-Const]`'s cross-reference);
`spec/conformance.md` 3.292.0; `impl/std/text.cb`, `collections.cb` and every `std` file that called a retired name;
both toolchains' checkers (the rule), interpreters and back ends (the static `String`; group C's bodies re-keyed),
and `cbrt` (`cb_str_string`); the self-hosted toolchain; the conformance cases, the showcases, the book, the guide
and the CobaltC programs in the Rust tests

## What changed

1. **`[Str-Literal-String-Ref]`.** A `str` literal, a `str` constant's use or a `str` value where
   `ref<String, shared>` is declared (and `ref<K, shared>` once `K` is fixed as `String`, as D-0162) is a reference
   to a never-ending static `String` of its bytes, written without `&`. The checkers record it as a rewrite to
   `std::String::static_ref(t)`, a function of `std` that is not exported (`diag.name-not-visible` for a program),
   exempt from "returns a reference but takes none" as `StringView::of` is. `coby` interns one `String` object per
   text (`str_string`, a std-only intrinsic); `cobc` and `cbc` write one `static const` `String` per literal (no
   call, no allocation) and, for a `str` value, a call of `cbrt`'s `cb_str_string`, which interns one leaked
   `String` per text. Not where `ref<String, exclusive>` is declared, not for a `K` nothing fixes, not `&L`.
2. **Deprecated** (`[[deprecated("…")]]`, each message naming its replacement): the six `_str` lookups
   (`HashMap::get_str`, `get_mut_str`, `contains_str`, `remove_str`, `HashSet::contains_str`, `remove_str`); the
   `String::` twins `len`, `chars`, `char_count`, `code_point`, `parse<T>`; and the functions `==` and the view form
   were written as, `String::eq_str`, `String::eq_view`, `StringView::eq_str`, `String::view`, `StringView::sub`.
   The bodies of the last five move to unexported names (`String::same_str`, `String::same_view`,
   `StringView::same_str`, `String::part`, `StringView::part`), which `[Text-Mixed-Eq]`, `[View-Eq]` and
   `[View-Form]` now name and the front ends' rewrites call, and every native of `cobc` and `cbc` that read the
   old name reads the new one; the public names are wrappers. `String::eq` and `String::less` stay (the key-type
   bounds call them), as do `String::join`, `as_bytes`, `from_utf8` and `StringView::of`.
3. **Migration.** Every tracked tree, by `stress/retire/migrate_c.py`: a lookup `X_str(` as `X(`; `String::NAME(`
   as `StringView::NAME(` for group B; `String::eq_str(&s, t)` and the rest as `s == t` (`!=` under a `!`),
   `String::view(&s, lo, hi)` and `StringView::sub(v, lo, hi)` as `&s[lo..hi]` and `&v[lo..hi]` (`$` for a bound
   that is the length). `std` and the self-hosted toolchain each checked as one unit (`COBALTC_CHECK_ALL`), the
   conformance cases, the showcases and the book's examples file by file; the guide, the book's prose, `spec/21`'s
   listings and the Rust tests' programs follow their sources.
4. Found and repaired: `cobc`'s and `cbc`'s account of what a function does with a reference parameter
   (`reader_uses`, D-0200) takes `&(*r)[lo..hi]` handed to a reader as a read of `r`, the pairing `CHG-0404` left
   open; the view's lowering accepts a reference parameter with no access path (token 0), the fault that withdrew
   it then.
5. Found by the migration of the self-hosted toolchain: `&v[lo .. hi]` of a `StringView` binding was taken by both
   checkers as a borrow of the binding (the slice of a place), where `[View-Form]` makes it
   `StringView::part(v, lo, hi)`, a call that takes the view by value; `StringView after = &rest[k..$]; … rest =
   &after[j..$];` was `diag.aliasing-conflict`, as `StringView::sub(rest, …)` had not been. Such a part now has no
   referent of its own and derives what `v` derives, its range pushed, as the call's result does
   (`referent_of`, `deriv_facts_of`, `inherited_facts_in` in `typecheck.rs`; their ports in `typecheck.cb`); a
   write of the viewed `String` while a part of a view of it is live, and its end before the part's last use, are
   rejected as before.
   Item 5's part was still taken as a reference into the binding where a call's result may give back a reference
   stored in its argument's referent (`inherited_facts_in`'s `reaches_stored`): `StringView::split(&args[0..end],
   ",")` was tier D, and `cbc`'s `main.cb` lost its decided lowering (`report.cb`); the part reaches what `v`
   reaches there too.
6. `&L` of a literal, still `diag.borrow-of-non-place` ([Ref-Form-Not-Place]), gives a repair naming the rule:
   the literal, without `&`, is the reference.
7. Conformance: `conf.view-part-reassigns-its-view` (item 5); `conf.map-lookup-by-literal`, `conf.set-lookup-by-literal`,
   `conf.map-get-literal-non-string-key-rejected` (replacing the four `_str` rows, whose file retires);
   `conf.literal-string-ref-param`, `-returned`, `-no-amp`, `-exclusive-rejected`, `-unfixed-k-rejected`;
   `conf.std-text-twins-deprecated`, `conf.std-text-twins-allowed`; `conf.text-private-names-hidden`.

8. Found by the gate on the migrated trees:
   - `cobc` lowered D-0302's whole-text view of a `Vec` element handed to a pure view parameter
     (`StringView::len(&w[1])`) from a copy of the argument it dropped after the call; its index lowering is
     cached by the node's address (`vec_index_exprs`), so the next copy, made at the same address, read the first
     one's element: `StringView::len(&w[0])` then `StringView::len(&w[1])` gave 3 and 3
     (`conf.generic-slice-index` under both C compilers, the guide's `swap` under gcc). The copy is kept for the
     function, as `alias_kept` keeps the lowering's other made nodes. `cbc` keys the cache by node id and had no
     such fault.
   - A `String` local only read had no runtime object when its readers took `ref<String, shared>`
     (`String::len(&s)`); read as `StringView::len(&s)`, D-0302's `&s[0..$]`, it was bound. `cobc`'s and
     `cbc`'s objectless analysis (`reader_arg`) takes `&s[0..$]` for a parameter that keeps nothing of the view
     (`pure_view`), when the call's other parameters are views or plain data and its other arguments inert, the
     conditions under which the call's lowering makes the view unminted.
   - `StringView::parse<T>`'s `T` is checked by both checkers as `String::parse<T>`'s was
     (`conf.parse-not-numeric` is static again).
   - `cbc`'s copy of the runtime header (`selfhost/back/header.cb`) is regenerated with `cb_str_string`, and
     `validate_c.py` classes `cb_str_string` as a `value` operation.

## Not as the proposal wrote it

- Items 5 and 8 were not foreseen: the proposal took `&v[lo .. hi]` to be what `StringView::sub` was in every respect, and the replacements to lower as the twins did.
- The listings of `spec/21` show the private bodies (`String::same_view`, `StringView::same_str`) in the places of
  the public functions whose bodies moved to them, as `CHG-0405` showed `copy_str` in `from_str`'s; a deprecated
  function whose body did not move is listed as it was, as `CHG-0404` left its wrappers.
- The git-ignored `stress/` corpus the gate's sweep reads (the programs `stress/d0281/objectless/decided.list`
  names, and `stress/perfguide`) was migrated by `migrate_c.py` file by file, checked as the tracked trees were,
  rather than by `sed`.
- `migrate_c.py` wrote two shapes the checker accepts but no one would write, `&(&s)[lo .. hi]` (for
  `StringView::sub(&s, lo, hi)`) and `s == &t` (for `String::eq_view(&s, &t)`); the script writes `&s[lo .. hi]`
  and `s == t` now, and the sites it had written were rewritten so.

## Verification

The gate (D-0307), gate profile, on the final tree (run `20261103_154655`): every unit green. The first run on the
migrated tree (`20261103_105943`) was red in `tests`, `corpus-coby`, `corpus-cobc`, `guide`, `model`, `selfhost` and
`lowering`; item 8 is what it found. On the final tree: the conformance corpus under `coby` and compiled under gcc
and clang; `std`'s 147 unittests; the guide's 413 examples, the book's 72 and the 90 showcases, identical under
`coby` and `cobc` (gcc, clang); `cbc`'s C identical to `cobc`'s for 1184 programs (0 different), its formatter and
parser for 438 and 1552; the model's compare, certify, factcheck, validate_c and replays; the sweep 6416 decided,
none decided before and not now; the perf guide's 72 rows and the bench's 18 `cobc` counts, none more than 3%
above the baseline (Update 163's), `bench/strings` measuring `StringView::len` where it measured `String::len`.
