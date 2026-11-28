/* Threads, mutexes and the whole-state check: the stack floor and the
   program's own stack (D-0077); spawned threads, an OS thread kept for the
   next job (D-0202 phase 4); joins, failures and cancellation (D-0204);
   channels' event counts (spec/19 §3, D-0063); atomics' waits (spec/19
   §2a, D-0253); mutexes and guards (spec/19 §2, D-0200, D-0289, D-0310);
   the trace events (D-0264) and the whole-state check (D-0261). Part of
   lib.c's translation unit. */

#include <stdio.h>

#include "io.h"
#include "rt.h"

/* ---- text without printf (the C library's integer formats differ on
   Windows' msvcrt) ---- */

static void buf_u64(cb_buf *b, uint64_t v)
{
    char t[24];
    int i = 24;
    do
    {
        t[--i] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    cb_buf_put(b, t + i, (size_t)(24 - i));
}

/* `std::env::var`: the value when set and valid UTF-8, else NULL (Rust's
   `Err` for a value that is not Unicode). */
static bool utf8_ok(const uint8_t *s, size_t n)
{
    size_t i = 0;
    while (i < n)
    {
        uint8_t c = s[i];
        if (c < 0x80)
        {
            i++;
            continue;
        }
        size_t k;
        uint32_t cp;
        if (c >= 0xC2 && c <= 0xDF)
        {
            k = 1;
            cp = c & 0x1F;
        }
        else if (c >= 0xE0 && c <= 0xEF)
        {
            k = 2;
            cp = c & 0x0F;
        }
        else if (c >= 0xF0 && c <= 0xF4)
        {
            k = 3;
            cp = c & 0x07;
        }
        else
        {
            return false;
        }
        for (size_t j = 1; j <= k; j++)
        {
            if (i + j >= n || (s[i + j] & 0xC0) != 0x80)
            {
                return false;
            }
            cp = (cp << 6) | (s[i + j] & 0x3F);
        }
        if ((k == 2 && cp < 0x800) || (k == 3 && (cp < 0x10000 || cp > 0x10FFFF)) || (cp >= 0xD800 && cp <= 0xDFFF))
        {
            return false;
        }
        i += k + 1;
    }
    return true;
}

static const char *env_var(const char *name)
{
    const char *v = plat_getenv(name);
    if (!v || !utf8_ok((const uint8_t *)v, strlen(v)))
    {
        return NULL;
    }
    return v;
}

/* ---- the stack (D-0077) ---- */

/* The program's own stack, while it runs on one (`find_stack_floor`). */
static _Atomic uintptr_t OWN_STACK_BASE;
static _Atomic size_t OWN_STACK_SIZE;

/* The floor of a stack whose lowest address is `base` and whose size is
   `size`: a margin above `base` for the frames the runtime and foreign
   code make, which carry no check: a sixteenth of the stack, at least
   256 KiB, at most 16 MiB. */
uintptr_t floor_above(uintptr_t base, size_t size)
{
    size_t margin = size / 16;
    if (margin < ((size_t)256 << 10))
    {
        margin = (size_t)256 << 10;
    }
    if (margin > ((size_t)16 << 20))
    {
        margin = (size_t)16 << 20;
    }
    return base + margin;
}

/* D-0077: the lowest address this thread's stack may reach, less a margin
   (1: unknown, so unchecked). The program's own stack (`run_on_own_stack`)
   is not the thread's; otherwise the platform's account of the thread's
   stack (Linux: its attributes; Windows: the reservation `VirtualQuery`
   finds under a local, up to it). */
uintptr_t find_stack_floor(void)
{
    uintptr_t base = atomic_load_explicit(&OWN_STACK_BASE, memory_order_relaxed);
    size_t size = atomic_load_explicit(&OWN_STACK_SIZE, memory_order_relaxed);
    uintptr_t here = (uintptr_t)__builtin_frame_address(0);
    if (size != 0 && here > base && here < base + size)
    {
        return floor_above(base, size);
    }
    uintptr_t lo;
    size_t n;
    if (!plat_thread_stack(&lo, &n))
    {
        return 1;
    }
    return floor_above(lo, n);
}

/* Bug 032: the floor this thread had when an overflow its stack probes
   found was raised as `diag.stack-exhausted` (0: none). The fault is raised
   on the signal stack with the floor lifted, so that the unwind's own checks
   there pass; a contained fault puts it back (`cb_thread_failed`). */
static __thread uintptr_t LIFTED_FLOOR;
static __thread uintptr_t THREAD_FLOOR;

CB_NORET void cb_stack_exhausted(void);

static void stack_overflowed(void)
{
    LIFTED_FLOOR = THREAD_FLOOR;
    cb_set_stack_floor(1);
    cb_stack_exhausted();
}

/* Gives the generated program this thread's floor (`cb_stack_check`), and
   has an access below it that the system refuses (a frame too large for
   what is left) raise `diag.stack-exhausted` too. */
void set_stack_floor(void)
{
    uintptr_t f = find_stack_floor();
    if (f > 1)
    {
        cb_set_stack_floor(f);
        THREAD_FLOOR = f;
        uintptr_t lo = f;
        uintptr_t base = atomic_load_explicit(&OWN_STACK_BASE, memory_order_relaxed);
        size_t size = atomic_load_explicit(&OWN_STACK_SIZE, memory_order_relaxed);
        uintptr_t here = (uintptr_t)__builtin_frame_address(0);
        size_t n;
        if (size != 0 && here > base && here < base + size)
        {
            lo = base;
        }
        else if (!plat_thread_stack(&lo, &n))
        {
            lo = f;
        }
        plat_watch_stack(lo, f, stack_overflowed);
    }
}

/* A contained fault ended the thread's job (or a library entry's `_try`
   call): its floor as it was, and on Windows the stack's guard. */
void restore_stack_floor(void)
{
    if (LIFTED_FLOOR != 0)
    {
        cb_set_stack_floor(LIFTED_FLOOR);
        LIFTED_FLOOR = 0;
        plat_stack_recovered();
    }
}

#define PROGRAM_STACK ((size_t)(sizeof(void *) == 8 ? (256u << 20) : (64u << 20)))

/* A larger stack for the program than `PROGRAM_STACK`, asked for by the
   program's own C code before `main` starts (a constructor): the compiler
   does (impl/cobc/host.c), whose checker and lowering recurse on the
   nesting of the program they read. Reserved like the usual one, so only
   what is used is committed; where it cannot be reserved the usual one is.
   Not part of cbrt.h: generated C never calls it. */
static _Atomic size_t REQUESTED_STACK;

void cbrt_request_stack(size_t bytes)
{
    atomic_store_explicit(&REQUESTED_STACK, bytes, memory_order_relaxed);
}

static size_t program_stack(void)
{
    size_t r = atomic_load_explicit(&REQUESTED_STACK, memory_order_relaxed);
    return r > PROGRAM_STACK ? r : PROGRAM_STACK;
}

typedef struct
{
    void (*start)(void);
} program_job;

static void program_thread(void *arg)
{
    program_job *j = (program_job *)arg;
    j->start();
}

static void run_on_own_stack(uintptr_t job);

/* D-0077: the whole program runs on a large stack, reserved rather than
   committed, so that a deep recursion has room. On Linux
   (x86-64, AArch64) that is a stack of its own on the main thread
   (`own_stack` set): a process that has started a thread pays for it in
   every `malloc` and `free` from then on, so the program starts none until
   it spawns. Elsewhere, and in a build under a sanitizer, it is a thread
   with that stack. `start` ends the process itself (`cb_terminate_ok`, or
   a fault). */
CB_NORET void cb_run_program(void (*start)(void), int own_stack)
{
    if (own_stack != 0)
    {
        run_on_own_stack((uintptr_t)start);
    }
    static program_job job;
    job.start = start;
    plat_thread t;
    int made = plat_thread_spawn(&t, program_thread, &job, program_stack());
    if (made != 0 && program_stack() != PROGRAM_STACK)
    {
        made = plat_thread_spawn(&t, program_thread, &job, PROGRAM_STACK);
    }
    if (made == 0)
    {
        plat_thread_join(t);
        flush_at_end();
        plat_exit(101);
    }
    /* No thread could be made: run where we are. */
    start();
    plat_exit(0);
}

/* The first C frame on the program's own stack: as the thread's body, and
   as its `join` if `start` ever returned (RT-001: nothing can unwind
   through it). */
CB_NORET __attribute__((used)) void cbrt_own_stack_entry(uintptr_t job);
CB_NORET __attribute__((used)) void cbrt_own_stack_entry(uintptr_t job)
{
    void (*f)(void) = (void (*)(void))job;
    f();
    flush_at_end();
    plat_exit(101);
}

#if defined(__linux__) && (defined(__x86_64__) || defined(__aarch64__))
/* RT-001: the outermost frame of the program's own stack (first argument
   the job, second the stack's top). Its call frame information marks the
   return address undefined, so a walk of the stack (a debugger's or
   profiler's backtrace) ends here, at the end of the stack, instead of
   reading `cb_run_program`'s frame off a stack it is not on. */
#if defined(__x86_64__)
__asm__(".text\n"
        ".p2align 4\n"
        ".type cbrt_own_stack_trampoline,@function\n"
        "cbrt_own_stack_trampoline:\n"
        ".cfi_startproc\n"
        ".cfi_undefined rip\n"
        "mov %rsi, %rsp\n"
        "xor %ebp, %ebp\n"
        "call cbrt_own_stack_entry@PLT\n"
        "ud2\n"
        ".cfi_endproc\n"
        ".size cbrt_own_stack_trampoline, .-cbrt_own_stack_trampoline\n");
#else
__asm__(".text\n"
        ".p2align 4\n"
        ".type cbrt_own_stack_trampoline,%function\n"
        "cbrt_own_stack_trampoline:\n"
        ".cfi_startproc\n"
        ".cfi_undefined x30\n"
        "mov sp, x1\n"
        "mov x29, xzr\n"
        "mov x30, xzr\n"
        "bl cbrt_own_stack_entry\n"
        "brk #1\n"
        ".cfi_endproc\n"
        ".size cbrt_own_stack_trampoline, .-cbrt_own_stack_trampoline\n");
#endif
CB_NORET void cbrt_own_stack_trampoline(uintptr_t job, uintptr_t top);

/* Reserves the stack (a guard below it) and moves this thread onto it;
   returns only if the reservation failed, for the thread to be made
   instead. */
static void run_on_own_stack(uintptr_t job)
{
    const size_t guard = (size_t)64 << 10;
    size_t size = program_stack();
    uintptr_t base = plat_reserve_stack(size, guard);
    if (base == 0 && size != PROGRAM_STACK)
    {
        size = PROGRAM_STACK;
        base = plat_reserve_stack(size, guard);
    }
    if (base == 0)
    {
        return;
    }
    atomic_store_explicit(&OWN_STACK_BASE, base, memory_order_relaxed);
    atomic_store_explicit(&OWN_STACK_SIZE, size, memory_order_relaxed);
    uintptr_t top = (base + size) & ~(uintptr_t)15;
    cbrt_own_stack_trampoline(job, top);
}
#else
static void run_on_own_stack(uintptr_t job)
{
    (void)job;
}
#endif

/* D-0077: `cb_stack_check` found the stack exhausted: the fault is at the
   statement making the call. */
CB_NORET void cb_stack_exhausted(void)
{
    uint32_t f, l;
    get_at(&f, &l);
    /* Report 166: the unwind that follows runs destructors, whose calls
       check the floor too: it is lifted, so they run in the margin below it
       (a sixteenth of the stack, at least 256 KiB), as on the signal stack
       after an overflow the probes found; a contained fault puts it back
       (`restore_stack_floor`). */
    if (LIFTED_FLOOR == 0 && THREAD_FLOOR > 1)
    {
        LIFTED_FLOOR = THREAD_FLOOR;
        cb_set_stack_floor(1);
    }
    if (DROPS_RUNNING > 0)
    {
        fault_without_unwind("diag.stack-exhausted", f, l);
    }
    fault("diag.stack-exhausted", f, l);
}

/* ---- threads (spec/19 §1) ---- */

/* A spawned thread's argument block, zeroed; its result goes at offset 0.
   Aligned as its C struct is (D-0255); the block's own allocation is kept
   for `free_env` (`ENV_LAYOUTS`: the address of the block `malloc` gave,
   which the aligned block lies in). */
static plat_mutex ENV_LOCK = PLAT_MUTEX_INIT;
static map_u64 ENV_LAYOUTS;

void *cb_env_alloc(uint64_t size, uint64_t align)
{
    uint64_t sz = size ? size : 1;
    uint64_t al = align < 16 ? 16 : align;
    /* `Layout::from_size_align`: a power-of-two alignment, and a size that
       rounded up to it stays within `isize::MAX`. */
    if ((al & (al - 1)) != 0 || sz > (uint64_t)INTPTR_MAX - (al - 1))
    {
        return NULL;
    }
    size_t total = (size_t)sz + (size_t)al;
    uint8_t *raw = (uint8_t *)calloc(1, total);
    if (!raw)
    {
        return NULL;
    }
    uintptr_t p = ((uintptr_t)raw + (uintptr_t)al - 1) & ~(uintptr_t)(al - 1);
    plat_mutex_lock(&ENV_LOCK);
    map_u64_insert(&ENV_LAYOUTS, p, (uint64_t)(uintptr_t)raw, NULL);
    plat_mutex_unlock(&ENV_LOCK);
    return (void *)p;
}

/* The threads running, the main one included, and the next thread's id
   (the main thread's is 1). Exported (D-0209); `cb_threaded` (D-0269): 1
   once a thread has been spawned. */
_Atomic uint64_t cb_live_threads = 1;
_Atomic uint8_t cb_threaded;
_Atomic uint64_t NEXT_TID = 2;

/* `cb_thread_id`'s slow half: the calling thread's number (`TID`) once a thread has been spawned. */
uint64_t cb_thread_here(void)
{
    return TID;
}

/* A program thread's job: what `spawn` hands an OS thread (`worker`). */
typedef struct
{
    uint64_t tid;
    void (*body)(void *);
    void *env;
    loc_t at;
} Job;

/* An OS thread waiting for its next job. */
typedef struct
{
    plat_mutex m;
    plat_cond cv;
    bool has;
    Job job;
} Idle;

typedef struct
{
    Job first;
    Idle *me;
} worker_start;

VEC(idlevec, Idle *)

static plat_mutex IDLE_LOCK = PLAT_MUTEX_INIT;
static idlevec IDLE;

/* How many finished OS threads wait for more work at most; more end. */
#define IDLE_MAX 64

static void worker(void *arg);
static void forget_thread(uint64_t tid);
static void threads_at_end(void);

/* `[Spawn]`: the arguments are already in the block; the new thread runs
   `body(env)`, which makes the call and reports its result (on an OS
   thread that finished an earlier job, if one waits: `worker`). */
uint64_t cb_spawn(void (*body)(void *), void *env, uint64_t env_size, uint32_t res_ty)
{
    replay_out(me(), "threads");
    /* D-0269: from here on a function notes its objectless locals. */
    atomic_store_explicit(&cb_threaded, 1, memory_order_release);
    /* Each thread its own heap of small blocks from here on. */
    alloc_threads_on();
    /* The handle this returns is ended by: */
    if (!HOOK(HANDLE_END))
    {
        HOOK_SET(HANDLE_END, handle_destructor);
        HOOK_SET(THREADS_AT_END, threads_at_end);
    }
    /* D-0204: the thread's id, its record in the book and the count of
       running threads need no hold on the runtime; only the first spawn,
       which turns locking on (`go_multi`), enters it. */
    ENTERED_SCOPE e = ENT_GONE;
    if (!multi())
    {
        e = enter();
    }
    uint64_t tid = atomic_fetch_add_explicit(&NEXT_TID, 1, memory_order_acq_rel);
    if (sched_on())
    {
        sched_add(tid);
        SCHED_PENDING = true;
    }
    {
        Book *b = book();
        ThreadRec rec = {ST_RUNNING, 0, (uintptr_t)env, env_size, res_ty, 0, false};
        threadmap_insert(&b->threads, tid, rec, NULL);
        book_unlock();
    }
    atomic_fetch_add_explicit(&cb_live_threads, 1, memory_order_acq_rel);
    go_multi();
    Job job;
    job.tid = tid;
    job.body = body;
    job.env = env;
    get_at(&job.at.f, &job.at.l);
    /* D-0202 phase 4: an OS thread that finished a job takes this one, if
       one waits; otherwise a new thread. */
    Idle *w = NULL;
    plat_mutex_lock(&IDLE_LOCK);
    if (IDLE.n)
    {
        w = IDLE.p[--IDLE.n];
    }
    plat_mutex_unlock(&IDLE_LOCK);
    if (w)
    {
        plat_mutex_lock(&w->m);
        w->job = job;
        w->has = true;
        plat_mutex_unlock(&w->m);
        plat_cond_signal(&w->cv);
    }
    else
    {
        Idle *n = (Idle *)cb_xcalloc(1, sizeof(Idle));
        plat_mutex_init(&n->m);
        plat_cond_init(&n->cv);
        worker_start *s = (worker_start *)cb_xmalloc(sizeof(worker_start));
        s->first = job;
        s->me = n;
        /* A program thread recurses as the main one does: a generous stack,
           reserved rather than committed (D-0077 checks its floor). */
        plat_thread t;
        size_t stack = sizeof(void *) == 8 ? ((size_t)64 << 20) : ((size_t)16 << 20);
        if (plat_thread_spawn(&t, worker, s, stack) == 0)
        {
            plat_thread_detach(t);
        }
        else
        {
            free(s);
            free(n);
        }
    }
    (void)e;
    return tid;
}

static void run_job(Job job);

/* An OS thread: its first job, then each one handed to it while it waits
   in `IDLE`. Every job starts as a new thread did and leaves nothing
   behind. */
static void worker(void *arg)
{
    worker_start *s = (worker_start *)arg;
    Job job = s->first;
    Idle *self = s->me;
    free(s);
    set_stack_floor();
    thread_known();
    for (;;)
    {
        run_job(job);
        plat_mutex_lock(&IDLE_LOCK);
        if (IDLE.n >= IDLE_MAX)
        {
            plat_mutex_unlock(&IDLE_LOCK);
            free(self);
            alloc_thread_end();
            thread_locals_free();
            plat_unwatch_stack();
            return;
        }
        idlevec_push(&IDLE, self);
        plat_mutex_unlock(&IDLE_LOCK);
        plat_mutex_lock(&self->m);
        while (!self->has)
        {
            plat_cond_wait(&self->cv, &self->m);
        }
        job = self->job;
        self->has = false;
        plat_mutex_unlock(&self->m);
    }
}

TState *tstate_new(void)
{
    TState *t = (TState *)cb_xcalloc(1, sizeof(TState));
    t->dloc.f = NOLOC;
    t->dloc.l = 0;
    return t;
}

void tstate_free(TState *t)
{
    for (size_t i = 0; i < t->stack.n; i++)
    {
        Entry *e = &t->stack.p[i];
        if (e->kind == E_FRAME)
        {
            u64vec_free(&e->u.owned);
        }
        else
        {
            u64vec_free(&e->u.s.temps);
            u64vec_free(&e->u.s.toks);
        }
    }
    entryvec_free(&t->stack);
    sizevec_free(&t->scopes);
    for (size_t i = 0; i < t->inflight.n; i++)
    {
        Inflight *f = infq_at(&t->inflight, i);
        if (f->kind == IF_DATUM)
        {
            u64vec_free(&f->datum);
        }
    }
    free(t->inflight.p);
    locvec_free(&t->pend);
    notevec_free(&t->notes);
    free(t->box_more);
    free(t->chain);
    held_free(t);
    free(t->lib_fault);
    free(t);
}

static void run_job(Job job)
{
    TID = job.tid;
    {
        Book *b = book();
        map_u64_insert(&b->os, job.tid, netio_os_thread(), NULL);
        book_unlock();
    }
    if (sched_on())
    {
        sched_wait(job.tid, sched_lock());
    }
    /* D-0202: the thread's own heap. */
    {
        Entered e = enter();
        uint8_t h = claim_heap(rt());
        HEAP = h;
        set_heap(rt(), h);
        entered_drop(e);
    }
    TState *own = tstate_new();
    set_at(job.at.f, job.at.l);
    TSP = own;
    held_register(job.tid, own);
    job.body(job.env);
    /* Report 315: its list of held mutexes no longer found by others. */
    held_unregister(job.tid);
    Entered e = enter();
    atomic_fetch_sub_explicit(&cb_live_threads, 1, memory_order_acq_rel);
    /* D-0202: its heap is free for another thread once empty; used by one
       thread again, it is no longer shared. */
    size_t h = HEAP;
    if (h != 0)
    {
        Rt *r = rt();
        uint64_t u = r->heap_users.p[h];
        r->heap_users.p[h] = u ? u - 1 : 0;
        if (r->heap_users.p[h] <= 1)
        {
            atomic_store_explicit(&HEAP_SHARED[h], false, memory_order_release);
        }
    }
    /* `live` counts the main thread: 1 is none spawned still running. */
    if (atomic_load_explicit(&cb_live_threads, memory_order_acquire) == 1)
    {
        atomic_store_explicit(&SOLO_PENDING, true, memory_order_relaxed);
    }
    TSP = NULL;
    tstate_free(own);
    entered_drop(e);
    {
        Book *b = book();
        map_u64_remove(&b->os, job.tid, NULL);
        book_unlock();
    }
    if (sched_on())
    {
        sched_leave(job.tid);
    }
}

/* `[Thread-Body-Done]`: the result (and its object, for a resource) is in
   the block, waiting to be claimed. */
void cb_thread_done(uint64_t res_obj)
{
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, me());
        if (t)
        {
            t->status = ST_DONE;
            t->res_obj = res_obj;
        }
        /* `[Deadlock]` (b) is judged over the threads still running: one
           fewer now, and the rest may all wait (D-0219). */
        book_find_deadlock(b);
        book_unlock();
    }
    signal_key(me());
}

