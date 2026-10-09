# D-0020 — What a String Literal Denotes

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §7, §9, §11, §12, §18
Depends on: rule.type.is-resource, rule.temporal.elision, inv.string.utf8-validity, D-0003, D-0006, D-0011, CHG-0022, `spec/21-standard-library-semantics.md`, `spec/22-surface-syntax.md`
Affects: type.str, rule.stdlib.str, inv.str.utf8-validity, feat.str-literal

## Problem

`spec/22` 2.8.1 §5 listed "string/char literals" as a deliberate
absence. Text was spelled as a `u8` array literal and emitted through
`write` inside `unsafe` (`ex.extern-write`). The human owner asked
that the absence be reconsidered: a byte array is the wrong mental
model for a human reading `"hello"`, and forcing `unsafe` into every
program that prints is a cost with no safety benefit. The question is
not whether to add a literal but *what a literal denotes* — its type
fixes its ownership, its validity story, and how it reaches `write`.

## Constraints

- §9: smallest sufficient mechanism. No new `Σ` component unless a
  candidate genuinely needs one.
- D-0006: no implicit conversion. Whatever a literal is, it becomes
  anything else only through a named function.
- D-0003: a resource carries a destroy obligation and single-holder
  authority. A constant that is a resource is tracked by
  `rule.resauth.*` and `rule.control.flow-analysis` at every use.
