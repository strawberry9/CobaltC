# CHG-0405 — `String::from_str` and the fourteen `String::` text wrappers removed: tombstones (D-0305, D-0306)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-02)
Implements: D-0305 and D-0306 (their removal step, `spec/STABILITY.md` §3a), after the edition that published
`CHG-0404` (2026.110202)
Affects: `impl/std/text.cb`; `spec/21` 4.76.0 (§2h's rows, a table of `std::text`'s retired names, the listing);
`spec/conformance.md` 3.291.0; both toolchains' front ends (the call a literal's `String` and `sprintf`'s result are
written as) and back ends (the natives that read it; `String::as_view`'s native body; a whole-text view argument,
item 7); `impl/tools/factcheck.py`, `cobdoc.py`, `std_coverage.py`;
the guide's text and its std reference; `impl/selfhost/front/prelude.cb`

## What changed

1. `String::from_str` and `String::as_view`, `find`, `starts_with`, `ends_with`, `trim`, `trim_start`, `trim_end`,
   `split`, `contains`, `replace`, `strip_prefix`, `strip_suffix`, `lines`, `split_whitespace` are tombstones at the
   end of `impl/std/text.cb`, under `// Retired (D-0305, D-0306)`, each `[[removed("…")]]` with the message its
   `[[deprecated]]` carried; their bodies and `unittest`s are gone. A use of one is `diag.removed-use`, with or
   without `--allow-deprecated`, and the name cannot be declared again.
2. **The literal's `String`.** `[Str-Literal-String]` and `[Sprintf]` give the `String` of the bytes,
   `String::from_view(StringView::of(…))`. The front ends realize it as a call of `std::String::copy_str`, a function
   of `std` that is not exported (so no program can name it: `diag.name-not-visible`), whose body is the one
   `from_str` had; `views.rs`/`views.cb` (D-0149, D-0155, D-0162) and the `sprintf` rewrite in `modres.rs`/`modres.cb`
   write it, and every native of `cobc` and `cbc` that read `std::String::from_str` (the copy of known length, its
   `_raw` form, the objectless binding, the fresh key and element, D-0200's compile-time discharge) reads
   `std::String::copy_str`, the natives otherwise unchanged.
3. `String::as_view`'s native body (`lower_as_view` in `cobc`, its port in `cbc`) is gone: no program can reach it.
4. `conf.std-from-str-removed` and `conf.std-string-wrappers-removed` (✗ `diag.removed-use`) replace the two
   `-deprecated` and the two `-allowed` cases, whose files are retired with the names they used.
5. `factcheck.py` reads `String::copy_str` where it read `String::from_str`. The std reference shows a tombstone
   with its message only.
6. Found by `impl/selfhost/compare.sh`: `cobfmt-cb --width 40` on `front/typecheck.cb` took 125–130 s, past the
   script's 120-second limit, before Update 159 already (the last run before it compared only the plain and
   no-wrap forms). Half of it was the self-hosted lexer's `is_type_name`, which read `TYPE_NAMES[i]` from the
   constant array 24 times for every identifier, and `cobc` copies a constant array whole for each element read. It
   checks the length and the first byte first and copies the array once per call: 82 s, the output the same. The
   copy of a constant array per element read is left for a record of its own.
7. Found by `cobc`'s `tracking` test (`append_and_as_view_mint_nothing`, whose `String::as_view(&text)` Update
   160 wrote `&text`): a whole-text view of a `String` place handed to a pure view parameter, D-0302's `&s[0..$]`
   and `&(*r)[0..$]`, was lowered as a slice, its borrow minted and its path stored (`absorb`, `borrow`, `new`,
   `store_ref` per call), where `String::as_view(&s)` made the view's checks only (`borrow_unminted`,
   `borrow_range_unminted`). `cobc`'s `whole_text_view_arg` and its port in `cbc` take such a view as the
   argument `as_view` was given: the call's unminted view argument and `inert_view_arg` accept it. D-0200's account
   of what a function does with a reference parameter (`reader_uses`) is unchanged (`CHG-0404`).
8. Found by the gate's `records` unit (D-0307): `cobdoc.py --check` and `std_coverage.py` counted the tombstones as
   exported items without a summary and without a case. A tombstone is neither documented nor callable; both
   tools skip it.

## Not as the proposal wrote it

- §6.4 had the front ends write `std::String::from_view(std::StringView::of(L))` itself and `cobc`'s natives match
  that call. Written so, every literal's `String` would be a `from_view` of a view: a different call and argument,
  so different C at every literal and every `sprintf` in every program, and each of the natives keyed to the copy
  of a `str` would need a second shape to recognize, in both compilers. A private function with `from_str`'s body
  keeps one shape and the natives as they were, and is the same `String` the rule names; no program can write it, so it is not a
  second spelling.
- `CHG-0403`'s Update kept the internal name `std::String::from_str`; it changes here, once, with the tombstone.

## Verification

- `std` and the three self-hosted programs check with every tombstone in force; the new cases under `coby` and
  `cobc` with GCC and Clang; a literal `String`, a pushed literal and `sprintf` give the same output under `coby`
  and under `cobc` with GCC and Clang; `String::copy_str` named by a program is `diag.name-not-visible`.
- The proposal's §9 gates, run once from this Update's source: their run and the evidence tier are in
  `spec/MANIFEST.md`. The battery was not run for this Update (the owner's instruction).
