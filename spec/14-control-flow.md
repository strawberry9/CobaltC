# CobaltC Control Flow

Status: normative artifact
Version: 1.9.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.control.*`)
Governed by: `CobaltC_Master_Instructions.md` §12, §23
Realizes: D-0008 (scope-end destruction), D-0018, D-0019

## Purpose

Defines blocks and frames, statement scopes, `if`, `while`, `break`,
`continue`, the unwinding fold that `return`/`break`/`continue`/fault
termination reuse, and the one static analysis
(`rule.control.flow-analysis`) every "`discharge: static` where
provable" side-condition in this specification refers to.

## 1. Blocks and frames

### `rule.control.block`
**Status:** ACCEPTED

    block ::= '{' statement* expr? '}'          -- spec/22 §2

    [Block-Enter]
        f ∉ any Σ.frame-stack(·)
        body = stmt-scoped(s1, discard) ; … ; stmt-scoped(sn, discard) ; stmt-scoped(e, keep)
                                                                            -- e absent ⇒ stmt-scoped((), keep)
        ────────────────────────────────────────────
        ⟨{ s1 … sn e }, Σ⟩ →^ℓ ⟨block'(f, body), Σ[ frame-stack(ℓ) := f :: Σ.frame-stack(ℓ) ]⟩

    [Block-Step]   via [Context] with E = block'(f, □)

    [Block-Exit]
        head(Σ.frame-stack(ℓ)) = f;  r ∈ {v, temp o}      -- the body's result, already in result form (§1a)
        Owned = { o | owned-by-frame(o, f, Σ) } \ objs-in(r)
        o_1, …, o_n = Owned ordered by descending origin(o_i, Σ)      -- last established first
        ────────────────────────────────────────────
        ⟨block'(f, r), Σ⟩ →^ℓ ⟨r, Σ_n[ frame-stack(ℓ) := tail(Σ.frame-stack(ℓ)),
                                       bindings := Σ_n.bindings restricted to frames ≠ f ]⟩
        where Σ_0 = Σ and
            Σ_i = result of ⟨destroy(holder(o_i, Σ)), Σ_{i-1}⟩      if o_i ∈ Σ_{i-1}.destruction-obligations
                                                                        -- rule.resauth.destroy
            Σ_i = result of ⟨end-object(o_i), Σ_{i-1}⟩                otherwise
                                                                        -- rule.value-object.object-end

A block pushes a frame, evaluates its statements each in a statement
scope (§1a), and on exit ends every object its frame owns — resources
by `destroy` (D-0008: automatic, deterministic, reverse order), plain
objects by `end-object` — except the block's own result, which is a
value or a temporary that flows to the enclosing construct (D-0019).
Ending the owned objects is what invalidates the frame's bindings and
every reference they held (`rule.value-object.object-end`);
`destroy`'s `solitary` precondition holds for each `o_i` because every
path to it other than its owner is either a temporary already ended
at its statement's exit (§1a), a reference held by an object of this
frame ended earlier in this same fold (descending origin order ends
holders before what they hold whenever the holder was established
later — a reference cannot be stored into an object established
before the referent's binding without `rule.temporal.ref-escape`
rejecting it or `[Block-Exit]` of the referent's own inner frame having
already ended it), or a reference held by an outer object, which
`rule.temporal.ref-escape` rejects statically and
`[Destroy-Not-Solitary]` catches dynamically.

`unsafe { … }` is a block with `spec/20` §1's additional static
permission; `spec/22` §2.

**Depends on:** inv.resource-authority, inv.temporal-validity, D-0008,
D-0016, D-0018, D-0019, rule.resauth.destroy,
rule.value-object.object-end
**Affects:** state.frame-stack, state.bindings, state.objects

## 1a. Statement scopes

### `rule.control.stmt`
**Status:** ACCEPTED

    [Stmt-Enter]
        t fresh;  k ∈ {keep, discard}
        ────────────────────────────────────────────
        ⟨stmt-scoped(e, k), Σ⟩ →^ℓ ⟨stmt-scoped'(t, k, e), Σ[ temp-scope-stack(ℓ) := (t, current-frame(ℓ,Σ)) :: … ]⟩

    [Stmt-Step]   via [Context] with E = stmt-scoped'(t, k, □)

    [Stmt-Exit]
        current-scope(ℓ, Σ) = t
        k = keep:     ⟨store(result, r), Σ⟩ →^ℓ ⟨r', Σ_0⟩       -- rule.value-object.store: place → value/temp
        k = discard:  r' = (),  Σ_0 = Σ                           -- the statement's value is dropped; a resource place
                                                                    -- is left owned by its binding, a temporary is destroyed below
        Unheld = { a | Σ_0.access-paths(a).temp-scope = t ∧ Σ_0.access-paths(a).held-by = ∅
                     ∧ Σ_0.access-paths(a).valid ∧ a ∉ ran(Σ_0.holder) } \ refs-in(r')
                 -- an owner path (a binding's root) is never a temporary of its own declaration statement
        Temps = { o | temporary(o, Σ_0) ∧ Σ_0.objects(o).temp-scope = t } \ objs-in(r')
        o_1, …, o_n = Temps ordered by descending origin
        t_parent = the scope beneath t in Σ.temp-scope-stack(ℓ), or ⊥ if none
        ────────────────────────────────────────────
        ⟨stmt-scoped'(t, k, r), Σ⟩ →^ℓ
            ⟨r', Σ_n[ temp-scope-stack(ℓ) := tail(…),
                     objects(o).temp-scope := t_parent          for o ∈ objs-in(r'),
                     access-paths(a).temp-scope := t_parent     for a ∈ refs-in(r') with held-by = ∅ ]⟩
        where Σ_1' = Σ_0[ access-paths(a).valid := false  for a ∈ Unheld ]
              and Σ_i (i = 1..n) = destroy/end o_i from Σ_{i-1} (Σ_0 read as Σ_1') exactly as in [Block-Exit]

At the end of every statement (and of a block's trailing
expression): the result is put in result form (a plain place is read;
a resource place is released into a temporary); every reference formed
during the statement that nothing holds and that is not the result is
invalidated; every temporary created during the statement that is
not the result is destroyed or ended; and the result's own temporary
or unheld reference is re-stamped to the parent scope so that, if the
enclosing statement does not store it either, that statement's exit
ends it. This is what makes `push(&mut v, 1); push(&mut v, 2);`
well-formed (the first borrow dies at `;`), `make();` leak-free (the
temporary dies at `;`), and `auto r = &v;` durable (the borrow is held
by `r`'s object).

**Depends on:** D-0018, D-0019, rule.value-object.store,
rule.resauth.destroy, rule.value-object.object-end
**Affects:** state.temp-scope-stack, state.access-paths, state.objects

## 2. Unwinding

### `rule.control.unwind`
**Status:** ACCEPTED

    unwind-to(f_target, r, Σ):        -- exits every scope and frame pushed at or above f_target
        while true:
            (t, f_t) = head(Σ.temp-scope-stack(ℓ)) or ⊥;   f = current-frame(ℓ, Σ)
            if t ≠ ⊥ and f_t = f:      Σ := result of [Stmt-Exit] on (t, keep, r) with r' := r (no re-store)
            else if f = f_target:      Σ := result of [Block-Exit] on (f, r);  return Σ
            else:                       Σ := result of [Block-Exit] on (f, r)

A LIFO fold over the interleaved statement scopes and frames of the
current thread, exempting `r` from every cleanup, down to and
including `f_target`. Used by `[Return]` (`spec/15` §4), `[Break]`/
`[Continue]` (§5), and `[Fault-Unwind]` (`spec/18` §1). `r` has
already been put in result form by whichever rule invokes the fold.

**Depends on:** rule.control.block, rule.control.stmt

## 3. `if`

### `rule.control.if`
**Status:** ACCEPTED

    [If-True]     ⟨if (true) b1 else b2, Σ⟩ → ⟨b1, Σ⟩
    [If-False]    ⟨if (false) b1 else b2, Σ⟩ → ⟨b2, Σ⟩
    [If-No-Else]  ⟨if (c) b, Σ⟩ → ⟨if (c) b else {()}, Σ⟩

The condition reduces first (`E ::= if (E) b1 else b2`); the chosen
branch is an ordinary block. Type: `rule.type.typing` `[T-If]`.

**Depends on:** type.bool, rule.control.block

## 4. `while`, `break`, `continue`

### `rule.control.while`
**Status:** ACCEPTED

    [While-Test]
        ⟨c, Σ⟩ →* ⟨v, Σ1⟩     -- c evaluated in a statement scope of its own: stmt-scoped(c)
        ────────────────────────────────────────────
        ⟨while (c) b, Σ⟩ →^ℓ ⟨while'(v, c, b), Σ1⟩

    [While-True]    ⟨while'(true, c, b), Σ⟩ → ⟨loop-body(b, c) ; while (c) b, Σ⟩
    [While-False]   ⟨while'(false, c, b), Σ⟩ → ⟨(), Σ⟩

`loop-body(b, c)` is the block `b` evaluated as a block (§1), whose
frame is tagged as a *loop frame* for `[Break]`/`[Continue]`; when it
finishes normally, `[Seq-Value]` discards its `()` and the loop
re-tests `c` (`spec/AUDIT-2.md` B-09: the body's frame is exited
before the next iteration starts; nothing nests). `c` is re-evaluated
from its syntax each iteration.

    [Break]
        f_w = the innermost loop frame in Σ.frame-stack(ℓ)          -- a static syntactic fact:
                                                                     -- break must be lexically inside a while
        Σ' = unwind-to(f_w, (), Σ)
        ────────────────────────────────────────────
        ⟨E_w[ loop-body(E_b[break], c) ; while (c) b ], Σ⟩ →^ℓ ⟨E_w[()], Σ'⟩
            where E_b contains no loop-body(…) and E_w is any context

    [Continue]
        f_w, Σ' as in [Break]
        ────────────────────────────────────────────
        ⟨E_w[ loop-body(E_b[continue], c) ; while (c) b ], Σ⟩ →^ℓ ⟨E_w[while (c) b], Σ'⟩

`break` exits every scope and frame up to and including the loop body
and replaces the whole loop with `()`; `continue` does the same and
replaces it with a fresh test of the loop. Both are `never`-typed
(`spec/12` §5) and are ill-formed outside a `while`
(`diag.break-outside-loop`, static).

**Depends on:** D-0008, rule.control.block, rule.control.unwind,
type.bool

### `rule.control.for`
**Status:** ACCEPTED

    [For]
        ⟨for (i; c; s) b, Σ⟩ ≡ ⟨{ i; while (c) b }, Σ⟩ with the step s:
            s runs after each loop-body(b, c) that finishes normally or by [Continue],
            evaluated in a statement scope of its own (stmt-scoped(s)), before c is tested again;
        a missing i is nothing, a missing c is `true`, a missing s is nothing

    [For-Continue]
        f_w, Σ' as in [Break]
        ────────────────────────────────────────────
        ⟨E_w[ loop-body(E_b[continue], c) ; while (c) b with step s ], Σ⟩ →^ℓ ⟨E_w[s ; while (c) b with step s], Σ'⟩

`for (i; c; s) b` (D-0035) is C's loop. `i` is a declaration or an
expression statement; a binding it declares belongs to the `for`'s
own block, so it is out of scope after the loop and a `b` that
declares the same name shadows it without affecting `s`. `continue`
in `b` runs `s` (`[For-Continue]`), and `break` leaves without it.
The flow analysis (§6) reaches the loop head from the body's end and
from every `continue` through `s`.

**Depends on:** rule.control.while, rule.control.stmt, D-0035

### `rule.control.foreach`
**Status:** ACCEPTED

`foreach (x in c) b` visits the elements of a collection `c` — a
`Vec<T>`, an `array<T, N>`, a slice `slice<T, m>` (`spec/16` §3a,
D-0047), a `HashSet<K>` or a `HashMap<K, V>` (`spec/21`) — in order
(a slice is already a borrow, and is looped over in its own mode): index order for a `Vec` or an array, insertion
order (`rule.stdlib.hashmap`) for a set or a map. How the loop holds
the collection is written, not inferred:

| Written | The loop holds | Each element is | After the loop |
|---|---|---|---|
| `foreach (x in c)` | `c`, moved into it | the element, moved out: `T` | `c` is moved-from |
| `foreach (x in &c)` | `&c` | `ref<T, shared>` | `c` is usable |
| `foreach (x in &mut c)` | `&mut c` | `ref<T, exclusive>` | `c` is usable |

A `c` that is itself a reference (`ref<C, m>`) is looped over as `&`
(`m` shared) or `&mut` (`m` exclusive) of what it refers to. With two
names, `foreach (k, x in c)`, `k` is the element's position (`usize`)
for a `Vec`, array or set, and the key for a map — a `ref<K, shared>`,
or the key itself when the map is consumed; a map must be looped over
with two names, and the value is `x`. A map also takes three,
`foreach (i, k, v in m)`: `i` is the entry's position (D-0044).

    [Foreach-Borrowed]   ⟨foreach (k, x in &m c) b, Σ⟩ ≡
        ⟨{ auto $c = &m c; for (usize $i = 0; $i < len($c); $i += 1)
             { auto k = key($c, $i); auto x = at_m($c, $i); b } }, Σ⟩

    [Foreach-Consumed]   ⟨foreach (k, x in c) b, Σ⟩ ≡
        ⟨{ auto $d = drain(c); for (usize $i = 0; $i < total($d); $i += 1)
             { auto k = take_key($d, $i); auto x = take($d, $i); b } }, Σ⟩

    [Foreach-Not-Iterable]   disposition: rejected
        c's type is none of the four collections (or a reference to one); or `&mut` of a HashSet
        (its keys are not changed in place); or a HashMap with one name; or three names, not a HashMap
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

(with one name, drop `auto k = …`). `$c`, `$d` and `$i` are bindings
no program can name. `len`, `key`, `at_m`, `drain`, `total`,
`take_key` and `take` are, for each collection, the functions of
`spec/21` that do what their names say: `Vec::len`,
`Vec::index_shared`/`Vec::index_exclusive`, an array's `&c[i]`,
`HashSet::at`, a map's `keys`/`values` vectors; for a consumed `Vec`,
set or map, a `std`-private holder (`Drain`, `SetDrain`, `HashDrain`,
`spec/21` §2g) that owns the collection, moves element `$i` out when it
is taken, and destroys the elements not yet taken when it is
destroyed. A consumed array is the array itself, element `$i` read
from it by value (so an array of resources is looped over by
reference).

- **Nothing new underneath.** Every step is a rule that already
  exists: the loop's borrow of `c` lasts the whole loop, so changing
  `c` in `b` (`Vec::push(&mut c, …)` in a `foreach (x in &c)`) is the
  aliasing conflict it would be anywhere (`rule.alias.*`); using `c`
  after a consuming loop is `diag.stale-binding`.
- **Leaving early.** `break`, `return`, `?` or a fault leave the
  consuming loop's holder to be destroyed at its block's end, which
  destroys the elements not yet taken exactly once; the ones already
  taken belong to the body.
- **`continue`** runs the step `$i += 1`, as a `for`'s does
  (`[For-Continue]`).

**Depends on:** rule.control.for, rule.stdlib.vec, rule.stdlib.hashmap,
rule.type.expected, D-0042

## 5. Static reasoning: lexical facts

`decl-block`, lexical enclosure, and the syntactic control-flow graph
used below are all functions of program text: block `B1` lexically
encloses `B2` iff `B2`'s text is nested in `B1`'s braces; a *program
point* is a position between two statements of a block, or the entry
or exit of a block; the CFG of a function body has an edge from each
point to its syntactic successor, two successors at `if`, a back edge
from the end of a `while` body to its test, and edges from
`return`/`break`/`continue` to the function exit / loop exit / loop
test respectively.

## 6. The static/dynamic boundary

### `rule.control.flow-analysis`
**Status:** ACCEPTED

Every side-condition elsewhere written "`discharge: static`
(rule.control.flow-analysis; dynamic where unknown)" is discharged by
exactly this analysis, so that the set of programs rejected at compile
time is the same for every conforming implementation (Master
Instructions §12; `spec/AUDIT.md` A-07, `spec/AUDIT-2.md` B-10).

**Facts.** For a function body, the finite family `Φ` contains, for
every binding `x` declared in it (parameters included) and every
reference-typed binding `r`:

    valid(x)        -- x's path is temporally valid (not moved-from; object not ended)
    init(x)         -- x's object has init = valid
    deriv(r, x.π, m) -- r currently holds a reference derived from x's object at syntactic
                       projection path π (ε for &x, f for &x.f, f.g for &x.f.g, [i] for a
                       literal index) with mode m (formed by a visible borrow of that place,
                       or a D-0011-elided call on such); two paths π1, π2 "overlap" iff one is
                       a prefix of the other, with literal indices compared by value and a
                       non-literal index overlapping every index
    escaped(x)      -- a reference to x's object has been stored somewhere the analysis
                       does not track (an aggregate field, a spawn argument, a call
                       argument other than the elided case, an assignment through *e)

**Lattice.** Each fact takes a value in `{T, F, ?}` with `?` the top
(join of `T` and `F` is `?`); a state is a map `Φ → {T, F, ?}`; the
analysis is a forward dataflow over §5's CFG from the body's entry
state (parameters: `valid = T`, `init = T`; every other binding `F`
until declared; every `deriv`/`escaped` `F`), joining pointwise at
merge points and iterating loops to the least fixed point (finite
lattice, monotone transfer functions).

**Transfer functions** (per statement/expression kind, applied in
evaluation order within the statement):

| Construct | Effect on state |
|---|---|
| `τ x = e;` / `auto x = e;` / `τ x;` | `valid(x) := T`; `init(x) := T` (with initializer) / `F` (without) |
| whole-object write `x = e` | `init(x) := T`; `valid(x) := T` (`spec/11` `[Assign-Reestablish]`: a gone value is re-established) |
| `auto r = &_m x.π;` or `r = &_m x.π` | `deriv(r, x.π, m) := T`; other `deriv(r, ·, ·) := F` |
| `auto r = f(&_m x…)` (D-0011 elided) | as the line above |
| resource-typed whole binding `x` consumed: as an argument; as a local-declaration, field, element, or payload initializer (`auto y = x`, `Name { .f = x }`, `Vi(x)`); as `return x` or a trailing result; as a `move` capture; by a `match (x)` arm that binds a resource payload (D-0049: within that arm); as the source of `Name{f} = x`; or as the value of a raw write `*p = x` | `valid(x) := F`; every `deriv(·, x, ·) := F` |
| `drop(x)` | `valid(x) := F`; every `deriv(·, x, ·) := F` |
| `&x…` / `&mut x…` in any position other than the two `deriv` lines | `escaped(x) := T` |
| `r = e` for reference-typed `r` from anything but a visible borrow | every `deriv(r, ·, ·) := F`; `escaped(x) := T` for the `x` the analysis cannot exclude |
| block exit | for every `r` declared in the block: `deriv(r, ·, ·) := F`; for every `x` declared in it: `valid(x) := F` |
| `spawn(…, &_m x …)` | `escaped(x) := T` |
| anything else | no change |

**Discharge.** A guarded side-condition `⟦ φ ⟧` at a program point,
where `φ` is one of the following, is resolved from the state there:

| Side-condition | `T` (proven) when | `F` (refuted) when |
|---|---|---|
| `temporally-valid(a_x)` | `valid(x) = T` | `valid(x) = F` |
| `init-state(o_x) = valid` | `init(x) = T` | `init(x) = F` |
| `authority(ℓ, destroy, o_x)` | `valid(x) = T` and `x` resource-typed | `valid(x) = F` |
| `¬clash(a, m)` for an access at place `x.π` | `escaped(x) = F` and every `deriv(r, x.π', m') = F` for which `π, π'` overlap and `¬permitted(m, m')` | some such `deriv(r, x.π', m') = T` |
| `solitary(a_x)` | `escaped(x) = F` and every `deriv(·, x.·, ·) = F` | some `deriv(·, x.·, ·) = T` |
| `¬live-resource-at(a_x)` for a whole-object write to resource-typed `x` | `init(x) = F` | `valid(x) = T` and `init(x) = T`, and every value of `x`'s type owns something (`spec/05` `owns`; D-0049) — otherwise the value decides, dynamically |
| `¬clash` / `solitary` for an access whose root is `*r` or a temporary | never `T` | never `F` |
| a value-range condition (`v2 ≠ 0`, `0 ≤ i < N`, `r ∈ represented-domain`) | the operands are literals or bound by a local declaration to literals with no intervening write, and the condition holds of them | likewise, and it fails |

Outcome per result: **proven** → the condition is discharged
statically, no runtime check; **refuted** → the program is ill-formed
(`disposition: rejected` for that instance, with the rule's own
diagnostic marked *static*); **unknown** (`?`, or a shape the table
does not cover) → `discharge: dynamic`: the runtime check named by the
rule is performed and its `↛` diagnostic applies if it fails.
`rule.init.definite-assignment` is the one exception where `unknown`
rejects (`spec/11` §3). `rule.temporal.ref-escape` is a separate
lexical rule, not an instance of this analysis.

**Scope.** Intraprocedural and syntactic: a call is opaque except for
its declared signature and the D-0011 elision; no value-range
reasoning beyond literals. A conforming implementation must reject
exactly the refuted instances and must not reject any proven or
unknown instance; it may report additional *warnings* from stronger
analyses but may not change acceptance.

**A confirmed trade, not a gap:** the "`&x…`/`&mut
x…` in any position other than the two `deriv` lines → `escaped(x) :=
T`" row fires for *any* call whose return type is not itself a
reference, including an ordinary library call like `Vec::push` — so
`escaped(nums)` becomes `T` after the very first `Vec::push(&mut nums,
…)` in a function, and every later access to `nums` can only ever be
*proven* safe when `escaped(x) = F`. This does not reject anything:
`conf.sequential-exclusive-borrows-ok` (two sequential `Vec::push`
calls, then a read) accepts the program, because nothing ever sets a
competing `deriv` fact to `T` either, so the outcome is `unknown` →
`discharge: dynamic`, not refuted. The cost is exclusively a lost
opportunity to prove the second and later accesses statically, in
exchange for a syntactic, intraprocedural analysis with no
value-range or call-graph reasoning (this section's own stated
`Scope`). No sharper rule is proposed: distinguishing "this call
definitely cannot have stored a reference to `x` anywhere" from
"might have" in general requires exactly the interprocedural or
alias-summary reasoning `rule.control.flow-analysis` is deliberately
not (Master Instructions §9's conceptual-economy priority over
`discharge: static`'s convenience, and this section's `Scope` already
rules out the alternative for other constructs).

**Depends on:** D-0018, rule.temporal.ref-escape,
rule.init.definite-assignment
**Affects:** rule.value-object.binding-lookup, rule.value-object.read,
rule.value-object.write, rule.alias.borrow, rule.resauth.transfer,
rule.resauth.destroy, rule.agg.index, rule.arith.checked,
rule.arith.div, rule.arith.shift, rule.arith.neg, rule.arith.convert

## Change Log

- 1.9.0 — `CHG-0057` (D-0049): `¬live-resource-at` is refuted
  statically only for a type every value of which owns something; a
  `match (x)` consumes `x` only in an arm that moves a resource
  payload out.

- 1.8.0 — `CHG-0055` (D-0047): `foreach` over a slice.

- 1.7.0 — `CHG-0052` (D-0044): `rule.control.foreach` takes a third
  name over a map, the entry's position.

- 1.6.0 — `CHG-0050` (D-0042): `rule.control.foreach`
  (`[Foreach-Borrowed]`, `[Foreach-Consumed]`, `[Foreach-Not-Iterable]`).

- 1.5.0 — `CHG-0044` (D-0035): `rule.control.for` (`[For]`,
  `[For-Continue]`).

- 1.4.0 — `CHG-0042` (D-0033): a whole-object write `x = e` also sets
  `valid(x) := T`, since it re-establishes a binding whose value was
  moved away or ended.

- 1.3.1 — Non-normative (`CHG-0016` §"Hygiene"): §6 gains a paragraph
  confirming `escaped(x)`'s call-boundary conservatism is an intended
  usability trade, not a gap; no rule semantics changed.
- 1.3.0 — `CHG-0010`: the consuming-use transfer row now lists every
  construct that transfers, relocates, or destroys a whole resource
  binding — `match (x)`, `Name{f} = x`, `*p = x`, payload initializers
  were missing, so `valid(x)` stayed `T` and a later use would have
  been "proven" valid with its runtime check elided
  (`conf.match-resource-payload-transfers`); a discharge row for
  `¬live-resource-at` added (`[Write]` cites this analysis for it, and
  `conf.overwrite-live-resource-rejected` is stated static).
- 1.2.1 — Non-normative (consistency pass, `CHG-0009` §"Hygiene"): one
  `let` mention in §6's discharge table re-worded.
- 1.2.0 — `CHG-0003`: `[If-*]`/`[While-*]`/`[Break]`/`[Continue]`
  restated with mandatory parentheses around the condition
  (`if (c) b`, `while (c) b`), matching `spec/22` 2.2.0. No rule
  semantics changed.
- 1.1.0 — `CHG-0001`: the flow-analysis transfer-function table and one
  illustrative fragment re-spelled to `spec/22` 2.0.0 syntax; no rule
  semantics changed.
- 1.0.0 — Rewritten per D-0018/D-0019 (`spec/AUDIT-2.md` B-03, B-04,
  B-06, B-09, B-10, B-13): `[Block-Exit]` ends owned objects (no
  separate stale pass); `[Stmt-Exit]` puts results in result form,
  invalidates unheld references, destroys unstored temporaries;
  `rule.control.unwind` shared by return/break/continue/fault;
  `while` as a primitive with the body frame exited per iteration;
  `[If-No-Else]`; `rule.control.flow-analysis` given facts, lattice,
  transfer functions, discharge table, and outcome policy. All
  entities `ACCEPTED`.
- 0.5.0 and earlier — superseded.
