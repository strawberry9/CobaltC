/* The runtime lock, local entries and waits: the book of threads, mutex
   holders and channel counts under its own lock; the runtime lock made
   reentrant over an RwLock (exclusive holds, read-only entries, D-0192);
   the heap spin locks and the local entries that hold only some of them,
   planned before they lock (D-0202); blocking waits with deadlock and
   cancellation (D-0204). Part of lib.c's translation unit. */

#include "rt.h"

/* ---- statics ---- */

_Atomic bool MULTI;
_Atomic bool LONE;
_Atomic bool SOLO_PENDING;
plat_rwlock RTLOCK = PLAT_RWLOCK_INIT;
_Atomic uint64_t OWNER;
_Atomic size_t HEAPS_N = 1;
_Atomic bool HEAP_SHARED[MAX_HEAPS];
_Atomic uint64_t N_LOCAL, N_EXCL, N_MIGRATED;
_Atomic bool STATS_ON;

__thread uint64_t TID = 1;
__thread uint8_t HEAP;
__thread TState *TSP;
__thread uint32_t HELD;
__thread bool GUARD;
__thread bool RGUARD;
__thread uint32_t RHELD;
__thread uint32_t LHELD;
__thread HeapSet LSET;
__thread size_t LSOLE = NO_SOLE;
__thread size_t XN;
__thread bool HAS_TIMER_DEADLINE;
__thread uint64_t TIMER_DEADLINE;

TState MAIN_TS = {.dloc = {NOLOC, 0}};
uint32_t CALLBACK_DEPTH;

/* ---- the book ---- */

static plat_mutex BOOK_LOCK = PLAT_MUTEX_INIT;
static Book BOOK;

Book *book(void)
{
    plat_mutex_lock(&BOOK_LOCK);
    return &BOOK;
}

void book_unlock(void)
{
    plat_mutex_unlock(&BOOK_LOCK);
}

/* ---- report 132: the stripes ---- */

#define STRIPES_N 64
/* Zero is an unlocked mutex on both platforms (PTHREAD_MUTEX_INITIALIZER,
   SRWLOCK_INIT). */
static Stripe STRIPES[STRIPES_N];
static _Atomic uint64_t LAST_EVENT;

Stripe *stripe_of(uint64_t k)
{
    Stripe *s = &STRIPES[(k * 0x9E3779B97F4A7C15ull) >> 58];
    plat_mutex_lock(&s->m);
    return s;
}

void stripe_unlock(Stripe *s)
{
    plat_mutex_unlock(&s->m);
}

bool stripe_event_get(uint64_t id, uint64_t *v)
{
    Stripe *s = stripe_of(id);
    uint64_t *p = map_u64_get(&s->events, id);
    bool r = p != NULL;
    if (p && v)
    {
        *v = *p;
    }
    stripe_unlock(s);
    return r;
}

uint64_t stripe_event_new(void)
{
    uint64_t id = atomic_fetch_add_explicit(&LAST_EVENT, 1, memory_order_relaxed) + 1;
    Stripe *s = stripe_of(id);
    map_u64_insert(&s->events, id, 0, NULL);
    stripe_unlock(s);
    return id;
}

void stripe_count(map_u64 *m, uint64_t k, int d)
{
    bool had;
    uint64_t *v = map_u64_slot(m, k, &had);
    if (!had)
    {
        *v = 0;
    }
    if (d > 0)
    {
        *v += (uint64_t)d;
    }
    else if (*v >= (uint64_t)(-d))
    {
        *v -= (uint64_t)(-d);
    }
    if (*v == 0)
    {
        map_u64_remove(m, k, NULL);
    }
}

/* Whether a waiting thread's wait still holds it (one woken but not yet run
   is not stuck). */
bool book_still_waits(const Book *b, const Wait *w)
{
    switch (w->k)
    {
    case W_JOIN:
    {
        ThreadRec *t = threadmap_get(&b->threads, w->a);
        return t && t->status == ST_RUNNING;
    }
    case W_LOCK:
        /* Report 315: the holder is in the mutex's state cells (`b`). */
        return mutex_holder((uintptr_t)w->b) != 0;
    case W_EVENT:
    {
        uint64_t v = 0;
        return stripe_event_get(w->a, &v) && v == w->b;
    }
    case W_ATOMIC:
    {
        uint64_t *v = map_u64_get(&b->atomic_counts, w->a);
        return (v ? *v : 0) == w->b;
    }
    default:
        return false;
    }
}

static bool wait_next(const Book *b, const Wait *k, uint64_t *t)
{
    if (k->k == W_JOIN)
    {
        *t = k->a;
        return true;
    }
    if (k->k == W_LOCK)
    {
        uint64_t holder = mutex_holder((uintptr_t)k->b);
        if (holder != 0)
        {
            *t = holder;
            return true;
        }
    }
    (void)b;
    return false;
}

/* Whether `me` waiting for `kind` would close a cycle of joins and locks. */
bool book_closes_cycle(const Book *b, uint64_t me, const Wait *kind)
{
    uint64_t t;
    bool at = wait_next(b, kind, &t);
    size_t steps = 0;
    while (at)
    {
        if (t == me)
        {
            return true;
        }
        /* A thread asked to stop ends at its next look: no cycle through it. */
        if (hset_has(&b->cancel, t) && !hset_has(&b->lent, t))
        {
            return false;
        }
        steps++;
        if (steps > b->waits.n + 1)
        {
            return false;
        }
        Wait *w = waitmap_get(&b->waits, t);
        at = w ? wait_next(b, w, &t) : false;
    }
    return false;
}

