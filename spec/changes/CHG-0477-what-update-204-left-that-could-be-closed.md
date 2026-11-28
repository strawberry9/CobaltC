# CHG-0477 — What Update 204 left that could be closed

Status: implemented (2026-11-21); Update 205
Affects: `spec/16` 1.38.0 (§4a), `spec/17` 2.22.0 (an array length), `spec/conformance.md` 3.322.0 (4 rows);
`impl/front` (`derive.cb`, `parser.cb`, `check.cb`, `consteval.cb`), `impl/cobc` (`host.c`), `impl/std/memory.cb`,
`impl/tests/windows`, `.github/workflows/bootstrap.yml`; the guide, `impl/STATUS.md`, `CHG-0476`; D-0334

## Why

The owner asked that no report stay open. D-0334 has what was decided; this record has what changed.

## What changes

| # | Update 204 left | Now |
|---|---|---|
| 022 | the derived `clone` of a tree recursed to its height | loop after 1,024 levels (`derive.cb`: `clone$chain`, `clone$iter`, `clone$down`); a tree two million levels deep clones (`conf.clone-deep-tree`) |
| 013 | an array length could not use a type known only after parsing | the lengths are settled after the items are known (`check.cb` `parse_program_text`, `consteval.cb` `const_usize`); `conf.const-array-length-layout`, `-type-param-rejected`, `-self-size-rejected`; also fixed: `sizeof<String>()` in a length was 0, two modules with a struct of one name, a comma or `>>` in a type argument, the niche of `Option<Box<T>>` in the constant evaluator |
| 037 | the forked child exited 127 silently | it says why (`host.c`) |
| 032, 037 | tested under Wine only | `impl/tests/windows/cleanup.py` passes on GitHub's Windows runner |

## What is left

- 001: the locking stays on in a program with entry points; `--lib` keeps its race (for review later, by the owner's word).
- 022: a type that links to itself only through another type (`Node(Box<Pair>)`, `Pair` holding the link) still
  recurses in `clone`: the derived `clone` is made from one type's declaration, and a loop over the cycle would have
  to make the other type's values too (its fields and their order, in another module or generic at other arguments, or
  its `clone` written by hand). The destruction of a deep chain or tree still recurses where a field declared before
  the link runs a destructor other than `Box`'s and a plain `Vec`'s (a `String`'s): a type's own `drop`, a
  `Vec<String>`'s, a `fn` value's; the link's whole destruction comes before that field's. Closed after this Update:
  in the bootstrap form the destruction of a deep tree that was walked or cloned, and in every form that of a deep
  chain or tree whose values own a `String` (the runtime's waiting list and quiet fields, `conf.drop-deep-owning`).
- 013: `sizeof` of a type parameter, a length that depends on its own holder's size, and a type the constant evaluator
  cannot lay out (an opaque `extern` type, a closure) are refused with a message.

## Checked

- Each case under GCC and Clang, default form, `--all-checks`, bootstrap form where it concerns the run; the pregate
  checks and the gate's run, named in the commit's message.
