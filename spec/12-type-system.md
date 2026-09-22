# CobaltC Type System

Status: normative artifact
Version: 1.12.1
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.type.*`)
Governed by: `CobaltC_Master_Instructions.md` §23
Realizes: D-0006 (nominal identity, no implicit conversion), D-0010
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
`∀T1..Tn. (τ1..τk) → τr`. Instantiation is substitution (D-0010); no
runtime type information exists. A generic body may use a type
parameter `T` only as: a stored field/element type, a local-declaration/parameter/
return type, a `ref<T,m>`/`rawptr<T>` pointee, an argument to another
generic or intrinsic (`sizeof<T>`, `reclaim<T>`, `drop`), and the
operand of `*p` for `p : rawptr<T>`. Any operation requiring more
(arithmetic, comparison, field access, calling) on a value of bare
type `T` is ill-formed (`diag.unbounded-type-parameter`, static) —
there is no trait/bound system.

Passing `T` to an intrinsic, or to `printf`'s `%v` (`spec/21`
`rule.stdlib.format`, realized natively), defers its typing to each
instantiation, where `T` is a concrete type. An instantiation that
fails it (`max_value<T>()` with `T = bool`, `wrapping_add(a, b)` with
`a, b : T = f64`) is ill-formed with the intrinsic's own diagnostic,
statically (D-0026; `spec/06` `[T-Convert]`, `[T-Alt]`,
`[Limits-Not-Number]`).

**Depends on:** term.type, D-0010, D-0026

## 2. Type identity

### `rule.type.identity`
**Status:** ACCEPTED

`τ1 = τ2` iff they are the same `type.<name>` id with equal type
arguments (nominal, D-0006). `ref<i32,shared> ≠ ref<i32,exclusive>`.
No subtyping, no implicit conversion: every type change is a named
intrinsic (`spec/06` §4, `spec/18` §3). One exception, which is a
borrow rather than a conversion (D-0049): an argument of type
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
before comparison).

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
    [T-Arith]     Γ ⊢ e1 : τ, Γ ⊢ e2 : τ, τ integer (or float for + - * /), op ∈ {+ - * / % & | ^}  ⇒  Γ ⊢ e1 op e2 : τ
    [T-Shift]     Γ ⊢ e1 : τ integer, Γ ⊢ e2 : u32  ⇒  Γ ⊢ e1 << e2 : τ   (likewise >>)
    [T-Cmp]       Γ ⊢ e1 : τ, Γ ⊢ e2 : τ, τ has =_τ (not fn, not resource)  ⇒  Γ ⊢ e1 == e2 : bool
                  (likewise !=; < <= > >= additionally require τ numeric)
    [T-Logic]     Γ ⊢ e1 : bool, Γ ⊢ e2 : bool  ⇒  Γ ⊢ e1 && e2 : bool   (likewise ||)
    [T-Neg]       Γ ⊢ e : τ signed or float  ⇒  Γ ⊢ -e : τ
    [T-Not]       Γ ⊢ e : τ integer  ⇒  Γ ⊢ ~e : τ;   Γ ⊢ e : bool  ⇒  Γ ⊢ !e : bool
    [T-Borrow]    e a place, Γ ⊢ e : τ  ⇒  Γ ⊢ &e : ref<τ,shared>,  Γ ⊢ &mut e : ref<τ,exclusive>
    [T-Deref]     Γ ⊢ e : ref<τ,m>  ⇒  Γ ⊢ *e : τ   (a place);   Γ ⊢ e : guard<τ>  ⇒  Γ ⊢ *e : τ   (a place; spec/19 [Guard-Deref])
                  Γ ⊢ e : rawptr<τ>  ⇒  Γ ⊢ *e : τ   (not a place: read or written directly by rule.trust.rawptr, inside unsafe)
    [T-Rawptr-Offset]  Γ ⊢ e1 : rawptr<τ>, Γ ⊢ e2 : isize  ⇒  Γ ⊢ e1 + e2 : rawptr<τ>   (rule.trust.rawptr [Rawptr-Offset]; inside unsafe)
    [T-Field]     Γ ⊢ e : τ_struct with field f: τf, or Γ ⊢ e : ref<τ_struct,m>  ⇒  Γ ⊢ e.f : τf   (a place)
    [T-Index]     Γ ⊢ e : array<τ,N> or ref<array<τ,N>,m>, Γ ⊢ i : usize  ⇒  Γ ⊢ e[i] : τ   (a place)
    [T-Call]      callable(type-of(e_f), (τ1..τn) -> τr), Γ ⊢ ei : τi  ⇒  Γ ⊢ e_f(e1..en) : τr   (rule.fn.closure)
    [T-Generic]   per rule.fn.generic-call (explicit, argument-inferred, expected-type-inferred)
    [T-Assign]    e1 a place (or *p with Γ ⊢ p : rawptr<τ>, rule.trust.rawptr [Rawptr-Write]/[Rawptr-Move-In]), Γ ⊢ e1 : τ, Γ ⊢ e2 : τ  ⇒  Γ ⊢ e1 = e2 : unit
    [T-If]        Γ ⊢ c : bool, Γ ⊢ b1 : τ, Γ ⊢ b2 : τ  ⇒  Γ ⊢ if (c) b1 else b2 : τ;   no else ⇒ τ = unit
    [T-While]     Γ ⊢ c : bool, Γ ⊢ b : unit  ⇒  Γ ⊢ while (c) b : unit
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
statement. A `str-literal` or `byte-literal` has one type regardless
of position (`[T-Lit-Str]`); an expected type there is simply
compared, as for any other typed expression.

**Depends on:** D-0012, D-0013, D-0014

## Change Log

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
