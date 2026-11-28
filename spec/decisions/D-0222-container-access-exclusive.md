# D-0222 — An exclusive element accessor takes its container exclusively

Status: ACCEPTED (2026-10-12; the owner chose D-0219's option 9(i): "7i, 8i, 9i, 10ii")
Kind: Decision (`spec/02-schema.md`); a soundness repair (`spec/STABILITY.md` §2)
Governed by: `CobaltC_Master_Instructions.md` §8, §17, §21
Depends on: D-0219 (finding 9), D-0218 (what a shared reference reaches is read-only), D-0070 (`[Index-Vec]`)
Affects: `spec/21` (`Vec::index_exclusive`, `Box::get_mut`), `impl/std/collections.cb`, `impl/std/memory.cb`,
`cobc` (native bodies of `Vec::index_exclusive`, `Box::get_mut`, `HashMap::get_mut`, `entry`, `insert`),
`impl/model`, `spec/conformance.md` (CHG-0261)

## Problem

D-0218 and Master Instructions §17 promise that what a shared reference reaches does not change while it lives.
D-0219 finding 9 showed a way around it with no rule broken:

    auto r = &mut w;          // w : Vec<i32>
    auto s = &*r;             // shared, lives on
    set(r, 0, 9);             // *Vec::index_exclusive(v, i) = x, through r passed on whole

Passing `r` on forms no borrow; `Vec::index_exclusive`'s body reads the vector through `v` (a shared access,
permitted beside `s`) and borrows the element, an object of its own, so nothing clashes; `s` then reads 9.
Memory safety and `[Conc-No-Race]` hold — the element's own accesses are checked — but a function given `s` and
`r` sees what its shared parameter reaches change.

Probing the shape across `std` (2026-10-12) narrowed it: `(*v)[i] = x`, `Vec::swap`, `sort`, `reverse`, slices
and every body that writes the vector's own fields already fault, because `[Index-Vec]`'s written form borrows
`&mut *v` and the others write through `v`. Two accessors remain — **`Vec::index_exclusive`** and
**`Box::get_mut`** called with a reference passed on — and one implementation defect of the same kind:
**`cobc`'s native `HashMap::get_mut`, `entry` and `insert`** made only the shared reads through `m`, not the
exclusive borrows of the map's vectors the text makes (`&mut m.values`, `&mut m.slots`, the pushes), so under
`cobc` alone they wrote past a live shared reference to the map (in Update 102 already; `coby` faulted).

## Options

(i) the element-writing bodies take the container exclusively, so a live shared path to it clashes;
(ii) state the limit in `spec/08` and the guide; (iii) a container rule in `clash`. D-0219 gives the trade-offs.

## Decision

(i), at the narrowest place that closes it:

- `Vec::index_exclusive(v, i)`, after its bounds check, accesses element `i` exclusively through `v`:
  `_ = &mut (*v)[i .. i + 1];` — the access `[Index-Vec]` checks for `(*v)[i]` written, so a live shared path
  to the vector, or a slice holding `i`, meets it, and a slice of other elements does not.
- `Box::get_mut(b)` first accesses the box exclusively through `b`: `_ = &mut *b;`.
- `cobc`'s native bodies make the checks the text makes: `cb_borrow_range_unminted` over `[i, i + 1)` in
  `Vec::index_exclusive` (its out-of-line body and the inline form a plain element's access takes),
  `cb_borrow_check` of the box in `Box::get_mut` (and its inline arm form), and of the map: in `entry` and
  `insert`, whose every path borrows one of the map's vectors exclusively and faults where any would, one
  exclusive check of the map at entry in place of `find`'s first read (the fields are private, so no path
  outside `std` reaches one alone); in `get_mut`, a check of `m.values` on the found branch only (an absent key
  borrows nothing exclusively), left out (the `_x` variants) when the call passes `&mut x` of a local, which the
  call already checked exclusively, and a key that forms no path.

## Rationale

A soundness repair by `spec/STABILITY.md` §2. The invariant is D-0218's (a shared reference's reach is
read-only while it lives); the unsafe step is a write through an exclusive element reference obtained while a
shared path to the container lives; the programs newly rejected are exactly those that make that step, each a
call of one of the two accessors with a reference that has a live shared reborrow (and, under `cobc`, the three
map operations, which `coby` and the text already rejected). The check is the one `(*v)[i]` written already
makes, so the explicit call and the syntax now agree; a call whose argument is a borrow formed for it pays
nothing new, because that borrow already made the check.

## Results

- `std` text and `spec/21`: one statement each in `Vec::index_exclusive` and `Box::get_mut`.
- Cases: `conf.vec-index-exclusive-while-container-shared`, `conf.box-get-mut-while-box-shared`,
  `conf.hashmap-get-mut-while-map-shared`, `conf.hashmap-get-mut-local-while-shared-path-kept`, `conf.hashmap-entry-while-map-shared`,
  `conf.hashmap-insert-while-map-shared`.
- The model: an explicit `Vec::index_exclusive` call, whose body takes the element exclusively.
- Cost: user-space instructions against Update 103's `cobc` (`bench/run.sh` and the guide's performance programs, GCC
  and Clang, one run each): `hashcount` +0.28% / +0.22%, `strings` +0.11% / +0.09%, every other benchmark and all
  72 performance-guide rows within ±0.3% (`e09_read` b −0.9%, as in earlier rounds, its I/O). The first form, a
  check of each borrow where the text makes it, cost `hashcount` +13%, `e11_counting` +8% and `strings` +5%; the
  entry check for `entry` and `insert` and `get_mut`'s `_x` variants brought it to the figures above. The `Vec`
  and `Box` checks cost nothing measurable.
