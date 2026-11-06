# D-0247 — A slice binding passed whole to `spawn` is lent as a value

Status: ACCEPTED (2026-10-15; delegated by the owner)
Kind: Decision (`spec/02-schema.md`); clarification of `rule.control.flow-analysis`'s spawn row, no tool change
Governed by: `CobaltC_Master_Instructions.md` §20 (adversarial review)
Depends on: D-0071, D-0213, D-0221, D-0246
Affects: `spec/14` §6 (CHG-0300)

## Problem

The spawn row forms `deriv(h, *r, m)` for an argument that is "a tracked binding `r` of reference type". A
slice is a borrow as a reference is (`type.slice`), so the row could be read to cover `spawn(f, s)` for a
slice binding `s`. Read so, `s`'s facts root the thread's lent borrow and `[Ref-Binding-Last-Use]` keeps `s`
alive to its block's exit, so the split-and-join pattern

    auto a = &mut v[0 .. 1];
    auto b = &mut v[1 .. 2];
    auto t = spawn(bump_slice, a);
    b[0] = b[0] + 1;
    join(t);
    printf("%v\n", v[0]);

faults at the last read. Both tools end `a` after the spawn (its last use) and print `11 21`. The model's
extension (D-0246) found the two readings.

## Options

(i) A `ref` only: a slice binding passed whole is lent as any value is (`[Thread-Lend]` borrows its token for
the thread) and forms no fact; (ii) a slice too, and both tools change, making the pattern above fault.

## Decision

(i), what both tools do. It is sound: the thread's lent path is checked at every access of either thread, and
the slice binding's end only ends its own path. A slice formed in the argument (`spawn(f, &mut v[0 .. 1])`)
is a visible slice and forms `deriv(h, v.[0, 1), m)`, as the row says.

## Results

The model reads the row so; `hand-l01` (the program above) agrees with both tools.