static int cmp_u64(const void *x, const void *y)
{
    uint64_t a = *(const uint64_t *)x, b = *(const uint64_t *)y;
    return a < b ? -1 : a > b;
}

/* When every live thread waits and no wait can end: one member of a cycle
   of joins and locks is deadlocked, if there is a cycle; else every one of
   them is. Hash-table order (LOG.md): the live threads are gathered in the
   threads table's order, which decides nothing here (they only go into a
   set). */
void book_find_deadlock(Book *b)
{
    if (atomic_load_explicit(&IN_IO, memory_order_acquire) != 0)
    {
        return;
    }
    u64vec live = {0};
    u64vec_push(&live, 1);
    for (size_t i = 0; i < b->threads.cap; i++)
    {
        if (b->threads.used[i] && b->threads.e[i].v.status == ST_RUNNING)
        {
            u64vec_push(&live, b->threads.e[i].k);
        }
    }
    /* The main thread waiting alone on a channel is `[Channel-Deadlock]`,
       not `[Deadlock]` (spec/19 §3): its own wait reports it (D-0219). */
    if (live.n == 1)
    {
        Wait *w = waitmap_get(&b->waits, 1);
        if (w && w->k == W_EVENT)
        {
            u64vec_free(&live);
            return;
        }
    }
    for (uint32_t i = 0; i < live.n; i++)
    {
        uint64_t t = live.p[i];
        Wait *w = waitmap_get(&b->waits, t);
        bool stuck = w && book_still_waits(b, w) && !(hset_has(&b->cancel, t) && !hset_has(&b->lent, t));
        if (!stuck)
        {
            u64vec_free(&live);
            return;
        }
    }
    u64vec waiting = {0};
    for (size_t i = 0; i < b->waits.cap; i++)
    {
        if (b->waits.used[i])
        {
            u64vec_push(&waiting, b->waits.e[i].k);
        }
    }
    qsort(waiting.p, waiting.n, sizeof(uint64_t), cmp_u64);
    for (uint32_t i = waiting.n; i-- > 0;)
    {
        uint64_t t = waiting.p[i];
        Wait *w = waitmap_get(&b->waits, t);
        if (w)
        {
            Wait k = *w;
            if (book_closes_cycle(b, t, &k))
            {
                hset_add(&b->deadlocked, t);
                u64vec_free(&waiting);
                u64vec_free(&live);
                return;
            }
        }
    }
    u64vec_free(&waiting);
    /* No cycle: the ends of the chains of waits -- the threads waiting on a
       channel, not on another thread -- fault; one outcome, whatever the
       timing. */
    u64vec ends = {0};
    for (uint32_t i = 0; i < live.n; i++)
    {
        Wait *w = waitmap_get(&b->waits, live.p[i]);
        if (!(w && (w->k == W_JOIN || w->k == W_LOCK)))
        {
            u64vec_push(&ends, live.p[i]);
        }
    }
    u64vec *use = ends.n ? &ends : &live;
    /* Report 321: alone only when found so the first time: a thread found
       deadlocked with others, which then ended by their faults, stays
       "each other" when it is looked at again before its wait ends. */
    bool lone = ends.n == 1 && !hset_has(&b->deadlocked, ends.p[0]);
    for (uint32_t i = 0; i < use->n; i++)
    {
        hset_add(&b->deadlocked, use->p[i]);
    }
    if (lone)
    {
        hset_add(&b->lone_waiter, ends.p[0]);
    }
    u64vec_free(&ends);
    u64vec_free(&live);
}

/* Whether program thread `t` is still running its body (the main thread,
   1, always is while the program runs). */
bool thread_running(uint64_t t)
{
    if (t == 1)
    {
        return true;
    }
    Book *b = book();
    ThreadRec *rec = threadmap_get(&b->threads, t);
    bool r = rec && rec->status == ST_RUNNING;
    book_unlock();
    return r;
}

/* D-0213 `[Thread-Affine]`: an access through the exclusive path `rec` by
   this thread, out of line. */
CB_COLD void affine_slow(PathRec *rec, uint64_t owner, uint64_t m, uint32_t file, uint32_t line)
{
    for (;;)
    {
        if (owner != TRANSIT && thread_running(owner))
        {
            fault("diag.aliasing-conflict", file, line);
        }
        uint64_t expect = owner;
        if (atomic_compare_exchange_strong_explicit(&rec->thread, &expect, m, memory_order_acq_rel, memory_order_acquire))
        {
            /* D-0237 (spec/18 `contained`): a spawned thread that takes over
               an exclusive path changes data that is not its own: it is not
               cancelled, and its fault is not contained. */
            if (m != 1)
            {
                Book *b = book();
                hset_add(&b->lent, m);
                book_unlock();
            }
            return;
        }
        if (expect == m)
        {
            return;
        }
        owner = expect;
    }
}

/* ---- the runtime lock ---- */

/* This thread's state, its pending statement scopes made first (D-0196). */
CB_COLD void make_pending(TState *t)
{
    for (size_t i = 0; i < t->pend.n; i++)
    {
        /* The scope's entry has its frame's flag (`top_rec`). */
        bool rec = top_rec(t);
        sizevec_push(&t->scopes, t->stack.n);
        Entry *e = entry_push(t);
        e->kind = E_SCOPE;
        e->rec = rec;
        memset(&e->u.s.temps, 0, sizeof e->u.s.temps);
        memset(&e->u.s.toks, 0, sizeof e->u.s.toks);
        e->u.s.loc = t->pend.p[i];
    }
    t->pend.n = 0;
}

