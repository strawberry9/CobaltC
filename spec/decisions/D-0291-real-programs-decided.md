# D-0291 — The real programs decided: wrapped handles, `?`, lent slices, and facts that outlive their reference

Status: ACCEPTED (2026-10-29, under D-0281's delegation: the owner handed the cost model to the assistant "to
manage autonomously, with authorisation given to make all necessary decisions", and asked for all of D-0281 to be
completed without further prompting). D-0281 phase 2/3 boundary.
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9, §12
Depends on: D-0071, D-0213, D-0236, D-0247, D-0281, D-0283, D-0284, D-0288, D-0289, D-0290
Affects: `spec/14-control-flow.md` 1.40.0 (§6 the spawn rows, Handing over, inherited facts, Slices and copies,
Liveness, the call-site rule, decided programs); `spec/conformance.md` 3.282.0; `impl/src/typecheck.rs`,
`impl/cobc/src/lower/program.rs`; `CHG-0386`

## Problem

After D-0290, 97% of the corpus is decided, but the programs that matter most are not: the self-hosted compiler
`cbc` (one function: `direct::fn_decl`, then three more behind it), the self-hosted parser `cbparse`
(`typecheck::bitfield_of`), the showcase `parallel_chunks`, and four `PriorityQueue` programs (`dijkstra`, `dj2`,
`eventsim`, `churn`). One undecided function makes the whole program keep paths: `dijkstra` runs 13.4 s
tracked and 1.86 s decided. Each is kept by a gap in what the static pass follows, not by a conflict.

And gaps ran the other way: programs `cobc` decided and ran with no check where `coby` faults
`diag.aliasing-conflict`. The soundness sweep (every program `cobc` decides, of the corpus and both conformance
suites, run under `coby`) and the variants written around each find showed five:

- A reference binding lent to a thread, or captured by a `move` closure a thread runs, gave the handle the fact
  `*r`; when `r`'s block ended, `r`'s own facts went with it, and a handle kept in a `Vec` of handles past the
  block held a fact about nothing (D-0289, Update 144).
- An exclusive reference lent to a thread whose parameter is shared was taken as lent shared. `[Thread-Lend]`
  lends before the parameter is bound: the lent path is exclusive, and the spawner's read through `r` faults
  (D-0213's fact, Update 144).
- An element of a slice binding (`s[i]`) is no place the static pass names, so no fact about `*s` (a thread
  lent `s`, a closure captured it) or about a reborrow of `s` (`&mut s[1 .. 2]`) was checked against it
  (D-0288 closures over slices, Update 143; `conf.slice-written-while-lent-reborrow` under `cobc`).
- A copy of a reference or slice binding (`auto w = s`, `Some(s)`, a struct field) reached `*s` under no fact.
- What a binding `s` holds was taken to end at `s`'s last mention even when a thread or closure still held
  `*s`, so a write to the vector `s` borrowed from was decided clash-free.

## Decision

1. **Forgetting hands over (repair).** When a reference or slice binding `r` ends, a fact `deriv(h, *r.π, m)`
   another binding `h` holds (a handle, a closure, a copy) becomes `deriv(h, x.π'.π, m ⊓ m')` for each fact
   `deriv(r, x.π', m')` `r` held, `?` unless both were `T`. The thread or closure still holds the reference `r`
   held; the fact now says where.
2. **What a callee reaches through an argument is bounded by its parameter's type.** The call-site rule (D-0290
   rule 4) took a parameter `q` to reach whatever the references stored anywhere in the argument's root reach
   (`**g`). Through `ref<Items, exclusive>` a callee reaches only references an `Items` can hold: none, or shared
   ones only, when the type says so. (`cbc`: `load_body(&mut g.prog.table, &g.prog.front, name)`.)
