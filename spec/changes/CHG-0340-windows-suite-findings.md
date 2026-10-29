# CHG-0340 — The Windows run of the suites on Update 126: the object cache's placement race, and a lend case that depended on the schedule

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-19, the owner's Windows run of the conformance suite on Update 126: two findings, both repaired)
Governed by: `CobaltC_Master_Instructions.md` §13, §20
Depends on: `spec/19` `[Thread-Lend]` (D-0213: a lent path is held until its thread ends), D-0209 (`conf.direct-call-lent-vec-element`), D-0255 (`cobc` on Windows), the object cache (`impl/cobc/src/cache.rs`)
Affects: `impl/cobc/src/cache.rs` (`place`, `key_for`); `impl/cobc/tests/cache_identity.rs` (new); `impl/conformance/08-alias-validity/direct_call_lent_vec_element_rejected.cb`; `spec/conformance.md` (3.250.0); `spec/02` §5 (1.0.120)

## What changed

- **The cache's placement race (`cobc`, Windows).** A cacheable object is compiled under a name of its
  process's own and renamed into place when whole. Two `cobc` processes that both found the object missing
  (a parallel suite on a cold cache: the compiler had just been rebuilt, so every key was new) both compiled
  it, and the second's rename replaced the first's object while the first's linker was opening it. On
  Windows a file being replaced is one an open refuses, and the suite's run of `conf.cstring-libc` ended
  with the linker's `cannot find …/cobc-cache/5a94….o: Permission denied`. The two objects are alike (the
  key names the text, the compiler and its options), so the first to arrive is the one to keep: `place`
  now links the whole object to its final name with a hard link, which fails where the name exists, and
  drops its own copy; where the file system has no hard links, a rename to a name still missing, and a
  copy as the last resort. Nothing ever replaces an object another process may be linking. Twelve
  processes on a cold cache three times over, under Linux: every link whole, no temporary left behind.
- **A case that depended on the schedule (`conf.direct-call-lent-vec-element`).** The case lent a `Vec`
  to a pusher thread of a thousand pushes and borrowed `&v[0]` at once, expecting `diag.aliasing-conflict`.
  `[Thread-Lend]` holds the lent path until the thread ends, so the fault depends on the pusher being
  alive at the borrow: under Linux it always was; under Windows, with the suite's cases running in
  parallel, the main thread lost its processor at the spawn and the pusher finished first, and `coby`
  rightly observed `ok`. The pusher now waits for a word on a channel before it pushes (the shape of
  `conf.atomic-op-beside-lent-exclusive`), and `read` would send that word and join it: the pusher is
  alive at the borrow under every schedule, and the borrow is the program's only defect. Both tools fault
  the case on every run.
- **The cache's premise made a test, and the key it found wanting.** The cache rests on a `std` function
  instantiated at `std` types alone lowering to the same C text in every program; the lowering keeps that
  by construction, and a change that lost it would show late and obliquely (a stale object, a unit
  compiled again in every program, two helpers named alike as `CHG-0337`). `impl/cobc/tests/cache_identity.rs`
  compiles one `std` use (a `Vec<i64>` grown, sorted and popped, a `String` searched, a
  `HashMap<String, i64>` filled and read) inside two programs of different shape, and the same program
  renamed with a function of its own beside it, into one fresh cache, and reads `-v`'s "N unit(s)
  reused, M compiled" line: what the first program put there, the next must find whole. The first run
  found five of twenty-seven units compiled again: their text was the same (the key's second hash
  agreed), but the key hashed the declarations a unit needs, the forward declarations, prototypes and
  type blocks, in the order the program had reached them, and a program that reaches the same types in
  another order got another key. `key_for` sorts each of those before hashing. No object was ever wrong;
  a unit was kept twice and compiled twice, once per order a program could reach it in.

## What changed for programs

Nothing in the language. A program compiled by several `cobc` processes at once on one cache links a whole
object under Windows as it did under Linux.
