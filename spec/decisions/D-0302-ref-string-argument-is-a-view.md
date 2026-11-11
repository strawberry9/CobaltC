# D-0302 — `&s` where a `StringView` is declared is the view of `s`

Status: ACCEPTED (2026-11-01, the owner: option 2 as proposed); implemented by `CHG-0400`
Kind: Decision (`spec/02-schema.md`); a typing rule
Governed by: `CobaltC_Master_Instructions.md` §1, §8 (item 3, semantic consistency), §9, §10
Depends on: D-0006 (no implicit conversion), D-0053 (`StringView`), D-0073, D-0103 (temporaries borrowed as
arguments), D-0113 (`String::as_view`), D-0133 (a view-holding binding ends at its last use), D-0134 (text a
function only reads is a `StringView`; `[Str-Literal-View]`), D-0135 (`[View-Form-Temporary-Argument]`), D-0149
(`[Str-Literal-String]`), D-0218, D-0278, D-0281
Affects: `spec/21` §2h (`rule.stdlib.stringview`: a `[Ref-String-View]` block beside `[View-Form]`); `spec/12`
`rule.type.expected` (the paragraph that lists `[Str-Literal-View]`'s positions); `spec/22` §5 (the "implicit
conversions (D-0006)" entry, qualified); `spec/conformance.md`; the shared checker (`impl/src/views.rs`, which
holds the literal rules, and the argument check that today prints "view the whole text: `&s[0..$]`" at
`impl/src/typecheck.rs`); the guide (`content_c.py`) and the book; `impl/selfhost` at its next port (D-0278)

## Problem

D-0134 made every text a function only reads a `StringView`, and the literal case was closed at once: a
`str-literal` in a `StringView` position is that view (`[Str-Literal-View]`), so `request_fn(g, "main")`
needs nothing. The non-literal case was left to the writer: a `String` is viewed by `String::as_view(&s)`
or `&s[0..$]`, a `str` value by `StringView::of(t)`. Both tools reject the plain borrow and tell the writer the
answer:

    error[diag.type-mismatch]: argument 1 (`s`) is `ref<String, shared>`, but the parameter is `StringView`;
    view the whole text: `&owned[0..$]`

Measured on 2026-11-01 over the self-hosted toolchain (`impl/selfhost/front`, `back`, `cbc`; 113,379 lines):

| form | sites |
|---|---|
| `String::as_view(&…)` as a call's argument | 1,282 |
| of which `String::as_view(&sprintf(…))`, the C emitter's commonest line | 653 |
| `String::as_view(&…)` as a local's initializer | 16 |
| `StringView::of(t)` on a `str` value (a parameter or a constant; literals excluded) | 149 |
| `StringView::of("…")` on a literal: already a view by D-0134, the port's own habit | 197, not counted |

One line in 78 of the compiler wraps an argument whose only possible meaning the checker already knows and
prints. `impl/std` itself has 52 such wrappers. The Rust the port reproduces passes `&s` to a `&str` parameter
(deref coercion) and `format!(…)` results the same way.

## Candidate mechanisms

1. **Leave it; teach `&s[0..$]`.** Six characters instead of `String::as_view(&s)`'s eighteen. Consistent
   with D-0006 as read strictly: nothing is ever accepted at a parameter of another type. The diagnostic
   already teaches it. Costs the wrapper at every site, and `&sprintf(…)[0..$]` reads worse than what it
   replaces.

2. **`&e` of type `ref<String, shared>`, where the declared type is `StringView`, is `&e[0..$]`; a `str`
   value there is `StringView::of(t)`.** The borrow stays written; what the rule supplies is the length,
   a fact of `s`. Positions: those of `[Str-Literal-View]` (D-0134, D-0225): an argument whose parameter is
   declared `StringView`, a local declared `StringView`, a struct literal's field declared `StringView`, a
   variant constructor's payload once its type is fixed. **Selected.**

3. **A `String` by name (`f(s)`) becomes a view.** Hides a borrow in a call that looks like a move, against
   the one convention every call in the language keeps (`Vec::push(&mut v, x)`: the borrow is written).
   Rejected.

4. **`ref<String, shared>` overloads of every text-reading function.** Two signatures per intent; D-0134
   removed them on purpose, and CobaltC has no overloading.

5. **A shorter view spelling (`&s[..]`).** Saves four characters and adds a form; the wrapper remains.

## Selected design

