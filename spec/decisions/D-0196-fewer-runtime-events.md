# D-0196 — Fewer runtime events per value: objectless `String`s by binding, builders, raw returns, `find_str`, lazy statement scopes

Status: ACCEPTED (2026-10-09; the owner: "proceed with 1,2 and 3 using your recommendations", "carry on with item 1")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0192, D-0193; Update 75's objectless `String` locals
Affects: `impl/cobc/src/lower.rs`; `impl/cbrt/src/lib.rs`; `spec/conformance.md`; `CHG-0224`

## Problem

Round 8's main finding: compiled programs that handle many `String`s ran 10–80× slower than Python. Callgrind showed
about 500 runtime calls per CSV field in `csvstat`, most from the program's own code. Timing single operations
showed where the cost was:

- **A local that is only read:** binding `String::clone(&base)` and reading it took 635 ns, against 3 ns for the
  loop itself. The objectless analysis (no runtime object for a `String` local that is only read) worked by name
  and required the name to be declared once in the whole function, so any repeated `String s` lost it.
- **A function that builds and returns a `String`:** 1,768 ns. Its local was lent to `append` and moved out,
  neither of which the analysis allowed. The returned value then got an object only for the caller to forget it.
- **A lookup by a `str` key:** `HashMap::get_str` took 1.3 µs, because `find_str` was the only probe without a
  native form.

## Decisions (no rule changes; every check is made as before)

1. **Objectless by binding.** Each `let` with an initializer is a candidate of its own, and a use resolves to the
   innermost binding of its name.
2. **Builders.** `&mut s` handed to one of `std`'s `String` builders (`append`, `push_ascii`, `push_code_point`,
   `clear`, `truncate`), each of which keeps no reference, leaves `s` objectless; the builder gets its address
   with token 0.
3. **Moving out.** `return s`, or the function's last expression `s`, moves an objectless `String` out. Control
   leaves at that point, so nothing can use `s` after it. The value takes an object there, and the binding is left
   an empty `String`, so its drop frees nothing.
4. **Raw returns.** A non-generic function of the program returning a `String`, whose every result is such a
   local, also gets a `__raw` form that returns the bytes with no object. A caller binding the result to an
   objectless local calls that form.
5. **`HashMap::find_str` is native**, as `find` is, so `get_str`, `get_mut_str`, `contains_str` and `remove_str`
   use it.
6. **Lazy statement scopes (cbrt).** A pushed scope stays pending in thread state until the runtime next looks at
   it, and a pending one pops for nothing. No lock is taken for statements that record nothing.

## Results

| Operation (stress/d0196/ops) | Before | After |
|---|---|---|
| `String s = String::clone(&base);` then a read | 635 ns | 104 ns |
| A function that builds a `String` with `append` and returns it | 1,768 ns | 314 ns |
| `HashMap::get_str(&m, "k")` | 1,335 ns | 815 ns |

The round-8 programs are not faster (csvstat, extsort, wordpar, dijkstra, eventsim within noise of before). Their
time goes to `String`s stored in `Vec`s and maps, element references and views, not to locals. Removing that cost
means values inside containers needing no runtime object: a change to the runtime's model, not a further local
analysis, and the main open performance question.

Also fixed on the way: `cb_borrow_range` and `cb_borrow_unstamped` accept the unchecked token 0, as `cb_borrow`
and the checks already did. `String::truncate` on an objectless `String` faulted `stale-binding` without that.

## Not decided here

Reading a field of an element of a `Vec` that is not confined still establishes an element object (about 400 ns).
A cheaper read would need the place lowering to tell field reads from borrows of fields. The object protocol for
`String`s passed by value into functions is unchanged (671 ns).