/* A resource argument waiting in a spawned thread's block: detached from
   the spawning statement, owned by nothing until the new thread sends it
   to the callee. */
void cb_hold(uint64_t obj)
{
    ENTERED_SCOPE e = enter();
    detach(rt(), obj);
    (void)e;
}

/* The argument block's references stop being occurrences once the call
   that received them has returned. */
void cb_env_forget(void *addr, uint64_t size)
{
    ENTERED_SCOPE e = enter();
    forget_slots(rt(), (uintptr_t)addr, size);
    (void)e;
}

/* The spawned callee, a `fn` value, is destroyed once its call is over. */
void cb_drop_fn(void *handle)
{
    ENTERED_SCOPE e = enter();
    drop_fn_handle((uintptr_t)handle);
    (void)e;
}

static bool done_ready(Book *b, void *ctx)
{
    uint64_t tid = *(uint64_t *)ctx;
    ThreadRec *t = threadmap_get(&b->threads, tid);
    return !t || t->status != ST_RUNNING;
}

void wait_done(uint64_t tid)
{
    Wait w = {W_JOIN, tid, 0};
    wait_until(tid, w, done_ready, &tid);
}

/* The block's remaining references stop being occurrences, the block is
   freed, and the result counts as taken. */
void free_env(uint64_t tid)
{
    Rt *r = rt();
    uintptr_t env;
    uint64_t size;
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, tid);
        if (!t)
        {
            book_unlock();
            return;
        }
        env = t->env;
        size = t->env_size;
        book_unlock();
    }
    forget_slots(r, env, size);
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, tid);
        if (t)
        {
            t->status = ST_TAKEN;
            t->env = 0;
        }
        book_unlock();
    }
    if (env != 0)
    {
        uint64_t raw;
        plat_mutex_lock(&ENV_LOCK);
        bool had = map_u64_remove(&ENV_LAYOUTS, env, &raw);
        plat_mutex_unlock(&ENV_LOCK);
        if (had)
        {
            free((void *)(uintptr_t)raw);
        }
    }
}