/* `make_pending` for the lone paths' cores, which run it at most
   statements of a checked program: one pending scope and room for it, made
   where it goes. A function of its own, so that a program that links no
   core links only the compact one above. */
void make_pending_one(TState *t)
{
    size_t at = t->stack.n;
    if (LIKELY(t->pend.n == 1) && LIKELY(t->scopes.n < t->scopes.cap) && LIKELY(at < t->stack.cap))
    {
        Entry *e = &t->stack.p[at];
        e->kind = E_SCOPE;
        e->rec = at != 0 && e[-1].rec;
        memset(&e->u.s.temps, 0, sizeof e->u.s.temps);
        memset(&e->u.s.toks, 0, sizeof e->u.s.toks);
        e->u.s.loc = t->pend.p[0];
        t->scopes.p[t->scopes.n++] = at;
        t->stack.n = at + 1;
        t->pend.n = 0;
        return;
    }
    make_pending(t);
}

#define SPIN_TRIES 1024u
#define SPIN_WAITS 200u

/* The runtime's critical sections are short, so a thread that finds the
   lock taken tries again for a while before it sleeps. */
static void lock_write_spinning(void)
{
    for (uint32_t i = 0; i < SPIN_TRIES; i++)
    {
        if (plat_rw_try_write(&RTLOCK))
        {
            return;
        }
        if (i % 256 == 255)
        {
            plat_thread_yield();
        }
        else
        {
            plat_spin_hint();
        }
    }
    plat_rw_write(&RTLOCK);
}

void lock_acquire(uint32_t count)
{
    uint64_t m = TID;
    if (atomic_load_explicit(&OWNER, memory_order_relaxed) == m)
    {
        HELD += count;
        return;
    }
    lock_write_spinning();
    /* D-0202: and every heap's lock, so no local entry runs meanwhile. */
    size_t n = atomic_load_explicit(&HEAPS_N, memory_order_acquire);
    for (size_t h = 0; h < n; h++)
    {
        hlock(h);
    }
    XN = n;
    atomic_store_explicit(&OWNER, m, memory_order_relaxed);
    HELD = count;
    GUARD = true;
}

/* The heap locks an exclusive hold took, given back. */
void unlock_heaps(void)
{
    size_t n = XN;
    XN = 0;
    for (size_t h = n; h-- > 0;)
    {
        hunlock(h);
    }
}

static void drop_guard(void)
{
    if (GUARD)
    {
        GUARD = false;
        plat_rw_write_unlock(&RTLOCK);
    }
}

void lock_release(void)
{
    uint32_t left = --HELD;
    if (left == 0)
    {
        atomic_store_explicit(&OWNER, 0, memory_order_relaxed);
        unlock_heaps();
        drop_guard();
    }
}

/* Gives the lock up entirely (a blocking wait); returns the depth to
   restore. */
uint32_t lock_release_all(void)
{
    uint32_t n = HELD;
    HELD = 0;
    atomic_store_explicit(&OWNER, 0, memory_order_relaxed);
    unlock_heaps();
    drop_guard();
    return n;
}

/* `enter` while threads run, out of line. */
CB_COLD void enter_multi(void)
{
    if (sched_on())
    {
        sched_entry();
    }
    /* D-0202: an exclusive entry inside a local one would wait for its own
       shared hold: a bug, reported rather than a hang. */
    if (LHELD > 0 && atomic_load_explicit(&OWNER, memory_order_relaxed) != TID)
    {
        cbrt_internal("an exclusive entry inside a local one");
    }
    if (atomic_load_explicit(&STATS_ON, memory_order_relaxed))
    {
        atomic_fetch_add_explicit(&N_EXCL, 1, memory_order_relaxed);
    }
    lock_acquire(1);
    if (atomic_load_explicit(&SOLO_PENDING, memory_order_relaxed))
    {
        go_solo();
    }
}

/* Back to a single thread (D-0192): on the main thread, holding the lock,
   with no spawned thread running, locking stops. */
CB_COLD void go_solo(void)
{
    if (TID != 1)
    {
        return;
    }
    /* A program with entry points keeps locking on: a C thread may call in
       at any moment (`cb_entries_on`). */
    if (atomic_load_explicit(&CB_ENTRIES_ON, memory_order_acquire))
    {
        atomic_store_explicit(&SOLO_PENDING, false, memory_order_relaxed);
        return;
    }
    if (atomic_load_explicit(&cb_live_threads, memory_order_acquire) == 1)
    {
        atomic_store_explicit(&SOLO_PENDING, false, memory_order_relaxed);
        Rt *r = rt();
        size_t n = 0;
        size_t hn = heaps_len(&r->reclaimed.n);
        for (size_t h = 0; h < hn; h++)
        {
            n += reclaimed_entries1(&r->reclaimed.h[h]);
        }
        atomic_store_explicit(&cb_reclaimed_entries, n, memory_order_relaxed);
        atomic_store_explicit(&MULTI, false, memory_order_release);
        lone_set();
    }
}

/* D-0192: entering for a check that only reads the state, with other
   threads running: the lock held shared. */
ReadEntered enter_read_multi(void)
{
    uint64_t m = TID;
    if (atomic_load_explicit(&OWNER, memory_order_relaxed) == m)
    {
        return 0;
    }
    if (RHELD > 0)
    {
        RHELD++;
        return 1;
    }
    plat_rw_read(&RTLOCK);
    RGUARD = true;
    RHELD = 1;
    return 1;
}

void read_entered_release(void)
{
    uint32_t left = --RHELD;
    if (left == 0 && RGUARD)
    {
        RGUARD = false;
        plat_rw_read_unlock(&RTLOCK);
    }
}

