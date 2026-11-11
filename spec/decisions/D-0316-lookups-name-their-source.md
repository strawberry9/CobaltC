# D-0316 — `HashMap::get` and `get_mut` name their result's source, and a lookup's probe receives nothing

Status: ACCEPTED (2026-11-10, the owner, on the agent's report that the allocation plan's `lookup_ref` needed a change
to `spec/21`'s signatures: "prefer the better strategic decisions/answers. Don't just go for easy/fast options if
strategic decisions make more sense. Remember that CobaltC has no users currently.", then "that's fine. keep going.");
implemented by `CHG-0425`
Kind: Decision (`spec/02-schema.md`); two `std` signatures, and the checker brought to `spec/10`'s text
Governed by: `CobaltC_Master_Instructions.md` §23
Depends on: D-0041 (hash tables), D-0296 (both toolchains in step), D-0312 (the `from` clause,
`[Result-Source-Probe-Only]`, `probe<K>`)
Affects: `spec/21` §2g (the listings of `HashMap::get` and `get_mut`; "Lookups take the key's probe"), `impl/std/collections.cb`,
`impl/src/typecheck.rs` and `impl/selfhost/front/typecheck.cb` (`can_receive`), `impl/selfhost/back/state.cb`
(`lookup_ref`) and the read-only callers of `lookup`; `spec/conformance.md` (two rows); `impl/selfhost/front/prelude.cb`

## 1. What was missing

The allocation plan (`CHG-0421` to `CHG-0424`) left one item undone: `cbc` copies a `Binding` out of its scope table
to answer every question about a name, because no function could return the binding where it lies. Such a function
takes the table and the name, so it has two reference parameters and needs a `from` clause naming the table; the name
must then be only *probed* (`[Result-Source-Probe-Only]`, `spec/10`). It is passed to `HashMap::get`, and two things made
that a *keep*:

1. `HashMap::get` returns a reference into the map's values and nothing else, but its signature had no `from` clause,
   so `spec/10`'s probe list did not cover an argument of it (a call "whose own `from` clause names another
   parameter").
2. Both checkers counted a parameter of a type they do not open (`probe<K>`, a `Vec<T>` by value) as able to *receive*
   a reference whenever an argument of the type may hold one. `spec/10` says otherwise: a parameter can receive a
   reference through an exclusive reference, a raw pointer, a `mutex`, a guard, a `Channel`, an `AtomicPtr`, or a
   `fn` value or closure. A `probe<K>` is passed by value and stores nothing. So every call of a `std` lookup
   (`HashMap::contains(stop, w)`, `get`, `HashSet::contains`) made all of its arguments kept, in any function with a
   `from` clause: a stricter rule than the specification's, found here.

## 2. Decision

1. **`HashMap::get` and `HashMap::get_mut` are declared `from m`.** Their bodies already meet
   `[Result-Source-Probe-Only]`: the probe reaches only `index_of`, which returns a `usize`. `entry` is not given the
   clause: it stores `fresh` and a copy of the key, so the checker refuses it, as the rule says it should.
2. **A parameter of a type the checker does not open can receive a reference when one of its type arguments can**,
   not when one of them may hold a reference. This is `spec/10`'s text, now what both checkers do. No rule text
   changes.
3. **Not breaking.** Both changes only accept more programs: the clause names the parameter
   `[Call-Elided-Lifetime]` already took the result from (the map), and
   the checker refuses nothing it refused only correctly before. No program's meaning changes. CobaltC has no users
   to migrate.
4. **Then the plan's last item:** `cbc`'s `lookup_ref(x, name) : Option<ref<Binding, shared>> from x`, used by the
   `lookup` callers that only read the binding.

## 3. Not decided, and why

- A `from` clause on the rest of `std`: no other exported function returns a reference with a probe or a second
  reference parameter and no clause (checked over `impl/std`).
- A result that may come from either of two inputs stays inexpressible (D-0312 §3).
