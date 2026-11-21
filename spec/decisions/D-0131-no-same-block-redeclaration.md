# D-0131 — A local is not redeclared in its own block

Status: ACCEPTED (2026-10-01, the owner's choice of the recommended candidate, with binders counted in the block they govern)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §17
Depends on: D-0033 (assignment re-establishes an ended binding), D-0024 (a program's items shadow `std`'s), D-0066 (local constants)
Affects: `spec/05` §1 `rule.value-object.binding-form` (1.4.0), `spec/14` §3 (`for`), `spec/17` §2 (local constants), `spec/registry/diagnostics.md`, `spec/conformance.md`, `spec/examples.md`, the guide §07 and §10, `impl/src/parser.rs`

## Problem

`[Binding-Form]` let a declaration rebind a name already bound in the
same frame: `i32 x = 1; i32 x = 2;` declared a second, separate
variable, and the first lived on, unnamed, to the end of the block —
still holding whatever it held, still lending to whatever borrowed it.
No decision ever argued for it: it came with the value and binding
rules, from Rust.

Rust needs it because its bindings are immutable by default: `let x = x
+ 1;` is how a value "changes" without `mut`. CobaltC has no such
reason. Every local can be assigned (`spec/05`: mutability is a
property of the access path, not the binding), and since D-0033 a moved
or dropped resource binding takes a new value with a plain `v = …`. What
redeclaration in the same block still did was hide a mistake — a second
`i32 total` a few lines below the first, meant as `total =` — and leave
a value alive that no name reaches.

## Candidate mechanisms

1. **Keep it** (Rust): any declaration may shadow, in any block.
2. **No shadowing at all** (Java, C#, Zig): a local may not reuse the
   name of any local in scope, nested blocks included. Adding a
   variable to an outer block can then break code inside it.
3. **Not in the same block** (C, C++): a declaration may not reuse a
   name already declared in its own block; a nested block may shadow,
   and a local may shadow an item. Selected.

## Decision

    [Binding-Form-Redeclared]   disposition: rejected (static)
        a declaration of name x in block B,
        x already declared in B's declaration region
        ────────────────────────────────────────────
        ill-formed; diag.duplicate-local

**The declaration region** of a block is its own statements together
with the names that govern it:

- a function's or a closure's parameters (and a closure's captures) for
  its body block;
- a `foreach`'s names for its body;
- a `for` header's declarations for its body;
- an arm's pattern binder for the arm's value when that value is a
  block, and an `if`- or `while`-pattern's binder for its body.

So `fn f(i32 n) { i32 n = n * 2; }` and `foreach (x in &v) { i32 x = 0; }`
are rejected, as in C++. Two binders of one region clash too: two
parameters of one function, two names of one `foreach`, two fields of
one destructuring bound to the same name.

**What counts as a declaration:** `τ x = e;`, `τ x;`, `auto x = e;`, a
destructuring's binders, a local `const` (D-0066: constants and
variables share the region), and the binders above. Local `struct` and
`enum` types are another namespace and are not affected.

**What stays allowed:**

- **A nested block** may declare a name an enclosing block has
  declared: `i32 x = 1; { i32 x = 2; }`. Forbidding it (candidate 2)
  would make code fragile: a variable added to an outer block would
  break inner code that never mentioned it.
- **A local may shadow an item**, a module's or `std`'s: `i32 max = 3;`
  inside a function is allowed. D-0024 depends on it: new `std` names
  must never break a program.
- **Reuse after the region ends**: two sibling blocks, or two `match`
  arms, may each declare `x`.

**The message** names the earlier declaration's line and the two ways
on: assign to the existing variable (`x = …`, after a `drop` for a live
resource), or choose another name — the only way when the type changes
(`String text = …; i32 n = parse_i32(&text)…;`).

The rule is checked when the program is parsed, before any type is
known; it is the same in both implementations.

## Compatibility

Breaking: a program that redeclares a name in its own block, or a
parameter or loop variable in its body, is rejected. The repair is
mechanical — assignment or a new name — and changes no meaning. `spec/14`
§3's note that a `for` body "shadows" the header's variable is
withdrawn, as is the alternative `drop(v); Vec<i32> v = Vec::new();` in
`spec/examples.md` (`drop(v); v = Vec::new();` remains).

## Revisit

A local `struct` or `enum` declared twice in one block is not decided
here.