/* A fault reached from a read-only entry: the shared hold given up, the
   lock taken exclusively, as every other fault holds it. D-0202: and from
   a local entry, its heap's lock given up as well. */
void fault_lock(void)
{
    if (LHELD > 0)
    {
        LHELD = 0;
        unlock_local();
        lock_acquire(1);
        return;
    }
    if (RHELD > 0)
    {
        RHELD = 0;
        if (RGUARD)
        {
            RGUARD = false;
            plat_rw_read_unlock(&RTLOCK);
        }
        lock_acquire(1);
        return;
    }
    /* A fault raised outside every entry (a cancelled socket wait): the
       unwind that follows changes the runtime's state, so it enters
       exclusively first (D-0261). */
    if (multi() && atomic_load_explicit(&OWNER, memory_order_relaxed) != TID)
    {
        lock_acquire(1);
    }
}

/* ---- D-0202: heap locks and local entries ---- */

/* Each heap's lock: a spin lock alone on its cache line. */
typedef struct
{
    _Atomic uint32_t l;
    char pad[60];
} HLock;

static HLock HEAP_LOCKS[256];

void hlock(size_t h)
{
    _Atomic uint32_t *l = &HEAP_LOCKS[h].l;
    uint32_t i = 0;
    for (;;)
    {
        uint32_t expect = 0;
        if (atomic_compare_exchange_weak_explicit(l, &expect, 1, memory_order_acquire, memory_order_relaxed))
        {
            return;
        }
        i++;
        if (i < 128)
        {
            plat_spin_hint();
        }
        else
        {
            plat_thread_yield();
        }
    }
}

void hunlock(size_t h)
{
    atomic_store_explicit(&HEAP_LOCKS[h].l, 0, memory_order_release);
}

CB_COLD Held own_heap_shared(void)
{
    return enter_set_multi(heapset_cur_and(cur_heap()));
}

/* An exclusive entry, as `enter()` made it. */
CB_COLD Held held_excl(void)
{
    Entered e = enter();
    return e ? H_EXCL : H_SOLO;
}

CB_COLD void held_release(uint8_t tag)
{
    switch (tag)
    {
    case H_SOLO:
    case H_EXCL:
        lock_release();
        break;
    case H_LOCAL:
    {
        uint32_t left = --LHELD;
        if (left == 0)
        {
            unlock_local();
        }
        break;
    }
    default:
        break;
    }
}

/* Takes the heap locks of `set`, in ascending order. */
void lock_set(const HeapSet *set)
{
    size_t n = 0, one = NO_SOLE;
    HEAPSET_EACH(*set, h, {
        hlock(h);
        n++;
        one = h;
    });
    LSET = *set;
    LSOLE = n == 1 ? one : NO_SOLE;
}

/* A local hold's heap locks, given back. */
void unlock_local(void)
{
    HeapSet set = LSET;
    LSET = HEAPSET_EMPTY;
    LSOLE = NO_SOLE;
    HEAPSET_EACH(set, h, hunlock(h));
}

bool held_strict(size_t h)
{
    return !multi() || atomic_load_explicit(&OWNER, memory_order_relaxed) == TID || (LHELD > 0 && heapset_has(&LSET, h));
}

/* A local hold on the heaps of `set` (which names this thread's own), out
   of line. */
CB_COLD Held enter_set_multi(HeapSet set)
{
    if (sched_on())
    {
        sched_entry();
    }
    uint64_t m = TID;
    if (atomic_load_explicit(&OWNER, memory_order_relaxed) == m)
    {
        return H_NESTED;
    }
    if (LHELD > 0)
    {
        /* A local entry reached from inside another: the same hold. */
        LHELD++;
        return H_LOCAL;
    }
    if (RHELD > 0)
    {
        return held_excl();
    }
    lock_set(&set);
    LHELD = 1;
    if (atomic_load_explicit(&STATS_ON, memory_order_relaxed))
    {
        atomic_fetch_add_explicit(&N_LOCAL, 1, memory_order_relaxed);
    }
    return H_LOCAL;
}

/* ---- Plan: what an entry's work will touch ----

   Reckoned under a local hold before anything changes: the heaps it needs
   (`need`), and whether its work can be done locally at all (`ok`). Each
   step mirrors the runtime function it plans for. */

Plan plan_new(void)
{
    Plan p;
    p.need = HEAPSET_EMPTY;
    heapset_add(&p.need, cur_heap());
    p.ok = true;
    return p;
}

bool plan_heap(Plan *p, size_t h)
{
    heapset_add(&p->need, h);
    return holds_heap(h);
}

/* The cells [lo, hi): whatever is registered over them by address. */
bool plan_cells(Plan *p, uintptr_t lo, uintptr_t hi)
{
    HeapSet more = addr_heaps(lo, hi > lo + 1 ? hi : lo + 1);
    bool all = true;
    HEAPSET_EACH(more, h, {
        heapset_add(&p->need, h);
        all &= holds_heap(h);
    });
    return all;
}

void plan_drop_occ(Plan *p, Rt *r, uint64_t tok)
{
    if (!plan_heap(p, heap_of(tok)))
    {
        return;
    }
    PathRec *q = paths_get(r, tok);
    if (q && q->occ <= 1 && (!q->valid || (q->scope == NO_IDX && !q->is_root)))
    {
        plan_retire(p, r, tok);
    }
}

void plan_retire(Plan *p, Rt *r, uint64_t tok)
{
    PathRec *q = paths_get(r, tok);
    if (!q || q->is_root)
    {
        return;
    }
    Obj *o = objs_get(r, q->of);
    if (o && o->ephemeral)
    {
        plan_end_obj(p, r, q->of);
    }
}

