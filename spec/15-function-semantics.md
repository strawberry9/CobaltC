# CobaltC Function Semantics

Status: normative artifact
Version: 1.5.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.fn.*`; Kind: Type
`type.fn`)
Governed by: `CobaltC_Master_Instructions.md` §17 (Function
Interfaces), §23
Realizes: D-0007, D-0010, D-0012, D-0013, D-0019

## 1. Function types

### `type.fn`
**Status:** ACCEPTED

`type.fn<(τ1,…,τn) : τr>`: the type of a named function item
(`fn` declaration, `spec/22` §3) and of a closure that captures
nothing. `is-resource = false`. Its values are item paths
(`spec/06` §7 `[Repr-Fn]`). A closure that captures is an anonymous
struct type (§6), not a `type.fn`; `callable` (§6) is what call
position and `fn`-typed parameters actually require.

**Depends on:** term.function, rule.type.kind

## 2. Call

### `rule.fn.call`
**Status:** ACCEPTED

    [Call]
        ⟨e_f, Σ⟩ →* ⟨r_f, Σ_0⟩;  callable(type-of(r_f), (τ1,…,τn) -> τr)            -- §6
        ⟨e_i, Σ_{i-1}⟩ →* ⟨r_i, Σ_i⟩  for i = 1..n, left to right                     -- D-0007;
            e_i in place position iff is-resource(τi) (spec/13 §1)
        f ∉ any frame stack;  Σ' = Σ_n[ frame-stack(ℓ) := f :: Σ_n.frame-stack(ℓ) ]
        ⟨store(binding(p_i), r_i), Σ'_{i-1}⟩ →^ℓ ⟨(), Σ'_i⟩  for i = 1..n              -- §3 (Σ'_0 = Σ')
        (closures: additionally bind `self` per §6)
        ⟨body(r_f), Σ'_n⟩ →* ⟨r_b, Σ_b⟩                                             -- the body block (spec/14 §1);
                                                                                    -- r_b is in result form
        ⟨block'(f, r_b), Σ_b⟩ →^ℓ ⟨r_b, Σ''⟩                                          -- [Block-Exit] of the parameter frame
        ────────────────────────────────────────────
        ⟨e_f(e1,…,en), Σ⟩ →^ℓ ⟨r_b, Σ''⟩
        side-conditions:
            ⟦ callable(…) ∧ r_i : τi ⟧ discharge: static (rule.type.typing)

A call evaluates callee then arguments left to right, pushes a
*parameter frame* `f`, binds each parameter by `store` (§3), evaluates
the body block in its own frame nested inside `f`, and finally exits
`f`, which ends the parameter objects. `return` inside the body folds
frames only up to the body's frame (§4); `[Call]` alone exits `f`, so
no frame is exited twice (`spec/AUDIT-2.md` B-06). The result `r_b`
— a value or a temporary — flows to the caller's enclosing
construct, which stores it or lets it end at its statement's exit.
Recursion needs nothing further.

**Depends on:** type.fn, D-0007, D-0019, rule.fn.closure,
rule.fn.bind-param, rule.control.block, rule.type.typing
**Affects:** state.frame-stack, state.bindings

## 3. Parameter binding

### `rule.fn.bind-param`
**Status:** ACCEPTED

Parameter `p_i : τi` is bound by `rule.value-object.store` into
`binding(p_i)` in the parameter frame:

| Declared `τi` | Argument result | Effect (`spec/05` `store`) |
|---|---|---|
| non-resource, not `ref` | value `v` | fresh object written with `v` (copy) |
| non-resource, not `ref` | place `a` | read, then as above (copy) |
| `ref<τ,m>` | value `a` (the caller formed the borrow) | fresh object holding the reference; `held-by ∋` that object; no new borrow |
| `ref<τ,shared>` (`slice<τ,shared>`) | value `a` of type `ref<τ,exclusive>` (`slice<τ,exclusive>`) | D-0049: first `[Borrow]` shared through `a` (`&*a`, `a` its ancestor), then as above with that reference |
| resource | place `a` | `[Store-Binding-Place-Transfer]`: moves authority; caller's binding invalidated |
| resource | temp `o` | `[Store-Binding-Temp]`: adopted |

The interface therefore states the whole agreement (Master
Instructions §17): a by-value resource parameter *takes* the resource;
a `ref<τ,shared>` parameter can only read; a `ref<τ,exclusive>`
parameter can mutate but not destroy or move; a plain parameter is a
copy. When the parameter frame exits, the parameter object holding a
reference ends and the reference is released from it
(`rule.value-object.object-end`); the reference itself remains valid
for whoever else holds it.

**Depends on:** rule.value-object.store, type.ref, D-0003, D-0006,
D-0018

## 4. `return`

### `rule.fn.return`
**Status:** ACCEPTED

    [Return]
        ⟨e, Σ⟩ →* ⟨r, Σ_0⟩;  ⟨store(result, r), Σ_0⟩ →^ℓ ⟨r', Σ_1⟩      -- result form (spec/05)
        f_b = the frame of the enclosing function's body block
        Σ_2 = unwind-to(f_b, r', Σ_1)                                    -- rule.control.unwind
        ────────────────────────────────────────────
        ⟨E_body[return e], Σ⟩ →^ℓ ⟨r', Σ_2⟩
            where E_body is the context from the body block's root down to the return

    [Return-Unit]   ⟨return, Σ⟩ → ⟨return (), Σ⟩

`return e` puts `e`'s result in result form (a resource binding is
released into a temporary, its obligation exempt from every sweep
that follows), unwinds every statement scope and frame inside the
function body, and makes `r'` the body's result; `[Call]` then exits
the parameter frame. A body's trailing expression reaches the same
state through `[Stmt-Exit]` and `[Block-Exit]` without unwinding —
the two forms are equivalent (`spec/AUDIT-2.md` B-06). `return` is
ill-formed outside a function body (`diag.return-outside-fn`,
static) and is `never`-typed.

