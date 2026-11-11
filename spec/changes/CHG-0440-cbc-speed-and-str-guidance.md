# CHG-0440 — cbc's speed against cobc, two lowering savings, and when to use `str`

Status: implemented (2026-11-13), D-0296; Update 184
Affects: `impl/cobc/src/lower/` (`analysis_strings.rs`, `frames.rs`) and their twins in `impl/selfhost/back`
(`strings4.cb`, `gen.cb`); `impl/selfhost` (`back/gen.cb`, `back/state.cb`, `back/vec3.cb`,
`back/literals.cb`, `front/ast.cb`, `front/typecheck.cb`); the guide (`content_b.py`, `content_c.py`,
`content_cprog.py`, `content_rust.py`); the book (chapters 7, 8, 9, 10, 12 and their examples; the preface;
`book/single_html.py`, `.github/workflows/book.yml`); twelve showcases; `stress/p4/RESULTS.txt`

## Why

The owner, after Update 183: "cbc's speed must at least match cobc's speed, or better it, but must not be
slower", and "your speed comparisons are likely out of date, as cbc is now almost on par with cobc, so you'll
need to also verify those speed comparisons as well". And, on text: a similar explanation of why CobaltC has
and needs `str`, "along with your explanation of when it should and shouldn't be used", in the guide and the
book; an audit of `str` parameters in `std`, the guide, the book, the showcases and the selfhost; and a book
that can be downloaded as one HTML file for offline reading.

## What changes

**Measured first.** The comparison the documents quoted (cbc 5.6–8.2 times cobc) was long out of date. On
Update 183's source (`stress/p4/bench.sh`, cbc built with Clang; instructions, wall time):

| Input | cobc | cbc |
|---|---|---|
| `showcase/tier3/tinyos.cb` | 2.61 G, 1.1 s | 1.86 G, 0.8 s |
| `showcase/tier12/dns_over_http.cb` | 13.30 G, 4.6 s | 14.78 G, 6.5 s |
| `impl/selfhost/cobfmt/main.cb` | 110.6 G, 33.2 s | 68.8 G, 29.7 s |

cbc already won on its own kind of code and lost on `std`-heavy programs, in the back end.

**Both toolchains (the C changes, so cbc and cobc stay in step).**

- A destructuring's binder that the body never mentions gets no runtime object in a decided program: it only
  ends, as an unused `let` binding's does (D-0315 step 4; `analysis_strings.rs`, `strings4.cb`).
- The fault-notes table (D-0269) is zeroed only when it is registered, that is when a thread or
  `notes_always` can read it: `if (cb_nt_on) cb_notes_push(memset(cb_nt, 0, sizeof cb_nt), n)`. The
  unconditional zeroing was cbc's largest source of first-level cache misses (`frames.rs`, `gen.cb`).

**cbc's own source (the C of every program is unchanged).** A field taken from a call's result moves out of
it (`into_temp_c`, `ModedTy::take_inner`, `SizedTy::take_inner`) in place of copying the whole result; a
function declaration lent by `get_fn` is reused from a small pool of copies in place of cloned for each
nested use; probes write into reused buffers; a callee's name is copied once per call, not once per argument.

**Measured after, on the final source** (cbc with Clang, then GCC):

| Input | cobc | cbc (Clang) | cbc (GCC) |
|---|---|---|---|
| `showcase/tier3/tinyos.cb` | 2.61 G, 1.1 s | 1.65 G, 0.7 s | 1.84 G, 0.8 s |
| `showcase/tier12/dns_over_http.cb` | 13.29 G, 4.5 s | 12.68 G, 5.3 s | 14.13 G, 5.7 s |
| `impl/selfhost/cobfmt/main.cb` | 110.6 G, 32.2 s | 62.5 G, 26.1 s | 67.5 G, 26.9 s |

The C is the same as cobc's on each. cbc built with Clang runs fewer instructions than cobc on every input,
built with GCC on all but `dns_over_http`; in time it is faster on tinyos (0.64 of cobc's), cobfmt (0.81) and
the small programs (calc, pipeline and tree, 0.27 to 0.45), and still slower on the `std`-heavy HTTP and
crypto programs: `dns_over_http` 1.18 times under Clang and 1.27 under GCC, `tiny_api` about 1.2, `jwt` 1.06,
`certcheck` 1.01 to 1.04.

What is left is spread through cbc's generated code and memory traffic: cleanup bookkeeping, drop glue,
`malloc`, the copying of types and declarations, and the statement location stores (`cb_at`, about 6%, a
language-level cost under D-0309 and the owner's to decide). Tried and not kept: `-O3` (2.5%), glibc
`malloc` settings, and half-full hash tables in `std` (no change; reverted with its spec text). The owner
deferred the rest of the work after this round.

**When to use `str`** (no language change). The guide's "Which text type?" (§21) and the book's chapter 10
("When to use `str`", with pointers from chapters 7 and 9) say why the type exists: CobaltC has no lifetime
parameters, so a type is the only way to say "this text lives for the whole program" (Rust's `&'static str`).
A `str` parameter is right when the function keeps the text past the call (`Args::flag`) or when every
caller passes a literal message or label (`Option::expect`); every other text parameter is a `StringView`.

**The audit.** `std` and the selfhost: every `str` parameter fits the rule. The book: four examples now take
`StringView`. The guide: four examples, and the Rust table maps `&'static str` and `&str`. The showcases: twelve
programs that parse, hash, encode or send text take `StringView`.

**The book offline.** `book/single_html.py` makes mdBook's print page (every chapter on one page) one
self-contained file, its stylesheets, fonts, icons and scripts inside it, the sidebar's links pointed at the
chapters on that page, the search and the print dialog left out; the workflow publishes it beside the book
as `cobaltc-book.html`, and the preface links it.