`spec/21` §2h, beside `[View-Form]` and `[View-Form-Temporary-Argument]`:

    [Ref-String-View]   (D-0302)
        e an expression of type ref<String, shared> written `&s` (s a place, through a reference too, or
        a temporary admitted by [Ref-Form-Temporary-Part-Argument]), or a binding of that type, in a
        position whose declared type is StringView ([Str-Literal-View]'s positions)
        ────────────────────────────────────────────
        e there  ≡  &s[0..$]       -- [View-Form]; for a temporary, [View-Form-Temporary-Argument]
                                   -- (so a temporary is admitted as an argument only, as today)
        a binding r : ref<String, shared> there  ≡  &(*r)[0..$]

    [Str-Value-View]   (D-0302)
        t an expression of type str in a position whose declared type is StringView
        ────────────────────────────────────────────
        t there  ≡  StringView::of(t)

Both are stated as the form the writer writes today, so every later rule sees that form: the borrow is checked
as `[Ref-Form]` checks it (clash, D-0218's shared reach, D-0103's and D-0135's temporaries), the view borrows
`s` as a view does and ends at its last use (D-0133), `rule.temporal.elision` counts it as the borrowed
argument it is, and D-0281's tiers and the certificate (D-0299) are those of the written form. Not admitted:
`&mut s` (a view is shared; writing `&mut` for a read misstates the access, `diag.type-mismatch` naming
`&s`), a `String` by value (option 3), a parameter whose type is a type parameter fixed as `StringView` (as
for literals, D-0134: the argument then fixes the parameter), and `ref<str, shared>` (rare; `*r` is a `str`
and `[Str-Value-View]` takes it).

**On D-0006.** D-0006 rejects implicit conversion and subtyping between value types: nothing changes
representation, widens, or stands in for another type here. A `StringView` is the shared borrow of `s` together
with `s`'s length (`[View-Form]`: `&s[0..$] ≡ String::view(&s, 0, $)`), and `&s` is that borrow; the rule
reads the length off the place the writer already named. It is, all the same, the first rule under which an
expression that has a type (`ref<String, shared>`) is accepted at a declared type that is not it;
`[Str-Literal-View]` escaped that by typing an untyped literal. If the owner reads D-0006 as forbidding this
too, option 1 is the consistent answer and the record should be closed as DECLINED with that reason; the
measured cost is then the price of the principle, paid most heavily by programs that build text to pass on.

Implementation: in the shared checker where `[Str-Literal-View]` is applied (`impl/src/views.rs`) and where
the mismatch is reported today (`impl/src/typecheck.rs`, "view the whole text"): the argument or initializer
is rewritten to the `[View-Form]` expression before it is typed, so `coby`, `cobc`, the model and the proofs
see only `&s[0..$]` / `StringView::of(t)`. `cobc`'s lowering is unchanged; D-0199's string natives and
fusions see the same expressions. `impl/std`'s 52 wrappers may be dropped in the same change or left; the
generated C is identical either way. `impl/selfhost` follows at its next port (D-0278). The guide's chapter on
text (`content_c.py`, 13 `as_view` mentions) and the book say `&s` where they say `String::as_view(&s)` for an
argument, and keep `String::as_view` as the function it is.

Conformance (`spec/conformance.md`): `&s` of a local, of a field through a reference, of a call's result
(`read_file(&sprintf(…))`), a `ref<String, shared>` binding passed on, a `str` parameter passed on; `&mut s`
rejected; a `String` by value rejected (the message unchanged); a view kept past its temporary's statement
`diag.destroy-while-aliased` as D-0135; the borrow still clashes with a write while the view lives.

## Trade-offs

- **For:** 1,447 wrappers gone from the compiler alone (one line in 78), the C emitter's `emit(x,
  &sprintf(…))` readable; the borrow stays written, so the alias rules the reader applies to `&s` are the
  ones that hold; additive; no lowering, cost-model or proof work beyond the `≡`.
- **Against:** the first position-directed acceptance of a typed expression at another declared type (see
  the D-0006 paragraph); two rules where D-0134 had one; a reader of a signature must know `StringView`
  accepts `&s`, as they already know it accepts `"…"`.
- **Doctrine (§9):** no new type, token or function; two typing clauses.

## Compatibility impact

Additive: every admitted form was `diag.type-mismatch`. `String::as_view` and `StringView::of` stay, with
the same meaning.

## Revisit conditions

If the owner declines on D-0006: close with the reason and record `&s[0..$]` as the taught form in the guide's
text chapter. If accepted and `StringView` initializers from `&mut`-held text turn out common, a later record
may admit `&mut s` as the shared view it would have to be.

## Implementation notes (CHG-0400)

- The checker admits the forms where it would otherwise report the mismatch: an argument whose parameter
  is declared `StringView`, a local's written type, a struct literal's field, and a variant's payload once
  the expected type has fixed it as `StringView` (as `[Str-Literal-View]` does since D-0225, so
  `Option<StringView> o = Some(&s);` and `Some(t)` are admitted). It records a rewrite (D-0164): `&e` becomes
  `&e[0..$]`, a binding or field `r` of type `ref<String, shared>` becomes `&(*r)[0..$]`, a `str` value `t`
  becomes `StringView::of(t)`; the program is checked again, so both tools, the model and the certificate
  see the written form.
- Two cases that were rejections under D-0134 are now admitted, and became positive cases:
  `conf.view-of-str-binding` (`StringView v = t;`) and `conf.str-binding-view-in-variant` (`Some(t)`).
- `&mut s` where a view is declared now says to pass `&s`.
- `impl/std`'s own wrappers are left as written. The C is the same either way, and a `std` function that a
  tool calls by name after the program is checked (`check_more`: a destructor, `printf`'s helpers) is
  checked where no rewrite can be recorded, so there the new forms are rejected; `std` keeps the form
  spelled out. The same holds for D-0301's `*x` through an exclusive reference or by value.
