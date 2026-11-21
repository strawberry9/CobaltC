# D-0205 — Round 9: shared reads that scale, and five std additions

Status: ACCEPTED (2026-10-10; the owner: "go with your recommendation on the shared-reads proposal, then go with
your recommendations for round9-options")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §9 (std helper admission), §11, §13
Depends on: D-0196, D-0200, D-0202, D-0204
Affects: `cobc` (CHG-0237), `spec/21` 4.52.0 and `std` (CHG-0238), both tools

## Problem

Round 9's real-world programs (`stress/round9`) found that several threads reading one structure through a shared
reference ran slower than one thread (each derived path lives in, and locks, the owner's heap), and five common
intents that `std` made the program spell out.

## Decision

1. **Shared reads: compile-time discharge (`cobc`, CHG-0237).** A shared reference parameter that the function
   never writes, borrows or captures ("frozen") is tested once at entry (`cb_frozen_ok`): its path valid, no
   exclusive path overlapping it, no element of a vector held by reference. A conflict is checked when an access
   is made, not when a borrow is formed (D-0018), so one could already exist; where the test finds none, none can
   be formed while the call runs, and a frozen body runs in which `cobc`
   - reads a plain field or a plain element with its bounds check alone;
   - hands `&x.f…` and `&x[i]` (fields, and elements of a `Vec`) to a parameter that keeps nothing of them (a reader:
     `reader_param`) without a path, and `String::as_view(&x.f…)` to a pure view parameter likewise.
   The reader and view analyses accept more shapes: `String::clone(&r.f)`, a copy of a view chain over `r`'s text
   (`String::from_view(StringView::sub(String::as_view(&r.f), a, b))`), a `match` on the reference whose arm
   binders are used as `r` may be, comparisons of a view, and functions returning a `String` (anything holding no
   reference). No rule changes: the checks left out are ones that cannot fail. The runtime option (frozen sharing,
   a second kind of path) is not taken.
2. **`Result::ok<T, E>(Result<T, E>) : Option<T>`** (option A(2); `err` not added).
3. **`slice_eq<T: eq>(slice<T, shared>, slice<T, shared>) : bool`** in `std::collections` (B(1)); `==` keeps
   meaning "compare values" and is still a diagnostic on a `Vec`.
4. **`UdpSocket::try_clone`** (C(1)), as `TcpStream::try_clone`.
5. **`StringView::from_utf8(slice<u8, shared>) : Result<StringView, Utf8Error>`** (D(1)), a checked view, no copy.
6. **`Child::kill_tree`** (F(2)): the child and its descendants; `kill` and `terminate` keep their meaning.

Not adopted (no recommendation was made): struct patterns inside patterns (E), moving a field out of an owned
local (G).

## Results

The first version assumed that a held shared parameter excludes every exclusive path to what it reaches; the
compiled suite showed it does not (a reference to an element is on the element's own object), and the entry test
and two bodies replaced it. The same premise had been made by D-0200's confined variants for a `Vec` passed `&v`
(`fresh_shared_vec_arg`): an element held by reference was read unchecked there too. Fixed with a run-time test at
the call (CHG-0237).

`stress/round9/csvquery.cb`, a query over 20 000 rows read through `ref<Vec<Row>, shared>`: one thread 128 ms -> 48
ms; four threads 225-371 ms (slower than one) -> 30-84 ms (faster than one).
