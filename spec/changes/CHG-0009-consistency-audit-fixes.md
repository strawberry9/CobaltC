# CHG-0009 — Consistency-Pass Fixes (Third Audit)

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: inv.alias-validity, inv.resource-authority, inv.trust-transition, inv.concurrency-validity
Affects: rule.type.typing, rule.expr.context, rule.conc.join, rule.trust.rawptr, rule.stdlib.rc, rule.stdlib.prelude, state.threads, conf.reclaim-two-paths-clash

## Problem / motivation

A full read of every artifact under `spec/`, rule by rule and
cross-reference by cross-reference (2026-09-20, after `CHG-0008`),
found the specification contradicting itself in a small number of
places: typing rules that did not admit constructs the library bodies
and examples use; two rules that both claimed the same result; a
library body that faults under the rules it is written against; a
conformance case derivable only under a false trusted assertion; and a
grammar disambiguation rule that excluded every built-in-typed
declaration in the corpus. Each is resolved here toward the evident,
already-documented intent — no design choice is revisited. A larger
set of non-normative slips (stale surface syntax the `CHG-0001`–
`CHG-0008` sweeps missed, citations of rule labels that do not exist,
duplicated citations, a dependency-extraction command that matched
almost nothing) is fixed in the same pass and listed under "Hygiene"
so the artifacts' Change Logs can cite one record.

## Affected entities and what changed

### C-01 · `rule.type.typing` (`spec/12` 1.4.0)
**Previous:** `[T-Deref]` typed `*e` only for `e : ref<τ,m>`; no
rule typed `p + n` for a raw pointer; `[T-Assign]` required a place
target; `[T-Spawn]` required `e_f : fn(...)`.
**New:** `[T-Deref]` also types `*g` for `g : guard<τ>` (`spec/19`
`[Guard-Deref]`) and `*p` for `p : rawptr<τ>` (`spec/20`
`[Rawptr-Read]`/`[Rawptr-Move-Out]`); `[T-Rawptr-Offset]` types
`p + n : rawptr<τ>` for `n : isize` (`[Rawptr-Offset]`); `[T-Assign]`
admits `*p` on a raw pointer as the target (`[Rawptr-Write]`/
`[Rawptr-Move-In]`); `[T-Spawn]` is stated over `callable`, so a
`move` closure is accepted, as `rule.conc.spawn` and `spec/21` §0
already required; `[T-Never]` names `fault(d)`.
**Why:** every one of these forms already appears in `spec/21`'s
`Vec`/`Rc`/`Mutex` bodies, in `ex.unsafe-required`, `ex.threads`,
and in `conf.rawptr-deref-*`/`conf.mutex-*`/`conf.spawn-*`, all of
which `rule.fn.program` requires to be well-typed. The runtime rules
were complete; the static rules were not.

### C-02 · `rule.expr.context` (`spec/13` 1.4.0)
**Previous:** the assignment contexts were `E = e | place a = E`;
`*p = e` on a raw pointer, whose target is not a place, had no context
under which the right-hand side could step. §1 listed `*e` as a place
expression unconditionally.
**New:** `| *p = E` added (pointer first, then value — the order
`[Rawptr-Write]` already states); §1 says `*p` on a raw pointer is not
a place.

