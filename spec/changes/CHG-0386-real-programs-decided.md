# CHG-0386 — The real programs decided: wrapped handles, `?`, lent slices, and facts that outlive their reference

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-29)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0291, D-0281, D-0213, D-0247, D-0283, D-0288, D-0289, D-0290
Affects: `spec/14-control-flow.md` 1.40.0 (§6 the spawn rows, Handing over, inherited facts, Slices and copies,
Liveness, the call-site rule, decided programs); `spec/conformance.md` 3.282.0; `impl/src/typecheck.rs`,
`impl/cobc/src/lower/program.rs`

## What changed

In the static pass (`typecheck.rs`):

1. **Handing over** (`hand_over_referent`, in `exit_scope`). When a reference or slice binding `r` ends, a fact
   `deriv(h, *r.π, m)` another live binding holds becomes, for each fact `deriv(r, x.π', m')` `r` held,
   `deriv(h, x.π'.π, m ⊓ m')`, `?` unless both were `T`. A handle kept in a `Vec` past the block that bound `r`
   holds a fact about `r`'s referent, not about nothing.
2. **The call-site rule's reach is bounded by the parameter's type.** Through `ref<Items, exclusive>` a callee
   reaches only what an `Items` can keep, read from the referent type as `kept_from` reads an argument's gift;
   through a type that keeps no reference, nothing beyond the referent.
3. **Wrapped handles** (`tail_spawns`, `spawn_let_depth`, `tail_spawn_sites`). A `let` whose value is
   `Some(spawn(…))`, or an `if`, `match` or block that gives one on some arms, holds what each such spawn was
   lent: `T` when there is one spawn and every arm giving a value gives it, `?` otherwise; the facts are taken
   where the spawn is, and a spawn whose facts name a binding the `let`'s scope does not see is left to the
   run, as a lending spawn no binding holds. A `match (h)` arm that takes the handle out holds them.
4. **`e?`** (`inherited_facts_in`, `contained_ref_facts`): a binding made by `call(…)?` holds what the call's
   `Option` or `Result` holds.
5. **A consuming `foreach`'s drain holds its holder** (`Vec::drain`'s hidden holder: D-0290 rule 1, as `auto d =
   h` would).
6. **A by-value parameter that keeps references in data holds `*v`**, so a result built from it reaches only
   what the parameters do (`Drain { .v = v }`).
7. **Slices lent to a thread** (`spawn_lent_refs`): a slice binding passed whole to `spawn` is lent as a
   reference binding is, `deriv(h, *c, m)`, and by 1 hands over when `c` ends. The lend's mode is the
   binding's own, not the parameter's: `[Thread-Lend]` lends before the parameters are bound. (An exclusive
   reference lent to a shared parameter was lent shared since Update 144, and a read through it in the spawner
   was decided clash-free.)
8. **Liveness while a referent is held** (`tier_access` asks `holder_live_after`, of a holder with a fact at the
   accessed root): a binding `s` whose referent `*s` a live thread, closure or copy holds keeps its own facts
   live for the frame's accesses, as the spec's Liveness rule says; before, `s`'s facts ended at its last
   mention, and a write to the vector `s` borrowed from was decided clash-free while a closure still wrote
   through `s`. A closure, a `fn` value holding one, holds what it captures to its scope's end.
9. **Slice elements, reborrows and copies** (`holder_deref_access`'s slice branch, `tier_slice_reborrow`,
   `tier_referent_reach`, `arg_ref_param`): an access to `s[i]` while another live holder holds `*s` in a
   clashing mode, or holds a reborrow of `s` whose range overlaps `i`; a reborrow `&s[a .. b]` or `&*r` formed
   while another holder holds the referent; and a copy of a reference or slice binding read while another
   holder holds its referent (`auto w = s`, `Some(s)`, a field) are the run's. A copy passed to a reference or
   slice parameter is the call-site rule's, as before; a hidden binding (`$c`, a `foreach`'s) is read by the
   desugaring, not copied.
10. **`slice_parts`' body is trusted** (`tier_report::trusted`, `in_trusted`): `[Slice-Parts]` makes its parts
    disjoint, so the point checks its body's pushes would make (each reborrow against the parts already in the
    result) cannot fault, and neither tool makes them; `left()` ignores the body, so nothing in it keeps a
    program's paths. The result reaches what the slice does, its signature.

In `cobc` (`lower/program.rs`): `pathless_blocker`'s type test (`sigs`, `fn_refs`, `keeps_refs_in_data_by`,
`modeless`) takes a `fn` value in data to keep references only when a capturing closure of its signature exists in
the program; `PriorityQueue<T>`'s comparator no longer makes every program using it tier D. The D-0283 point-check
site returns early in a trusted body, so `slice_parts` adds no "a call is checked when it is made" blocker.

## What changed for programs

`cbc`, `cbparse`, `parallel_chunks`, `dijkstra`, `dj2`, `eventsim` and `churn` are decided. Over the 13,397
programs of the soundness sweep (the corpus `stress/d0281/why`, both conformance suites, the book's and the guide's
examples): 10,648 decided (Update 145: 10,635), 996 keep paths, 1,753 refuted before the run. Fourteen programs
decided that kept paths (the four `PriorityQueue` programs among them), one decided before kept now (`hand-x03`,
whose run faults: a hole), two that kept paths refuted before the run.

Five holes closed, each found by the soundness sweep (three decided programs faulted `diag.aliasing-conflict`
under `coby` where the binary ran with no check) or by variants written around the finds: the handed-over fact
(D-0289, Update 144), the lend's mode (Update 144), slice elements against a lent or captured slice (D-0288,
Update 143), copies of a lent reference or slice, and a holder's liveness while its referent is held. Where the
conflict is sure, both tools refuse the program before the run; where only the values decide, the run checks it
as before, and the binary faults as `coby` does. Every decided program whose `coby` run faults
(`diag.stale-binding`, `diag.overwrite-of-live-resource`) faults the same way compiled.

Compile cost: `cobc` on the self-hosted compiler (`impl/selfhost/cbc/main.cb`, no C compiled) takes 56 s against
Update 145's 35 s: the static pass 24 s against 20 s, and the lowering 32 s against 15 s because `cbc` is decided
now, and a decided program is lowered a second time knowing it (D-0285: 15 s, then 17 s; `COBALTC_DECIDED=0` gives
39.5 s in all). Peak memory 567 MB against 367 MB (467 MB with one lowering).

## Conformance

- `conf.ref-lent-in-loop-handle-kept-written`, `conf.wrapped-handle-maybe-lent`, `conf.slice-lent-copy-written`,
  `conf.lent-parts-written-after-loop`, `conf.slice-reborrow-parent-element-written` (dynamic);
  `conf.exclusive-ref-lent-to-shared-param`, `conf.wrapped-handle-lent`, `conf.closure-captured-slice-source-read`,
  `conf.propagated-ref-holds-borrow` (static); `conf.slice-parts-lent-to-threads` (ok, a cost case: decided).
- `conf.vec-predicate-value-keeps-element`: its `fn` value, whose closure captures `kept`, held `kept` to its
  last use in the static pass and to its scope's end at run time, so the case faulted at the first read of
  `kept` where the static pass now refutes it. The `fn` value lives in its own block, and the case tests what
  it was written for: the kept element reference is stale after the growing push (`diag.stale-binding`,
  dynamic; prints `true` and `2` first).
