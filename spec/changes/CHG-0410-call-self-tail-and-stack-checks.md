# CHG-0410 — `[Call-Self-Tail]`; the stack check only where depth can grow; the stack floor off Linux

Status: implemented (2026-11-04), D-0309
Affects: `spec/15` 1.20.0 (`[Call-Self-Tail]`, §2a); `spec/conformance.md` 3.294.0; `coby` (`src/self_tail.rs`,
`Items::self_tails`, the loop in `call_user_fn_expected`); `cobc` (`lower/stack_checks.rs`, the jump in
`lower_user_call`, the label in `emit_fn_body`); `cbc` (`front/self_tail.cb`, `back/stack_checks.cb`, the same
lowering); `cbrt` (`find_stack_floor` for Windows and macOS); `bench/run.sh`

## What changes

1. **`[Call-Self-Tail]`.** A call of the enclosing function itself, in a tail position of its body, is not a level
   of nesting for `[Call-Stack-Exhausted]`. It must be direct, the function must not be generic, its frame must hold
   nothing to destroy, nothing in the frame may ever be borrowed, and the body must be decided (`spec/15` §2a).
   - The static pass of each toolchain records the calls (`coby`: `Items::self_tails` by address; `cbc`:
     `Facts::self_tails` by id).
   - `coby` returns the evaluated arguments to the call running the body (`Flow::SelfTail`). That call ends the
     frame and runs the body again in a new one, so neither the host stack nor the frame stack grows.
   - `cobc` and `cbc` lower the call as the arguments into temporaries, the frame left as a `return` leaves it,
     the parameters assigned, and `goto self_tail;`. The label is after the copies of over-aligned parameters,
     before the rest of the prologue. This applies in a body that records nothing (`tracked` false). The target is
     the body's own C name, or its dispatcher where the body is the function's own, `__dir` or `__chk`: light
     references always pass the dispatcher's test.
2. **The stack check (D-0077) only where depth can grow** (`stack_checks.rs`, the same rule in `cbc`). It stays in
   a function that is on a cycle of direct calls, has its address taken, has no caller in the program, calls
   `alloca`, or heads a chain of check-free frames over 64 KiB, estimated from the C. `main` keeps none.
3. **The stack floor off Linux.** On Windows the floor comes from the stack's reservation (`VirtualQuery`, present
   since Windows 7). On macOS it comes from `pthread_get_stackaddr_np` and `pthread_get_stacksize_np`. Before, the
   floor was `1` there, so the check never fired and a deep recursion crashed the host.
4. **`bench/run.sh`** writes the ratio of each `cobc` build to "Rust, overflow checks" (D-0309's measure).

## Conformance

- `conf.self-tail-call-completes` replaces `conf.tail-recursion-stack-exhausted`: `down(300000)` completes. The
  pushed `coby` faults there.
- `conf.fn-value-self-tail-call-stack-exhausted`, `conf.self-tail-call-resource-stack-exhausted` and
  `conf.self-tail-call-borrowed-local-stack-exhausted` keep the fault where a condition fails. Mutual recursion is
  not a row: which function of the cycle crosses the limit depends on the frames' sizes, so `cobc` under clang
  and `coby` name different functions. `coby`'s test of the first row runs it too, and `cobc/tests/stack_checks.rs`
  runs it under both C compilers.
- `cobc/tests/stack_checks.rs` covers which functions keep their check, and recursion still faulting.
  `cobc/tests/self_tail_calls.rs` covers 100 000 000 levels of a self tail call, and a list walked through a
  reference parameter.
