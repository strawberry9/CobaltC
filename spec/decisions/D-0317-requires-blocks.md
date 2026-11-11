# D-0317 — A function's `requires` block: what its caller must establish

Status: ACCEPTED (2026-11-11, the owner, on the agent's proposal and two rounds of review: "agreed. implement
requires after the push autonomously.", and on the form: "honestly I prefer the block version."); implemented by
`CHG-0435`
Kind: Decision (`spec/02-schema.md`); one optional production, one rule defined by rewrite, one row of the
discharge table, two diagnostics
Governed by: `CobaltC_Master_Instructions.md` §4, §9, §17 (Function Interfaces), §18, §19
Depends on: D-0009 (faults), D-0065 (`assert`: every check always on), D-0296 (both toolchains in step), D-0312
(`from`, the contextual word in the same slot)
Affects: `spec/22` §3 (`fn-decl`, `requires`), `spec/15` (`rule.fn.requires`), `spec/14` §6 (a row of the
discharge table), `spec/registry/diagnostics.md` (`diag.precondition-failed`, `diag.requires-misplaced`),
`spec/registry/features.md` (`feat.requires-block`), `spec/21` (the `std` functions whose guards become
requirements), both front ends, both back ends, both formatters, `impl/tools/cobdoc.py`; `spec/conformance.md`
Analysis: `private/contracts_proposal/contracts-implementation-proposal.md` (not part of the repository)

## 1. Problem

A function cannot state what its caller must establish. It writes a guard at the top of its body and the same
condition again in a comment for the reader. Three things follow:

1. **The report names the wrong place.** `if (c) { fault(d); }` or `assert(c)` in a program function is reported
   at the guard, inside the function; the call that broke the condition is not mentioned. A fault in `std` is
   reported at the program's call only because `std`'s code has no location.
2. **A call that can never satisfy it is accepted.** `withdraw(5, 9)` against `amount <= balance` fails only
   when run, where `10 / z` with `z` literal-bound to `0` is rejected before it.
3. **The interface is in a comment.** `std` states 24 such conditions twice, as a guard and as "Faults when"
   prose; `cobdoc.py` exports the prose; nothing checks the two agree. §17: "Function interfaces must carry
   enough semantic information to preserve interprocedural guarantees without undocumented caller/callee
   agreements."

## 2. Candidates

1. Nothing: the guard and the comment.
2. A `std` function `require(c, f, a…)`: an `assert` reported at the caller. It fixes 1 only, and is legal
   mid-body, where "the caller's fault" means nothing.
3. **A `requires` block on the signature, between the result (and its `from` clause) and the body.**
   **Selected.**
4. Separate `requires(…)` clauses, one per requirement: the same meaning; the owner preferred one block.
5. Inference from a body's leading guard: position would change what a call means.
6. Contracts in separate files, or an attribute naming one: two authorities (D-0229's reason), and D-0303
   closed the attribute set.
7. `ensures` and `old` with it: deferred (§6).

## 3. Decision

### 3.1 Surface

    fn-decl     ::= vis 'fn' name type-params? '(' params? ','? ')' (':' type result-source?)? requires? block
    requires    ::= 'requires' '{' requirement+ '}'
    requirement ::= expr (',' identifier)? (',' str-literal (',' expr)*)? ';'

`requires` is a contextual word, as `from` is: the block's word only where it follows the signature and is
followed by `{`. At most one block; none empty. The identifier names a diagnostic as `fault(d)`'s first
argument does (its phase `dynamic` or `both`); without it the diagnostic is `diag.precondition-failed`. The
message is a format and its arguments, as `assert`'s, formatted only on failure.

    fn withdraw(u64 balance, u64 amount) : u64
        requires
        {
            amount > 0, "amount must be positive";
            amount <= balance, "amount %v exceeds balance %v", amount, balance;
        }
    {
        balance - amount
    }

### 3.2 Meaning (`spec/15` `rule.fn.requires`)

A function with requirements r1 … rn behaves as the same function whose body begins, for each k in order, with

    if (!(ck)) { fault(dk, message_k) }      -- the fault located at the call

evaluated with the parameters bound, once per call, in every build. So a later requirement may rely on an
earlier one; a fault inside a requirement (an overflow) is that fault, at the requirement; a generic function's
requirements are checked per instantiation; a call through a `fn` value, a `std` callback or `spawn` checks them
(the check is in the callee), located at the call the program wrote (the `spawn` for a spawned function). A self
tail call stays one (`spec/15` §2a is unchanged): each iteration is a call and checks them, a failure located at
that tail call.

### 3.3 Where a block may stand

On a `fn` with a body, associated and generic ones included. `diag.requires-misplaced` (static) on `main`, on
`T::drop`, on a `const fn`, on an `extern fn` definition (a C entry) and on an `unsafe extern fn` declaration.

### 3.4 Discharge (`spec/14` §6)

A requirement is *foldable* when it is built only from integer literals, `true`, `false`, the function's
by-value parameters of an integer type, `+ - * / %`, unary `-`, the six comparisons, `&&`, `||`, `!` and
parentheses. At a call that names the function by its path, a foldable requirement whose named parameters all
have arguments of known value (a literal or a literal-bound local) is evaluated: `true` proves it, `false`
rejects the program at the call with `diag.precondition-failed` (static), whatever diagnostic the requirement
names. A fold that overflows or divides by zero, a requirement that is not foldable, and a call through a `fn`
value leave it to the run. Not being foldable never makes a requirement invalid.

## 4. Rejected alternatives

- **2:** blame only; the interface still in a comment; no static rejection.
- **4:** the owner's preference for one block; the cost is a `;` separator and teaching every reader of
  signatures that the first `{` after one may be the block's.
- **5:** moving a line would add or remove a static rejection with nothing in the signature to show it.
- **6:** D-0229, D-0303.
- **A read-only rule inside the block:** not needed. The block is body code; the flow analysis already rejects
  a body that uses a parameter a requirement moved.
- **Banning self tail calls in such a function:** `spec/15` §2a condition 6 concerns the checks the static pass
  leaves to the run, not every check a body can fail; a body with an `assert` makes self tail calls today.

## 5. Costs

None for a function without a block. A requirement costs its predicate once per call; a caller of a program
function with a block stores its position before the call (one thread-local store, which `std` callees
already get); the callee reads it on entry. The runtime library is unchanged.

## 6. Deferred

`ensures` and `old`: no code in the repository states a postcondition; an always-on check of what a body just
computed works against the goal of matching checked Rust; and its rules (when it runs against the destruction of
locals; `old`'s snapshot) are not small. The block's word and slot leave room for it.

## 7. Revisit when

Programs show postconditions written as comments or as checks before a return (`ensures`); the checker's value
reasoning grows beyond literals (the block is where its facts would be stated).
