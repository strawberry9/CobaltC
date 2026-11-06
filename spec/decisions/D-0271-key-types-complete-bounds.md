# D-0271 — Key types complete the bound system

Status: ACCEPTED (2026-10-19, the owner: "proceed autonomously using your recommendations", on the proposal
of the same day; the proposal's option 1)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §8, §9, §12
Depends on: D-0090 (bounds), D-0108 (`ordered` admits text and `bool`), D-0110 (structs and enums of key
types are key types), D-0116 (`clone` by derivation), D-0218 (`clone` reads through a shared reference)
Affects: `spec/06` `rule.arith.cmp` (`[Cmp-Key]`); `spec/12` `rule.type.bound`; `spec/22` §5 restriction 5;
`spec/21` `rule.stdlib.sort` (a note); `spec/conformance.md`; `impl/src/typecheck.rs`,
`impl/src/interp.rs`, `impl/src/interp/borrows.rs`; `impl/cobc/src/lower/calls.rs`, `equality.rs`,
`operators.rs`; the guide's generics chapter and §21 table; the book's chapter 11; `CHG-0344`

## Problem

D-0110 gave every struct or enum of key types an equality and a total order: `HashSet<Point>` finds equal
keys, `Vec::sort<Point>` orders field by field, `PriorityQueue::new<Point>` pops the least. But that order
and that equality were reachable only through the `std` functions the front end checks by name
(`is_key_type` at `Vec::sort`, `binary_search`, `PriorityQueue::new`, the hash tables). The bound system did
not know them. Measured on 2026-10-19 with the tools of Update 128, for `struct Point { i32 x; i32 y; }`
and `enum Lvl { Low, Mid, High }`:

| Written | Before this decision |
|---|---|
| `HashSet<Point>`, `Vec::sort(&mut v)` on `Vec<Point>` | accepted (D-0110) |
| `p == q` | accepted (`[Eq-Struct]`) |
| `Vec::contains(&v, &p)`, `Vec::index_of`, `Vec::dedup` | rejected: `T` must be `eq` |
| `p < q`; `Lvl::Low < Lvl::High` | rejected, though `Vec::sort` orders both |
| `fn biggest<T: ordered>(…)` at `T := Point` | rejected |
| `==` on a struct holding a `String` | rejected as a read of a resource, though a set compares it |

A program could sort a `Vec<Point>` but not ask whether one `Point` is less than another, put a `Point` in
a set but not ask a `Vec<Point>` whether it holds one, and could not write `Vec::sort`'s own signature for
its own `max`, `min`, `clamp`, `dedup` or `is_sorted`. `spec/22` §5 recorded the state as restriction 5, and
`spec/12` justified it by "`==` and `<` on a user type would be operator overloading", a sentence written
before D-0110 made the order the language's.

## Options

1. **Key types satisfy `eq` and `ordered`, and the comparison operators apply to every key type, by
   D-0110's order.** Selected. No syntax; the derivation already exists; concrete and generic code agree.
2. **The bounds only**, `<` still rejected in concrete code: incoherent with `[T-Bound-Op]`, which checks
   each instantiation at its concrete type, so `v[best] < v[i]` at `Point` must be legal there.
3. **Keep the closed set** and document the comparator (`sort_by`, a `less` function): the state before.
4. **A user-written order**, `fn N::less` recognised by name as `N::clone` is: the operator overloading
   `spec/12` declines, and two orders for one type (`Vec::sort`'s bytes against `<`) unless `[Sort]` also
   followed it, which would put a hash table's key equality under user code. Declined.

## Decision

- `rule.type.bound`: `eq` and `ordered` admit every key type (an integer, `bool`, `str`, `String`, or a
  struct or enum whose fields or payloads are all key types). `number`, `integer`, `clone` and `plain`
  are unchanged, as is the implication chain.
- `rule.arith.cmp` gains `[Cmp-Key]`: `<` on a struct or enum key type is `[Sort]`'s order — a struct field
  by field in declaration order, the first unequal field deciding, each field in its own order; an enum by
  variant index, then payload. `==` stays `[Eq-Struct]`. `<=`, `>`, `>=`, `!=` follow as before.
- A comparison operand of a key type that holds a `String` is borrowed shared, as a `String` operand is
  (D-0108): nothing is moved, and `==` on such a type becomes legal, by the text's bytes, which is what
  the hash tables already did.
- One order: for a key type, `a < b` holds exactly when `Vec::sort` puts `a` before `b`, and `a == b`
  exactly when a hash table treats them as one key. `Vec::sort`, `binary_search`, `PriorityQueue::new`,
  `HashMap`, `<` and a generic body at `T := Point` cannot disagree.
- Floats stay out: a struct with a float field is not a key type, satisfies neither bound and has no `<`;
  the rejection names the field. `Vec::sort<f64>` keeps its top-level exception (D-0181).
- A struct or enum declared `resource` is not a key type (a clarification of D-0110, which was silent; no
  program or case used one as a key): its identity is its own, and `[Read-Resource-Rejected]` keeps
  `==` off it (`conf.read-of-resource-via-comparison`), as before this decision.
- Not included: bounds on a struct's or enum's own type parameters (D-0090); a user-written order (option
  4); keys through `Rc`, `Box`, `Vec`, arrays or references (D-0110's revisit conditions).

## Compatibility impact

Additive. Every program accepted before is accepted with the same result: the change admits what was
rejected and alters no accepted comparison. One conformance case changed its example: `Option<i32>` is a key
type and so now `ordered` (`None` after `Some`, by variant order), so `conf.bound-unsatisfied-rejected` and
`conf.bound-unsatisfied` use `Option<f64>`; `conf.vec-contains-not-eq-rejected` uses a struct with an `f64`
field (a struct of integers is now `eq`).

## Results

- `coby`: a `KeyCmp` marker set by the static pass on a comparison whose operand type is a struct or enum
  key type (an ordering, or `==`/`!=` on one holding a `String`); evaluated by `key_cmp`, the order
  `Vec::sort` already used, with the operands read where they stand. The marker is decided from whichever
  operand's type is known, so `lo < Lvl::Mid` (a bare variant, typed later) is marked too.
- `cobc`: the same marker lowers to the generated `cb_kc_…` (D-0110) or `cb_eq_…` function on the operands'
  addresses, a resource operand borrowed shared as a compared `String` is, plain data read as a value; `<`
  on a key type in `lower_binary` likewise. `cb_eq_…` compares a `String` part by its bytes (it had compared
  the buffer struct, unreachable before this decision).
- The type checker's bound test (`bound_holds`) admits key types to `eq` and `ordered` at instantiations,
  in `N::clone`'s bounds and at the operators; messages name "a struct or enum of those" and the field
  that disqualifies a type.
- Conformance: `conf.cmp-struct-key`, `conf.cmp-enum-key`, `conf.bound-ordered-struct-key`,
  `conf.vec-contains-struct-key`, `conf.eq-string-field-struct`, `conf.sort-order-is-lt`,
  `conf.cmp-float-field-rejected`, `conf.bound-ordered-float-field-rejected`; each under `coby`, `cobc`
  with gcc and clang, and `--check`.

## Revisit conditions

Bounds on a struct's or enum's own type parameters, if generic containers written outside `std` want to
state "`T` a key type" at the declaration rather than at every call; a user-written order (option 4), if a
program needs a struct ordered otherwise than by its fields and `sort_by` is found wanting.
