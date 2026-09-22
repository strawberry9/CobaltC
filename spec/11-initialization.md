# CobaltC Initialization

Status: normative artifact
Version: 1.3.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.init.*`)
Governed by: `CobaltC_Master_Instructions.md` §17 (Initialization and
Valid State), §23
Realizes: D-0019

## 1. Baseline (already in force)

- `InitState ::= uninitialized | valid` (`spec/04`); establishment
  sets `uninitialized`; a whole-object `[Write]` or aggregate
  construction sets `valid`; `[Read]` and projection `[Write]` require
  `valid` (`spec/05`).
- Partial initialization is rejected by decision (D-0019): a field
  of an uninitialized aggregate cannot be written
  (`[Write-Partial-Init]`); the aggregate is initialized whole.
- A resource's value is never read out; `consumed` is unreachable
  (`spec/05` `[Read-Resource-Rejected]`).

## 2. Local declarations

### `rule.init.let`
**Status:** ACCEPTED

    [Let]
        τ = the written type (rule.type.typing)
        ⟨e, Σ⟩ →* ⟨r, Σ1⟩                       -- e evaluated with expected type τ (rule.type.expected)
        r : τ                                     -- value, place, or temp of type τ
        ⟨store(binding(x), r), Σ1⟩ →^ℓ ⟨(), Σ2⟩  -- rule.value-object.store
        ────────────────────────────────────────────
        ⟨τ x = e; , Σ⟩ →^ℓ ⟨(), Σ2⟩

    [Let-Auto]   ⟨auto x = e; , Σ⟩ ≡ ⟨τ x = e; , Σ⟩   where τ = the synthesized type of e (rule.type.typing)

    [Let-Uninit]
        ⟨establish(τ), Σ⟩ →^ℓ ⟨o, Σ1⟩            -- rule.value-object.object-establish
        ⟨bind(x, o), Σ1⟩ →^ℓ ⟨a, Σ2⟩             -- rule.value-object.binding-form
        ────────────────────────────────────────────
        ⟨τ x; , Σ⟩ →^ℓ ⟨(), Σ2⟩

    [Assign-Reestablish]   -- D-0033 (1)
        x a binding declared with type τ, in the frame F that declared it
        ⟨e, Σ⟩ →* ⟨r, Σ1⟩                       -- e evaluated with expected type τ (rule.type.expected)
        ¬temporally-valid(binding(x), Σ1) ∨ ¬alive(object(x), Σ1)     -- x's value was moved away or ended,
                                                                     -- before e or while it was evaluated
        ⟨establish(τ), Σ1⟩ →^ℓ ⟨o, Σ2⟩;  ⟨bind(x, o) in F, Σ2⟩ →^ℓ ⟨a, Σ3⟩   -- as [Let-Uninit]
        ⟨store(a, r), Σ3⟩ →^ℓ ⟨(), Σ4⟩             -- rule.value-object.store; o is uninitialized, so
                                                    -- ¬live-resource-at holds
        ────────────────────────────────────────────
        ⟨x = e, Σ⟩ →^ℓ ⟨(), Σ4⟩

    [Let-Destructure]   -- Name{f1, …, fn} = e;  every field of Name, each once (spec/22 §2, D-0044)
        ⟨e, Σ⟩ →* ⟨r, Σ1⟩ in place position;  r = place a with base(a,Σ1) = None, or r = temp o
        o = the object;  type-of(o)'s fields are exactly f1..fn : τ1..τn;  Name has no destructor;
        init-state(o,Σ1) = valid;  solitary(a) if r is a place
        for i = 1..n in order, Σ_{i−1} → Σ_i by:
            ¬is-resource(τi):   a_i = projection of the object's root path at fi;  ⟨store(binding(fi), place a_i), Σ_{i−1}⟩ →^ℓ ⟨(), Σ_i⟩
            is-resource(τi):    ⟨relocate-out(o, fi), Σ_{i−1}⟩ →^ℓ ⟨temp o_i, Σ'⟩;  ⟨store(binding(fi), temp o_i), Σ'⟩ →^ℓ ⟨(), Σ_i⟩
        ⟨destroy(root path of o), Σ_n⟩ →^ℓ ⟨(), Σ'⟩       -- ends the container; every field has left it
        ────────────────────────────────────────────
        ⟨Name{f1, …, fn} = e; , Σ⟩ →^ℓ ⟨(), Σ'⟩

    [Let-Destructure-Fields]   disposition: rejected
        the names are not exactly Name's fields, each once
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

    [Let-Destructure-Destructor]   disposition: rejected
        Name has a destructor (fn Name::drop): it would run on a struct its fields had left
        ────────────────────────────────────────────
        ill-formed; diag.move-out-of-field (static)

A local declaration is `store` into a fresh binding (D-0019): a scalar
is written into a fresh object; a non-resource place is copied; a
resource place is transferred (its source invalidated —
`ex.use-after-move`); a temporary (an aggregate literal, a call
result) is adopted. `auto x = e;` (`[Let-Auto]`) is sugar for `τ x =
e;` with `τ` synthesized from `e`, exactly as `[If-No-Else]`
(`spec/14` §3) desugars a missing `else`; there being nothing to
synthesize from, `τ x;` (uninitialized) always requires the written
type.

`[Assign-Reestablish]` (D-0033): an assignment `x = e` to a whole
binding whose value has gone — moved away, or ended by `drop` —
gives it a new value the way its declaration gave it the first: a new
object, bound to `x` in the frame that declared `x`, so it ends with
`x`'s scope. It also covers `x = f(x)`, where `e` moves the old value
away. Writing a whole binding that still holds a live resource
remains `[Write-Resource-Overwrite-Rejected]`, so a value is never
silently lost.

**Depends on:** rule.value-object.store,
rule.value-object.object-establish, rule.value-object.binding-form,
rule.type.typing, rule.type.expected, D-0019, D-0033
**Affects:** state.objects, state.bindings, state.holder

## 3. Static refinement: definite assignment

### `rule.init.definite-assignment`
**Status:** ACCEPTED

    [Definite-Assignment-Rejected]   disposition: rejected
        a read (rule.value-object.read) or projection write through binding x
        rule.control.flow-analysis yields refuted or unknown for
            φ = "x's object is valid" at that program point
        ────────────────────────────────────────────
        ill-formed; diag.use-of-uninitialized

Definite assignment is `rule.control.flow-analysis` (`spec/14` §6)
instantiated with fact `φ_x` = "the object bound to `x` has
`init = valid`", established by `[Let]`, `[Store-Binding-*]`, and a
whole-object `[Write]` to `x`, never invalidated within a function.
Unlike most guarded rules, an `unknown` result here is a static
rejection, not a dynamic fallback: the fact depends only on the
function's own syntactic control flow, so `unknown` can only arise
from a path on which no write occurs. `[Read-Uninitialized]`'s dynamic
check remains as defense in depth and is unreachable in a well-formed
program.

**Depends on:** inv.initialization-validity,
rule.control.flow-analysis, rule.value-object.read,
rule.value-object.write

## Change Log

- 1.3.0 — `CHG-0052` (D-0044): `[Let-Destructure]` takes every field of
  a struct, not only a single one; `[Let-Destructure-Fields]` and
  `[Let-Destructure-Destructor]`.

- 1.2.0 — `CHG-0042` (D-0033): `rule.init.let` gains
  `[Assign-Reestablish]`: an assignment to a whole binding whose value
  was moved away or ended gives it a new object in its own frame.

- 1.1.0 — `CHG-0001`: `[Let]`/`[Let-Uninit]`/`[Let-Destructure]`
  restated over `spec/22` 2.0.0 syntax (`τ x = e;`, `τ x;`,
  `Name{f} = e;`), with a new `[Let-Auto]` desugaring rule for
  `auto x = e;`. `rule.init.let`'s id and store-into-a-fresh-binding
  semantics are unchanged — this quotes the current concrete syntax
  in place of the retired `let` keyword, nothing more.
- 1.0.0 — Rewritten per D-0019 (`spec/AUDIT-2.md` B-05, B-07):
  `[Let]` is `store`; `[Let-Value]`/`[Let-Resource]` merged;
  definite assignment stated against `rule.control.flow-analysis`
  with its static-rejection policy explicit. All entities `ACCEPTED`.
- 0.3.0 and earlier — superseded.
