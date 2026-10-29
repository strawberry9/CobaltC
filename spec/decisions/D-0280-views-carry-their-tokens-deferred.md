# D-0280 — Views and slices carry their tokens: frame-held reference data (deferred)

Status: DEFERRED (2026-10-25, the owner: "make all necessary decisions to close off this work once and for all").
Option 2 was accepted (owner, 2026-10-25: "accept D-0280 option 2, prototype phase 1") and phase 1 prototyped in
`cobc`: 2.3–4.1% on `cbc`, under the 5% abandonment threshold ("Prototype results"), so the lowering is reverted;
the patch is `stress/p4/d0280/archive/phase1-prototype.patch`. The runtime bug the prototype found shipped as
CHG-0372 (Update 135). Revisit condition: after the scope costs (P4 plan §5 item 6: lazy frames, `cb_stmt_pop`/
`cb_frame_pop`) are cut, when the same absolute saving is a larger share of `cbc` and the frame list the
prototype builds on has settled; phase 2's `foreach`/`match` binders first, fields not at all (no gain measured).
Drafted
PROPOSED (2026-10-25, drafted at the owner's request after Update 134: "draft the StringView
representation D-record"; P4 plan §5 item 1). Nothing is implemented. `D-0280` follows the drafts D-0278 and
D-0279; its change record takes the next free number when accepted (`CHG-0372`).
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0047 (slices), D-0053 (`StringView`), D-0070 (range borrows), D-0107 (pending paths), D-0111
(a reference binding's last use), D-0200 (compile-time discharge: objectless locals, frame-held reference
locals), D-0274 (the bootstrap form), D-0276 (`cbc` replaces `cobc`)
Affects (option 2, recommended): `impl/cobc/src/lower/` (`frames.rs`, `layout.rs`, `user_calls.rs`, the native
bodies that make views: `native_text.rs`, `native_misc.rs`), `impl/selfhost/back/` (the same in `cbc`),
`impl/cbrt/src/lib.rs` and `include/cbrt.h` (at most a few entry points; the contract table,
`spec/IMPLEMENTATION-NOTES.md` §6), `impl/STATUS.md`; not `spec/`'s rules

## Problem

A `StringView` is `struct { slice<u8, shared> bytes; }`, and a slice's C value is
`struct { void *src; T *data; uint64_t len; }` (`spec/16` §3a: three words, impl-defined). The `src` word
is the borrow of the viewed `String`. In memory, a reference is a bare address; its **token** (the path
the runtime checks accesses against, and counts as held) lives in the runtime's slot table, keyed by the
address of the word that holds it (`cb_store_ref`, `cb_load_ref`). So every place a view occupies has a
slot, and a value holding a slot has a runtime object to be its holder. Copying a view into a local is,
in the C `cobc` and `cbc` write today:

    l_w = l_v;
    cb_copy_datum(&l_w, &l_v, TY);     /* forget l_w's slots, find l_v's, store each into l_w's */
    uint64_t o = cb_new(&l_w, TY, …);  /* an object to hold them */
    uint64_t root = cb_bind(o);        /* the binding's root path; ended with the frame */

A bare reference local does not pay this. Since D-0200 a reference local that is never assigned, borrowed
or captured has no binding object: its C value is `cb_ref { p, tok }`, the token beside the address, and
its copy is `cb_frame_hold(tok)`: the frame holds the path as the binding object would, until the frame
ends or the binding's last use (D-0111). Views and slices never got this treatment, because the token of
a struct's reference field has nowhere to live but the slot table.

### What it costs

Instructions per loop iteration, the loop's own included (`cobc -O 2`, GCC 11, user space, 10⁶ iterations;
the empty loop is 5):

