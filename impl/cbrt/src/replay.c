/* D-0299 step 4: a run's steps written in the replay's action format
   (`proofs/Replay.lean`, D-0270), so that a compiled program's run is
   checked step by step against the proven machine of `proofs/` (part of
   lib.c's translation unit).

   On only when `COBALTC_REPLAY_TRACE` names a file; the trace is written
   there when the program ends. A build meant for it keeps every check
   (`cobc --all-checks`, `COBALTC_PATHLESS=0`) and records every reference
   (`COBALTC_RECORD=full`): `impl/tools/replay_runtime.py` does all of it.
   With the variable unset, every hook is one relaxed load.

   What the trace says, against the fragment's rules:
   - frames and statement scopes: `ben`, `bex`, `sen`, `sex`;
   - objects: `est` (`cb_new`), `bnd` (`cb_bind`), `dst`/`epl`, and `xfr`
     for a resource taken out of one binding (`cb_take`) and bound in
     another;
   - paths: a borrow is `prj` for each projection step, then `brw`, then
     `set` for the steps; a read `prj`…`rd`…`set`; a write `prj`…`wr`…`set`,
     the write carrying the reference it stores (`cb_store_ref`);
   - holding: a borrow is written pending (D-0107) until something holds
     it: a frame (`bpa`, or `est`/`bnd`/`wr` for a path already bound), or
     a slot (a write into its object);
   - the readings the fragment does not take yet (D-0270): `inv` when a
     path ends because its last holder let go, `rsc` when a path leaves its
     statement;
   - a fault of the alias and temporal rules as `flt DIAG` before the
     action the runtime refused; any other fault ends the trace;
   - `out REASON` where the run leaves the fragment.

   Hash-table order: the only walk over a table (`store`'s search for the
   object a slot lies in) takes the greatest id, which no order changes. */

#include <errno.h>
#include <stdio.h>

#include "rt.h"

/* 0: not yet read; 1: off; 2: on; 3: on and stopped (an `out`, a fault). */
_Atomic uint8_t RP_STATE;
static _Atomic bool RP_WRITTEN;

VEC(strvec, char *)

typedef struct
{
    u64vec scopes;
    u64vec settle;
    bool has_last_write; /* the path the last write went through */
    uint64_t last_write;
    size_t group;
    /* A statement scope the trace opened around an object established
       outside every statement (D-0196), closed once it is bound. */
    bool own_scope;
} Thr;

typedef struct
{
    uintptr_t a;
    uint64_t n;
} rp_cell;

typedef struct
{
    uintptr_t slot;
    uint64_t old;
    uint64_t tok;
} rp_orphan;

VEC(orphanvec, rp_orphan)

HMAP(rp_threads, Thr *)
HMAP(rp_cells, rp_cell)
/* Vec<Option<u64>> (0: None; the trace's own ids are above 2^60) and
   Vec<(u64, u64)> (pairs, flattened). */
HMAP(rp_lists, u64vec)

typedef struct
{
    char *path;
    strvec lines;
    /* Identities of the trace's own making: above every identity the
       runtime gives. */
    uint64_t next;
    uint64_t frames;
    uint64_t scopes;
    /* The runtime's token, as the trace names it. */
    map_u64 alias;
    /* Paths written pending and not yet bound. */
    hset pending;
    /* The holder objects the trace made for a frame's hold of a path,
       innermost last, per path. */
    rp_lists holders;
    rp_threads threads;
    /* A resource taken out of its binding, not yet bound again. */
    map_u64 taken;
    /* Each object's cells; its root path once bound; the references
       stored into it before it was bound. */
    rp_cells cells;
    map_u64 roots;
    rp_lists early;
    /* References stored into a slot before any object is established over
       it: by the slot's address. */
    orphanvec orphans;
} Trace;

static plat_mutex RP_LOCK = PLAT_MUTEX_INIT;
static Trace *TRACE;

