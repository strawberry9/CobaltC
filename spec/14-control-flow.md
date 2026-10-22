# CobaltC Control Flow

Status: normative artifact
Version: 1.29.0
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
        o_1, …, o_n = Owned ordered by descending formed-at of holder(o_i, Σ)
                                          -- last bound first: reverse declaration order, an object moved
                                          -- into a later binding ending with it (D-0008, D-0233)
        ────────────────────────────────────────────
        ⟨block'(f, r), Σ⟩ →^ℓ ⟨r, Σ_n[ frame-stack(ℓ) := tail(Σ.frame-stack(ℓ)),
                                       bindings := Σ_n.bindings restricted to frames ≠ f ]⟩
        where Σ_0 = Σ and
            Σ_i = result of ⟨destroy(holder(o_i, Σ)), Σ_{i-1}⟩      if o_i ∈ Σ_{i-1}.destruction-obligations
                                                                        -- rule.resauth.destroy
            Σ_i = result of ⟨end-object(o_i), Σ_{i-1}⟩                otherwise
                                                                        -- rule.value-object.object-end
            -- each end-object and destroy here takes E = refs-in(r): a reference
            -- the block's result holds is not invalidated by the end of the object
            -- that held it, here or in a destructor's own ends (D-0219)

    [Ref-Binding-Last-Use]   (D-0111, D-0221)
        x declared by a `let` among s1 … sn, at s_d, of a type that holds a reference (ref<τ, m>,
        slice<τ, m>, StringView, or an aggregate, option or array holding one) and holds no
        resource and no fn value (D-0133);
        s_k (k ≥ d) the last of s_d … sn that mentions x, k < n or e present; e does not mention x;
        no closure in the block captures x; x declared once in the block;
        s_j (j ≥ k) the first statement after which no binding that may hold a reference, other
        than x, and mentioned in a statement with x since s_d (or declared by s_d), is mentioned
        again in s_{j+1} … sn or e;  j < n or e present;
        (D-0275: a binding y of type ref<τ, m>, slice<τ, m> or StringView, τ holding no reference
        and no fn value, counts through a statement s_i only if s_i declares y, assigns y whole,
        borrows y whole exclusively (&mut y), or has a closure that captures y)
        after s_j, rule.control.flow-analysis's facts root no other binding's reference in x:
        deriv(y, x.π, m) = F and deriv(y, *x.π, m) = F for every y ≠ x, π, m
        (x passed whole to `spawn` is borrowed through: the lend is a borrow of *x the handle
        holds, §6, D-0219)
        ────────────────────────────────────────────
        after stmt-scoped(s_j, discard):  ⟨end-object(o_x), Σ⟩ as in [Block-Exit], and x is unbound
        -- the path x held is held no more (`held-by`); nothing a resource owns ends earlier
        -- bindings ending after the same s_j end one at a time, in the order of their
        -- declarations, each judged after the ends before it; one whose facts still root
        -- another binding's reference after s_j is not ended before its block's exit

The end is deferred, never early: a binding `x` that shares a
statement with another binding that may hold a reference — a struct
built from it, an `Option` made of it, a second reference formed
beside it — waits until that other binding is no longer mentioned,
because the other binding may hold `x`'s path. A reference, slice or
`StringView` that reaches only reference-free storage (`ref<Vec<i32>,
exclusive>`, `slice<u8, shared>`) can come to hold it only by being
rebound, so it waits for `x` only through a statement that may rebind it
(D-0275). Whether the other binding does hold `x`'s path is then read
from the flow facts, which see exactly which borrows were formed through
`x`. So

    auto r1 = &mut y;
    Hold h1 = Hold { .r = r1 };
    *h1.r = *h1.r + 1;
    printf("%v\n", y);

