# D-0309 — Decided programs at least as fast as Rust with comparable checks

Status: ACCEPTED (2026-11-04, the owner: "I want equal or better performance for decided programs compared to Rust
with comparable runtime checks. This needs to be a goal for decided programs."; on the proposal: "1(b) now, 2(d),
and 3 agreed; proceed autonomously"); implemented by `CHG-0410`
Kind: Decision (`spec/02-schema.md`); a goal and its measure, a call rule (`[Call-Self-Tail]`), and an
implementation rule for the stack check (D-0077)
Governed by: `CobaltC_Master_Instructions.md` §8 (priorities: performance with every check kept)
Depends on: D-0077 (`[Call-Stack-Exhausted]`), D-0270 (the path model), D-0281/D-0285 (decided programs),
D-0292 (light references), D-0283 (point checks)
Affects: `spec/15` §2 (`[Call-Self-Tail]`, §2a); `spec/conformance.md`; `bench/run.sh` (the ratio table); `coby`
(`self_tail.rs`, the interpreter's loop), `cobc` and `cbc` (the jump, `stack_checks.rs`), `cbrt` (the stack floor
off Linux)

## The goal

A decided program (D-0285) is to be as fast as Rust with comparable runtime checks, or faster. The measure is
`bench/RESULTS.md`'s "Rust, overflow checks" column: instructions as the number the gate holds, time reported
beside it, under both C compilers. `bench/run.sh` prints each `cobc` build's ratio to it. No check that checked
Rust also makes (bounds, overflow, division, narrowing, `assert`) may be weakened or moved to meet the goal; only
costs Rust does not pay are in play.

## The stack check (option 1b, no rule change)

`cb_stack_check` stays only where the depth of calls can grow without bound: in a function on a cycle of direct
calls, one whose address is taken anywhere (a `fn` value, a closure body, a destructor in a type descriptor, a
thread body), one nothing in the program calls, one calling `alloca`, and one whose chain of callers without a
check, together with the chain below it, could use more than 64 KiB below the last check. The floor's margin
(`max(256 KiB, size/16)`) covers that budget, as it covered a leaf before. `main` is the first frame of a thread
whose stack is all unused and needs none of its own. `[Call-Stack-Exhausted]` is unchanged. Guard pages (1c) were
not taken: the unwind cannot run in a signal handler that may have interrupted the allocator or the runtime.

The floor is now found off Linux too (Windows: the stack's reservation through `VirtualQuery`; macOS: the thread's
stack address and size), so the check fires there; before, it never did, and a deep recursion was a host crash on
those targets.

## `[Call-Self-Tail]` (option 2d)

A self tail call is not a level of nesting for `[Call-Stack-Exhausted]` when the function's frame holds nothing to
destroy and nothing in it is ever borrowed, and its body is decided (`spec/15` §2a). Everything else is `[Call]`'s.
The three toolchains decide the same calls from the same syntax and the static pass's facts. `coby` runs one by
returning to the call running the body, `cobc` and `cbc` by a jump to the body's start. Letting the C compiler
turn tail calls into jumps (2b) was declined: the fault would depend on the C compiler and the optimisation
level, and `coby` and `cobc` would disagree.

## Measured (2026-11-04, user-space instructions, gcc)

`tree` 71.4 M → 51.9 M (−27%, clang −33%); `sieve` 2.77 G → 2.53 G and `sieve_fn` 4.06 G → 3.82 G with the stack
check gone from their non-recursive functions; `hashcount` back to its count of 2026-10-31.