/* `[Join]`: waits, claims the result into `dst` (its object, if any,
   moving with it into this statement), and frees the block. Returns the
   result's object, or 0. */
uint64_t cb_join(uint64_t tid, void *dst)
{
    ENTERED_SCOPE e = enter();
    (void)e;
    ts()->joining = tid;
    wait_done(tid);
    ts()->joining = 0;
    /* D-0204: `[Join-Failed]`: the thread's contained fault, raised again
       here. */
    bool failed = false;
    uint64_t fid = 0;
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, tid);
        if (t && t->status == ST_FAILED)
        {
            failed = true;
            fid = t->failed;
        }
        book_unlock();
    }
    if (failed)
    {
        free_env(tid);
        forget_thread(tid);
        reraise(fid);
    }
    Rt *r = rt();
    uintptr_t env;
    uint32_t res_ty;
    uint64_t res_obj;
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, tid);
        if (!t || t->status != ST_DONE)
        {
            book_unlock();
            cbrt_internal("join of a thread whose result was already taken");
        }
        env = t->env;
        res_ty = t->res_ty;
        res_obj = t->res_obj;
        book_unlock();
    }
    uint64_t size = rt_typ(r, res_ty)->size;
    if (size > 0)
    {
        memcpy(dst, (const void *)env, (size_t)size);
    }
    res_obj = res_obj != 0 ? migrate(r, res_obj) : 0;
    if (res_obj != 0)
    {
        cb_move_to(res_obj, dst);
        Rt *r2 = rt();
        size_t scope = top_scope(r2);
        stamp_temp(r2, res_obj, scope);
    }
    else
    {
        rekey_slots(r, env, (uintptr_t)dst, size);
    }
    free_env(tid);
    /* Report 236: the handle is consumed: its record leaves the book now,
       not when a handle's destructor (which a decided program does not run
       for a joined handle) finds it. */
    forget_thread(tid);
    return res_obj;
}

/* `[Handle-Destructor]`: waits; an unclaimed result is discarded. During a
   fault's (or `exit`'s) unwind nothing waits: the other threads take no
   further steps (`[Fault-Unwind]`); the thread is abandoned with the
   process. */
void handle_destructor(uint64_t tid)
{
    if (rt()->unwinding)
    {
        Book *b = book();
        threadmap_remove(&b->threads, tid, NULL);
        book_unlock();
        return;
    }
    /* Report 134: the handle is destroyed on the thread it names (the
       thread held it, through an `Arc`): waiting would be waiting for
       itself. Nothing waits for the thread now but the program's end
       (`threads_at_end`), which raises a failure of it. */
    if (tid == me())
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, tid);
        if (t)
        {
            t->self_owned = true;
        }
        book_unlock();
        return;
    }
    /* D-0204: while a contained fault unwinds this thread, its threads are
       cancelled first, then waited for: none outlives its handle. */
    bool unwinding = ts_raw()->cunwind;
    if (unwinding)
    {
        request_cancel(tid);
    }
    wait_done(tid);
    Rt *r = rt();
    uint32_t status;
    uint64_t fid, res_obj;
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, tid);
        if (!t)
        {
            book_unlock();
            return;
        }
        status = t->status;
        fid = t->failed;
        res_obj = t->res_obj;
        book_unlock();
    }
    /* D-0340 §6: a handle ended without `join` whose thread gave a
       `Result`'s `Err` loses it: that is the fault `diag.result-discarded`
       (a handle binding is refused statically; one in a container is
       decided here). */
    if (status == ST_DONE && !unwinding)
    {
        uintptr_t at = 0;
        uint32_t rty = NOTYPE;
        if (res_obj != 0)
        {
            Obj *o = objs_get(r, res_obj);
            if (o != NULL)
            {
                at = o->addr;
                rty = o->ty;
            }
        }
        else
        {
            Book *b = book();
            ThreadRec *t = threadmap_get(&b->threads, tid);
            if (t)
            {
                at = t->env;
                rty = t->res_ty;
            }
            book_unlock();
        }
        if (at != 0 && rty != NOTYPE)
        {
            TypeInfo *info = rt_typ(r, rty);
            if (info->kind == K_ENUM && info->name != NULL && strncmp(info->name, "std__Result_L", 13) == 0 && enum_tag(info, at) == 1)
            {
                cb_fault("diag.result-discarded", 0, 0);
            }
        }
    }
    if (status == ST_DONE && res_obj != 0 && objs_get(r, res_obj) != NULL)
    {
        destroy_obj(res_obj, 0);
    }
    /* D-0315 (step 4): a decided program's result whose end frees memory
       alone came back as its bytes, with no object (`cb_thread_done(0)`):
       unclaimed, it ends by its type's drop (the block holds it first). */
    if (status == ST_DONE && res_obj == 0 && atomic_load_explicit(&DECIDED_PROGRAM, memory_order_relaxed))
    {
        bool found = false;
        uintptr_t env = 0;
        uint32_t res_ty = 0;
        {
            Book *b = book();
            ThreadRec *t = threadmap_get(&b->threads, tid);
            if (t)
            {
                found = true;
                env = t->env;
                res_ty = t->res_ty;
            }
            book_unlock();
        }
        if (found && env != 0 && rt_typ(r, res_ty)->is_resource)
        {
            drop_in_place(env, res_ty, 0);
        }
    }
    if (status == ST_DONE || status == ST_FAILED)
    {
        free_env(tid);
    }
    bool asked;
    {
        Book *b = book();
        asked = hset_has(&b->cancel, tid);
        book_unlock();
    }
    {
        Book *b = book();
        threadmap_remove(&b->threads, tid, NULL);
        book_unlock();
    }
    /* A failure nobody took is the destroyer's, raised again here -- unless
       this thread is unwinding already, or asked for the stop. */
    if (status == ST_FAILED)
    {
        bool cancelled;
        {
            Book *b = book();
            const char *raw = b->failures.p[fid].raw;
            cancelled = strncmp(raw, "diag.thread-cancelled", 21) == 0;
            book_unlock();
        }
        if (!unwinding && !(asked && cancelled))
        {
            reraise(fid);
        }
        Book *b = book();
        failure_free(b, fid);
        book_unlock();
    }
}

