# cobmodel — an executable model of CobaltC's alias and thread rules

`cobmodel` (D-0219) runs a fragment of CobaltC by the specification's rules
and gives every outcome they allow a program: its static verdict, and, for a
program the static rules accept, every result of every interleaving of its
threads. It is written from the specification alone. It shares no code with
`coby`, `cobc` or `cbrt`, and does not depend on them, so that where it and
the tools agree the agreement means something.

## What it covers

The fragment is the corner where the reviews found their soundness holes: a
reference carried across a boundary — into a thread, into storage, through a
shared path, out of a function.

- **Types**: `i32`; `ref<i32, m>`; `struct Hold { ref<i32, exclusive> r; }`
  and `ref<Hold, shared>`; `Vec<i32>`, `Vec<ref<i32, exclusive>>` and
  `Vec<handle<void>>`; `struct Job { handle<void> h; }`; `slice<i32, m>`;
  `fn() : τ` and closures; `array<i32, 2>`; `mutex<i32>` and its guards;
  `Channel<ref<i32, exclusive>>`; handles; `Atomic<i32>` (D-0253).
- **Statements**: `let` (typed or `auto`), assignment, expression statements,
  `printf("%v\n", e)`, `_ = e;`, blocks, `if`/`else`, `while`, `break`,
  `continue`, `match` on an `Option`. A loop's head joins its entry with the
  back edge and each `continue`, iterated to the least fixed point, and its
  exit joins the test's false edge with each `break` (`spec/14` §5); the
  diagnostics and proofs are those of a last pass at it. A `break` or
  `continue` exits every block up to and including the loop body
  (`[Break]`), ending its bindings and waiting for its handles.
- **Arrays**: `[e1, e2]`, `a[k]` with a literal `k` (a place of the flow
  analysis, rejected statically outside the array), element borrows, slices
  of an array, an element lent to a thread.