| operation | today |
|---|---|
| a reference local copied (`auto q = r; … *q`) | 881 |
| a view passed to a function that only reads it (D-0191's pure view parameter) | 10 |
| a view copied into a local (`StringView w = v;`) | 2,689 |
| a slice copied into a local (`slice<u8, shared> t = s;`) | 2,728 |
| a view put in a struct literal (`Tok { .text = v, .at = i }`) | 4,213 |
| a sub-view made (`&v[0 .. 1]`) | 7,140 |
| a sub-view made by a function and returned | 14,277 |

The same loop hand-rewritten into the form option 2 writes (the view's token in a C variable, the frame
holding it, no object, no slot): **668** instructions instead of 2,689. The rest is the block's own frame
and statement scope, which the reference copy pays too.

How much of a run is views (uprobes on the runtime's entry points, filtered by the type's id):

| program | objects made | of which views | reference-data copies, sends, receives | of which views |
|---|---|---|---|---|
| `cbc --emit-c showcase/tier3/tinyos.cb` (33.6 G instructions) | 7.03 M | 1.99 M (28%) | 3.74 M | 3.05 M (82%) |
| `stress/perf_oct9/csvstat` (0.94 G instructions) | 91,351 | 91,122 (99.7%) | 91,132 | 91,132 (100%) |

In `cbc`, the reference-data entry points alone (`cb_copy_datum`, `cb_send_datum`, `cb_recv_datum`,
`cb_store_ref`, `cb_load_ref`) take 8.3% of the run (inclusive); with the views' share of making,
binding, moving and ending objects, views cost an estimated 12–18% of `cbc`'s instructions. In `csvstat`,
whose own tracking D-0199/D-0200 already cut to almost nothing else, views are nearly every object left:
about 2,000 instructions saved per copy would be roughly a fifth of its run.

This is a real cost but not the whole of P4: `cbc`'s statement and frame scopes (`cb_stmt_pop` 14.9%,
`cb_frame_pop` 13.5% inclusive on tinyos) are larger, and no change to views alone brings `cbc` within
D-0276's working bound of 10 times `cobc`.

## What the specification fixes, and what it leaves free

- **Nothing in the rules is about where a token is kept.** `spec/04` and `spec/08` speak of paths, their
  holders (`held-by`) and their validity; that the runtime finds a stored reference's token by address is
  the implementation's choice (`spec/14` §6 lets it prove checks away and keep less state).
- **A slice is three words** (`spec/16` §3a), impl-defined as `[Sizeof-Str]` is. A change that keeps the
  memory layout and puts tokens only in C variables needs no spec change; one that adds a fourth word does.
- **Slices, views and references never cross into C.** `FfiType` (`spec/20` §3) admits no reference,
  slice or `StringView`, and D-0257's library exports are FfiTypes only. The generated functions' calling
  convention for these types is the compilers' own, between C they both write.
- **D-0200's precedent:** a binding with no object, whose frame holds its path, is observably the same as
  one with an object, provided nothing can name that object: it is never the target of a borrow, never
  captured, never written through a path, and its end is the binding's (block exit or last use).

## Options

### 1. Keep the representation; prove more view uses untracked, case by case

