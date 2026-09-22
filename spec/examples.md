# CobaltC Examples

Status: normative artifact (illustrative; examples carry no normative force, Master Instructions §19)
Version: 3.13.1
Conforms to: `spec/02-schema.md` (Kind: Example, `ex.<name>`)

Every fragment is grammatical per `spec/22` and is derived in
`spec/conformance.md` (the `conf.*` cited under each example), where
the step-by-step reasoning lives. Library functions are called by
their qualified names (`Vec::push`, `spec/21`); `//` comments are
annotations; `c` in `if (c)` is a `bool` in scope whose value is not
statically known (`spec/conformance.md` conventions). Every example has **Status:** ACCEPTED. Standalone code
blocks use Allman brace style (`spec/22` §1); inline backtick-quoted
code embedded in a prose sentence (`**Corrected:** \`...\``) stays
single-line, the same structural exemption documented for table cells
in `spec/conformance.md` — breaking those across lines would break the
sentence, not just the code.

### `ex.unbound-name` — common mistake
    fn f() : i32
    {
        y
    }                                          // ✗ diag.unbound-name (static): y never declared
**Corrected:** `fn f() : i32 { i32 y = 1; y }`. → `conf.unbound-name`.

### `ex.use-after-move` — common mistake / corrected form
    Vec<i32> v = Vec::new();
    auto w = v;                               // moves v's object to w (rule.init.let → store → transfer)
    Vec::push(&mut v, 1);                     // ✗ diag.stale-binding (static): v's path was invalidated by the move
**Corrected:** use `w`. → `conf.transfer-invalidates-source`.

### `ex.no-copy-of-resource` — common mistake / corrected form
    Vec<i32> v = Vec::new();
    Vec<i32> w = Vec::new();
    auto same = v == w;                       // ✗ diag.read-of-resource (static): operands are value position
**Corrected:** compare through references or lengths: `Vec::len(&v) == Vec::len(&w)`. → `conf.read-of-resource-rejected`.

### `ex.no-silent-resource-overwrite` — common mistake / corrected form
    Vec<i32> v = Vec::new();
    v = Vec::new();                           // ✗ diag.overwrite-of-live-resource: v still owns a live Vec
**Corrected:** `drop(v); v = Vec::new();` — once the old value has ended, the binding can take a new one (`spec/11` `[Assign-Reestablish]`, D-0033); shadowing, `drop(v); Vec<i32> v = Vec::new();`, also works. → `conf.overwrite-live-resource-rejected`, `conf.reassign-after-drop-ok`, `conf.drop-then-shadow-ok`.

### `ex.definite-assignment` — common mistake / corrected form
    i32 x;
    if (c)
    {
        x = 1;
    }
    x                                         // ✗ diag.use-of-uninitialized (static): no write on the else path
**Corrected:** `if (c) { x = 1; } else { x = 0; }`. → `conf.definite-assignment-*`.

### `ex.checked-arithmetic` — canonical / boundary
    fn add(i32 a, i32 b) : i32
    {
        a + b
    }
    add(2147483647, 1)                        // ✗ diag.arith-overflow (dynamic) — D-0002 checked default
    wrapping_add(2147483647, 1)               // ✓ → -2147483648, explicit opt-in
    fn div(i32 a, i32 b) : i32
    {
        a / b
    }
    div(-2147483648, -1)                      // ✗ diag.div-overflow (dynamic)
    2147483647 + 1                            // ✗ diag.arith-overflow (static): both operands literal
→ `conf.i32-add-overflow-*`, `conf.i32-div-min-neg-one`, `conf.u8-wrapping-add`.

### `ex.negation` — canonical / boundary
    i32 x = 5;  auto y = -x;                  // ✓ -5
    fn neg(i32 m) : i32
    {
        -m
    }
    neg(-2147483648)                          // ✗ diag.arith-overflow (dynamic): -min(i32)
→ `conf.neg-*`.

### `ex.borrow-conflict` — common mistake / corrected form / canonical
    i32 x = 1;
    auto r1 = &mut x;
    auto r2 = &x;                             // ✗ diag.aliasing-conflict (static): r1 is exclusive and live
