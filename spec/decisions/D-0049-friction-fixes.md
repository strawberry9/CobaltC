# D-0049 — Four frictions from writing larger programs

Status: ACCEPTED (2026-09-26, owner-delegated: "resolve all friction points using your best judgment")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §12, §17
Depends on: D-0006, D-0022, D-0033, D-0037, D-0046, D-0047, rule.type.expected, rule.fn.bind-param, rule.value-object.write, rule.agg.match
Affects: rule.type.identity, rule.type.expected, rule.fn.bind-param, rule.value-object.write (`live-resource-at`), rule.control.flow-analysis, rule.agg.match

## Problem

Writing a dozen larger programs with the features added on 2026-09-26
(slices, `Box`, `HashMap`, `foreach`, match through a reference,
`swap`/`replace`) ran into five points where the language was correct
but got in the way:

1. `auto a = if (c) { x } else { 0 };` with `x : i64` was
   `diag.type-mismatch`: the `0` was typed `i32`. `match` already let a
   later arm take the first arm's type, but not an earlier literal arm
   the type of a later one (`match (o) { None : 0, Some(v) : v }`).
2. A function holding `ref<Vec<T>, exclusive> v` could not call
   `Vec::len(v)`; it had to write `Vec::len(&*v)`. The same for any
   shared-reference parameter, and for slices.
3. `*slot = Some(x)` over a `None` of an `Option<Box<Node>>` was
   `diag.overwrite-of-live-resource`: whether a value "holds a live
   resource" was decided by its type, although a `None` owns nothing.
   The workaround, `replace(slot, Some(x))`, had to be taught.
4. `match (o) { None : …, _ : o }` was `diag.stale-binding`: a by-value
   match consumed a resource scrutinee whichever arm ran, even one that
   takes nothing out of it.
5. `auto m = Mutex::new(HashMap::new());` could not infer the map's
   types. This one is the ordinary rule — types flow down from a
   declaration, not up from later uses — and a declaration fixes it:
   `mutex<HashMap<String, i64>> m = Mutex::new(HashMap::new());`. No
   change; the guide shows the form.

## Selected design

1. **Literal branches take their sibling's type.** Where nothing
   outside fixes a type, the branches of an `if` or `match` take the
   type of the first branch that is not a *literal branch* (a literal
   expression, D-0037's definition, or a block holding only one).
2. **Shared reborrow at calls.** An argument of type
   `ref<τ, exclusive>` is accepted for a parameter `ref<τ, shared>` of a
   called function item (directly or through `spawn`) and passed as
   `&*e`: a shared borrow through it, with it as ancestor. Likewise
   `slice<τ, exclusive>` for `slice<τ, shared>`. Not for calls through
   a `fn` value or of a closure, whose parameter types are part of a
   value's type.
3. **Overwrite judged by the value.** `live-resource-at` also requires
   that the value at the target *owns* something: a type declared
   `resource` or with a destructor always does, as do handles, mutexes,
   guards and closures; a derived struct owns if a field does; a
   derived enum if the active variant's payload does. So a `None`
   (or a struct whose `Option` fields are all `None`) may be written
   over. Statically the write is refuted only for a type all of whose
   values own something; otherwise the run time decides.
4. **A match consumes only what it moves.** A by-value `match` on a
   resource enum consumes it only in an arm that binds a resource
   payload; any other arm leaves the scrutinee with its owner (a
   temporary one ends with its statement).

## Rejected alternatives

- (1) **Type the literal after both branches are known** (unification
  over branch types): a new mechanism for one case; "the first
  non-literal branch" reuses the existing expected-type rule.
- (2) **Implicit weakening everywhere** (bindings, returns, fields):
  the call is where the friction is, and `&*v` stays available;
  wider coercion would blur what `rule.type.identity` promises.
  **Keep `&*v`:** it is the idiom Rust users do not write either.
- (3) **Keep the type-based rule and teach `replace`:** a `None` has
  no obligation to strand, so the rejection guarded nothing.
  **Track the variant statically:** the flow analysis follows
  validity and initialization, not values; a dynamic check where the
  type does not decide is how other `unknown` facts are handled.
- (4) **Consume in every arm (as before):** a scrutinee left intact
  was a hazard nowhere, and the rejection taught `match (&o)` for a
  case that needs nothing.

## Semantic rationale

(2) is `[Borrow]` shared through an exclusive path, which D-0018 and
D-0022 already admit; the callee sees a shared reference, as with
`&*v`. (3) keeps `inv.resource-authority`: an obligation is stranded
only if the overwritten value owns a resource. (4) keeps it too: the
un-moved scrutinee keeps its owner, who ends it as usual.

## Usability

    auto size = if (big) { n } else { 0 };
    usize n = Vec::len(v);                 // v : ref<Vec<T>, exclusive>
    *slot = Some(Box::new(node));          // slot held None
    match (o) { None : {}, _ : use(o) }

## Explainability

"A literal branch takes the other branch's type." "An exclusive
reference can be passed where a shared one is expected." "You may
write over an empty `Option`." "A `match` only uses up what it moves
out."

## Implementation-feasibility

The checker records a literal branch's type for `coby` (as for
`match`); `cobc` probes the branches. `coby` reborrows when it binds a
parameter; `cobc` emits the borrow at the call and ends a slice
formed for the call when it returns, as `coby` does. The run-time
check of (3) reads the value (`cbrt`'s `cb_owns`, with an `owner`
flag in the type descriptor). (4) moves out of a binding only in the
arm that binds the payload.

## Compatibility impact

Relaxations: every program accepted before is accepted and behaves
the same, except that a temporary resource scrutinee matched by an arm
that moves nothing now ends at the end of its statement rather than
before the arm runs (its destructor's output moves after the arm's).

## Prior-art status

(1) Rust infers the literal's type from the other branch. (2) Rust
coerces `&mut T` to `&T`. (3) Rust allows assigning to an
`Option<Box<T>>` place holding `None` (the old value is dropped,
which for `None` does nothing). (4) Rust's `_` pattern does not move.

## Invariant traceability

`inv.alias-validity` (2: a reborrow is a borrow), `inv.resource-authority`
(3, 4: no obligation lost or duplicated).

## Revisit conditions

If a program needs (2) through a `fn` value, extend it there.
