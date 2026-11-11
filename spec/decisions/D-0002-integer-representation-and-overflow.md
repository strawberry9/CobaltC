# D-0002 — Integer Representation and Overflow Disposition

Status: ACCEPTED
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §11, §17 (Arithmetic), §12
Depends on: inv.spatial-validity, term.type

## Problem

CobaltC needs a numeric-type family and a disposition for arithmetic
overflow. Master Instructions §17 calls this out for "extra scrutiny...
where arithmetic feeds allocation, indexing, offsets, extents" — the
classic root cause of memory-safety failures in C-family languages is
silent overflow (wraparound or true UB) reaching an address computation.
§12 forbids an open "undefined behavior" outcome for any safe-program
construct.

## Constraints

- §12: every operation must be `rejected`, `checked`, `fallible`,
  `trusted-unchecked`, or `unsupported` — no silent wrong value.
- §8 priority order: invariant preservation and safety outrank
  performance (11th) and C-family familiarity (12th) — but §5 still
  requires CobaltC to support efficient native execution and predictable
  representation "where requested," so a design that sacrifices
  performance without necessity is also wrong.
- §17 requires the mathematical domain and represented domain to be
  distinguished per type.
- `inv.spatial-validity`/`inv.initialization-validity` already establish
  that a false semantic claim (here: a wrapped or saturated integer
  silently treated as an exact count) must never become trusted (§6).

## Candidate mechanisms

1. **Arbitrary-precision integers as the default numeric type**
   (mathematical domain = represented domain; overflow structurally
   impossible short of memory exhaustion). Eliminates the overflow
   invariant class entirely for default arithmetic. Rejected as the
   *default*: representation is not predictable or native-word-sized,
   working against §5's "predictable data representation where
   requested" and "efficient native execution," for no safety gain over
   option 3 below once addressing-critical arithmetic is checked anyway.
2. **Fixed-width, wrapping (modular) arithmetic by default** (classic
   C/Rust-release-mode/Zig-release behavior). Fast, deterministic,
   predictable representation — but a wrapped value is a plausible,
   silently-wrong value indistinguishable from a correct one at the use
   site, i.e. exactly the "false semantic claim becomes trusted" failure
   §6 asks to be designed out. Rejected as the *default* for
   general-purpose arithmetic; retained as an explicitly-named
   operation family for callers who want modular arithmetic on purpose
   (hashing, checksums, ring-buffer indices).
3. **Fixed-width, checked (trapping) arithmetic by default**: ordinary
   operators (`+`, `-`, `*`, ...) produce a `↛` diagnostic
   (`disposition: checked`) when the mathematical result falls outside
   the represented domain, instead of silently wrapping. Predictable
   native representation preserved; overflow becomes a defined,
   diagnosable fault rather than a silently wrong value; implementable
   efficiently on essentially all target hardware via native
   overflow flags, so the performance cost relative to option 2 is
   small in practice. **Selected as the default disposition.**
4. **Fixed-width, saturating arithmetic by default.** Rejected as
   default for the same reason as option 2: a saturated value is a
   silently-wrong-but-plausible value, arguably worse than wrapping for
   safety-relevant uses since it looks like a legitimate boundary value.
   Retained as an explicitly-named operation family.
5. **Fixed-width, fallible-by-default (every arithmetic op returns a
   result/error pair).** Maximally explicit, but imposes handling
   ceremony on every arithmetic expression in the common case where
   overflow is a genuine program defect, not a recoverable condition —
   conflicts with §9's conceptual-economy priority for the overwhelming
   common case. Retained as an explicitly-named operation family for
   call sites that need to recover from overflow rather than fault.

## Selected design

- **Representation:** fixed-width, two's-complement signed and
  unsigned integer types at widths 8/16/32/64/128 bits
  (`type.i8`...`type.i128`, `type.u8`...`type.u128`), plus
  platform-address-width `type.isize`/`type.usize` reserved specifically
  for extents, offsets, and indices (`inv.spatial-validity`'s primary
  consumers) so address-space width is not conflated with general
  application arithmetic width.
- **Default arithmetic operators** (`+ - * / %` and friends) are
  `disposition: checked`: a result outside the represented domain
  produces diagnostic `diag.arith-overflow` (or `diag.div-by-zero`,
  `diag.div-overflow` for the division family) rather than a value.
- **Explicit named alternative operation families** provide `wrapping`,
  `saturating`, and `fallible` (result-returning) arithmetic for callers
  who need them on purpose. None of these are reachable through the
  default operators — a caller must opt in by name.
- **Literal range-checking is static** (`disposition: rejected`): an
  integer literal's mathematical value must fit its type's represented
  domain at compile time; this is never a runtime concern (§12's
  preference for static rejection wherever the fact is knowable
  without execution).

## Rejected alternatives

Arbitrary-precision default (1); wrapping default (2, retained as an
explicit opt-in); saturating default (4, retained as an explicit
opt-in); fallible default (5, retained as an explicit opt-in).

## Semantic rationale

The failure this decision exists to prevent is a false arithmetic result
silently reaching `inv.spatial-validity`'s `addr-in` check or a length
used for allocation. Checked-by-default closes that path without
discarding predictable native representation, and does so more cheaply
than it might appear because hardware overflow detection is nearly
universal — this is retained as the strongest known technique per §7,
not adopted for C-family familiarity (which this design explicitly
departs from: C's default is wrapping/UB, not checked).

## Usability implications

Ordinary arithmetic reads exactly like arithmetic; the cost is paid only
on overflow, as a diagnosable fault instead of a silent error much
further from its cause.

## Explainability implications

`diag.arith-overflow` can report the exact operation, operand values,
and represented domain violated, satisfying §19's diagnostic-field
requirements directly.

## Implementation-feasibility implications

Checked arithmetic on fixed-width two's-complement integers is
standard, efficiently implementable behavior on essentially all target
architectures (flag-based overflow detection); an implementation-
feasibility analysis (§20) finds no gap requiring an unresolved
mechanism.

## Compatibility impact

None yet — first definition of this area.

## Prior-art status

Checked-by-default with explicit wrapping/saturating/fallible families
is independently justified here by the invariant-preservation argument
above; that other languages have converged on similar designs is
consistent with, not the basis for, this derivation (§7).

## Invariant traceability

Directly serves `inv.spatial-validity` and `inv.initialization-validity`
wherever arithmetic feeds a length, index, or offset; instance of the
general `inv.resource-authority`-adjacent principle that a false claim
must not become trusted (§6).

## Revisit conditions

Revisit if implementation-feasibility analysis later finds a target
class where checked-by-default is not efficiently implementable, or if
the explicit alternative-operation-family naming proves unusable in
practice once surface syntax is designed.
