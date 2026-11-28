# D-0303 — Attributes: `[[deprecated("…")]]` and `[[removed("…")]]`, a name's standing

Status: ACCEPTED (2026-11-01, the owner: "I think CobaltC needs some kind of support for attributes, so that I
can make APIs obsolete, or even remove them completely … deprecated APIs are errors by default, but a compile
time parameter such as --allow-deprecated would bypass the error. API removal/obsolete are always errors and
cannot be bypassed", then: "rewrite the proposal with your recommendations … The only consideration of my
preferences you need to take into account, is my dislike of being able to do the same thing two different
ways", and authorised its implementation as written, `private/D-0303-attributes-deprecated-removed-proposal.md`);
implemented by `CHG-0401`
Kind: Decision (`spec/02-schema.md`); a surface form, a rule of `spec/17`, two diagnostics, a stability amendment
Governed by: `CobaltC_Master_Instructions.md` §8 (priorities 3, 6, 9, 10, 12), §9, §10, §18, §21, §22
Depends on: D-0220 (stability policy, the deprecation window, amended here), D-0223 (the frozen `std` map
moves only through that window), D-0229 (`///`), D-0230 (`unittest`), D-0278 (both toolchains), D-0116, D-0118
(derived functions)
Affects: `spec/22` §3 and §5 (2.58.0); `spec/17` §1 `rule.module.retired` (2.18.0); `spec/registry/diagnostics.md`
(1.62.0: `diag.deprecated-use`, `diag.removed-use`); `spec/registry/features.md` (1.6.0: `feat.attributes`);
`spec/STABILITY.md` (1.1.0: §3, §3a, §5); `spec/examples.md` (3.21.0); `spec/conformance.md`; D-0220's Status line;
both toolchains' parsers, resolvers and checkers, `cobfmt`, `cobdoc.py`, the harnesses' `flags:` header

## Problem

A name could not be retired. Deleting an item made each use `diag.unbound-name` ("this name is not declared …
fix the spelling"), which is wrong twice: the name was declared, and the repair is not a spelling; and the
name was free again for another meaning. A `///` comment saying "deprecated" reaches the reference, not the
checker. The stability policy (D-0220) promised a deprecation window with nothing to mark a form deprecated,
and said a deprecated form "may not be rejected", which let a second spelling of one thing live on unread.

## Required facts and state

Each declared name has a *standing*, live, deprecated(m) or removed(m), fixed at its declaration and changed
by no run. No invariant of `spec/03` is involved and no state of `spec/04` changes: `Σ.items` gains the
standing of each item, and a use of a name is checked against it where the name is resolved.

## Candidate mechanisms

1. **`[[deprecated("m")]]` and `[[removed("m")]]` before a declaration**, the C23 and C++14 attribute form.
   Selected. A C programmer reads it without being told; `[[` is free where an item, a field or a variant
   begins; `deprecated` and `removed` are contextual words; the list is closed.
2. **Contextual words in the `export` slot**, `export deprecated("m") fn f`. Lighter in the grammar, but it
   reads as a property of the function's meaning, looks like a payload on a variant, and has nowhere to stand
   on a tombstone.
3. **`#[deprecated]`** (Rust). `#` is refused today with "CobaltC has no preprocessor".
4. **`@deprecated`** (Java, Swift). A new sigil with no C meaning.
5. **Keywords** `deprecated fn`, `removed fn`. Two reserved words for one small capability (§10).
6. **No mechanism; delete and document.** The diagnostic misleads, the name is reused, and the window has
   nothing to point at.

## Selected design

`spec/22` §3: `attr ::= '[[' 'deprecated' '(' str-literal ')' ']]'` before an item, a field, a bit field or a
variant; `tombstone ::= '[[' 'removed' '(' str-literal ')' ']]' vis kind name ';'` for a `fn` (also `T::f`),
`struct`, `bitstruct`, `enum`, `module`, `const` or `static`. The message is required and not empty; one
attribute per declaration; `removed` only on a tombstone; nothing on `import`, `extern "…";`, `unittest`, a
local, a parameter or a closure. Each misuse is `diag.syntax-error` with a message saying what is allowed.

`spec/17` `rule.module.retired`: a use of a deprecated name is `diag.deprecated-use`, with the message, unless
the build passes `--allow-deprecated`, which accepts the program with its old meaning and reports each use as a
warning once the program is accepted; a use of a removed name is `diag.removed-use`, which nothing bypasses; a
tombstone is an item, so its name cannot be declared again (`[Item-Duplicate]`). A module's or a type's
standing carries to what it declares, and a path through a retired module or type is a use of it.

**What does not count as a use**, decided while implementing because the proposal's "own body" was too narrow:
a deprecated type's own associated functions name the type in their signatures and bodies, so with "own body"
alone a type could not be deprecated without the flag. The rule, as Rust's lint has it: a use inside a
declaration that is itself retired (its own body; an associated function of a deprecated type; an item of a
deprecated module) does not count, nor one inside a function the front end derives, nor one inside the
`unittest` that immediately follows a retired item.

**The flag** is all-or-nothing: `--allow-deprecated` on `coby`, `cobc` and `cbc`, before the file, before or
after `--check`, and `COBALTC_ALLOW_DEPRECATED=1` in the environment as the tools' other switches have it.
A per-name allowance was not adopted: two spellings of one flag, and the warnings already name each name.

**The stability policy** (`spec/STABILITY.md` §3, amending D-0220): during a window, the old form is
`[[deprecated]]`, rejected unless `--allow-deprecated`; when the window closes it becomes a tombstone. §3a states
the lifecycle: live, deprecated, removed (not before the next published edition after the deprecating one in a
moving part, never in the same Update), deleted (optional). The error is the default because a deprecation that
does not stop a build is read by no one: the two spellings coexist until the removal breaks the build anyway,
with less notice than the error gives; and a deprecated spelling that still builds silently is a second way of
writing one thing.

## Rejected alternatives

- A warning by default (C, C++, D, Rust's lint): see the previous paragraph.
- A warning while a part is stable and an error while it moves: two behaviours for one word.
- A message-less `[[deprecated]]`, or a `since` edition: the message is the attribute's whole value at the use
  site, and the edition row records when.
- `obsolete` as the strong word (C#): in English it is the soft state.
- An open set of attributes (`nodiscard`, `inline`, `test`, …): each would be a feature with its own semantics;
  CobaltC has `_ = e;`, the cost model and `unittest` for those needs. `spec/22` §5 records the absence.

## Costs (§18)

Syntax: one bracket form in three positions, two contextual words, one item form. Conceptual: a name's standing
in three states. Specification: one rule block, two diagnostics, one grammar production pair, an amended policy.
Diagnostics: two ids. Runtime: none. Interaction: name resolution (one check, where names are resolved), field and
pattern typing (one check each), aliases (resolved through), `unittest` adjacency (one exemption). Compatibility:
additive; no accepted program has `[[` where an item, a field or a variant begins. Prior art: C23/C++14
`[[deprecated]]` (warning), clang's `unavailable` (error), Rust `#[deprecated]` (warning lint), C#
`[Obsolete(m, true)]` (error), D `deprecated("m")` with `-de` (error), Swift `@available(unavailable)`. CobaltC
takes C's spelling and D's `-de` as the default.

## Compatibility impact

Additive. A program that marks nothing is unaffected; `std`'s first retirement is D-0305's.

## Revisit conditions

- A module ecosystem: once a program depends on modules it does not control, a per-name allowance, or a warning
  default for another program's names, is reconsidered; a global flag would then hide a program's own
  deprecated uses behind a library's.
- The tombstone, if a cheaper way to keep a name and its message appears.

## Implementation notes (CHG-0401)

- The standing is a field of each declaration in both toolchains' trees (`Retired`), set by the parser.
  Name resolution keeps a table of the retired names and the functions in which no use counts, checks every
  path it resolves, an `import`'s target, and a path that names nothing through a tombstone, and drops the
  tombstones once every name is resolved. The type checker checks a field (access, literal, destructuring,
  struct pattern) and a variant named in a pattern. The warnings are kept and printed once the program is
  accepted, sorted and each once.
- `coby --dump-ast` (and the `cbparse` dumps compared with it) show a standing after a declaration's other
  fields only when it is not live, so every earlier dump is unchanged.
- The proposal's `= note: declared at …` line is not printed: the message names the item, and a note line would
  be a third piece of the shared diagnostic form (D-0228) for both toolchains to agree on.
