# D-0301 — A pattern binder through a `Box`: `V(*x)`

Status: ACCEPTED (2026-11-01, the owner: option 2 as proposed); implemented by `CHG-0399`
Kind: Decision (`spec/02-schema.md`); a language extension
Governed by: `CobaltC_Master_Instructions.md` §1, §8, §9, §10
Depends on: D-0045 (`Box`), D-0046 (match by reference), D-0056, D-0057, D-0058 (patterns), D-0109 (match
through a reference payload), D-0218 (reads through shared references), D-0224 (struct patterns), D-0272 (copy
permissions), D-0278 (changes land in `coby`/`cobc` first), D-0281 (the cost model), D-0298 (a `Box` is one
pointer)
Affects: `spec/22` §2 (`pat`) and §5; `spec/16` §4 `rule.agg.match` (a `[Match-Through-Box]` block);
`spec/conformance.md` (new rows); the shared parser (`impl/src/parser.rs`) and checker
(`impl/src/typecheck.rs`, the match code at `[Match-By-Ref]`); `coby`'s binding of a match arm; `cobc`
(`impl/cobc/src/lower/matching.rs`: no new lowering, see §Selected design); the guide (`content_b.py`,
`content_c.py`) and the book; `proofs/COVERAGE.md` (a rule by `≡`); `impl/selfhost` at its next port (D-0278)

## Problem

A CobaltC variant carries one payload, so a recursive or many-field payload is a `Box` of a struct
(`impl/selfhost/front/ast.cb`: 32 of its 40 structs exist only as the payload behind a `Box`). Every arm that
reads such a payload through a reference binds the `Box` and reaches through it on the next line:

    match (&e.kind)
    {
        ExprKind::ECall(cb) :
        {
            ref<CallExpr, shared> c = Box::get(cb);
            rep_pos_in(&c.callee, m);
            …
        },
        ExprKind::EField(f) : sprintf("field .%s", &Box::get(f).name),
        …
    }

    Type::TRef(rb)   : Box::get(rb).mode == Mode::Exclusive,
    Type::TSlice(sb) : Box::get(sb).mode == Mode::Exclusive,

Measured on 2026-11-01 over the self-hosted toolchain (`impl/selfhost/front`, `back`, `cbc`; 113,379 lines):

| form | sites |
|---|---|
| `Box::get(x)` or `Box::get_mut(x)` on a bare name, nearly all a match binder | 4,382 |
| `Box::get(&p.f)` on a field (not touched by this decision) | 130 |

One line in 26 of the compiler is this reach-through. The Rust it reproduces writes `ExprKind::Call(c) =>
c.callee` (auto-deref); the port spends a line, a type and a name on each. Outside the port the form is rare
(`impl/std`: 1; the conformance suite: 21; the showcases and examples: 0), because few programs outside a
compiler match recursive trees by reference at every turn. A tree walk over `Option<Box<Node>>`
(`Some(b) : Box::get(b).key`) is the same shape in a program's own data.

## Candidate mechanisms

1. **Leave it.** `Box::get(b)` is explicit and says where the pointer is crossed. Costs the line above at every
   site; the arm's binder name (`cb`) exists only to be passed to `Box::get` once.

2. **A binder written `*x` looks through a `Box`.** In a pattern, `V(*x)` where `V`'s payload type is
   `Box<τ>` binds `x` to the boxed value: through a reference ([Match-By-Ref], [Match-Through-Ref]) `x :
   ref<τ, m>`, as `Box::get(x0)` / `Box::get_mut(x0)` gives it; by value ([Match]) `x : τ`, as
   `Box::into_inner(x0)` gives it, the `Box` consumed as a resource payload is. Additive: `*` never starts a
   pattern today. One character per site. **Selected.**

