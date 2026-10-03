# CobaltC Modules

Status: normative artifact
Version: 2.14.0
Conforms to: `spec/02-schema.md` (Kind: Rule, `rule.module.*`)
Governed by: `CobaltC_Master_Instructions.md` §23
Realizes: D-0017 (modules), D-0021 (§5, file-backed declarations), D-0024 (module imports, `std`), D-0136 (re-export)

## Purpose

Name resolution and visibility for items. Modules introduce no
invariant: every rule about objects, authority, aliasing, and validity
applies unchanged across module boundaries, and a module boundary is
neither a trust boundary (`inv.trust-transition` concerns data from
outside the program) nor an authority boundary.

## 1. Items and qualified names

### `rule.module.resolve`
**Status:** ACCEPTED

`Σ.items` (`spec/04`) maps a **qualified name** — a path
`m1::…::mk::name` from the root module — to its item. Every `fn`,
`extern fn`, `struct`, `enum`, and `module` declaration (in either of
`spec/22` §3's forms, §5) in module `M` adds `M::name`; an associated function `fn T::name(…)` declared in
module `M` where `T` is a struct/enum declared in `M` adds
`M::T::name` (`spec/22` §3); declared in a module that does not itself
declare `T`, it is `[Assoc-Fn-Foreign-Type]` (`CHG-0033`). Every variant `Vi` of `enum M::E` is
addressable as `M::E::Vi`. The standard library is the root-level
module `std` (`spec/21` §0), so its items are `std::Vec`, `std::printf`,
… like any other module's.

A module's items also include the names its `export import`
declarations add (§3, D-0136). Each such name is an **alias**: it
denotes the item it names, not a new item. So `std::Vec` and
`std::collections::Vec` are one type. In every clause below, and in
`[Resolve-Qualified]`, "an item of `M`" includes `M`'s aliases. An alias
is exported.

    [Resolve-Unqualified]
        name is not bound in any frame of the current thread (rule.value-object.binding-lookup)
        exactly one of the following yields an item, tried in order:
            (1) M::name, where M is the module lexically containing the reference;
            (2) the target of an `import p;` in M whose last segment is name;
            (2b) M''::name, exported and not a module, for a module M'' that an `import p;` in M
                 names (CHG-0033; D-0136: a submodule is reached by its path or its own import);
            (3) M'::name for the nearest enclosing module M' of M, repeating outward to the root,
                unless name is an intrinsic's (spec/21 §0) (D-0055);
            (4) E::name for exactly one enum E declared in M or, when no enum of M has a variant
                name, in the nearest enclosing module where one does (variant names);
            (4b) only when (4) finds no enum: E::name for exactly one enum E an `import p;` in M
                names, or exported by a module an `import p;` in M names (CHG-0041)
        ────────────────────────────────────────────
        ⟨name, Σ⟩ → ⟨the item: a fn value p for fn/extern items; the type for struct/enum items in
                      type position; the variant constructor for (4)⟩

    [Resolve-Qualified]
        p = q1::…::qk::name;  q1 resolves per [Resolve-Unqualified] to a module or type;
        each qi+1 is an item of qi;  name ∈ items of qk
        visible(qk, name, M)
        ────────────────────────────────────────────
        ⟨p, Σ⟩ → ⟨the item⟩

    [Resolve-Ambiguous]   disposition: rejected   step (2), (2b), (4) or (4b) yields more than one candidate   ill-formed; diag.ambiguous-name
    [Resolve-Not-Visible] disposition: rejected   ¬visible(qk, name, M)                              ill-formed; diag.name-not-visible
    [Resolve-Unbound]     disposition: rejected   nothing yields an item                             ill-formed; diag.unbound-name
    [Item-Duplicate]      disposition: rejected   two declarations add the same qualified name       ill-formed; diag.duplicate-item
    [Assoc-Fn-Foreign-Type] disposition: rejected `fn T::name` in M, T not a struct/enum declared in M   ill-formed; diag.foreign-associated-fn

An item named as an intrinsic (`spec/21` §0) hides the intrinsic only
where clause (1) or (2) finds it: in its own module and where an
`import` names it (D-0055). Clause (3) does not carry the name outward,
so a program's own `fn sizeof` at its root is not what `std`'s code, a
nested module's or a file-backed module's calls `sizeof`; they reach the
intrinsic. Without that, `std`'s `Vec`, `String`, `Box`, … stopped
working in any program that declared an item of an intrinsic's name at
its root, and a library file behaved differently depending on the
program that loaded it.

A variant of the program's own enums is therefore found before a
variant of an imported one, as its own items are found before imported
ones (clause 1 before 2 and 2b), so an imported module (`std`
included) that gains an enum never makes an existing program's variant
names ambiguous. `E::V` always names `E`'s variant.

`[Item-Duplicate]` (`CHG-0032`) counts every name this section adds —
`M::name` for a `fn`, `extern fn`, `struct`, `enum` or `module`,
`M::T::name` for an associated function, `M::E::V` for a variant — in
one namespace. An alias (§3) counts too: an `export import` that would
give `M` a name `M` already has for a *different* item is
`[Item-Duplicate]`, located at the `export import`. The same item
brought in twice, by two re-exports or by a chain that returns to
`M`, is one name and no duplicate. `std`'s declarations are the root-level module `std`'s
items (`CHG-0033`), so a program's own `printf` adds `printf`, not
`std::printf`, and is no duplicate; a program cannot declare a root
module named `std`. The same simple name in two modules is two
qualified names. Location: the later declaration.

**Resolution order and `std`** (`CHG-0033`). A module's own
declarations (clause 1) come before anything imported, and single-name
imports (2) before module imports (2b), so a module declaring `printf`
and importing `std` calls its own `printf`; `std::printf` stays reachable
by its qualified name. Ambiguity between two imported modules arises
only where the shared name is used, so a module gaining an export never
invalidates a program that does not name it.

Generic instantiation on a path: `p<σ1,…>` (`spec/22` §2) selects
the item and supplies type arguments (`rule.fn.generic-call`,
`rule.agg.generic-decl`).

**Depends on:** term.module, term.binding, rule.value-object.binding-lookup

## 1a. Constants

### `rule.module.const`
**Status:** ACCEPTED

`const τ N = e;` (D-0036) declares the item `M::N` in its module `M`,
with the visibility, import and resolution of any item (§1–§3). A use
of `N` in an expression is `N`'s **value**: the value of `e`, as
evaluated in a function of no parameters and no locals whose result
type is `τ` (so `e` is typed with `τ` as its expected type, exactly as
a declaration `τ x = e;` types it).

    [Const-Use]
        N resolves to the constant M::N = e of type τ     ⟨e, Σ⟩ →* ⟨v, Σ'⟩ with no effect on Σ
        ────────────────────────────────────────────
        ⟨N, Σ⟩ → ⟨v, Σ⟩

    [Const-Not-Constant]   disposition: rejected
        is-resource(τ), or e is not a constant expression
        ────────────────────────────────────────────
        ill-formed; diag.const-not-constant

    [Const-Cycle]          disposition: rejected
        the value of N depends, through the constants its initializer uses, on N itself
        ────────────────────────────────────────────
        ill-formed; diag.const-not-constant

    [Const-Checked-Failure]   disposition: rejected
        evaluating e reaches a checked failure d (an overflow, a division by zero, …)
        ────────────────────────────────────────────
        ill-formed; d, reported at the constant

A **constant expression** is a literal; another constant; a unary or
binary operator applied to constant expressions; a struct, array or
enum-variant literal of constant expressions; a field of a constant
expression, or an element of one at an index that is a constant
expression (D-0066: `SCREEN.w`, `PRIMES[2]`); the name of a function
(a `fn` value); or `min_value`, `max_value`, `sizeof`, `alignof`,
`widen`, `narrow`, `narrow_wrapping`, `reinterpret`, `to_float`,
`to_int`, `wrapping_*` or `saturating_*` of constant expressions. None
has an effect, so when `e` is evaluated is not observable, and an
implementation computes it before the program runs where it can; a
checked failure in it is a static rejection at the constant.

- **A value, not a place.** A constant cannot be assigned to or
  borrowed (`&N` is `[Ref-Form-Temporary]`); each use is a new copy.
- **Not a resource.** A resource constant would give a new resource
  at every use, destroyed where it ends — a hidden allocation per use.
- **Typing is a declaration's.** `e` is typed as in `τ x = e;`, so
  `const u64 MIB = 1024 * 1024;` computes in `u64` (D-0037), and
  `const i32 BIG = 65536 * 65536;` overflows.
- **Array lengths.** `array<τ, N>` and `[e; N]` take a constant
  expression (`spec/22` `length`, D-0126): an integer literal, the name
  of a constant whose initializer is itself such an expression (D-0074;
  a bare name, or a qualified `m::N`, D-0105, resolved as
  `[Resolve-Qualified]` resolves it, from the current module outward,
  and visible there — exported, or used inside its own module), or
  `+ - * / % & | ^ << >> ~ ( )` over them, folded before the program
  runs; a constant may name one declared after it. A length that is not
  such an expression, is negative, divides by zero or overflows is a
  syntax error.
- **`const auto N = e;`** (D-0068) gives `N` the type `auto x = e;`
  would give `x` (`rule.type.typing`): `e`'s own type, a bare integer
  literal `i32` and a float `f64`, an operator's the type of its operand
  that is not a bare literal. When `e` alone does not determine a type
  (`None`, a generic struct literal without its type arguments), the
  constant is `diag.type-mismatch` and the type must be written.

**Local constants** (D-0066). `const τ N = e;` may also be a statement
of a block. It declares a constant exactly as a module's does — the
same typing, the same `[Const-Not-Constant]`, `[Const-Cycle]` and
`[Const-Checked-Failure]` — but its name is **scoped like a local**:
from the statement to the end of its block, shadowing a name of an
enclosing scope or an item, and shadowed by a local of the same name in
a nested block, as a local variable is (`spec/05`); a variable or
constant of the same name in its own block is `diag.duplicate-local`
(D-0131). It is not a local variable:
it names no object, needs no capture in a closure, and is a constant
expression, so `static_assert` may use it.

    [Const-Local]
        const τ N = e;  is a statement of a block B in a function f, with no local variable
        of f and no type parameter of f named in τ or e
        ────────────────────────────────────────────
        within B after the statement, N resolves to a constant of the module of f with type τ
        and initializer e, as a module constant `const τ N' = e;` for a name N' no program
        can write

    [Const-Local-Not-Constant]   disposition: rejected
        e names a local variable of f (a variable is not a constant), or τ or e names a type
        parameter of f (a constant has one value, not one per instantiation)
        ────────────────────────────────────────────
        ill-formed; diag.const-not-constant

- **Why not a variable?** A local variable initialized by a constant
  expression is still a variable: any later statement may write it, so
  its value is not known where a `static_assert` stands. Writing
  `const` says the name never changes.
- **In patterns.** A local constant is a pattern as a module constant is
  (`spec/16`, D-0058): `SPACE : …` tests for the value.

**Depends on:** rule.module.resolve, rule.module.visibility,
rule.type.typing, rule.type.expected, rule.arith.literal, D-0036

## 1b. Static assertions

### `rule.module.static-assert`
**Status:** ACCEPTED

    static_assert(e);    static_assert(e, "message");      -- an intrinsic call (`spec/21` §0), type unit
    static_assert(e, "format", a1, …, an);                 -- D-0068: a message formatted as printf's,
                                                           -- each ai a constant expression

    [Static-Assert]
        e : bool is a constant expression (§1a), in the function body where the call is, with that
        body's type arguments (so `sizeof<T>()` names the instantiation's T); the message, if any,
        a `str-literal`
        the value of e (§1a: computed as at run time, before the program runs) is true
        ────────────────────────────────────────────
        the program is well-formed as far as this assertion goes; at run time the call does nothing

    [Static-Assert-Failed]   disposition: rejected
        the value of e is false, for the function as written or for one instantiation of a generic
        function whose body holds the call
        ────────────────────────────────────────────
        ill-formed; diag.static-assert-failed, with the message

    [Static-Assert-Not-Constant]   disposition: rejected
        e is not a constant expression, or not of type bool; or an argument of the message is
        not a constant expression
        ────────────────────────────────────────────
        ill-formed; diag.static-assert-not-constant (D-0067)

    [Static-Assert-Checked-Failure]   disposition: rejected
        computing e reaches a checked failure d (an overflow, …)
        ────────────────────────────────────────────
        ill-formed; d, reported at the assertion

