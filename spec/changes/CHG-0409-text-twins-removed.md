# CHG-0409 — The `_str` lookups and the last text twins removed: tombstones (D-0308)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-11-03)
Implements: D-0308 (its removal step, `spec/STABILITY.md` §3a), after the edition that published `CHG-0408`
(2026.110301)
Affects: `impl/std/text.cb`, `impl/std/collections.cb`; `spec/21` 4.78.0 (§2g's and §2h's rows, the table of
retired names, the listing); `spec/conformance.md` 3.293.0; both toolchains' front ends (the lazy loop sources)
and back ends (the natives keyed to the retired names, `HashMap::find_str`'s native, the fused text loop); the
guide's text; `impl/STATUS.md`; `impl/selfhost/front/prelude.cb`

## What changed

1. `String::len`, `chars`, `char_count`, `code_point`, `parse<T>`, `String::eq_str`, `String::eq_view`,
   `StringView::eq_str`, `String::view`, `StringView::sub`, `HashMap::get_str`, `get_mut_str`, `contains_str`,
   `remove_str`, `HashSet::contains_str` and `remove_str` are tombstones at the end of `impl/std/text.cb` and
   `impl/std/collections.cb`, under `// Retired (D-0308)`, each `[[removed("…")]]` with the message its
   `[[deprecated]]` carried; their bodies are gone. A use of one is `diag.removed-use`, with or without
   `--allow-deprecated`, and the name cannot be declared again. The `unittest`s that were filed under them
   (`String::chars: …`, `String::parse: …`, `HashMap::get_str: …`), which test the spellings that remain, are
   filed under those (`StringView::chars`, `StringView::parse`, `HashMap::get`).
2. `HashMap::find_str`, which only the four `HashMap` lookups called, is deleted, with its native in `cobc`
   (`native_map_find_str`) and its port in `cbc`.
3. The front ends' and back ends' keys for the retired names are gone: the sub-view facts (`typecheck.rs`'s
   `inherited_facts_in`, `cobc`'s `reader_uses` view chain, their ports) name only `String::part` and
   `StringView::part`; `parse<T>`'s check of `T` and its native name only `StringView::parse`; the lazy loop
   source (`modres.rs`, `modres.cb`) and the fused text loop (`fusion_plan`, `lower_fused_text_loop`) only the
   `StringView::` functions, so the fused loop's `String` source (`on_string`) is gone from both compilers.
   The four `Parts::string_*` functions of `std::text`, which only the `String::` loop sources reached (those of
   `String::split`, `lines` and `split_whitespace`, removed by `CHG-0405`, and `String::chars`), are deleted.
4. The repair for indexing a `String` names `&s[lo..hi]` where it named `String::view`.
5. `conf.std-text-twins-removed` (✗ `diag.removed-use`) replaces `conf.std-text-twins-deprecated` and
   `conf.std-text-twins-allowed`, whose files are retired with the names they used.
6. `spec/21`'s rows say **removed**; its table of deprecated names joins the table of retired names; `String::len`'s
   body leaves the listing. The std reference shows each tombstone with its message only.

## Not as the proposal wrote it

- The proposal's `CHG-0409` was the tombstones, `find_str` and one row. The keys of item 3, which `CHG-0408` kept
  so that both spellings were read alike while both were allowed, and the dead `Parts::string_*` functions (item
  3, the three `CHG-0405` left behind with them) go here too: none can be reached once the names are removed.

## Verification

The gate (D-0307), gate profile, on this tree (run `20261103_191124`): every unit green on the first run. The
conformance corpus under `coby` and compiled under gcc and clang; `std`'s 147 unittests; the guide's 413 examples, the
book's 72 and the 90 showcases, identical under `coby` and `cobc` (gcc, clang); `cbc`'s C identical to `cobc`'s for
1184 programs (0 different), its formatter and parser for 438 and 1552; the model's compare, certify, factcheck,
validate_c and replays; the sweep 6416 decided, none decided before and not now; the perf guide's 72 rows and the
bench's 18 `cobc` counts, none more than 3% above the baseline.
