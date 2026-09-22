# D-0033 — Ergonomics from Tier 5: Re-assignment, `b'x'`, Float Exponents, `unwrap_or`

Status: ACCEPTED (2026-09-25, owner-delegated: the owner asked for a decision on each and accepted all)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0008, D-0019, D-0020, D-0025, D-0028, D-0032, rule.init.let, rule.value-object.write, rule.arith.literal
Affects: rule.init.let, rule.value-object.write, rule.control.flow-analysis, rule.arith.literal, rule.stdlib.prelude, spec/22 §1 and §5

## Problem

Writing four command-line tools (`showcase/tier5/`) with the features
of D-0031 and D-0032 found six places where the language made
ordinary code awkward:

1. A binding whose value was moved away or dropped could not be given
   a new one: `cur = Vec::new();` after `Vec<u8> done = cur;` was
   `diag.stale-binding`, so a loop that hands off a buffer needs a
   fresh binding per iteration. `x = f(x)` was accepted statically but
   faulted at run time.
2. There is no way to write a single byte readably: the parsers compare
   against 32, 44, 45 and 101.
3. `&` of a temporary is rejected, so `parse<i32>(&String::from_str("42"))`
   takes two lines.
4. A float literal has no exponent, though `print` writes `1.0e-7`.
5. `never` cannot be written as a return type.
6. Taking a value out of an `Option` or `Result` with a default is a
   four-line `match` every time.

## Constraints

- Every destructor still runs, and no value is overwritten while it
  owns a resource (D-0008, `[Write-Resource-Overwrite-Rejected]`).
- No character type (D-0020's considered absence).
- As few new tokens as possible.
- No program accepted today changes meaning.

## Selected design, one choice per item

- **(1) Allowed.** `x = e` for a whole binding whose value was moved
  away or ended — before `e`, or by `e` itself as in `x = f(x)` —
  gives `x` a new object, bound in the frame that declared `x`, as
  `[Let-Uninit]` and a first write would (`spec/11`
  `[Assign-Reestablish]`). Overwriting a binding that still holds a
  live resource stays `diag.overwrite-of-live-resource` (static where
  the analysis sees it, dynamic where it cannot tell). The flow
  analysis sets `valid(x) := T` after a whole-binding write.
- **(2) A byte literal, not a character type:** `b'x'`, an integer
  literal of type `u8` in every context, spelled like one byte of a
  `byte-literal` (ASCII, or `\n \r \t \0 \\ \' \" \xHH`). `b''`, two
  bytes, and a non-ASCII character are lexically ill-formed.
- **(3) No change.** `[Ref-Form-Temporary]` is what keeps a reference
  from outliving the temporary it points into; binding first is one
  line.
- **(4) Exponents:** `float-literal ::= digit+ ('.' digit+ exponent? |
  exponent)`, `exponent ::= ('e' | 'E') ('+' | '-')? digit+` — the
  grammar `parse<f64>` accepts, without the sign, `inf` or `NaN`. A
  float literal's value is its decimal value rounded to its type, and
  one that rounds beyond the type's range is
  `diag.literal-out-of-range` (it was silently infinite).
- **(5) No change for now.** `: never` is needed only by a function
  that never returns, which a program today can write only by
  faulting on purpose. Decide it with `std::exit` or a user-visible
  `fault`, whose signatures need it.
- **(6) `Option::unwrap_or<T>(Option<T> o, T d) : T` and
  `Result::unwrap_or<T, E>(Result<T, E> r, T d) : T`**, ordinary
  CobaltC in `std`. `d` is evaluated either way; an unused `d`, or an
  `Err` payload, is destroyed at the call's end.

## Rejected alternatives

- **(1) Keep the rule:** a hand-off loop needs a fresh binding per
  iteration, and `x = f(x)`, which the static rules accept, faults.
- **(1) Destroy a live old value on assignment:** a resource would be
  destroyed silently by an `=`; D-0008's "destroyed where you can see
  it" is why `[Write-Resource-Overwrite-Rejected]` exists.
- **(2) A character type:** code points and encodings, which D-0020
  deliberately left out; parsing works on bytes.
- **(2) Only document `b","[0]`:** readable enough in one place, not
  in a parser full of them.
- **(3) Allow `&` of a temporary:** `[Ref-Form-Temporary]` is a
  temporal-validity rule, not a convenience.
- **(6) `map`, `and_then`, `ok`, …:** not needed by any program yet.

## Semantic rationale

(1) is `[Let-Uninit]` followed by `[Write]`, reached from an
assignment instead of a declaration: no new state, and every
reference to the old value was already invalid. (2) and (4) change
only the lexical grammar and `val(L)`. (6) is library code.

## Usability

    Vec<u8> cur = Vec::new();
    while (i < n)
    {
        if (*Vec::index_shared(b, i) == b',')
        {
            Vec::push(&mut cells, cur);       // cur's value moves away
            cur = Vec::new();                 // and cur takes a new one
        }
        …
    }

    String a = Result::unwrap_or(arg(0), String::new());
    f64 tiny = 1.0e-9;

## Explainability

"After a value moves away you may assign the variable again";
"`b'x'` is the byte of `x`"; "float literals take an exponent";
"`unwrap_or` gives the value or a default" — one sentence each.

## Implementation-feasibility

(1): `coby` binds a new object in the declaring frame (each frame now
records its bindings' types); `cobc` records the frame of every
binding the function assigns to as a whole (`cb_frame_here`) and calls
`cb_rebind` before the write, which returns the old root while its
object is alive. (2), (4): the shared lexer. (4) also fixed `coby`
leaving an unsuffixed literal typed `f32` by its context unrounded.
(6): `std` source.

## Compatibility impact

Extension. (1) accepts programs that were rejected
(`conf.reassign-after-drop-rejected` becomes
`conf.reassign-after-drop-ok`) and makes `x = f(x)` run. (4) rejects a
float literal that rounds to infinity, which no repository program
has. `b'` could not begin a token before.

## Prior-art status

- **Rust:** re-assigning a moved-from binding is allowed; `b'x'` is a
  `u8`; `1e5` is a float; `Option::unwrap_or`, `Result::unwrap_or`.
- **Zig:** character literals `'x'` are `comptime_int`; exponents.
- **Go:** rune literals `'x'`; exponents.
- **C:** `'x'` is an `int`; exponents.

## Invariant traceability

`inv.temporal-validity`: (1) makes a binding valid again only by
establishing a new object; references to the old one stay invalid.
`inv.resource-authority`: (1) never overwrites a live resource.
`inv.arith.range-validity`: (4)'s range check.

## Revisit conditions

- (5) with `std::exit` or a user-visible `fault`.
- (6) when programs need `map`/`and_then`-style helpers.
- A need for non-ASCII character constants.