static const char *rp_env(const char *name)
{
    const char *v = plat_getenv(name);
    if (!v)
    {
        return NULL;
    }
    /* `env::var`'s `Err` for a value that is not UTF-8: unset. */
    const uint8_t *s = (const uint8_t *)v;
    size_t n = strlen(v), i = 0;
    while (i < n)
    {
        uint8_t c = s[i];
        size_t k;
        uint32_t cp;
        if (c < 0x80)
        {
            i++;
            continue;
        }
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
            return NULL;
        }
        for (size_t j = 1; j <= k; j++)
        {
            if (i + j >= n || (s[i + j] & 0xC0) != 0x80)
            {
                return NULL;
            }
            cp = (cp << 6) | (s[i + j] & 0x3F);
        }
        if ((k == 2 && cp < 0x800) || (k == 3 && (cp < 0x10000 || cp > 0x10FFFF)) || (cp >= 0xD800 && cp <= 0xDFFF))
        {
            return NULL;
        }
        i += k + 1;
    }
    return v;
}

static char *rp_strdup(const char *s)
{
    size_t n = strlen(s);
    char *d = (char *)cb_xmalloc(n + 1);
    memcpy(d, s, n + 1);
    return d;
}

CB_COLD bool replay_init(void)
{
    const char *path = rp_env("COBALTC_REPLAY_TRACE");
    if (!path || !path[0])
    {
        atomic_store_explicit(&RP_STATE, 1, memory_order_relaxed);
        lone_set();
        return false;
    }
    plat_mutex_lock(&RP_LOCK);
    if (!TRACE)
    {
        Trace *t = (Trace *)cb_xcalloc(1, sizeof(Trace));
        t->path = rp_strdup(path);
        strvec_push(&t->lines, rp_strdup("# cbrt replay trace (D-0299 step 4)"));
        t->next = 1ull << 60;
        t->frames = 1;
        t->scopes = 1;
        TRACE = t;
    }
    plat_mutex_unlock(&RP_LOCK);
    atomic_store_explicit(&RP_STATE, 2, memory_order_relaxed);
    return true;
}

/* ---- lines: words and numbers, no printf ---- */

typedef struct
{
    char b[160];
    size_t n;
} line_t;

static void ln_s(line_t *l, const char *s)
{
    size_t k = strlen(s);
    if (l->n + k < sizeof l->b)
    {
        memcpy(l->b + l->n, s, k);
        l->n += k;
    }
}