A message may carry values, as `assert`'s does (D-0068): a format and
its arguments, each a constant expression (otherwise
`[Static-Assert-Not-Constant]`), checked as `printf`'s
(`diag.format-invalid`, `[Format-Arg-Mismatch]`) and formatted, before
the program runs, only when the assertion fails.

A condition that is not a `bool` is `[Static-Assert-Not-Constant]`,
whose diagnostic says what to write instead: `assert` (`spec/18` §1a)
for a condition only known as the program runs, or a `const` (a local
one works) to name a value known before. A type error inside the
condition is that error, and a message that is not a string literal is
`diag.type-mismatch` (`rule.type.typing`). Every function is
checked whether or not it is called (`rule.fn.program`), so an assertion
holds for the program as a whole, not for a run of it. In a generic
function it is checked for each instantiation the program uses
(D-0026): `static_assert(sizeof<T>() <= 16, "…")` bounds `T`.

**Depends on:** rule.module.const, rule.type.kind, D-0026, D-0052

## 1c. Local types

### `rule.module.local-type`
**Status:** ACCEPTED

A `struct`, `enum` or `bitstruct` may be declared as a statement of a
block (D-0069, D-0132),
and with it, in the same block, functions of it: `fn T::name(…)`, a
destructor `fn T::drop(…)` included. Such a **local type** is a type of
the module in all but the scope of its name: from its declaration to
the end of its block (its own declaration included, so it may be
recursive), shadowing and shadowed by a type of the same name as a
local constant is (§1a). Types and values are separate names: a local
variable does not hide a local type of the same name.

    [Local-Type]
        struct-decl, enum-decl or bitstruct-decl T is a statement of a block B in a function f
        ────────────────────────────────────────────
        within B after the statement, T resolves to a type of f's module, with the fields,
        variants and type parameters written, under a name no program can write; each
        `fn T::g` declared in B is a function of that type

