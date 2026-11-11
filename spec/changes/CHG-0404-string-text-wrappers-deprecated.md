# CHG-0404 — The fourteen `String::` text wrappers deprecated; every tracked tree written without them (D-0306)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-02)
Implements: D-0306
Affects: `impl/std/text.cb` (the fourteen attributes) and every `std` file that called a wrapper; `spec/21` 4.75.0
(§2h's rows, `[Contains]`'s text, the listings); `spec/15` 1.19.1 and `spec/OVERVIEW.md` 1.3.1 (an example);
`spec/conformance.md` 3.290.0; the self-hosted toolchain; the conformance cases, the showcases, the book, the guide,
the fuzz harness and the CobaltC programs in the Rust tests; `impl/src/lib.rs` and the self-hosted `check.cb` (a
repair, item 3)

## What changed

1. `String::as_view`, `find`, `starts_with`, `ends_with`, `trim`, `trim_start`, `trim_end`, `split`, `contains`,
   `replace`, `strip_prefix`, `strip_suffix`, `lines` and `split_whitespace` carry `[[deprecated("…")]]`, each
   message naming the `StringView::` form (`as_view`'s: `&s` where a `StringView` is declared, `&s[0..$]`
   elsewhere). Their own `unittest`s stay (they follow them).
2. Every call in the tracked trees was rewritten by `stress/retire/migrate_b.py`: `String::NAME(` as
   `StringView::NAME(` (a first argument that is a reference but not a place, a call's result such as
   `HashMap::key_at(…)`, written `&(*e)[0..$]`: D-0302 views a place, not a call's result); `String::as_view(E)`
   as `E` where the checker gives the same verdict, otherwise `&X[0..$]` or `&(*E)[0..$]` (an `auto` initializer
   at once); `StringView::of(L)` as `L` where the checker gives the same verdict (a literal given to a type
   parameter, such as `Vec::push`'s, keeps `of`). `std` and the self-hosted toolchain are each one unit, checked with
   `COBALTC_CHECK_ALL`; the conformance cases, the showcases, the book's examples and the fuzz harness file by file.
   Two rejection cases whose diagnostic quotes the rewritten line were written by hand, the diagnostic the same. The
   book's chapters, `spec/21`'s listings and `spec/conformance.md`'s rows follow their sources; the guide and the
   Rust tests' programs were rewritten by hand.
3. **Found and repaired: a rewrite of a body reached only through a rewrite was dropped.** The checker records its
   rewrites (D-0162's literals, D-0164's text comparisons, D-0302's views) and the program is checked again with
   them applied; the second check's own rewrites were discarded. A body first reached in the second check, such as
   `String::eq_view`, reached from `Json::get`'s `members[i].key == key` once D-0164 wrote that `==` as the call,
   was then run with a `ref<String, shared>` where a `StringView` was declared: under `coby` every key compared
   unequal (`Json::get` found nothing), and `cobc` wrote C that does not compile. Each round's rewrites are now
   applied and the program checked again until a round records none (at most eight), in `lib.rs` and in the
   self-hosted `check.cb`. Before this Update no `std` body reached so needed a D-0302 rewrite, which is why it
   was not seen.
4. The diagnostic that suggested `String::as_view(&s)` for ordering two texts suggests `&s[0..$]`, in both
   checkers.
5. Conformance: `conf.std-string-wrappers-deprecated` (✗ `diag.deprecated-use`, `String::find`),
   `conf.std-string-wrappers-allowed` (ok under `--allow-deprecated`, all fourteen); `conf.json-get-keys`
   (item 3's repair).

## Not as the proposal wrote it

- The renames are not always type-correct: a reference that is not a place is not a view (item 2).
- **The C is not kept byte-identical.** The proposal took `cobc`'s `as_view` paths to pair `String::as_view(r)` with
  the forms D-0302 writes (`&s[0..$]`, `&(*r)[0..$]`). They pair them where a call's argument is lowered, but not in
  D-0200's account of what a function does with a reference parameter (`reader_uses`), nor in the unminted lowering
  of a view argument (`lower_as_view_unminted`): a function whose body hands `&(*r)[0..$]` to a reader is taken to
  keep `r` (`String::eq_view`'s body, written `StringView::eq(a, v)`, makes its callers form a checked borrow), and a
  `&s` argument is minted where `String::as_view(&s)` was not. Teaching the account the slice form changed which
  `foreach` binders are lowered as bare element pointers, which the slice's lowering does not serve (`cobc` stopped
  with "a view of a String with no access path"), so it was withdrawn: pairing the forms is lowering work in both
  compilers, for a later record. The second forms are lowered as the first, so a rewrite refused for its C would
  only have been longer, and every rewrite the checker accepts is kept. `cobc` writes different C for the
  self-hosted programs and for `std`'s bodies where they used `as_view`; the perf guide and the bench, run from the
  final source of Update C, decide whether it costs anything.

## Verification

- `COBALTC_CHECK_ALL=1 coby --check` of `std` and of the three self-hosted programs with the attributes in
  force: no use left; `impl/selfhost/build.sh` builds them.
- Every new case under `coby`, and `cobc --run` with GCC and Clang; `conf_coverage`.
- The proposal's other §9 gates run once, from the final source of `CHG-0405`'s Update.