- `CHG-0022` (D-0011's boundary): a function with zero reference
  parameters may not return a reference —
  `[Call-Multi-Ref-Return-Rejected]` — and the "reference into static
  data needs no caller-side check" narrowing was rejected permanently.
  Any candidate in which a literal *is* a `ref<τ, shared>` reopens
  that decision.
- `inv.string.utf8-validity`: a `String` is well-formed UTF-8; today
  the only establishment is `from_utf8`'s validator, a
  `[Trust-Transition]`. A literal must not create a second, weaker
  establishment.
- `term.extent` is spatial (an address set). A literal's lifetime
  story must not be phrased as an "extent".

## Candidate mechanisms

1. **Character literals only** (`'H'` : `u8`). Arrays become
   `['H', 'e', …]`. Minimal, but still a byte array, still
   hand-counted, still no `String`. Rejected: does not address the
   problem.
2. **`"…"` as sugar for `array<u8, N>`.** Pure `[Array-Construct]`;
   nothing downstream changes. Rejected as the *text* literal — it is
   bytes at the type level, and reading `"hi"` as a `u8` array is
   the very confusion the owner named. Retained as the *byte*
   literal, spelled `b"…"`, for the FFI/`write` path where bytes are
   the honest type.
3. **`"…"` as `Vec<u8>`.** Matches `from_utf8`'s parameter, so
   `String::from_utf8("hi")?` reads well. Rejected: still bytes, now
   also a resource and an allocation per evaluation.
4. **`"…"` as an owned `String`, validated lexically.** If the
   literal grammar admits no byte escape, the bytes are UTF-8 by
   construction and the invariant is established statically —
   sound. Rejected on D-0003 grounds: `String` is a resource by
   derivation (it holds a `Vec<u8>`), so every `"hi"` would be
   allocated, moved, destroyed, subject to `[Store-Binding-Place-
   Transfer]`, stale-binding facts, and use-after-move for what is
   semantically a constant; `"hi"` inside a loop allocates each
   iteration. A constant should not be a resource.
5. **An untyped literal typed by `rule.type.expected`**, as integer
   literals are: `"hi"` becomes `array<u8,N>`, `Vec<u8>`, or
   `String` from context. The most CobaltC-shaped sugar and not an
   implicit conversion (literal typing already exists, D-0013). Rejected
   for the same reason as 2–4 in combination: each target type it
   can take is either bytes or a resource; with no expected type it
   needs a default, and any default is one of 2–4.
6. **A copyable, non-resource text value type `str`.** Its domain is
   the set of well-formed UTF-8 byte sequences; a `str` is a value in
   `term.value`'s sense — an element of a domain, independent of
   storage — exactly as an integer is. `is-resource(str) = false`, so
   it copies by `[Read]`, is never moved or destroyed, and no
   `Σ.objects` entry, access path, or temporal check is involved. It
   is *not* a `ref`: no target, no mode, no `held-by` — so
   `[Call-Multi-Ref-Return-Rejected]` and `CHG-0022` are untouched.
   Validity is by lexical construction (`inv.str.utf8-validity`);
   `String::from_str` carries it into `inv.string.utf8-validity`
   with no validator. Operations: `str_len`, `str_byte`, `str_ptr`
   (safe, like `rawptr_of`), `==`/`!=`, `String::from_str`, and an
   ordinary prelude `print` holding the one `unsafe`. Representation
   is an address/length pair (`[Sizeof-Str]`, `[Repr-Str]`, both
   `impl-defined`). **Selected.**
7. **No literal; tooling renders byte arrays as text.** Rejected: it
   concedes the language is unreadable and fixes the editor.

## Selected design

Candidate 6, plus candidate 2 as `b"…"`, plus `print`:

- `type.str` and `rule.stdlib.str` (`spec/21` §2a): `[Str-Literal]`,
  `[Str-Len]`, `[Str-Byte]`/`[Str-Byte-Out-Of-Bounds]` (reusing
  `diag.index-out-of-bounds`, `checked` like `[Index-Checked]`),
  `[Str-Ptr]` (safe; widens `state.storage` outside every object's
  extent, the shape `[Rawptr-Of]` already has), and `print`.
- `String::from_str` (`spec/21` §2), a second `String` constructor.
- `inv.str.utf8-validity` (`spec/03`), `discharge: static` by the
  grammar; `inv.string.utf8-validity`'s establishment names both
  constructors.
- `rule.type.is-resource`: `str` is plain. `[Eq-Str]` bytewise;
  `[T-Cmp]` already denies the orderings (`str` is not numeric).
  `[T-Lit-Str]`: a `str-literal` is `str` and a `byte-literal` is
  `array<u8, N>` regardless of expected type.
- `spec/06`: `⟪b0..b_{n-1}⟫` in the `Value` grammar; `[Sizeof-Str]`,
  `[Repr-Str]`.
- `spec/22`: `str-literal` with escapes `\n \r \t \0 \\ \" \u{…}`
  (no `\x`, so validity is by construction); `byte-literal` with
  `\x` and ASCII source only, non-empty; `str` reserved as a
  `type-name`; a literal that is not closed on its line is
  ill-formed.
- No character literal, no `str` indexing syntax, no slicing, no
  borrowed view — see Revisit conditions.

## Rejected alternatives

1, 3, 4, 5, 7 as above; 2 as the text literal (kept as the byte
literal). Also rejected within 6: making `print` fallible (return
`isize` or `Result`) — `write`'s claim is an unchecked trust value
and a program that only prints should not have to handle it; and
interning/identity guarantees for equal literals — a `str` is a
value, so two evaluations of `"a"` have no identity to compare, and
which address `str_ptr` yields is left implementation-defined.

## Semantic rationale

The whole decision is the observation that a string constant should
be a *value*: copyable, unowned, undestroyed, exactly as `5` is.
Every candidate that made it a `Vec`, a `String`, or a reference
imported ownership or lifetime machinery that a constant does not
need and that the existing decisions (D-0003, CHG-0022) make
expensive or closed. Candidate 6 adds no `Σ` component and no
invariant that any rule must preserve — `inv.str.utf8-validity` has
no preservation obligation because nothing writes into a `str`.

## Usability

`fn main() { print("Hello, CobaltC!\n"); }` is a complete program with
no `unsafe`, no pointer, and no byte. A `str` can be passed, stored
in a struct or `Vec<str>`, compared, and sent to a thread without any
ownership annotation; the one thing a human must learn is that it is
not a `String` and that `String::from_str` is the (explicit,
allocating) bridge.

## Explainability

Every failure has an existing name: an ill-formed literal does not
parse; `str_byte` out of range is `diag.index-out-of-bounds`; `<` on
`str` is `diag.type-mismatch` (`[T-Cmp]`); `extern fn f(str)` is
`diag.extern-non-ffi-type`. No new diagnostic was needed.

## Implementation-feasibility

A compiler emits the literal's bytes into read-only data and passes
an (address, length) pair by value; `str_ptr` is the address. An
interpreter interns each distinct byte sequence on first `str_ptr` or
arena encoding. `String::from_str` is real CobaltC over `str_len`/
`str_byte`.

## Compatibility impact

Additive. `str` becomes a reserved `type-name`; no program in the
corpus (`spec/`, `impl/conformance`, `impl/cobaltc_examples`, the
prelude) used it as an identifier. `spec/22` §5's absence entry is
narrowed, not contradicted: character literals remain absent.

## Prior-art status

Rust's `&'static str` is the same value — address, length, `Copy`,
immutable, UTF-8 by construction — reached through a lifetime system
CobaltC does not have; C's string literal is an array with static
storage duration, which is candidate 2. The derivation here is from
`term.value` and D-0003, not from either precedent.

## Invariant traceability

Introduces `inv.str.utf8-validity`; extends `inv.string.utf8-validity`'s
establishment. Relies on `inv.spatial-validity` only through
`[Str-Ptr]`'s side-condition (the bytes lie outside every live
object's extent), the same reliance `[Rawptr-Of]` has.

## Revisit conditions

- A demonstrated need to index or slice a `str` in syntax (`s[i]`,
  `s[a..b]`): expected resolution is a `[T-Index]` case yielding a
  `u8` *value* (not a place) and a slicing intrinsic, never a
  reference into the bytes.
- A demonstrated need for a borrowed text view over a `String`'s
  bytes: a separate decision, since it would be a `ref` and meet
  `CHG-0022`'s boundary.
- A demonstrated need for character literals: `b"a"[0]` covers the
  byte case; a scalar-value literal would need a `char` type first.
- A demonstrated need for a fallible `print` (short writes observable).
- `String::from_str`'s copy proving costly in a program that only
  needs to read text: the answer is more `str` operations, not a
  `String` literal.