A local type's declaration and its functions see what the block's
items do — module items, and the local types and constants in scope —
but not the enclosing function's variables or type parameters (a type
has one definition, not one per call or instantiation): naming them is
`diag.unbound-name`. A local type cannot be exported, and a function
declared in a block must be a function of a local type declared there
(a local function is written as a closure); anything else is a syntax
error.

**Depends on:** rule.module.resolve, rule.module.const, D-0069

## 2. Visibility

### `rule.module.visibility`
**Status:** ACCEPTED

Every item is `export` or private (default). `visible(M', name, M) ≝
M'::name is export ∨ M is M' or nested inside M'`. A private item is
usable from its own module and its sub-modules. An `export` item
inside a private module is reachable only where that module is.
Purely lexical; always `disposition: rejected`.

A struct's **fields** are items of the struct for visibility
(`spec/22` §3: `field ::= vis type identifier`). A field is named by
a field access `e.f` (`[Field-Access]`, `spec/16` §2), by a struct
literal `S { …, .f = e, … }` (`[Struct-Construct]`), and by a
destructuring `S { f1, …, fn } = e` (`[Let-Destructure]`,
`spec/11`); each is well-formed in module `M` only if the field is
visible there, where `M'` is the module declaring `S`:

    [Field-Not-Visible]   disposition: rejected   a field f of S named in M;  ¬visible(M', f, M)   ill-formed; diag.name-not-visible

A private field is therefore usable by `S`'s own module and its
sub-modules — the module keeps full control of the struct's
invariants — while an `export` field is usable wherever `S` is.

An alias (§3) is exported: `export import p;` passes on only what is
visible and exported already, so it never makes a private item
reachable. A private item of a module is not brought in by any import.

**Depends on:** term.module, rule.agg.struct-construct, rule.init.let

## 3. `import`

### `rule.module.use`
**Status:** ACCEPTED

`import p;` in module `M` makes `p`'s last segment resolve, within
`M`, as `p` would (`[Resolve-Unqualified]` step 2). When `p` names a
module, it also makes every exported item of that module resolve
unqualified within `M` (step 2b) except its submodules, which are reached
by their paths (`std::text`) or imported themselves (D-0136), so a
library's submodule never hides a module of the importing program, and the variants of its exported
enums (step 4) — `import std;` is how a program uses the standard
library (`spec/21` §0, `CHG-0033`). An import acts only in the module
that declares it; a nested module, or the body of a file-backed module,
declares its own. It introduces no binding and no `Σ` structure. `import p as x;` is not provided
(`spec/22` §5: renaming imports are a deliberate absence). The rule id
`rule.module.use` is unchanged from when the keyword itself was `use`
— stable per `spec/02-schema.md` §1, the same treatment `rule.init.let`
got when `let` was removed (`CHG-0001`).

**Re-export** (D-0136, `CHG-0163`). An `import` takes a visibility as
any other declaration does (`spec/22` §3). `export import p;` in `M`
does everything `import p;` does in `M`, and also adds to `M`'s items
an alias (§1) for each name it brings in:

- if `p` names a module, each exported item of that module, under its
  own name (its own aliases included, so re-exports chain);
- if `p` names one item, that item, under `p`'s last segment.

A program that imports `M`, or names `M::name`, then finds those
names as `M`'s own. This is how a library divided into modules is
used with a single `import`: `std`'s root re-exports each of its
submodules (`spec/21` §0), so `import std;` brings in the whole
library and `std::collections::Vec` is `std::Vec`. A re-export cycle
(`M` re-exports `N`, `N` re-exports `M`) adds each item once and is
accepted.

    [Reexport-Unbound]  disposition: rejected   `export import p;` and p names nothing     ill-formed; diag.unbound-name

A diagnostic that names an item which has several paths names it by
its shortest one (`std::Vec`, not `std::collections::Vec`):
implementation-defined.

**Depends on:** rule.module.resolve

## 4. `unsafe` and modules

`unsafe { }` (`spec/20` §1) is lexical; no module-level gating exists.
A module that wants to confine `trusted-unchecked` operations simply
leaves the functions wrapping them un-exported (D-0017: a considered
"no").

## 5. File-backed module declarations

### `rule.module.file`
**Status:** ACCEPTED (D-0021, `CHG-0026`)

`vis module m "p";` in module `M`, written in file `F`, declares
module `M::m` exactly as `vis module m { … }` would, with the body
taken from the file `p` names (`spec/22` §3, `module-decl`'s second
alternative). The declaring site names the module; the file's top
level is anonymous, as an inline body is. The file is parsed as an
`item*` — an item sequence, not a program: it has no `main`
requirement, and a `fn main` it declares is the ordinary item
`M::m::main`, not the program's entry (`rule.fn.program`). The
declaration introduces no binding and no `Σ` structure beyond what
the equivalent inline form does; `rule.module.resolve` and
`rule.module.visibility` apply to the body unchanged, and `vis` on
the declaration governs the module as it does inline.

`p` is resolved against the directory containing `F`; a file-backed
declaration inside `p` resolves its own path against `p`'s directory.
A conforming implementation must accept a `/`-separated relative path
with no `..` segment and no leading `/`; whether any other form (an
absolute path, `..`, a platform separator, an environment or
home-directory expansion) is accepted, and how the resolved path
denotes a file, is `outcome: impl-defined` (`spec/22` §5). The
literal's escape set is `str-literal`'s (`spec/22` §1); it is a path
in item position, not an expression, and has no type.

    [Module-File]
        `vis module m "p";` occurs in module M in file F
        p, resolved against F's directory, names a file whose contents parse as item* I
        ────────────────────────────────────────────
        the declaration is `vis module m { I }` in M

    [Module-File-Missing]    disposition: rejected   p names no readable file                                    ill-formed; diag.module-file-not-found
    [Module-File-Cycle]      disposition: rejected   p names F, or a file whose body is being assembled to reach F   ill-formed; diag.module-cycle
    [Module-File-Duplicate]  disposition: rejected   two declarations in one program name the same file          ill-formed; diag.module-file-duplicate

A file whose contents do not parse as `item*` is a parse failure of
the program, as any other syntax error is — not a diagnostic. This
mechanism is source assembly into one program (`term.program` is
unchanged: a finite set of items in one root module); it is not
linking, which remains a deliberate absence (`spec/22` §5).

**Depends on:** term.module, rule.module.resolve, rule.module.visibility, D-0021

## Change Log

- 2.14.0 — `CHG-0163` (D-0136): re-export. `import` takes a visibility;
  `export import p;` adds aliases to the module's items (§1, §3);
  `[Item-Duplicate]` counts aliases; `[Reexport-Unbound]`. Clause (2b)
  brings no submodule: a module import makes a module's exported items
  other than modules resolve unqualified.
- 2.13.0 — `CHG-0155` (D-0132): a `bitstruct` may be a local type.
- 2.12.0 — `CHG-0154` (D-0131): a local constant is not redeclared in
  its own block.
- 2.11.0 — `CHG-0149` (D-0126): §1a's array-length bullet: a constant
  expression.
- 2.10.0 — `CHG-0124` (D-0105): an array length may name another
  module's exported constant.
- 2.9.0 — `CHG-0115`: §1a's array-length bullet states D-0074 (a constant
  whose initializer is an integer literal); it still said "not a
  constant". Editorial: spec/22 already had the rule.