ends `r1`, last mentioned in the second statement, after the third: it
shares the second with `h1`, a struct holding a reference, which is
mentioned in the third. `h1` ends after the third as well, and the
program runs; `h1`'s copy of `r1`'s path is ended before `y` is read.
(D-0219 finding 8: the text before D-0221 ended `x` at `s_k` or at its
block's exit; both implementations always deferred as above. Before
D-0275 the example was `*r1 = *r1 + 1` in the third statement, with `h1`
waiting for `r1`; a reference to an `i32` no longer holds anything past
its last mention.)
The facts the end clears are forgotten, not denied (§6 "Forgetting",
D-0236): the referent becomes `escaped`, so an access to it after the
end is checked dynamically. A copy of `x`'s value the flow facts do not
see — `Hold h1 = Hold { .r = r1 }` reached later through `&h1`, or `r1`
pushed into a `Vec` — keeps the path valid after `x` ends, and the check
finds it.

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
frame ended earlier in this same fold (last-bound-first order ends
holders before what they hold whenever the holder was bound
later — a reference cannot be stored into an object bound
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

    [If-Pattern]   (D-0115)
        p a pattern whose top is a variant with a payload pattern, a qualified variant, or a literal
        ────────────────────────────────────────────
        ⟨if (p = e) b1 else b2, Σ⟩ ≡ ⟨match (e) { p : b1, _ : b2 }, Σ⟩       -- `else b2` absent: b2 = {()}

The condition reduces first (`E ::= if (E) b1 else b2`); the chosen
branch is an ordinary block. Type: `rule.type.typing` `[T-If]`.

`[If-Pattern]` is sugar: `if (Some(v) = o) b1 else b2` is the `match`
shown, with every rule of `rule.agg.match` — the binders of `p` are in
scope in `b1` only, `e` is consumed exactly as that `match` consumes it
(a resource payload moved out by `p`, D-0049), the value is the chosen
block's, and `else if` chains as `b2`. A bare name on the left of `=`
is not a pattern here but the assignment it is everywhere
(`diag.type-mismatch`: the condition is `unit`, `[T-If]`), so `if (x =
e)` never silently tests anything; `None` or another payload-less
variant is written through `match`, `Option::is_none`, or `==`. A `_`
arm that `[Match-Unreachable]` rejects (a pattern that always matches)
is reported at the `if`.

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

`while (true) b`, with the literal `true` and no `break` in `b` that
leaves this loop, has no normal completion: `[While-False]` never
applies, so it ends only by `return`, a fault or `exit`. Its type is
`never` (`spec/12` `[T-While-Forever]`, D-0175); the code after it is
unreachable, and the static pass's flow facts there assert nothing, as
after `return`.

    [While-Pattern]   (D-0115)
        p as in [If-Pattern]
        ────────────────────────────────────────────
        ⟨while (p = e) b, Σ⟩ ≡ ⟨while (true) { match (e) { p : b, _ : break } }, Σ⟩

`e` is re-evaluated each iteration inside the loop frame, and its
first value `p` does not match ends the loop — for a `Result`, an
`Err` as much as an `Ok` of the wrong shape: the `while` form is the
written decision to stop on every other case, as a `_ : break` arm
is, so a loop that must tell `Err` from the end of its input writes the
`match`. `break` and `continue` in `b` belong to this loop; `b`'s
binders are `p`'s.

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
own block, so it is out of scope after the loop. It governs `b`
(`spec/05` `[Binding-Form-Redeclared]`, D-0131): `b` may not declare the
same name again, though a block nested in `b` may. `continue`
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
`HashSet::key_at`, a map's `keys`/`values` vectors; for a consumed `Vec`,
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

### `rule.control.foreach-range`
**Status:** ACCEPTED

`foreach (x in lo .. hi) b` visits the integers from `lo` up to, not
including, `hi` — the same half-open reading `..` has in a slice
(`&a[lo .. hi]`, `spec/16` §3a). With two names,
`foreach (k, x in lo .. hi)`, `k` is the position (`usize`, from 0).
`lo` and `hi` are evaluated once, `lo` first, before the first step.
They have one integer type `T`, the type of `x`: a bound that is a
literal expression (D-0037) takes the other's type, as it would across
`<`; when both are literal expressions, `T` is `usize` (a range most
often counts positions or indexes). A range is written only
here: it is not a value (`auto r = 0 .. 3;` does not parse), is not
borrowed, and `$` has no length to name in it.

    [Foreach-Range]   ⟨foreach (k, x in lo .. hi) b, Σ⟩ ≡
        ⟨{ T $v = lo; T $h = hi; usize $i = 0;
           for (; $v < $h; { $v += 1; $i += 1; }) { usize k = $i; T x = $v; b } }, Σ⟩

    [Foreach-Range-Ill-Typed]   disposition: rejected
        lo and hi have no one integer type (either is not an integer, or they are of two types;
        or both are literal expressions and one is negative, so not a usize); or three names
        ────────────────────────────────────────────
        ill-formed; diag.type-mismatch (static)

(with one name, drop `k`, `$i` and its step). `$v`, `$h` and `$i` are
bindings no program can name.

- **No overflow.** `$v` stops at `$h`, which is a value of `T`, so
  `$v += 1` never leaves `T`: `foreach (b in 0 .. top)` with
  `u8 top = 255` visits 0 to 254.
- **Empty.** When `hi ≤ lo` the body never runs; nothing faults.
- **A copy.** `x` is a copy of `$v`: assigning to `x` in `b` does not
  change which values the loop visits.
- **`continue`** runs the step; `break` leaves without it
  (`[For-Continue]`).
- **Not added:** an inclusive range, a step, or a reverse range; a
  counted `for` writes them.

**Depends on:** rule.control.for, rule.control.foreach,
rule.type.expected, D-0037, D-0097

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

**This analysis defines the language; any other only optimizes**
(D-0206). Its facts, lattice, CFG and transfer functions fix the set of
accepted programs. An implementation may run any further analysis to
discharge a check statically (`discharge: static`, `spec/03`;
`[Conc-Checks-Omitted]`, `spec/19` §4; `spec/08` §4) and leave the
check out where it has proved it cannot fail, but no further analysis
accepts a program this rule rejects or rejects one it accepts. Where
two static diagnostics apply to one program, the first in source order
is the one reported (D-0110). The reference interpreter's `--check` is
the executable form of this rule (`spec/IMPLEMENTATION-NOTES.md` §4).

**Facts.** For a function body, the finite family `Φ` contains, for
every binding `x` declared in it (parameters included) and every
reference-typed binding `r`:

    valid(x)        -- x's path is temporally valid (not moved-from; object not ended)
    init(x)         -- x's object has init = valid
    deriv(r, x.π, m) -- r currently holds a reference derived from x's object at syntactic
                       projection path π (ε for &x, f for &x.f, f.g for &x.f.g, [i] for a
                       literal index into an array) with mode m (formed by a visible borrow of that place,
                       or a D-0011-elided call on such); two paths π1, π2 "overlap" iff one is
                       a prefix of the other, with literal indices compared by value and a
                       non-literal index overlapping every index; a slice's step is its range
                       [lo, hi) (D-0070), which meets a literal index inside it or a literal
                       range it intersects when lo and hi are literals, and otherwise meets no
                       index or range here (the dynamic check decides)
                       (D-0071: r is also a match-by-reference binder, or a binding holding
                       a spawn's handle or (CHG-0100) a closure without `move`; and x may be the referent root *q of a tracked
                       binding q of reference or guard type, whose places are *q, (*q).f,
                       q.f, q[i] and their projections)
                       (D-0221: a `Vec`'s elements are not places here: a borrow of an element
                       `&_m v[k]` — or a reference from `Vec::index_shared`/`index_exclusive` —
                       forms deriv(r, v, m), a borrow of the whole container, so a later borrow
                       or a call taking `&v`/`&mut v` meets it; an element access `v[k]` itself — a
                       read, a write or an element borrow — is not checked against these facts:
                       the dynamic check of `spec/16` `[Index-Vec]` decides it, at the element)
    escaped(x)      -- a reference to x's object has been stored somewhere the analysis
                       does not track (an aggregate field, a spawn argument, a call
                       argument other than the elided case, an assignment through *e),
                       or a deriv fact rooted in x was forgotten while the reference it
                       described may still be held (D-0236, "Forgetting", below)

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
| `auto r = &_m x.π;` or `r = &_m x.π` | `deriv(r, x.π, m) := T`; other `deriv(r, ·, ·) := F`, forgetting them (below) |
| `auto r = f(&_m x…)` (D-0011 elided) | as the line above |
| resource-typed whole binding `x` consumed: as an argument; as a local-declaration, field, element, or payload initializer (`auto y = x`, `Name { .f = x }`, `Vi(x)`); as `return x` or a trailing result; as a `move` capture; by a `match (x)` arm that binds a resource payload (D-0049: within that arm); as the source of `Name{f} = x`; or as the value of a raw write `*p = x` | `valid(x) := F`; every `deriv(·, x, ·) := F`; `deriv(x, ·, ·) := F`, forgetting them (below; D-0244: what `x` held goes on with it — a handle moved into a `Vec` or a struct keeps its thread's borrows) |
| `drop(x)` | `valid(x) := F`; every `deriv(·, x, ·) := F` |
| `&x…` / `&mut x…` in any position other than the two `deriv` lines | `escaped(x) := T` |
| `r = e` for reference-typed `r` from anything but a visible borrow | every `deriv(r, ·, ·) := F`, forgetting them (below); `escaped(x) := T` for the `x` the analysis cannot exclude |
| block exit | for every `r` declared in the block: `deriv(r, ·, ·) := F`, forgetting them (below); for every `x` declared in it: `valid(x) := F` |
| a binding `x` ended after `s_j` by `[Ref-Binding-Last-Use]` (D-0219, D-0221) | as block exit, for `x` alone: `deriv(x, ·, ·) := F`, forgetting them (below), `deriv(·, *x.·, ·) := F`, `valid(x) := F` |
| `spawn(…, &_m x …)` | `escaped(x) := T` |
| `auto h = spawn(f, a1, …, an);` or `h = spawn(…)` (D-0071) | `deriv(h, x.π, m) := T` for each `ai` that is a visible borrow, slice or elided call forming `deriv(·, x.π, m)` by the lines above; `deriv(h, *r, m) := T` for each `ai` that is a tracked binding `r` of reference type, `m` the mode `f`'s parameter takes (D-0213: the thread is lent `r`, `spec/19` `[Thread-Lend]`) — a `ref`, not a slice: a slice binding passed whole is lent as any value is, and forms no fact (D-0247); other `deriv(h, ·, ·) := F` |
| `spawn(…, r …)` with `r` a tracked binding of reference type (D-0213) | the lend is a borrow of `*r` in the parameter's mode: `¬clash` against `deriv` facts, as `&_m *r` would be |
| `join(h)` | `deriv(h, ·, ·) := F` (the thread has finished) |
| `auto o = e;` / `o = e;` / a `match (e)` arm's binder `o`, where `e` is a call `f(a1, …, an)` (or `Option::unwrap`/`expect`, `Result::unwrap`/`expect` of one) whose result holds a reference without being one (`Option<ref<V, m>>`, D-0088) | `deriv(o, x.π, m) := T` for the first reference-typed parameter's argument, when it is a visible borrow or slice forming `deriv(·, x.π, m)`; other `deriv(o, ·, ·) := F` |
| `auto c = [x1, …, xn](…) { … };` or `c = [x1, …](…) { … }` — a closure without `move` (CHG-0100) | `deriv(c, xi, m) := T` for each captured local `xi` not of reference type, `m` = `exclusive` when the body writes `xi` (spec/15 §6's capture mode) and `shared` otherwise; `escaped(r) := T` for each captured local `r` of reference type (D-0243: the closure holds a borrow of `r` itself, which no fact describes); other `deriv(c, ·, ·) := F` |
| a closure without `move` in any other position (an argument, a callee, `spawn`'s first argument) | `escaped(xi) := T` for each captured local `xi` (D-0243) |
| the arm of `match (&_m e)` / `match (q)` (`q` a tracked reference binding, mode `m`) with binder `b` (D-0071) | in the arm: `deriv(b, e's place, m) := T` / `deriv(b, *q, m) := T` |
| `q` assigned, moved, or ended at block exit (`q` of reference or guard type, D-0071) | every `deriv(·, *q.·, ·) := F`; a moved binding also `deriv(q, ·, ·) := F`, forgetting them (below) |
| anything else | no change |

**Forgetting** (D-0236). A row above that sets `deriv(r, x.π, m)` from
`T` or `?` to `F` "forgetting" it — because `r` ends (block exit,
`[Ref-Binding-Last-Use]`), is reassigned, or is moved — also sets
`escaped(x) := T` for the binding `x` at the fact's root (a root `*q`
needs nothing: an access there is never proven). The fact said that the
reference `r` held reaches `x`; `r`'s end does not end that reference,
which `r`'s value may have been copied into — a struct field, a `Vec`'s
element, a spawned thread's parameter, a handle stored in a container —
where it stays valid and held (`spec/04` `held-by`) after `r` is gone.
Without this, the next access to `x` would be *proven* clash-free while
`clash` is true of it (`conf.ref-copied-into-struct-then-binding-ended`,
`conf.ref-copied-into-vec-then-block-exit`). Escape changes no
acceptance: `escaped` decides only between *proven* and *unknown*, and
the dynamic check then decides. The rows that clear facts because their
subject is gone — `join(h)` (the thread has finished; what it was lent
was a borrow its spawn already marked, by `spawn(…, &_m x …)` or by the
lent binding's own facts), and the consumption or `drop` of `x` itself
— do not forget in this sense the facts rooted in `x`; the facts `x`
itself held, as a holder, are forgotten when it is consumed (D-0244).

**Discharge.** A guarded side-condition `⟦ φ ⟧` at a program point,
where `φ` is one of the following, is resolved from the state there:

| Side-condition | `T` (proven) when | `F` (refuted) when |
|---|---|---|
| `temporally-valid(a_x)` | `valid(x) = T` | `valid(x) = F`, or (D-0087) `valid(x) = ?` for a use of the binding `x` |
| `init-state(o_x) = valid` | `init(x) = T` | `init(x) = F` |
| `authority(ℓ, destroy, o_x)` | `valid(x) = T` and `x` resource-typed | `valid(x) = F` |
| `¬clash(a, m)` for an access at place `x.π` | `escaped(x) = F` and every `deriv(r, x.π', m') = F` for which `π, π'` overlap and `¬permitted(m, m')` | some such `deriv(r, x.π', m') = T` |
| `solitary(a_x)` | `escaped(x) = F` and every `deriv(·, x.·, ·) = F` | some `deriv(·, x.·, ·) = T` |
| `¬live-resource-at(a_x)` for a whole-object write to resource-typed `x` | `init(x) = F` | `valid(x) = T` and `init(x) = T`, and every value of `x`'s type owns something (`spec/05` `owns`; D-0049) — otherwise the value decides, dynamically |
| `¬live-resource-at(a)` for a write to a part `x.π` of an object (D-0095) | never `T` | every value of the part's type owns something, every index step of `π` is a literal inside a fixed array's length, and either `x` is a binding with `valid(x) = T` and `init(x) = T`, or `x = *r` (written `*r.π` or `r.π`) for a reference binding `r` with `valid(r) = T` — a part of a valid object always holds its value: moving one out is rejected (`[Store-Binding-Place-Transfer-Sub]`), and `[Rawptr-Move-Out]`'s trusted side-condition forbids taking cells a live object claims; what could end the referent of a valid exclusive `r` would first clash with `r`'s borrow. A computed index (the part may not exist: `diag.index-out-of-bounds` comes first), a `Vec` element (reached through a call), and a raw pointer's cells are left to the value, dynamically |
| `¬clash` for an access at a place rooted at `*q` (D-0071) | never `T` (the caller may hold other paths to the referent, D-0022) | some overlapping, non-permitted `deriv(r, *q.π', m') = T` |
| `¬clash` / `solitary` for an access whose root is a temporary, or `*r` for `r` not a tracked reference or guard binding | never `T` | never `F` |
| a value-range condition (`v2 ≠ 0`, `0 ≤ i < N`, `r ∈ represented-domain`) | the operands are literals or literal-bound locals (below), and the condition holds of them | likewise, and it fails |

A binding `x` is **literal-bound** to `v` at a program point when its
declaration's initializer is an integer literal `v` (optionally
parenthesized or negated) and, on every path from the declaration to
the point, `x` is not written (assignment or compound assignment),
borrowed (`&x…` or `&mut x…`, whatever becomes of the reference),
or captured by a closure. At a join, `x` is literal-bound only if it
is literal-bound to the same `v` on every incoming path; a loop's
entry counts its back edges as incoming paths (CHG-0092).

**A closure call** (D-0245). `[Closure-Call]`'s borrow of the closure's
place (`spec/15` §6) is an access at that place in mode `m_self`:
`exclusive` when the call names a binding `c` initialized by a `move`
closure literal whose body writes a capture (writes-self, D-0239), and
`shared` when `c`'s literal does not; for any other callee (a `fn`
value, `(*f)(…)`, a binding the analysis did not see formed) `m_self`
is not known, and the call is refuted only as a shared access would be
and proven only as an exclusive one would be. So `auto s = &c; c();`
for a self-writing `c` is rejected statically, as both tools now reject
it (`conf.move-closure-writing-capture-while-shared`); before D-0245 the
table already refuted it, and both tools faulted at run time instead.

Outcome per result: **proven** → the condition is discharged
statically, no runtime check; **refuted** → the program is ill-formed
(`disposition: rejected` for that instance, with the rule's own
diagnostic marked *static*); **unknown** (`?`, or a shape the table
does not cover) → `discharge: dynamic`: the runtime check named by the
rule is performed and its `↛` diagnostic applies if it fails.
`rule.init.definite-assignment` (`spec/11` §3) and a use of a binding
that may have been moved or dropped (`temporally-valid(a_x)` with
`valid(x) = ?`, D-0087) are the two exceptions where `unknown` rejects. `rule.temporal.ref-escape` is a separate
lexical rule, not an instance of this analysis.

**Scope.** Intraprocedural and syntactic: a call is opaque except for
its declared signature and the D-0011 elision; no value-range
reasoning beyond literals and literal-bound locals. A conforming implementation must reject
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

- 1.29.0 — `CHG-0365` (D-0275): `[Ref-Binding-Last-Use]`: a reference, slice or `StringView` to reference-free storage defers another binding's end only through a statement that may rebind it; the prose after the rule.
- 1.28.0 — `CHG-0300` (D-0247): the spawn row's tracked binding is a `ref`, not a slice; `CHG-0295` (D-0243): the closure row marks a captured reference binding escaped, and a row for a closure anywhere else; `CHG-0296` (D-0244): the consumption row forgets the consumed binding's own facts, and "Forgetting" says so; `CHG-0297` (D-0245): "A closure call" after the discharge table.
- 1.27.0 — `CHG-0285` (D-0236): §6 "Forgetting": a fact cleared because its holder ends, is reassigned or moved marks its root `escaped`; the `escaped` definition and five rows; §1's prose after `[Ref-Binding-Last-Use]`.
- 1.26.1 — `CHG-0279` (D-0233): `[Block-Exit]` ends the frame's objects last bound first, by when each one's holder was formed, as D-0008 says and both implementations do (an object moved into a later binding ends with that binding).
- 1.26.0 — `CHG-0260` (D-0221): `[Ref-Binding-Last-Use]` states the
  deferred end both implementations apply: a binding waits until no
  binding sharing a statement with it is mentioned, and is then ended
  unless the flow facts root another binding's reference in it (D-0219
  finding 8). §6: a borrow of a `Vec` element is a borrow of the
  container (finding 10). Clarification: what both implementations do.
- 1.25.0 — `CHG-0258` (D-0219): `[Block-Exit]` spares its result's
  references from the ends it makes; `[Ref-Binding-Last-Use]`: a reference
  lent to a thread is borrowed through; the flow table's row for a binding
  ended at its last use. Clarification: what both implementations do.
- 1.24.0 — `CHG-0251` (D-0213): a reference binding passed whole to
  `spawn` is a borrow of its referent, held by the handle until `join`:
  lending it twice, or writing through it before the join, is refuted
  where both are visible.
- 1.23.0 — `CHG-0244` (D-0206): `rule.control.flow-analysis` says that
  it alone fixes the accepted set, that a further analysis may only
  discharge checks, and where diagnostic precedence is stated. No rule
  changed.
- 1.22.0 — `CHG-0203` (D-0175): a `while (true)` no `break` leaves has no normal
  completion. Additive.
- 1.21.1 — `CHG-0158` (D-0134): `HashSet::key_at` (was `HashSet::at`).
- 1.21.0 — `CHG-0156` (D-0133): `[Ref-Binding-Last-Use]` covers every local
  that holds a reference and no resource or `fn` value.
- 1.20.0 — `CHG-0154` (D-0131): §3: a `for` header's declaration
  governs the body, which may not redeclare it.
- 1.19.0 — `CHG-0138` (D-0115): `[If-Pattern]` and `[While-Pattern]`: a
  condition `pattern = e` as the `match` it stands for.
- 1.18.0 — `CHG-0131` (D-0111): `[Ref-Binding-Last-Use]`: a local
  reference, slice or `StringView` binding ends after its last use.
- 1.17.0 — `CHG-0112` (D-0097): `rule.control.foreach-range`:
  `foreach (x in lo .. hi)` and `foreach (k, x in lo .. hi)` over the
  integers from `lo` up to, not including, `hi`.
- 1.16.0 — `CHG-0110` (D-0095): `¬live-resource-at` for a write to a
  part of an object — a field, a literal-index array element, a place
  reached through a valid reference — is refuted statically on the
  same terms as a whole-object write; previously only whole-object
  writes were, and a part was left to the run.
- 1.15.0 — `CHG-0103` (D-0088): a call's result that holds a reference
  (`Option<ref<V>>`) borrows from the first reference argument.
- 1.14.0 — `CHG-0102` (D-0087): a use of a binding that may have been
  moved or dropped (`valid(x) = ?`) is rejected, as a read that may be
  uninitialized is.
- 1.13.0 — `CHG-0100`: a binding holding a closure without `move`
  holds `deriv` facts for its captures.
- 1.12.0 — `CHG-0092`: "literal-bound" defined for the value-range
  row: a write, borrow or closure capture on any path ends it.
- 1.11.0 — `CHG-0083` (D-0071): `deriv` through referent roots `*q`,
  match-by-reference binders, and spawn handles until `join`.
- 1.10.0 — `CHG-0080` (D-0070): `deriv`'s paths take a slice's range.
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