void plan_invalidate(Plan *p, Rt *r, uint64_t tok)
{
    if (!plan_heap(p, heap_of(tok)))
    {
        return;
    }
    PathRec *q = paths_get(r, tok);
    if (q && q->occ == 0)
    {
        plan_retire(p, r, tok);
    }
}

typedef struct
{
    Plan *p;
    Rt *r;
} plan_ctx;

static void forget_each(uint64_t t, void *c)
{
    plan_ctx *x = (plan_ctx *)c;
    plan_drop_occ(x->p, x->r, t);
}

void plan_forget(Plan *p, Rt *r, uintptr_t lo, uintptr_t hi)
{
    if (lo >= hi || !plan_cells(p, lo, hi))
    {
        return;
    }
    plan_ctx c = {p, r};
    slots_each_in(r, lo, hi, forget_each, &c);
}

void plan_end_obj(Plan *p, Rt *r, uint64_t obj)
{
    if (!plan_heap(p, heap_of(obj)))
    {
        return;
    }
    Obj *o = objs_get(r, obj);
    if (!o)
    {
        return;
    }
    TypeInfo *t = rt_typ(r, o->ty);
    uintptr_t hi = o->addr + (uintptr_t)(t->size ? t->size : 1);
    if (!o->ephemeral && t->has_refs)
    {
        plan_forget(p, r, o->addr, hi);
    }
    if (o->reclaimed || o->ephemeral)
    {
        plan_cells(p, o->addr, hi);
    }
}

void plan_move_obj(Plan *p, Rt *r, uint64_t obj, uintptr_t to)
{
    if (obj == 0 || !plan_heap(p, heap_of(obj)))
    {
        return;
    }
    Obj *o = objs_get(r, obj);
    if (!o)
    {
        return;
    }
    TypeInfo *t = rt_typ(r, o->ty);
    if (!t->has_refs || o->addr == to)
    {
        return;
    }
    uintptr_t size = (uintptr_t)(t->size ? t->size : 1);
    plan_cells(p, o->addr, o->addr + size);
    plan_forget(p, r, to, to + size);
}

void plan_lock_path(Plan *p, Rt *r, uint64_t tok)
{
    if (!plan_heap(p, heap_of(tok)))
    {
        return;
    }
    PathRec *rec = paths_get(r, tok);
    if (!rec)
    {
        return;
    }
    for (uint32_t i = 0; i < rec->ancestors.n; i++)
    {
        plan_heap(p, heap_of(rec->ancestors.p[i]));
    }
}

void plan_store(Plan *p, Rt *r, uintptr_t slot, uint64_t tok)
{
    if (tok != 0)
    {
        plan_heap(p, heap_of(tok));
    }
    plan_forget(p, r, slot, slot + 8);
}

static void slots_of_each(uint64_t t, void *c)
{
    plan_ctx *x = (plan_ctx *)c;
    plan_heap(x->p, heap_of(t));
}

void plan_slots_of(Plan *p, Rt *r, uintptr_t addr, uint32_t ty)
{
    uint64_t sz = rt_typ(r, ty)->size;
    uintptr_t size = (uintptr_t)(sz ? sz : 1);
    if (!plan_cells(p, addr, addr + size))
    {
        return;
    }
    plan_ctx c = {p, r};
    slots_each_in(r, addr, addr + size, slots_of_each, &c);
}

void plan_destroy(Plan *p, Rt *r, uint64_t obj)
{
    if (!plan_heap(p, heap_of(obj)))
    {
        return;
    }
    Obj *o = objs_get(r, obj);
    if (!o || !o->alive)
    {
        return;
    }
    if (o->is_resource || rt_typ(r, o->ty)->holds_fn)
    {
        plan_drop_value(p, r, o->addr, o->ty);
    }
    plan_end_obj(p, r, obj);
}

size_t enum_tag(TypeInfo *info, uintptr_t addr);

/* `drop_in_place`: local only for what the runtime itself does -- a
   guard's release, a plain vector's buffer, and the owning parts of a
   struct, enum or array of those. */
void plan_drop_value(Plan *p, Rt *r, uintptr_t addr, uint32_t ty)
{
    if (!p->ok)
    {
        return;
    }
    TypeInfo *info = rt_typ(r, ty);
    if (info->has_box_of || info->kind == K_FN || info->kind == K_HANDLE)
    {
        p->ok = false;
        return;
    }
    if (!info->is_resource && !info->holds_fn)
    {
        return;
    }
    if (info->kind == K_GUARD)
    {
        /* `guard_drop`: the guard's slot, and the lock path it holds. */
        if (!plan_cells(p, addr, addr + 8))
        {
            return;
        }
        uint64_t tok = slots_get(r, addr);
        if (tok)
        {
            plan_invalidate(p, r, tok);
        }
        return;
    }
    if (info->has_vec_plain && info->drop)
    {
        /* The buffer's release: local while no object lies over its cells. */
        uintptr_t po = (uintptr_t)info->fields[0].off, co = (uintptr_t)info->fields[2].off;
        uintptr_t ptr = *(uintptr_t *)(addr + po);
        uint64_t cap = *(uint64_t *)(addr + co);
        if (cap > 0)
        {
            uintptr_t hi = ptr + (uintptr_t)(cap * info->vec_plain);
            if (!plan_cells(p, ptr, hi))
            {
                return;
            }
            if (!quiet_cells(ptr, hi - ptr) && reclaimed_any_in(r, ptr, hi))
            {
                p->ok = false;
            }
        }
        return;
    }
    if (info->drop)
    {
        p->ok = false;
        return;
    }
    switch (info->kind)
    {
    case K_STRUCT:
        for (uint32_t i = 0; i < info->nowning; i++)
        {
            plan_drop_value(p, r, addr + (uintptr_t)info->owning[i].off, info->owning[i].ty);
        }
        break;
    case K_ENUM:
    {
        size_t tag = enum_tag(info, addr);
        if (tag < info->nvariants)
        {
            uint32_t pty = info->variants[tag];
            if (pty != NOTYPE)
            {
                plan_drop_value(p, r, addr + (uintptr_t)info->payload_off, pty);
            }
        }
        break;
    }
    case K_ARRAY:
    {
        uintptr_t esize = (uintptr_t)rt_typ(r, info->elem)->size;
        for (uint64_t i = 0; i < info->count; i++)
        {
            plan_drop_value(p, r, addr + (uintptr_t)i * esize, info->elem);
        }
        break;
    }
    default:
        p->ok = false;
        break;
    }
}