- 2.8.0 — `CHG-0079` (D-0069): §1c (new) `rule.module.local-type`.
- 2.7.0 — `CHG-0078` (D-0068): `const auto`; `static_assert`'s message may
  be a format with constant arguments.
- 2.6.0 — `CHG-0077` (D-0067): `[Static-Assert-Not-Constant]` reports
  `diag.static-assert-not-constant`, and covers a condition that is not a
  `bool`.
- 2.5.0 — `CHG-0076` (D-0066): §1a local constants (`[Const-Local]`,
  `[Const-Local-Not-Constant]`); a constant expression may take a field
  or an element of a constant expression.
- 2.4.0 — `CHG-0063` (D-0055): `[Resolve-Unqualified]` clause (3) does not
  apply to an intrinsic's name.

- 2.3.0 — `CHG-0060` (D-0052): §1b (new) `rule.module.static-assert`.

- 2.2.3 — Non-normative (`CHG-0052`, D-0044): destructuring names
  every field.

- 2.2.2 — Non-normative (`CHG-0047`, D-0039): examples of `std`'s
  names use `printf`, `print` being private to `std`.

- 2.2.1 — Non-normative (`CHG-0045`, D-0037): §1a's typing note follows
  the new literal-expression typing.

- 2.2.0 — `CHG-0044` (D-0036): §1a (new) `rule.module.const`
  (`[Const-Use]`, `[Const-Not-Constant]`, `[Const-Cycle]`,
  `[Const-Checked-Failure]`).

