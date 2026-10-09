# D-0021 — File-Backed Module Declarations

Status: ACCEPTED (2026-09-22)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §10, §11, §22
Depends on: term.program, term.module, rule.fn.program, rule.module.resolve, rule.module.visibility, rule.module.use, D-0017, `spec/22-surface-syntax.md`
Affects: rule.module.file, feat.module-file, diag.module-file-not-found, diag.module-cycle, diag.module-file-duplicate

## Problem

A CobaltC program is "a finite set of items in one root module"
(`term.program`, `rule.fn.program`). The specification never says how
that set is *spelled*: `spec/22` §3 gives one grammar for items, and
`spec/17` organizes them into modules, but the only way to write a
module today is inline, `module m { … }`, so every item of a program
lives in one source text. The human owner asked how a module in a
different file is imported. The answer under 1.3.0 is that it cannot
be: the reference interpreter takes one file (`coby <file.cb>`), and
nothing in the language names a second one.

The question is not whether a program may span files — nothing in
`rule.fn.program` forbids it — but which construct spells the
assembly, and what it may and may not add to the semantics. Modules
were derived (D-0017) as "the minimum needed for name resolution and
visibility, nothing more," with no invariant and no `Σ` structure of
their own. A file mechanism must not quietly become the place where
that changes.

## Constraints

- §9: smallest sufficient mechanism. The file mechanism must add no
  `Σ` component and no invariant; `rule.fn.program`'s `Σ_0.items` is
  still "the program's declarations," however they were assembled.
- §10 / `spec/22`'s Purpose: surface syntax introduces no meaning of
  its own. Whatever the construct is, its meaning must be stated as
  an equivalence to syntax that already has one.
- `spec/22` §5: `import … as` renaming is a deliberate absence. A
  file mechanism in which the *importer* chooses a name for something
  the file already names is renaming through the back door.
- `spec/22` §5: linking, ABI, and calling convention are deliberate
  absences. A file mechanism must be source assembly into one
  program, not separate compilation of several.
- `rule.module.use`: `import p;` "introduces no binding and no `Σ`
  structure." Whatever loads a file must not be `import`, or that
  sentence stops being true.
- `spec/22` §2 rule 4: a declaration statement is recognized by its
  leading identifier naming a struct or enum, and the grammar assumes
  the full item set is known before statements are parsed. A file
  mechanism must not make an item's type names invisible to the
  statements of another file in the same program.
- Reproducibility: `spec/conformance.md` and `impl/conformance/` run
  on any machine. A path form that names a particular user's home
  directory or environment cannot appear in a conformance case.

## Candidate mechanisms

1. **Status quo.** One file; organize with inline modules;
   concatenate by hand for larger programs. Rejected: does not address
   the problem — hand concatenation *is* candidate 6 minus the name
   and the visibility boundary, done outside the language.

2. **A file clause on `import`** — `import inner from file("…");`,
   the owner's first sketch. `import` would both shorten lookup (its
   only job today) and load a file into `Σ.items`. Rejected on two
   constraints: `rule.module.use` would acquire `Σ`-changing
   semantics, and the name written before `from` is the importer's
   choice for the file's contents — renaming, which `spec/22` §5
   rejects. The two jobs are also visibly different: one is a
   resolution-order fact, the other is a parse-time fact.

3. **Conventional mapping** — Rust's bodiless `mod foo;`, with the
   file found by a directory convention (`foo.cb`, `foo/mod.cb`).
   The declaration shape is right (see 6) but the file is found by a
   rule the specification would have to state in terms of
   directories, which it otherwise never mentions. Rejected for now:
   one explicit token (the path) removes the convention entirely, and
   a convention can be layered on later without changing the
   declaration's meaning. A revisit condition.