/* Reports 070, 169: a contained failure in a few words, for a fault it
   caused in another thread: "`diag.x`: its message, at FILE:LINE". Under
   the book's lock. */
void failure_brief(cb_buf *b, Book *bk, uint64_t fid)
{
    if (fid >= bk->failures.n || bk->failures.p[fid].raw == NULL)
    {
        cb_buf_str(b, "a fault");
        return;
    }
    Failure *f = &bk->failures.p[fid];
    fault_brief(b, f->raw, f->file, f->line);
}

/* The same of a fault's raw text and location. */
void fault_brief(cb_buf *b, const char *raw, uint32_t file, uint32_t line)
{
    const char *sep = strchr(raw, DETAIL_SEP);
    cb_buf_str(b, "`");
    cb_buf_put(b, raw, sep ? (size_t)(sep - raw) : strlen(raw));
    cb_buf_str(b, "`");
    if (sep)
    {
        cb_buf_str(b, " (");
        cb_buf_str(b, sep + 1);
        cb_buf_str(b, ")");
    }
    Rt *r = rt();
    if (file != NOLOC && file < r->nfiles)
    {
        char at[64];
        snprintf(at, sizeof at, ":%u", (unsigned)line);
        cb_buf_str(b, " at ");
        cb_buf_str(b, r->files[file]);
        cb_buf_str(b, at);
    }
}

/* Report 238: a failure nothing can name any more (raised again, or
   discarded, or its `ThreadFailure` dropped): its texts freed and its
   index free for the next failure. */
void failure_free(Book *b, uint64_t fid)
{
    if (fid >= b->failures.n || b->failures.p[fid].raw == NULL)
    {
        return;
    }
    Failure *f = &b->failures.p[fid];
    free(f->raw);
    free(f->text);
    f->raw = NULL;
    f->text = NULL;
    u64vec_push(&b->free_failures, fid);
}

/* Report 236: a thread's record leaves the book once its handle is
   consumed. */
static void forget_thread(uint64_t tid)
{
    Book *b = book();
    threadmap_remove(&b->threads, tid, NULL);
    hset_remove(&b->cancel, tid, NULL);
    hset_remove(&b->lent, tid, NULL);
    book_unlock();
}

/* Report 134 (`[Terminate-Ok]`: no other thread is live at the end): the
   threads still running when `main` has returned are those whose handle
   no frame of the main thread owns (one the thread held itself, through an
   `Arc`): each is waited for, and a failure of one is the program's. */
static void threads_at_end(void)
{
    for (;;)
    {
        uint64_t tid = 0;
        uint64_t fid = 0;
        bool failed = false;
        {
            Book *b = book();
            for (size_t i = 0; i < b->threads.cap; i++)
            {
                if (!b->threads.used[i])
                {
                    continue;
                }
                ThreadRec *t = &b->threads.e[i].v;
                if (t->status == ST_RUNNING)
                {
                    tid = b->threads.e[i].k;
                    break;
                }
                if (t->status == ST_FAILED && t->self_owned)
                {
                    tid = b->threads.e[i].k;
                    fid = t->failed;
                    failed = true;
                    break;
                }
            }
            book_unlock();
        }
        if (tid == 0)
        {
            return;
        }
        if (failed)
        {
            forget_thread(tid);
            reraise(fid);
        }
        ENTERED_SCOPE e = enter();
        (void)e;
        wait_done(tid);
        bool keep = false;
        {
            Book *b = book();
            ThreadRec *t = threadmap_get(&b->threads, tid);
            keep = t && t->status == ST_FAILED && t->self_owned;
            book_unlock();
        }
        if (!keep)
        {
            free_env(tid);
            forget_thread(tid);
        }
    }
}

/* A contained failure raised again in this thread (`[Join-Failed]`): the
   same fault, at its own location. */
CB_NORET void reraise(uint64_t fid)
{
    char *raw;
    uint32_t file, line;
    {
        Book *b = book();
        Failure *f = &b->failures.p[fid];
        size_t n = strlen(f->raw);
        raw = (char *)cb_xmalloc(n + 1);
        memcpy(raw, f->raw, n + 1);
        file = f->file;
        line = f->line;
        failure_free(b, fid);
        book_unlock();
    }
    fault_owned(raw, file, line);
}

/* D-0204: `std::try_join`'s and `std::is_finished`'s primitive: 0 while
   the thread runs, 1 when it finished, its failure's id plus 2 when it
   failed. Mode 0 waits for the end and takes a failure; mode 1 only
   looks. */
uint64_t cb_join_status(uint64_t tid, uint8_t mode)
{
    if (mode == 1)
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, tid);
        uint64_t v = 1;
        if (t && t->status == ST_FAILED)
        {
            v = t->failed + 2;
        }
        else if (t && t->status == ST_RUNNING)
        {
            v = 0;
        }
        book_unlock();
        return v;
    }
    ENTERED_SCOPE e = enter();
    (void)e;
    if (mode == 0)
    {
        ts()->joining = tid;
        wait_done(tid);
        ts()->joining = 0;
    }
    bool has = false;
    ThreadRec st;
    {
        Book *b = book();
        ThreadRec *t = threadmap_get(&b->threads, tid);
        if (t)
        {
            has = true;
            st = *t;
        }
        book_unlock();
    }
    if (has && st.status == ST_FAILED)
    {
        if (mode == 0)
        {
            free_env(tid);
        }
        return st.failed + 2;
    }
    if (has && st.status == ST_RUNNING)
    {
        return 0;
    }
    return 1;
}

/* D-0204: `cancel(&h)`: the thread's current or next wait faults
   (`diag.thread-cancelled`), unless it was lent an exclusive reference. */
void cb_cancel(uint64_t tid)
{
    request_cancel(tid);
}

/* Marks `tid` cancelled and wakes it: a runtime wait through its event key,
   a socket wait through its OS thread (under the book's lock, so the
   thread cannot have ended in between). */
void request_cancel(uint64_t tid)
{
    atomic_fetch_add_explicit(&CANCELS, 1, memory_order_acq_rel);
    bool has;
    uint64_t key = 0;
    {
        Book *b = book();
        hset_add(&b->cancel, tid);
        uint64_t *t = map_u64_get(&b->os, tid);
        if (t)
        {
            netio_wake_thread(*t);
        }
        uint64_t *k = map_u64_get(&b->wake, tid);
        has = k != NULL;
        if (k)
        {
            key = *k;
        }
        book_unlock();
    }
    if (has)
    {
        signal_key(key);
    }
}

/* D-0204: a spawned thread's trampoline gives the runtime the buffer its
   `setjmp` filled: a contained fault's unwind ends there. */
void cb_thread_jmp(void *jb)
{
    jump_plainly(jb);
    ts()->jmp = (uintptr_t)jb;
}

typedef struct
{
    Rt *r;
    bool lent;
} lent_ctx;

static void lent_each(uint64_t t, void *c)
{
    lent_ctx *x = (lent_ctx *)c;
    /* A guard's lock path is not lent, and stays its locking thread's
       (D-0240): holding one does not make the thread lent (CHG-0304). */
    PathRec *p = paths_get(x->r, t);
    if (p && p->exclusive && !p->has_lock_of)
    {
        x->lent = true;
    }
}

/* D-0204: as the thread starts, whether it was lent an exclusive
   reference: one held in its argument block, or in the captures of the
   closure it runs. */
void cb_thread_start(void *env, uint64_t size, void *h)
{
    ENTERED_SCOPE e = enter();
    (void)e;
    Rt *r = rt();
    lent_ctx c = {r, false};
    uintptr_t lo = (uintptr_t)env;
    slots_each_in(r, lo, lo + (uintptr_t)size, lent_each, &c);
    if (h && map_u64_has(&r->fn_boxes, (uint64_t)(uintptr_t)h))
    {
        const CbFnBox *b = (const CbFnBox *)h;
        uintptr_t d = (uintptr_t)b->data;
        slots_each_in(r, d, d + (uintptr_t)rt_typ(r, b->ty)->size, lent_each, &c);
    }
    if (c.lent)
    {
        uint64_t m = me();
        Book *b = book();
        hset_add(&b->lent, m);
        book_unlock();
    }
}

/* D-0204: this thread locked a mutex whose interior may hold an exclusive
   reference: it may now change data that is not its own, so it is never
   cancelled and its fault is not contained. */
void cb_thread_lent(void)
{
    uint64_t m = me();
    if (m != 1)
    {
        Book *b = book();
        hset_add(&b->lent, m);
        book_unlock();
    }
}

/* D-0204: the trampoline's last step after a contained fault: the thread's
   result is the failure. */
void cb_thread_failed(void)
{
    restore_stack_floor();
    TState *t = ts();
    uint64_t fid = t->has_failed ? t->failed : 0;
    t->has_failed = false;
    t->failed = 0;
    {
        Book *b = book();
        ThreadRec *rec = threadmap_get(&b->threads, me());
        if (rec)
        {
            rec->status = ST_FAILED;
            rec->failed = fid;
        }
        book_find_deadlock(b); /* as `cb_thread_done` (D-0219) */
        book_unlock();
    }
    signal_key(me());
}

/* D-0204: `std::task_op`: a contained failure's report (op 0) or its
   diagnostic's name (op 1), at most `cap` bytes at `out`; the whole length
   back. */