static void ln_u(line_t *l, uint64_t v)
{
    char t[24];
    int i = 24;
    do
    {
        t[--i] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    size_t k = (size_t)(24 - i);
    if (l->n + k < sizeof l->b)
    {
        memcpy(l->b + l->n, t + i, k);
        l->n += k;
    }
}

/* A space, then the number. */
static void ln_su(line_t *l, uint64_t v)
{
    ln_s(l, " ");
    ln_u(l, v);
}

static char *ln_take(line_t *l)
{
    char *s = (char *)cb_xmalloc(l->n + 1);
    memcpy(s, l->b, l->n);
    s[l->n] = 0;
    return s;
}

static void tr_push(Trace *tr, line_t *l)
{
    strvec_push(&tr->lines, ln_take(l));
}

/* A line of a word and numbers: `w n0 n1 ...`. */
static void tr_line(Trace *tr, const char *w, int k, uint64_t a, uint64_t b, uint64_t c, uint64_t d)
{
    line_t l = {{0}, 0};
    ln_s(&l, w);
    uint64_t v[4] = {a, b, c, d};
    for (int i = 0; i < k; i++)
    {
        ln_su(&l, v[i]);
    }
    tr_push(tr, &l);
}

/* ---- the trace's own steps ---- */

static uint64_t tr_fresh(Trace *tr)
{
    tr->next += 1;
    return tr->next;
}

static uint64_t tr_name(Trace *tr, uint64_t tok)
{
    uint64_t *v = map_u64_get(&tr->alias, tok);
    return v ? *v : tok;
}

static Thr *tr_thr(Trace *tr, uint64_t t)
{
    bool had;
    Thr **p = rp_threads_slot(&tr->threads, t, &had);
    if (!had)
    {
        *p = (Thr *)cb_xcalloc(1, sizeof(Thr));
    }
    return *p;
}

/* The projection paths a read or a borrow formed, settled; done before
   any other line of the thread, except the reference a write stores. */
static void tr_settle(Trace *tr, uint64_t t)
{
    Thr *th = tr_thr(tr, t);
    u64vec s = th->settle;
    th->settle.p = NULL;
    th->settle.n = 0;
    th->settle.cap = 0;
    for (uint32_t i = s.n; i-- > 0;)
    {
        tr_line(tr, "set", 2, t, s.p[i], 0, 0);
    }
    u64vec_free(&s);
    tr_thr(tr, t)->has_last_write = false;
}

static void tr_close_own(Trace *tr, uint64_t t)
{
    Thr *th = tr_thr(tr, t);
    if (th->own_scope)
    {
        th->own_scope = false;
        if (th->scopes.n)
        {
            th->scopes.n--;
        }
        tr_line(tr, "sex", 1, t, 0, 0, 0);
    }
}

/* The start of an entry's lines. */
static void tr_begin(Trace *tr, uint64_t t, bool keep)
{
    tr_settle(tr, t);
    if (!keep)
    {
        tr_close_own(tr, t);
    }
    tr_thr(tr, t)->group = tr->lines.n;
}

/* `prj` for each step from `base`; the path the last step forms. */
static uint64_t tr_project(Trace *tr, uint64_t t, uint64_t base, const uint64_t *steps, size_t n)
{
    uint64_t cur = tr_name(tr, base);
    for (size_t i = 0; i < n; i++)
    {
        uint64_t a = tr_fresh(tr);
        tr_line(tr, "prj", 4, t, cur, a, steps[i]);
        u64vec_push(&tr_thr(tr, t)->settle, a);
        cur = a;
    }
    return cur;
}

/* A path is held by a new object of the thread's current frame: a pending
   one is bound so (`bpa`); one already bound gains a holder. 0: none. */
static uint64_t tr_hold(Trace *tr, uint64_t t, uint64_t tok)
{
    uint64_t a = tr_name(tr, tok);
    uint64_t o = tr_fresh(tr);
    uint64_t root = tr_fresh(tr);
    if (hset_remove(&tr->pending, a, NULL))
    {
        tr_line(tr, "bpa", 4, t, a, o, root);
        return o;
    }
    /* Bound already: a further holder, a temporary of the current statement
       adopted by the frame that stores it; outside any statement the path's
       holders are left as they are. */
    if (tr_thr(tr, t)->scopes.n == 0)
    {
        return 0;
    }
    tr_line(tr, "est", 3, t, o, 0, 0);
    tr_line(tr, "bnd", 3, t, o, root, 0);
    line_t l = {{0}, 0};
    ln_s(&l, "wr");
    ln_su(&l, t);
    ln_su(&l, root);
    ln_s(&l, " 0 1");
    ln_su(&l, a);
    tr_push(tr, &l);
    return o;
}

/* `[Write]` through `a` of a slot that held `old` and now holds `tok`. */
static void tr_store_via(Trace *tr, uint64_t t, uint64_t a, uint64_t old, uint64_t tok)
{
    uint64_t nw = tr_name(tr, tok);
    if (tok != 0 && hset_has(&tr->pending, nw))
    {
        /* A stored reference is bound (D-0107: no pending path is stored):
           the trace's holder object binds it first. */
        tr_hold(tr, t, tok);
    }
    line_t l = {{0}, 0};
    ln_s(&l, "wr");
    ln_su(&l, t);
    ln_su(&l, tr_name(tr, a));
    if (old != 0)
    {
        ln_s(&l, " 1");
        ln_su(&l, tr_name(tr, old));
    }
    else
    {
        ln_s(&l, " 0");
    }
    if (tok != 0)
    {
        ln_s(&l, " 1");
        ln_su(&l, nw);
    }
    else
    {
        ln_s(&l, " 0");
    }
    tr_push(tr, &l);
}

/* The thread as the trace numbers it: the main thread is 0. */
static uint64_t rp_tid(uint64_t m)
{
    return m ? m - 1 : 0;
}

/* `emit_on`: the entry's lines under the trace's lock, the thread's
   pending settles first; `keep`: an object's establishment and binding
   keep the scope the trace opened for them. Returns the trace locked (the
   caller unlocks with `rp_done`), or NULL. */
static Trace *rp_begin(uint64_t m, bool keep, uint64_t *t)
{
    plat_mutex_lock(&RP_LOCK);
    Trace *tr = TRACE;
    if (!tr)
    {
        plat_mutex_unlock(&RP_LOCK);
        return NULL;
    }
    *t = rp_tid(m);
    tr_begin(tr, *t, keep);
    return tr;
}

static Trace *rp_with(void)
{
    plat_mutex_lock(&RP_LOCK);
    Trace *tr = TRACE;
    if (!tr)
    {
        plat_mutex_unlock(&RP_LOCK);
    }
    return tr;
}

static void rp_done(void)
{
    plat_mutex_unlock(&RP_LOCK);
}

/* The run leaves the fragment: the trace ends here. */
CB_COLD static void replay_out_on(uint64_t m, const char *why)
{
    Trace *tr = rp_with();
    if (tr)
    {
        uint64_t t = rp_tid(m);
        tr_settle(tr, t);
        cb_buf b = {0};
        cb_buf_str(&b, "out ");
        cb_buf_str(&b, why);
        cb_buf_byte(&b, 0);
        strvec_push(&tr->lines, (char *)b.p);
        rp_done();
    }
    atomic_store_explicit(&RP_STATE, 3, memory_order_relaxed);
}

void replay_out(uint64_t m, const char *why)
{
    if (replay_on())
    {
        replay_out_on(m, why);
    }
}

void replay_frame_push(uint64_t m)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t f = tr->frames++;
        tr_line(tr, "ben", 2, t, f, 0, 0);
        rp_done();
    }
}

