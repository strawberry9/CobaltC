# CHG-0452 — Messages that give the written form; the appendix's missing lines; the cold drafts

Status: implemented (2026-11-14), D-0296; Update 189
Affects: `impl/src/typecheck.rs` (`check_program`'s `main`, `?` on a `Result`, `[T-Index]`, `*` on a `Box`,
`conversion_hint`, the qualified-name hint, `check_field`, `check_struct_lit`), `impl/src/parser.rs`
(`parse_postfix`, `parse_closure`, `parse_enum`), `impl/src/hints.rs` (`plain_name`, `fn_name`); their twins in
`impl/selfhost/front/typecheck.cb`, `parser.cb`, `hints.cb`; `impl/tests/foreign_forms/make_cases.py` and
`expected.txt` (seventeen `cw_` cases), `impl/tests/cold_drafts/` (new); `book/src/appendix-a-syntax.md`
(threads, struct patterns, `V(*x)`, variants with fields, `--doc --find`, and "What is not written");
`CARD.md`

## Why

Item 3 of `private/EASE-PLAN-2-2026-11-14.md`. Of the four programs written cold, eight rejections refused
without giving the form to write. Every message here gives the form that writes the type, the borrow and the
function's name; none suggests a form the language does not have.

## What changes

| Written | Now says |
|---|---|
| `fn main() : i32`, a generic `main`, one with parameters | that `main` takes no parameters and returns nothing or a `u8`, what this one has, and `exit(n)`, `arg_count()`, `arg(i)` |
| `?` on a `Result<U, E1>` in a function returning `Result<T, E2>` | both error types and `Result::map_err(r, [](E1 x) : E2 { … })?` written out |
| `v[i]` on a `StringView` | `StringView::as_bytes(v)[i]`, `&v[lo..hi]` |
| `*b` on a `Box` | `Box::get(&b)`, `Box::get_mut(&mut b)`, `Box::into_inner(b)`, `V(*x)` in a pattern |
| `&b` or a `ref<Box<T>>` where a `ref<T>` is declared | `Box::get(&b)` / `Box::get(x)`, and `V(*x)` for a match binder |
| `b.x` on a `Box<P>` whose `P` has an `x` | `Box::get(&b).x`, `Box::get_mut(&mut b).x` |
| `String::from("x")` | a literal is the `String` where one is declared; `String::copy(v)` copies a view |
| `String::f` where `StringView::f` exists | `StringView::f(&s)` (or `(&s, …)`), `&s` being the view |
| `Channel::clone(&ch)` | a `Channel` is not copied: every thread uses it through `&ch` |
| `read_to_string`, `stdin`, `parse_f64`, `atoi`, … | `read_all()`, `read_line()`, `StringView::parse<T>(s)` |
| `String::push`, `String::bytes` | `String::push_ascii`, `StringView::as_bytes(&s)` |
| `&v[..]`, `&v[lo ..]` | a range has both ends, `$` the length; the whole of `v` is `&v` |
| `Bin(u8, i64, i64)` in an `enum` | the variant with fields, `Bin { u8 a; … }` (D-0324) |
| `[](a, b) { … }` | a closure's parameters are written with their types, as a function's are |
| a struct literal missing a field | the fields not given, by name (it said "the types do not match") |
| a `ref<T>` where a `T` is wanted (not a named type) | its value is `*x` |
| an arithmetic operand that is an `Option<T>` or a `ref<T>` beside a `T` | `Option::unwrap(x)`, `Option::unwrap_or(x, d)` or `match`; `*x` (it said "convert one") |
| `Expr::Bin(a, b, c)` of a variant with fields | made by naming the fields (it said "write `Bin(value)`") |
| a capture list that does not match | the list the body needs and the names that differ (it said only "does not match") |

Walking the cold drafts again, taking each fix only from the report (or from `--doc`, where a report pointed
there), gave the last four rows, and an older bug: a range `foreach` inside a closure was
`diag.capture-list-mismatch`, its hidden `$each_range` counted as a name the body used; a `$` name, which no
program can write, is no capture now (both toolchains; `conf.closure-foreach-range`).

Each is a case in `impl/tests/foreign_forms` (`cw_*`), whose first lines the gate's `records` unit compares.
`impl/tests/cold_drafts/` keeps the four drafts as first written, with `run.sh --compare expected.txt` (the
`records` unit too); the report comparison `impl/selfhost/compare_reports.sh` holds cbc's reports of them,
of the foreign forms and of the `_rejected` conformance cases to coby's.

Appendix A (so `CARD.md`) gains the lines the drafts lacked (threads, struct patterns, `V(*x)`, `--doc --find`)
and a section "What is not written": the language's rules that act without being written on the line (reads
through references and guards, the two coercions, a literal's type, binders through a reference, copy or move,
a reference's last use, destruction at a scope's end, closure captures), each with its place in the spec.

## The cold drafts walked

Each draft was taken from its first form to a working program applying only what each report said (or
`--doc`, where a report pointed there): the word count in 6 runs, the expression parser in 8 (with a variant
with fields), the inventory in 6, the prime counter in 7; 27 in all. The first time (`private/EASE-PLAN-2-
2026-11-14.md` §1) took 18, but many of those fixes came from knowing the library rather than from a report,
so the figures do not compare; what this walk shows is that every fix was named by a report. Most runs end at
an earlier phase (a removed name, `main`'s form, a syntax error), which stops the bodies' errors from being
reported (D-0321's phases); within the bodies' phase, a run gave up to six errors at once.