**Depends on:** D-0008, D-0019, rule.value-object.store,
rule.control.unwind

## 5. Generic functions

### `rule.fn.generic-call`
**Status:** ACCEPTED

A generic item `fn name<T1..Tn>(τ1 p1, …) : τr` is instantiated by
substitution before `[Call]` applies (D-0010):

    [Generic-Call-Explicit]      name<σ1..σn>(e1..em)                     substitute σi for Ti
    [Generic-Call-Inferred]      name(e1..em); each Ti occurs as exactly the declared type of
                                 some parameter pk whose argument synthesizes σk (all such
                                 parameters agreeing)                        substitute σk for Ti   (D-0012 §3c)
    [Generic-Call-Expected-Type] the remaining Ti are solved by matching τr[known Ti] against
                                 the expected type at the call (rule.type.expected), one
                                 top-level constructor match                 (D-0013)
    [Generic-Call-Uninferable]   disposition: rejected   some Ti determined by none of the above
                                 ill-formed; diag.cannot-infer-type-parameter

`identity(5: i32)` infers `T := i32` from the argument; `Vec<i32> v
= Vec::new();` from the annotation; `auto v = Vec::new();` is rejected.
After substitution the body is type-checked as a concrete function
(`rule.type.typing`); `rule.type.kind`'s restriction on how `T` may
be used inside the body makes every instantiation well-typed iff the
generic body is.

**Depends on:** D-0010, D-0012, D-0013, rule.type.expected,
rule.type.typing

## 6. Closures

### `rule.fn.closure`
**Status:** ACCEPTED

    closure-lit ::= 'move'? '[' captures? ']' '(' params? ')' block   -- spec/22 §2
    captures    ::= identifier (',' identifier)*                      -- spec/22 §2