void replay_frame_pop(uint64_t m)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        tr_line(tr, "bex", 1, t, 0, 0, 0);
        rp_done();
    }
}

void replay_stmt_push(uint64_t m)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t s = tr->scopes++;
        u64vec_push(&tr_thr(tr, t)->scopes, s);
        tr_line(tr, "sen", 2, t, s, 0, 0);
        rp_done();
    }
}

void replay_stmt_pop(uint64_t m)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        Thr *th = tr_thr(tr, t);
        if (th->scopes.n)
        {
            th->scopes.n--;
        }
        tr_line(tr, "sex", 1, t, 0, 0, 0);
        rp_done();
    }
}

static void lists_drop(rp_lists *m, uint64_t k)
{
    u64vec v;
    if (rp_lists_remove(m, k, &v))
    {
        u64vec_free(&v);
    }
}

void replay_establish(uint64_t m, uint64_t obj, bool resource, uintptr_t addr, uint64_t size)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, true, &t);
    if (!tr)
    {
        return;
    }
    Thr *th = tr_thr(tr, t);
    if (th->scopes.n == 0 && !th->own_scope)
    {
        uint64_t s = tr->scopes++;
        th->own_scope = true;
        u64vec_push(&th->scopes, s);
        tr_line(tr, "sen", 2, t, s, 0, 0);
    }
    rp_cell c = {addr, size};
    rp_cells_insert(&tr->cells, obj, c, NULL);
    map_u64_remove(&tr->roots, obj, NULL);
    lists_drop(&tr->early, obj);
    tr_line(tr, "est", 3, t, obj, resource ? 1 : 0, 0);
    /* The orphans in its cells become its early stores, in order. */
    u64vec mine = {0};
    size_t j = 0;
    for (size_t i = 0; i < tr->orphans.n; i++)
    {
        rp_orphan o = tr->orphans.p[i];
        if (addr <= o.slot && o.slot + 8 <= addr + (uintptr_t)size)
        {
            u64vec_push(&mine, o.old);
            u64vec_push(&mine, o.tok);
        }
        else
        {
            tr->orphans.p[j++] = o;
        }
    }
    tr->orphans.n = j;
    if (mine.n)
    {
        rp_lists_insert(&tr->early, obj, mine, NULL);
    }
    rp_done();
}

/* An object's identity has ended: its cells no longer place a slot. */
void replay_forget(uint64_t obj)
{
    if (!replay_on())
    {
        return;
    }
    Trace *tr = rp_with();
    if (tr)
    {
        rp_cells_remove(&tr->cells, obj, NULL);
        map_u64_remove(&tr->roots, obj, NULL);
        lists_drop(&tr->early, obj);
        rp_done();
    }
}

