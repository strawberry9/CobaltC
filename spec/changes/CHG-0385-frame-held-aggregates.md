# CHG-0385 — Frame-held aggregates

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-28)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0290, D-0281, D-0282, D-0284, D-0288, D-0289, D-0244
Affects: `spec/14-control-flow.md` 1.39.0 (§6 holders, inherited facts, frame-held aggregates, the call-site rule,
decided programs); `spec/conformance.md` 3.281.0; `impl/src/typecheck.rs`, `impl/cobc/src/lower/program.rs`,
`impl/model/src/statics.rs`; `spec/examples.md` 3.20.0

## What changed

In the static pass (`typecheck.rs`):

1. **Holders.** A local (not a parameter or a capture) whose type keeps references in data, elements included
   (`stores_refs`), and holds no `fn` value, guard, lock or handle (`holder_free`), is a holder of the frame
   (`holders`, `is_holder`), registered where it is bound (`tier_inherit`, the `let` and `match` binder rows).
   It is no longer reported "keeps references in data (tier D)", and its facts are checked as any holder's
   (`tier_access`, the call-site rule no longer skip it).
2. **What it holds.** A literal holds the facts of its parts (`union_facts`); a call's result that keeps
   references in data holds its first reference argument's facts as `?`, in the parameter's mode, and every
   reference argument's as `?` when there are several; a generic call given no reference holds what its values
   hold (`Vec::new()`: nothing). `Vec::push`/`insert` add what the element gives (`holder_call_facts`,
   `holder_call`); any other call given `&mut a` adds what its other arguments give, `?`, and leaves `a`'s own
   `?`. What an argument gives is read from the types (`ref_leaves`, `kept_from`): the reference it is when the
   holder's type can keep one of that type, what the references kept where it reaches reach when it can keep
   those. A field or element assigned adds and leaves the old facts `?`; a whole assignment replaces them
   (`holder_store`). A holder that owns no resource is copied, not moved, and keeps its facts.
3. **Reads.** A field, an element, a `Vec::pop`/`get` result or a `foreach` binder over a holder holds its facts,
   `?` (`stored_facts`); through a reference parameter `q`, `**q` (`deep_root`); through a reference of the
   frame, what the roots it reaches keep (`stored_through_ref`, `reach_of_root`). A range of a view's bytes
   (`&v.bytes[i..j]`) holds what `v` holds. The call-site rule adds, to an argument's reach and effect, what the
   references kept where it reaches reach, the holder counting as one the argument is formed through.
4. **What is the run's.** A holder holding facts moved where they do not follow (`holder_leaves`); a result that
   keeps references in data holding anything but what the parameters reach (`result_reach_ok`); a store through
   a reference, or a call given an exclusive reference to data that keeps references and is no holder of the
   frame, unless what is stored is static data or reaches only parameters' referents or what they keep
   (`caller_sees`, `store_target`, which also follows a raw pointer moved along: `*(v.ptr + i)`); a holder that
   may hold a view of a temporary. A holder given a reference binding `q` of the frame holds what `q` holds, not
   `*q`, so it keeps it after `q` ends (`through_refs`; found by the model and `coby` statics sweep).
5. **Access through what an aggregate keeps** (`holder_deref_access`): a read or write through a reference read
   out of a holder or out of a reference's referent (`*s.inner`, `*v[i]`) is the run's when a live holder other
   than the aggregate holds, in a mode that does not permit the access, a fact meeting what the aggregate keeps,
   or one rooted at the aggregate (`auto r = &*s.inner`; `give(&s)` returning `s.inner`). A holder is live while
   a live binding holds a fact rooted at it (`auto hr = &h`; `holder_live_from`).
6. **Repairs of older rules found on the way** (a soundness sweep: every program of the corpus the static pass
   decides run under `coby`): a move of a place that a fact `?` names, `solitary` neither proven nor refuted,
   is the run's (`flow_solitary`; decided since D-0284); a thread lent, as an argument or a closure's capture,
   a shared reference through which an exclusive one is reached is the run's (`excl_under_shared`; decided
   since D-0289; `[Thread-Lend]` lends only the exclusive paths in an argument's own value). A generic call's
   result type is the declared one with its type parameters bound from the arguments (`value_shape`): read as
   `ref<T, exclusive>`, `Vec::index_exclusive(&mut v, 0)` pushed onto a `Vec<ref<i32, exclusive>>` seemed a
   reference the `Vec` could not keep, and the push added nothing to what it held.
7. **`r = e` where `r` held nothing** (`tier_inherit`): a fact of `e` rooted at `*r` is static data, or, for a
   parameter, what its caller gave (`*r`): `rest = StringView::sub(rest, …)` of a static view is static.

In `cobc` (`lower/program.rs`): `pathless_blocker`'s type test keeps only the types `frame_holdable` rejects (a
`fn` value's, a guard's, a lock's, a handle's in data). At a call's point check (`lower/user_calls.rs`,
`emit_point_checks`), a struct argument's kept references are each checked as a read, as `coby` checks them
(`kept_ref_tokens`).

In the model (`cobmodel`, `statics.rs`): a `Hold` made from a literal holds its field's facts, a copy of one what
it holds, and one assigned whole what the new value holds (`hold_facts`); `Vec::push` onto a `Vec<ref<…>>` of the
frame adds the element's facts, any other exclusive operation on it leaves them `?`; a `Vec<ref<…>>` is live to its
scope's end.

## What changed for programs

Over `stress/d0281/why` (11,022 programs, `new8.why`): 132 programs that kept their record of references are
decided, none that was decided keeps it, and 19 that the run faulted with `diag.aliasing-conflict` are refuted
before it (an access to what a `Hold` or a `Vec<ref<…>>` of the frame still holds); on the 17 of them the model
expresses, the model agrees. Among the hand-written ones: every user of `std::sync::Once`, the showcases that inflate
(`static_site`, `tiny_api`, `dns_over_http`, `token_gate`), `anagrams`, `table`, `banner`, `logscan`,
`string_processing`, `password_store`, `release_signing`, and selfhost `cobfmt`.

## Conformance

- `conf.vec-of-refs-holds-pushed-ref` (static), `conf.struct-of-ref-copy-keeps-borrow` (static),
  `conf.popped-vec-of-refs-frees-referent`, `conf.split-parts-hold-text` (dynamic), `conf.split-parts-read`.
- `conf.move-capture-beside-exclusive-ref` is static: `w`, a `Vec` of the frame, holds what was pushed onto it.
- `conf.write-through-kept-ref-beside-reborrow`, `conf.move-while-maybe-borrowed`,
  `conf.thread-lent-exclusive-through-shared`, `conf.thread-closure-lent-exclusive-through-shared` (dynamic).
- Eighteen rows (one `ok`, the rest dynamic) whose programs held their reference in a struct or `Vec` literal or push store it by a
  call (`keep_in`, `hold`), which the frame holds as `?`, so they stay the run's; stored directly, the same
  programs are refuted before the run (as `conf.vec-of-refs-holds-pushed-ref`). `ex.e2e-vec-realloc-stale-ref`
  and `ex.e2e-vec-pop-push-reuse-stale-ref` store theirs by `hold` (`spec/examples.md` 3.20.0).
- `conf.fault-unwind-callee-tail-conflict-dynamic` faults when the call is made (the struct argument's kept
  reference is checked there).