/* Enters for work `plan` reckons: a local hold on the heaps it needs,
   widened (and the work planned again) until it covers them; exclusive
   when the work cannot be local, or the plan does not settle. */
CB_COLD Held enter_planned_multi(plan_fn f, void *ctx)
{
    HeapSet set = HEAPSET_EMPTY;
    heapset_add(&set, cur_heap());
    for (int i = 0; i < 4; i++)
    {
        Held h = enter_set_multi(set);
        if (h != H_LOCAL || LHELD > 1)
        {
            return h;
        }
        Plan p = plan_new();
        f(&p, rt(), ctx);
        if (!p.ok)
        {
            /* `break`: the loop body's hold is dropped on leaving it, then
               the exclusive entry is made. */
            held_drop(h);
            break;
        }
        if (heapset_covers(&set, &p.need))
        {
            return h;
        }
        held_drop(h);
        HEAPSET_EACH(p.need, x, heapset_add(&set, x));
    }
    return held_excl();
}

static void scope_end_plan(Plan *p, Rt *r, void *ctx)
{
    (void)ctx;
    TState *t = ts();
    Entry *e = entryvec_last(&t->stack);
    if (!e || e->kind != E_SCOPE)
    {
        return;
    }
    Scope *s = &e->u.s;
    for (uint32_t i = 0; i < s->temps.n; i++)
    {
        uint64_t obj = s->temps.p[i];
        if (!plan_heap(p, heap_of(obj)))
        {
            continue;
        }
        Obj *o = objs_get(r, obj);
        if (o && o->alive && o->frame == NO_IDX && o->root == 0)
        {
            plan_destroy(p, r, obj);
        }
    }
    for (uint32_t i = 0; i < s->toks.n; i++)
    {
        uint64_t tk = s->toks.p[i];
        if (!plan_heap(p, heap_of(tk)))
        {
            continue;
        }
        PathRec *q = paths_get(r, tk);
        if (q && q->occ == 0)
        {
            plan_retire(p, r, tk);
        }
    }
}

/* D-0202: entering to end the statement scope on top of this thread's
   stack (`end_scope`). */
Held enter_scope_end(void)
{
    return enter_planned(scope_end_plan, NULL);
}

static void frame_end_plan(Plan *p, Rt *r, void *ctx)
{
    (void)ctx;
    TState *t = ts();
    Entry *e = entryvec_last(&t->stack);
    if (!e || e->kind != E_FRAME)
    {
        return;
    }
    u64vec *owned = &e->u.owned;
    for (uint32_t i = 0; i < owned->n; i++)
    {
        uint64_t x = owned->p[i];
        if (x & HOLD)
        {
            plan_drop_occ(p, r, x & ~HOLD);
        }
        else
        {
            plan_destroy(p, r, x);
        }
    }
}

/* D-0202: entering to end the frame on top of this thread's stack
   (`end_owned`). */
Held enter_frame_end(void)
{
    return enter_planned(frame_end_plan, NULL);
}

/* Whether every path `tok` was formed through is in a heap this entry
   holds. */
bool ancestors_local(uint64_t tok)
{
    PathRec *p = paths_get(rt(), tok);
    if (!p)
    {
        return true;
    }
    for (uint32_t i = 0; i < p->ancestors.n; i++)
    {
        if (!holds_heap(heap_of(p->ancestors.p[i])))
        {
            return false;
        }
    }
    return true;
}

/* The first spawn turns locking on. The main thread is then inside
   `cb_spawn`'s own entry plus one entry per destructor callback it is
   running (`CALLBACK_DEPTH`); the lock is taken once for each. */
/* The MT_ pointers (rt.h), and what fills them. */
void (*MT_held_release)(uint8_t tag);
Held (*MT_enter_set)(HeapSet set);
void (*MT_enter)(void);
void (*MT_lock_release)(void);
ReadEntered (*MT_enter_read)(void);
void (*MT_read_release)(void);
Held (*MT_own_heap_shared)(void);
Held (*MT_enter_planned)(plan_fn f, void *ctx);
void (*MT_heap_page_count)(size_t h, uintptr_t a, uint64_t size, bool up);
Held (*MT_enter_elem)(uintptr_t a, uint32_t ty);
Held (*MT_enter_live)(uintptr_t lo, uintptr_t hi);
Held (*MT_enter_live_empty)(uintptr_t lo, uintptr_t hi);
Held (*MT_enter_cells)(uintptr_t a, uint64_t n);
void (*MT_fault_lock)(void);
void (*MT_contained_fault)(char *id, uint32_t file, uint32_t line, uint64_t m) CB_NORET;
void (*LIB_contained_fault)(const char *id, uint32_t file, uint32_t line, uint64_t m, uintptr_t jb) CB_NORET;

