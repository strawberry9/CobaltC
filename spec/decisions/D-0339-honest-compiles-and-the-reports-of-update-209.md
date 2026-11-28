# D-0339 — Honest compiles, the D-0338 review, and the reports of Update 209

Status: §2.1–§2.4 are the owner's rulings in the review of D-0338 (2026-11-24); §2.5 onwards decided by the
implementer under the owner's instruction to fix the most serious of reports 350–451 (the "priority 1 and 2" groups:
memory unsafety, remote crashes of the HTTP server and client, programs `--check` accepts that crash `cobc`, silent
wrong results, and faults left to the run), with one gate at the end. Implemented by `CHG-0482`
Kind: Decision (`spec/02-schema.md`)
Depends on: D-0099 (`[Stmt-Result-Discarded]`), D-0150 (`exit`), D-0173 (the HTTP client), D-0213 (`spawn` lends),
D-0230 (`unittest`), D-0258 (`static`), D-0277 (no object cache), D-0336 §2.19 (the executable cache), §2.20 (the
variadic table), D-0337 §1 (`diag.handle-discarded`), D-0338 §2.1, §2.3, §2.10
Revisits: D-0336 §2.19 (withdrawn), D-0336 §2.20 (the table now refuses), D-0338 §2.10 (now both build forms)
Affects: `spec/17`, `spec/20`, `spec/21`, `spec/registry/diagnostics.md`, `spec/IMPLEMENTATION-NOTES.md`

## 1. Problem

The owner reviewed D-0338's twelve decisions one by one and kept all of them, with four changes: the executable
cache D-0336 §2.19 added (report 059) is to go; every build is to be an honest compile; the variadic table of
D-0336 §2.20 is to refuse instead of quietly fixing a declaration; and `--all-checks` is to destroy `Rc` chains
without recursion as the default build does. A fifth round of reports (350–451, found on Update 209) was grouped by
priority, and the owner asked for the first two groups to be fixed.

## 2. Decision

