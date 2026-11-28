# D-0327 — An object never written ends with nothing

Status: ACCEPTED (2026-11-16, decided under the owner's instruction of that day on the work after D-0326: "you
need to make all decisions needed yourself, so that the migration project is complete"); implemented by
`CHG-0463`
Kind: Decision (`spec/02-schema.md`); `[Block-Exit]`'s text, corrective
Governed by: `CobaltC_Master_Instructions.md` §12, §17
Depends on: D-0008 (scope-end destruction), D-0033 (`[Assign-Reestablish]`), D-0315 (a decided program's
`let` with no value)
Affects: `spec/14` §1 (`rule.control.block`, `[Block-Exit]`, 1.47.0); `impl/cbrt/src/rt_objs.c`;
`spec/conformance.md` (3.316.0)

## 1. Problem

`τ x;` establishes an object with no value (`[Let-Uninit]`). Definite assignment (`spec/11` §3) asks that `x` be
written before it is read, not that it be written at all: a function may leave by a path on which `x` was never
assigned, and a fault may end the frame between the declaration and the first assignment.

    fn pick(bool c) : i32
    {
        Noisy n;                      // a resource struct with a destructor
        if (c)
        {
            n = Noisy { .id = 7 };
            return n.id;
        }
        0                             // n was never written
    }

What ends then? `[Object-Establish-Resource]` gives a resource object its destruction obligation at
establishment, and `[Block-Exit]` as written destroys every owned object under obligation: read to the letter,
the destructor is called on an object with no value, and its first read is `diag.use-of-uninitialized`, in a
program the static rules accept.

The implementations disagreed, which is how this was found (the release profile's `all-checks` unit on Update
195, its first run on the tools written in CobaltC):

- The interpreter (retired by D-0326), the model (`impl/model`, an object whose value is absent ends with
  nothing) and `conf.unassigned-let-takes-first-value` (`CHG-0420`: "a binding of `[Let-Uninit]` that a fault
  leaves before its first write ends with nothing in it") ended it with nothing.
- The compiler with every check on followed the letter: the destructor ran and faulted.
- The compiler's default form ran the destructor with its checks removed, on memory never written: `pick(false)`
  above printed the destructor's line with whatever the stack held. A destructor that releases something would
  release garbage. (`std`'s memory types escaped this in a decided program, which gives them no object and a
  moved flag, D-0315.)

## 2. Options

1. **Reject statically** a binding of a resource type that can meet its end unassigned. It would refuse the
   pattern the suite blesses (declare, assign on each path later) whenever a `return`, a `?` or a call that can
   fault stands between the declaration and the assignment: nearly every use of `τ x;`.
2. **Keep the letter**: the destructor runs and its read faults at run time. A fault in a program with nothing
   wrong in it; and the default form would need the check it has discharged kept for every destructor.
3. **An object never written ends with nothing.** No value was ever put there, so there is nothing to destroy.

## 3. Decision

Option 3, which the interpreter, the model and the conformance suite already held. `[Block-Exit]` destroys an
owned object under obligation only when `init-state = valid`; one that is `uninitialized` ends by
`[Object-End]`, its obligation and its authority discharged, no destructor run and nothing in it destroyed.
Every unwind that ends a frame as `[Block-Exit]` does follows: a `return`, a fault's unwind, a contained
fault.

## 4. Consequences

No program that ran correctly changes: the programs affected ran a destructor on memory never written, or
faulted with every check on. An object is `uninitialized` only between `[Let-Uninit]` (or
`[Assign-Reestablish]`'s establishment) and the whole-object write that follows; a moved-from binding has no
live object and was never in question. The runtime's record holds the same flag the rule reads
(`cb_new_uninit`), so the rule costs nothing at run time.
