# CHG-0283 — `cobc`: `(*v)[k]` on a confined `Vec` reference parameter; the guide's section for C programmers

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-15; a tool defect found while writing the guide's section for C programmers, which the
owner asked for: "create a new section specifically oriented towards helping C programmers … pay particular
attention to error handling")
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0047 (`[Index-Vec]`), D-0191 (confined parameters)
Affects: `impl/cobc/src/lower/vec_calls.rs`; `spec/conformance.md` (3.225.0); `htmlguide/generator/content_cprog.py`
(new), `gen.py`, `README.md`; `spec/02` §5 (1.0.89)

## What changed

- **A `cobc` defect, repaired.** `(*v)[k]`, with `v` a `ref<Vec<T>, m>` parameter the compiler had *confined*
  (D-0191: a parameter used only for element access and length, lowered without a binding of its own), was
  refused with `unsupported: internal error in cobc: unresolved path 'v'` (exit status 3) under GCC and Clang,
  while `v[k]` on the same parameter, `(*a)[k]` on an array parameter and `(*r)[k]` on a local reference all
  compiled, and `coby` ran every form. `vec_index_call` recognised a confined parameter only when the index
  base was the bare name; it now also recognises the dereference of the name, parentheses stripped, and
  lowers `(*v)[k]` exactly as `v[k]`: `*Vec::index_shared(&*v, k)`, or `index_exclusive` where the place is
  written. No program that compiled before is lowered differently. Case:
  `conf.vec-index-through-deref-param` (read, written, and with a computed index; passes under `coby`, `cobc`
  with GCC and Clang, and `cobc --all-checks`; fails on the Update 107 compiler).
- **The guide: "CobaltC for C programmers"**, a new section in the "Start here" group
  (`htmlguide/generator/content_cprog.py`): what carries over from C; a table of C spellings and what they
  mean here; the three habits the compiler stops (silent overflow, implicit conversion, escaping pointers);
  memory without `malloc`; strings; and, at length, error handling: the two channels (error values and
  faults) against C's single convention, `Option` in place of `NULL` with its four ways of being taken apart,
  `Result` in place of return codes and `errno`, `?` as the early return with its three rules (same error
  type, `map_err` at boundaries, `ok_or` between `Option` and `Result`), the three-layer shape of a program
  whose `main` matches once and reports, faults as an `abort()` that unwinds, `assert`, and a side-by-side
  table of idioms. Twelve checked examples, one of which found the defect above.

## What changed for programs

A program writing `(*v)[k]` on a confined `Vec` reference parameter now compiles under `cobc`, as the text
always said it should and `coby` always did. Nothing else.