- 2.1.0 — `CHG-0041` (D-0032): `[Resolve-Unqualified]` clause (4) finds a
  variant name among the enums of M, else of the nearest enclosing
  module that has one; the new clause (4b) looks among imported enums
  only when (4) finds none. `[Resolve-Ambiguous]` covers (4b).

- 2.0.0 — `CHG-0033` (D-0024): `import p;` naming a module also brings
  that module's exported items (new `[Resolve-Unqualified]` clause 2b)
  and its exported enums' variants (clause 4, which now also counts an
  imported enum); `[Resolve-Ambiguous]` covers clauses 2, 2b and 4.
  `[Assoc-Fn-Foreign-Type]` (`diag.foreign-associated-fn`) states the
  rejection §1's associated-function sentence implied. The standard
  library is the module `std`; `[Item-Duplicate]`'s paragraph no
  longer treats it as part of the root.
- 1.6.0 — `CHG-0032`: `rule.module.resolve` gains `[Item-Duplicate]`
  (`diag.duplicate-item`): each qualified name is added by at most one
  declaration, the prelude's included. Before, the rule's map from name
  to item left a second declaration unspecified.
- 1.5.0 — `CHG-0027`: `rule.module.visibility` gains
  `[Field-Not-Visible]`, making the field visibility `spec/22` §3's
  `field` production has promised since 2.0.0 an actual rule: a field
  named by an access, a struct literal, or a destructuring must be
  visible at the naming site. `diag.name-not-visible`, no new id.
