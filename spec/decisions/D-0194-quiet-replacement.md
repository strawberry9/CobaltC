# D-0194 — Quiet types: `=` replaces a value whose end only frees memory

Status: ACCEPTED (2026-10-09; the owner: "proceed with 1,2 and 3 using your recommendations" — item 2, the `overwrite` friction)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §6, §17
Depends on: D-0033, D-0049, D-0080, D-0095
Affects: `spec/05` (1.5.0), `spec/11`, `spec/examples.md`, `spec/registry/diagnostics.md` (1.49.0); `impl/src/typecheck.rs`, `impl/src/interp.rs`; `impl/cobc/src/lower.rs`; `impl/cbrt/`; `spec/conformance.md` (3.187.0); the guide; `CHG-0222`

## Problem

`[Write-Resource-Overwrite-Rejected]` makes every assignment over a live resource an error, so that a resource's
end is always visible where it happens (D-0080's `overwrite(&mut x, v)` is the repair). In round 8 it was the most
frequent friction: `s = sprintf(…)`, `r.name = String::clone(v)`, `c.least = Some(…)`, `t.cols = cells` each needed
`overwrite`. For a `String`, whose end runs nothing of the program's and releases nothing outside it, the explicit
form shows nothing a reader needs to see.

## Options

1. Keep D-0080 as it is. Safe, but the friction stays.
2. Let `=` replace any live resource, destroying the old value (as Rust does). The friction goes, but a `File`
   closing, a `handle` joining, a `guard` unlocking or a program's own destructor would happen unseen.
3. **Let `=` replace a live value only when its type is *quiet*: destroying any value of it only frees memory.**
   Everything with an observable end keeps the rule.

## Decision

Option 3.

- **Quiet types:** `quiet(τ)` holds for plain data, and for `std`'s memory types `String`, `Vec`, `HashMap`,
  `HashSet`, `Queue`, `Box` and `Json` over quiet types. It also holds for `array`, `Option` and `Result` of quiet
  types, and for a struct or enum that is not declared `resource`, has no destructor, and has only quiet fields or
  payloads.
- **Never quiet:** references, slices, `fn` values (a closure may own anything), handles, mutexes, guards,
  closures, and `std`'s other resources (`File`, sockets, `Child`, `Channel`, `Rc`, …).
- **The rule:** `[Write-Resource-Overwrite-Rejected]` now requires `¬quiet`. A new rule, `[Write-Quiet-Replace]`,
  covers a write to a place holding a live quiet value: once every other check of `[Write]` passes, the old value
  is destroyed, then the new one is stored.
- **Why this is safe:** destroying a quiet value cannot run program code or fault, so nothing observable happens
  in between, and `overwrite(&mut x, v)` stays meaningful and equivalent.
- **Static refutation (D-0095)** applies only to types that are not quiet.

Also fixed on the way: when such a write faults over a non-quiet resource, `coby` now destroys the value that was
to be written before the unwind, as `[Fault-Unwind]` requires (it is a temporary of the faulting statement; `cobc`
already did).

## Implementation

- **Type checker** (shared by both tools): `typecheck::quiet_ty`. The static refutations skip quiet types.
- **coby:** `write_evaluated` destroys the old quiet value after the write's checks pass.
- **cobc:** `cb_write_quiet(base, path, addr, type)` makes `cb_write`'s check. It destroys the old value in place
  when the target object is initialized and the value owns something. A moved-out binding is re-established, not
  initialized, so its stale bytes are never touched.