int64_t cb_task_op(uint64_t op, uint64_t id, void *out, uint64_t cap)
{
    /* Report 238: op 2, the failure's `ThreadFailure` is dropped. */
    if (op == 2)
    {
        Book *b = book();
        failure_free(b, id);
        book_unlock();
        return 0;
    }
    cb_buf text = {0};
    {
        Book *b = book();
        if (id < b->failures.n && b->failures.p[id].raw != NULL)
        {
            Failure *f = &b->failures.p[id];
            if (op == 0)
            {
                cb_buf_str(&text, f->text ? f->text : "");
            }
            else
            {
                const char *s = f->raw;
                const char *sep = strchr(s, '\x01');
                cb_buf_put(&text, s, sep ? (size_t)(sep - s) : strlen(s));
            }
        }
        book_unlock();
    }
    if ((uint64_t)text.n <= cap && text.n)
    {
        memcpy(out, text.p, text.n);
    }
    int64_t n = (int64_t)text.n;
    cb_buf_free(&text);
    return n;
}

/* ---- channels' event counts (spec/19 §3, D-0063) ---- */

uint64_t event_key(uint64_t id)
{
    return id | (1ull << 62);
}

typedef struct
{
    uint64_t id;
    uint64_t seen;
    bool is_main;
} event_ctx;

static bool event_ready(Book *b, void *ctx)
{
    event_ctx *c = (event_ctx *)ctx;
    uint64_t v = 0;
    if (!(stripe_event_get(c->id, &v) && v == c->seen))
    {
        return true;
    }
    if (c->is_main && !hset_has(&b->deadlocked, 1))
    {
        for (size_t i = 0; i < b->threads.cap; i++)
        {
            if (b->threads.used[i] && b->threads.e[i].v.status == ST_RUNNING)
            {
                return false;
            }
        }
        return true;
    }
    return false;
}

/* Report 132: a waiter's wake-up after a change made under a stripe alone:
   the book is taken and let go, so a waiter that found nothing changed
   under it has registered for the signal (`wait_until`), and then the
   key is signalled. */
static void wake_waiters(uint64_t key)
{
    Book *b = book();
    (void)b;
    book_unlock();
    signal_key(key);
}

/* Event count `id` one more; its waiters woken. */
static uint64_t event_add(uint64_t id)
{
    Stripe *s = stripe_of(id);
    bool had;
    uint64_t *v = map_u64_slot(&s->events, id, &had);
    if (!had)
    {
        *v = 0;
    }
    *v += 1;
    uint64_t c = *v;
    bool waited = map_u64_has(&s->ewait, id);
    stripe_unlock(s);
    if (waited)
    {
        wake_waiters(event_key(id));
    }
    else if (sched_on())
    {
        SCHED_PENDING = true;
    }
    return c;
}

/* ---- report 315: the mutexes each thread locked ----

   Kept in the thread's own state (`TState::held`), for `[Fault-Contain]`,
   which poisons them (`poison_held`). The thread alone adds and removes
   its entries, with no lock. A guard may be moved to another thread and
   ended there (`conf.guard-moved-to-thread-and-dropped`): that thread
   clears the locking thread's entry (its key set to 0) under the book's
   lock, before it lets the mutex go, so no entry outlives its lock and
   `poison_held`, under the book's lock too, never meets a mutex that has
   ended. An entry's key is atomic for that reason; the thread that owns
   the list drops cleared entries at its top. */
static HeldMx *held_at(TState *t, uint32_t i)
{
    HeldChunk *c = &t->held;
    while (i >= HELD_CHUNK)
    {
        c = __atomic_load_n(&c->next, __ATOMIC_ACQUIRE);
        i -= HELD_CHUNK;
    }
    return &c->e[i];
}

static CB_COLD HeldMx *held_grow(TState *t, uint32_t i)
{
    HeldChunk *c = &t->held;
    while (i >= HELD_CHUNK)
    {
        HeldChunk *n = __atomic_load_n(&c->next, __ATOMIC_ACQUIRE);
        if (!n)
        {
            n = (HeldChunk *)cb_xcalloc(1, sizeof(HeldChunk));
            __atomic_store_n(&c->next, n, __ATOMIC_RELEASE);
        }
        c = n;
        i -= HELD_CHUNK;
    }
    return &c->e[i];
}

uint64_t mutex_holder(uintptr_t cells)
{
    return __atomic_load_n((uint64_t *)cells, __ATOMIC_ACQUIRE) & MX_HOLDER;
}

static inline void held_push(uintptr_t key, uintptr_t cells, uint64_t m)
{
    (void)m;
    TState *t = ts_raw();
    uint32_t n = __atomic_load_n(&t->nheld, __ATOMIC_RELAXED);
    /* Entries cleared by another thread at the top go first. */
    while (n > 0 && __atomic_load_n(&held_at(t, n - 1)->key, __ATOMIC_ACQUIRE) == 0)
    {
        n--;
    }
    HeldMx *h = LIKELY(n < HELD_CHUNK) ? &t->held.e[n] : held_grow(t, n);
    h->cells = cells;
    __atomic_store_n(&h->key, key, __ATOMIC_RELEASE);
    __atomic_store_n(&t->nheld, n + 1, __ATOMIC_RELEASE);
}

/* This thread's entry for the mutex at `key`, cleared; its state cells,
   or 0 when this thread has none (a guard moved here from the thread that
   locked it). */
static inline uintptr_t held_pop(uintptr_t key)
{
    TState *t = ts_raw();
    uint32_t n = __atomic_load_n(&t->nheld, __ATOMIC_RELAXED);
    for (uint32_t i = n; i-- > 0;)
    {
        HeldMx *h = held_at(t, i);
        if (__atomic_load_n(&h->key, __ATOMIC_ACQUIRE) == key)
        {
            uintptr_t c = h->cells;
            __atomic_store_n(&h->key, 0, __ATOMIC_RELEASE);
            while (n > 0 && __atomic_load_n(&held_at(t, n - 1)->key, __ATOMIC_ACQUIRE) == 0)
            {
                n--;
            }
            __atomic_store_n(&t->nheld, n, __ATOMIC_RELEASE);
            return c;
        }
    }
    return 0;
}

/* The state of thread `tid`, for clearing its entry, or NULL once it has
   ended. Under the book's lock. */
static TState *held_owner(Book *b, uint64_t tid)
{
    if (tid == 1)
    {
        return &MAIN_TS;
    }
    void **p = map_ptr_get(&b->held_lists, tid);
    return p ? (TState *)*p : NULL;
}

/* A thread's state, findable by its id while it runs (`held_owner`). */
void held_register(uint64_t tid, TState *t)
{
    Book *b = book();
    map_ptr_insert(&b->held_lists, tid, t, NULL);
    book_unlock();
}

/* ... and no longer: called before the state is freed. */
void held_unregister(uint64_t tid)
{
    Book *b = book();
    map_ptr_remove(&b->held_lists, tid, NULL);
    book_unlock();
}

void held_free(TState *t)
{
    HeldChunk *c = t->held.next;
    while (c)
    {
        HeldChunk *n = c->next;
        free(c);
        c = n;
    }
    t->held.next = NULL;
    t->nheld = 0;
}

/* Under the book's lock (`release_locks_of`). */
void poison_held(uint64_t m, u64vec *keys, u64vec *cells)
{
    TState *t = ts_raw();
    uint32_t n = __atomic_load_n(&t->nheld, __ATOMIC_ACQUIRE);
    for (uint32_t i = 0; i < n; i++)
    {
        HeldMx *h = held_at(t, i);
        uintptr_t key = __atomic_load_n(&h->key, __ATOMIC_ACQUIRE);
        if (key == 0)
        {
            continue;
        }
        uint64_t *c = (uint64_t *)h->cells;
        uint64_t old = __atomic_load_n(c, __ATOMIC_ACQUIRE);
        while ((old & MX_HOLDER) == m)
        {
            if (__atomic_compare_exchange_n(c, &old, (old & ~MX_HOLDER) | MX_POISON, true, __ATOMIC_ACQ_REL, __ATOMIC_ACQUIRE))
            {
                u64vec_push(keys, (uint64_t)key);
                u64vec_push(cells, (uint64_t)h->cells);
                break;
            }
        }
        __atomic_store_n(&h->key, 0, __ATOMIC_RELEASE);
    }
    __atomic_store_n(&t->nheld, 0, __ATOMIC_RELEASE);
    /* Key order: the order is not observable (each is poisoned and
       signalled), as before. */
    for (uint32_t i = 1; i < keys->n; i++)
    {
        uint64_t x = keys->p[i], y = cells->p[i];
        uint32_t j = i;
        while (j > 0 && keys->p[j - 1] > x)
        {
            keys->p[j] = keys->p[j - 1];
            cells->p[j] = cells->p[j - 1];
            j--;
        }
        keys->p[j] = x;
        cells->p[j] = y;
    }
}

/* The state cells let go (a poisoned mutex stays poisoned, [Guard-Drop]);
   what they held before. */
static inline uint64_t cells_clear(uintptr_t cells)
{
    uint64_t *c = (uint64_t *)cells;
    uint64_t old = __atomic_load_n(c, __ATOMIC_RELAXED);
    while (!__atomic_compare_exchange_n(c, &old, old & MX_POISON, true, __ATOMIC_RELEASE, __ATOMIC_RELAXED))
    {
    }
    return old;
}

/* The mutex at `key`, its state cells at `cells`, let go; a thread waiting
   for it woken. One compare-and-swap when no thread registered to wait. */
static void mutex_release_cells(uintptr_t key, uintptr_t cells)
{
    uint64_t old = cells_clear(cells);
    if (old & MX_WAITED)
    {
        wake_waiters(mutex_key(key));
    }
    else if (sched_on())
    {
        SCHED_PENDING = true;
    }
}

/* The mutex at `key` let go by a thread other than the one that locked it
   (a guard moved here): the locker's entry cleared and the cells let go in
   one hold of the book's lock (see above). */