- **Calls**: the helpers a generated program uses (`bump`, `peek`, `two`,
  `pass`, `bump_hold`, `read_hold`, `add_locked`, `read_locked`,
  `lock_add_hold`, `bump_first`, `swap_first`, `take_one`, `call_val`,
  `call_shared`, `bump_slice`, `first`; their CobaltC text is in
  `src/print.rs` and runs through the same rules as `main`), `Vec::new`,
  `push`, `pop`, indexing, `Mutex::new`, `lock`, `Channel::new`, `send`,
  `recv`, `close`, `spawn`, `join`, and `swap` and `replace` at `i32`
  (`[Swap-Places]` with `[Write]`'s premises, D-0235).
- **Atomics** (D-0253): `Atomic::new`, `load`, `store`, `fetch_add`,
  `notify_all` and `wait`, each operation one step under `[Atomic-Op]`'s
  checks (a shared access, no path formed); the wake count in the atomic's
  value; `Atomic::wait` as `std` writes it, a check and a sleep until the
  count moves, so a store without a notification can leave a waiter for
  ever (`[Deadlock]` (b), an atomic's wait the end of a chain).
- **Statics** (D-0258): `static Atomic<i32> S1 = k;`, one object established
  in `Σ_0` with a shared root path, which no scope ends and every thread
  borrows by name (`&S1`, `[Ref-Form-Static]`), in closures with no capture;
  `&mut S1` is `diag.write-through-shared` and `S1` read whole
  `diag.read-of-resource` (`spec/17` §1a′).
- **Closures** (D-0246): literals with and without `move`, capture modes from
  the body (D-0017), `c()`, `(*f)()`, a closure passed by value or by shared
  reference, `spawn(c)` and `spawn(move [..]() { .. })` (its captures lent);
  `[Closure-Call]` with `self` shared or, for a self-writing closure,
  exclusive (D-0239), borrow captures used as stored, move captures as the
  closure's fields, `[Read-Fn-Through-Shared]`.
- **Slices** (D-0246): `&_m v[lo .. hi]` of a `Vec` and of a slice, `s[k]`,
  slices passed to functions and threads; path steps `range` and `index`
  compared by `meet`, and `v[k]` as D-0238's element borrow; paths formed
  for one access pending until it (D-0242).
- **Static rules** (`src/statics.rs`): typing for the fragment, with D-0218's
  shared-path rules; `rule.control.flow-analysis` (`spec/14` §6) — the facts,
  the transfer table (with D-0236's forgetting) and the discharge table as
  written, recording for each statement which bindings' accesses it proves
  as well as which it refutes; `rule.temporal.ref-escape`;
  `[Ref-Binding-Last-Use]`. The first finding in source order is the outcome.
- **Dynamic rules** (`src/machine.rs`): `Σ` as `spec/04` gives it — objects,
  access paths with base, mode, thread, frame and statement scope, holders
  derived from storage (D-0215); `[Read]`, `[Write]`, `[Borrow]`, `clash` with
  `sync-exempt`; `[Thread-Affine]`, `[Thread-Lend]`; statement and block exits;
  calls and parameter binding (pending arguments, D-0107); `Vec`'s bodies'
  effects (reclaim, `[Release]` with `foreign-held`, growth); `[Lock]`,
  `[Guard-Drop]`, poisoning; channels; `[Deadlock]` and `[Channel-Deadlock]`;
  contained faults and `[Join-Failed]`; a handle's destruction wherever it is
  held — a binding, a `Job`, a `Vec`'s elements in order — waiting for its
  thread and raising a failed one's fault.
- **Interleavings** (`src/explore.rs`): every one, depth-first, with states
  deduplicated by hash; a step no other thread can observe is taken without
  branching.

Not modelled: closures with parameters, computed indices,
arithmetic beyond `+`, text, generics, raw pointers.
One simplification: `Move` ends an object and re-creates it under the new
binding (the text transfers the holder and keeps the identity); it changes
no outcome in the fragment.

What extending the model to closures, slices and handles in containers
found (D-0246): coby did not wait for a thread whose handle was held in a
`Vec`, a struct, an `Option` or an array, nor unlock a guard so held
(CHG-0293); and four places where the text disagreed with both tools —
D-0242 (`v[k] = v[k] + 1`, `h.r = h.r`, `Vec::push(&mut v, v[0])` faulted
by the letter), D-0243 (a reference captured by a closure was proven
clash-free while borrowed: `model.proven-but-false`), D-0244 (a handle moved
into a container kept its facts) and D-0245 (a self-writing closure's call:
static by the text, dynamic in the tools). Later rounds added loops, arrays,
closures owning a resource and guards/slices/closures across threads, and
found three more: coby copied a `move` capture without its read check
(CHG-0301), both tools faulted late on an owning closure passed by value
(CHG-0302), and a data race through a slice whose reborrow was lent to a
thread, in the text and coby (D-0248); D-0247 settled how a slice binding
passed to `spawn` is read. Round 5 (channels of slices) found that coby read
a reference without a check when dereferencing it (CHG-0305).

Where the text did not decide something, the model takes the reading both
tools take, says so in the source, and D-0219 lists it as a finding. D-0221 settled
findings 7, 8 and 10 the tools' way, and D-0222 finding 9 by a repair of
the tools; the model follows the text on each.

## Commands

    cobmodel check [--literal-dead] [--as-licensed [--pre-d0236] [--pre-d0243]] [--witness] FILE...
        Each program's outcome set. --witness prints one schedule reaching
        each outcome; --literal-dead reads [Object-End] without D-0219's
        result exemption (finding 2). --as-licensed (D-0241) models what
        the text lets an implementation omit: an access whose `¬clash`
        the discharge table proves, if `clash` is true of it all the
        same, ends in the outcome `model.proven-but-false` (the exit
        status is then 1, and a last line counts such programs);
        --pre-d0236 reads the transfer table as it stood before D-0236
        (a forgotten fact left its root unescaped), --pre-d0243 the closure
        row as it stood before D-0243 (a captured reference binding left
        unescaped). Over `sample/` --as-licensed reports none; with
        --pre-d0236 eleven (nine enumerated programs and the fourth
        review's two hand-written ones), with --pre-d0243 one (`hand-k02`).

    cobmodel enum --size N --out DIR [--max K] [--random M --len L --seed S]
        Every well-scoped program of up to N statements over the alphabet in
        src/enumerate.rs (x and y declared first, printed last), each with its
        outcome set in `// MODEL:` header lines; at most K written: first
        every program with more than one outcome or a dynamic fault, then an
        even sample. --random adds M programs of L statements.

    cobmodel compare DIR --coby PATH --cobc PATH [--explore N] [--cc CC]... [--jobs J] [--report FILE]
        Runs every program of DIR under coby once and under cobc --explore N
        with each C compiler, recomputes the model's set, and reports each
        program a tool gives an outcome outside it (exit status 1), and each
        program with a model outcome no tool showed (advisory: the tools try
        a few schedules, the model all of them).

## The sample and the battery

`sample/` holds the programs battery stage 1 and `impl/tools/suites.sh`
compare (`cobmodel compare impl/model/sample … --explore 8`): the hand-written
cases, every program that ever disagreed (each now agreeing, with what it
showed in its header comment), and a stratified selection of the enumeration.
A green comparison lets the manifest add *modelled* to the evidence tier.

The full enumeration is a command, not a battery step:

    cargo build --release -p coby -p cbrt -p cobc -p cobmodel
    target/release/cobmodel enum --size 3 --out /tmp/m3 --max 3000
    target/release/cobmodel compare /tmp/m3 --coby target/release/coby --cobc target/release/cobc --explore 4 --jobs 3
    target/release/cobmodel enum --size 0 --random 2000 --len 6 --seed 11 --out /tmp/r6

Running programs creates nothing in the directory but the compiler's
temporary files; still, run it outside the repository root.
