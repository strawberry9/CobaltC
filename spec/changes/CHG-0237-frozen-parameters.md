# CHG-0237 — `cobc`: reads through a frozen shared parameter need no runtime entry

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-10; D-0205 (1))
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0196, D-0200, D-0205
Affects: `impl/cobc/src/lower.rs`, `impl/cbrt/src/lib.rs` (`cb_frozen_ok`, `cb_frame_hold` of no path),
`impl/cbrt/include/cbrt.h`, `spec/conformance.md` 3.199.0

## What changed

- **Two bodies, chosen at entry.** A function with frozen parameters -- shared references it never writes,
  borrows or captures (`unstored_params`), its other parameters holding no reference but shared ones -- is
  compiled twice: `NAME__frz`, where reads through a frozen parameter need no runtime entry, and `NAME__chk`, as
  before. `NAME` calls `cb_frozen_ok` for each frozen parameter (its path valid, not formed under a lock, no valid
  exclusive path to its object overlapping it; for a `Vec`, no object over its buffer -- an element held by
  reference) and runs the frozen body when all pass, the checked one otherwise, which faults where and as it
  always did. A token of 0 (a path its caller already vouched for) passes. While the call runs no exclusive path
  to what the parameters reach can be formed: not by the function, and not by another thread (it would clash
  with the held paths the parameters were formed through). The `__raw` form (D-0196) is built the same way; a
  frozen body longer than 3 000 lines of C is not kept.
- **In a frozen body** (`Fx::frozen_vec`, `Fx::frozen_root`): an element of a frozen `ref<Vec<T>, shared>`, read
  as plain data or handed to a reader parameter, is its bounds check and its address; `&x.f…` (fields only) is
  handed to a reader parameter with no path; `String::as_view(&x.f…)` to a pure view parameter is a bare view; a
  plain field is read with no check. Pathless hand-offs only of values that keep no buffer an element object could
  lie over: plain data, `String`, and structs and enums of those (`flat_value`).
- **The analyses** (`reader_uses`, `view_uses`, `pure_view`): `String::clone` is a reader; `String::from_view` of
  a view chain over `r` (`as_view` of `r` or `&r.f…`, narrowed by `StringView::sub`/`trim*`); `as_view(&r.f…)`
  handed on; `match (r)` with its binders used as `r` may be; a comparison of a view; a result holding no
  reference (a `String`).
- **An older hole closed.** A `Vec` passed as `&v` to a shared confining parameter (`fresh_shared_vec_arg`, the
  `_c` variants) was read unchecked on the premise that the shared borrow made for the call meets every exclusive
  path to the vector or an element. It does not meet one to an element, which lives on the element's own object:
  `held.push(&mut v[2]); total(&v)` read element 2 without the `[Read-Conflict]` fault `coby` reports. The call
  now tests the buffer (`cb_live_in`) and takes the checked variant where an object lies over it
  (`conf.frozen-param-held-element`). Present since D-0200.
- **`cbrt`**: `cb_frozen_ok`; `cb_frame_hold(0)` returns at once.
- **Conformance:** `conf.frozen-param-held-element`, `conf.frozen-param-released`, `conf.frozen-param-held-field`,
  `conf.frozen-param-threads`.

## What did not change

No rule, diagnostic or output. A first, unsound version (frozen reads without the entry test) was caught by
`conf.vec-index-of-held-element` and `conf.vec-clone-held-element` before it left the working tree.
