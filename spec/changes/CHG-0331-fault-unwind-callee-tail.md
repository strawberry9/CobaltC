# CHG-0331 — `coby`: a fault raised as a callee's tail flows out left the frame stack unbalanced; the fuzzer's first finding

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-18; a tool defect found by the grammar fuzzer of the verification plan's B4, under the
owner's "after the push focus on closing off those open items autonomously")
Governed by: `CobaltC_Master_Instructions.md` §20, §21
Depends on: D-0022 (conflicts found at first use), D-0107 (pending arguments), D-0261 (the verification round),
`[Fault-Unwind]` (`spec/18` §1)
Affects: `impl/src/interp/exec.rs`, `impl/src/interp/native_vec.rs`; `spec/conformance.md` (3.241.0); `spec/02` §5
(1.0.112)

## What changed

- **A `coby` defect, repaired.** When a function body's trailing expression is a place read out of a parameter
  bound pending (D-0107) — `fn first(ref<Vec<i64>, shared> s, ref<Vec<i64>, exclusive> e) : i64 { s[0] }`,
  called as `first(&v, &mut v)` — the `[Read]` that puts the place in result form is the parameter's first
  access, where D-0022's conflict with the exclusive argument beside it surfaces as `diag.aliasing-conflict`.
  The interpreter raised the fault *after* the body's statements had run and *before* the body's frame was
  popped, and returned without popping it. Every enclosing unwind then popped the frame below its own:
  the call's parameter frame was popped as the body's, the caller's statement scope as the parameter
  frame's, and `main`'s own frame — the outermost — was never popped at all. Its resources' destructors
  did not run, against `[Fault-Unwind]` ("running every destructor D-0008 would run on ordinary exit");
  `cobc` ran them. Scope-level resources (an inner block's) were unaffected, which is why no conformance
  case saw it: every fault case with a destructor kept its resource in a block, or faulted by a rule the
  body's statements raise. The read is now `result_form_or_unwind`: on a fault it pops the frame as a
  fault's unwind would, then propagates. The same shape in the native `Vec` fast paths
  (`native_body_end`) is fixed alike. Case: `conf.fault-unwind-callee-tail-conflict` (`[drop 3][drop 1]`
  under both tools).
- **Found by** the grammar fuzzer (`stress/verify/fuzz`, D-0261): program 159 of seed 11, the one
  disagreement in 300 — the same fault in both tools, `cobc`'s standard output with two more destructor
  lines.

## What changed for programs

A program whose fault is raised as a callee's tail flows out now runs `main`'s destructors under `coby`
as it always did under `cobc`, and as the text always said. Nothing else: no program that ran to
completion changes, and the fault, its line and its exit status are as before.
