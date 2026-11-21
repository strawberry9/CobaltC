# D-0332 — The bug reports of Update 200

Status: ACCEPTED (2026-11-20). Decided for the owner: an agent testing Update 200 (`e212300`) on another
machine filed 45 reports, and the owner asked for all of them to be fixed in the next push, with one gate run
over the whole set, and left every choice to the implementation. Most reports are plain defects and need no
decision; this record holds the ones that changed a rule, refused programs that were accepted before, or were
found not to be defects. Implemented by `CHG-0475`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0178 (standard output), D-0181 (`[Sort-Float]`), D-0284 and D-0315 (decided programs), D-0290
(frame-held aggregates), CHG-0357 (unbounded instantiation), D-0134 (`remove`)
Affects: `spec/14`, `spec/15`, `spec/16`, `spec/19`, `spec/20`, `spec/21`, `spec/22`, `spec/registry/diagnostics.md`

## 1. Problem

The reports fall into seven groups (`CHG-0475` has each one): programs the static pass decided wrongly, so
that the default form, which keeps no run-time record of references, read freed memory (004, 005, 008, 009,
014, 031); accepted programs that crashed `cobc` or produced C that does not compile (001, 010, 012, 015, 016,
017, 020, 034, 040); tools that overwrite or delete the user's files (024, 025, 026, 027, 037); security (023,
030); places where the checker and the specification disagree (003, 006, 007, 011, 013, 018, 036); checks and
lowerings that take quadratic, exponential or unbounded time (019, 021, 022, 033, 042, 043, 044, 045); and
diagnostics (002, 028, 029, 035, 038, 039, 041).

## 2. Decision

1. **A missed fact is a fact, not a new refusal.** Where the static pass missed what a program's references
   reach (004, 008, 009, 014), the fact is added. Where the rules already refuse such a program the checker now
   refuses it; where only the run can tell, the program is no longer decided and keeps its records, so the
   default form faults as `--all-checks` does. A program that was correctly decided stays decided: the sample
   of every `bench` program and the decided list was checked by each group.
2. **A reference through a guard ends with the guard (005).** `&*lock(&m)` kept past its statement, or a
   reference through a bound guard kept past `drop(g)` or the guard's block, is `diag.destroy-while-aliased`
   at compile time (`spec/19`). Some programs that compiled before are refused; each of them read memory the
   lock no longer protected.
3. **`Rc` stays in its thread (031).** New rule `[Thread-Confined]` (`spec/19`) and diagnostic
   `diag.not-thread-safe`: `spawn` refuses an `Rc` or `Weak`, or any value that holds or reaches one, as an
   argument, a result, a `move` capture or a closure binding that captured one. A closure that captured an
   `Rc` elsewhere and reaches `spawn` as a plain `fn` value is not caught; closing that needs closure types
   that carry the property or a thread check in `Rc`, and is left (`CHG-0475`, "What is left").
4. **A failed write to standard output is seen (028).** D-0178 made a refused write unobservable. New rule
   `[Print-Failure]` (`spec/21`): a program whose standard-output bytes were refused, and which would exit with
   status 0, exits with 1, and says so in one line on standard error unless the reader closed the pipe (so
   `prog | head` stays quiet). D-0178's other choices stand.
5. **Source text cannot hide its order (023).** Bidirectional control characters (U+202A–U+202E,
   U+2066–U+2069) are refused anywhere in source, `[Bidi-Controls]` (`spec/22` §1); in a literal they are
   written `\u{…}`.
6. **A C entry point's name is C's (017).** An `extern fn` entry named as a C keyword, a macro of the standard
   headers, a C library or `libm` function, a name the runtime imports, or anything beginning with `_`, is
   refused (`diag.duplicate-item`, `spec/20` `[Extern-Entry]`). Such a name would be refused by the C compiler,
   or would replace the library function for the whole process.
7. **An inclusive range (038).** `foreach (x in lo ..= hi)` visits `hi` without overflowing (`0 ..= 255` over
   `u8` visits 256 values) and none when `lo > hi` (`[Foreach-Range-Inclusive]`, `spec/14`; grammar in
   `spec/22`). It is written out and read as it runs, so it keeps to the owner's rule that code reads on its
   own. A slice's range stays half-open, and `..=` there is refused with a message.
8. **`match` has an arm (011).** The grammar says so; `match (e) { }` is a syntax error. A value of an enum with
   no variants is taken apart with `_ : …`.
9. **A `const` fails before the run (013).** A `const` that uses `min_value`, `max_value`, `sizeof`, `widen` or
   `narrow` is evaluated when the program is checked, and a checked failure is reported at the constant, as the
   rule for constants already said.
10. **Types that grow without end are refused (042, 043).** A generic type whose fields lead back to it at a
    larger type argument cannot be laid out (`[Type-Recursion-Expanding]`, `spec/16`); an instantiation whose
    type argument has more than 4,096 type constructors is unbounded as one nested too deep is
    (`[Generic-Instantiation-Unbounded]`, `spec/15`). Both are `diag.instantiation-unbounded`.
11. **`remove` keeps its order and says what it costs (045).** `HashMap::remove` keeps insertion order, as
    D-0134 specifies, so it moves the entries after the removed one; it no longer rescans the table when few
    entries follow, and its documentation and `spec/21` give its cost beside `swap_remove`'s. `cobc` uses
    `swap_remove` where its maps' order does not matter.
12. **The standard library's errors name the common causes (041).** `FileError` gains `IsDirectory`,
    `NotDirectory`, `NoSpace` and `TooManyOpen`, `NetError` gains `NoDescriptors` and `MessageTooLong`. A
    `match` on either that names every variant without `_` needs the new arms.

## 3. Not defects

- **018.** `[Sort-Float]` (D-0181) allows `f32` and `f64` in `Vec::sort`, `binary_search` and
  `PriorityQueue::new`, NaN last. The report followed two texts that said otherwise; they are corrected.
- **022.** The derived `clone` of a ten-million-deep `Box` chain ends in `diag.stack-exhausted`, a clean fault.
  An iterative derived `clone` was not made; `Box::clone`'s documentation says that copying a deep chain
  recurses, the remedy the report names when an iterative one is not.

## 4. Consequences

- Programs newly refused: those of §2.2, §2.3, §2.5, §2.6, §2.8, §2.9, §2.10, and calls where an inline
  closure argument captures what another argument writes (009). Each was unsound, ill-formed by the existing
  rules, or could not be built.
- Programs that are no longer decided are those whose conflicts only the run can see; they are slower and no
  longer wrong.
- `std`'s error enums gained variants (§2.12).

## 5. Verification

See `CHG-0475`.