Let `x1..xk` be the free variables of the closure body (names bound
outside it and used inside). The written capture list must name
exactly this set, in any order — `[Closure-Capture-List-Rejected]`
otherwise. `m_i` is the *capture mode* of `x_i`: `exclusive` iff the
body contains a write, exclusive borrow, `drop`, or move whose place
root is `x_i`, else `shared` (a syntactic scan of the body's AST —
D-0017); the capture list plays no part in deciding mode, only in
confirming the written set matches the derived one.

    [Closure-Capture-List-Rejected]   disposition: rejected
        {x1,…,xk} (the written capture list, as a set) ≠ the closure body's free variables
        ────────────────────────────────────────────
        ill-formed; diag.capture-list-mismatch

    [Closure-Form-Borrow]
        no 'move';  {x1,…,xk} = the (validated) capture list
        c = the anonymous struct type { f1: ref<τ1,m1>, …, fk: ref<τk,mk> }
        ⟨c{ .f1 = &_{m1} x1, …, .fk = &_{mk} xk }, Σ⟩ →* ⟨temp o, Σ'⟩     -- rule.agg.struct-construct
        ────────────────────────────────────────────
        ⟨[x1,…,xk](params) body, Σ⟩ →^ℓ ⟨temp o, Σ'⟩

    [Closure-Form-Move]
        'move';  {x1,…,xk} = the (validated) capture list;  c = { f1: τ1, …, fk: τk }
        ⟨c{ .f1 = x1, …, .fk = xk }, Σ⟩ →* ⟨temp o, Σ'⟩                   -- fields stored per store:
                                                                            -- resources transferred/relocated, plain copied
        ────────────────────────────────────────────
        ⟨move [x1,…,xk](params) body, Σ⟩ →^ℓ ⟨temp o, Σ'⟩

    [Callable-Fn]        callable(type.fn<(τ1..τn) : τr>, (τ1..τn) -> τr)
    [Callable-Closure]   c formed for a closure with parameter types τ1..τn and body type τr
                         ─────────────────────────────────────
                         callable(c, (τ1..τn) -> τr)

    [Closure-Call]
        r_f = place a_c (a binding holding the closure object) or temp o_c
        ⟨borrow(a_c, exclusive), Σ⟩ →^ℓ ⟨a_self, Σ_1⟩       -- for temp o_c: from a fresh root path of o_c
        proceeds as [Call] with `self` bound to the reference value a_self
            (a fresh parameter object holding it) and body' = body with each
            free x_i replaced by *self.f_i (borrow capture) or self.f_i (move capture)
        ────────────────────────────────────────────
        ⟨r_f(e1..en), Σ⟩ →^ℓ ⟨r_b, Σ''⟩   as [Call]; a_self invalidated after [Block-Exit] of the parameter frame