4. **Textual include** — `#include`-style: the file's items are
   spliced into the *current* module with no name. Rejected: it
   bypasses `term.module` ("a named container of items governing
   visibility and qualified names"). Two included files' items land
   in the same module and can collide; `rule.module.visibility`
   cannot tell an included private item from a local one; nothing
   can be reached by qualified path. It is also the C mechanism whose
   failure modes (double inclusion, order dependence) motivated
   C++20's `module`/`import`/`export`, which `CHG-0001`/`CHG-0006`
   chose as this language's vocabulary.

5. **Separate compilation** — each file a unit with an interface,
   combined by a linker. Rejected: a `spec/22` §5 absence, an order of
   magnitude larger than the problem, and no invariant motivates it.

6. **A file-backed `module` declaration** — the inline form with the
   body replaced by a path:

       module geo "./geometry.cb";

   meaning exactly `module geo { <the items geometry.cb parses to> }`,
   with `geo` named by the declaring site, as the inline form's body
   is. **Selected.**

   Within 6, two spellings were considered and rejected as redundant
   tokens, on the reasoning that removed `let` (`CHG-0001`) and the
   turbofish (`CHG-0004`): a `from` keyword (`module geo from "…";`)
   and a `file(…)` wrapper (`module geo from file("…");`). After
   `module identifier` the only legal continuations are `{` and a
   string literal, so neither word disambiguates anything; they only
   add a keyword (`from`) or a pseudo-call that is not a call
   (`file`). C's `#include "x.h"` shows a bare literal in this position
   reads fine.

## Selected design

Candidate 6, as `rule.module.file` (`spec/17` §5) and one grammar
alternative (`spec/22` §3):

    module-decl ::= vis 'module' identifier ( '{' item* '}' | str-literal ';' )

- **Equivalence.** `vis module m "p";` in module `M`, written in file
  `F`, is `vis module m { I }` where `I` is the `item*` the file `p`
  names parses to. `rule.module.resolve` and `rule.module.visibility`
  apply to `I` unchanged; `vis` on the declaration governs the module
  as it does inline; each item in the file carries its own `export`.
- **The file is an item sequence, not a program.** It has no `main`
  requirement; a `fn main` in it is the ordinary item `M::m::main`
  and is not the program's entry (`rule.fn.program` requires the
  program's `main` at the root). The root module is the entry file's
  top level; how the entry file is named is an implementation matter,
  as `coby <file.cb>` already is.
- **Path resolution.** `p` is resolved against the directory
  containing `F`. A file-backed declaration inside `p` resolves its
  own path against `p`'s directory, so a library can be moved as a
  unit. A conforming implementation must accept a `/`-separated
  relative path with no `..` segment and no leading `/`; whether any
  other form (an absolute path, `..`, a platform separator, an
  environment or home-directory expansion) is accepted, and how the
  resolved path denotes a file, is `outcome: impl-defined` and
  documented under `spec/22` §5. Conformance cases use only the
  required core.
- **The literal is a path, not an expression.** Its escape set is
  `str-literal`'s (`spec/22` §1); it has no type and is never
  evaluated.
- **Three static rejections**, `disposition: rejected`: the path
  names no readable file (`diag.module-file-not-found`); the file is
  one whose body is currently being assembled to reach this
  declaration — a cycle (`diag.module-cycle`); the same file is named
  by two declarations in one program (`diag.module-file-duplicate`).
  A file whose contents do not parse as `item*` is a parse failure of
  the program, as any other syntax error is.
- **Nothing else changes.** `term.program`, `term.module`,
  `rule.fn.program`, `rule.module.resolve`, `rule.module.visibility`,
  and `rule.module.use` keep their text and meaning. `Σ` gains no
  component. No invariant is added.

## Rejected alternatives

1–5 as above; `from` and `file(…)` within 6.

Also rejected within 6: **allowing the same file twice** under two
names. The equivalence makes it well-defined — two modules, two
distinct `struct Rect` types, exactly as pasting the text twice
would — but it is the surprising reading of "the same file," and a
rejection is easy to relax later and hard to tighten. And **treating
the file as a program** (requiring or forbidding `main` in it):
requiring one is nonsense for a library; forbidding one adds a rule
whose only effect is to reject an item that, as `M::m::main`, is
harmless.

## Semantic rationale

The whole decision is that a file-backed declaration is *spelling*,
not *meaning*: it is defined by equivalence to a form that already
has a meaning, so every rule that governs the inline form governs it
without being touched. The three rejections are well-formedness
conditions on the program text (does the item set exist, is it
finite, is it unambiguous), the same category as "the program does
not parse," not semantic rules. This is the framing D-0017's modules
entry insists on — "no deeper invariant surface to over-derive" — and
the framing `rule.module.use` already has ("introduces no binding and
no `Σ` structure"). The one place the language *could* have acquired
new semantics — `import` loading files (candidate 2) — is exactly the
place the constraints closed.

## Usability

    // geometry.cb -- its top level is the module body; nothing names it here
    export struct Rect
    {
        f64 w;
        f64 h;
    }

    export fn area(ref<Rect, shared> r) : f64
    {
        (*r).w * (*r).h
    }

    fn scale_factor() : f64      // private: not export
    {
        1.0
    }

    // main.cb
    module geo "./geometry.cb";

    import geo::Rect;
    import geo::area;

    fn main()
    {
        Rect r = Rect { .w = 3.0, .h = 4.0 };
        f64 a = area(&r);        // via import
        f64 b = geo::area(&r);   // via the qualified path
    }

`geo::scale_factor()` from `main.cb` is `diag.name-not-visible`,
unchanged. The one thing a human must learn is that the declaring site
names the module and the file does not — the same fact as for the
inline form, where the body has no name of its own either.

## Explainability

Three new diagnostics, each with a concrete provenance: the path and
the directory it was resolved against; the declarations on the cycle;
the two declarations naming one file. Every other failure inside a
file-backed module has its existing name (`diag.name-not-visible`,
`diag.unbound-name`, `diag.ambiguous-name`, or a parse failure), and
the equivalence means a reader can always ask "what would this be if
the file's text were pasted here?" and get the answer.

Diagnostic locations inside a file-backed module lie in that file. How
a location names its file is part of "the form in which diagnostics
are reported," already impl-defined (`spec/22` §5).

## Implementation-feasibility

The reference interpreter already does exactly this for the prelude:
`impl/src/lib.rs` concatenates the prelude's source ahead of the user
program into one token stream, *because* the parser's declaration-
statement pre-scan collects `struct`/`enum` names from the whole
stream (`impl/src/parser.rs`, `Parser::new`) and a qualified type
name in statement position is recognized by its last segment. A
separately parsed file would leave `Rect r = …` in another file
mis-parsing as an expression — which is the `spec/22` §2 constraint
above, met by construction when the file's text is spliced in as the
module body before parsing. So the equivalence is not only the
specification's definition but the natural implementation: a pre-parse
pass that finds `[export] module IDENT "…" ;`, reads the file relative
to the current file's directory (with a load stack for cycles and a
seen-set for duplicates), and replaces the declaration with
`module IDENT { … }`, keeping a table from combined line ranges back
to (file, line). The parser, resolver (`impl/src/modres.rs`), item
table, type checker, and evaluator need no change. A compiler does the
same at its front end.

## Compatibility impact

Additive. No keyword is added; a string literal after `module m` was
a parse error under 2.9.0, so no program's meaning changes.
`spec/22` §5's "linking" absence is not contradicted — a sentence is
added there saying so. The conformance-row runner (`impl/tests/
spec_rows.rs`) assembles single-file programs from `spec/conformance.md`
and is unaffected; multi-file cases live in the file-based suite.

## Prior-art status

Rust's bodiless `mod foo;` (with `#[path = "…"]` for an explicit
file) has the selected shape: the declaring site names the module,
the file's top level is anonymous, `use` remains a lookup shortener,
and the crate is one compilation. The derivation here is from
`term.module` and `rule.module.use`'s "no `Σ` structure," not from
that precedent, but it lands in the same place. C's `#include` is
candidate 4. C++20's named modules are candidate 3's shape with the
mapping delegated to the build system.

## Invariant traceability

None, by design — the same recorded absence as D-0017's modules
entry. The mechanism is an equivalence over syntax; the only
invariant-bearing rules it touches are the ones it leaves unchanged.

## Revisit conditions

- A demonstrated need for a **conventional mapping** (candidate 3):
  programs large enough that explicit paths are noise. Expected
  resolution: `module m;` as sugar for `module m "./m.cb";`, an
  additive grammar alternative with no change to `rule.module.file`.
- A demonstrated need to **name the same file twice**: relax
  `[Module-File-Duplicate]`; the equivalence already defines the
  result.
- A demonstrated need for a **search path or package** mechanism
  (library code not reachable by a relative path from the program):
  a separate decision, since it is the first place an environment
  would enter the language's meaning.
- A demonstrated need for `..` or absolute paths **in the conformance
  core**: widen the required path form; no rule changes.
