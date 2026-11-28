# D-0325 — `Vec::any`, `Vec::all` and `Vec::count`

Status: ACCEPTED (2026-11-14, the owner, on `private/EASE-PLAN-2-2026-11-14.md`, item 5: "implement the revised
plan in full and push once completed"); implemented by `CHG-0454`
Kind: Decision (`spec/02-schema.md`); three functions of `std`
Governed by: `CobaltC_Master_Instructions.md` §1, §23
Depends on: D-0112 (`Vec::position` and its family)
Affects: `spec/21` §1 (`rule.stdlib.vec`, the row of D-0112's helpers); `impl/std/collections.cb`;
`spec/conformance.md` (3.312.0)

## 1. Problem

A program that asks whether some element, or every element, holds a predicate, or how many do, writes the loop
each time; `Vec::position` answers only where the first is. A program written cold asked for `Vec::count` first
(`private/EASE-PLAN-2-2026-11-14.md` §1).

## 2. Decision

    export fn Vec::any<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>) : bool p) : bool
    export fn Vec::all<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>) : bool p) : bool
    export fn Vec::count<T>(ref<Vec<T>, shared> v, fn(ref<T, shared>) : bool p) : usize

`any` calls `p` in index order and stops at the first element for which it is true; `all` stops at the first
for which it is false; `count` calls it once for each element. An empty `Vec` gives `false`, `true` and `0`.
Each borrows each element for its predicate, as `Vec::position` does (a live exclusive reference to an element
is `diag.aliasing-conflict`).

## 3. Consequences

Ordinary CobaltC functions beside `Vec::position`; no rule of the language changes. `hints::fn_name` keeps
mapping another language's `count` to `len` for the types that have no `count`.