**Corrected:** `{ auto r1 = &mut x; *r1 = 2; } auto r2 = &x;` — r1's holder ended with its block.
**Reborrow:** `auto r1 = &mut x; { auto r2 = &*r1; *r2; } *r1 = 3; *r1` ✓ — r2 is a child of r1; a parent
may be used again once its children are gone. Using r1 *while* r2 lives (`*r1 = 2`) is
`diag.aliasing-conflict`. → `conf.shared-then-exclusive-rejected`, `conf.sequential-borrows-ok`,
`conf.reborrow-ok`, `conf.parent-use-while-child-live-rejected`.

### `ex.shared-readers` — canonical valid use
    i32 x = 1;
    auto r1 = &x;  auto r2 = &x;              // ✓ any number of shared readers
    x + *r1 + *r2                             // ✓ the owner may read while shared children live
    x = 2;                                    // ✗ diag.aliasing-conflict (static): writing while r1, r2 live
→ `conf.two-shared-borrows-ok`, `conf.owner-read-while-shared-ok`, `conf.owner-write-while-shared-rejected`.

### `ex.projection-conflict` — common mistake (the case `spec/AUDIT-2.md` B-01 found)
    struct P
    {
        i32 a;
        i32 b;
    }
    auto p = P { .a = 1, .b = 2 };
    auto r = &mut p.a;
    p.a = 5;                                  // ✗ diag.aliasing-conflict (static): r is a live exclusive
                                              //   path to p.a not derived from this write's path
    p.b = 6;                                  // ✓ disjoint field
    auto r2 = &mut p.b;                       // ✓ disjoint fields may be exclusively borrowed together
→ `conf.projection-write-while-borrowed-rejected`, `conf.disjoint-field-borrows-ok`.

### `ex.shared-is-read-only` — common mistake
    fn bump(ref<P, shared> p)
    {
        p.a = 1;
    }                                          // ✗ diag.write-through-shared (static)
    fn grab(ref<P, shared> p)
    {
        auto w = &mut p.a;
    }                                          // ✗ diag.borrow-exceeds-source (static)
**Corrected:** take `ref<P, exclusive>`. → `conf.write-through-shared-rejected`, `conf.exclusive-from-shared-rejected`.

### `ex.sequential-borrows` — canonical valid use
    Vec<i32> nums = Vec::new();
    Vec::push(&mut nums, 10);                 // the argument borrow dies when the parameter frame exits
    Vec::push(&mut nums, 20);                 // ✓ a fresh borrow; nothing conflicts
→ `conf.sequential-exclusive-borrows-ok`.

### `ex.destroy-while-borrowed` — common mistake / corrected form
    Vec<i32> v = Vec::new();
    auto r = &v;
    drop(v);                                  // ✗ diag.destroy-while-aliased (static): r still live
**Corrected:** `{ Vec<i32> v = Vec::new(); auto r = &v; }` — block exit ends r's object first
(it was established later), then destroys v. → `conf.destroy-while-borrowed-rejected`,
`conf.destroy-after-borrow-scope-ends-ok`.

### `ex.move-while-borrowed` — common mistake
    Vec<i32> v = Vec::new();
    auto r = &v;
    auto w = v;                               // ✗ diag.move-while-aliased (static)
→ `conf.move-while-borrowed-rejected`.

### `ex.double-destroy` — common mistake
    Vec<i32> v = Vec::new();
    drop(v);
    drop(v);                                  // ✗ diag.stale-binding (static): the first drop ended v's object
→ `conf.double-destroy-rejected`.

### `ex.returned-resource` — canonical valid use
    fn make_vec() : Vec<i32>
    {
        Vec<i32> v = Vec::new();
        Vec::push(&mut v, 1);
        v                                     // released into a temporary; not swept by make_vec's frames
    }
    {
        auto result = make_vec();             // adopted: result owns it
        Vec::push(&mut result, 2);            // destroyed here, at the caller's block exit
    }
    make_vec();                               // ✓ unstored: the temporary is destroyed at the `;`