void replay_moved(uint64_t obj, uintptr_t addr)
{
    if (!replay_on())
    {
        return;
    }
    Trace *tr = rp_with();
    if (tr)
    {
        rp_cell *c = rp_cells_get(&tr->cells, obj);
        if (c)
        {
            c->a = addr;
        }
        rp_done();
    }
}

void replay_bind(uint64_t m, uint64_t obj, uint64_t root)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, true, &t);
    if (!tr)
    {
        return;
    }
    map_u64_remove(&tr->alias, root, NULL);
    uint64_t old;
    if (map_u64_remove(&tr->taken, obj, &old))
    {
        /* `[Authority-Transfer]`: the resource taken out of one binding is
           bound here, by the thread that took it. */
        uint64_t o = tr_name(tr, old);
        line_t l = {{0}, 0};
        ln_s(&l, "xfr");
        ln_su(&l, t);
        ln_su(&l, t);
        ln_su(&l, o);
        ln_su(&l, obj);
        ln_su(&l, obj);
        ln_su(&l, root);
        tr_push(tr, &l);
    }
    else
    {
        tr_line(tr, "bnd", 3, t, obj, root, 0);
    }
    map_u64_insert(&tr->roots, obj, root, NULL);
    /* References stored into it before it was bound, through its root. */
    u64vec early;
    if (rp_lists_remove(&tr->early, obj, &early))
    {
        for (uint32_t i = 0; i + 1 < early.n; i += 2)
        {
            tr_store_via(tr, t, root, early.p[i], early.p[i + 1]);
        }
        u64vec_free(&early);
    }
    tr_close_own(tr, t);
    rp_done();
}

void replay_take(uint64_t m, uint64_t obj, uint64_t root)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        map_u64_insert(&tr->taken, obj, root, NULL);
        rp_done();
    }
}

/* An object ends by itself: `drop(x)`, a binding ended at its last use, a
   temporary consumed. */
void replay_end(uint64_t m, uint64_t obj, bool resource)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        map_u64_remove(&tr->taken, obj, NULL);
        tr_line(tr, resource ? "dst" : "epl", 2, t, obj, 0, 0);
        rp_done();
    }
}

/* `[Borrow]` from `base` through `steps`: the trace's name for the new
   path, which the runtime gives once it is minted (`replay_minted`). */
uint64_t replay_borrow(uint64_t m, uint64_t base, const uint64_t *steps, size_t n, bool exclusive)
{
    uint64_t a = 0;
    if (!replay_on())
    {
        return 0;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t from = tr_project(tr, t, base, steps, n);
        a = tr_fresh(tr);
        hset_add(&tr->pending, a);
        line_t l = {{0}, 0};
        ln_s(&l, "brw");
        ln_su(&l, t);
        ln_su(&l, from);
        ln_su(&l, a);
        ln_su(&l, exclusive ? 1 : 0);
        ln_s(&l, " 1");
        tr_push(tr, &l);
        rp_done();
    }
    return a;
}

void replay_minted(uint64_t named, uint64_t tok)
{
    if (named == 0 || !replay_on())
    {
        return;
    }
    Trace *tr = rp_with();
    if (tr)
    {
        map_u64_insert(&tr->alias, tok, named, NULL);
        rp_done();
    }
}

void replay_read(uint64_t m, uint64_t base, const uint64_t *steps, size_t n)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t a = tr_project(tr, t, base, steps, n);
        tr_line(tr, "rd", 2, t, a, 0, 0);
        rp_done();
    }
}

void replay_write(uint64_t m, uint64_t base, const uint64_t *steps, size_t n)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t a = tr_project(tr, t, base, steps, n);
        line_t l = {{0}, 0};
        ln_s(&l, "wr");
        ln_su(&l, t);
        ln_su(&l, a);
        ln_s(&l, " 0 0");
        tr_push(tr, &l);
        Thr *th = tr_thr(tr, t);
        th->has_last_write = true;
        th->last_write = a;
        rp_done();
    }
}