3. **Every binder looks through a `Box` by itself** (Rust's auto-deref in patterns): `ExprKind::ECall(c)`
   binds `ref<CallExpr, shared>`. The same saving with no character at all, but not additive: every program
   that writes `Box::get(b)` on a match binder today is `diag.type-mismatch` under it (the 4,382 sites in the
   port, 21 conformance cases, the guide's and the book's examples), and a reader can no longer tell from the
   arm whether a pointer is crossed. Rejected for the migration and the loss of the written hop; 2 gives the
   writer the same line back and keeps the hop visible.

4. **Field access through a `Box`** (`b.f`, `b[i]`, Rust's `Deref`): broader than the problem. It touches
   `[Field-Access]`, `[Index]`, place forms, D-0272's copy permissions and D-0281's path forms, makes a `Box`
   transparent everywhere a reference is, and is the auto-borrowing `.` the owner declined for calls in another
   guise. Rejected; it could be added later on top of 2 if field-position sites ever dominate (they are 130
   today).

5. **A unary `*b` on a `Box` in expressions.** `(*cb).callee` saves little over `Box::get(cb).callee`, and
   `*` is the reference, raw-pointer and guard operator; giving it a fourth operand type for a one-token gain
   is not worth the rule.

## Selected design

Grammar (`spec/22` §2):

    pat ::= '_' | x | '*' x | lit | V | V '(' pat ')'        -- '*' x: D-0301

`*x` may stand only as a payload pattern (`V(*x)`, at any depth); `*x` alone as a whole-scrutinee pattern is
a syntax error, as there is no payload to look through. The parser needs no lookahead: `*` does not begin any
other pattern form.

Meaning (`spec/16` §4, one block, stated by `≡` to the arm the programmer writes today):

    [Match-Through-Box]   (D-0301)
        an arm whose chain ends in V_k(*x), V_k's payload type Box<τ>
        ────────────────────────────────────────────
        ≡ the arm V_k(x0), x0 fresh and not in scope, whose body begins
            ref<τ, m> x = Box::get(x0);       -- [Match-By-Ref]/[Match-Through-Ref]: m the mode those rules give x0
            ref<τ, m> x = Box::get_mut(x0);   -- the same, m = exclusive
            τ x = Box::into_inner(x0);        -- [Match]: by value; x0 : Box<τ> moved out, the scrutinee consumed
                                              --   as any resource payload's move consumes it (D-0049)

    [Pattern-Type] gains the case: `*x` on a payload whose type is not `Box<τ>` is ill-formed,
    diag.type-mismatch ("`*x` binds through a `Box`; this payload is `τ'`").

Everything else follows from the arm it stands for: `x` derives from the scrutinee's token through the box as
`Box::get`'s result does, so the box and its owner stay borrowed while `x` lives (D-0088), `[Match-Exclusive-
Through-Shared]` and `[T-Read-Through-Shared]` (D-0218) apply to `x` as to any binder, exhaustiveness is
unchanged (a binder covers its level), and by value the arm owns `τ` and the `Box`'s cells are released by
`into_inner` (D-0298: the pointer nulled, the `Box` then dropped empty). A struct pattern (D-0224) is not
admitted after `*`: it is rejected through a reference already, and by value `V(*x)` followed by `S { a, b } =
x;` is the explicit form.

Not in this decision: a chain continuing through a box (`TRawptr(*TInt(i))`): the one enum behind a `Box` in
the port is `Type`, and `TRawptr(*t)` then `match (t)` is the written form; `Rc` and `Arc` (`Rc::get` is
shared-only, an exclusive `*x` through one could not be given a meaning; revisit if a program asks); `*x` in
`[Let-Destructure]`'s struct patterns (a field that is a `Box` is read by `Box::get(&s.f)`, the 130 sites,
not by a binder).

Implementation: the shared parser records the `*` on the binder; the checker, where it types each arm's
binder (`impl/src/typecheck.rs`, the `[Match-By-Ref]`/D-0109 code), rewrites the arm into the form above, so
`coby`'s interpreter, `cobc`'s lowering, the model, the certificate (D-0299) and the proofs see only `Box::get`,
`Box::get_mut` and `Box::into_inner` on a binder used once, first. That shape is the one `cobc` already lowers
without minting a token (`lower/matching.rs`, `binder_used_once_first` with `binder_box_call`), so the C of a
rewritten arm is the C a hand-written arm gets today and no tier (D-0281) or decided-program answer moves.
`cobfmt` prints `*x` as it prints a binder. `proofs/COVERAGE.md` lists the block as a rule by `≡`, as
`[If-Pattern]` is listed. `impl/selfhost` gains the syntax at its next port (D-0278); until then it is written
in the old form and compiles under both toolchains.

Conformance (`spec/conformance.md`): `*x` through a shared reference and through an exclusive one (the arm
writes through `x`); by value (the box consumed, the value owned, a resource `τ`); `*x` on a non-`Box` payload
rejected; the box's owner borrowed while `x` lives (`Vec::push` on it rejected, as `conf.match-through-ref-
binder-borrows-rejected`); `*x` in an `if`-pattern (D-0115) and in `Option<Box<T>>` under D-0298's layout.

## Trade-offs

- **For:** one line and one name per site at 4,382 sites in the compiler; the pointer hop stays written (`*`);
  additive, no migration; no new lowering, cost-model answer or proof obligation; `Option<Box<T>>` trees read
  as `Some(*n) : n.key`.
- **Against:** a third meaning for `*` (dereference in expressions, destructure here); a pattern language that
  D-0057 closed gains a form (D-0127 and D-0224 have opened it twice since, for the same reason: measured
  friction); writers who want Rust's silence still write the `*`.
- **Doctrine (§9):** the mechanism is a desugaring to three `std` functions that exist; it adds no semantics.

## Compatibility impact

Additive: `V(*x)` was a syntax error. No existing program changes meaning. `Box::get(b)` on a binder stays
legal.

## Revisit conditions

If field-position reaches through a `Box` (`Box::get(&s.f)`) come to dominate, option 4 on top of this one.
If `Rc`/`Arc` payloads are matched by reference often, a shared-only `*x` for them under the same block.
If the owner prefers option 3's silence, the block above stands with `x` for `*x` and a migration of the
corpus (mechanical: delete `Box::get(` around a binder).

## Implementation notes (CHG-0399)

- The parser writes `V(*x)` as `V($box<n>_x)`, a hidden binder no program can name, and the arm's body
  begins `auto x = Box::get($box<n>_x);`. The checker, where it checks that call, reads the hidden binder's
  type: `ref<Box<τ>, shared>` keeps `Box::get`, `ref<Box<τ>, exclusive>` makes it `Box::get_mut`, `Box<τ>`
  makes it `Box::into_inner`, and any other type is the `[Pattern-Type]` rejection. The rename is one of the
  checker's rewrites (D-0164), so the program is checked again and every later pass sees the written form.
  A diagnostic names `x`, not the hidden binder.
- `cobc`'s fast path for a binder used once, first (`binder_used_once_first`) applies when the arm's first
  statement is an expression; an arm that begins with a binding, as this one does and as the hand-written
  `ref<τ, m> x = Box::get(b);` does, is lowered as that hand-written arm is. The C of a `*x` arm is the C of
  the hand-written arm, names aside.
- `proofs/COVERAGE.md` lists `spec/23`'s rules, which a `≡` block is not; nothing there changed.
