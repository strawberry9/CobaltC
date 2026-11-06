# D-0135 — A `String` a call returns, viewed as an argument

Status: ACCEPTED (2026-10-03, the owner: "proceed with resolving that new friction using your recommendation")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 3, semantic consistency)
Depends on: D-0053 (`StringView`), D-0073, D-0084, D-0103 (temporaries borrowed or sliced as arguments), D-0134 (text a function only reads is a `StringView`)
Affects: `spec/21` §2h `[View-Form]`, `spec/09` `[Ref-Form-Temporary-Part-Argument]`, `spec/conformance.md`, the guide, both implementations

## Problem

D-0134 made every path and every needle a `StringView`. A path that a
program builds with a call used to be passed as `&build_path(…)`
(D-0073); it now has to be viewed, `&build_path(…)[0..$]`, and that was
rejected (`diag.borrow-of-non-place`) even as an argument, so it needed
a binding first:

    String p = sprintf("%s/%s", dir, name);
    match (read_file(&p[0..$])) { … }

D-0103 already lets a call's result be *sliced* as an argument
(`sum(&make()[0..2])`) and a field of one be borrowed
(`String::len(&person().name)`), but `[View-Form]` (D-0053), which
`&s[lo .. hi]` on a `String` is, still required `s` to be a place. The
two forms with the same syntax disagreed, and D-0134 made the
disagreement meet every program that opens a computed path.

## Candidate mechanisms

1. **Extend D-0103 to `[View-Form]`.** As an argument of a call whose
   result is not a reference, a `String` that is a temporary, or part of
   one, may be viewed; it lives to the statement's end. Selected.
2. **Restore `ref<String, shared>` overloads for paths.** Two signatures
   for one intent, which D-0134 removed on purpose.
3. **Keep the rule and teach binding first.**

## Selected design

`[View-Form-Temporary-Argument]` (`spec/21` §2h): `&s[lo .. hi]` as an
argument, `s` a `String` that is a call's result or part of a
temporary, is formed as for a place; the `String` is a temporary of the
statement and ends after the call. Outside an argument the form is
still rejected. A view that a callee returns from such an argument and
the program keeps past the statement
(`StringView w = StringView::trim(&f()[0..$]);`) is
`diag.destroy-while-aliased` (dynamic) when the `String` ends, as a
kept slice of a temporary already is under D-0103.

## Compatibility impact

Additive: programs that were rejected now run.

## Revisit conditions

None.
