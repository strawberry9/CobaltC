# D-0340 — A certain fault is refused: the left items of D-0339

Status: the owner's rulings (2026-11-25), item by item over D-0339 §3, each on the implementer's recommendation.
Implemented by `CHG-0483` (§2.10 left, §3)
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0037 (literal expressions), D-0049 (`overwrite`), D-0065 (`assert`), D-0213 (`spawn` lends), D-0284
(a result whose borrow is not visible), D-0313 (report 313), D-0336 §2.7, D-0339
Revisits: D-0037 (a literal expression's overflow is no longer the run's), D-0284 (a reference returned through a
binding or an `if` is refused when a path reaches a local)
Affects: `spec/06`, `spec/10`, `spec/12`, `spec/14`, `spec/18`, `spec/19`, `spec/registry/diagnostics.md`,
`impl/conformance/README.md`

## 1. Problem

D-0339 §3 left thirteen groups of reports to the run-time record or to later changes. The owner's principle on
reading them: a certain result from a static check is not left as a run-time check. The principle is made a rule of
the language, and each left item is judged by it.

## 2. Decision

0. **The rule (owner): a certain fault is refused.** `rule.fail.certain-fault-refused` `[Certain-Fault-Refused]`
   (`spec/18` §1): a fault that is certain once a path is taken is a compile-time error with that fault's
   diagnostic, even when whether the path runs depends on input or on threads' timing (owner, item 2: certainty is
   per path, as Rust's checks are). The run-time check stays only for what the static facts do not know. The
   program's own stop (`exit`, `assert(false)`, a `: never` call, a failed `requires`) is not a fault in this
   sense.
1. **Literal expressions are folded wherever they stand (384).** An operator whose operands are known, nested or
   not, with an expected type or none (`printf("%v", 100 * 99999 * 100000 / 2)`), is checked at `--check`: an
   overflow is `diag.arith-overflow`, a division or remainder by a known zero `diag.div-by-zero`. Each node is folded
   once (its value kept by its id), so a deep expression stays linear. D-0037's
   `conf.literal-expression-overflow-dynamic` becomes a refusal; the cases that failed on purpose with
   `1 / (1 - 1)` use `assert(false)` (`std::assert` where the case has no `import std;`).
2. **A local's loan returned through a binding is refused (395, 414).** A returned reference, view or handle whose
   facts on any path reach a local of the frame is `diag.reference-escapes-scope`, as the direct form is; a closure
   without `move` given to a binding and returned is refused with the `move [...]` repair. A `move` closure is not
   refused. D-0284's `conf.result-through-if-checked-at-run` becomes a refusal.
3. **Views kept beyond their text (394, 392).** A holder (`Vec<StringView>`, a struct holding one) that may refer
   to a binding when that binding's block ends, on some path, and is used later, is
   `diag.destroy-while-aliased` at `--check`, as a sure one was (394). A write to the text while such a holder
   may hold its views (392) stays the run's: the checker's "may hold" does not tell a `Vec` that may be empty, or
   whose reference was popped, from one that holds the view, so by §0 the fault is not certain (§3).
4. **`Option<R>` assigned whole (413).** A plain `=` to an `Option<R>` binding, `R` owning something whose end does
   more than free memory, is `diag.overwrite-of-live-resource` unless the binding is `None` on every path reaching
   the assignment (declared or last assigned `None`); the repair names `overwrite` and `replace`.
5. **A thread's lost `Result` (380).** A `handle<Result<…>>` binding that may reach its scope's end unjoined is
   `diag.result-discarded`; a handle in a container that ends unjoined after its thread gave `Err` faults with
   `diag.result-discarded` at run time (whether it is joined is not known statically).
6. **A handle that outlives what its thread borrows (382).** Returning a `spawn(…)` whose arguments lend a local
   of the frame is `diag.reference-escapes-scope`.
7. **The same exclusive element lent to threads across rounds (381).** Report 313's refusal covers element places:
   two indices are one place unless both are known and differ.
8. **A guard taken to another thread (390).** `diag.thread-affine-moved`: a guard stays on the thread that locked
   it.
9. **Inference (409, 410, 411).** The lowering takes the type arguments the checker settled; `--check` accepts only
   what builds. Unification across array elements, `if`/`match` arms and struct fields completes 409's programs;
   a type argument nothing fixes (410) is `diag.type-annotation-needed` at `--check`; 411 is the type error
   `Option::unwrap(&a)` gets alone.
10. **A reassigned reference (418).** Assigning a reference binding whole is not a use of its old loan: the old
    loan ends at its last read before the assignment (`spec/14`, "the last statement that reads x").
11. **Condition temporaries (412).** The temporaries of an `if` condition, and of a `match` scrutinee of an integer
    or `bool` type, end once it is evaluated, before any branch; `--all-checks` agrees with the default build.
12. **`never` in signatures (399).** `: never` may be written as a function's result type only; every path of the
    body ends in a `never` call or a loop without `break`; a `return` or a path reaching the end is
    `diag.never-returns`. A call is `[T-Never]`.

## 3. Left

- 392's write to a text a holder may view: left to the run (§2.3); both builds fault (D-0339 §2.5).
- §2.10 (418) is decided but not yet implemented: ending the old loan at its last read needs liveness that
  follows branches (a rule by source position alone could let a real conflict through in a program that keeps no
  run-time record). Until then the program faults at run time in both builds.
- §2.9's `auto` `if`/`match` whose branches each fix one type argument (409) is refused at the check with
  `diag.type-annotation-needed` rather than compiled: a variant literal alone has no partial type in the checker.
- 411 is refused with `diag.type-annotation-needed` (the lowering, now run by `--check`), not with the type error
  `Option::unwrap(&a)` gets on its own.
- Priorities 3–5 of reports 350–451.
