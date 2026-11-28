# CHG-0382 — Evaluation order counts only what can move what an earlier argument reaches

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-27)
Governed by: `CobaltC_Master_Instructions.md` §21
Depends on: D-0281, D-0282, D-0284, D-0007
Affects: `spec/14-control-flow.md` 1.36.0 (§6 `rule.control.frame-references`, evaluation order);
`impl/src/typecheck.rs`

## Problem

A call's arguments are evaluated left to right, and a reference argument is formed before the later ones. The
rule that leaves such a call to the run, when a later argument may move what an earlier one reaches, compared
roots only, and counted every argument:

- a value argument (`f(x, g(x))` with `x` an integer, `q.flag`) was taken for a reference to `x`, so passing `x`
  again by value in a later argument "moved" it;
- a later argument that is itself a borrow (`swap_places(&mut (*v)[i], &mut (*v)[j])` in `std::Vec::swap`,
  `Queue::shift(&mut q.tail, &mut q.head, q.took_back)` in `std::Queue::pop_front`) counted as taking the root
  exclusively, though forming a borrow in place moves nothing;
- an exclusive borrow of a disjoint field counted as one of the whole root.

Every program that called `Vec::swap` or `Queue::pop_front`, or the crypto helpers that pass the same array twice,
kept its record of references for the whole run.

## What changed

1. Only an argument that is or may hold a reference is formed early (`arg_may_be_reference`: a borrow, a slice,
   or a value whose type may hold a reference; a literal never).
2. A later argument that is a borrow (`&_m p`, a slice of `p`) is formed in place: only the exclusive borrows its
   operand evaluates first (an index, a call inside it) count. Two borrows that meet are the call-site rule's,
   decided before the run or checked at the call, as before.
3. An exclusive borrow counts only when its path overlaps what the earlier argument reaches (`overlap3`), not
   whenever it shares a root.

Assignments into the root and moves of it in a later argument count as before; so does a later argument that
assigns a reference binding an earlier argument read.

## What changed for programs

No program's outcome changes: the rule only leaves conflicts to the run, and no rejection depends on it. Over
11,022 programs (the static comparison's corpus, `showcase/`, `cbc`, `cobfmt-cb`, `cbparse`), 116 more are decided
(9,179 in all), among them every user of `Vec::swap`; 57 more stop at a later reason (most, a type that keeps
references in data). `set(&mut v[0], grow(&mut v))` is still left to the run.

## Conformance

None: what is decided is a lowering property, gated by `stress/d0281/pathless/gate/gate.sh` over the programs it
newly decides (see `impl/CHANGELOG.md`). `impl/cobc/tests/driver.rs`
`vec_swap_and_value_arguments_leave_a_program_decided` pins it.
