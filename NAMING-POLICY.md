# CobaltC naming and conformance policy

Copyright © 2026 strawberry9 (the "Author").

Anyone may implement CobaltC (see [LICENSE-IMPLEMENTATION](LICENSE-IMPLEMENTATION)).
This policy says when an implementation may use the name. Its purpose
is that "CobaltC" always means the language the Author's specification
defines, so that a program written for one CobaltC compiler runs the
same way on another.

## The name

"CobaltC" names the programming language defined by the specification
the Author publishes (the `spec/` directory of the CobaltC repository),
and the Author's reference implementations, `coby` and `cobc`. The
Author reserves all rights in the name and the logo not granted here.

## Conforming implementations

An implementation is **conforming** to an edition of the
specification when:

1. it accepts, rejects and runs programs as that edition requires; in
   particular, it passes that edition's conformance suite (the rows of
   `spec/conformance.md` and the cases in `impl/conformance/` as
   published with that edition), except
   cases that need a capability the specification lets an
   implementation lack and that the implementation's documentation says
   it lacks (for example, calling C code); and
2. its documentation names the edition it conforms to (the
   specification's edition, recorded in `spec/README.md` as
   `YYYY.MMDDnn`: its publication date, UTC, and a two-digit counter for
   that day), and lists any capabilities it lacks as in 1.

A conforming implementation may call itself "a CobaltC compiler" (or
interpreter, and so on) and say that it "conforms to CobaltC 2026.100301"
(naming its edition).

## Extensions

A conforming implementation may offer extensions only if every
conforming program behaves exactly as the specification requires with
them present, and a program that uses an extension is either rejected by
default or reported as using one. Its documentation must describe each
extension as not part of CobaltC.

## Other implementations

An implementation that is not conforming, including a partial or
experimental one, may describe itself as "based on CobaltC", "a partial
implementation of CobaltC" or "a subset of CobaltC", but must not call
itself a CobaltC compiler or claim to conform.

## Not permitted

- Calling a different language, a modified specification, or a
  language that deliberately departs from the specification "CobaltC".
- Suggesting that the Author endorses, certifies or maintains an
  implementation, unless the Author has said so in writing.
- Using the CobaltC logo (for example `images/`) for an implementation
  without the Author's written permission.

## Questions

Ask the Author through the CobaltC repository.