→ `conf.returned-resource-destroyed-at-caller-exit`, `conf.unstored-temporary-destroyed-at-stmt-end`.

### `ex.reference-escape` — common mistake / corrected form
    fn dangling() : ref<i32, shared>
    {
        i32 x = 1;
        &x
    }                                          // ✗ diag.reference-escapes-scope (static)
**Corrected:** `fn ok() : i32 { i32 x = 1; x }`. → `conf.reference-escape-rejected`.

### `ex.field-reference` — canonical valid use (closes `spec/AUDIT-STATUS.md` A-32)
    struct H
    {
        ref<i32, shared> r;
    }
    i32 x = 1;
    auto h = H { .r = &x };                   // the borrow is held by h's object
    *h.r                                      // ✓ → 1, as long as h lives
→ `conf.reference-in-field-survives-statement`.

### `ex.bounds-check` — canonical / boundary
    array<i32, 3> a = [10, 20, 30];
    a[2]                                      // ✓ → 30
    a[3]                                      // ✗ diag.index-out-of-bounds (static): literal index
    Vec<i32> v = Vec::new();  Vec::push(&mut v, 10);
    *Vec::index_shared(&v, 0)                 // ✓ → 10
    *Vec::index_shared(&v, 1)                 // ✗ diag.index-out-of-bounds (dynamic): checked against len
→ `conf.index-*`, `conf.vec-index-*`.

### `ex.exhaustive-match` — common mistake / corrected form
    enum Sign
    {
        Pos,
        Neg,
        Zero
    }
    fn describe(Sign s) : i32
    {
        match (s)
        {
            Pos : 1,
            Neg : -1
        }
    }                                          // ✗ diag.non-exhaustive-match (static)
**Corrected:** add `Zero : 0`, or `_ : 0`. → `conf.match-*`.

### `ex.resource-payload` — canonical valid use
    enum E
    {
        Has(Vec<i32>),
        Empty
    }
    auto e = E::Has(Vec::new());
    match (e)
    {
        Has(x) : Vec::len(&x),
        Empty : 0
    }                                          // ✓ payload moved into x; e consumed
    Vec::len(&x)                              // (outside the arm) ✗ diag.unbound-name; and e is now stale
→ `conf.match-resource-payload-transfers`.

### `ex.unsafe-required` — common mistake / corrected form
    fn bad(rawptr<i32> p) : i32
    {
        *p
    }                                          // ✗ diag.trusted-outside-unsafe (static)
    fn ok(rawptr<i32> p) : i32
    {
        unsafe
        {
            *p
        }
    }                                          // ✓ the caller is now trusted to pass a valid p
→ `conf.rawptr-deref-*`.

### `ex.extern-write` — canonical valid use
    fn send(rawptr<u8> p, usize n) : isize
    {
        unsafe
        {
            write(p, n)
        }
    }                                          // ✓ [Extern-Call] is trusted-unchecked; unsafe covers it
    auto message = [104:u8, 101:u8, 108:u8, 108:u8, 111:u8, 10:u8];  // "hello\n"
    auto p = reinterpret_ptr<u8>(rawptr_of(&message));
    send(p, 6)                                 // ✓ → claim : isize, Σ.trust(claim) = unchecked-claim
→ `conf.extern-write-observed`.

### `ex.str-literal` — canonical valid use / common mistakes
    str greeting = "hello\n";                  // ✓ [Str-Literal]: a str value, six bytes, valid UTF-8 by construction
    str again = greeting;                      // ✓ copied, not moved: is-resource(str) = false
    usize n = str_len(greeting);               // ✓ → 6; greeting is still valid
    u8 h = str_byte(greeting, 0);              // ✓ → 104
    bool same = greeting == "hello\n";         // ✓ [Eq-Str] bytewise → true
    String owned = String::from_str("caf\u{e9}");   // ✓ 5 bytes; no validator runs (inv.str.utf8-validity)
    array<u8, 3> raw = b"\x00\xff!";           // ✓ byte-literal: [0:u8, 255:u8, 33:u8]
    printf("hello\n");                          // ✓ writes the six bytes; no unsafe in the program
    str_byte(greeting, 6);                     // ✗ diag.index-out-of-bounds (static): literal index, FA refuted
    bool lt = greeting < "z";                  // ✗ diag.type-mismatch (static): [T-Cmp] orderings need τ numeric
    extern fn put(str s);                      // ✗ diag.extern-non-ffi-type (static): str ∉ FfiType
