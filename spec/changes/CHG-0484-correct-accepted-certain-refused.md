# CHG-0484 — Correct programs accepted, certain faults refused: the improvements D-0340 left

Status: implemented (2026-11-25); Update 212
Affects: `spec/14` 1.54.0, `spec/21` 4.92.0, `spec/conformance.md` 3.328.0 (4 rows); `impl/front` (`typecheck.cb`, `loader.cb`, `prelude.cb` generated), `impl/back`
(`gen.cb`, `refs4.cb`, `vec_scan.cb`), `impl/std` (`http.cb`, `http/server.cb`, `builtins.md`, `intrinsics.md`); `impl/model` (`statics.cb`, `machine.cb`:
`ReleaseRef`; `sample/hand-w06.cb`);
`impl/tests/cases/cobc/reports_update212.case`, `reports_update211.case`; `htmlguide/` (`content_intro.py`,
`content_c.py`); D-0341.

## Why

The owner's rule (D-0341 §1): a correct program is accepted, a certainly incorrect one refused, and a doubt is met
by improving the compiler, the run deciding only where no improvement is viable. D-0340 §3 left items the rule
judges; the guide also lacked the book's `#!` scripts.

## What changes

| Item | Was | Now |
|---|---|---|
| 418 | a cursor `ref` reassigned after writing what it read faulted in both builds | `[Ref-Reassign-Releases]`: the loan ends at its last read before the reassignment (`Facts.releases`; the run stores token 0 in the slot; `cobmodel`'s machine lets it go, `ReleaseRef`) |
| 392 | a `Vec<StringView>` (or a struct holding one) written past, then indexed, faulted at run time | refused at the check when an element of the holder is read later (`rep_elem_read_after`, `Use.elem`) |
| 409 | `auto r = if … { Ok(true) } else { Err("…") }` (and through `match`) was `diag.type-annotation-needed` | each branch's open slot filled from the other (`partial_variant_type`, `merge_open`): compiles |
| dead branches | a certain fault in a branch whose condition is a known constant was refused | the branch is checked as unreachable (`const_bool`) |
| 396 | `StringView::trim(&line[0..i])` kept past `line`'s block was refused (any early exit; since 211 without one) | a view's slice passed to a call refers to what the view views (`source_arg_fact`) |
| 417 | an arm binder used only in an `if` condition stayed borrowed through the branch | ended after the condition, in the checker and the run |
| 415 | `Option<StringView> t = c.peeked;` through `ref<Cur, exclusive>` borrowed `*c` exclusively | a copy whose references are all shared views is a shared fact on its field (`shared_views_only`) |
| 437 | a literal beside a generic body's `foreach` index was `i32` | it takes the type the context expects |
| 445 | `foreach (d in &DAYS)` over a `const` array was `diag.borrow-of-non-place` | accepted (`[Ref-Form-Const]`) |
| 400 | `printf("%*d", 8, 42)` was refused (`i32` width) | a literal `*` width is `usize` (and settled for the lowering) |
| 370 | a module file's `#!` first line was refused | emptied before the file is spliced in |
| lowering | `elem_ref_lets` copied the rest of a block for each `let` | judged on the statements naming it (`rest_naming`) |
| 427 | `reason_phrase` empty for 16 RFC 9110 codes | each has its phrase |
| 436 | the HTTP server sent no `Date` | `Date` (IMF-fixdate) unless the handler gives one |
| 383, 391, 406 | the guide said other deadlocks were not detected; `--doc spawn` and `--doc printf` lagged | corrected |
| guide | no `#!` scripts | §1 describes them |

## What is left

- 369 (`Vec<u8> b = Vec::from_slice(&[1, 2, 3])`): the borrow of an array literal does not pass the element type
  it is expected to have inward; for a later change.
- The lowering's copies of 100,000-deep types; 411's message; 416, 419 and the other priority 3–5 reports of
  350–451 not listed above.

## Verification

`reports_update212.case` and `reports_update211.case` under GCC and Clang; the conformance suite; one gate-profile
run before the push.
