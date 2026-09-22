# CHG-0001 — C-Style Declarator Surface Syntax

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §21

## Problem / motivation

`spec/22-surface-syntax.md` 1.0.0's concrete syntax read as
Rust-derived at the declarator level (`let x: T = e;`, `x: T`
parameters/fields, `->`-introduced return types, `mod`, colon-based
struct-literal field initializers), which was never a semantic
requirement — `spec/22` itself states it "introduces no meaning of its
own" (§10). The human owner directed a revision toward a C-family
declarator order without changing any semantics, arrived at
incrementally across a design conversation (each fork resolved as a
recorded choice below).

## Affected entities

No `rule.*`, `inv.*`, `type.*`, `state.*`, `term.*`, or `D-XXXX`
entity's *semantic definition* changed — every entity's behavior is
identical before and after this record. Its scope is `spec/22`
(2.0.0); the illustrative concrete-syntax code in
`spec/21-standard-library-semantics.md`, `spec/examples.md`, and
`spec/conformance.md`; and, on a follow-up sweep, small illustrative
fragments quoted in prose across `spec/05`, `07`, `08`, `14`, `15`,
`17`, `19`, `20` (each bumped to 1.1.0; see their own Change Logs).
`spec/decisions/*.md` and `spec/AUDIT*.md` are unaffected and, per
`spec/02-schema.md` §6, immutable once `ACCEPTED`/historical; their
illustrative fragments remain in the pre-2.0.0 concrete syntax as a
historical record and are intentionally not updated.

One entity needed more than a prose re-spelling: `rule.init.let`
(`spec/11` §2). Its formal `[Let]`/`[Let-Uninit]`/`[Let-Destructure]`
rules had the retired `let` keyword baked directly into their
reduction subjects (`⟨let x : τ; , Σ⟩ → …`) — not an illustrative
aside but the rule's own definition of what surface construct it
governs, which after `spec/22` 2.0.0 no longer names anything the
grammar produces. This is restated over the current grammar
(`⟨τ x; , Σ⟩`, `⟨τ x = e; , Σ⟩`, `⟨Name{f} = e; , Σ⟩`), with a new
`[Let-Auto]` rule desugaring `auto x = e;` to the explicit-type form —
the same pattern `[If-No-Else]` already uses for a different piece of
sugar. `rule.init.let`'s id and its actual behavior (store into a
fresh binding) are unchanged; only which concrete tokens it quotes as
its own subject changed, closing a real dangling-reference gap rather
than a cosmetic one.

## Previous semantics (concrete syntax; no rule semantics involved)

`spec/22` 1.0.0: `let`-introduced local declarations; name-first
parameters and struct fields (`x: T`); `->`-introduced return types on
`fn-decl`, `extern-decl`, and the `fn(...)` type constructor; `mod` as
the module keyword; colon-based struct-literal field initializers
(`Name { f: e }`), comma-terminated struct fields.

## New semantics (concrete syntax; no rule semantics involved)

`spec/22` 2.0.0: type-first local declarations with no introducing
keyword (`T x = e;`), plus `auto` for inferred locals — disambiguated
from expression statements by whether the leading identifier names a
known struct/enum type (§2 disambiguation (5)); type-first parameters
and struct fields (`T x`); `:`-introduced return types everywhere a
return type is written, with `->` removed from the grammar entirely;
`module` replacing `mod`; C99-style `.f = e` struct-literal
initializers (§2 disambiguation (6)); semicolon-terminated struct
fields. Every production maps to the exact same rule id it did under
1.0.0 — `spec/22` §4's correspondence table is unchanged in substance.

## Affected invariants

None. Every invariant in `spec/03-invariants.md` is stated over `Σ`
and rule behavior, neither of which this change touches.

## Dependency impact

None beyond the three cascade artifacts named above. No `Depends on`/
`Affects` line in any `rule.*`/`inv.*`/`type.*`/`state.*` entity
requires updating, since none of those entities' own text changed.

## Compatibility classification

Source-breaking, semantics-preserving. Any program text conforming to
`spec/22` 1.0.0 is no longer syntactically valid against 2.0.0 (every
declaration site's token order changed), but no program's *meaning*,
once re-spelled, differs from what it meant under 1.0.0. The
`auto`-vs-explicit-type choice at each local-declaration site is a
human style decision with no canonical answer, so no automated,
guaranteed-faithful 1.0.0→2.0.0 source rewrite is claimed by this
record.

## Migration implications

Every local variable declaration, function/extern parameter, struct
field, function/extern/function-type return type, `mod` declaration,
and struct-literal construction in existing CobaltC source text must
be re-spelled to the `spec/22` 2.0.0 grammar. A conforming
implementation targets exactly one grammar at a time — `spec/22` is,
per `spec/02-schema.md`, the sole concrete syntax — so no
implementation may accept both forms.

## Example changes

`spec/examples.md` re-spelled in full to `spec/22` 2.0.0 syntax; the
category, illustrated rule(s), and semantic content of every `ex.*`
entity are unchanged.

## Conformance changes

`spec/conformance.md` re-spelled in full to `spec/22` 2.0.0 syntax;
the expected outcome and Depends-on rule(s) of every `conf.*` entity
are unchanged.

## Future implementation implications

A future implementation targets `spec/22` 2.0.0 only. Disambiguation
(5) (declaration vs. expression statement) requires an implementation
to have collected the full struct/enum type-name set (from
`Σ.items`) before parsing any statement body — already required for
name resolution (`spec/17`) independent of this change, so this record
imposes no new implementation-feasibility burden.

## Prior-art status

Not applicable in the D-XXXX sense (§11's derivation-record fields);
this is a surface-syntax preference change directed by the human
owner (Master Instructions §3: escalation not required, since no
constitutional or invariant question is raised), not an independent
semantic derivation.

## Revisit conditions

Revisit if a future syntax change (e.g. resolving the still-open
`pub`/`use`/`match`/`unsafe`/`spawn`/`join` spellings, or the `fn`
placement relative to generics) needs to be layered on top of this
one; that would be a new Change record, not an edit to this one.