1. **Every build is an honest compile (owner's ruling).** `cobc` keeps no cache of earlier builds: no object cache
   (D-0277 removed it), no executable cache, no cached `std` parts, no cached front-end results, no other reuse of an
   earlier build's output. A change that would add any form of compile-time caching needs the owner's explicit
   approval. The executable cache (D-0336 §2.19, `COBALTC_CACHE`, `COBALTC_CACHE_DIR`) is removed, and with it
   report 393 (the cache ignored a rebuilt `.o` or `.a`). Report 059 (a large program's build time) is closed as
   "won't do" by caching; `--lto` and `-O 0` remain the ways to build faster.
2. **The variadic table refuses (owner's ruling).** A declaration without `...` of one of the C library's variadic
   functions cobc lists (the `printf` and `scanf` families, `open`, `fcntl`, `ioctl`, `syscall`, `execl`, `strfmon`,
   `error`, …) is `diag.extern-variadic-without-ellipsis` (`[Extern-Variadic-Without-Ellipsis]`, `spec/20`), whose
   message says to declare the fixed parameters and `...`. The form D-0336 allowed, the call's arguments listed in
   place of `...`, is gone: a declaration says what the function is, and `...` is how C says it.
3. **`Rc` chains are destroyed iteratively under `--all-checks` too (owner's ruling).** D-0338 §2.10's deferral of an
   `Rc` in tail position of an `Rc` destructor now also takes a field's `Rc` in a checked build, and the value an
   `Rc` destructor's `overwrite` consumes is in tail position there as in the decided form; a chain of a million
   links ends in constant stack in both forms.
4. **D-0338's other decisions stand as written** (owner's review: §2.1, §2.2, §2.4–§2.9, §2.11, §2.12 kept).
5. **A container's raw pointer carries its elements' references (392).** For the checker's table of which types hold
   references, a `rawptr<T>` field holds what `T` holds: `Vec<StringView>`, and a struct with such a field, hold the
   views' references. A struct field of that type kept no loan, so the default build read freed memory.
6. **A borrow of a part of a parameter keeps the parameter (407).** Under `from a`, `&b[0]` or `&b.text` of another
   parameter `b` is a kept use of `b` (`diag.result-source-kept`), even when the part's own value would be plain:
   reading `b[0]` is a probe, borrowing it is not.
7. **`drop` of an element or a field is a move out of it (439).** `drop(v[i])` and `drop(r.f)` are checked as
   `consume(v[i])` is, so a resource is not left as a hole (`diag.move-out-of-field`).
8. **A variant literal's open type parameters are fixed by the other arguments first (408).** In a generic call, an
   argument `Ok(x)`, `Err(e)` or `Some(x)` is checked after the others, as a bare literal is (D-0079), so the type
   parameters they and the expected result fix are compared with `x`'s type.
9. **Assignments that change nothing, or what cannot change, are refused (440, 441, 442).** The left side of `=` is
   not a constant or a part of one, a part of a call's result that is no reference, a variant, or a `static`
   (`[Static-Not-Assigned]`, `diag.static-assigned`, which `drop(S)` of a static also gives). A function item is not
   borrowed (443): `&f` is refused, the repair to pass `f` itself.
10. **What the lowering cannot infer is an error, not an assertion (409, 410, 411).** Where `cobc` cannot take a
    generic call's or variant's type arguments from what is written, it stops with
    `error[diag.type-annotation-needed]`, naming the function and line and the annotation to write. Making `--check`
    infer exactly what the lowering does is left (§3).
11. **A shift's literal left operand is not typed by the amount (358).** `1 << s` is an `i32` (or the expected type),
    as in C, Rust and Go; the amount stays a `u32`.
12. **A handle as a statement's value (350, 380).** `spawn(f, x);` written as a statement gets
    `diag.handle-discarded` (the handle ends at once and joins its thread); one whose thread gives a `Result` is
    `diag.result-discarded`, as any discarded `Result`.
13. **`exit` during a test fails the run (374).** `cobc --unittest`'s runner tells the runtime a run is on
    (`cb_unittest_runner`); an `exit` reached in a test prints the test as FAILED, says why, and ends the run with
    status 1.
14. **The HTTP server and client (423, 424, 425, 430, 431, 432).** A chunk size of more than eight hex digits is
    `BadMessage`, not an overflow fault; `HttpConnection::respond` answers what the last `request` read, once (a
    second call is `Err(BadMessage)` and writes nothing); a head line that is not UTF-8 is read as ISO-8859-1 (RFC 9110
    §5.5's obs-text); one empty line before a request line is ignored (RFC 9112 §2.2); after a failed request the
    closing connection reads and drops up to 64 MiB of what the client still sends, so the client sees the 413; and a
    request's whole head must arrive within the server's `timeout_ms` of its first byte.
15. **A live resource in a `Vec` or slice element (356).** Its overwrite is refused statically, as a field's is:
    an element the write reaches exists (an index past the end faults first).
16. **Non-ASCII bytes in paths (438).** A path written into the generated C carries each byte past ASCII as an octal
    escape; `cobmodel` keeps a non-ASCII character of a string as it is.

## 3. Left

- 384: a literal expression's overflow outside a declaration stays the run's: `conf.literal-expression-overflow-
  dynamic` (D-0037) records that both implementations fault at run time, and folding nested constants changed what
  `1 / (1 - 1)`, the way 123 tests fault on purpose, means. A static check needs the owner's ruling.
- 395, 414: a reference, view or closure of a local returned through a binding stays the run's: the binding's facts
  cannot show that another branch holds a parameter's reference (`conf.result-through-if-checked-at-run`, D-0284).
- 381, 382, 390, 394, 412, 413, 418: programs `--check` accepts whose fault the run-time record still catches
  (safe; the static pass does not yet decide them).
- 409, 410, 411: `--check` still accepts the programs whose type arguments the lowering cannot infer; they now stop
  with `diag.type-annotation-needed` instead of an assertion.
- 380: a handle whose thread gives a `Result` and that ends unjoined at its scope's end (not as a statement).
- 399 (`never` in a signature) is a language question for the owner.
- Priority 3–5 reports (refused programs, performance, tools, messages, documentation) for later changes.
