# D-0121 — `read_volatile` and `write_volatile`

Status: ACCEPTED (2026-09-30, the owner: "proceed with 1, 2, 3, 5, 6 and 7 as D-0121 onwards" — item 1 of the assistant's list of worthwhile additions; the design the owner was shown when asking about `volatile`)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §5, §9
Depends on: rule.trust.rawptr (`[Rawptr-Read]`, `[Rawptr-Write]`), rule.trust.unsafe, D-0118 (a `bitstruct` names a register's fields)
Affects: `spec/20` §2 (1.11.0), `spec/21` §0, `spec/registry/diagnostics.md`, `spec/conformance.md`, the guide §20, both tools

## Problem

Every raw access CobaltC had was inside its abstract machine: a read
gives what the program last stored, so an implementation may merge
two reads of one address, as C's compilers do. A device register
changes on its own, and a read of it may have an effect; the only way
to touch one was C code behind `extern fn`. With `bitstruct` and the
byte functions in, this was the one thing between CobaltC and a
driver.

## Candidate mechanisms

1. **Nothing**: device access stays in C. Keeps the machine closed.
2. **Two intrinsics**, `read_volatile<T>(rawptr<T>) : T` and
   `write_volatile<T>(rawptr<T>, T)`, each an observable event that
   an implementation performs exactly once, in order, never merged,
   elided or reordered across another volatile access or a call
   (Rust's model). Selected: two names, no token, one rule pair beside
   `[Rawptr-Read]`.
3. **A `volatile` qualifier on types**, as C has. Spreads through the
   type system; most of its C uses are ones CobaltC already removed.

## Selected design

`[Rawptr-Read-Volatile]`, `[Rawptr-Write-Volatile]` (`spec/20`): as the
plain raw rules, plus the event property; `T` not a resource (a
resource crosses raw memory by `*p`, which moves it); inside `unsafe`,
as every raw access. `cobc` lowers to an access through a `volatile`
lvalue; `coby` performs the access — it has no devices, and the
property concerns what an implementation may not do. Not for threads:
the language's threads share through a mutex or a channel.

## Compatibility impact

Additive; two new intrinsic names (a root item named so is keyed apart, D-0055).

## Revisit conditions

Memory fences or atomics, if a real target needs them, as their own decision.