→ `conf.str-*`, `conf.byte-literal-array`, `conf.string-from-str`, `conf.print-observed`.

### `ex.lifetime-elision` — canonical / common mistake
    struct Pair
    {
        i32 a;
        i32 b;
    }
    fn first(ref<Pair, shared> p) : ref<i32, shared>
    {
        &p.a
    }                                          // ✓ one reference parameter
    fn pick(ref<i32, shared> p1, ref<i32, shared> p2) : ref<i32, shared>
    {
        p1
    }                                          // ✗ diag.lifetime-elision-ambiguous (static)
    auto pr = Pair { .a = 1, .b = 2 };  auto y = first(&pr);  *y     // ✓ → 1
    pr.a = 3;                                 // ✗ diag.aliasing-conflict (static): y still reaches pr.a
→ `conf.elision-*`.

### `ex.generic-box` — canonical valid use
    struct Box<T>
    {
        T value;
    }
    Box<i32> b = Box { .value = 5 };                          // is-resource(Box<i32>) = false
    Box<Vec<i32>> c = Box { .value = Vec::new() };             // is-resource = true (derived); Vec::new's T from the field type
    fn identity<T>(T x) : T
    {
        x
    }
    identity(5)                                               // ✓ T := i32 from the argument
    auto bad = Vec::new();                                    // ✗ diag.cannot-infer-type-parameter (static)
→ `conf.generic-*`.

### `ex.closure-capture` — canonical / common mistake
    i32 x = 10;
    auto add_x = [x](i32 y)
    {
        x + y
    };                                                   // borrow-captures x (shared: the body only reads it)
    add_x(5)                                              // ✓ → 15
    Vec<i32> v = Vec::new();
    auto pusher = move [v]()
    {
        Vec::push(&mut v, 1);
    };                                                     // move-captures v
    pusher();                                              // ✓ pushes through the closure's own field
    Vec::push(&mut v, 2);                     // ✗ diag.stale-binding (static): v was moved into pusher
    auto bad = [](i32 y)
    {
        x + y
    };                                         // ✗ diag.capture-list-mismatch (static): x is used but not listed
→ `conf.closure-*`.

### `ex.rc-shared` — canonical valid use
    auto a = Rc::new(1);
    auto b = Rc::clone(&a);                   // count = 2
    drop(a);                                  // count = 1, box still allocated
    drop(b);                                  // count = 0, box deallocated
→ `conf.rc-*`.

### `ex.threads` — canonical valid use
    fn work(i32 n) : i32
    {
        n * 2
    }
    auto h = spawn(work, 21);
    join(h)                                   // ✓ → 42; the handle is consumed
    auto m = Mutex::new(0);
    {
        auto g = lock(&m);
        *g = *g + 1;
    }                                          // guard released at block exit
    *lock(&m)                                 // ✓ → 1 (the temporary guard dies at the `;`)
→ `conf.spawn-join-*`, `conf.mutex-*`.

### `ex.module-file` — canonical valid use (two files; `spec/17` §5)
    // geometry.cb -- the body of a module; nothing here names it
    export struct Rect
    {
        export f64 w;                         // field visibility is per field (spec/17 §2)
        export f64 h;
    }
    export fn area(Rect r) : f64
    {
        r.w * r.h
    }
    fn scale_factor() : f64                   // private: usable only inside this module
    {
        1.0
    }

    // main.cb -- the program; run this file
    module geo "./geometry.cb";               // ≡ module geo { …geometry.cb's items… }
    import geo::Rect;
    fn main()
    {
        Rect r = Rect { .w = 3.0, .h = 4.0 };
        f64 a = geo::area(r);                 // ✓ → 12.0
        // geo::scale_factor();               // ✗ diag.name-not-visible (static): not export
    }