/* A reference stored into a slot (`old`: what the slot held): by the write
   the thread made just before; or into an object established and not yet
   bound, written through its root once bound; or into a bound object with
   no write before it, through its root. */
void replay_store(uint64_t m, uintptr_t slot, uint64_t old, uint64_t tok)
{
    if (!replay_on())
    {
        return;
    }
    Trace *tr = rp_with();
    if (!tr)
    {
        replay_out(m, "a reference stored into data the trace cannot place");
        return;
    }
    uint64_t t = rp_tid(m);
    Thr *th = tr_thr(tr, t);
    if (th->has_last_write)
    {
        tr_store_via(tr, t, th->last_write, old, tok);
        rp_done();
        return;
    }
    /* The object the slot is in: the latest established that contains it
       (the greatest id, whatever the table's order). */
    bool found = false;
    uint64_t o = 0;
    for (size_t i = 0; i < tr->cells.cap; i++)
    {
        if (tr->cells.used[i])
        {
            rp_cell c = tr->cells.e[i].v;
            if (c.a <= slot && slot + 8 <= c.a + (uintptr_t)c.n)
            {
                uint64_t k = tr->cells.e[i].k;
                if (!found || k > o)
                {
                    o = k;
                }
                found = true;
            }
        }
    }
    if (!found)
    {
        rp_orphan x = {slot, old, tok};
        orphanvec_push(&tr->orphans, x);
        rp_done();
        return;
    }
    uint64_t *root = map_u64_get(&tr->roots, o);
    if (root)
    {
        tr_store_via(tr, t, *root, old, tok);
    }
    else
    {
        bool had;
        u64vec *v = rp_lists_slot(&tr->early, o, &had);
        if (!had)
        {
            memset(v, 0, sizeof *v);
        }
        u64vec_push(v, old);
        u64vec_push(v, tok);
    }
    rp_done();
}

void replay_frame_hold(uint64_t m, uint64_t tok)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t a = tr_name(tr, tok);
        uint64_t o = tr_hold(tr, t, tok);
        bool had;
        u64vec *v = rp_lists_slot(&tr->holders, a, &had);
        if (!had)
        {
            memset(v, 0, sizeof *v);
        }
        u64vec_push(v, o);
        rp_done();
    }
}

/* A parameter's reference: held by a parameter object of the callee's
   frame, which ends with the frame. */
void replay_param(uint64_t m, uint64_t tok)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        tr_hold(tr, t, tok);
        rp_done();
    }
}

void replay_frame_unhold(uint64_t m, uint64_t tok)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t a = tr_name(tr, tok);
        u64vec *v = rp_lists_get(&tr->holders, a);
        if (v && v->n)
        {
            uint64_t o = v->p[--v->n];
            if (o != 0)
            {
                tr_line(tr, "epl", 2, t, o, 0, 0);
            }
        }
        rp_done();
    }
}

/* A scope no thread has open: a path re-stamped to it outlives every
   statement until something holds it. */
#define RP_NONE ((uint64_t)UINT32_MAX)

/* A path that leaves its statement: a result for the parent statement, or
   one in flight (held by no scope). */
void replay_restamp(uint64_t m, uint64_t tok, bool parent)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t a = tr_name(tr, tok);
        Thr *th = tr_thr(tr, t);
        uint64_t s = RP_NONE;
        if (parent && th->scopes.n >= 2)
        {
            s = th->scopes.p[th->scopes.n - 2];
        }
        tr_line(tr, "rsc", 2, a, s, 0, 0);
        rp_done();
    }
}

void replay_flight_in(uint64_t m, uint64_t tok)
{
    if (!replay_on())
    {
        return;
    }
    uint64_t t;
    Trace *tr = rp_begin(m, false, &t);
    if (tr)
    {
        uint64_t a = tr_name(tr, tok);
        Thr *th = tr_thr(tr, t);
        uint64_t s = th->scopes.n ? th->scopes.p[th->scopes.n - 1] : RP_NONE;
        tr_line(tr, "rsc", 2, a, s, 0, 0);
        rp_done();
    }
}

