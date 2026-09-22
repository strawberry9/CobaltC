# CobaltC Arithmetic Semantics and Value Representation

Status: normative artifact
Version: 1.10.0
Conforms to: `spec/02-schema.md` (Kind: Type `type.*`; Kind: Rule
`rule.arith.*`)
Governed by: `CobaltC_Master_Instructions.md` §17 (Arithmetic), §23
Realizes: D-0002 (checked integers), D-0019 (representation), D-0025 (negated literals),
D-0026 (type limits), D-0027 (conversion directions)

## Purpose

Defines CobaltC's numeric types and their arithmetic — each type's
mathematical domain, represented domain, conversions, overflow,
division/remainder, shifts, comparisons, literal interpretation — and
(§7) the representation of every value in storage: `sizeof`,
`alignof`, `represent`, `value-at`.

## 1. Integer types

| Entity | Signed | Width `bitwidth(τ)` | Represented domain |
|---|---|---|---|
| `type.i8`, `type.i16`, `type.i32`, `type.i64`, `type.i128` | yes | 8, 16, 32, 64, 128 | `[-2^(N-1), 2^(N-1)-1]` |
| `type.u8`, `type.u16`, `type.u32`, `type.u64`, `type.u128` | no | 8, 16, 32, 64, 128 | `[0, 2^N-1]` |
| `type.isize` | yes | `AddrWidth` | `[-2^(AddrWidth-1), 2^(AddrWidth-1)-1]` |
| `type.usize` | no | `AddrWidth` | `[0, 2^AddrWidth-1]` |

**Status (all):** ACCEPTED. `is-resource = false` for every type in
this artifact (`spec/12` §3). Mathematical domain `ℤ`; represented
domain the finite subset above. `AddrWidth ∈ {16, 32, 64, 128}` is a
**platform parameter**: a conforming implementation commits to one
value and documents it. Every rule whose conclusion depends on
`AddrWidth` carries `outcome: impl-defined` over the induced set
(resolves `spec/AUDIT-2.md` B-25: the parameter itself is not a rule;
the rules that read it are tagged). `Address ≝ [0, 2^AddrWidth)`.