→ `conf.module-file-ok`, `conf.module-file-import-ok`, `conf.module-file-type-ok`, `conf.module-file-private-rejected`; the runnable pair is `impl/cobaltc_examples/13_modules.cb` + `13_geometry.cb`.

### `ex.e2e-threads-mutex` — feature interaction (end-to-end)
    fn worker(ref<mutex<i32>, shared> m, i32 n) : i32
    {
        i32 i = 0;
        while (i < n)
        {
            auto g = lock(m);
            *g = *g + 1;
            i = i + 1;
        }                                          // each iteration's guard is destroyed at the body's exit
        n
    }
    fn main()
    {
        auto m = Mutex::new(0);
        auto h1 = spawn(worker, &m, 3);
        auto h2 = spawn(worker, &m, 4);
        i32 a = join(h1);
        i32 b = join(h2);
        i32 total = *lock(&m);                    // ✓ 7, whatever the interleaving; a + b == 7
    }
Two threads hold shared references to one mutex; each write goes through a lock path whose only
non-ancestor competitor is the other thread's shared reference, exempted by `sync-exempt`.
→ `conf.e2e-threads-mutex-total`.

### `ex.e2e-vec-nested-realloc` — feature interaction (end-to-end)
    import std;

    fn main()
    {
        Vec<Vec<i32>> outer = Vec::new();
        i32 k = 0;
        while (k < 5)
        {
            Vec<i32> inner = Vec::new();
            Vec::push(&mut inner, k);
            Vec::push(&mut outer, inner);        // moves inner into raw storage; outer grows at 0→4 and 4→8
            k = k + 1;
        }
        {
            auto first = Vec::index_shared(&outer, 0);
            usize n = Vec::len(first);            // ✓ 1; first dies with this block
        }
        match (Vec::pop(&mut outer))
        {
            Some(v) : { Vec::push(&mut outer, v); },   // moved out, moved back in
            None : {},
        }
        usize total = Vec::len(&outer);           // ✓ 5; block exit destroys five inner Vecs, then outer's buffer
    }
→ `conf.e2e-vec-nested-realloc`.

### `ex.e2e-rc-resource-payload` — feature interaction (end-to-end)
    import std;

    fn main()
    {
        Vec<i32> payload = Vec::new();
        Vec::push(&mut payload, 42);
        auto a = Rc::new(payload);                // payload moved into the box (RcBox<Vec<i32>> is a resource)
        auto b = Rc::clone(&a);                   // count = 2
        usize n = Vec::len(Rc::get(&b));          // ✓ 1
        drop(a);                                  // count = 1
        usize m = Vec::len(Rc::get(&b));          // ✓ 1: the box outlives a
        drop(b);                                  // count = 0: inner Vec::drop, then the box is deallocated
    }
→ `conf.e2e-rc-resource-payload`.

### `ex.e2e-propagate-chain` — feature interaction (end-to-end)
    import std;

    fn parse(u8 b) : Result<i32, i32>
    {
        if (b > 9)
        {
            return Err(1);
        }
        Ok(widen<i32>(b))
    }
    fn sum(ref<Vec<u8>, shared> bytes) : Result<i32, i32>
    {
        i32 acc = 0;
        usize i = 0;
        while (i < Vec::len(bytes))
        {
            i32 d = parse(*Vec::index_shared(bytes, i))?;   // Err returns out of the loop, the arm, and the body
            acc = acc + d;
            i = i + 1;
        }
        Ok(acc)
    }
    fn main()
    {
        Vec<u8> v = Vec::new();
        Vec::push(&mut v, 3);
        Vec::push(&mut v, 4);
        i32 total = match (sum(&v)) { Ok(t) : t, Err(_) : -1 };    // ✓ 7
        Vec::push(&mut v, 12);
        i32 bad = match (sum(&v)) { Ok(t) : t, Err(_) : -1 };      // ✓ -1
    }
→ `conf.e2e-propagate-chain`.