- 1.4.0 — `CHG-0026` (D-0021, human-directed): new §5, `rule.module.file` — a `module` declaration
  whose body is a file, defined by equivalence to the inline form;
  path resolution against the declaring file's directory; three
  static rejections (missing file, cycle, duplicate). §1's
  enumerating sentence notes both declaration forms. No existing
  rule's semantics changed; `rule.module.use` is untouched.
- 1.3.0 — `CHG-0006`: `use` renamed `import`, `pub` renamed `export`
  throughout (§1 resolution order, §2 visibility, §3's own heading and
  body), matching `spec/22` 2.5.0. `rule.module.use`'s id is
  unchanged. No rule semantics changed.
- 1.2.0 — `CHG-0004`: illustrative `p::<σ1,…>` re-spelled `p<σ1,…>`
  to match `spec/22` 2.3.0's turbofish removal. No rule semantics
  changed.
- 1.1.0 — `CHG-0001`: `mod` renamed `module` in prose to match
  `spec/22` 2.0.0; no rule semantics changed.
- 1.0.0 — Rewritten (`spec/AUDIT-2.md` B-19, B-20): resolution order
  for unqualified names including `use`, enclosing modules, and
  variant names; qualified paths through modules and types;
  ambiguity and visibility diagnostics. All entities `ACCEPTED`.
- 0.3.0 and earlier — superseded.