`isize`/`usize` are the types of extents, offsets, and indices
(`inv.spatial-validity`'s consumers). Signed representation is two's
complement (independently justified: it makes `[Reinterpret-Sign]` a
no-op on cells and is the near-universal hardware form; retained per
§7).

## 2. Floating-point types

`type.f32`, `type.f64` (**Status:** ACCEPTED): IEEE 754 binary32 /
binary64. Represented domain: exactly the IEEE value set including
`±Inf` and `NaN`. Exceeding the finite range yields `±Inf` and invalid
operations yield `NaN`, both members of the represented domain, so
§3's checked disposition does not apply. `NaN` breaks reflexivity of
`=` and totality of `<`; a total order is not defined (no consumer).

## 3. Default (checked) integer arithmetic

### `rule.arith.checked`
**Status:** ACCEPTED

    [Arith-Checked]
        v1, v2 : τ    τ an integer type    op ∈ {+, -, *}
        r = v1 ⊕_ℤ v2    r ∈ represented-domain(τ)
        ────────────────────────────────────────────
        ⟨v1 op v2, Σ⟩ → ⟨r, Σ⟩

    [Arith-Checked-Overflow]   disposition: checked
        v1, v2 : τ    r = v1 ⊕_ℤ v2    r ∉ represented-domain(τ)
        ────────────────────────────────────────────
        ⟨v1 op v2, Σ⟩ ↛ diag.arith-overflow
        side-conditions:
            ⟦ r ∈ represented-domain(τ) ⟧ discharge: dynamic
              (discharge: static, disposition: rejected, when v1 and v2
               are literals or compile-time constants — rule.control.flow-analysis §C)

    [Arith-Float]
        v1, v2 : τ ∈ {f32, f64}    op ∈ {+, -, *, /}
        ────────────────────────────────────────────
        ⟨v1 op v2, Σ⟩ → ⟨IEEE-754 result (round-to-nearest-even), Σ⟩

Operands of differing types have no rule: `1: i32 + 1: i64` is
ill-typed (`rule.type.typing`, D-0006 — no implicit conversion).

**Depends on:** inv.arith.range-validity, D-0002, rule.type.typing

### `rule.arith.div`
**Status:** ACCEPTED

    [Div-Checked]
        v1, v2 : τ integer    v2 ≠ 0    ¬(signed(τ) ∧ v1 = min(τ) ∧ v2 = -1)
        r = truncate-toward-zero(v1 /_ℚ v2)
        ────────────────────────────────────────────
        ⟨v1 / v2, Σ⟩ → ⟨r, Σ⟩

    [Rem-Checked]
        premises as [Div-Checked];  r = v1 - truncate-toward-zero(v1 /_ℚ v2) · v2
        ────────────────────────────────────────────
        ⟨v1 % v2, Σ⟩ → ⟨r, Σ⟩

    [Div-By-Zero]   disposition: checked
        v2 = 0
        ────────────────────────────────────────────
        ⟨v1 / v2, Σ⟩ ↛ diag.div-by-zero        (likewise ⟨v1 % v2, Σ⟩)
        side-conditions: ⟦ v2 ≠ 0 ⟧ discharge: dynamic (static where constant)

    [Div-Overflow]   disposition: checked
        signed(τ) ∧ v1 = min(τ) ∧ v2 = -1
        ────────────────────────────────────────────
        ⟨v1 / v2, Σ⟩ ↛ diag.div-overflow        (likewise ⟨v1 % v2, Σ⟩)
        side-conditions: ⟦ ¬(v1 = min(τ) ∧ v2 = -1) ⟧ discharge: dynamic (static where constant)

Remainder follows the truncated-division convention (sign of the
dividend). `min(τ) / -1` is the one division that overflows.

**Depends on:** inv.arith.range-validity, D-0002

### `rule.arith.shift`
**Status:** ACCEPTED

    [Shl-Checked]
        v : τ integer    n : u32    0 ≤ n < bitwidth(τ)
        r = (v · 2^n) mod 2^bitwidth(τ), reinterpreted per signedness(τ)
        ────────────────────────────────────────────
        ⟨v << n, Σ⟩ → ⟨r, Σ⟩

    [Shr-Checked]
        v : τ integer    n : u32    0 ≤ n < bitwidth(τ)
        r = floor(v / 2^n)          -- arithmetic (sign-preserving) for signed τ;
                                       for unsigned τ this coincides with a logical shift
        ────────────────────────────────────────────
        ⟨v >> n, Σ⟩ → ⟨r, Σ⟩

    [Shift-Amount-Invalid]   disposition: checked
        ¬(0 ≤ n < bitwidth(τ))
        ────────────────────────────────────────────
        ⟨v << n, Σ⟩ ↛ diag.shift-amount-out-of-range     (likewise >>)
        side-conditions: ⟦ 0 ≤ n < bitwidth(τ) ⟧ discharge: dynamic (static where constant)

The shift amount is always `u32` (any integer type would do; one is
fixed so the rule is total, D-0006). Left shift discards high bits by
definition — this is not a range-validity violation; a caller wanting
checked multiplication uses `*`.

**Depends on:** inv.arith.range-validity

### `rule.arith.bitwise`
**Status:** ACCEPTED

    [Bit-And] / [Bit-Or] / [Bit-Xor]
        v1, v2 : τ integer
        ────────────────────────────────────────────
        ⟨v1 & v2, Σ⟩ → ⟨bitwise and of the two's-complement cell images, as τ, Σ⟩
        (likewise |, ^)

    [Bit-Not]      v : τ integer      ⟨~v, Σ⟩ → ⟨bitwise complement, as τ, Σ⟩
    [Bool-Not]     v : bool           ⟨!v, Σ⟩ → ⟨¬v, Σ⟩

Total on well-typed operands; no `disposition`. Added because
`spec/21` §3's UTF-8 validation needs them (Master Instructions §24:
a consumer now exists).

**Depends on:** rule.arith.represent