static void mt_on(void)
{
    HOOK_SET(MT_held_release, held_release);
    HOOK_SET(MT_enter_set, enter_set_multi);
    HOOK_SET(MT_enter, enter_multi);
    HOOK_SET(MT_lock_release, lock_release);
    HOOK_SET(MT_enter_read, enter_read_multi);
    HOOK_SET(MT_read_release, read_entered_release);
    HOOK_SET(MT_own_heap_shared, own_heap_shared);
    HOOK_SET(MT_enter_planned, enter_planned_multi);
    HOOK_SET(MT_heap_page_count, heap_page_count_multi);
    HOOK_SET(MT_enter_elem, enter_elem_multi);
    HOOK_SET(MT_enter_live, enter_live_multi);
    HOOK_SET(MT_enter_live_empty, enter_live_empty_multi);
    HOOK_SET(MT_enter_cells, enter_cells_multi);
    HOOK_SET(MT_fault_lock, fault_lock);
    HOOK_SET(MT_contained_fault, contained_fault);
}

void go_multi(void)
{
    if (!multi())
    {
        atomic_store_explicit(&LONE, false, memory_order_relaxed);
        mt_on();
        lock_acquire(1 + CALLBACK_DEPTH);
        rebuild_heap_pages(rt());
        atomic_store_explicit(&cb_reclaimed_entries, THREADS_ENTRIES, memory_order_relaxed);
        atomic_store_explicit(&STATS_ON, plat_getenv("COBALTC_RT_STATS") != NULL, memory_order_relaxed);
        atomic_store_explicit(&MULTI, true, memory_order_release);
    }
}

/* ---- waits ---- */

uint64_t mutex_key(uintptr_t addr)
{
    return (uint64_t)addr | (1ull << 63);
}

/* What blocked threads wait for, by key: (key, times signalled, threads
   waiting, their condition variable). A signal wakes only the threads
   waiting on that very key. The condition variable is shared by count
   (Rust's `Arc<Condvar>`): a signaller notifies it after letting the
   table's lock go. */
typedef struct
{
    plat_cond c;
    _Atomic uint32_t refs;
} cvbox;

typedef struct
{
    uint64_t key;
    uint64_t signalled;
    uint32_t waiting;
    cvbox *cv;
} Waiters;

VEC(waitersvec, Waiters)

static plat_mutex EVENTS_LOCK = PLAT_MUTEX_INIT;
static waitersvec EVENTS;

static cvbox *cv_clone(cvbox *c)
{
    atomic_fetch_add_explicit(&c->refs, 1, memory_order_relaxed);
    return c;
}

static void cv_release(cvbox *c)
{
    if (atomic_fetch_sub_explicit(&c->refs, 1, memory_order_acq_rel) == 1)
    {
        free(c);
    }
}

static Waiters *events_find(uint64_t key, size_t *at)
{
    for (size_t i = 0; i < EVENTS.n; i++)
    {
        if (EVENTS.p[i].key == key)
        {
            if (at)
            {
                *at = i;
            }
            return &EVENTS.p[i];
        }
    }
    return NULL;
}

void signal_key(uint64_t key)
{
    /* A release or a signal: another thread may go on now (a schedule's
       switch point, made at this thread's next entry holding nothing). */
    if (sched_on())
    {
        SCHED_PENDING = true;
    }
    plat_mutex_lock(&EVENTS_LOCK);
    Waiters *ev = events_find(key, NULL);
    if (ev)
    {
        ev->signalled++;
        if (ev->waiting > 0)
        {
            cvbox *cv = cv_clone(ev->cv);
            plat_mutex_unlock(&EVENTS_LOCK);
            plat_cond_broadcast(&cv->c);
            cv_release(cv);
            return;
        }
    }
    plat_mutex_unlock(&EVENTS_LOCK);
}

/* Why this thread's wait must end in a fault (D-0204), if it must: found
   deadlocked, or asked to stop -- unless it was lent an exclusive
   reference, or is already unwinding a contained fault. */
const char *stop_reason(Book *b, uint64_t m)
{
    if (hset_remove(&b->deadlocked, m, NULL))
    {
        /* Report 321: the one thread that waited on something other than
           a join or a lock (a semaphore, an atomic, a channel) when the
           deadlock was found, every other thread ended or waiting for it
           through joins and locks (`book_find_deadlock`): it waits alone,
           on something no running thread can change -- said so, not
           "each other". */
        if (hset_remove(&b->lone_waiter, m, NULL))
        {
            return m == 1 ? "diag.deadlock\x01`main` waits here forever: no other thread can end this wait (each has ended or waits too)"
                          : "diag.deadlock\x01this thread waits here forever: no other thread can end this wait (each has ended or waits too)";
        }
        return "diag.deadlock";
    }
    if (hset_has(&b->cancel, m) && !hset_has(&b->lent, m) && !ts_raw()->cunwind)
    {
        return "diag.thread-cancelled";
    }
    return NULL;
}

static bool ready_now(ready_fn ready, void *ctx)
{
    Book *b = book();
    bool r = ready(b, ctx);
    book_unlock();
    return r;
}

/* Blocks, with the runtime lock released in full, until `ready` holds.
   `ready` is evaluated only under the book's lock, and may change the book
   when it answers yes; the waiter registers on `key` before that lock is
   let go, so no change made under it is missed. An exclusive hold on the
   runtime is given up for the wait and taken again after. */