/* A path ended because its last holder let go (`[Object-End]`'s `Dead`). */
void replay_dead(uint64_t tok)
{
    if (!replay_on())
    {
        return;
    }
    Trace *tr = rp_with();
    if (tr)
    {
        tr_line(tr, "inv", 1, tr_name(tr, tok), 0, 0, 0);
        rp_done();
    }
}

/* A fault: one of the alias and temporal rules' is written before the
   lines of the entry that raised it, which the machine must refuse too;
   any other ends the trace. */
static const char *const COMPARED[4] = {"diag.aliasing-conflict", "diag.stale-binding", "diag.destroy-while-aliased", "diag.move-while-aliased"};

void replay_fault(uint64_t m, const char *id)
{
    if (!replay_on())
    {
        return;
    }
    /* The diagnostic's name: up to the first character that is not an ASCII
       letter, digit, `.` or `-` (a non-ASCII byte ends it too). */
    size_t k = 0;
    while (id[k])
    {
        char c = id[k];
        if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '.' || c == '-'))
        {
            break;
        }
        k++;
    }
    char *diag = (char *)cb_xmalloc(k + 1);
    memcpy(diag, id, k);
    diag[k] = 0;
    Trace *tr = rp_with();
    if (tr)
    {
        uint64_t t = rp_tid(m);
        size_t g = tr_thr(tr, t)->group;
        if (g > tr->lines.n)
        {
            g = tr->lines.n;
        }
        bool compared = false;
        for (int i = 0; i < 4; i++)
        {
            if (strcmp(diag, COMPARED[i]) == 0)
            {
                compared = true;
            }
        }
        cb_buf b = {0};
        if (compared && g < tr->lines.n)
        {
            cb_buf_str(&b, "flt ");
            cb_buf_str(&b, diag);
            cb_buf_byte(&b, 0);
            strvec_push(&tr->lines, NULL);
            memmove(&tr->lines.p[g + 1], &tr->lines.p[g], (tr->lines.n - 1 - g) * sizeof(char *));
            tr->lines.p[g] = (char *)b.p;
        }
        else
        {
            cb_buf_str(&b, "out a fault the trace does not compare: ");
            cb_buf_str(&b, diag);
            cb_buf_byte(&b, 0);
            strvec_push(&tr->lines, (char *)b.p);
        }
        rp_done();
    }
    free(diag);
    atomic_store_explicit(&RP_STATE, 3, memory_order_relaxed);
}

/* The program ends: the trace is written (`std::fs::write`: created or
   truncated, then written whole). */
void replay_finish(void)
{
    if (atomic_load_explicit(&RP_STATE, memory_order_relaxed) < 2 || atomic_exchange_explicit(&RP_WRITTEN, true, memory_order_relaxed))
    {
        return;
    }
    Trace *tr = rp_with();
    if (!tr)
    {
        return;
    }
    cb_buf text = {0};
    for (size_t i = 0; i < tr->lines.n; i++)
    {
        if (i)
        {
            cb_buf_byte(&text, '\n');
        }
        cb_buf_str(&text, tr->lines.p[i]);
    }
    cb_buf_byte(&text, '\n');
    FILE *f = fopen(tr->path, "wb");
    int err = 0;
    if (!f)
    {
        err = errno;
    }
    else
    {
        if (fwrite(text.p, 1, text.n, f) != text.n)
        {
            err = errno ? errno : EIO;
        }
        if (fclose(f) != 0 && err == 0)
        {
            err = errno ? errno : EIO;
        }
    }
    if (err)
    {
        /* Rust's `io::Error` display: the OS's text and its code. */
        cb_buf m = {0};
        cb_buf_str(&m, "cbrt: cannot write the replay trace ");
        cb_buf_str(&m, tr->path);
        cb_buf_str(&m, ": ");
        cb_buf_str(&m, strerror(err));
        cb_buf_str(&m, " (os error ");
        char t[16];
        int i = 16;
        unsigned v = (unsigned)err;
        do
        {
            t[--i] = (char)('0' + v % 10);
            v /= 10;
        } while (v);
        cb_buf_put(&m, t + i, (size_t)(16 - i));
        cb_buf_str(&m, ")\n");
        fwrite(m.p, 1, m.n, stderr);
        cb_buf_free(&m);
    }
    cb_buf_free(&text);
    rp_done();
}