### `rule.arith.cmp`
**Status:** ACCEPTED

    [Cmp-Lt]   v1, v2 : τ (same type, integer or float)
               ⟨v1 < v2, Σ⟩ → ⟨v1 <_ℤ v2, Σ⟩     -- IEEE order for floats: false if either is NaN
    [Cmp-Eq]   v1, v2 : τ           ⟨v1 == v2, Σ⟩ → ⟨v1 =_τ v2, Σ⟩          -- rule.type.eq
    [Cmp-Le]   v1, v2 : τ           ⟨v1 <= v2, Σ⟩ → ⟨(v1 < v2) ∨ (v1 =_τ v2), Σ⟩
    [Cmp-Gt]   v1, v2 : τ           ⟨v1 > v2, Σ⟩  → ⟨v2 < v1, Σ⟩
    [Cmp-Ge]   v1, v2 : τ           ⟨v1 >= v2, Σ⟩ → ⟨(v2 < v1) ∨ (v1 =_τ v2), Σ⟩
    [Cmp-Ne]   v1, v2 : τ           ⟨v1 != v2, Σ⟩ → ⟨¬(v1 =_τ v2), Σ⟩

All results are `type.bool` (`spec/13` §2). `<=`, `>=` are stated
against `<` and `=_τ` directly, not by negating `<`, so IEEE
non-totality is preserved: every comparison with `NaN` except `!=` is
`false`. Cross-type comparison has no rule (ill-typed). `==`/`!=` on
`ref` compare identity (`rule.type.eq` `[Eq-Ref]`).

**Depends on:** rule.type.eq, type.bool

### `rule.arith.neg`
**Status:** ACCEPTED

    [Neg]
        v : τ ∈ {i8..i128, isize}    r = -v    r ∈ represented-domain(τ)
        ────────────────────────────────────────────
        ⟨-v, Σ⟩ → ⟨r, Σ⟩

    [Neg-Overflow]   disposition: checked
        v : τ signed    v = min(τ)
        ────────────────────────────────────────────
        ⟨-v, Σ⟩ ↛ diag.arith-overflow
        side-conditions: ⟦ v ≠ min(τ) ⟧ discharge: dynamic (static where constant)

    [Neg-Float]   v : τ ∈ {f32, f64}    ⟨-v, Σ⟩ → ⟨IEEE sign flip, Σ⟩

Unary minus on an unsigned type is ill-typed. Literals are unsigned
digit strings (`spec/22` §1), so `-1: i32` is a minus applied to
`1: i32`. A minus applied directly to a literal is range-checked as one
value (`[Literal-Negated]`, §6), so `-2147483648` is `min(i32)`; `[Neg]`
and `[Neg-Overflow]` govern every other operand.

**Depends on:** inv.arith.range-validity

## 4. Conversions

