# CHG-0332 — `Vec` growth freed the old cells before its checked writes; a fault there freed them twice

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18; a defect of both tools found by the grammar fuzzer's sanitizer run, under the
owner's "after the push focus on closing off those open items autonomously")
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0022 (conflicts found at first use), D-0199/D-0200 (`cobc`'s native `Vec` growth), D-0261 (the
verification round), `[Write-Conflict]` (`spec/08`), `[Fault-Unwind]` (`spec/18` §1)
Affects: `impl/std/collections.cb` (`Vec::grow`, `Vec::reserve`); `impl/cobc/src/lower/native_vec.rs`
(`native_vec_grow`, `native_vec_reserve`); `spec/conformance.md` (3.242.0); `spec/02` §5 (1.0.113)

## What changed

- **A defect of `std` and of `cobc`'s native lowering, repaired.** `Vec::grow` and `Vec::reserve` allocated
  the new cells, copied the elements, deallocated the old cells, and then wrote `v.ptr` and `v.cap` — two
  checked writes (`[Write]` through the exclusive parameter). When the write's check faults — the
  parameter's first conflicting access, D-0022, as in `push(e, x)` with `e` bound beside a shared reference
  to the same `Vec` — the `Vec` was left pointing at cells already freed, and the unwind's `Vec::drop`
  (`[Fault-Unwind]`) freed them again. `cobc`'s native growth, which fuses the three steps into one
  `realloc` for plain elements, had the same order, so the second free was of a block `realloc` had
  already released: a double free, which AddressSanitizer reports and which GCC's build survived only
  because `realloc` grew the block in place. Now the two writes come before the old cells go: in `std` the
  stores, in `cobc` their checks (nothing between a check and its store touches the `Vec`), then the
  reallocation. A fault at the write leaves the `Vec` as it was, its old cells freed once by the unwind;
  the new block is lost with the thread, as any allocation a fault interrupts is. Case
  `conf.vec-grow-fault-intact` (`[drop 1]` and the fault under both tools; the double free is what the
  fuzzer's sanitizer wrapper sees on the old `cobc`).
- **Found by** the grammar fuzzer (`stress/verify/fuzz`, D-0261): program 108 of seed 12, under the
  `clang` ASan+UBSan wrapper — "attempting double-free" in `cb_deallocate` from `Vec::drop` under
  `cbrt::fault`, the block "freed by `cb_reallocate_plain`". `coby` and `cobc` with GCC agreed on the
  program's output; only the sanitizer saw it.

## What changed for programs

A program whose `Vec` growth faults at its write (an exclusive and a shared reference to one `Vec` passed
together, the exclusive one pushed through) terminates with the same fault, line and exit status, and the
process no longer frees the old cells twice on the way out. A growth that does not fault is as before:
the same reallocation, the same stores, the same checks in the same order relative to each other.