static CB_COLD void release_moved(uintptr_t key, uintptr_t cells)
{
    Book *b = book();
    uint64_t holder = mutex_holder(cells);
    TState *t = holder ? held_owner(b, holder) : NULL;
    if (t)
    {
        uint32_t n = __atomic_load_n(&t->nheld, __ATOMIC_ACQUIRE);
        for (uint32_t i = 0; i < n; i++)
        {
            HeldMx *h = held_at(t, i);
            uintptr_t k = key;
            if (__atomic_compare_exchange_n(&h->key, &k, 0, false, __ATOMIC_ACQ_REL, __ATOMIC_RELAXED))
            {
                break;
            }
        }
    }
    uint64_t old = cells_clear(cells);
    book_unlock();
    /* The book was held while the cells changed: a waiter's look at them,
       under it too, came before (and it registered) or comes after. */
    if (old & MX_WAITED)
    {
        signal_key(mutex_key(key));
    }
    else if (sched_on())
    {
        SCHED_PENDING = true;
    }
}

/* The mutex at `key` let go by the thread that locked it (a bare guard). */
static void release_mutex(uintptr_t key)
{
    uintptr_t cells = held_pop(key);
    if (UNLIKELY(cells == 0))
    {
        cbrt_internal("a mutex released by a thread that holds no lock of it");
    }
    mutex_release_cells(key, cells);
}

/* `std::event_op`: 0 makes a count (its id), 1 reads one, 2 waits until
   one differs from `seen`, 3 adds one to it and wakes its waiters, 4 frees
   it. D-0202: the book alone. */
uint64_t cb_event_op(uint64_t op, uint64_t id, uint64_t seen)
{
    switch (op)
    {
    case 0:
        return stripe_event_new();
    case 1:
    {
        uint64_t r = 0;
        (void)stripe_event_get(id, &r);
        return r;
    }
    case 2:
    {
        /* `[Channel-Deadlock]`: the main thread waits on a count no other
           thread is running to change -- checked when the wait starts and
           whenever it wakes. Not when `[Deadlock]` (b) already made the main
           thread an end of a chain of waits (D-0219). */
        event_ctx c = {id, seen, me() == 1};
        Wait w = {W_EVENT, id, seen};
        /* Report 132: counted among the count's waiters, so a change wakes
           this thread (`event_add`). */
        {
            Stripe *s = stripe_of(id);
            stripe_count(&s->ewait, id, 1);
            stripe_unlock(s);
        }
        wait_until(event_key(id), w, event_ready, &c);
        {
            Stripe *s = stripe_of(id);
            stripe_count(&s->ewait, id, -1);
            stripe_unlock(s);
        }
        uint64_t now = 0;
        bool has = stripe_event_get(id, &now);
        if (has && now == seen)
        {
            /* Report 169: a thread that could have changed it ended by a
               fault (not yet joined): that fault, named. */
            cb_buf msg = {0};
            cb_buf_str(&msg, "diag.channel-deadlock");
            {
                Book *b = book();
                for (size_t i = 0; i < b->threads.cap; i++)
                {
                    if (b->threads.used[i] && b->threads.e[i].v.status == ST_FAILED)
                    {
                        cb_buf_byte(&msg, DETAIL_SEP);
                        cb_buf_str(&msg, "the only running thread waits on a channel nothing else can change: a thread that could have changed it failed first: ");
                        failure_brief(&msg, b, b->threads.e[i].v.failed);
                        break;
                    }
                }
                book_unlock();
            }
            cb_buf_byte(&msg, 0);
            fault_owned((char *)msg.p, NOLOC, 0);
        }
        return has ? now : 0;
    }
    case 3:
        return event_add(id);
    default:
    {
        Stripe *s = stripe_of(id);
        map_u64_remove(&s->events, id, NULL);
        stripe_unlock(s);
        return 0;
    }
    }
}

/* ---- atomics' waits (spec/19 §2a, D-0253) ---- */

/* The atomics share this many wake counts, by address: a notify wakes the
   waiters on every atomic of its bucket, and each checks its own value
   again, so sharing costs a spurious wake-up, never a lost one. */
#define ATOMIC_BUCKETS 256u

static uint64_t atomic_bucket(uintptr_t addr)
{
    uint64_t a = (uint64_t)addr;
    return ((a >> 3) ^ (a >> 11) ^ (a >> 19)) % ATOMIC_BUCKETS;
}

static uint64_t atomic_key(uint64_t bucket)
{
    return bucket | (1ull << 61);
}

typedef struct
{
    uint64_t bucket;
    uint64_t seen;
} atomic_ctx;

static uint64_t atomic_count(Book *b, uint64_t bucket)
{
    uint64_t *v = map_u64_get(&b->atomic_counts, bucket);
    return v ? *v : 0;
}

static bool atomic_ready(Book *b, void *ctx)
{
    atomic_ctx *c = (atomic_ctx *)ctx;
    return atomic_count(b, c->bucket) != c->seen;
}

/* `std::atomic_wait_op`: 0 reads the wake count of the atomic at `addr`, 1
   waits until it differs from `seen`, 2 and 3 add one to it and wake its
   waiters. A wait no thread can end is `[Deadlock]`. */
uint64_t cb_atomic_wait(uint64_t op, const void *addr, uint64_t seen)
{
    uint64_t bucket = atomic_bucket((uintptr_t)addr);
    switch (op)
    {
    case 0:
    {
        Book *b = book();
        uint64_t v = atomic_count(b, bucket);
        book_unlock();
        return v;
    }
    case 1:
    {
        atomic_ctx c = {bucket, seen};
        Wait w = {W_ATOMIC, bucket, seen};
        wait_until(atomic_key(bucket), w, atomic_ready, &c);
        Book *b = book();
        uint64_t v = atomic_count(b, bucket);
        book_unlock();
        return v;
    }
    default:
    {
        uint64_t c;
        {
            Book *b = book();
            bool had;
            uint64_t *v = map_u64_slot(&b->atomic_counts, bucket, &had);
            if (!had)
            {
                *v = 0;
            }
            *v += 1;
            c = *v;
            book_unlock();
        }
        signal_key(atomic_key(bucket));
        return c;
    }
    }
}

/* D-0253: `spin_hint()`, the processor's pause. */
void cb_spin_hint(void)
{
    plat_spin_hint();
}

/* ---- mutexes (spec/19 §2) ---- */

static void claim_checked(uintptr_t key, uintptr_t cells, uint64_t m, uint32_t file, uint32_t line);

/* Whether thread `m` holds the mutex whose state cells are at `cells`. */
static bool held_by(uintptr_t cells, uint64_t m)
{
    return mutex_holder(cells) == m;
}

static void lock_path_plan(Plan *p, Rt *r, void *ctx)
{
    plan_lock_path(p, r, *(uint64_t *)ctx);
}

/* `[Lock]`: `tok` is the shared reference to the mutex at `m`. Returns the
   lock path's token, which the guard holds. */
uint64_t cb_lock(uint64_t tok, void *m, uint32_t ty, uint32_t file, uint32_t line)
{
    replay_out(me(), "mutexes");
    if (UNLIKELY(!HOOK(GUARD_END)))
    {
        HOOK_SET(GUARD_END, guard_drop);
    }
    uintptr_t key = (uintptr_t)m;
    uint64_t mine = me();
    /* D-0292: through a light reference: the mutex's place from its address
       and type; the guard's path lists no ancestor. */
    if (light_ref(tok))
    {
        uint64_t obj = tok & ~TOK_FLAGS;
        {
            HELD_SCOPE h = enter_on(heap_of(tok));
            (void)h;
            if (held_by(state_cells(m, ty), mine))
            {
                fault("diag.mutex-reentrant-lock", file, line);
            }
            Obj *o = objs_get(rt(), obj);
            if (!(o && o->alive))
            {
                fault("diag.stale-binding", file, line);
            }
        }
        claim_mutex(key, state_cells(m, ty), mine, file, line);
        HELD_SCOPE h = enter_on(heap_of(tok));
        (void)h;
        projvec proj = proj_of_addr(obj, (uintptr_t)m, ty);
        projvec gproj = projvec_clone(&proj);
        projvec_push(&gproj, proj_field(0));
        u64vec none = {0};
        return mint_locked(rt(), obj, gproj, true, false, none, &proj);
    }
    /* D-0202: the checks and the guard's path under a local hold on the heaps
       of the reference to the mutex and the paths it was formed through;
       the wait under none (only the book's lock). */
    {
        HELD_SCOPE h = enter_planned(lock_path_plan, &tok);
        (void)h;
        /* [Lock-Reentrant]: a fault, not a deadlock. */
        if (held_by(state_cells(m, ty), mine))
        {
            fault("diag.mutex-reentrant-lock", file, line);
        }
        path_target_ret pt = path_target(rt(), tok, file, line);
        projvec_free(&pt.proj);
        u64vec_free(&pt.excludes);
    }
    /* Claimed in the same hold of the book that finds it free. */
    claim_mutex(key, state_cells(m, ty), mine, file, line);
    HELD_SCOPE h = enter_planned(lock_path_plan, &tok);
    (void)h;
    Rt *r = rt();
    path_target_ret pt = path_target(r, tok, file, line);
    u64vec anc = u64vec_clone(&pt.excludes);
    for (uint32_t i = 0; i < pt.excludes.n; i++)
    {
        PathRec *pe = paths_get(r, pt.excludes.p[i]);
        if (pe)
        {
            for (uint32_t j = 0; j < pe->ancestors.n; j++)
            {
                uint64_t a = pe->ancestors.p[j];
                if (!u64vec_contains(&anc, a))
                {
                    u64vec_push(&anc, a);
                }
            }
        }
    }
    if (!u64vec_contains(&anc, tok))
    {
        u64vec_push(&anc, tok);
    }
    u64vec_free(&pt.excludes);
    projvec gproj = projvec_clone(&pt.proj);
    projvec_push(&gproj, proj_field(0));
    return mint_locked(r, pt.of, gproj, true, false, anc, &pt.proj);
}

