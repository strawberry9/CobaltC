# D-0228 — Diagnostics are short by default and located to the column

Status: ACCEPTED (2026-10-14; Q8 of the 2026-10-12 plan, recommended form accepted by the owner; the details
below settled under the owner's delegation for Phase 3)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §19
Depends on: D-0206 (the executable reference checker), CHG-0082 (syntax errors as diagnostics)
Affects: `spec/registry/diagnostics.md` (1.53.0) (CHG-0271)

## Decision

Both tools show each diagnostic in one form, byte for byte the same.

```
error[diag.type-mismatch]: the operands are `i32` and `bool`; convert one (`widen`, `narrow`, `to_float`)
 --> m.cb:4:13
  |
4 |     i32 x = 1 + b;
  |             ^
  = repair: convert explicitly (`rule.arith.convert`) or fix the types
```

1. **The first line.**
   - It is `error[diag.x]: message` for a static diagnostic and `fault[diag.x]: message` for a dynamic one.
     The phase is the first word, so the first line carries the id and the phase that a conformance runner
     compares.
   - The message is the occurrence's own: the rule's message, or `assert`'s or `static_assert`'s text. When
     there is none, it is the entry's new **Summary** field. Every registry entry has a Summary.
2. **The location.**
   - It is `--> file:line:column`. The column counts characters from 1.
   - The source line follows, read from the file when the diagnostic is shown, with carets under the token.
     An identifier, keyword or number is underlined whole, a text literal to its closing quote, anything
     else one character. A file that cannot be read (`std`'s, or a compiled program run where its source
     is not) gives the location alone.
3. **The repair.** `= repair:` and the entry's Repair close the short form.
4. **`--explain`.**
   - In both tools, `--explain`, or `COBALTC_EXPLAIN` set to anything but empty or `0`, adds the entry's
     Rule, Invariant, Required, Observed and Provenance, then the Related examples.
   - A compiled program is asked through the environment variable, which `cobc --explain --run` sets.
5. **Which column.**
   - A **static** diagnostic is located at the first token of the expression the rule's premises were
     evaluated against, or at the token the registry names. Both tools share the front end, so they agree
     by construction.
   - A declaration or statement is located at its first token. Where the front end records no column for
     it, the first token on its line is used.
   - A **dynamic** diagnostic is located by its line, and its caret goes under the line's first token.

### Why dynamic diagnostics have no finer column

The two implementations agree on the line of every fault, and the compiled suites hold them to it. Within
a statement, though, they check some operations at different sub-expressions. In `a.sum += s[i]`, `coby`
reports an aliasing conflict when the write completes, and `cobc` when it reads `s[i]`. Over the
conformance cases, 15 dynamic faults had different columns, though both tools named the same rule and the
same line. Making every such check agree would mean mirroring the interpreter's order of checks in each
of the compiler's specialised lowerings, and the differential generator's programs would keep finding new
differences.

Reporting by line keeps the two tools identical. The generated C and the runtime needed no change. The
column stays in the AST (`Expr::col`, `Stmt::Let`'s `col`), so this can be revisited.

**Revisit** when the lowering gives every runtime check an expression of its own (WI-15's split of
`lower.rs` would be the time), or when users ask for the column of a fault.

## Alternatives considered

- **Keep the full block by default, and add `--brief`.** Declined in Q8: the first reading of an error
  should be short.
- **`error[diag.x] (dynamic): …` on the first line.** Declined: `fault` is the specification's word for a
  dynamic diagnostic, and it keeps the line in the familiar `error[...]` shape.
- **Columns for dynamic faults from mirrored check order.** Tried and set aside, for the reason above.
