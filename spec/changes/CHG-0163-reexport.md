# CHG-0163 — Re-export: `export import p;`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-03; D-0136)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0136
Affects: `spec/17` (2.14.0) §1, §2, §3; `spec/22` (2.45.0) §3; `spec/conformance.md`; the guide; `impl/src/parser.rs`, `impl/src/ast.rs`, `impl/src/modres.rs`

## What changed

- **Grammar** (`spec/22` §3): `import-decl ::= vis 'import' path ';'`.
- **`rule.module.use`** (`spec/17` §3): `export import p;` in `M` does what `import p;` does and adds an alias to `M`'s items for each exported item of the module `p` names (aliases included), or for the item `p` names. `[Reexport-Unbound]`.
- **`rule.module.resolve`** (`spec/17` §1): a module's items include its aliases, in every clause of `[Resolve-Unqualified]` and in `[Resolve-Qualified]`; an alias denotes the item itself. `[Item-Duplicate]` counts an alias for a different item; the same item twice is one name.
- **`[Resolve-Unqualified]` (2b)** brings an imported module's exported items other than its submodules; a submodule is reached by its path or by an import of its own. Found by the battery: `import std;` in a nested module made `std::memory` hide the program's own root module `memory` (`showcase/tier3/tinyos.cb`). No case in the repository used an imported module's submodule unqualified.
- **`rule.module.visibility`** (`spec/17` §2): an alias is exported; a re-export never makes a private item reachable.
- **Previous semantics:** `export import` was `diag.syntax-error`.
- **Rows:** `conf.reexport-module`, `conf.reexport-item`, `conf.reexport-item-only`, `conf.reexport-chain`, `conf.reexport-same-item-twice`, `conf.reexport-conflict-rejected`, `conf.reexport-cycle`, `conf.reexport-private-not-passed`, `conf.reexport-unbound-rejected`, `conf.module-import-no-submodule`.

## Compatibility classification

Additive: programs that were rejected now run.