### C-03 · `rule.conc.join`, `state.threads` (`spec/19` 1.3.0, `spec/04` 1.1.0)
**Previous:** `[Join]` invoked `destroy(a)` on the handle, which runs
`[Handle-Destructor]`, which "discards `r_b`: `objs-in(r_b)`
destroyed/ended" — the temporaries `[Join]` then returned. The rule's
comment called the destructor "a no-op".
**New:** `state.threads(ℓ).value` may be `taken`; `[Join]` sets it so
before destroying the handle; `[Handle-Destructor]` discards only an
unclaimed result. `conf.spawn-join-value` now derives (`join(h)` → the
thread's value, not a destroyed temporary).

### C-04 · `rule.stdlib.rc` `Rc::drop` (`spec/21` 2.6.0)
**Previous:** `b` (an exclusive borrow of the reclaimed box, held by
`b`'s object) was still live when `drop(reclaim<RcBox<T>>(self.ptr))`
ran in the same block; `[Destroy]` requires `solitary`, so every last
`drop` of an `Rc` faulted with `diag.destroy-while-aliased`, and
`conf.rc-last-drop-frees`/`ex.rc-shared` did not derive.
**New:** the decrement and the `count == 0` test happen in a block that
ends before the destroy; the destroy runs in a second `unsafe` block
with no path into the box live. Observable behavior as intended
(and as the conformance case states) is now what the rules produce.

### C-05 · `rule.trust.rawptr` `[Reclaim]` (`spec/20` 1.3.0)
**Previous:** side-condition "hold a valid τ that no other object
claims", while the rule's premise re-attaches an existing reclaimed
object over the same cells — reachable only by asserting something
false.
**New:** "no live fresh-storage object claims"; a reclaimed object
over exactly these cells is re-attached. Membership of the trusted
condition, not its discharge, changed.

### C-06 · `conf.reclaim-two-paths-clash` (`spec/conformance.md` 3.4.0)
**Previous:** reclaimed the cells of a live fresh-storage local `x`,
violating `[Reclaim]`'s trusted condition (C-05's corrected wording,
and the old one).
**New:** reclaims an element of a `Vec` allocation through the pointer
`rawptr_of(Vec::index_shared(&v, 0))`. Outcome unchanged
(`diag.aliasing-conflict`, dynamic).

### C-07 · `spec/22` §1–§2 (2.8.0)
**Previous:** disambiguation (4) called a statement a declaration only
when its leading *identifier* named a struct or enum; the built-in
type constructors were bare terminals that `identifier` did not
exclude, so `i32 x = 5;` was an expression statement (ill-formed) by
the prose while `decl-stmt ::= type identifier` admitted it.
**New:** `type-name` groups the built-in type constructors and
`identifier` excludes them (as it excludes keywords); a statement
beginning with a `type-name` or `fn` is a declaration. `shared`/
`exclusive` are unaffected (they occur only inside `ref<τ, ·>`).
**Compatibility:** a program that named a binding, function, or
module `ref`, `array`, `bool`, `i32`, … is now ill-formed; none exists
in the corpus, and every C-family grammar reserves these names.

### Hygiene (non-normative; each artifact's Change Log cites this record)
`spec/02` 1.0.1 (dependency-extraction command now matches the bold
and wrapped labels the corpus uses — the 1.0.0 command found 57 of 696
ids; "none yet" for change records; example ids that exist);
`spec/04` (duplicated citations in `state.sync`/`state.trust`);
`spec/05` 1.1.1 (scope-membership test against the pair-valued stack;
`[Definite-Assignment]` → rule id); `spec/06` 1.1.1 (`[Sizeof-Handle]`
`outcome` tag; duplicated citations); `spec/07` 1.1.1 (destructor
signature in current syntax; `[Destroy-Composite]` as a caller of
`[Run-Destructor]`); `spec/08` 1.1.1 (`rule.ref.deref-*`); `spec/10`
1.0.1, `spec/14` 1.2.1 (`let`); `spec/12` (`[T-Closure]`,
`[T-Struct]`, generic-signature illustration, `let`); `spec/13`
(`reclaim<τ>(p)`, `[Auto-Deref]`); `spec/15` 1.2.1 (`fn main() :
void`, generic-item illustration); `spec/16` 1.2.1 (struct declaration
and literal shapes, `[Ref-Form-Temporary]`, `let`); `spec/19`
(`[Mutex-New]` placed under `rule.conc.lock`; `type.handle`'s
destructor citation); `spec/20` (`reclaim<τ>(p)`); `spec/22` (`[Shl]`/
`[Shr]`/`[Propagate]` label citations; correspondence-table rows for
raw-pointer and guard forms); `spec/README.md`, `spec/conformance.md`
and `spec/examples.md` conventions (`c`), `spec/AUDIT-STATUS.md`.

## Affected invariants

None is restated. C-03 and C-04 restore `inv.resource-authority`'s
"exactly one owner discharges each obligation" for the joined result
and the `Rc` box respectively — the previous text had `[Join]`
destroying a temporary it then handed to the caller, and `Rc::drop`
unable to reach its own deallocation. C-05 concerns the wording of a
`discharge: trusted` condition under `inv.trust-transition`; C-01/C-02
close static gaps beneath rules whose dynamic checks were already
complete (`inv.alias-validity`, `inv.spatial-validity` unaffected).

## Dependency impact

`rule.type.typing`'s Depends on is unchanged (it already depended on
the rules whose forms it now types through `spec/22`'s correspondence
table). `state.threads` gains a value alternative; `rule.conc.join`'s
Affects already lists `state.threads`. No other `Depends on`/`Affects`
line changes. The `spec/02` §4 extraction command was replaced because
the previous one could not have detected a dependency drift at all.

## Compatibility classification

C-01, C-02, C-03, C-04, C-05: semantics-completing, not
source-breaking — every program the corpus contains was intended to
be, and is now, well-formed with the stated outcome; no previously
well-formed program changes meaning. C-06: a conformance fragment
rewritten, same outcome. C-07: source-breaking for programs using a
built-in type constructor's name as an identifier (none in the
corpus); semantics-preserving otherwise.

## Migration implications

Rename any user identifier spelled as a built-in type constructor
(`ref`, `rawptr`, `array`, `handle`, `mutex`, `guard`, `bool`, `f32`,
`f64`, or an `int-type`). Nothing else.

## Example changes

`spec/examples.md` 3.4.1: header convention for `c`; no example's
fragment, category, or outcome changed. `ex.rc-shared`, `ex.threads`,
`ex.unsafe-required` now derive as written.

## Conformance changes

`conf.reclaim-two-paths-clash` re-based (C-06). `conf.rc-last-drop-
frees`, `conf.spawn-join-value`, `conf.unjoined-handle-waits`,
`conf.rawptr-deref-inside-unsafe-ok`, `conf.mutex-*` now derive
without appeal to unstated rules. No outcome changed.

## Future implementation implications

A type checker must implement `[T-Deref]`'s two new cases,
`[T-Rawptr-Offset]`, the `*p` assignment target, and `callable`-based
`[T-Spawn]`; a lexer reserves the `type-name` tokens; a runtime's join
must record that a result was claimed. Every one of these was already
required to run the specification's own library and examples; this
record makes the requirement derivable from the rules.

## Prior-art status

Not applicable (`spec/02-schema.md` §2: a Change record); no
mechanism was selected or rejected.

## Revisit conditions

If a future record reserves further contextual tokens (`shared`,
`exclusive`), fold C-07's `type-name` list into a single reserved-word
table. If `[T-Join]` is ever folded into ordinary generic typing
(`CHG-0008`'s open question), re-check `[T-Spawn]`'s `callable`
statement at the same time.
