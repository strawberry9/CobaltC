# CobaltC Expression Semantics

Status: normative artifact
Version: 1.6.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.expr.*`; Kind: Type
`type.bool`, `type.unit`)
Governed by: `CobaltC_Master_Instructions.md` §23
Realizes: D-0007 (evaluation order), D-0019 (result kinds)

## 1. Results, positions, and evaluation contexts

### `rule.expr.context`
**Status:** ACCEPTED

Every expression reduces to a **result** `r ::= v | place a | temp o`
(`spec/05`). Reduction is driven by evaluation contexts; the hole
`□` marks the unique sub-expression that may step next, which encodes
strict left-to-right order (D-0007):

    E ::= □
        | E op e  |  r op E                          -- binary operators (spec/06)
        | -E  |  ~E  |  !E                            -- unary
        | &_m E                                       -- borrow (spec/09)
        | *E                                          -- deref
        | E.f  |  E[e]  |  r[E]                       -- projection (spec/16)
        | E(e1..en)  |  r(r1..r_{i-1}, E, e_{i+1}..en) -- call (spec/15): callee, then args left to right
        | E = e  |  place a = E  |  *p = E            -- assignment: target place first, then value;
                                                        -- `*p = E` with p a rawptr value is the raw write (spec/20)
        | if (E) b1 else b2                            -- condition
        | E ; e                                        -- sequencing
        | Name{.f1 = r1, …, .fi = E, …}  |  Vi(E)  |  [r1, …, E, …]   -- aggregate literals (spec/16), fields in written order
        | match (E) { … }                              -- scrutinee
        | return E  |  propagate(E)                    -- `spawn`/`join` are ordinary calls (spec/22 §2 correspondence
                                                        -- table): already covered by the `E(e1..en)`/`r(…)` entry above
        | stmt-scoped'(t, E)  |  {E}  |  unsafe {E}    -- spec/14

    [Context]
        ⟨e, Σ⟩ →^ℓ ⟨e', Σ'⟩
        ────────────────────────────────────────────
        ⟨E[e], Σ⟩ →^ℓ ⟨E[e'], Σ'⟩

A context only ever admits the leftmost non-result sub-expression
(`E op e` vs `r op E`), so order is structural. `&&`/`||`, `while`,
and closure bodies are not contexts for their right operand/body: they
are evaluated by their own rules (§4, `spec/14` §4, `spec/15` §6).

**Place positions.** A sub-expression is in *place position* — its
`place a` result is consumed as an access path, not read — when it is:
the operand of `&`/`&mut`; the target of `=`; the operand of `*` when
its static type is `guard<τ>` (`spec/19` `[Guard-Deref]`: a guard is a
resource and is never read as a value); the base of `.f`/`[i]`
whose static type is the aggregate itself (not a reference to it — the
reference case reads the reference first, `rule.agg.field-access`
`[Field-Access-Auto-Deref]`); the operand of `drop(...)`; the scrutinee of `match`;
a resource-typed argument, local-declaration initializer, field initializer, or
`return`/result operand (`rule.value-object.store` decides what to do
with it). Every other position is *value position*.

**Place expressions** are: a name, `*e`, `e.f`, `e[i]`, and the
intrinsic `reclaim<τ>(p)` (`spec/20` §2). `*e` is a place only for `e`
of reference or `guard` type; for `p : rawptr<τ>`, `*p` is read or
written directly by `rule.trust.rawptr` (`[Rawptr-Read]`,
`[Rawptr-Write]`, `[Rawptr-Move-*]`) and is not a place. No other form
yields `place`.

**Depends on:** D-0007, D-0019

### `rule.expr.lvalue-to-rvalue`
**Status:** ACCEPTED

    [LValue-To-RValue]
        ⟨e, Σ⟩ →* ⟨place a, Σ1⟩;  e is in value position
        ⟨read(a), Σ1⟩ →^ℓ ⟨v, Σ1⟩                  -- rule.value-object.read
        ────────────────────────────────────────────
        ⟨e, Σ⟩ →* ⟨v, Σ1⟩

    [Temp-To-RValue]
        ⟨e, Σ⟩ →* ⟨temp o, Σ1⟩;  e is in a position that requires a value and is not a
            store position (an operand of an operator, a rawptr write, a condition)
        ¬is-resource(type-of(o,Σ1));  v = value-at(type-of(o,Σ1), Σ1.storage, extent(o,Σ1))
        ────────────────────────────────────────────
        ⟨e, Σ⟩ →* ⟨v, Σ1⟩          -- o remains a temporary and ends at statement exit

A place reached in value position is read; a resource-typed place in
value position is therefore `[Read-Resource-Rejected]` (static). A
`temp o` reaching a `store` position (local declaration, argument, field, result)
is handed to `store` as a temporary (D-0019); reaching an operator or
other value-requiring position, a plain temporary is read
(`[Temp-To-RValue]`) and a resource temporary is
`[Read-Resource-Rejected]`. A temporary nothing stores ends at
statement exit.

**Depends on:** rule.value-object.read, D-0019

### `rule.expr.compound-assign`
**Status:** ACCEPTED

    [Compound-Assign]   op ∈ { +, -, *, /, %, &, |, ^, <<, >> }
        ⟨p op= e, Σ⟩ ≡ ⟨p = p op e, Σ⟩ with the place p evaluated once:
            p is evaluated to a place a (target first, as for `=`), then a is read, then e is evaluated,
            then (value of a) op (value of e) is written to a

`p op= e` (D-0035) is typed and checked exactly as `p = p op e`:
the same operand types (`<<=` and `>>=` take a `u32` amount), the same
checked arithmetic (`[Arith-Checked]`: `x += 1` can overflow), the
same write rules. It differs only in evaluating `p` once, which is
observable only when `p` contains a call, as in
`*Vec::index_exclusive(&mut v, i) += 1`: that call runs once, and its
result is the place both read and written, so the exclusive reference
it gives is live while `e` is evaluated (`e` then cannot read `v`,
`rule.alias.borrow`). Its type is `unit`, like `=`'s. There is no
`++` or `--`.

**Depends on:** rule.expr.context, rule.value-object.write,
rule.arith.checked, D-0035

## 2. `type.bool` and `type.unit`
**Status:** ACCEPTED

`type.bool`: represented domain `{true, false}`, `is-resource =
false`; produced by comparisons and `&&`/`||`/`!`, consumed by `if`/
`while`. `type.unit`: the single value `()`, `is-resource = false`;
the type of assignment, of `if` without `else`, of `while`, of a
block with no trailing expression, of a function with no declared
return type, and of the unit literal. `type.unit`'s surface spelling
is `void` in type position (`spec/22` §3) and `()` in value position —
two spellings for the one type, split the same way C's own `void`
never appears as a value:

    [Unit-Literal]   ⟨(), Σ⟩ → ⟨(), Σ⟩

**Depends on:** term.type, rule.type.kind

## 3. Sequencing

### `rule.expr.seq`
**Status:** ACCEPTED

    [Seq-Value]   ⟨r ; e2, Σ⟩ → ⟨e2, Σ⟩

The left result is discarded (its temporaries end at the enclosing
statement exit, `spec/14` §1a). A block's `statement*` sequence is
sugar for nested `;` with each statement wrapped in `stmt-scoped`
(`spec/14` §1a).

**Depends on:** D-0019

## 4. Short-circuit logical operators

### `rule.expr.logic`
**Status:** ACCEPTED

    [And-False]   ⟨false && e2, Σ⟩ → ⟨false, Σ⟩       [And-True]   ⟨true && e2, Σ⟩ → ⟨e2, Σ⟩
    [Or-True]     ⟨true || e2, Σ⟩ → ⟨true, Σ⟩         [Or-False]   ⟨false || e2, Σ⟩ → ⟨e2, Σ⟩

`e2` is evaluated only when needed (`E ::= E && e`, not `r && E`).

**Depends on:** type.bool

## Change Log

- 1.6.0 — `CHG-0044` (D-0035): `rule.expr.compound-assign`
  (`[Compound-Assign]`).

- 1.5.0 — `CHG-0010`: the operand of `*` is a place position when its
  type is `guard<τ>`; under 1.4.0 `*g` read `g` as a value and
  `[Read-Resource-Rejected]` applied (`conf.mutex-lock-unlock`,
  `ex.threads`).
- 1.4.0 — `CHG-0009`: `rule.expr.context` gains the `*p = E` context
  for a raw-pointer write (`spec/20` `[Rawptr-Write]`/`[Rawptr-Move-In]`
  evaluate the pointer before the value; no context previously
  admitted the value to step), and §1 states that `*p` on a raw
  pointer is not a place expression. Non-normative in the same pass:
  `reclaim`'s intrinsic form written as the grammar spells it
  (`reclaim<τ>(p)`); a citation of `[Auto-Deref]` corrected to
  `[Field-Access-Auto-Deref]`; one `let` mention re-worded.
- 1.3.0 — `CHG-0008`: removed `join(E)` and the dedicated
  `spawn(E, e1..en) | spawn(r, r1.., E, ..)` context entries — now
  that `spawn`/`join` are ordinary calls (`spec/22` 2.7.0), their
  evaluation order (callee first, then arguments left to right) was
  already fully specified by the generic `E(e1..en) | r(…)` call
  entry; the dedicated entries had become a redundant restatement, not
  a needed one. `rule.expr.context`'s actual evaluation-order
  guarantee (D-0007) is unchanged for every construct, including
  `spawn`/`join`.
- 1.2.0 — `CHG-0007`: noted `type.unit`'s split surface spelling
  (`void` in type position, `()` in value position, `spec/22` 2.6.0).
  No rule semantics changed.
- 1.1.0 — Retroactive catch-up (found during `CHG-0005`'s work, not a
  new normative change of its own): this file had been missed by
  every earlier `spec/22` re-spelling pass. `rule.expr.context`'s
  evaluation-context grammar re-spelled to current syntax —
  `Name{f1: r1, …}` to `Name{.f1 = r1, …}` (`CHG-0001`), `if E b1 else
  b2`/`match E { … }` to `if (E) b1 else b2`/`match (E) { … }`
  (`CHG-0003`) — and a stray reference to the retired `let` keyword
  fixed to "local-declaration initializer". No rule semantics changed
  at any point; `rule.expr.context`'s evaluation-order definition
  (D-0007) is unaffected.
- 1.0.0 — Rewritten per D-0019 (`spec/AUDIT-2.md` B-07, B-19): the
  context grammar covers every expression form; results and place
  positions defined in one place; `type.unit` and `type.bool`
  consolidated. All entities `ACCEPTED`.
- 0.3.0 and earlier — superseded.
