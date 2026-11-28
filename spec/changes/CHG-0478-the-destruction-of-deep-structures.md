# CHG-0478 — The destruction of deep structures

Status: implemented (2026-11-21); Update 206
Affects: `spec/16` 1.39.0 (§4a, §6), `spec/conformance.md` 3.323.0 (1 row); `impl/cbrt` (`rt_objs.c`, `rt_init.c`,
`rt_threads.c`, `rt.h`); the guide, the book (chapter 8), `impl/STATUS.md`, `CHG-0477`; D-0335

## Why

The owner asked that no report stay open. D-0335 has what was decided; this record has what changed.

## What changes

| # | Update 205 left | Now |
|---|---|---|
| 022 | in the bootstrap form the destruction of a deep tree that was walked or cloned recurses | `box_drop` keeps further tail Boxes on a per-thread list (`TState::box_more`) |
| 022 | in the default form that of a chain or tree whose values own a `String` recurses | `TypeInfo::quiet` and `nquiet_owning` (`rt_init.c`): a field is in tail position when what is destroyed after it only frees memory; `conf.drop-deep-owning`, three shapes two million levels deep, default form 4 s, `--bootstrap-form` 320 s |

## What is left

- 022: the derived `clone` of a type that links to itself only through another type recurses; a field declared before
  the link that runs another destructor (a `Vec<String>`, a `drop` of its own) still recurses on destruction.
- 001: the runtime's locking stays on in a program with entry points (left for the owner's review).

## Checked

- The case under GCC and Clang, default, `--all-checks` (200,000 levels under Clang) and `--bootstrap-form`; ASan on
  200,000 levels; threads (300,000 levels); chapters 16, 07 and 14; `cbtest cases`; the pregate checks and the
  gate's run, named in the commit's message.