### `rule.arith.convert`
**Status:** ACCEPTED

    [Widen]      τ, τ' integer, represented-domain(τ) ⊆ represented-domain(τ')   ⟨widen<τ'>(v : τ), Σ⟩ → ⟨v, Σ⟩
                 (e.g. u8 → i32, i8 → i64; every value is preserved, so no check)
    [Widen-Float]   (τ, τ') ∈ {(f32, f64), (f32, f32), (f64, f64)}         ⟨widen<τ'>(v : τ), Σ⟩ → ⟨v, Σ⟩
                 (D-0051: every f32 value, NaN and the infinities included, is an f64 value)
    [Narrow-Checked]   τ, τ' integer, v ∈ represented-domain(τ')
                                                                          ⟨narrow<τ'>(v : τ), Σ⟩ → ⟨v, Σ⟩
    [Narrow-Overflow]   disposition: checked    v ∉ represented-domain(τ')
                                                                          ⟨narrow<τ'>(v : τ), Σ⟩ ↛ diag.narrowing-overflow
        side-conditions: ⟦ v ∈ represented-domain(τ') ⟧ discharge: dynamic (static where constant)
    [Narrow-Wrapping]  τ, τ' integer                                    ⟨narrow_wrapping<τ'>(v : τ), Σ⟩ → ⟨v mod 2^bitwidth(τ') reinterpreted per τ', Σ⟩
    [Reinterpret-Sign]   τ, τ' same bitwidth, differing signedness        ⟨reinterpret<τ'>(v : τ), Σ⟩ → ⟨the value whose cell image equals v's, as τ', Σ⟩
    [Reinterpret-Float]  one of τ, τ' ∈ {f32, f64}, the other an integer type of the same bitwidth (32 for f32, 64 for f64)
                                                                          ⟨reinterpret<τ'>(v : τ), Σ⟩ → ⟨the value whose cell image equals v's, as τ', Σ⟩
                 (D-0051: a float's IEEE-754 bits as an integer, and back; every bit pattern is some float, NaNs included)
    [Int-To-Float]   τ integer, τ' ∈ {f32,f64}                          ⟨to_float<τ'>(v : τ), Σ⟩ → ⟨nearest τ' value (ties to even), Σ⟩
    [Float-To-Float] τ, τ' ∈ {f32,f64}                                   ⟨to_float<τ'>(v : τ), Σ⟩ → ⟨v converted as IEEE-754 does, Σ⟩
                 (D-0051: the nearest τ' value, ties to even, as [Arith-Float] rounds; beyond τ''s finite range an
                  infinity of v's sign; a NaN stays a NaN; from f32 to f64 exact, and to its own type the same value)
    [Float-To-Int-Checked]   τ ∈ {f32,f64}, τ' integer, v finite, truncate(v) ∈ represented-domain(τ')
                                                                          ⟨to_int<τ'>(v : τ), Σ⟩ → ⟨truncate(v), Σ⟩
    [Float-To-Int-Invalid]   disposition: checked   otherwise            ⟨to_int<τ'>(v : τ), Σ⟩ ↛ diag.narrowing-overflow
    [Usize-Isize]   ⟨reinterpret<isize>(v : usize), Σ⟩ and the converse: as [Reinterpret-Sign]

Every conversion is an explicitly named intrinsic (`spec/21` §0);
CobaltC has no implicit numeric conversion (D-0006). `widen`,
`narrow`, `narrow_wrapping`, `reinterpret`, `to_float`, `to_int` each
take their target type as an explicit type argument.

    [T-Convert]   disposition: rejected
        a conversion whose operand and target types do not meet its rule's type
        premises: the kind of each (an integer where it says `τ integer`, f32 or
        f64 where it says `τ ∈ {f32, f64}`); for `widen`, represented-domain(τ) ⊆
        represented-domain(τ') between integers, or `[Widen-Float]`'s pairs; for
        `reinterpret`, the same bitwidth and differing signedness between integers,
        or `[Reinterpret-Float]`'s float and integer of one width
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

The types are checked at each instantiation (D-0026): a generic body
may pass its `T` to a conversion (`rule.type.kind`), and an
instantiation whose `T` is of the wrong kind is ill-formed.

`narrow` and `narrow_wrapping` take any two integer types (D-0027). A
`narrow` whose source domain is contained in the target's cannot fault,
and is how a program converts between `isize`/`usize` and a fixed-width
type portably: whether `usize → u64` is a widening depends on
`AddrWidth`, so `widen<u64>(n : usize)` is well-formed on some
implementations only, and `narrow<u64>(n)` on all of them.

**Depends on:** inv.arith.range-validity, D-0006, D-0026, D-0027

## 5. Explicit alternative operation families

### `rule.arith.alt`
**Status:** ACCEPTED

    [Wrapping]     ⟨wrapping_op(v1, v2), Σ⟩ → ⟨(v1 ⊕_ℤ v2) mod 2^bitwidth(τ), reinterpreted per τ, Σ⟩
    [Saturating]   ⟨saturating_op(v1, v2), Σ⟩ → ⟨clamp(v1 ⊕_ℤ v2, min(τ), max(τ)), Σ⟩
    [Checked-Ok]   disposition: fallible   r = v1 ⊕_ℤ v2 ∈ represented-domain(τ)
                   ⟨checked_op(v1, v2), Σ⟩ → ⟨Some(r), Σ⟩
    [Checked-None] disposition: fallible   r ∉ represented-domain(τ)  (or v2 = 0 / min,-1 for div)
                   ⟨checked_op(v1, v2), Σ⟩ → ⟨None, Σ⟩

for `op ∈ {add, sub, mul}` and, for `checked_`, also `div`, `rem`.
Result type of the `checked_` family is `Option<τ>` (`spec/21` §0).
`wrapping_`/`saturating_` are total, no `disposition`. These are the
only route to non-checked policies (D-0002).

    [T-Alt]   disposition: rejected
        the two operands of an operation of this family are not of one
        integer type τ
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

An unsuffixed literal operand takes `τ` from the other operand
(`rule.type.expected`). As for the conversions, the types are checked
at each instantiation (D-0026).

**Depends on:** D-0002, rule.agg.enum-construct

## 5a. Type limits

### `rule.arith.limits`
**Status:** ACCEPTED

    [Min-Value]   τ integer, or τ ∈ {f32, f64}
        ────────────────────────────────────────────
        Γ ⊢ min_value<τ>() : τ;   ⟨min_value<τ>(), Σ⟩ → ⟨min(τ), Σ⟩

    [Max-Value]   τ integer, or τ ∈ {f32, f64}
        ────────────────────────────────────────────
        Γ ⊢ max_value<τ>() : τ;   ⟨max_value<τ>(), Σ⟩ → ⟨max(τ), Σ⟩

    [Limits-Not-Number]   disposition: rejected
        τ neither an integer type nor f32 or f64
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    [Limits-Uninferable]   disposition: rejected
        min_value() or max_value() with no type argument
        ────────────────────────────────────────────
        ill-formed; diag.cannot-infer-type-parameter (static)

`min(τ)` and `max(τ)` are the bounds of `represented-domain(τ)` (§1),
so `max_value<usize>()` is `2^AddrWidth - 1` on every implementation.
For a float type (D-0051) `max(τ)` is its largest finite value
(`3.4028235e38` for f32, `1.7976931348623157e308` for f64) and `min(τ)`
its negation; the infinities lie beyond them.
As for the conversions, the type argument is written explicitly, and
it is checked at each instantiation: `max_value<T>()` in a generic
body is well-formed, and an instantiation with a non-number `T` is
`[Limits-Not-Number]`. A call is not a literal, so
`rule.control.flow-analysis` does not refute an operation on it:
`max_value<i32>() + 1` is `diag.arith-overflow (dynamic)` (D-0026).

**Depends on:** inv.arith.range-validity, rule.type.kind, D-0026

## 6. Literal typing

### `rule.arith.literal`
**Status:** ACCEPTED

    [Literal-In-Range]   disposition: rejected
        L a literal with determined type τ    val(L) ∈ represented-domain(τ)
        ────────────────────────────────────────────
        Γ ⊢ L : τ;   ⟨L, Σ⟩ → ⟨val(L), Σ⟩

    [Literal-Out-Of-Range]   disposition: rejected
        val(L) ∉ represented-domain(τ)
        ────────────────────────────────────────────
        ill-formed; diag.literal-out-of-range

    [Literal-Type-Suffix]        L = digits ':' τ                          type of L is τ
    [Literal-Type-From-Context]  L has no suffix; an expected type τ is available at L's
                                 position (rule.type.expected: local-declaration annotation, parameter/return
                                 type, field type, the other operand's determined type, `u32` as a shift
                                 amount, `usize` as an index, a type propagated through if/match/block, or
                                 through an operator whose operands are all literal expressions, D-0037)
                                                                            type of L is τ
    [Literal-Default]            L has no suffix and no expected type      type of L is i32 (integer) / f64 (float)

    [Literal-Negated]   disposition: rejected
        L an integer literal with determined type τ;   `-` applied directly to L
        (whitespace allowed, no parentheses)
        signed(τ)    -val(L) ∈ represented-domain(τ)
        ────────────────────────────────────────────
        Γ ⊢ -L : τ;   ⟨-L, Σ⟩ → ⟨-val(L), Σ⟩
        otherwise: unsigned τ is ill-typed (`[T-Neg]`, diag.type-mismatch);
                   -val(L) ∉ represented-domain(τ) is diag.literal-out-of-range

`[Literal-Negated]` takes the place of `[Literal-In-Range]` and `[Neg]`
for `-L`: the range check is made on the value the expression denotes
(D-0025). It changes no value, since a negated in-range literal has the
same value either way. The only `-L` it accepts that the literal's own
range check would reject is the one where `val(L)` is `max(τ) + 1`,
which denotes `min(τ)`. `L`'s type comes from its suffix, the expected
type or the default, as above, and a suffix belongs to `L`
(`-2147483648: i32`). In `-(2147483648)` the minus is not applied
directly, so the literal is checked alone and is out of range.

A float literal's `val(L)` is the decimal number it spells, exponent
included, rounded to the nearest value of `τ` (ties to even); one that
rounds beyond `τ`'s largest finite magnitude (`1e400`, `1e39: f32`) is
not in `represented-domain(τ)`, so `[Literal-Out-Of-Range]`. A
`byte-char-literal` `b'x'` (`spec/22` §1, D-0033) has type `u8` in
every context, like a suffixed literal, and its value is the byte.

This rule governs numeric literals only. A `str-literal` is typed and
evaluated by `rule.stdlib.str` `[Str-Literal]` (`spec/21` §2a) and a
`byte-literal` by `rule.agg.array-construct` (`spec/16` §3); neither
has a suffix, a default, or an expected-type case.

**Depends on:** inv.arith.range-validity, rule.type.expected, rule.type.typing, D-0012, D-0025, D-0033

## 7. Representation: `sizeof`, `alignof`, `represent`, `value-at`

### `rule.arith.sizeof`
**Status:** ACCEPTED

    [Sizeof-Int]      sizeof(iN) = sizeof(uN) = N/8            alignof = sizeof
    [Sizeof-Float]    sizeof(f32) = 4,  sizeof(f64) = 8         alignof = sizeof
    [Sizeof-Bool]     sizeof(bool) = 1                          alignof = 1
    [Sizeof-Unit]     sizeof(unit) = 0                          alignof = 1
    [Sizeof-Addr]   outcome: impl-defined { 2, 4, 8, 16 }
                      sizeof(isize) = sizeof(usize) = sizeof(ref<τ,m>) = sizeof(rawptr<τ>)
                        = sizeof(fn(...):τ) = AddrWidth/8        alignof = sizeof
    [Sizeof-Handle]   outcome: impl-defined { 2, 4, 8, 16 }
                      sizeof(handle<τ>) = sizeof(guard<τ>) = AddrWidth/8   (spec/19)
    [Sizeof-Str]      outcome: impl-defined { 4, 8, 16, 32 }
                      sizeof(str) = 2 · AddrWidth/8,  alignof(str) = AddrWidth/8   (an address and a length; spec/21 §2a)
    [Sizeof-Mutex]    sizeof(mutex<τ>), alignof(mutex<τ>) = those of the struct { τ inner; usize state; }
                      per [Layout-Struct] (spec/19 §2: one field plus sizeof(usize) of implementation state)
    [Sizeof-Struct]   per rule.agg.layout (spec/16 §1)
    [Sizeof-Array]    sizeof(array<τ,N>) = N · sizeof(τ),  alignof = alignof(τ)
    [Sizeof-Enum]     per rule.agg.layout (spec/16 §3)

**Depends on:** type.i8, type.u8, type.f32, type.f64, type.bool,
type.unit, type.str, type.ref, type.rawptr, rule.agg.layout

### `rule.arith.represent`
**Status:** ACCEPTED

**Cell alphabet.** `datum ::= byte(n) | ref(a) | cont | pad`, with
`n ∈ [0,255]`, `a ∈ AccessPathToken`. `Σ.storage` maps each address to
`uninit` or one datum. `represent(τ, v)` is a sequence of exactly
`sizeof(τ)` datums; `value-at(τ, S, addrs)` is its partial inverse over
the contiguous cells `addrs` (ascending address order), undefined
where the cells are not an image of `represent(τ, ·)` — such a read is
reachable only under `discharge: trusted` (`rule.trust.rawptr`
`[Rawptr-Read]`/`[Reclaim]`), whose asserted precondition excludes it.

    [Repr-Byte-Order]   outcome: impl-defined { little-endian, big-endian }
        BO = the platform's byte order, committed and documented

    [Repr-Int]     represent(τ, v) for integer τ = the bitwidth(τ)/8 bytes of v's two's-complement
                   image, in order BO;  value-at inverts.
    [Repr-Float]   the IEEE 754 interchange encoding, bytes in order BO.
    [Repr-Bool]    represent(bool, false) = [byte(0)],  represent(bool, true) = [byte(1)];
                   value-at(bool, ·) is undefined for any other byte.
    [Repr-Unit]    represent(unit, ()) = [];  value-at(unit, ·, ∅) = ().
    [Repr-Ref]     represent(ref<τ,m>, a) = [ref(a)] ++ [cont]^(AddrWidth/8 − 1);
                   value-at(ref<τ,m>, S, addrs) = a  iff S(min addrs) = ref(a).
    [Repr-Rawptr]  represent(rawptr<τ>, x) = the AddrWidth/8 bytes of address x in order BO.
    [Repr-Fn]      represent(fn(...):τ, p) = an implementation-chosen injective AddrWidth/8-byte
                   image of item path p;  outcome: impl-defined { any injective encoding }.
    [Repr-Struct]  represent(τ_struct, ⟨f1: v1, …, fn: vn⟩) places represent(τi, vi) at
                   off(fi) (rule.agg.layout) and pad elsewhere;  value-at reads each field.
    [Repr-Array]   represent(array<τ,N>, [v0,…]) places represent(τ, vi) at i·sizeof(τ).
    [Repr-Enum]    outcome: impl-defined { 1, 2, 4, 8 }  (discriminant width DW, documented)
                   represent(τ_enum, Vi(v)) = the DW-byte unsigned image of i (variants numbered
                   from 0 in declaration order, BO) at offset 0, represent(τi, v) at offset
                   payload-off (rule.agg.layout), pad elsewhere.
    [Repr-Handle]  outcome: impl-defined { any injective encoding }
                   represent(handle<τ>, ℓ') = an implementation-chosen injective AddrWidth/8-byte image of thread ℓ'
                   (read only by rule.conc.join; a handle is not an FfiType).
    [Repr-Guard]   represent(guard<τ>, a_g) = [ref(a_g)] ++ [cont]^(AddrWidth/8 − 1), as [Repr-Ref].
    [Repr-Mutex]   represent(mutex<τ>, ·) places represent(τ, v) at off(inner) and pad in the state cells
                   ([Sizeof-Mutex]; the state cells' contents are outcome: impl-defined and never read by a rule).
    [Repr-Str]     outcome: impl-defined
                   represent(str, ⟪b0..b_{n-1}⟫) = the AddrWidth/8-byte image of an address p in order BO, then that of n,
                   where the cells [p, p+n) hold byte(b0)..byte(b_{n-1}) and lie outside every live object's extent
                   (spec/21 §2a [Str-Ptr]: the same p that str_ptr yields); value-at reads the n bytes back.

**Byte view.** `bytes(ref(a)) ≝` the AddrWidth/8-byte image of
`min(target(a))` in order `BO`; `bytes(cont) = bytes(pad) =` one
unspecified byte (`outcome: unspecified { 0..255 }`); `bytes(byte(n))
= n`. `rule.trust.rawptr` reads the byte view; `rule.trust.
extern-call` passes it. Consequently a reference passed through FFI
arrives as an address, and an address read back through
`rawptr`/`reclaim` is a claim, not a `ref` (`inv.trust-transition`).

**Values.** `Value ::= integer | float | true | false | () | ⟪b0..b_{n-1}⟫
(a byte sequence, type str) | a (AccessPathToken, type ref) | x
(Address, type rawptr) | p (item path, type fn) | ⟨f1: v1, …⟩ (struct)
| [v0, …] (array) | Vi(v) (enum)`. `refs-in(v)` (`spec/04` §2) is `{a}`
for a `ref` value, the union over components for aggregates, `∅`
otherwise (a `str` value holds no reference).

**FFI/`reclaim` well-definedness.** `rule.trust.rawptr` and
`rule.trust.extern-call` are well-defined
exactly when both sides of the boundary agree on the implementation's
committed `AddrWidth`, `BO`, `DW`, `[Sizeof-*]`, and `rule.agg.layout`
choices; asserting so is part of what the enclosing `unsafe` block
records (`spec/20` §1).

**Depends on:** type.ref, type.rawptr, type.str, rule.agg.layout,
rule.arith.sizeof, D-0019
**Affects:** state.storage

## Deferred

- A total order on floats: not needed by any consumer.
- 128-bit `AddrWidth` targets: admitted by the parameter set, not
  otherwise exercised.

## Change Log

- 1.10.0 — `CHG-0059` (D-0051): float conversions — `[Widen-Float]`
  (`f32` into `f64`), `[Float-To-Float]` (`to_float` from a float),
  `[Reinterpret-Float]` (a float's bits); `min_value`/`max_value` of
  `f32` and `f64`; `[Limits-Not-Integer]` renamed `[Limits-Not-Number]`.

- 1.9.0 — `CHG-0045` (D-0037): `[Literal-Type-From-Context]`
  names the operator-on-literals position of `rule.type.expected`.

- 1.8.0 — `CHG-0042` (D-0033): `rule.arith.literal` states a float
  literal's value (exponent included, rounded to `τ`) and that one
  rounding beyond `τ`'s range is out of range; `b'x'` is a `u8`.

- 1.7.0 — `CHG-0036` (D-0027): `[Narrow-Checked]` and
  `[Narrow-Wrapping]` apply to any two integer types; `[T-Convert]`
  names `[Widen]`'s containment and `[Reinterpret-Sign]`'s width and
  signedness among the premises it checks.

- 1.6.0 — `CHG-0035` (D-0026): `rule.arith.limits` (§5a),
  `min_value<τ>()` and `max_value<τ>()`; `[T-Convert]` and `[T-Alt]`
  state the operand typing the conversions and the alternative
  operations always required, checked at each instantiation.

- 1.5.0 — `CHG-0034` (D-0025): `[Literal-Negated]` added to
  `rule.arith.literal`: a minus applied directly to an integer literal
  is range-checked as one value, so every signed type's minimum can be
  written as a literal. `rule.arith.neg`'s note now points to it.

- 1.4.0 — `CHG-0025` (D-0020): `str` values `⟪b0..b_{n-1}⟫` added to
  the `Value` grammar; `[Sizeof-Str]` and `[Repr-Str]` (an address and
  a length, `outcome: impl-defined`) added; `rule.arith.literal` states
  that it governs numeric literals only.
- 1.3.0 — `CHG-0011`: `[Widen]` applies whenever the source domain is
  contained in the target's (u8 → i32 included); `[Narrow-*]`
  otherwise. Under 1.2.0 `widen<i32>(b : u8)` had no rule and the
  always-safe conversion had to be spelled `narrow` (found deriving
  `ex.e2e-propagate-chain`).
- 1.2.0 — `CHG-0010` (per-case derivation of `spec/conformance.md`):
  `[Narrow-Checked]`/`[Narrow-Wrapping]` no longer require equal
  signedness (`narrow<u8>(x : i32)`, `conf.narrow-overflow`, had no
  rule); `[Literal-Type-From-Context]` names the shift-amount, index,
  and propagated positions `rule.type.expected` now lists;
  `[Sizeof-Mutex]` split from `[Sizeof-Handle]` (a mutex holding an
  `inner` plus `usize` of state cannot be `AddrWidth/8` bytes);
  `[Repr-Handle]`/`[Repr-Guard]`/`[Repr-Mutex]` added — `spec/19`
  wrote `represent(handle<τ>, ℓ')` and `represent(guard<τ>, a_g)`
  with no defining case.
- 1.1.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"):
  `[Sizeof-Handle]` now carries the `outcome: impl-defined` tag §1
  says every `AddrWidth`-dependent rule carries (`[Sizeof-Addr]`
  already did; the value set is the same); two duplicated
  `rule.trust.rawptr` citations in §7 collapsed; one `let` mention in
  §6 re-worded.
- 1.1.0 — `CHG-0002`: two `fn(...)` type-shape illustrations re-spelled
  to `spec/22` 2.0.0 syntax (`:` instead of `->`); missed by `CHG-0001`'s
  sweep since they weren't inside a `let`/`mod`/`extern fn` pattern.
  No rule semantics changed.
- 1.0.0 — Rewritten with rule ids and Status (`spec/AUDIT-2.md`
  B-19); added `[Shr-Checked]`, bitwise operators, float conversions,
  `[Rem-*]` stated rather than "mirrored" (B-22); `[Literal-Type-*]`
  restated against `rule.type.expected`; §7 now defines
  `represent`/`value-at`, the cell alphabet, byte order and
  discriminant width as `outcome: impl-defined`, the byte view for FFI,
  and the value grammar (B-11, D-0019); `checked_*` results are
  `Option<τ>`; `AddrWidth` treatment reconciled (B-25). All entities
  `ACCEPTED`.
- 0.4.0 and earlier — superseded.