/* D-0289: `[Lock]` in a program lowered pathless (`CB_PATHLESS`): no lock
   path; the guard holds the mutex's address, which is all its drop needs.
   Under a schedule, a point where the turn may pass, and must when a
   release has made a switch pending (session finding 001-A1). */
uint64_t cb_lock_pathless(void *m, uint32_t ty, uint32_t file, uint32_t line)
{
    if (UNLIKELY(!HOOK(GUARD_END)))
    {
        HOOK_SET(GUARD_END, guard_drop);
    }
    if (sched_on())
    {
        sched_entry();
    }
    uintptr_t key = (uintptr_t)m;
    uint64_t mine = me();
    claim_checked(key, state_cells(m, ty), mine, file, line);
    return 0;
}

typedef struct
{
    uintptr_t key;
    uintptr_t cells;
    uint64_t me;
    bool poisoned;
    /* Report 315: whether the claim also looks for the mutex held by this
       thread (`[Lock-Reentrant]`), in the same hold of its stripe, and
       found it so. */
    bool check_self;
    bool reentrant;
} claim_ctx;

/* Report 315: the claim itself, in the mutex's state cells: its poison,
   then whether it is free; a free one is claimed by one compare-and-swap.
   `wait` (under the book's lock, the waiter about to register for the
   mutex's signal): a held one is marked waited for, in the same
   compare-and-swap that finds it held, so its release wakes this thread. */
static bool try_claim_cells(claim_ctx *c, bool wait)
{
    uint64_t *cells = (uint64_t *)c->cells;
    uint64_t old = __atomic_load_n(cells, __ATOMIC_RELAXED);
    for (;;)
    {
        if (old & MX_POISON)
        {
            c->poisoned = true;
            return true;
        }
        uint64_t holder = old & MX_HOLDER;
        if (holder != 0)
        {
            if (c->check_self && holder == c->me)
            {
                c->reentrant = true;
                return true;
            }
            if (!wait || (old & MX_WAITED))
            {
                return false;
            }
            if (__atomic_compare_exchange_n(cells, &old, old | MX_WAITED, true, __ATOMIC_ACQ_REL, __ATOMIC_RELAXED))
            {
                return false;
            }
            continue;
        }
        if (__atomic_compare_exchange_n(cells, &old, old | c->me, true, __ATOMIC_ACQUIRE, __ATOMIC_RELAXED))
        {
            held_push(c->key, c->cells, c->me);
            return true;
        }
    }
}

static bool try_claim(claim_ctx *c)
{
    return try_claim_cells(c, false);
}

static bool claim_ready(Book *b, void *ctx)
{
    (void)b;
    return try_claim_cells((claim_ctx *)ctx, true);
}

/* The mutex at `key` claimed by `me`. D-0204: a poisoned mutex
   (`[Lock-Poisoned]`) is not claimed; locking it faults. D-0310: its
   holder and poison are in its own state cells at `cells`. */
static void claim_mutex_with(uintptr_t key, uintptr_t cells, uint64_t m, uint32_t file, uint32_t line, bool check_self);

void claim_mutex(uintptr_t key, uintptr_t cells, uint64_t m, uint32_t file, uint32_t line)
{
    claim_mutex_with(key, cells, m, file, line, false);
}

/* Report 315: `[Lock-Reentrant]`'s check and the claim in one look at the
   mutex's cells; under a schedule the check comes first, as before, so a
   reentrant lock is not taken for a wait. */
static void claim_checked(uintptr_t key, uintptr_t cells, uint64_t m, uint32_t file, uint32_t line)
{
    if (sched_on())
    {
        if (held_by(cells, m))
        {
            fault("diag.mutex-reentrant-lock", file, line);
        }
        claim_mutex_with(key, cells, m, file, line, false);
        return;
    }
    /* The uncontended lock: one compare-and-swap of free cells. */
    uint64_t free_cells = 0;
    if (LIKELY(m <= MX_HOLDER && __atomic_compare_exchange_n((uint64_t *)cells, &free_cells, m, false, __ATOMIC_ACQUIRE, __ATOMIC_RELAXED)))
    {
        held_push(key, cells, m);
        return;
    }
    claim_mutex_with(key, cells, m, file, line, true);
}

static void claim_mutex_with(uintptr_t key, uintptr_t cells, uint64_t m, uint32_t file, uint32_t line, bool check_self)
{
    if (UNLIKELY(cells == 0 || m > MX_HOLDER))
    {
        cbrt_internal(cells == 0 ? "a lock without its mutex's state cells" : "too many threads for a mutex's holder");
    }
    claim_ctx c = {key, cells, m, false, check_self, false};
    /* Held by another thread, it is asked again a while first (a lock is
       held for a short time), before this thread registers to wait. */
    if (!sched_on())
    {
        for (uint32_t i = 0; i < 64; i++)
        {
            if (try_claim(&c))
            {
                goto claimed;
            }
            if (!multi())
            {
                break;
            }
            for (int k = 0; k < 32; k++)
            {
                plat_spin_hint();
            }
            if (i % 16 == 15)
            {
                plat_thread_yield();
            }
        }
    }
    {
        /* The deadlock finder reads the holder from the cells (`b`). */
        Wait w = {W_LOCK, (uint64_t)key, (uint64_t)cells};
        wait_until(mutex_key(key), w, claim_ready, &c);
    }
claimed:
    if (c.reentrant)
    {
        fault("diag.mutex-reentrant-lock", file, line);
    }
    if (c.poisoned)
    {
        /* Report 070: the failure that poisoned it, named. */
        cb_buf msg = {0};
        cb_buf_str(&msg, "diag.mutex-poisoned");
        {
            Book *b = book();
            void **why = map_ptr_get(&b->poisoned_by, (uint64_t)cells);
            if (why && *why)
            {
                cb_buf_byte(&msg, DETAIL_SEP);
                cb_buf_str(&msg, "a thread failed while holding this lock: ");
                cb_buf_str(&msg, (const char *)*why);
            }
            book_unlock();
        }
        cb_buf_byte(&msg, 0);
        fault_owned((char *)msg.p, file, line);
    }
}

/* D-0200: `[Lock]` for compiled code that reaches the mutex's interior only
   between this call and `cb_unlock_bare` (a native `Channel::send`/`recv`):
   no lock path. `ty` is the mutex's type (D-0310), or 0 where nothing can
   fault while the lock is held. D-0202: the book alone. */
void cb_lock_bare(void *m, uint32_t ty, uint32_t file, uint32_t line)
{
    if (UNLIKELY(!HOOK(GUARD_END)))
    {
        HOOK_SET(GUARD_END, guard_drop);
    }
    replay_out(me(), "mutexes");
    /* Under a schedule, a point where the turn may pass, as in
       `cb_lock_pathless`: a guard the program holds through `auto` is
       claimed here, between an unlock and its next lock nothing else
       enters the runtime, and `cobc --explore` never ran another thread
       there (the guide's counter example showed one outcome of two). */
    if (sched_on())
    {
        sched_entry();
    }
    uintptr_t key = (uintptr_t)m;
    uint64_t mine = me();
    claim_checked(key, state_cells(m, ty), mine, file, line);
}

/* The address of the state cells of the mutex at `m`, of type `ty`
   (`struct { τ inner; uint64_t state; }`, `[Sizeof-Mutex]`: the cells at
   the next multiple of 8 after `inner`), or 0 for `ty` 0. */
uintptr_t state_cells(void *m, uint32_t ty)
{
    if (ty == 0)
    {
        return 0;
    }
    /* Report 315: the last type's offset, kept: a lock in a loop looked its
       type up twice each time. Types never change once the program runs. */
    static __thread uint32_t last_ty;
    static __thread uintptr_t last_off;
    if (ty == last_ty)
    {
        return (uintptr_t)m + last_off;
    }
    Rt *r = rt();
    TypeInfo *t = rt_typ(r, ty);
    uintptr_t off;
    if (t->nfields > 0)
    {
        off = ((uintptr_t)t->fields[0].off + (uintptr_t)rt_typ(r, t->fields[0].ty)->size + 7) & ~(uintptr_t)7;
    }
    else
    {
        off = (uintptr_t)t->size - 8;
    }
    last_ty = ty;
    last_off = off;
    return (uintptr_t)m + off;
}

/* D-0200: `event_op(3, id, 0)` and the mutex at `m` released, in one entry:
   a native `Channel::send`/`recv`'s last steps under the lock. */
void cb_unlock_signal(void *m, uint64_t id)
{
    (void)event_add(id);
    release_mutex((uintptr_t)m);
}

/* D-0200: the mutex at `m` released, as a guard's drop releases it. */
void cb_unlock_bare(void *m)
{
    release_mutex((uintptr_t)m);
}

/* `[Guard-Drop]`: the guard at `addr` holds the address of the mutex's
   interior (its first field, so the mutex's own address) and, in the slot
   table, the lock path. */
void guard_drop(uintptr_t addr, uint64_t cells_off)
{
    Rt *r = rt();
    uint64_t tok = slots_get(r, addr);
    if (tok)
    {
        invalidate(r, tok);
    }
    guard_release(addr, cells_off);
}

/* Report 315: the mutex a guard at `addr` holds let go, by whichever thread
   ends the guard (it may have been moved to another thread): the entry of
   the thread that locked it ends too. Needs no hold on the runtime: a
   decided program's guard ends here directly (`cb_drop_local`). */
void guard_release(uintptr_t addr, uint64_t cells_off)
{
    uintptr_t key = *(const uintptr_t *)addr;
    uintptr_t cells = key + (uintptr_t)cells_off;
    if (LIKELY(held_pop(key) != 0))
    {
        mutex_release_cells(key, cells);
    }
    else
    {
        release_moved(key, cells);
    }
}

/* ---- D-0261: the whole-state check ----

   With `COBALTC_VERIFY` set, `cb_stmt_pop` re-derives, from the whole of
   the runtime's tables and independently of the per-access checks that
   built them, what those checks rely on: every live path's object exists
   and lists it; every object's token lists name paths of that object, and
   its exclusive list is the non-root exclusive ones among them; every
   ancestors list is closed. A violation is a defect of the runtime or of
   `cobc`'s lowering, never of the program. */