### `ex.e2e-vec-realloc-stale-ref` — feature interaction (end-to-end)
    import std;

    fn hold_across_grow(ref<Vec<i32>, exclusive> v) : i32
    {
        auto r = Vec::index_shared(&*v, 0);       // shared reborrow of *v: v is a parameter, so
                                                    // rule.control.flow-analysis names no referent for it
        Vec::push(v, 20);                          // v already ref<Vec<i32>,exclusive>: passed by value, no new borrow
        Vec::push(v, 30);
        Vec::push(v, 40);
        Vec::push(v, 50);                          // len = cap = 4 here: grow reallocates and releases element 0
        *r                                          // ✗ diag.stale-binding (dynamic): r's reclaimed object ended at [Release]
    }
    fn main()
    {
        Vec<i32> v = Vec::new();
        Vec::push(&mut v, 10);                     // grow 0→4; len = 1, cap = 4
        hold_across_grow(&mut v);
    }
Unlike `conf.vec-ref-then-push-rejected` (the same-function shape, caught
statically), holding the element reference across a reallocating `push`
reached through a reference *parameter* is invisible to
`rule.control.flow-analysis`: its `deriv`/`escaped` facts are defined only
over a plain binding's own field/index projections, and `&*v`'s place root
is a dereferenced reference, not one. Nothing rejects the program; the
fourth `push` inside `hold_across_grow` reallocates, `[Release]` ends the
reclaimed element `r` still names, and the later `*r` faults dynamically —
confirming `spec/21`'s own account of this hazard and closing the first
`CHG-0011` revisit condition with no rule change needed.
→ `conf.e2e-vec-realloc-stale-ref`.

### `ex.e2e-mutex-vec-resource-interior` — feature interaction (end-to-end)
    import std;

    fn main()
    {
        Vec<i32> inner = Vec::new();
        auto m = Mutex::new(inner);
        {
            auto g = lock(&m);
            Vec::push(&mut *g, 1);
            Vec::push(&mut *g, 2);
            usize n = Vec::len(&*g);           // ✓ 2
        }
    }
`Mutex::new` moves a resource-typed `inner`; `lock`/`[Guard-Deref]` reach
it through the guard for both an exclusive push and a shared length
read; `m`'s automatic destruction at `main`'s block exit runs
`destroy-composite` on the `inner` obligation, which is what actually
frees the `Vec`'s buffer (not the mutex's own storage).
→ `conf.e2e-mutex-vec-resource-interior`.

### `ex.e2e-closure-move-rc` — feature interaction (end-to-end)
    import std;

    fn main()
    {
        Vec<i32> payload = Vec::new();
        Vec::push(&mut payload, 7);
        auto a = Rc::new(payload);
        auto f = move [a]() { Vec::len(Rc::get(&a)) };
        usize x = f();
        usize y = f();
        drop(f);
    }
The closure's anonymous struct owns `a` outright (moved, not borrowed);
calling it twice only ever forms shared sub-borrows of that field, so
nothing is consumed between calls. `drop(f)` runs `destroy-composite`
on the closure's captured field exactly as it would on any struct
field, reaching `Rc::drop` from inside that composite destroy.
→ `conf.e2e-closure-move-rc`.

### `ex.e2e-thread-fault-abandons-guard` — feature interaction (end-to-end)
    fn worker(i32 n) : i32
    {
        n / 0
    }
    fn main()
    {
        auto m = Mutex::new(0);
        auto g = lock(&m);
        auto h = spawn(worker, 5);
        *g = *g + 1;
        join(h);
    }
`worker` always faults, independently of `main`. `[Fault-Unwind]` only
unwinds the faulting thread; `main`'s guard `g` — and everything it was
doing with it — is abandoned mid-flight, unrun, exactly as `spec/18`
§1 already states. Because none of `main`'s remaining steps perform an
`extern` call, every interleaving `rule.conc.spawn`'s `[Thread-Step]`
admits reaches the same observable result: the program's termination
outcome is well-defined despite the abandoned guard.
→ `conf.e2e-thread-fault-abandons-guard`.