A closure is a struct holding its captures (references, or moved
values) plus a call operation; construction, destruction, and
`is-resource` are exactly `spec/16` §1's struct rules — a closure that
moved a resource in owns it (destroyed at its owner's block exit) and
a borrow-capturing closure holds references that live as long as the
closure object does (D-0018; `spec/AUDIT-2.md` B-08). The capture list
is documentation the language holds you to, not a selection mechanism:
it cannot narrow, widen, or override which variables are captured or
how — that is exactly the auto-derived `x1..xk`/`m_i` facts above,
unchanged by this artifact's 2.0.0-era surface-syntax revision. Calling
a closure borrows it exclusively for the duration of the call, so a
closure cannot be called re-entrantly through a shared reference and
cannot be called while another borrow of it is live; both are
`diag.aliasing-conflict`. A `fn`-typed parameter accepts any argument
whose type satisfies `callable` with the same signature (a closure
value is passed like any struct — by move).

**Depends on:** rule.agg.struct-construct, rule.alias.borrow,
rule.fn.call, rule.value-object.store, D-0003, D-0017, D-0018

## 7. Programs and `main`

### `rule.fn.program`
**Status:** ACCEPTED

A **program** is a finite set of items (`spec/22` §3) in one root
module. It is well-formed iff every item is well-typed
(`rule.type.typing`), every static `disposition: rejected` rule in
this specification is satisfied, and it declares exactly one
`main` at the root: `fn main() : void` (or `fn main()`) or
`fn main() : u8`, with no parameters, not generic, not `extern`.

    [Program]
        Σ_0 = { items := the program's declarations, frame-stack(ℓ_0) := [], temp-scope-stack(ℓ_0) := [],
                threads := { ℓ_0 ↦ {expr: main(), handle: None, value: None} },
                every other component := ∅ }
        ────────────────────────────────────────────
        execution is the (possibly interleaved, spec/19 §1) reduction sequence from
        ⟨main(), Σ_0⟩ in thread ℓ_0

    [Program-No-Main]   disposition: rejected   no `main` at the root, or it has parameters, a return type other than `void` or `u8`, type parameters, or is `extern`   ill-formed; diag.no-main

    [Terminate-Ok]
        ⟨main(), Σ_0⟩ →* ⟨v, Σ⟩ in ℓ_0
        s = v if main's return type is u8;  s = 0 otherwise (v = ())
        ────────────────────────────────────────────
        ⟨program, Σ_0⟩ ↛↛ terminate(ok(s), Σ)
            -- no other thread is live: every handle is a resource owned by some frame of
            -- main's thread, and destroying a handle joins its thread (rule.conc.join), so
            -- main's final [Block-Exit] has already joined every thread, and run every
            -- destructor (D-0008), before s is reported

`[Fault-Unwind]` (`spec/18` §1) gives the other outcome,
`terminate(d, Σ')`. `s` is the program's **exit status** (D-0031).
The **observable behavior** of a program is: the termination outcome
(`ok(s)` or `d`), and the sequence of `extern` calls it performs
(`spec/20` §3) with their arguments — nothing else. A conforming
implementation must produce a termination outcome and extern-call
sequence that some reduction sequence of these rules produces, must
report `ok(s)` to an environment that has exit statuses as status `s`,
must make each `d` distinguishable from every `ok(s)` and from every
other `d` (the form — exit status, message — is not a language
concern), and must not perform an extern call no reduction sequence
performs. The program's arguments, which `rule.stdlib.args` reads, are
part of its environment: fixed before `main` begins, and unchanged
while it runs.

**Depends on:** term.program, term.conformance, rule.fn.call,
rule.fail.fault-unwind, rule.conc.join, D-0019, D-0031

## Change Log

- 1.5.0 — `CHG-0057` (D-0049): `rule.fn.bind-param` binds an
  exclusive reference or slice argument to a shared parameter as a
  shared reborrow of it.

- 1.4.0 — `CHG-0040` (D-0031): `rule.fn.program` allows
  `fn main() : u8`, whose value is the exit status (`[Terminate-Ok]`:
  `terminate(ok(s), Σ)`); `[Program-No-Main]` also rejects a root
  `main` with parameters or another return type; the program's
  arguments belong to its environment.
- 1.3.0 — `CHG-0028`: `rule.fn.program` gains `[Program-No-Main]`,
  the `disposition: rejected` form of the well-formedness sentence
  that already required `main` at the root; it names `diag.no-main`
  (new, `spec/registry/diagnostics.md` 1.6.0). No other change.
- 1.2.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"): §5's
  generic-item illustration and §7's `main` signature re-spelled to
  the current surface syntax (`τ p` parameters, `: void`); `fn main()
  : unit` was not a spelling `spec/22` 2.6.0 admits.
- 1.2.0 — `CHG-0002`: closure surface syntax changed from
  `move? |params| expr` to `move? [captures](params) block`
  (`spec/22` 2.1.0). `rule.fn.closure` gains `[Closure-Capture-List-
  Rejected]`: the written capture list must name exactly the body's
  derived free-variable set (`diag.capture-list-mismatch`, new). This
  is the one new rejection case; capture-mode inference (D-0017),
  ownership/move behavior, borrowing, lifetimes, resource handling,
  and type inference are all unchanged — the list documents an
  already-derived fact and is checked against it, never selects it.
  `type.fn`'s own illustration (§1) and `[Callable-Fn]`'s first
  argument re-spelled to `:` for the same reason as `CHG-0001`'s other
  fixes; `callable(...)`'s bare `(τ1..τn) -> τr` signature argument is
  unaffected (never literal surface syntax).
- 1.1.0 — `CHG-0001`: illustrative fragments and `main`'s signature
  re-spelled to `spec/22` 2.0.0 syntax; no rule semantics changed.
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-06, B-07,
  B-08, B-17, B-18): `[Call]` exits only the parameter frame and takes
  a result-form body value; `[Return]` uses `rule.control.unwind` to
  the body frame; parameter binding is `store`; closures construct a
  struct temporary and are called through an exclusive self-borrow;
  `rule.fn.program` defines `Σ_0`, `main`, termination, and observable
  behavior. All entities `ACCEPTED`.
- 0.4.0 and earlier — superseded.
