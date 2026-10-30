# D-0149 — A `str` literal where a `String` is declared is that `String`

Status: ACCEPTED (2026-10-04, the owner: "should CobaltC allow: String s = \"my string\"; rather than the more verbose: String s = String::from_str(\"my string\")", then "accept and implement D-0149, the narrow version")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §10 (every rule must justify its cognitive cost)
Depends on: D-0006 (no implicit conversion of values), D-0020 (`str` literals), D-0037 and D-0079 (a literal is typed by its context), D-0113 (which declined this once), D-0134 (`[Str-Literal-View]`)
Affects: `spec/21` §2a (4.14.0), `spec/12` `rule.type.expected` (1.20.0), `spec/conformance.md` (3.139.0), the guide §21, `impl/src/views.rs` (the shared pass, both tools); `CHG-0177`

## Problem

Owned text is made from a literal by `String::from_str("…")`: 181 times
in the conformance cases, 62 in the guide, 15 in the showcases. Most of
these are initializers and struct-literal fields where the declared type
already says `String` on the same line, so the constructor's name adds
nothing a reader needs. D-0113 declined to let the literal be the
`String`, calling it an implicit conversion and the one place a literal
would allocate. D-0134 then introduced exactly this shape for the other
text type: a literal where a `StringView` is declared is the view
(`[Str-Literal-View]`), as typing rather than conversion, and the
objection of principle fell: a literal has no type until its position
gives it one, as `u64 x = 1024 * 1024;` shows (D-0037).

## Candidate mechanisms

1. **Keep `String::from_str` everywhere.** Explicit about the
   allocation; the most frequent ceremony in real programs. Rejected.
2. **Type the literal by its declared position, as `[Str-Literal-View]`
   does**: a parameter declared `String`, a local declared `String`, a
   struct literal's field declared `String`, and the result of a
   function declared `: String`. Selected.
3. **Also for a type parameter solved as `String`** (`Vec::push(&mut
   names, "x")` for a `Vec<String>`). Needs the solved type, which the
   literal pass does not have and `[Str-Literal-View]` does not use
   either. Not adopted; a revisit condition.
4. **Also for a `str` binding** (`String s = t;`). A conversion of a
   value, D-0006. Rejected.

## Selected design

`spec/21` §2a `[Str-Literal-String]`, beside `[Str-Literal-View]`: a
`str` literal (or a `str` constant's use, which is that literal) in a
position whose declared type is `String` is `String::from_str(L)`. The
positions are the view rule's three, and one more that only an owned
value needs: what a function declared `: String` gives, `return L;` or
`L` as its body's tail expression (through `if` and `match` arms), since
a function returns owned text and never a view of a literal. `auto s =
"x";` stays a `str`; `String s = t;` with `t : str` stays
`diag.type-mismatch`; a parameter of a type-parameter type is not a
position. No program accepted before changes meaning: every newly
accepted program was a type error.

**The cost, accepted knowingly:** this is the one place a literal
allocates, and `String::from_str` can fault on allocation failure
(`diag.alloc-failure`) where only a literal is written. It is bounded to
positions where the word `String` is written in the declaration the
literal fills, which is where C++ programmers already expect
`std::string s = "x";` to allocate. The guide says it in one sentence.

**Realization:** `impl/src/views.rs`, the shared pass that already
rewrites view literals after `modres` and `consts`, now carries two
kinds (`StringView` → `StringView::of`, `String` → `String::from_str`)
and, for functions declared `: String`, walks `return` expressions (not
into closures, whose `return` is their own) and the body's tail through
`if` and `match`. Both tools share it; no change to the checker, the
interpreter or the compiler.

## Compatibility impact

Additive: programs that were type errors are accepted; no accepted
program changes meaning. `String::from_str` remains, and the existing
calls need not change.

## Revisit conditions

- A type parameter solved as `String` (`Vec::push(&mut names, "x")`):
  adopted by D-0162.
- A closure declared `: String` returning a literal, if programs ask.