### `ex.e2e-spawn-join-resource-result` — feature interaction (end-to-end)
    import std;

    fn make() : Vec<i32>
    {
        Vec<i32> v = Vec::new();
        Vec::push(&mut v, 5);
        v
    }
    fn main()
    {
        auto h1 = spawn(make);
        auto v1 = join(h1);
        usize n = Vec::len(&v1);           // ✓ 1
        drop(v1);
        { auto h2 = spawn(make); }         // unjoined: the handle's automatic destruction discards a Vec<i32>
    }
`join`'s result and an unjoined handle's discarded result both carry a
resource whose destroy authority was granted, at establishment, to the
*worker* thread — not the joiner or the thread sweeping the handle.
`drop(v1)` and the automatic discard of `h2`'s result both need that
authority re-keyed to the thread now doing the destroying, which
`rule.conc.join`'s `[Join]`/`[Handle-Destructor]` now do.
→ `conf.spawn-join-resource-result`.

### `ex.e2e-vec-pop-push-reuse-stale-ref` — feature interaction (end-to-end)
    import std;

    fn hazard(ref<Vec<i32>, exclusive> v) : i32
    {
        auto r = Vec::index_shared(&*v, 2);       // shared ref into element 2 (value 30)
        Vec::pop(v);                               // releases element 2's reclaimed object (F-05 fix, CHG-0019)
        Vec::push(v, 99);                          // reuses the same slot — no reclaimed object survives to alias
        *r                                          // ✗ diag.stale-binding (dynamic)
    }
    fn main()
    {
        Vec<i32> v = Vec::new();
        Vec::push(&mut v, 10);
        Vec::push(&mut v, 20);
        Vec::push(&mut v, 30);
        hazard(&mut v);
    }
Before `CHG-0019`, `Vec::pop`'s read of a plain-typed element left its
reclaimed object alive, so the second statement's `push` would have
silently overwritten the value `r` observes with no diagnostic at all
(`spec/AUDIT-STATUS.md` finding F-05). `Vec::pop` now `release`s the
popped slot, so this hazard is caught exactly like a reallocation:
dynamically, at `r`'s next use, never silently.
→ `conf.e2e-vec-pop-push-reuse-stale-ref`.

### `ex.e2e-hello-print` — feature interaction (end-to-end)
    import std;

    fn main()
    {
        str who = "CobaltC";
        String owned = String::from_str(who);
        if (String::len(&owned) != str_len(who))
        {
            1 / (1 - 1);                       // never reached: both are 7
        }
        printf("Hello, ");
        printf("%v", who);
        printf("!\n");
    }                                          // ✓ writes "Hello, CobaltC!\n"; terminates ok
The whole program is text in, text out, and never spells `unsafe`,
a raw pointer, or a byte: the trust transition lives inside `std`'s
private `print`, which `printf` writes through (`spec/21` §2a, §2f). `who` is used three times — a `str` is a value, so
none of those uses is a move.
→ `conf.e2e-hello-print`.

### `ex.local-inference` — canonical valid use
    auto total = 0;                           // i32 by default
    u8 cap = 200;                             // 200 checked against u8
    auto sum = total + 1;                     // 1 takes i32 from total
→ `conf.literal-*`, `conf.let-synthesis`.

## Change Log

- 3.13.1 — Non-normative (`CHG-0047`, D-0039): every example writes
  output with `printf`; `ex.e2e-hello-print`'s note follows.

- 3.13.0 — `CHG-0042` (D-0033): `ex.no-silent-resource-overwrite`'s
  corrected form gives the dropped binding a new value; it names
  `conf.reassign-after-drop-ok`.

- 3.12.0 — `CHG-0029`: `ex.module-file` added — the canonical two-file
  program (`module geo "./geometry.cb";`), the example Master
  Instructions §19 expects for a new construct and `CHG-0026` had
  deferred. Not run by `impl/tests/spec_rows.rs` (two files); its
  `conf.module-file-*` rows run the equivalent file-based cases.
- 3.11.0 — `CHG-0025` (D-0020): `ex.str-literal` and
  `ex.e2e-hello-print` added — the `str` value type, both literal
  forms, `String::from_str`, and `print` (`spec/21` §2a).