3. **A handle the value wraps is held by the binding.** `Option<handle<T>> h = match (e) { … : Some(spawn(…)),
   … : None }` (or `if`, or a block's tail): the binding holds what each such spawn was lent, `?` unless it is
   the only spawn and every arm that gives a value gives it; the facts are taken where the spawn is, and only
   when they name no binding of a scope the `let` does not see. A `match (h)` arm that takes the handle out
   holds them, as a `Vec` of handles' does (D-0289).
4. **`e?` holds what `e` holds.** A binding made by `call(…)?` has the facts the call's `Option`/`Result` has.
5. **A consuming `foreach`'s drain holds its holder.** `foreach (x in f(…))` over a `Vec` of views drains the
   hidden holder into a drain that holds what it held (D-0290 rule 1, as `auto d = h`).
6. **A by-value parameter that keeps references in data holds what its caller put there** (`*v`), so a result
   built from it (`Vec::drain`'s `Drain { .v = v }`) reaches only what the parameters do.
7. **A slice binding lent whole to a thread is lent as a reference binding is** (D-0213): `deriv(h, *c, m)`, and
   by 1, what `c` reaches when `c` ends. D-0247's "a slice binding passed whole … forms no fact" no longer holds
   for `spawn`.
8. **`cobc`: a `fn` value in data keeps references only when a closure of its signature does.** A program with
   a capturing closure of another signature no longer makes every type holding a `fn` value tier D
   (`PriorityQueue`'s `less`).
9. **`slice_parts`' body is decided by its rule.** It pushes reborrows of its slice into its result, each
   checked at the call against the parts already there; `[Slice-Parts]` makes them disjoint, so no check of
   the body can fault, and the result reaches what the slice does (its signature). Neither tool makes those
   point checks, and nothing in the body keeps a program's paths.
10. **`[Thread-Lend]`'s mode (repair).** An exclusive reference or slice binding lent to a thread is lent
    exclusively whatever mode the parameter takes: `deriv(h, *r, exclusive)`. `spec/19` said so already; the
    static pass did not.
11. **What a holder holds lives while its referent is held (repair).** A binding `s` whose referent another
    live holder holds (`*s`: a thread, a closure, a copy) holds its own facts as long, for the frame's
    accesses as for `[Write]`/`[Read]`. A closure holds what it captures while it exists, to its scope's end.
12. **A slice binding's elements and copies (repair).** An access to `s[i]`, a reborrow `&s[a .. b]`, and a
    copy of `s` (or of a reference binding `r`) made while another live holder holds `*s` in a clashing mode,
    or a reborrow of `s` overlapping `i`, is the run's (kept), not decided. A copy passed to a reference or
    slice parameter is the call-site rule's, as before. A hidden binding (a `foreach`'s) is read by the
    desugaring, not copied: what is borrowed from it is checked where it is borrowed.

## Per-function lowering

Not adopted as a separate lowering (D-0281 §24). A tracked caller may pass a reference that is stale (read out of
data after its element was released), and passing it is no fault; a pathless callee that drops the token would
read freed memory where the tracked callee faults, and a check on entry would fault where it does not. So a
pathless function is exact only when no tracked function calls it with a token, which in a connected program is
all of them or none. Phase 3's light tokens do what per-function lowering was for: a decided function's borrows
from light bases record nothing, while a holder's token still checks validity at access
(`stress/d0281/phase3/PERFN.md`).

## Consequences

- `cbc`, `cbparse`, `parallel_chunks`, `dijkstra`, `dj2`, `eventsim` and `churn` are decided. Over the 13,397
  programs of the soundness sweep (the corpus `stress/d0281/why`, both conformance suites, the book's and the
  guide's examples): 10,648 decided (Update 145: 10,635), 996 keep paths, 1,753 refuted before the run. Fourteen
  programs decided that kept paths: the four `PriorityQueue` programs, `conf.foreach-consumes-vec-of-references`
  (rule 5), `conf.propagate-slice-index` and `conf.hashmap-get-mut-match` (rule 4), the four `postgres` cases
  (`?` through the protocol code), the model sample `hand-l01`, two fuzz programs. One program decided before
  keeps paths now, `hand-x03` (a slice copied into a spawned `move` closure, written by the spawner meanwhile),
  whose `coby` run faults: it was a hole. Two programs that kept paths are refuted before the run (a `fn` value
  holding a closure over `kept`, read while the `fn` value exists; rule 11). Of this Update's own first sweep,
  three decided programs whose run faulted are kept now (`conf.slice-written-while-lent-reborrow`, `hand-x03`,
  `hand-x06`), and every decided program whose run faults faults the same way compiled (`diag.stale-binding`,
  `diag.overwrite-of-live-resource`: value-state faults a decided program keeps).
- Programs whose conflict through a wrapped handle, a lent exclusive reference, or a captured slice is sure are
  rejected before the run (`diag.aliasing-conflict`, as for a handle binding since D-0289) where they faulted at
  run time; where only the values decide, the run checks them as before, and the binary faults as `coby` does.
- Compile cost: `cobc` on the self-hosted compiler (`impl/selfhost/cbc/main.cb`, no C compiled) takes 56 s against
  Update 145's 35 s: the static pass 24 s against 20 s, and the lowering 32 s against 15 s because `cbc` is
  decided now, and a decided program is lowered a second time knowing it (D-0285: 15 s, then 17 s;
  `COBALTC_DECIDED=0` gives 39.5 s in all). Peak memory 567 MB against 367 MB (467 MB with one lowering). The
  second lowering's cost on a large decided program is phase 3's to look at, with the objectless track.
