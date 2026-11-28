# D-0324 — A variant with named fields; a struct pattern through a reference

Status: ACCEPTED (2026-11-14, the owner: "I want them all, write the plan so my agent can implement it in full
autonomously", then "drop 5 and 6 then review the plan again, with my concerns in mind", then "implement the
revised plan in full and push once completed", on `private/EASE-PLAN-2-2026-11-14.md`, item 4 and its decisions
Q5 to Q9); implemented by `CHG-0453`
Kind: Decision (`spec/02-schema.md`); a language extension
Governed by: `CobaltC_Master_Instructions.md` §1, §8, §9
Depends on: D-0045 (enums), D-0046 (match by reference), D-0056 (nested patterns), D-0073 (one payload), D-0106
(codes), D-0116 (derived `clone`), D-0224 (struct patterns), D-0296 (both toolchains in step), D-0301
(`V(*x)`)
Affects: `spec/22` §2 (`variant`, `pattern`), §5; `spec/16` §4 (`rule.agg.enum`, `rule.agg.match`); `spec/12`
`[T-Enum]`; `spec/conformance.md` (3.312.0); `impl/src/` (`ast.rs`, `parser.rs`, `modres.rs`, `typecheck.rs`,
`views.rs`); `impl/selfhost/front/` (`ast.cb`, `parser.cb`, `modres.cb`, `typecheck.cb`)

## 1. Problem

A variant carries one payload (D-0073), so a variant with several values needs a struct of its own beside the
enum, and a match that reads its fields names that struct too. An expression tree, the common case, was
written

    struct BinOp { u8 op; Box<Expr> a; Box<Expr> b; }
    enum Expr { Num(i64), Bin(BinOp), }
    … Expr::Bin(BinOp { .op = b'+', .a = l, .b = r }) …
    … Expr::Bin(bo) : f(bo.op, &bo.a, &bo.b) …

Four programs written cold (`private/EASE-PLAN-2-2026-11-14.md` §1) met it twice: the first try was Rust's
`Bin(u8, Box<Expr>, Box<Expr>)`. And a struct pattern (D-0224) could not be used through a reference at all
(D-0224 left that to a later decision), so a payload struct reached through `match (&e)` was bound whole and
read field by field.

## 2. Options

1. **Several positional payloads**, `Bin(u8, Box<Expr>, Box<Expr>)`, matched `Bin(o, l, r)`. Fewest tokens;
   but a reader of `Bin(o, l, r)` cannot tell which value is which without the declaration, and the language
   has no unnamed product. Rejected.
2. **Named fields**, `Bin { u8 op; Box<Expr> a; Box<Expr> b; }`, made `Expr::Bin { .op = …, … }` and matched
   `Expr::Bin { op, a, b }`: the struct literal and the struct pattern a program writes today, on the variant.
   **Chosen.**
3. **Leave it.** The form above stays the written form.

The owner's condition on the plan (2026-11-14: changes must not "make it more difficult for humans to
understand the code from simply reading the code"): option 2 puts more on the line than today, the field names
where the value is made and where it is matched, and its variant is always written with its enum's name.

## 3. Decision

- **Declaration** (`spec/22` `variant`): in an `enum`, `V { τ1 f1; …; τn fn; }`, at least one field, each
  named once, without `export` (a variant's fields are seen wherever the variant is). It is the variant
  `V(E$V<T…>)` of a struct `E$V` the implementation adds beside the enum: those fields, all exported, the
  enum's type parameters `T…`, exported when the enum is. `$` cannot be written in a program, and a message
  names the struct as the variant, `E::V`. Not in an enum with codes (D-0106), which has no payloads, nor in
  an enum declared inside a function.
- **Value**: `E::V { .f1 = e1, …, .fn = en }` is `E::V(E$V { .f1 = e1, … })`: a struct literal, every field
  named once.
- **Pattern**: `E::V { f1, f2: x, .. }` (D-0224's field list) is `E::V(p)` whose arm begins
  `E$V { f1, f2: x, .. } = p;`, whole or innermost in a chain (`Some(E::V { a, .. })`), in a `match`, an `if`
  and a `while` (D-0115).
- **Always with the enum's name**: bare `V { … }`, as a value or a pattern, is `diag.unbound-name`, since it
  would read as a struct named `V`.
- **Refused**, each with the form to write: `E::V(x)` as a value, or as a pattern with one binder, of a variant
  with fields (`diag.type-mismatch`: the binder's type would be one no program can write); the statement
  `E::V { a, b } = e;` (`diag.type-mismatch`: a variant may not match; `match` or `if` takes it apart).
- **`[Struct-Pattern-By-Ref]`** (`spec/16` §4, for every struct pattern, D-0224's "Revisit"): a struct pattern
  reached through a reference of mode `m` binds each named field `f` (as `x`) by `auto x = &p.f;` (`&mut p.f`
  when `m` is exclusive), `p` the pattern's hidden binder, in the pattern's order; `..` binds nothing.
  By value it is D-0224's destructuring, unchanged. So, with D-0046: what a pattern binds through a reference
  is a reference, in that reference's mode, with no exception left.
- For exhaustiveness the pattern is a binder (D-0224); a missing arm is shown `E::V { .. }`.

## 4. Consequences

- Both definitions are by `≡` to forms that existed: the interpreter, both compilers, the model and the proofs
  see a struct, a one-payload variant, a struct literal, a struct pattern and `auto x = &p.f;`. The rewrite of a
  by-reference struct pattern is recorded by the checker (`Rewrite::PatternFields`) and applied before the
  program is checked again (D-0164's loop); in the first round the declarations it becomes are checked
  directly, so no round reports an error the rewritten program would not.
- A derived `E::clone` (D-0116) makes and matches the variant by its hidden payload; the refusals above do not
  apply in a derived function.
- In `std`, whose bodies are checked after the rewrite rounds, a struct pattern through a reference stays an
  error (as D-0301's `*x` through a reference does); `std` and the self-hosted toolchain do not use the new
  forms in this Update.
- `struct_pattern_through_ref_rejected.cb` became `struct_pattern_through_ref_ok.cb`; the statement
  `S { a, b } = r;` on a reference (`conf.destructure-reference`) stays refused.

## Trade-offs

- **For:** the fields are named where they are declared, made and matched; no second declaration and no second
  name; the by-reference rule loses its one exception.
- **Against:** a variant has two forms of payload; a reader meets `E::V { … }` where a struct literal was
  `S { … }`, told apart by the `::`.
- **The reading test:** a reader sees every field named; the one rule not on the line, that names bound
  through a reference are references, is the rule every binder already follows.