- 3.10.0 — `CHG-0021`: `ex.extern-write` added, demonstrating the
  `write` prelude `extern fn` `CHG-0020` added.
- 3.9.0 — `CHG-0019`: `ex.e2e-vec-pop-push-reuse-stale-ref` added,
  demonstrating the `Vec::pop` fix that closes finding F-05.
- 3.8.0 — `CHG-0015`: `ex.e2e-spawn-join-resource-result` added,
  demonstrating the `spec/19` 1.5.0 authority-transfer fix (`F-06`).
- 3.7.0 — `CHG-0013`: the remaining three `CHG-0011` revisit-condition
  programs added (`ex.e2e-mutex-vec-resource-interior`,
  `ex.e2e-closure-move-rc`, `ex.e2e-thread-fault-abandons-guard`), each
  derived in `spec/conformance.md` §13. No rule changed.
- 3.6.0 — `CHG-0012`: `ex.e2e-vec-realloc-stale-ref` added — the first
  of `CHG-0011`'s revisit-condition programs, derived in
  `spec/conformance.md` §13. No rule changed; the derivation confirms
  the post-`CHG-0011` rules already produce `diag.stale-binding`
  dynamically for this shape.
- 3.5.0 — `CHG-0011`: four end-to-end programs added
  (`ex.e2e-threads-mutex`, `ex.e2e-vec-nested-realloc`,
  `ex.e2e-rc-resource-payload`, `ex.e2e-propagate-chain`), each fully
  derived in `spec/conformance.md` §13 (Master Instructions §20
  feasibility analysis). The `match` arms in `ex.e2e-propagate-chain`'s
  `main` stay on one line because they sit inside a declaration
  statement's initializer (the same structural exemption as inline
  snippets).
- 3.4.1 — Non-normative (consistency pass, `CHG-0009`): header states
  what `c` denotes in `ex.definite-assignment`.
- 3.4.0 — Reformatted every standalone code block to Allman brace
  style (human-directed documentation convention, `spec/22` §1; non-
  normative per `spec/02-schema.md` §6, no `CHG-XXXX` record). Inline
  backtick-quoted code embedded in prose (`**Corrected:** \`...\`,
  `**Reborrow:** \`...\``) is left single-line — the same structural
  exemption already documented for `spec/conformance.md`'s table
  cells, since breaking an inline snippet across lines would break the
  sentence it sits in, not just the code. No example's id, category,
  illustrated rule, or semantic content changed.
- 3.3.0 — `CHG-0005`: `ex.exhaustive-match` and `ex.resource-payload`
  re-spelled with `:` instead of `=>` for match arms. No example's id,
  category, or semantic content changed.
- 3.2.0 — `CHG-0003`: every `if`/`while`/`match` fragment re-spelled
  with mandatory condition parentheses (`spec/22` 2.2.0). No example's
  id, category, or semantic content changed.
- 3.1.0 — `CHG-0002`: `ex.closure-capture` re-spelled to `spec/22`
  2.1.0's C++11-style closure syntax (`[captures](params) { ... }`);
  added a `diag.capture-list-mismatch` mistake case to the same
  example. No other example touched.
- 3.0.0 — Re-spelled to `spec/22` 2.0.0's C-style declarator syntax
  (`CHG-0001`): type-first local declarations and parameters, `auto`
  for inferred locals, `.f = e` struct-literal initializers,
  semicolon-terminated struct fields, `:`-introduced return types. No
  category, illustrated rule, or semantic content of any `ex.*` entity
  changed.
- 2.0.0 — Regenerated against the 1.0.0 rules with every fragment
  derived in `spec/conformance.md` (`spec/AUDIT-2.md` §5 step 6).
  Corrected expected diagnostics (B-02, B-14); added
  `ex.shared-readers`, `ex.projection-conflict`,
  `ex.shared-is-read-only`, `ex.move-while-borrowed`,
  `ex.field-reference`, `ex.resource-payload`, `ex.threads`.
- 1.0.0 and earlier — superseded.