void wait_until(uint64_t key, Wait kind, ready_fn ready, void *ctx)
{
    /* A timer's wait ends at its deadline (`cb_sleep_ns`). */
    bool has_deadline = kind.k == W_TIMER && HAS_TIMER_DEADLINE;
    uint64_t deadline = has_deadline ? TIMER_DEADLINE : 0;
    /* A short wait first, asking again now and then, before sleeping (D-0202).
       Not while this thread holds the runtime exclusively. */
    if (atomic_load_explicit(&OWNER, memory_order_relaxed) != TID && !sched_on())
    {
        for (uint32_t i = 0; i < SPIN_WAITS; i++)
        {
            if (ready_now(ready, ctx))
            {
                return;
            }
            for (int k = 0; k < 64; k++)
            {
                plat_spin_hint();
            }
            if (i % 16 == 15)
            {
                plat_thread_yield();
            }
        }
    }
    /* D-0204: a wait ends in a fault when this thread is asked to stop, when
       waiting would close a cycle of joins and locks, or when it is found
       deadlocked. */
    uint64_t m = TID;
    bool timer = kind.k == W_TIMER;
    {
        Book *b = book();
        if (ready(b, ctx))
        {
            book_unlock();
            return;
        }
        const char *d = stop_reason(b, m);
        if (d)
        {
            book_unlock();
            fault(d, NOLOC, 0);
        }
        if (book_closes_cycle(b, m, &kind))
        {
            book_unlock();
            fault("diag.deadlock", NOLOC, 0);
        }
        waitmap_insert(&b->waits, m, kind, NULL);
        map_u64_insert(&b->wake, m, key, NULL);
        book_find_deadlock(b);
        book_unlock();
    }
    /* A schedule: ask, and if not yet, pass the turn. A timer's wait leaves
       the schedule until its deadline instead. */
    bool sched = sched_on() && !timer;
    bool asleep = sched_on() && timer;
    if (asleep)
    {
        sched_io(m, true);
    }
    for (;;)
    {
        if (sched)
        {
            Book *bk = book();
            if (ready(bk, ctx))
            {
                waitmap_remove(&bk->waits, m, NULL);
                map_u64_remove(&bk->wake, m, NULL);
                book_unlock();
                return;
            }
            const char *d = stop_reason(bk, m);
            if (d)
            {
                waitmap_remove(&bk->waits, m, NULL);
                map_u64_remove(&bk->wake, m, NULL);
                book_unlock();
                fault(d, NOLOC, 0);
            }
            book_unlock();
            uint32_t held = atomic_load_explicit(&OWNER, memory_order_relaxed) == m ? lock_release_all() : 0;
            if (LHELD != 0)
            {
                cbrt_internal("a scheduled wait inside a local hold");
            }
            rt_sched_yield();
            /* Every other thread outside the program: only their return can
               end this wait. */
            if (sched_alone(m))
            {
                plat_sleep_ns(200000);
            }
            if (held > 0)
            {
                lock_acquire(held);
            }
            continue;
        }
        Book *bk = book();
        if (ready(bk, ctx))
        {
            waitmap_remove(&bk->waits, m, NULL);
            map_u64_remove(&bk->wake, m, NULL);
            book_unlock();
            if (asleep)
            {
                sched_io(m, false);
            }
            return;
        }
        const char *d = stop_reason(bk, m);
        if (d)
        {
            waitmap_remove(&bk->waits, m, NULL);
            map_u64_remove(&bk->wake, m, NULL);
            book_unlock();
            if (asleep)
            {
                sched_io(m, false);
            }
            fault(d, NOLOC, 0);
        }
        uint64_t e;
        cvbox *cv;
        plat_mutex_lock(&EVENTS_LOCK);
        {
            Waiters *ev = events_find(key, NULL);
            if (ev)
            {
                ev->waiting++;
                e = ev->signalled;
                cv = cv_clone(ev->cv);
            }
            else
            {
                cvbox *c = (cvbox *)cb_xmalloc(sizeof(cvbox));
                plat_cond_init(&c->c);
                atomic_init(&c->refs, 1);
                Waiters w = {key, 0, 1, c};
                waitersvec_push(&EVENTS, w);
                e = 0;
                cv = cv_clone(c);
            }
        }
        plat_mutex_unlock(&EVENTS_LOCK);
        book_unlock();
        uint32_t held = atomic_load_explicit(&OWNER, memory_order_relaxed) == m ? lock_release_all() : 0;
        plat_mutex_lock(&EVENTS_LOCK);
        for (;;)
        {
            Waiters *ev = events_find(key, NULL);
            uint64_t now = ev ? ev->signalled : e;
            if (now != e)
            {
                break;
            }
            uint64_t wait = 50000000ull;
            if (has_deadline)
            {
                uint64_t t = plat_now_ns();
                if (t >= deadline)
                {
                    break;
                }
                if (deadline - t < wait)
                {
                    wait = deadline - t;
                }
            }
            if (!plat_cond_wait_ns(&cv->c, &EVENTS_LOCK, wait))
            {
                break;
            }
        }
        size_t i;
        Waiters *ev = events_find(key, &i);
        if (ev)
        {
            ev->waiting--;
            if (ev->waiting == 0)
            {
                cv_release(ev->cv);
                EVENTS.p[i] = EVENTS.p[EVENTS.n - 1];
                EVENTS.n--;
            }
        }
        plat_mutex_unlock(&EVENTS_LOCK);
        cv_release(cv);
        if (held > 0)
        {
            lock_acquire(held);
        }
    }
}
