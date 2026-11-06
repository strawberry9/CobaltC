# D-0157 — `BigUint`: unsigned integers of any size, in `std::math`

Status: ACCEPTED (2026-10-04, the owner: "proceed with the entire roadmap using your discretion", on the crypto roadmap agreed toward a DNS-over-TLS client)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9 (the std helper admission test)
Depends on: D-0148 (`std::math`), D-0083 (`fault`)
Affects: `spec/21` §0, §3e (new) (4.22.0), `spec/conformance.md` (3.147.0), the guide §21, `impl/std/math.cb`; `CHG-0185`

## Problem

RSA verification raises a 2048- to 4096-bit number to a power modulo
another; such numbers do not fit any integer type. Go programmers also
reach for `math/big` for big factorials, exact money arithmetic and
identifiers. `std` had nothing larger than `u128`.

## Candidate mechanisms

1. **A general `BigUint`** (unsigned, any size) in `std::math`, with the
   arithmetic, comparison, shifts, modular power, decimal text and
   big-endian bytes. Selected.
2. **Fixed-width types** (`u2048`). Every key size would be a type, and
   nothing else would use them. Rejected.
3. **A signed `BigInt` as well.** Nothing in the roadmap needs negative
   numbers; a later record if a program asks.

## Selected design

`spec/21` §3e, `rule.stdlib.bigint`: `BigUint` holds 32-bit limbs, least
significant first, normalized; `zero`, `from_u64`, `from_bytes_be`,
`to_bytes_be`, `to_bytes_be_padded`, `is_zero`, `bit_len`, `bit`, `eq`,
`less`, `add`, `sub` (`diag.arith-overflow` below zero), `mul`,
`div_rem` (Knuth's algorithm D; `diag.div-by-zero`) giving a
`BigDivision`, `rem`, `mod_pow`, `shift_left`, `shift_right`, `text` and
`parse` (decimal). A `BigUint` owns a `Vec`, so it is a resource: a
program reassigns one with `overwrite`, and takes a division apart with
`BigDivision { quotient, remainder } = …`.

**Not constant-time**, stated in the spec and the guide: operations take
time that depends on the values, so `BigUint` is for public values only
(signature verification, never a private key).

Verified by a case against Python's integers and by a sweep
(`stress/crypto/bigint_sweep.py`) of 400 random cases (sizes 0 to 2048
bits, all-ones and top-bit-set values, divisors that exercise Knuth's
add-back step) under all three tools.

## Compatibility impact

Additive: `BigUint`, `BigDivision` and their functions in `std::math`.

## Revisit conditions

- A native realization of `mul` and `div_rem` if `coby` is measured too
  slow for a real program (D-0023's latitude); `cobc` is fast.
- A signed `BigInt`, `gcd` and `mod_inverse`, if programs ask.