/* D-0264: `COBALTC_TRACE_EVENTS`, read once: one `EV` line on standard
   error per event the trace comparison matches between the tools. */
bool trace_events_on(void)
{
    static _Atomic int on = -1;
    int v = atomic_load_explicit(&on, memory_order_relaxed);
    if (v < 0)
    {
        const char *s = env_var("COBALTC_TRACE_EVENTS");
        v = s && s[0] && strcmp(s, "0") != 0;
        atomic_store_explicit(&on, v, memory_order_relaxed);
    }
    return v != 0;
}

/* Rust's `str::trim` then `parse::<u64>`: an optional `+`, decimal digits,
   no overflow. */
static bool parse_u64_trimmed(const char *s, uint64_t *out)
{
    size_t n = strlen(s);
    size_t i = 0;
    while (i < n && (s[i] == ' ' || (s[i] >= '\t' && s[i] <= '\r')))
    {
        i++;
    }
    while (n > i && (s[n - 1] == ' ' || (s[n - 1] >= '\t' && s[n - 1] <= '\r')))
    {
        n--;
    }
    if (i < n && s[i] == '+')
    {
        i++;
    }
    if (i >= n)
    {
        return false;
    }
    uint64_t v = 0;
    for (; i < n; i++)
    {
        if (s[i] < '0' || s[i] > '9')
        {
            return false;
        }
        uint64_t d = (uint64_t)(s[i] - '0');
        if (v > (UINT64_MAX - d) / 10)
        {
            return false;
        }
        v = v * 10 + d;
    }
    *out = v;
    return true;
}

/* The stride: `COBALTC_VERIFY=1` (or any non-number) checks at every
   statement's end, `COBALTC_VERIFY=N` at every Nth; 0 or unset is off. */
_Atomic int64_t VERIFY_STRIDE = -1;

uint64_t verify_stride(void)
{
    int64_t v = atomic_load_explicit(&VERIFY_STRIDE, memory_order_relaxed);
    if (v < 0)
    {
        const char *s = env_var("COBALTC_VERIFY");
        uint64_t x;
        if (!s || s[0] == 0 || strcmp(s, "0") == 0)
        {
            x = 0;
        }
        else
        {
            if (!parse_u64_trimmed(s, &x))
            {
                x = 1;
            }
            if (x < 1)
            {
                x = 1;
            }
        }
        /* kept below 2^63 so the cache's sign marks "unread" */
        if (x > (uint64_t)INT64_MAX)
        {
            x = (uint64_t)INT64_MAX;
        }
        v = (int64_t)x;
        atomic_store_explicit(&VERIFY_STRIDE, v, memory_order_relaxed);
    }
    return (uint64_t)v;
}

/* A check scans the tables, so it is made no more often than every
   (entries / 64)th statement, whatever the stride asks. */
bool verify_on(void)
{
    uint64_t stride = verify_stride();
    if (stride == 0)
    {
        return false;
    }
    static _Atomic uint64_t count;
    Rt *r = rt();
    uint64_t len = 0;
    size_t no = heaps_len(&r->objs.n), np = heaps_len(&r->paths.n);
    for (size_t h = 0; h < no; h++)
    {
        len += r->objs.h[h].n;
    }
    for (size_t h = 0; h < np; h++)
    {
        len += r->paths.h[h].n;
    }
    uint64_t n = len / 64;
    if (n < stride)
    {
        n = stride;
    }
    return atomic_fetch_add_explicit(&count, 1, memory_order_relaxed) % n == n - 1;
}

/* A violation: one line on standard error, `cbrt verify: {} (thread
   {}{})`, and exit status 4 (internal errors). */
CB_NORET static void verify_fail(cb_buf *what, const cb_buf *at)
{
    cb_buf line = {0};
    cb_buf_str(&line, "cbrt verify: ");
    cb_buf_put(&line, what->p, what->n);
    cb_buf_str(&line, " (thread ");
    buf_u64(&line, me());
    cb_buf_put(&line, at->p, at->n);
    cb_buf_str(&line, ")\n");
    fwrite(line.p, 1, line.n, stderr);
    fflush(stderr);
    plat_exit(4);
}

static bool toklist_has(const toklist *l, uint64_t t)
{
    return u64vec_contains(&l->v, t);
}

/* Each piece of a message: text, or a number (NULL text). */
static void msg(cb_buf *b, const char *s)
{
    cb_buf_str(b, s);
}

CB_COLD void verify_state(void)
{
    ENTERED_SCOPE e = enter();
    (void)e;
    Rt *r = rt();
    TState *t = ts_raw();
    bool has_loc = false;
    loc_t loc = {0, 0};
    if (t->pend.n)
    {
        has_loc = true;
        loc = t->pend.p[t->pend.n - 1];
    }
    else if (t->scopes.n)
    {
        size_t i = t->scopes.p[t->scopes.n - 1];
        if (i < t->stack.n && t->stack.p[i].kind == E_SCOPE)
        {
            has_loc = true;
            loc = t->stack.p[i].u.s.loc;
        }
    }
    cb_buf at = {0};
    if (has_loc && loc.f != NOLOC)
    {
        cb_buf_str(&at, " at ");
        cb_buf_str(&at, loc.f < r->nfiles ? r->files[loc.f] : "?");
        cb_buf_byte(&at, ':');
        buf_u64(&at, loc.l);
    }
    cb_buf w = {0};
    /* Paths: a valid path's object exists and lists the path; ancestors
       lists are closed. In id order: heap by heap, slot by slot. */
    size_t np = heaps_len(&r->paths.n);
    for (size_t h = 0; h < np; h++)
    {
        PathArena *a = &r->paths.h[h];
        for (size_t i = 0; i < a->n; i++)
        {
            if (!a->slots[i].some)
            {
                continue;
            }
            uint64_t tok = PathArena_id_at(a, i);
            PathRec *p = &a->slots[i].v;
            if (!p->valid)
            {
                continue;
            }
            Obj *o = objs_get(r, p->of);
            if (!o)
            {
                msg(&w, "path ");
                buf_u64(&w, tok);
                msg(&w, " (of object ");
                buf_u64(&w, p->of);
                msg(&w, ") is valid but its object has no entry");
                verify_fail(&w, &at);
            }
            if (!toklist_has(&o->toks, tok))
            {
                msg(&w, "path ");
                buf_u64(&w, tok);
                msg(&w, " is a valid path of object ");
                buf_u64(&w, p->of);
                msg(&w, " but the object's token list does not hold it");
                verify_fail(&w, &at);
            }
            if (p->exclusive && !p->is_root && !toklist_has(&o->xtoks, tok))
            {
                msg(&w, "path ");
                buf_u64(&w, tok);
                msg(&w, " is a valid exclusive path of object ");
                buf_u64(&w, p->of);
                msg(&w, " but the object's exclusive list does not hold it");
                verify_fail(&w, &at);
            }
            for (uint32_t k = 0; k < p->ancestors.n; k++)
            {
                uint64_t an = p->ancestors.p[k];
                PathRec *pa = paths_get(r, an);
                if (!pa)
                {
                    continue;
                }
                for (uint32_t j = 0; j < pa->ancestors.n; j++)
                {
                    uint64_t x = pa->ancestors.p[j];
                    if (!u64vec_contains(&p->ancestors, x))
                    {
                        msg(&w, "path ");
                        buf_u64(&w, tok);
                        msg(&w, "'s ancestors list is not closed: it holds ");
                        buf_u64(&w, an);
                        msg(&w, " but not ");
                        buf_u64(&w, an);
                        msg(&w, "'s ancestor ");
                        buf_u64(&w, x);
                        verify_fail(&w, &at);
                    }
                }
            }
        }
    }
    /* Objects: every token listed names a path of this object; the
       exclusive list is the non-root exclusive ones. */
    size_t no = heaps_len(&r->objs.n);
    for (size_t h = 0; h < no; h++)
    {
        ObjArena *a = &r->objs.h[h];
        for (size_t i = 0; i < a->n; i++)
        {
            if (!a->slots[i].some)
            {
                continue;
            }
            uint64_t id = ObjArena_id_at(a, i);
            Obj *o = &a->slots[i].v;
            for (uint32_t k = 0; k < o->toks.v.n; k++)
            {
                uint64_t tk = o->toks.v.p[k];
                PathRec *p = paths_get(r, tk);
                if (!p)
                {
                    msg(&w, "object ");
                    buf_u64(&w, id);
                    msg(&w, " lists token ");
                    buf_u64(&w, tk);
                    msg(&w, ", which names no path");
                    verify_fail(&w, &at);
                }
                if (p->of != id)
                {
                    msg(&w, "object ");
                    buf_u64(&w, id);
                    msg(&w, " lists token ");
                    buf_u64(&w, tk);
                    msg(&w, ", which is a path of object ");
                    buf_u64(&w, p->of);
                    verify_fail(&w, &at);
                }
            }
            for (uint32_t k = 0; k < o->xtoks.v.n; k++)
            {
                uint64_t tk = o->xtoks.v.p[k];
                if (!toklist_has(&o->toks, tk))
                {
                    msg(&w, "object ");
                    buf_u64(&w, id);
                    msg(&w, "'s exclusive list holds token ");
                    buf_u64(&w, tk);
                    msg(&w, ", which its token list does not");
                    verify_fail(&w, &at);
                }
                PathRec *p = paths_get(r, tk);
                if (!(p && p->exclusive && !p->is_root))
                {
                    msg(&w, "object ");
                    buf_u64(&w, id);
                    msg(&w, "'s exclusive list holds token ");
                    buf_u64(&w, tk);
                    msg(&w, ", which is not a non-root exclusive path");
                    verify_fail(&w, &at);
                }
            }
        }
    }
    cb_buf_free(&at);
    cb_buf_free(&w);
}
