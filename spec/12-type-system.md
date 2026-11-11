# CobaltC Type System

Status: normative artifact
Version: 1.31.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.type.*`)
Governed by: `CobaltC_Master_Instructions.md` §23
Realizes: D-0006 (nominal identity, no implicit conversion; `[Coerce-Text]` and `[Coerce-Slice]`,
D-0312, are its two coercions — `[Coerce-Slice-Literal]`, D-0314, a clause of the second — each
the expression a writer could have spelled), D-0010
(monomorphic generics), D-0012/D-0013/D-0014 (local inference)

## 1. Kinds

### `rule.type.kind`
**Status:** ACCEPTED

    Kind ::= Type | Type^n → Type   (n ≥ 1)

Built-in constructors: `ref : Type × Mode → Type`, `rawptr : Type →
Type`, `array : Type × ℕ → Type`, `fn : Type^n × Type → Type`,
`handle`, `mutex`, `guard : Type → Type` (`spec/19`). A declaration
`struct Name<T1..Tn>` / `enum Name<T1..Tn>` has kind `Type^n → Type`;
a generic function `fn name<T1..Tn>(τ1 p1, …, τk pk) : τr` has schema
`∀T1..Tn. (τ1..τk) → τr`. `[Type-Arity]` — a type `Name<σ1..σk>` of a
declaration of kind `Type^n → Type` has `k = n`: `Option<i32, i32>` and a
bare `Vec` name no type (`diag.type-mismatch`, static; `CHG-0438`), as
`[Generic-Call-Arity]` (`spec/15` §5) says for a call. Instantiation is substitution (D-0010); no
runtime type information exists. A generic body may use a type
parameter `T` only as: a stored field/element type, a local-declaration/parameter/
return type, a `ref<T,m>`/`rawptr<T>` pointee, an argument to another
generic or intrinsic (`sizeof<T>`, `reclaim<T>`, `drop`), and the
operand of `*p` for `p : rawptr<T>`, and the operators its bound
provides (`rule.type.bound`, D-0090). Any operation requiring more
(arithmetic, comparison, field access, calling) on a value of type `T`
is ill-formed (`diag.unbounded-type-parameter`, static).

Passing `T` to an intrinsic, or to `printf`'s `%v` (`spec/21`
`rule.stdlib.format`, realized natively), defers its typing to each
instantiation, where `T` is a concrete type. An instantiation that
fails it (`max_value<T>()` with `T = bool`, `wrapping_add(a, b)` with
`a, b : T = f64`) is ill-formed with the intrinsic's own diagnostic,
statically (D-0026; `spec/06` `[T-Convert]`, `[T-Alt]`,
`[Limits-Not-Number]`).

**Depends on:** term.type, D-0010, D-0026

### `rule.type.bound`
**Status:** ACCEPTED

A function's type parameter may carry a bound (`spec/22`
`type-param`, D-0090), one of a closed set; each implies those before it:

| Bound | Types that satisfy it | Operators on a value of `T` |
|---|---|---|
| `eq` | the integer and floating-point types, `bool`, `str`, `String`; every key type (D-0271: a struct or enum whose fields or payloads are all key types, `spec/21` `rule.stdlib.hashmap`, D-0110) | `==` `!=` |
| `ordered` | the integer and floating-point types, `bool`, `str`, `String` (D-0108); every key type (D-0271, ordered as `[Sort]` and `[Cmp-Key]` order it) | and `<` `<=` `>` `>=` |
| `number` | the integer and floating-point types | and binary `+ - * / %` (both operands `T`), unary `-`, compound assignment |
| `integer` | the integer types | and `& \| ^ ~`, `<< >>` (amount `u32`) |
| `clone` (D-0116) | every type with `clone-holds` below: a plain type; a type with a `T::clone`; a struct or enum whose fields are all `clone` | `clone(&x) : T` |
| `plain` (D-0257) | bytes that mean the same anywhere: the integer and floating-point types, `bool`, `rawptr<τ>`, `simd<τ, N>`, and an array, struct or enum of `plain` types (no resource, reference, slice, handle, mutex, guard, function value or `str` in it) | none of its own: what `Vec::from_raw` and `copy_to_raw` (`spec/21`) move across raw memory whole |

`clone` is implied by each of the four above (their types are all
copied by a read or, for `String`, by `String::clone`) and implies
none of them. `plain` implies `clone` (a plain value is copied by a
read) and nothing else; none of the operator bounds implies it, since
each admits `String`.

    clone-holds(τ)  ≝  ¬is-resource(τ) ∧ ¬holds-exclusive(τ)                      -- a read copies it (D-0218)
                     ∨ τ = N<τ1..τn> and `N::clone` is declared (spec/17 §1: `fn N::clone<P1: b1, …>(ref<N<P1..>, shared> self) : N<P1..>`)
                          and each τi satisfies bi                                -- `Vec<τ: clone>`, `Rc<τ>`, `Box<τ: clone>`, …
                     ∨ τ = N<τ1..τn>, N a struct or enum declared without `resource`, with no declared `N::clone`,
                          and every field (payload) type, at τ1..τn, has clone-holds          -- derived
                     ∨ τ = array<τe, n> and ¬is-resource(τe)
        -- never: a `resource`-marked type without its own `N::clone`, `fn`, `handle`, `mutex`, `guard`;
        -- never: an exclusive reference or slice, or a type holding one (holds-exclusive: a field, array
        -- element or payload that is one, at any depth) — `clone` reads through a shared reference (D-0218);
        -- a type met again while its own fields are examined has clone-holds (a recursive type through `Box` or `Rc`)

    [T-Clone]
        Γ ⊢ r : ref<τ, m>;  clone-holds(τ)  (τ a type parameter: its bound implies clone)
        ────────────────────────────────────────────
        Γ ⊢ clone(r) : τ       -- ¬is-resource(τ): the value read through r (rule.value-object.read);
                               -- else N::clone(r), declared or derived — spec/16 rule.agg.derived-clone

    [Clone-Unsatisfied]   disposition: rejected
        clone(r) with r : ref<τ, m> and ¬clone-holds(τ)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (the message says which field or payload is not clone)

    [T-Bound-Op]
        T : b in the enclosing generic function;  op provided by b
        Γ ⊢ e1 : T  (Γ ⊢ e2 : T)
        ────────────────────────────────────────────
        Γ ⊢ e1 op e2 : T  (or bool for a comparison)

    [T-Bound-Literal]
        T : b, b implies number;  an unsuffixed integer literal L where T is expected
        ────────────────────────────────────────────
        Γ ⊢ L : T      -- at each instantiation, L's value at that type
                        -- (at a floating-point type, the value as a float)

    [Bound-Unsatisfied]   disposition: rejected
        a call of f<…, T : b, …> fixes T := τ;  τ does not satisfy b
        (τ a caller's own type parameter: its bound does not imply b)
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch

    [Bound-Missing]   disposition: rejected
        an operator on a value of type parameter T that T's bound (if any) does not provide
        ────────────────────────────────────────────
        ill-formed; diag.unbounded-type-parameter

Each instantiation is still checked at its concrete type, so what the
operator itself requires there applies: `-x` at `u32` is
`diag.type-mismatch` there, `T x = 300` at `u8` is
`diag.literal-out-of-range`. Structs and enums take no bounds. A user
type satisfies `clone` by `clone-holds`, and `eq` and `ordered` when it
is a key type (D-0271): its equality is `[Eq-Struct]`'s and its order
`[Cmp-Key]`'s, both the language's, so no operator is overloaded. It
never satisfies `number` or `integer`; an order other than its fields'
is passed in as a `fn` value, as `Vec::sort_by` takes `less` (`spec/22`
§5). `N::clone` is recognised by name
as `N::drop` is (`spec/07` §1): a function so named and shaped is the
type's copy, called by `clone` and by nothing else in particular.

**Depends on:** rule.type.kind, rule.arith.checked, rule.arith.cmp, rule.arith.bitwise, rule.arith.literal, D-0090

## 2. Type identity

### `rule.type.identity`
**Status:** ACCEPTED

`τ1 = τ2` iff they are the same `type.<name>` id with equal type
arguments (nominal, D-0006). `ref<i32,shared> ≠ ref<i32,exclusive>`.
No subtyping, no implicit conversion: every type change is a named
intrinsic (`spec/06` §4, `spec/18` §3), with the two coercions of
`rule.type.coerce-text` and `rule.type.coerce-slice` (text and a sequence
fit the type declared for their position, D-0312; each is the expression
a writer could have spelled) and one admission that is a borrow rather
than a conversion (D-0049): an argument of type
`ref<τ, exclusive>` (`slice<τ, exclusive>`) is accepted for a
function's parameter of type `ref<τ, shared>` (`slice<τ, shared>`)
and passed as the shared reborrow `&*e` (`&e[0 .. $]`)
(`spec/15` `rule.fn.bind-param`). It applies to a call of a function
item, directly or through `spawn`, not to a call through a `fn` value
or of a closure.

**Depends on:** D-0006

## 3. `is-resource`

### `rule.type.is-resource`
**Status:** ACCEPTED

| Type | `is-resource` |
|---|---|
| integers, floats, `bool`, `unit`, `str` | `false` |
| `ref<τ,m>`, `rawptr<τ>`, `fn(...):τ` | `false` |
| `array<τ,N>` | `is-resource(τ)` |
| `struct`/`enum` declared without `resource` marker | `∃ field/payload type τi. is-resource(τi)` |
| `struct`/`enum` declared with `resource` marker (`spec/22` §3) | `true` |
| `handle<τ>`, `mutex<τ>`, `guard<τ>` | `true` (`spec/19`) |

A struct declared without the marker whose fields include a resource
is a resource by derivation; declaring the marker on a type with no
resource-bearing field is how a type that manages something beyond
its fields (`Vec<T>`, `Rc<T>`, `spec/21`) declares itself one. A type
with the marker should declare a destructor (`spec/07` §1); a resource
type without one is permitted (its only cleanup is its fields'). For
a generic instantiation, `is-resource` is evaluated after substitution
(D-0010).

**Depends on:** inv.resource-authority, D-0003, D-0006, D-0010

## 4. Value equality

### `rule.type.eq`
**Status:** ACCEPTED

    [Eq-Int]      v1 =_τ v2  ≝  v1 =_ℤ v2                    (integer τ)
    [Eq-Float]    v1 =_τ v2  ≝  IEEE 754 equality             (NaN ≠ NaN; +0.0 = -0.0)
    [Eq-Bool]     v1 =_τ v2  ≝  v1 ⇔ v2
    [Eq-Unit]     () =_unit ()
    [Eq-Str]      ⟪b0..b_{n-1}⟫ =_str ⟪c0..c_{m-1}⟫  ≝  n = m ∧ ∀ i. bi = ci     (bytewise; spec/21 §2a)
    [Eq-Ref]      v1 =_τ v2  ≝  rule.ref.identity-eq
    [Eq-Rawptr]   v1 =_τ v2  ≝  same address
    [Eq-Struct]   ⟨f: v⟩ =_τ ⟨f: w⟩  ≝  ∀ i. vi =_τi wi     (structs, arrays elementwise, enums:
                                                              same variant and equal payload)
    [Eq-Fn]       no rule: `==` on function values is ill-typed

`==` on a resource-bearing type is ill-typed (`rule.arith.cmp`'s
operands are value position, `[Read-Resource-Rejected]` applies
before comparison), except on `String`, whose operands `[Cmp-Text]`
borrows instead (D-0108).

**Depends on:** rule.ref.identity-eq

## 5. Static typing

### `rule.type.typing`
**Status:** ACCEPTED

`Γ ⊢ e : τ` is decidable from program text. `Γ` maps bindings to
their declared or synthesized types and item paths to their
signatures (`Σ_0.items`). Every rule below is syntax-directed on
`spec/22`'s grammar; `τ_expected` (§6) is threaded downward where the
form admits it.

    [T-Lit]       type per rule.arith.literal (suffix, expected, or default)
    [T-Lit-Str]   Γ ⊢ "…" : str   (rule.stdlib.str [Str-Literal]; never from an expected type)
                  Γ ⊢ b"…" : array<u8, N>, N its byte count   (rule.agg.array-construct)
    [T-Unit]      Γ ⊢ () : unit             [T-Bool]  Γ ⊢ true/false : bool
    [T-Name]      Γ(x) = τ  ⇒  Γ ⊢ x : τ    (a place)
    [T-Item]      Γ(path) = (τ1..τn) -> τr  ⇒  Γ ⊢ path : fn(τ1..τn) : τr
    [T-Item-Generic]  Γ(path) = <T1..Tn>(τ1..τm) -> τr, every Ti fixed — by explicit
                  arguments (path<σ1..σn>, one per parameter, [Generic-Call-Arity])
                  or by matching the signature against the expected fn type
                  (rule.type.expected), the instantiation equal to it —
                  ⇒  Γ ⊢ path : the instantiated fn type   (D-0093)
    [T-Item-Value-Uninferable]  disposition: rejected   a generic path bound as a value
                  (`auto f = id;`) with some Ti fixed by neither; ill-formed;
                  diag.cannot-infer-type-parameter — a binding is never polymorphic,
                  as `auto v = Vec::new();` is never unbounded ([Generic-Call-Uninferable]).
                  In an argument position the call instantiates it by its own rules
                  (rule.fn.generic-call, [T-Spawn]); nothing is decided there.   (D-0093)
    [T-Arith]     Γ ⊢ e1 : τ, Γ ⊢ e2 : τ, τ integer (or float for + - * /), op ∈ {+ - * / % & | ^}  ⇒  Γ ⊢ e1 op e2 : τ
    [T-Shift]     Γ ⊢ e1 : τ integer, Γ ⊢ e2 : u32  ⇒  Γ ⊢ e1 << e2 : τ   (likewise >>)
    [T-Cmp]       Γ ⊢ e1 : τ, Γ ⊢ e2 : τ, τ has =_τ (not fn, not resource unless String)  ⇒  Γ ⊢ e1 == e2 : bool
                  (likewise !=; < <= > >= additionally require τ ordered: a number, bool,
                  str, String or StringView — D-0108)
    [T-Logic]     Γ ⊢ e1 : bool, Γ ⊢ e2 : bool  ⇒  Γ ⊢ e1 && e2 : bool   (likewise ||)
    [T-Neg]       Γ ⊢ e : τ signed or float  ⇒  Γ ⊢ -e : τ
    [T-Not]       Γ ⊢ e : τ integer  ⇒  Γ ⊢ ~e : τ;   Γ ⊢ e : bool  ⇒  Γ ⊢ !e : bool
    [T-Borrow]    e a place, Γ ⊢ e : τ  ⇒  Γ ⊢ &e : ref<τ,shared>,  Γ ⊢ &mut e : ref<τ,exclusive>
    [T-Deref]     Γ ⊢ e : ref<τ,m>  ⇒  Γ ⊢ *e : τ   (a place);   Γ ⊢ e : guard<τ>  ⇒  Γ ⊢ *e : τ   (a place; spec/19 [Guard-Deref])
                  Γ ⊢ e : rawptr<τ>  ⇒  Γ ⊢ *e : τ   (not a place: read or written directly by rule.trust.rawptr, inside unsafe)
    [T-Rawptr-Offset]  Γ ⊢ e1 : rawptr<τ>, Γ ⊢ e2 : isize  ⇒  Γ ⊢ e1 + e2 : rawptr<τ>   (rule.trust.rawptr [Rawptr-Offset]; inside unsafe)
    [T-Field]     Γ ⊢ e : τ_struct with field f: τf, or Γ ⊢ e : ref<τ_struct,m>  ⇒  Γ ⊢ e.f : τf   (a place)
    [T-Index]     Γ ⊢ e : array<τ,N> or ref<array<τ,N>,m>, Γ ⊢ i : usize  ⇒  Γ ⊢ e[i] : τ   (a place)
    [T-Read-Through-Shared]  (D-0218)  e a place reached through a shared path (spec/08 via-shared), Γ ⊢ e : ref<τ,exclusive>
                  (slice<τ,exclusive>), e read as a value  ⇒  the value is ref<τ,shared> (slice<τ,shared>);
                  of any other type holding an exclusive reference: ill-formed (spec/08 [Read-Exclusive-Through-Shared])
    [T-Call]      callable(type-of(e_f), (τ1..τn) -> τr), Γ ⊢ ei : τi  ⇒  Γ ⊢ e_f(e1..en) : τr   (rule.fn.closure)
    [T-Generic]   per rule.fn.generic-call (explicit, argument-inferred, expected-type-inferred)
    [T-Assign]    e1 a place (or *p with Γ ⊢ p : rawptr<τ>, rule.trust.rawptr [Rawptr-Write]/[Rawptr-Move-In]), Γ ⊢ e1 : τ, Γ ⊢ e2 : τ  ⇒  Γ ⊢ e1 = e2 : unit
    [T-If]        Γ ⊢ c : bool, Γ ⊢ b1 : τ, Γ ⊢ b2 : τ  ⇒  Γ ⊢ if (c) b1 else b2 : τ;   no else ⇒ τ = unit
    [T-While]     Γ ⊢ c : bool, Γ ⊢ b : unit  ⇒  Γ ⊢ while (c) b : unit
    [T-While-Forever]   (D-0175)   Γ ⊢ b : unit, c the literal `true`, and no `break` in b leaves this loop
                  (one inside a nested loop leaves that loop)  ⇒  Γ ⊢ while (true) b : never
    [T-Block]     each statement well-typed under Γ extended by its local declarations; trailing e : τ  ⇒  block : τ;
                  no trailing e ⇒ unit, except that a block whose last statement is an expression statement of type never is itself never
    [T-Let]       Γ ⊢ e : τ (or annotation τ with e checked against it)  ⇒  Γ, x:τ ⊢ …
    [T-Let-Destructure]  Name{f1..fn} = e with Γ ⊢ e : Name, Name's fields exactly f1..fn : τ1..τn  ⇒  Γ, f1:τ1, …, fn:τn ⊢ …   (rule.init.let [Let-Destructure])
    [T-Return]    Γ ⊢ e : τr (the enclosing fn's return type; unit if absent)  ⇒  Γ ⊢ return e : never
    [T-Break]     inside a while  ⇒  Γ ⊢ break : never;   likewise continue
    [T-Struct]    Name<σ>{.fi = ei} with Γ ⊢ ei : τi[σ] for every declared field, each exactly once  ⇒  : Name<σ>
    [T-Enum]      Vi(e) with Γ ⊢ e : τi[σ]  ⇒  : Name<σ>;   payload-less Vi  ⇒  Vi(())
    [T-Match]     Γ ⊢ e : Name<σ>; each arm Vi(xi) : ei with Γ, xi:τi[σ] ⊢ ei : τ; exhaustive (rule.agg.match)  ⇒  : τ
    [T-Closure]   move? [x1..](τ1 p1..) b with Γ, pi:τi ⊢ b : τr  ⇒  : closure type c with callable(c, (τ1..) -> τr)   (rule.fn.closure)
                  where a `fn(τ1..) : τr'` is expected (rule.type.expected), b is checked against τr' and
                  τr = τr'; a closure's value used at `fn(…) : τ` must have τr = τ (D-0073);
                  a written result type `: τd` (D-0081) checks b against τd and makes τr = τd
    [T-Unsafe]    Γ ⊢ b : τ  ⇒  Γ ⊢ unsafe b : τ
    [T-Spawn]     callable(type-of(e_f), (τ1..τn) -> τr), e_f a fn value or a move closure (rule.conc.spawn), Γ ⊢ ei : τi  ⇒  Γ ⊢ spawn(e_f, e1..en) : handle<τr>
    [T-Join]      Γ ⊢ e : handle<τ>  ⇒  Γ ⊢ join(e) : τ
    [T-Never]     an expression of type never is accepted at any expected type