Extend D-0191/D-0200's proofs: a view local that is only read and lent to pure view parameters gets token
0 and no object, where the proof also shows nothing can conflict with the `String` it views while it lives
(as D-0200's held element references require the vector to be named nowhere else in the block).

- For: no change to the calling convention; each proof is local and falls back when unsure.
- Against: the cost is in the general case. A view stored in a local, passed on (not just read), returned,
  or sub-viewed is the common text-processing pattern (`cbc`'s passes, `std::text`'s helpers), and none of
  these is "read only, conflict impossible". Each proof is written twice (cobc, cbc) and adds to the
  analyses' size. Expected gain on `cbc`: a few percent.

### 2. Frame-held reference data: a view's token travels beside it (recommended)

The D-0200 treatment of reference locals, extended to values that *hold* references: a local, parameter or
result of a reference-data type carries each reference's token in C beside the value, and the frame holds
those paths as an occurrence (`cb_frame_hold`), with no binding object and no slot. Memory is unchanged: a
view inside an object (a struct field of an object, a `Vec` element, a closure's capture, a thread's
argument block) keeps its slot, so the slot table and `cb_copy_datum` stay for those.

- **Types, phase 1:** `slice<τ, m>` and `StringView`: one reference each, at offset 0 of the value.
  **Phase 2** (after phase 1 is measured): any non-resource struct or enum whose references sit at offsets
  fixed by its type (`TypeInfo::fixed_refs` already computes them): `Option<StringView>` (402,729 objects on
  tinyos), tuples and token-like structs (`struct Tok { StringView text; usize at; }`), an enum payload's
  token 0 when the variant has none.
- **Locals.** `T x = e;` keeps the C struct and adds `uint64_t x_tk` (k of them in phase 2), held by the
  frame; a copy `T y = x;` is a C copy plus `cb_frame_hold(x_tk)`. Reassignment (`rest = &rest[k .. $]`,
  common in parsers) lets the old path go and holds the new (`cb_frame_unhold` then `cb_frame_hold`, or one
  new entry point). A local that is borrowed (`&w`), captured by a closure, or handed to a thread falls back
  to today's object, as reference locals do.
- **Parameters and results.** A parameter of such a type is the value plus its token(s) (`T p, uint64_t
  p_tk`); the argument's path is in flight as a reference argument's is (`cb_send_ref`, D-0107: pending
  until the callee binds it), instead of `cb_send_datum`/`cb_recv_datum`. A result is returned as
  `struct { T v; uint64_t tk; }`, re-stamped outward as a reference result is (`cb_result_ref`). Pure view
  parameters (D-0191) are unchanged: they already pass nothing.
- **Native bodies** that make views (`String::view`, `StringView::sub`, `find`, `split` and the D-0199
  text natives) return the token they mint instead of sending it as one-reference data
  (`cb_send_tok_datum`).
- **Into memory and out of it.** Storing such a value into an object's memory is today's store
  (`cb_store_ref` of the carried token at the slot's address); reading one out of memory into a local is
  `cb_load_ref` then a hold. A struct literal holding a view builds the object as today.
- **The bootstrap form** (D-0274) gains a level that switches this off, so `cbc` is ported level by level
  against `cobc` (`compare_c.sh LEVEL=N`) as every optimisation was.
- **What does not change:** every rule, diagnostic, fault point and output. A write to the `String` while a
  view of it lives meets the frame-held path and faults where it does now; a view used after its `String`
  ended faults at that use. `--all-checks` keeps checking every access. The interpreter is untouched.

Measured: the copy above, 2,689 → 668 instructions (hand-rewritten C). Estimated: `cbc` 10–15% fewer
instructions on tinyos (ceiling about 18%); `csvstat` about 20%; any program that slices text, more. Not
measured: sub-views and returned views (each still mints its path; the object, slot, send and receive go).

- For: general (every view in a local, parameter or result, whatever the code does with it); one mechanism
  that already exists for references; no spec change; memory layout and `sizeof` unchanged.
- Against: the calling convention of every generated function with a view or slice parameter or result
  changes, in both compilers, with a bootstrap level and the port to `cbc` (about 85–90 emission sites of the
  reference-data entry points in each compiler, and the native bodies that make views). The fallbacks (a
  borrowed or captured view local) must be exact, as D-0200's are. Effort: comparable to D-0200 phase 1 in
  `cobc`, then its port.

### 3. Put the token in the slice's bytes (four words)

`struct { void *src; T *data; uint64_t len; uint64_t tok; }`: every C copy carries its token.

- For: a copy in memory no longer looks its source's token up.
- Against: a path is still counted as held by each place that holds it (`held-by`), so every new location
  must still be registered and every overwritten one forgotten: the slot table, the object and
  `cb_copy_datum` stay, minus one lookup. `spec/16` §3a's "three words" changes (impl-defined, but stated),
  as do every layout assertion and `sizeof(slice<…>)` in programs. Small gain, a spec edit.

### 4. Change what a view is: unheld views, checked at use

A view holds no path; the `String` keeps a version that each write bumps, and a view's access checks it.

- For: no tracking of views at all.
- Against: a conflicting write would no longer fault at the write but at the view's next use, or never if
  the view is not used again. That changes outcomes D-0018, D-0053 and `rule.alias.*` define, the model
  (`impl/model`), the mechanised fragment and conformance cases. Breaking; listed for completeness only.

(A static lifetime analysis, as Rust has, that discharges every view at compile time is a language redesign,
not a representation choice, and is out of scope.)

## Recommendation

Option 2, in this order:

1. **Prototype phase 1 in `cobc`** (slices and `StringView`: locals, parameters, results, the view-making
   natives), behind the new bootstrap level; measure the micro cases above, `cbc` on tinyos and
   dns_over_http, and `csvstat`/`wordpar`. Abandon if `cbc` gains under 5%.
2. **Verify:** both conformance suites, the compiled suite under GCC and Clang (C identity cannot catch a
   bug both compilers share), the model sample, and a fault probe set (write beside a live view local,
   parameter and returned view; a view after its `String` ended; reassigned view locals; a view captured or
   borrowed, which falls back).
3. **Port to `cbc`** level by level; `compare_c.sh` and the fixed point.
4. **Phase 2** (fixed-offset reference data) as a second step, decided on phase 1's numbers.

Separately, and not part of this decision: routing `String::append(s, view)` straight to `append_view`
(the P4 plan's related item) stays out, because of `append(&mut s, as_view(&s))`'s fault point.

## Questions for the owner

1. Option 2 (recommended), 1, 3 or 4?
2. Phase 2 (fixed-offset reference data such as `Option<StringView>`) now, or only after phase 1's numbers?
3. Is the abandonment threshold (under 5% on `cbc`) right?

## Evidence

Micro programs and the hand-rewritten C: `stress/p4/d0280/` (`base.cb`, `copy.cb`, `pass.cb`, `sub.cb`,
`ret.cb`, `refcopy.cb`, `slicecopy.cb`, `structview.cb`; `copy2.c` as `cobc` writes it, `copy3.c` rewritten); uprobe counts on
`impl/target/selfhost/cbc` (Update 134) and on `csvstat` built by `cobc`, type ids `470981660`
(`std::StringView`) and `1206827990` (`slice<u8, shared>`); call-graph profile of `cbc` on tinyos.

## Prototype results (2026-10-25)

Built in `cobc` and `cbrt` only (uncommitted; `cbc` not ported), gated by the bootstrap form's stage 4.

**Form.** Measured before writing it: keeping the view's slot at its own address and dropping only the object
(the frame forgets the slot when the binding ends) costs 783 instructions per copy against 668 for the token in
a C variable, so the prototype keeps the slot. Every existing read of a slot then works unchanged, and no calling
convention changes.

- Objectless view bindings: a `let` or a parameter of type `StringView` or `slice<τ, shared>` that nothing
  assigns, borrows (other than `&v[lo .. hi]` of the name itself, whose path is the source's), drops or
  captures (`view_bound_excluded`) has no object: its slot stays, `cb_frame_slot` records it on the frame
  (forgotten at the frame's end, in order with the frame's other entries; `cb_frame_unslot` at its last use).
- Arguments and results sent from such a binding: a plain copy, and `cb_send_tok_datum` of the binding's
  slot (`datum_arg`, `send_datum_arg`), with no temporary.
- A call's result bound at once to such a binding is received into it (`recv_into`).
- `store` of a one-slot value is one `cb_store_ref` of the loaded token instead of `cb_copy_datum`.

**Measured** (instructions per iteration; `stress/p4/d0280/`):

| operation | before | prototype |
|---|---|---|
| view copied into a local | 2,689 | 1,070 |
| slice copied into a local | 2,728 | 1,109 |
| sub-view made (`&v[0 .. 1]`) | 7,140 | 3,648 |
| sub-view returned by a function | 14,277 | 8,062 |
| view put in a struct literal (not covered) | 4,213 | 4,221 |

| program (instructions) | before | prototype | gain |
|---|---|---|---|
| `cbc` on tinyos | 33.56 G | 32.19 G | 4.1% |
| `cbc` on dns_over_http | 269.93 G | 260.38 G | 3.5% |
| `cbc` on cobfmt | 864.94 G | 845.06 G | 2.3% |
| csvstat, wordpar, extsort | 0.937, 2.529, 2.786 G | 0.937, 2.532, 2.786 G | none |

`cbc`'s view objects on tinyos fell from 1.99 M to 0.93 M, but each removed object saved about 1,000
instructions, not the 2,000 the estimate assumed, and the text programs' remaining views are `foreach` and
`match` binders and fields, which phase 1 does not cover. **Below the 5% threshold on every input.** Correct
where checked: the C every prototype-built `cbc` writes is byte-identical to the reference `cbc`'s on the three
inputs; the conformance suite passes in full under the prototype `cobc` with GCC and Clang (1,072 cases each);
eight fault probes (`stress/p4/d0280/probes/`) match `coby` at bootstrap levels 0, 1, 4, 5 and 10 under both.

**Found on the way, independent of D-0280: a soundness bug in Update 134 (D-0277's LTO).** A function returning
reference data made a temporary object for its result, sent the result's slots on and returned; that object
kept the address of the callee's dead local until the caller's statement ended, and its end forgot the slots in
that range. Harmless while the callee's frame was dead storage; with GCC's LTO (`-flto=auto`, every GCC build
above `-O0` since D-0277) the callee is inlined and its storage given to a live local of the caller, whose slot
was forgotten: a returned `StringView`'s path lost its holder and a conflicting write went unreported, and the
second print showed empty text (`probes/p4_returned_view.cb` at bootstrap levels 1–10; level 0 happened to be
right). Fixed in `cbrt` (`unname_sent`: `cb_send_datum` makes the sender's own temporary at that address stop
naming it). The fix stands on its own and should ship whatever is decided about the prototype, with its own
change record and a conformance case that reproduces it at level 0 if one can be found.

**Options now:** (a) revert the prototype's lowering, keep the runtime fix (the threshold's answer); (b) keep it
(about 4% on `cbc`, no regression found) and port it to `cbc`; (c) extend it (`foreach`/`match` binders, struct
fields: phase 2) and measure again.

## Decision (2026-10-25)

Deferred, as the threshold said. The owner delegated the close-out ("make all necessary decisions"); the call
and its reasons:

- **Revert the lowering, keep the runtime fix.** The gain is 2.3–4.1% on `cbc` and nothing on the text
  programs; the plan set 5% to stop sunk cost here.
- **Sequencing, not failure.** The prototype pushes slot pairs on the frame list, which the larger P4 items
  (statement and frame scopes, 14.9% and 13.5% inclusive on tinyos; lazy frames) will change. Done first, part
  of it is done twice. Its saving is a fixed 1.4 G instructions on tinyos: once scopes are cut it is a larger
  share of a faster `cbc`, and the port to `cbc` costs the same either way.
- **Risk.** 300 lines of elision analysis and text surgery on emitted C in the area that produced the
  literal frame-elision, last-use-argument and CHG-0262 bugs, to be mirrored in `cbc` and the fixed point
  re-established, for 4%.
- **Phase 2.** Fields gave nothing in the probe (4,213 → 4,221) and need struct-lifetime tracking: dropped.
  `foreach`/`match` binders are the one piece with a plausible payoff above 5%, on csvstat and wordpar rather
  than `cbc`; estimate 10–15% of csvstat, with the phase-1 estimate having been high by a factor of three. If
  revisited, measure the number of view objects those programs make at binders before any code.
- **Shipped:** CHG-0372 (`unname_sent` in `cb_send_datum`), `conf.returned-view-beside-write` (3.272.0), the
  test `impl/cobc/tests/returned_view_inlined.rs` (the case at `-O1`/`-O2`, GCC and Clang). A level-0
  reproducer exists: the same program under GCC at `-O1`.
- **Kept:** `stress/p4/d0280/` (micro-benchmarks, probes, the three prototype builds' logs) and the patch.
