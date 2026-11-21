# D-0290 — Frame-held aggregates: a local that keeps references in data holds what it keeps

Status: ACCEPTED (2026-10-28, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 phase 1 close (§23 item 1).
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0011, D-0088, D-0111, D-0236, D-0281, D-0282, D-0284, D-0288, D-0289
Affects: `spec/14-control-flow.md` 1.39.0 (§6 holders, inherited facts, the call-site rule, decided programs);
`impl/src/typecheck.rs`, `impl/cobc/src/lower/program.rs`, `impl/model/src/statics.rs`; `CHG-0385`

## Problem

After D-0289 the largest group of programs that kept their record of references kept it for one reason: a type
they used keeps references in data. `cobc` tested it over the whole program (`pathless_blocker`'s type test), and
the static pass gave a binding of such a type no facts ("tier D by its type", D-0281 §7). Over the corpus of
`stress/d0281/why/` (`new6.why`) that was `Once::call`'s run guard (`OnceRun`, a `ref<Once>` in a struct: every
program using `std::sync::Once`), the `BitReader` of `inflate_from` (every program that inflates), every
`Vec<StringView>` that `String::split` or `lines` returns, `Vec<ref<T>>` locals, and every view read out of one.
Of the 95 hand-written programs that kept paths, 41 did so for this reason alone.

Yet almost all of these values never leave the frame that makes them. A `Vec<StringView>` of a line's fields, a
struct of views built and read in one function: the frame sees every reference stored into them, as it sees a
reference binding's.

## Options

- **A. Keep them tier D** (as D-0281 §7). Every program that splits a string pays for paths everywhere.
- **B. `tracked` aggregates** (D-0281 O3): a struct with a reference field is spelled `tracked`, and stays tier D.
  Visible, but it marks a type for what only some of its values do; the D-0281 phase 4 study found it needed on
  0 of 96 showcase aggregates, 2 of 272 in `cbc` and 4 of 145 in `std`.
- **C. Frame-held aggregates.** A local of such a type is a holder, as a reference binding is: it holds the facts
  of what is stored into it, and what is read out of it holds them. What the frame cannot see — the aggregate
  moved out, stored where a reference parameter reaches, made of what is not known — is the run's, function by
  function, as any other conflict the static pass leaves.

## Decision

C. A local binding (not a parameter, not a capture) whose type keeps references in data, its elements' among
them, and holds no `fn` value, guard, lock or thread handle, is a **holder** (`spec/14` §6):

1. **What it holds.** Made from a literal, the facts of its parts; from a call, the facts of the call's first
   reference argument, each `?` (the aggregate may hold none of it: `String::split_whitespace` of blank text is
   empty), or of every reference argument, `?`, when there are several; copied from another holder, that one's.
   `Vec::push` and `Vec::insert` add what the element holds; a field or element assigned adds what the value holds
   and leaves the old facts `?`; an assignment of the whole replaces them; any other call that takes it
   exclusively may store what its other arguments reach and remove what it held, so it adds those `?` and leaves
   its own `?`. A holder that owns no resource ends at its last use (`[Ref-Binding-Last-Use]`); a `Vec` holds its
   facts to its scope's end, as the run holds the references it keeps until they are removed or the `Vec` is
   destroyed.
2. **What is read out of it** — a field, an element, `Vec::pop`, `Vec::get`, a `foreach` over it, a view
   copied out — holds its facts, each `?` (one element may reach any of them, or nothing). A call given `&a`
   reaches what `a` holds, in no stronger mode than the parameter's, besides `a` itself (the call-site rule).
3. **Through a reference parameter `q`**, a reference read out of `*q` reaches `**q`: a root of its own, disjoint
   from the frame's places and from what the other parameters reach, as `*q` is; an exclusive one is `?`. The
   caller's call-site rule makes that so, by rule 2.
4. **A store through a reference** (`p.f = r`, `*(v.ptr + i) = x`, `Vec::push(p, r)`) is checked against what the
   caller can see: allowed when what is stored reaches only parameters' referents (`*q`) or what they keep
   (`**q`), or is static data, since every call adds what its other arguments reach (rule 1) to a holder it gives
   exclusively; otherwise the run's. A call that gives an exclusive reference to data that keeps references and is
   no holder of the frame, beside an argument whose references that data can keep, is the run's when what that
   argument gives is not what the caller sees. Which references a type can keep is read from the types: a
   `BitReader` keeps byte slices, so `read_dynamic(&mut r, &mut lit, &mut dist)` stores neither `&mut lit` nor
   `&mut dist` into `r`.
5. **Leaving the frame.** A holder holding facts moved anywhere its facts do not follow (an argument, a field, a
   closure) is the run's; a result that keeps references in data must reach only what the parameters reach
   (`*q`), which is what the caller takes it to hold (D-0011, rule 1), else the run's.

`cobc`'s type test keeps only the types whose values the static pass does not follow (a `fn` value's, a guard's,
a lock's, a handle's, in data). A program none of whose functions leaves a conflict to the run is decided, and
runs pathless (D-0284).

**`tracked`.** Neither aggregates nor parameters gain the keyword D-0281 O3 and §15 rule 1 proposed. An aggregate
is tier D by where a value of it is, not by its type, and the static pass finds that per function; an escape
through a parameter is sound without a marker because the call adds what it may store (rule 4). What a program
leaves to the run is named by `cobc -v` (`kept paths: …`), the function and line, as for every other reason.
O3 is not adopted.

**Phase 1's other items** (D-0281 §23 item 1). The cost contract (D-0281 §14) moves into `spec/14` §6, its tier-D
lines provisional; the conformance cases headed `decided: yes` are the cost cases. Per-function lowering moves to
phase 3, whose owner-side marks it needs (D-0281 §24).

## Consequences

- Corpus (`stress/d0281/why/new8.why`, 11,022 programs): 132 programs that kept paths are decided; none that was
  decided keeps them; 19 are refuted before the run that the run faulted, each `diag.aliasing-conflict` (the model
  agreeing on the 17 it expresses) (a write or read of what a `Hold` or a `Vec<ref<…>>` of the frame still holds).
- Of the 95 hand-written programs that kept paths, 36 are decided: the `Once` users, the showcases that inflate
  (`static_site`, `tiny_api`, `dns_over_http`, `token_gate`), `anagrams`, `table`, `banner`, `logscan`,
  `string_processing`, `password_store`, `release_signing`, selfhost `cobfmt`.
- The release suites and a soundness sweep (each program of the corpus the static pass decides run under `coby`)
  found an access through what an aggregate keeps, a holder reached by a live binding, a move a fact `?` names
  (since D-0284), a thread lent an exclusive reference through a shared one (since D-0289) and a generic call's
  result read with its type parameters unbound decided where the run faults; each is the run's now (`CHG-0385` items 5–6), and over `stress/d0281/why/new9.why` (10,930
  programs) no program's decision changes but one that kept paths for another reason. Eighteen
  conformance cases and two end-to-end examples store their reference by a call so they stay the run's; four rows
  are added.
- One conformance row changes to *static*, `conf.move-capture-beside-exclusive-ref`; five new rows
  (`CHG-0385`). The headers of the 17 model-enumerated programs refuted earlier follow the model.