`never` is the type of expressions that do not produce a value
(`return`, `break`, `continue`, and the prelude's `fault(d)`, `spec/21`
§0); it has no values and no `type.<name>`
entity is needed for it beyond this rule. A program is **well-typed**
iff every function body, with parameters in `Γ` and body type equal to
the declared return type, satisfies these rules; a well-typed program
is a precondition of every runtime rule (`spec/01` §5). Ill-typed
programs are `disposition: rejected`, `diag.type-mismatch` (registry).

`[T-Join]` (like `[T-Spawn]`) is not a special case of `[T-Item]`/
`[T-Call]` kept only for symmetry: `Γ`'s item-path bindings come solely
from `Σ_0.items` (§0 above), and `join` — a body-less prelude
intrinsic — is never entered there (`state.items`, `spec/04` §1;
`spec/22` §3's `item` grammar has no production for one); `[T-Item]`
therefore has nothing to look up for the bare name `join` regardless
of its fixed arity, and `[T-Join]` is the sole source of `join(e)`'s
type. `feat.consolidate-join-typing`'s redundancy question is settled,
not open: `[T-Call]`/`[Generic-Call-Inferred]` cannot ever apply here,
so there is nothing for `[T-Join]` to be redundant with (`CHG-0023`).

**Depends on:** rule.arith.literal, rule.fn.closure,
rule.fn.generic-call, rule.agg.match, rule.type.expected, D-0006,
CHG-0023

### `rule.type.expected`
**Status:** ACCEPTED

The **expected type** at a position is (D-0012 §3b, D-0013, D-0014):
the written type of an enclosing local declaration; the declared type of the
parameter an argument is passed to, or of the return type for the
function body's result / `return` operand; the declared field type
inside a struct/enum literal whose own type is known; the already
determined type of the other operand within one arithmetic or
comparison expression; `u32` for the right operand of `<<`/`>>` and
`usize` for the index operand of `e[i]` (`[T-Shift]`, `[T-Index]`);
the scrutinee's type for match-arm patterns, and, where nothing outside
fixes a type, the type of the first branch that is not a *literal
branch* for the other branches of an `if` or `match` (D-0049) — a
literal branch is a literal expression (below), or a block holding
only one, so `if (c) { x } else { 0 }` and `if (c) { 0 } else { x }`
both take `x`'s type. An expected type at an `if` expression is
the expected type of each branch's trailing expression; at a `match`,
of every arm's body; at a block or `unsafe` block, of its trailing
expression; at `(e)`, `-e`, `~e`, and `!e`, of `e`. At an arithmetic
or bitwise operator whose operands are all *literal expressions*
(unsuffixed numeric literals, and `-`, `~`, parentheses and these
operators applied to them; a shift's amount aside), an expected number
type is the expected type of each operand, the left one only for a
shift (D-0037): `u64 x = 1024 * 1024;` computes in `u64`. Expected types propagate downward into
numeric literals, generic calls, and struct/enum literals only, and only
through the positions just listed; nothing is inferred from a later
statement. A `byte-literal` has one type regardless of position, and so
does a `str-literal` (`[T-Lit-Str]`), except where `[Coerce-Text]` (below)
gives it its position's text type. Elsewhere an expected type there is
simply compared, as for any other typed expression.

**Depends on:** D-0012, D-0013, D-0014

### `rule.type.coerce-text`
**Status:** ACCEPTED

    [Coerce-Text]   (D-0312)
        e at a position whose expected type is τ_exp (rule.type.expected: an argument
        whose parameter is declared τ_exp, or whose type parameter the other arguments or
        the expected type fix as τ_exp; a local's written type; the value assigned to a
        place; a struct literal's field; a variant constructor's payload once fixed; an
        array literal's element; what a function or closure declared : τ_exp gives, by
        `return` or as a tail; and the tails of a block, an `if` or a `match` standing in
        any of these)
        ────────────────────────────────────────────
        e a str-literal or a str constant, τ_exp = String       ⇒  e : String, the String of e's bytes
        e a str-literal or a str constant, τ_exp = StringView   ⇒  e : StringView, a view of static text
        e : str,                           τ_exp = StringView   ⇒  StringView::of(e)
        e = &s, s a String place,          τ_exp = StringView   ⇒  &s[0..$]             -- [View-Form]
        e : ref<String, m>,                τ_exp = StringView   ⇒  &(*e)[0..$]
        -- and nothing else: no str value becomes a String, no String by value a view,
        -- nothing becomes a str or a ref<String, _>. Under `auto`, and where nothing fixes
        -- a type parameter, a literal is a str.

    [Coerce-Text-Generic-Disagree]   disposition: rejected
        e is in the body of a generic function, and [Coerce-Text] converts e in one
        instantiation and not in another
        ────────────────────────────────────────────
        ill-formed at e; diag.type-mismatch

`[Coerce-Text]` is one of the language's two coercions (D-0006; `[Coerce-Slice]` is the
other): each conversion is the
expression a writer could have spelled, changes no representation, and allocates
nothing but a literal where `String` is declared. The front end writes the
expression and checks the program again; a tool sees only what could have been
written. A `ref<String, exclusive>` is viewed as a shared one is: a view only reads.
`--check` warns where the rule makes a call redundant (D-0312 §4): `String::copy("…")` where a
`String` is declared is the literal (`diag.text-copy-literal`), and `String::copy(&s)` of a whole
`String` reads better as `String::clone(&s)` (`diag.text-copy-whole`); the program is accepted.

**Depends on:** rule.type.expected, D-0006, D-0302, D-0312

### `rule.type.probe`
**Status:** ACCEPTED

    probe<String>  = StringView
    probe<τ>       = τ                  τ an integer type, f32, f64, bool or str
    probe<τ>       = ref<τ, shared>     any other τ (a struct, an enum, an array, …)

`probe<τ>` is a type-level function, as `is-resource(τ)` is: the type by which a lookup
names a value of type `τ` without owning one (D-0312). It is evaluated where `τ` is
known: at once when written with a type, after substitution when written with a type
parameter (`probe<K>`, `rule.type.kind`'s instantiation); it fixes no type parameter
(inference skips it, the other arguments fix `K`). `std`'s lookups take it (`spec/21`
§1, §2g): a `String` key is looked up by any text (`[Coerce-Text]` makes a literal, a
`str` and `&s` its view), an integer key by its value, a struct key by `&k`.

**Depends on:** rule.type.kind, rule.type.coerce-text, D-0312

### `rule.type.coerce-slice`
**Status:** ACCEPTED

    [Coerce-Slice]   (D-0312)
        e at a position whose expected type is slice<τ, m> (the positions of [Coerce-Text])
        ────────────────────────────────────────────
        e = &v,     v a Vec<τ> or array<τ, N> place, m = shared       ⇒  &v[0..$]
        e = &mut v, v a Vec<τ> or array<τ, N> place, m = exclusive    ⇒  &mut v[0..$]
        e : ref<Vec<τ> | array<τ, N>, m'>, m = shared or m' = m        ⇒  &(*e)[0..$]
        -- a slice parameter's element type is fixed by such an argument (`Vec::from_slice(&[5: u8, 6])`)

The same rule as `[Coerce-Text]`'s for `&s`, applied to every sequence: `[lo..hi]` is
written only where a part is meant. `&K` of a constant array is the third form's `e`, a shared
reference to the constant's one object (spec/09 `[Ref-Form-Const]`): `&(*&K)[0..$]`. A shared borrow is no exclusive slice. `--check` warns where
`[0..$]` is written for the whole (`diag.slice-of-whole`, D-0312 §3.2): `&v[0..$]` where a slice is
declared is `&v`, `&s[0..$]` where a `StringView` is declared is `&s`, and a view, a slice, a `str`
or a reference to a sequence is passed as it is; the program is accepted.

    [Coerce-Slice-Literal]   (D-0314)
        e at a position whose expected type is slice<τ, shared> (the positions of [Coerce-Text]),
        e an array literal [e1, …, eN] or [e1; N] whose elements are written out as literals
        (numbers, bools, arrays and structs of them; spec/09 [Ref-Form-Const]), each of type τ,
        τ plain (spec/12 §1)
        ────────────────────────────────────────────
        e  ⇒  &(*&K)[0..$]       -- K an unnamed constant `const array<τ, N> K = e;`: one object
                                 -- with program extent, as [Ref-Form-Const] gives a named one
        -- the elements are typed with τ as their expected type, so `[1, 2]` is of `u8` where
        -- `slice<u8, shared>` is declared

    [Coerce-Slice-Literal-Generic-Disagree]   disposition: rejected
        e is in the body of a generic function, and [Coerce-Slice-Literal] gives e's constant one
        array type in one instantiation and another in another
        ────────────────────────────────────────────
        ill-formed at e; diag.type-mismatch

A `byte-literal` (`b"…"`, `spec/22` §1) is such a literal: where a `slice<u8, shared>` is
declared it is a slice of bytes with program extent, `slice<u8, shared> magic = b"\x7fELF";`,
a field `.magic = b"PK\x03\x04"`, an argument `sum(b"abc")`. Where it is not, it stays an
`array<u8, N>` value, and `&b"…"` is a slice as an argument, as any borrowed temporary is. An
exclusive slice is never given (a literal's object is no one's to write), nor is a literal with
an element that is not a literal (`[x, 2]`: bind the array first). Equal literals may share one
object: their elements cannot be written, so no program can tell.

**Depends on:** rule.type.coerce-text, D-0047, D-0312, D-0314

## Change Log

- 1.31.0 — `CHG-0438`: `[Type-Arity]`, a generic type written with as many type arguments as it has parameters (it was not checked).
- 1.30.1 — `CHG-0416`: the header, `rule.type.identity` and `rule.type.coerce-text` say "two coercions" alike (one said "the one coercion"); `rule.type.coerce-slice` says `&K` of a constant array is a reference its third form takes (`&(*&K)[0..$]`). Clarification.
- 1.30.0 — `CHG-0416` (D-0312 completed): `rule.type.coerce-slice` names `--check`'s lint `diag.slice-of-whole`, a whole sequence sliced where a slice or a view is declared. Additive.
- 1.29.0 — `CHG-0415` (D-0314): `rule.type.coerce-slice`: `[Coerce-Slice-Literal]` and `[Coerce-Slice-Literal-Generic-Disagree]`; a `byte-literal` where a `slice<u8, shared>` is declared is a slice of static bytes (D-0312 §2's deferred piece). Additive.
- 1.28.0 — `CHG-0413` (D-0312): `rule.type.coerce-text` (`[Coerce-Text]`, `[Coerce-Text-Generic-Disagree]`), `rule.type.coerce-slice` (`[Coerce-Slice]`) and `rule.type.probe` (`probe<T>`); `rule.type.expected` names `[Coerce-Text]` in place of its exceptions for a `str-literal`; the header says they are D-0006's two coercions. Breaking (source) where a literal or a `str` was a `ref<String, shared>` (D-0308).
- 1.27.0 — `CHG-0402` (D-0304, D-0305): `rule.type.expected`: the five positions where a text literal is now typed by its position; the literal's `String` named without `String::from_str`. Additive.
- 1.26.0 — `CHG-0400` (D-0302): `rule.type.expected`: where a `StringView` is declared, `&s` of a
  `String`, a `ref<String, shared>` place and a `str` value are views (`spec/21` `[Ref-String-View]`,
  `[Str-Value-View]`). Additive.
- 1.25.0 — `CHG-0344` (D-0271): `rule.type.bound`'s `eq` and `ordered` admit every key type; a user
  type satisfies them when it is one. Additive.
- 1.24.0 — `CHG-0319` (D-0257): §1 the bound `plain`.
- 1.23.0 — `CHG-0257` (D-0218): `[T-Read-Through-Shared]`; `clone-holds`
  excludes exclusive references and types holding them.
- 1.22.0 — `CHG-0203` (D-0175): `[T-While-Forever]`: a `while (true)` no `break`
  leaves has the type `never`. Additive.
- 1.21.0 — `CHG-0190` (D-0162): a text literal argument whose parameter a type
  parameter fixed as `String` is `String::from_str` of it.
- 1.20.0 — `CHG-0177` (D-0149): `rule.type.expected`: a `str-literal` whose
  expected type is a declared `String` is `String::from_str` of it (`spec/21`
  `[Str-Literal-String]`).
- 1.19.0 — `CHG-0157` (D-0134): `rule.type.expected`: a `str-literal` whose
  expected type is a declared `StringView` is that view (`spec/21`
  `[Str-Literal-View]`).
- 1.18.0 — `CHG-0139` (D-0116): the `clone` bound, `clone-holds`, `[T-Clone]`,
  `[Clone-Unsatisfied]`; a user type satisfies `clone`.
- 1.17.0 — `CHG-0127` (D-0108): `bool`, `str` and `String` satisfy
  `eq` and `ordered`; `[T-Cmp]` orders them (and `StringView`) and
  admits `String` operands, which `[Cmp-Text]` borrows.
- 1.16.0 — `CHG-0108` (D-0093): `[T-Item-Generic]` and
  `[T-Item-Value-Uninferable]`: a generic item as a value is
  instantiated by explicit arguments or the expected fn type, and a
  binding with neither is rejected. Previously no rule covered it,
  and the two implementations decided differently.
- 1.15.0 — `CHG-0105` (D-0090): `rule.type.bound`: `eq`, `ordered`,
  `number`, `integer` on a function's type parameters.
- 1.14.0 — `CHG-0097` (D-0081): `[T-Closure]` with a written result type.
- 1.13.0 — `CHG-0086` (D-0073): `[T-Closure]` checks the body against an
  expected `fn` type's result, statically.

- 1.12.1 — `CHG-0059` (D-0051): cites `[Limits-Not-Number]` (renamed).

- 1.12.0 — `CHG-0057` (D-0049): `rule.type.identity` admits an
  exclusive reference or slice argument for a shared parameter, as a
  shared reborrow; `rule.type.expected`: a literal branch of an `if` or
  `match` takes the type of the first branch that is not one.

- 1.11.0 — `CHG-0052` (D-0044): `[T-Let-Destructure]` over every field.

- 1.10.1 — Non-normative (`CHG-0047`, D-0039): `rule.type.kind`'s note
  names `printf`'s `%v`, `print` being private to `std`.

- 1.10.0 — `CHG-0045` (D-0037): `rule.type.expected` passes an
  expected number type into an operator whose operands are all literal
  expressions.

- 1.9.0 — `CHG-0037` (D-0028): `rule.type.kind`'s note covers
  `std::print`.

- 1.8.0 — `CHG-0035` (D-0026): `rule.type.kind` states that an
  intrinsic applied to a type parameter is typed at each
  instantiation.

- 1.7.0 — `CHG-0025` (D-0020): `str` added to `rule.type.is-resource`'s
  plain row; `[Eq-Str]` (bytewise) added to `rule.type.eq`;
  `[T-Lit-Str]` types `"…"` as `str` and `b"…"` as `array<u8, N>`;
  `rule.type.expected` notes that neither literal takes an expected
  type. `[T-Cmp]` is unchanged: `str` has `=_str` and is not numeric,
  so `==`/`!=` are admitted and the orderings are not.
- 1.6.1 — Non-normative (`CHG-0023` §"Hygiene"): §5 gains a paragraph
  settling `feat.consolidate-join-typing` — `[T-Join]` cannot be
  redundant with `[T-Item]`/`[T-Call]` since `join` is never a
  `Σ.items` entry. No rule changed.
- 1.6.0 — `CHG-0011`: an expected type propagates through the unary
  operators; without this `i64 x = -1;` typed `1` as `i32` and
  rejected the declaration (found deriving `ex.e2e-propagate-chain`).
- 1.5.0 — `CHG-0010` (per-case derivation of `spec/conformance.md`):
  `rule.type.expected` gains the shift-amount (`u32`) and index
  (`usize`) positions — without them `x >> 1` and `a[2]` were
  ill-typed (`conf.shr-arithmetic`, `conf.index-in-bounds`) — and
  propagation through `if` branches, `match` arms, block trailing
  expressions, and parentheses, which `spec/21`'s `usize new_cap = if
  (…) { 4 } else { … }` requires; `[T-Block]` types a block ending in
  a `never` statement as `never` (`{ return Err(…); }` as an `if`
  branch in `String::from_utf8`); `[T-Let-Destructure]` added
  (`conf.let-destructure` had no typing rule).
- 1.4.0 — `CHG-0009`: `[T-Deref]` extended to `guard<τ>` (typing
  `spec/19` `[Guard-Deref]`, which every mutex example already used)
  and to `rawptr<τ>` (typing `spec/20` `[Rawptr-Read]`, which
  `spec/21`'s `Vec`/`Rc` bodies and `ex.unsafe-required` already
  used); new `[T-Rawptr-Offset]` for `p + n` on a raw pointer
  (`[Rawptr-Offset]`, likewise already relied on); `[T-Assign]` admits
  `*p` for a raw pointer target; `[T-Spawn]` restated over `callable`
  so a `move` closure is accepted, as `rule.conc.spawn` and `spec/21`
  §0 always said it was; `[T-Never]` names `fault(d)`. Non-normative in
  the same pass: `[T-Closure]`, `[T-Struct]`, `rule.type.kind`'s
  generic-signature illustration, and two `let` mentions re-spelled to
  the current surface syntax (missed by `CHG-0001`/`CHG-0002`).
- 1.3.0 — `CHG-0005`: `[T-Match]`'s arm-shape illustration re-spelled
  with `:` instead of `=>`, matching `spec/22` 2.4.0. No rule
  semantics changed.
- 1.2.0 — `CHG-0003`: `[T-If]`/`[T-While]` restated with mandatory
  parentheses around the condition, matching `spec/22` 2.2.0. No rule
  semantics changed.
- 1.1.0 — `CHG-0002`: `fn(...)`-shaped type illustrations in `[T-Item]`,
  `[T-Spawn]`, and the `is-resource` table re-spelled to `spec/22` 2.0.0
  syntax (`:` instead of `->`); the *bare* `(τ1..τn) -> τr` signature
  notation used by `callable(...)` elsewhere in this file is unaffected
  — it never carried the literal `fn` keyword and was never meant to
  mirror real surface syntax, only the abstract signature shape. No
  rule semantics changed.
- 1.0.0 — Rewritten (`spec/AUDIT-2.md` B-19, B-20, B-21): full
  syntax-directed typing rules added (previously "deferred to
  Expression Semantics", which never supplied them); `is-resource`
  table updated for aggregates, `resource` marker, and `spec/19`
  types; equality for aggregates; `never`; `rule.type.expected`
  consolidates D-0012/13/14's source list. All entities `ACCEPTED`.
- 0.1.2 and earlier — superseded.
