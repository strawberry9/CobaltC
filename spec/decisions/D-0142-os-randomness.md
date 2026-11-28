# D-0142 — Operating-system randomness in `std::random`

Status: ACCEPTED (2026-10-04, the owner, through `private/std-deployable-plan.md` §5 and §10.3: "`os_random_bytes` faults rather than returning a `Result` when the OS has no randomness")
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0120 (`Rng`), D-0134 (`std`'s naming conventions), D-0136 (`std::random`)
Affects: `spec/21` §0 (4.8.0), `spec/registry/diagnostics.md` (`diag.entropy-unavailable`), `spec/conformance.md` (3.132.0), the guide §21, `impl/std/random.cb`, `impl/std/std.cb`, `impl/src/osrand.rs` (new), `impl/src/interp.rs`, `impl/cbrt`, `impl/cobc/src/lower.rs`

## Problem

`Rng` (D-0120) is reproducible by design and says it is not for
cryptography; its seed had to come from somewhere, and the only
candidate in `std` was the clock — predictable, and the same for two
programs started in one second. A deployed program needs unpredictable
bytes for session tokens, keys, nonces and temporary names, and a seed
that differs between runs.

## Candidate mechanisms

1. **Seed from the clocks.** Predictable; never for secrets. Rejected.
2. **A cryptographic generator written in CobaltC** (ChaCha20) seeded
   once from the system. More code to get right, and still needs the
   system's bytes once. Not needed: the system's source is fast enough.
3. **The system's secure source, directly: `os_random_bytes`,
   `os_random_u64`, and `Rng::from_os` for a seed.** Selected.

## Selected design

`spec/21` §0, `[Os-Random]`, `[Rng-From-Os]`:

- `os_random_bytes(slice<u8, exclusive> out)` fills `out`;
  `os_random_u64()` is eight such bytes, little-endian;
  `Rng::from_os()` is `Rng::new(os_random_u64())`.
- One std-private primitive, `extern fn os_random(rawptr<u8> buf,
  usize n) : isize` (`impl/src/osrand.rs`, both tools): Linux
  `getrandom(2)` by its system-call number (so an old C library does not
  matter), falling back to `/dev/urandom` on a kernel without it; other
  Unix systems `/dev/urandom`; Windows `BCryptGenRandom` with the
  system's preferred generator (`bcrypt` joins `cbrt::LINK_LIBS` on
  Windows). Never a clock.
- **A fault, not a `Result`, when the system has none**
  (`diag.entropy-unavailable`, disposition `checked`): no system either
  tool runs on lacks a source, a program has nothing sensible to do
  instead (falling back to a clock is the bug this prevents), and every
  caller would otherwise carry an error path that never runs.
- The bytes are **unspecified** (`outcome: unspecified`); conformance
  checks only what holds of every run.
- `Rng::from_os` gives an unpredictable seed, but the sequence is still
  `Rng`'s and still not for cryptography; the guide says to take secret
  bytes from `os_random_bytes` itself.

## Compatibility impact

Additive: three new names in `std`, shadowed by a program's own.

## Revisit conditions

- A fallible form, if a platform without a secure source is ever
  targeted.
- A cryptographic generator in `std` (for speed, or for reproducible
  secure streams), if programs measured need more bytes than the system
  gives quickly.
