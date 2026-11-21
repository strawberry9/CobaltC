/* The runtime's tables: the reference slots by address in pages, an
   object's path list, projections and their normalized comparison, the
   marks (D-0293), the reclaimed objects by address, and the page buckets
   that let the generated code and local entries skip them (D-0202). Part
   of lib.c's translation unit. */

#include "rt.h"

/* ---- vectors ---- */

void projvec_grow(projvec *v, size_t need)
{
    size_t c = v->cap ? (size_t)v->cap * 2 : 4;
    if (c < need)
    {
        c = need;
    }
    if (c > 0xffffffffu)
    {
        cbrt_internal("a projection of more than 2^32 steps");
    }
    v->p = (Proj *)cb_xrealloc(v->p, c * sizeof(Proj));
    v->cap = (uint32_t)c;
}

/* ---- the reference slots by address (`SlotMap`) ----

   A hash of 4 KiB pages, each a direct array of its 512 pointer-sized
   positions. Point operations (`cb_store_ref`, `cb_load_ref`) are a hash
   probe and an index; a range is visited page by page, in address order.
   An ordered tree of every slot was a deep descent per operation once a
   program held many references; sorted lists per page shifted on every
   slot a recursion's frames took. Slots at addresses not 8-aligned (none
   in practice) are kept apart (`odd`) so pages stay direct arrays; emptied
   pages, all zeros again, are kept for reuse (`spare`); the page used last
   is remembered (`last`). */

SlotPage SLOT_NONE;

static inline SlotPage *slotmap_page(SlotMap *m, uintptr_t p)
{
    if (m->last_p == p)
    {
        return m->last_ptr;
    }
    SlotPage **b = pagemap_get(&m->pages, p);
    if (b)
    {
        slotmap_last(m, p, *b);
        return *b;
    }
    return NULL;
}

static void slotmap_drop_page(SlotMap *m, uintptr_t p)
{
    if (m->last_p == p)
    {
        slotmap_last_none(m);
    }
    SlotPage *pg;
    if (pagemap_remove(&m->pages, p, &pg))
    {
        heap_page_count(m->heap, p << SLOT_PAGE, 1, false);
        if (m->nspare < 256)
        {
            if (m->nspare == m->spare_cap)
            {
                m->spare_cap = m->spare_cap ? m->spare_cap * 2 : 16;
                m->spare = (SlotPage **)cb_xrealloc(m->spare, m->spare_cap * sizeof(SlotPage *));
            }
            m->spare[m->nspare++] = pg;
        }
        else
        {
            free(pg);
        }
    }
}

/* Page `p`, `pg`, holds no slot now: it stays as the one kept empty page,
   and the page kept before it, if still empty, goes. */
static void slotmap_emptied(SlotMap *m, uintptr_t p, SlotPage *pg)
{
    if (m->kept && m->kept != pg && m->kept->n == 0)
    {
        slotmap_drop_page(m, m->kept_p);
    }
    m->kept = pg;
    m->kept_p = p;
}

uint64_t slotmap_get(SlotMap *m, uintptr_t a)
{
    if (a & 7)
    {
        uint64_t v;
        return omap_get(&m->odd, a, &v) ? v : 0;
    }
    SlotPage *pg = slotmap_page(m, a >> SLOT_PAGE);
    if (!pg)
    {
        return 0;
    }
    return pg->s[(a >> 3) & 511];
}

/* 1 once a reference was stored into data (`cbrt.h`): never 0 again. */
uint8_t cb_slots_used;

uint64_t slotmap_insert(SlotMap *m, uintptr_t a, uint64_t t)
{
    if (UNLIKELY(!HOOK(cb_slots_used)))
    {
        full_slots();
        __atomic_store_n(&cb_slots_used, 1, __ATOMIC_RELAXED);
    }
    if (a & 7)
    {
        uint64_t old = 0;
        if (!omap_insert(&m->odd, a, t, &old))
        {
            heap_page_count(m->heap, a, 1, true);
            return 0;
        }
        return old;
    }
    uintptr_t p = a >> SLOT_PAGE;
    SlotPage *pg = slotmap_page(m, p);
    if (!pg)
    {
        if (m->nspare)
        {
            pg = m->spare[--m->nspare];
        }
        else
        {
            pg = (SlotPage *)cb_xcalloc(1, sizeof(SlotPage));
        }
        pagemap_insert(&m->pages, p, pg, NULL);
        heap_page_count(m->heap, p << SLOT_PAGE, 1, true);
        slotmap_last(m, p, pg);
    }
    size_t i = (a >> 3) & 511;
    uint64_t old = pg->s[i];
    pg->s[i] = t;
    if (old == 0)
    {
        pg->n++;
        return 0;
    }
    return old;
}

uint64_t slotmap_remove(SlotMap *m, uintptr_t a)
{
    if (a & 7)
    {
        uint64_t old;
        if (omap_remove(&m->odd, a, &old))
        {
            heap_page_count(m->heap, a, 1, false);
            return old;
        }
        return 0;
    }
    uintptr_t p = a >> SLOT_PAGE;
    SlotPage *pg = slotmap_page(m, p);
    if (!pg)
    {
        return 0;
    }
    size_t i = (a >> 3) & 511;
    uint64_t old = pg->s[i];
    if (old == 0)
    {
        return 0;
    }
    pg->s[i] = 0;
    pg->n--;
    if (pg->n == 0)
    {
        slotmap_emptied(m, p, pg);
    }
    return old;
}

/* Hash-table order (LOG.md): where fewer pages are held than the range
   spans, the held pages are sorted first, so the pages are visited in
   address order in both cases. `each_in`'s callers only count or retire,
   see LOG.md. */
static size_t held_pages_in(SlotMap *m, uintptr_t p0, uintptr_t p1, uintptr_t **out)
{
    size_t k = 0;
    uintptr_t *v = (uintptr_t *)cb_xmalloc(m->pages.n * sizeof(uintptr_t));
    for (size_t i = 0; i < m->pages.cap; i++)
    {
        if (m->pages.used[i])
        {
            uintptr_t p = m->pages.e[i].k;
            if (p >= p0 && p <= p1)
            {
                v[k++] = p;
            }
        }
    }
    /* insertion sort: few pages */
    for (size_t i = 1; i < k; i++)
    {
        uintptr_t x = v[i];
        size_t j = i;
        while (j > 0 && v[j - 1] > x)
        {
            v[j] = v[j - 1];
            j--;
        }
        v[j] = x;
    }
    *out = v;
    return k;
}

void slotmap_each_in(SlotMap *m, uintptr_t lo, uintptr_t hi, tok_fn f, void *ctx)
{
    if (lo >= hi || slotmap_is_empty(m))
    {
        return;
    }
    uintptr_t p0 = lo >> SLOT_PAGE, p1 = (hi - 1) >> SLOT_PAGE;
#define VISIT_EACH(P, PG)                                                        \
    do                                                                           \
    {                                                                            \
        uintptr_t base_ = (P) << SLOT_PAGE;                                      \
        uintptr_t to_ = hi < base_ + ((uintptr_t)1 << SLOT_PAGE) ? hi : base_ + ((uintptr_t)1 << SLOT_PAGE); \
        uintptr_t a_ = ((lo > base_ ? lo : base_) + 7) & ~(uintptr_t)7;          \
        while (a_ < to_)                                                         \
        {                                                                        \
            uint64_t t_ = (PG)->s[(a_ >> 3) & 511];                              \
            if (t_ != 0)                                                         \
            {                                                                    \
                f(t_, ctx);                                                      \
            }                                                                    \
            a_ += 8;                                                             \
        }                                                                        \
    } while (0)
    if (m->pages.n)
    {
        if (p1 - p0 >= m->pages.n)
        {
            uintptr_t *ps;
            size_t k = held_pages_in(m, p0, p1, &ps);
            for (size_t i = 0; i < k; i++)
            {
                SlotPage *pg = *pagemap_get(&m->pages, ps[i]);
                VISIT_EACH(ps[i], pg);
            }
            free(ps);
        }
        else
        {
            for (uintptr_t p = p0; p <= p1; p++)
            {
                SlotPage **pg = pagemap_get(&m->pages, p);
                if (pg)
                {
                    VISIT_EACH(p, *pg);
                }
            }
        }
    }
#undef VISIT_EACH
    if (m->odd.n)
    {
        omap_it it = omap_lower_bound(&m->odd, lo);
        uint64_t k, t;
        while (omap_it_next(&it, &k, &t) && k < hi)
        {
            f(t, ctx);
        }
    }
}

static bool page_hit(const SlotPage *pg, uintptr_t p, uintptr_t lo, uintptr_t hi)
{
    uintptr_t base = p << SLOT_PAGE;
    uintptr_t end = base + ((uintptr_t)1 << SLOT_PAGE);
    uintptr_t to = hi < end ? hi : end;
    uintptr_t a = ((lo > base ? lo : base) + 7) & ~(uintptr_t)7;
    while (a < to)
    {
        if (pg->s[(a >> 3) & 511] != 0)
        {
            return true;
        }
        a += 8;
    }
    return false;
}

bool slotmap_any_in(SlotMap *m, uintptr_t lo, uintptr_t hi)
{
    if (lo >= hi || slotmap_is_empty(m))
    {
        return false;
    }
    uintptr_t p0 = lo >> SLOT_PAGE, p1 = (hi - 1) >> SLOT_PAGE;
    if (m->pages.n)
    {
        if (p1 - p0 >= m->pages.n)
        {
            for (size_t i = 0; i < m->pages.cap; i++)
            {
                if (m->pages.used[i])
                {
                    uintptr_t p = m->pages.e[i].k;
                    if (p >= p0 && p <= p1 && page_hit(m->pages.e[i].v, p, lo, hi))
                    {
                        return true;
                    }
                }
            }
        }
        else
        {
            for (uintptr_t p = p0; p <= p1; p++)
            {
                SlotPage **pg = pagemap_get(&m->pages, p);
                if (pg && page_hit(*pg, p, lo, hi))
                {
                    return true;
                }
            }
        }
    }
    if (m->odd.n)
    {
        omap_it it = omap_lower_bound(&m->odd, lo);
        uint64_t k;
        if (omap_it_next(&it, &k, NULL) && k < hi)
        {
            return true;
        }
    }
    return false;
}

static inline __attribute__((always_inline)) void scan_page(SlotPage *pg, uintptr_t p, uintptr_t lo, uintptr_t hi, bool take, atvec *out, uintptr_t *emptied, size_t *nempt)
{
    uintptr_t base = p << SLOT_PAGE;
    uintptr_t end = base + ((uintptr_t)1 << SLOT_PAGE);
    uintptr_t from = lo > base ? lo : base;
    uintptr_t to = hi < end ? hi : end;
    uintptr_t a = (from + 7) & ~(uintptr_t)7;
    while (a < to)
    {
        uint64_t *e = &pg->s[(a >> 3) & 511];
        if (*e != 0)
        {
            atvec_push(out, a, *e);
            if (take)
            {
                *e = 0;
                pg->n--;
            }
        }
        a += 8;
    }
    if (take && pg->n == 0)
    {
        emptied[(*nempt)++] = p;
    }
}

void slotmap_scan_into(SlotMap *m, uintptr_t lo, uintptr_t hi, bool take, atvec *out)
{
    if (lo >= hi || slotmap_is_empty(m))
    {
        return;
    }
    size_t start = out->n;
    uintptr_t p0 = lo >> SLOT_PAGE, p1 = (hi - 1) >> SLOT_PAGE;
    /* The pages a `take` empties, dropped after the scan: on the stack for a
       range of a few pages, so that such a scan allocates nothing (a
       `Queue` that grows scans the range it moves out of at each move). */
    uintptr_t small[32];
    uintptr_t *emptied = small;
    size_t nempt = 0;
    if (m->pages.n)
    {
        if (p1 == p0)
        {
            SlotPage *pg = slotmap_page(m, p0);
            if (pg)
            {
                scan_page(pg, p0, lo, hi, take, out, emptied, &nempt);
                if (nempt)
                {
                    slotmap_emptied(m, p0, pg);
                    nempt = 0;
                }
            }
        }
        else if (p1 - p0 >= m->pages.n)
        {
            if (m->pages.n > 32)
            {
                emptied = (uintptr_t *)cb_xmalloc(m->pages.n * sizeof(uintptr_t));
            }
            for (size_t i = 0; i < m->pages.cap; i++)
            {
                if (m->pages.used[i])
                {
                    uintptr_t p = m->pages.e[i].k;
                    if (p >= p0 && p <= p1)
                    {
                        scan_page(m->pages.e[i].v, p, lo, hi, take, out, emptied, &nempt);
                    }
                }
            }
            atvec_sort_from(out, start);
        }
        else
        {
            size_t span = (size_t)(p1 - p0 + 1);
            if (span > 32)
            {
                emptied = (uintptr_t *)cb_xmalloc(span * sizeof(uintptr_t));
            }
            for (uintptr_t p = p0; p <= p1; p++)
            {
                SlotPage **pg = pagemap_get(&m->pages, p);
                if (pg)
                {
                    scan_page(*pg, p, lo, hi, take, out, emptied, &nempt);
                }
            }
        }
    }
    for (size_t i = 0; i < nempt; i++)
    {
        SlotPage **pg = pagemap_get(&m->pages, emptied[i]);
        if (pg)
        {
            slotmap_emptied(m, emptied[i], *pg);
        }
    }
    if (emptied != small)
    {
        free(emptied);
    }
    if (m->odd.n)
    {
        size_t before = out->n;
        omap_it it = omap_lower_bound(&m->odd, lo);
        uint64_t k, t;
        while (omap_it_next(&it, &k, &t) && k < hi)
        {
            atvec_push(out, (uintptr_t)k, t);
        }
        if (take)
        {
            for (size_t i = before; i < out->n; i++)
            {
                omap_remove(&m->odd, out->p[i].a, NULL);
                heap_page_count(m->heap, out->p[i].a, 1, false);
            }
        }
        atvec_sort_from(out, start);
    }
}

/* `give_vec` when the pool's list is full or the block is none. */
void give_vec_more(u64vecvec *pool, u64vec v)
{
    if (v.cap > 0 && pool->n < 4096)
    {
        v.n = 0;
        u64vecvec_push(pool, v);
    }
    else
    {
        u64vec_free(&v);
    }
}

/* ---- TokList ----

   The paths of one object (`Obj::toks`, `Obj::xtoks`): appended when a
   path is minted, removed when it is retired, scanned by every access
   check. Small lists stay a plain scan; past `TOK_INDEX_AT` entries a
   position index makes removal a constant-time `swap_remove`. The order
   of the entries is not meaningful: every check over them faults with the
   same diagnostic whichever clash it meets. */

void toklist_push_more(toklist *l, uint64_t t)
{
    u64vec_push(&l->v, t);
    if (l->at)
    {
        map_u64_insert(l->at, t, l->v.n - 1, NULL);
    }
    else if (l->v.n > TOK_INDEX_AT)
    {
        l->at = (map_u64 *)cb_xcalloc(1, sizeof(map_u64));
        for (uint32_t i = 0; i < l->v.n; i++)
        {
            map_u64_insert(l->at, l->v.p[i], i, NULL);
        }
    }
}

void toklist_remove_more(toklist *l, uint64_t t)
{
    if (!l->at)
    {
        for (uint32_t i = 0; i < l->v.n; i++)
        {
            if (l->v.p[i] == t)
            {
                l->v.p[i] = l->v.p[l->v.n - 1];
                l->v.n--;
                return;
            }
        }
        return;
    }
    uint64_t i;
    if (!map_u64_remove(l->at, t, &i))
    {
        return;
    }
    l->v.p[i] = l->v.p[l->v.n - 1];
    l->v.n--;
    if (i < l->v.n)
    {
        map_u64_insert(l->at, l->v.p[i], i, NULL);
    }
}

/* ---- normalized projections ---- */

void norm_push(Norm *n, Proj x)
{
    if (n->spill.n)
    {
        projvec_push(&n->spill, x);
    }
    else if (n->len < NORM_INLINE)
    {
        n->buf[n->len++] = x;
    }
    else
    {
        projvec_extend(&n->spill, n->buf, NORM_INLINE);
        projvec_push(&n->spill, x);
    }
}

void norm_set_last(Norm *n, Proj x)
{
    if (n->spill.n)
    {
        n->spill.p[n->spill.n - 1] = x;
    }
    else
    {
        n->buf[n->len - 1] = x;
    }
}

void norm_of(Norm *n, const Proj *p, size_t len)
{
    n->len = 0;
    n->spill.p = NULL;
    n->spill.n = 0;
    n->spill.cap = 0;
    for (size_t i = 0; i < len; i++)
    {
        Proj x = p[i];
        const Proj *last = NULL;
        if (n->spill.n)
        {
            last = &n->spill.p[n->spill.n - 1];
        }
        else if (n->len)
        {
            last = &n->buf[n->len - 1];
        }
        if (last && last->k == P_RANGE && x.k == P_INDEX)
        {
            norm_set_last(n, proj_index(last->a + x.a));
        }
        else
        {
            norm_push(n, x);
        }
    }
}

/* `Norm`'s folding as an iterator (`NormIter`), for comparing a stored
   path without building it. */
bool norm_iter_next(NormIter *it, Proj *out)
{
    if (it->i >= it->n)
    {
        return false;
    }
    Proj x = it->s[it->i++];
    if (x.k == P_RANGE && it->i < it->n && it->s[it->i].k == P_INDEX)
    {
        *out = proj_index(x.a + it->s[it->i].a);
        it->i++;
        return true;
    }
    *out = x;
    return true;
}

/* Whether two paths overlap, `a` normalized, `b` as stored: one a prefix
   of the other, step by step, a range meeting an index inside it or a
   range it intersects. */
bool ranged_overlap_with(const Proj *a, size_t an, const Proj *b, size_t bn)
{
    NormIter it = {b, bn, 0};
    size_t i = 0;
    Proj y;
    while (i < an && norm_iter_next(&it, &y))
    {
        Proj x = a[i++];
        bool ok;
        if (x.k == P_RANGE && y.k == P_INDEX)
        {
            ok = x.a <= y.a && y.a < x.b;
        }
        else if (x.k == P_INDEX && y.k == P_RANGE)
        {
            ok = y.a <= x.a && x.a < y.b;
        }
        else if (x.k == P_RANGE && y.k == P_RANGE)
        {
            uint64_t l = x.a > y.a ? x.a : y.a;
            uint64_t h = x.b < y.b ? x.b : y.b;
            ok = l < h;
        }
        else if ((x.k == P_RANGE && y.k == P_FIELD) || (x.k == P_FIELD && y.k == P_RANGE))
        {
            /* A slice of a `Vec` and the `Vec`'s own fields reach one
               storage (spec/16 `[Slice-Form]`, D-0070). */
            ok = true;
        }
        else
        {
            ok = proj_eq(x, y);
        }
        if (!ok)
        {
            return false;
        }
    }
    return true;
}

/* ---- marks (D-0293) ----

   Per heap, one byte per object slot, nonzero while the object has paths
   on it: a light token whose object is unmarked needs no check a call
   could make (`cb_vouched` in cbrt.h). An array that grows is never freed,
   since a reader on another thread sharing the heap may still be testing a
   byte of the old one. */

void ensure_mark(uint64_t id)
{
    size_t h = (size_t)((id >> HEAP_SHIFT) & 0xff);
    size_t i = (size_t)(uint32_t)id - 1;
    size_t n = MARK_LEN[h];
    if (i < n)
    {
        return;
    }
    size_t cap = n * 2;
    if (cap < i + 1)
    {
        cap = i + 1;
    }
    if (cap < 1024)
    {
        cap = 1024;
    }
    uint8_t *v = (uint8_t *)cb_xcalloc(cap, 1);
    if (n > 0)
    {
        memcpy(v, __atomic_load_n(&cb_marks[h], __ATOMIC_RELAXED), n);
    }
    __atomic_store_n(&cb_marks[h], v, __ATOMIC_RELEASE);
    MARK_LEN[h] = cap;
}

/* ---- the arenas over every heap ---- */

bool objs_heap_empty(Rt *r, size_t h)
{
    if (h >= heaps_len(&r->objs.n))
    {
        return true;
    }
    ObjArena *a = &r->objs.h[h];
    for (size_t i = 0; i < a->n; i++)
    {
        if (a->slots[i].some)
        {
            return false;
        }
    }
    return true;
}

bool paths_heap_empty(Rt *r, size_t h)
{
    if (h >= heaps_len(&r->paths.n))
    {
        return true;
    }
    PathArena *a = &r->paths.h[h];
    for (size_t i = 0; i < a->n; i++)
    {
        if (a->slots[i].some)
        {
            return false;
        }
    }
    return true;
}

/* ---- HeapSlots: the reference slots of every heap ----

   A slot is registered in the heap of the thread that stores it (`cur`)
   and found in whichever heap holds it: the current one first, then any
   other with a page there. */

size_t slots_holder(Rt *r, uintptr_t a)
{
    size_t c = cur_heap();
    if (slotmap_get(&r->slots.h[c], a) != 0)
    {
        return c;
    }
    if (heaps_one(&r->slots.n))
    {
        return NO_IDX;
    }
    size_t n = heaps_len(&r->slots.n);
    for (size_t h = 0; h < n; h++)
    {
        if (h != c && holds_heap(h) && slotmap_get(&r->slots.h[h], a) != 0)
        {
            return h;
        }
    }
    return NO_IDX;
}

uint64_t slots_get(Rt *r, uintptr_t a)
{
    size_t h = sole_heap(heaps_one(&r->slots.n));
    if (h != NO_IDX)
    {
        return slotmap_get(&r->slots.h[h], a);
    }
    h = slots_holder(r, a);
    if (h == NO_IDX)
    {
        return 0;
    }
    return slotmap_get(&r->slots.h[h], a);
}

uint64_t slots_insert(Rt *r, uintptr_t a, uint64_t t)
{
    size_t h = sole_heap(heaps_one(&r->slots.n));
    if (h != NO_IDX)
    {
        return slotmap_insert(&r->slots.h[h], a, t);
    }
    size_t c = cur_heap();
    h = slots_holder(r, a);
    if (h != NO_IDX && h != c)
    {
        uint64_t old = slotmap_remove(&r->slots.h[h], a);
        slotmap_insert(&r->slots.h[c], a, t);
        return old;
    }
    return slotmap_insert(&r->slots.h[c], a, t);
}

uint64_t slots_remove(Rt *r, uintptr_t a)
{
    size_t h = sole_heap(heaps_one(&r->slots.n));
    if (h != NO_IDX)
    {
        return slotmap_remove(&r->slots.h[h], a);
    }
    h = slots_holder(r, a);
    if (h == NO_IDX)
    {
        return 0;
    }
    return slotmap_remove(&r->slots.h[h], a);
}

void slots_each_in(Rt *r, uintptr_t lo, uintptr_t hi, tok_fn f, void *ctx)
{
    size_t h = sole_heap(heaps_one(&r->slots.n));
    if (h != NO_IDX)
    {
        slotmap_each_in(&r->slots.h[h], lo, hi, f, ctx);
        return;
    }
    size_t n = heaps_len(&r->slots.n);
    for (h = 0; h < n; h++)
    {
        if (holds_heap(h))
        {
            slotmap_each_in(&r->slots.h[h], lo, hi, f, ctx);
        }
    }
}

bool slots_any_in(Rt *r, uintptr_t lo, uintptr_t hi)
{
    size_t h = sole_heap(heaps_one(&r->slots.n));
    if (h != NO_IDX)
    {
        return slotmap_any_in(&r->slots.h[h], lo, hi);
    }
    size_t n = heaps_len(&r->slots.n);
    for (h = 0; h < n; h++)
    {
        if (holds_heap(h) && slotmap_any_in(&r->slots.h[h], lo, hi))
        {
            return true;
        }
    }
    return false;
}

void slots_scan_into(Rt *r, uintptr_t lo, uintptr_t hi, bool take, atvec *out)
{
    size_t h = sole_heap(heaps_one(&r->slots.n));
    if (h != NO_IDX)
    {
        slotmap_scan_into(&r->slots.h[h], lo, hi, take, out);
        return;
    }
    size_t start = out->n;
    int parts = 0;
    size_t n = heaps_len(&r->slots.n);
    for (h = 0; h < n; h++)
    {
        if (!holds_heap(h))
        {
            continue;
        }
        size_t before = out->n;
        slotmap_scan_into(&r->slots.h[h], lo, hi, take, out);
        if (out->n > before)
        {
            parts++;
        }
    }
    if (parts > 1)
    {
        atvec_sort_from(out, start);
    }
}

atvec slots_range(Rt *r, uintptr_t lo, uintptr_t hi)
{
    atvec out = {0};
    slots_scan_into(r, lo, hi, false, &out);
    return out;
}

bool slots_heap_empty(Rt *r, size_t h)
{
    if (h >= heaps_len(&r->slots.n))
    {
        return true;
    }
    return slotmap_is_empty(&r->slots.h[h]);
}

bool slots_all_empty(Rt *r)
{
    size_t n = heaps_len(&r->slots.n);
    for (size_t h = 0; h < n; h++)
    {
        if (!slots_heap_empty(r, h))
        {
            return false;
        }
    }
    return true;
}

/* ---- page buckets ---- */

/* Whether no object lies over `[a, a + n)` by the global counts (a long
   run is not asked). */
bool quiet_pages(uintptr_t a, size_t n)
{
    uintptr_t first = a >> 12;
    uintptr_t last = (a + (n ? n : 1) - 1) >> 12;
    if (last - first >= 64)
    {
        return false;
    }
    for (uintptr_t p = first; p <= last; p++)
    {
        if (atomic_load_explicit(&cb_reclaimed_pages[p & (RECLAIMED_PAGE_BUCKETS - 1)], memory_order_relaxed) != 0)
        {
            return false;
        }
    }
    return true;
}

/* D-0202: each heap's own page buckets, counting both its reclaimed
   entries and its pages of reference slots; made on first use, never
   freed. */
static _Atomic(_Atomic uint32_t *) HEAP_PAGES[256];
/* How many extents each heap has registered in its buckets. */
static _Atomic uint64_t HEAP_TOTAL[MAX_HEAPS];
/* For each bucket, the heaps whose buckets count something there. */
static _Atomic uint64_t PAGE_HEAPS[RECLAIMED_PAGE_BUCKETS][4];

static _Atomic uint32_t *heap_pages(size_t h)
{
    _Atomic uint32_t *p = atomic_load_explicit(&HEAP_PAGES[h], memory_order_acquire);
    if (p)
    {
        return p;
    }
    _Atomic uint32_t *fresh = (_Atomic uint32_t *)cb_xcalloc(RECLAIMED_PAGE_BUCKETS, sizeof(uint32_t));
    _Atomic uint32_t *expect = NULL;
    if (atomic_compare_exchange_strong_explicit(&HEAP_PAGES[h], &expect, fresh, memory_order_acq_rel, memory_order_acquire))
    {
        return fresh;
    }
    free(fresh);
    return expect;
}

/* A page's bucket in a heap's table: hashed, not its low bits. Every
   thread's stack sits at the same offset in an equally sized region, so
   by low bits the stacks of all threads shared their buckets. */
static inline size_t heap_bucket(uintptr_t page)
{
    return (size_t)(((uint64_t)page * 0x9E3779B97F4A7C15ull) >> 52);
}

void heap_page_count_multi(size_t h, uintptr_t a, uint64_t size, bool up)
{
    if (up)
    {
        atomic_fetch_add_explicit(&HEAP_TOTAL[h], 1, memory_order_relaxed);
    }
    else
    {
        atomic_fetch_sub_explicit(&HEAP_TOTAL[h], 1, memory_order_relaxed);
    }
    _Atomic uint32_t *t = heap_pages(h);
    uintptr_t first = a >> 12;
    uintptr_t last = (a + (uintptr_t)(size ? size : 1) - 1) >> 12;
    size_t n = (size_t)(last - first + 1);
    if (n > RECLAIMED_PAGE_BUCKETS)
    {
        n = RECLAIMED_PAGE_BUCKETS;
    }
    size_t w = h >> 6;
    uint64_t bit = 1ull << (h & 63);
    for (uintptr_t p = first; p < first + n; p++)
    {
        size_t i = heap_bucket(p);
        /* A bucket's count changes only under its heap's lock, so its
           passing 0 to 1 and back is this heap's bit in PAGE_HEAPS. */
        if (up)
        {
            if (atomic_fetch_add_explicit(&t[i], 1, memory_order_relaxed) == 0)
            {
                atomic_fetch_or_explicit(&PAGE_HEAPS[i][w], bit, memory_order_release);
            }
        }
        else if (atomic_fetch_sub_explicit(&t[i], 1, memory_order_relaxed) == 1)
        {
            atomic_fetch_and_explicit(&PAGE_HEAPS[i][w], ~bit, memory_order_release);
        }
    }
}

static void rebuild_add(size_t h, _Atomic uint32_t *t, uintptr_t a, uint64_t size)
{
    atomic_fetch_add_explicit(&HEAP_TOTAL[h], 1, memory_order_relaxed);
    uintptr_t first = a >> 12;
    uintptr_t last = (a + (uintptr_t)(size ? size : 1) - 1) >> 12;
    size_t n = (size_t)(last - first + 1);
    if (n > RECLAIMED_PAGE_BUCKETS)
    {
        n = RECLAIMED_PAGE_BUCKETS;
    }
    for (uintptr_t p = first; p < first + n; p++)
    {
        size_t i = heap_bucket(p);
        atomic_fetch_add_explicit(&t[i], 1, memory_order_relaxed);
        atomic_fetch_or_explicit(&PAGE_HEAPS[i][h >> 6], 1ull << (h & 63), memory_order_relaxed);
    }
}

/* Every heap's page buckets, counted afresh from its tables (the order of
   the additions is not observable: they only count). */
void rebuild_heap_pages(Rt *r)
{
    for (size_t i = 0; i < RECLAIMED_PAGE_BUCKETS; i++)
    {
        for (int w = 0; w < 4; w++)
        {
            atomic_store_explicit(&PAGE_HEAPS[i][w], 0, memory_order_relaxed);
        }
    }
    size_t ns = heaps_len(&r->slots.n), nr = heaps_len(&r->reclaimed.n);
    size_t n = ns > nr ? ns : nr;
    for (size_t h = 0; h < n; h++)
    {
        _Atomic uint32_t *t = heap_pages(h);
        for (size_t i = 0; i < RECLAIMED_PAGE_BUCKETS; i++)
        {
            atomic_store_explicit(&t[i], 0, memory_order_relaxed);
        }
        atomic_store_explicit(&HEAP_TOTAL[h], 0, memory_order_relaxed);
        if (h < ns)
        {
            SlotMap *m = &r->slots.h[h];
            for (size_t i = 0; i < m->pages.cap; i++)
            {
                if (m->pages.used[i])
                {
                    rebuild_add(h, t, m->pages.e[i].k << SLOT_PAGE, 1);
                }
            }
            omap_it it = omap_lower_bound(&m->odd, 0);
            uint64_t k;
            while (omap_it_next(&it, &k, NULL))
            {
                rebuild_add(h, t, (uintptr_t)k, 1);
            }
        }
        if (h < nr)
        {
            Reclaimed *m = &r->reclaimed.h[h];
            for (size_t i = 0; i < m->ordered_sz.cap; i++)
            {
                if (m->ordered_sz.used[i])
                {
                    rebuild_add(h, t, m->ordered_sz.e[i].k, m->ordered_sz.e[i].v);
                }
            }
            for (uint32_t i = 0; i < m->ephemeral.nsmall; i++)
            {
                rebuild_add(h, t, m->ephemeral.small[i].a, m->ephemeral.small[i].size);
            }
            for (size_t i = 0; i < m->ephemeral.more_sz.cap; i++)
            {
                if (m->ephemeral.more_sz.used[i])
                {
                    rebuild_add(h, t, m->ephemeral.more_sz.e[i].k, m->ephemeral.more_sz.e[i].v);
                }
            }
        }
    }
}

/* The heaps other than this thread's whose buckets say they may hold a
   slot or a reclaimed entry on a page of [lo, hi), with this thread's. */
HeapSet addr_heaps(uintptr_t lo, uintptr_t hi)
{
    HeapSet set = HEAPSET_EMPTY;
    heapset_add(&set, cur_heap());
    uintptr_t first = lo >> 12;
    uintptr_t top = hi > lo + 1 ? hi : lo + 1;
    uintptr_t last = (top - 1) >> 12;
    size_t n = (size_t)(last - first + 1);
    if (n > RECLAIMED_PAGE_BUCKETS)
    {
        n = RECLAIMED_PAGE_BUCKETS;
    }
    size_t hn = atomic_load_explicit(&HEAPS_N, memory_order_acquire);
    /* A long run of pages: every heap with anything registered (more heaps
       locked, never too few). */
    if (n > 64)
    {
        for (size_t h = 0; h < hn; h++)
        {
            if (atomic_load_explicit(&HEAP_TOTAL[h], memory_order_relaxed) != 0)
            {
                heapset_add(&set, h);
            }
        }
        return set;
    }
    size_t words = (hn + 63) / 64;
    for (uintptr_t q = first; q < first + n; q++)
    {
        _Atomic uint64_t *b = PAGE_HEAPS[heap_bucket(q)];
        for (size_t w = 0; w < words; w++)
        {
            set.w[w] |= atomic_load_explicit(&b[w], memory_order_acquire);
        }
    }
    return set;
}

/* Adds +1 or -1 to the global bucket of every page [a, a + size) overlaps
   (each bucket once). */
void page_count(uintptr_t a, uint64_t size, bool up)
{
    uintptr_t first = a >> 12;
    uintptr_t last = (a + (uintptr_t)(size ? size : 1) - 1) >> 12;
    size_t n = (size_t)(last - first + 1);
    if (n > RECLAIMED_PAGE_BUCKETS)
    {
        n = RECLAIMED_PAGE_BUCKETS;
    }
    for (uintptr_t p = first; p < first + n; p++)
    {
        _Atomic uint32_t *b = &cb_reclaimed_pages[p & (RECLAIMED_PAGE_BUCKETS - 1)];
        if (up)
        {
            atomic_fetch_add_explicit(b, 1, memory_order_relaxed);
        }
        else
        {
            atomic_fetch_sub_explicit(b, 1, memory_order_relaxed);
        }
    }
}

/* ---- Reclaimed: raw address -> the reclaimed object there ----

   `[Release]` asks for every object in a range, so persistent objects are
   kept ordered (and again by hash, `ordered_ix`: every element or `Box`
   access looks its address up); ephemeral element objects are established
   and ended at every `Vec` element access, a few alive at once (`Eph`), and
   `range` sorts what it finds of them. Each entry's extent by address, one map per
   kind, feeds the page buckets. */

static void reclaimed_pc(uint8_t heap, uintptr_t a, uint64_t size, bool up)
{
    /* An entry on one page, counted by the only thread there is: the two
       counters change in place (nothing else writes them). */
    if (LIKELY(!multi()) && LIKELY(((a + (uintptr_t)(size ? size : 1) - 1) >> 12) == (a >> 12)))
    {
        _Atomic uint32_t *b = &cb_reclaimed_pages[(a >> 12) & (RECLAIMED_PAGE_BUCKETS - 1)];
        uint32_t d = up ? 1u : (uint32_t)-1;
        atomic_store_explicit(b, atomic_load_explicit(b, memory_order_relaxed) + d, memory_order_relaxed);
        size_t n = atomic_load_explicit(&cb_reclaimed_entries, memory_order_relaxed);
        atomic_store_explicit(&cb_reclaimed_entries, up ? n + 1 : n - 1, memory_order_relaxed);
        return;
    }
    page_count(a, size, up);
    if (multi())
    {
        heap_page_count(heap, a, size, up);
        return;
    }
    if (up)
    {
        atomic_fetch_add_explicit(&cb_reclaimed_entries, 1, memory_order_relaxed);
    }
    else
    {
        atomic_fetch_sub_explicit(&cb_reclaimed_entries, 1, memory_order_relaxed);
    }
}

static uint64_t take_size(map_u64 *m, uintptr_t a)
{
    uint64_t s = 0;
    map_u64_remove(m, a, &s);
    return s;
}

/* ---- the ephemeral objects (`Eph`) ---- */

static inline bool eph_get(const Eph *e, uintptr_t a, uint64_t *obj)
{
    for (uint32_t i = 0; i < e->nsmall; i++)
    {
        if (e->small[i].a == a)
        {
            *obj = e->small[i].obj;
            return true;
        }
    }
    return e->more.n != 0 && omap_get(&e->more, a, obj);
}

/* `insert`: true when an object was at `a` (its extent in *old_size). */
static inline bool eph_insert(Eph *e, uintptr_t a, uint64_t obj, uint64_t size, uint64_t *old_size)
{
    for (uint32_t i = 0; i < e->nsmall; i++)
    {
        if (e->small[i].a == a)
        {
            *old_size = e->small[i].size;
            e->small[i].obj = obj;
            e->small[i].size = size;
            return true;
        }
    }
    if (UNLIKELY(e->more.n != 0) && omap_get(&e->more, a, NULL))
    {
        omap_insert(&e->more, a, obj, NULL);
        *old_size = take_size(&e->more_sz, a);
        map_u64_insert(&e->more_sz, a, size, NULL);
        return true;
    }
    if (LIKELY(e->nsmall < EPH_SMALL))
    {
        if (UNLIKELY(!e->small))
        {
            e->small = (EphEnt *)cb_xmalloc(EPH_SMALL * sizeof(EphEnt));
        }
        EphEnt *x = &e->small[e->nsmall++];
        x->a = a;
        x->obj = obj;
        x->size = size;
        return false;
    }
    omap_insert(&e->more, a, obj, NULL);
    map_u64_insert(&e->more_sz, a, size, NULL);
    return false;
}

/* The object at `a` removed -- only if it is `obj`, when `only` is set:
   true when one was, its extent in *size. */
static inline bool eph_remove(Eph *e, uintptr_t a, bool only, uint64_t obj, uint64_t *size)
{
    /* The newest first: an access's object ends before the ones it nests in. */
    for (uint32_t i = e->nsmall; i-- > 0;)
    {
        if (e->small[i].a == a)
        {
            if (only && e->small[i].obj != obj)
            {
                return false;
            }
            *size = e->small[i].size;
            e->small[i] = e->small[--e->nsmall];
            return true;
        }
    }
    if (e->more.n == 0)
    {
        return false;
    }
    uint64_t v;
    if (!omap_get(&e->more, a, &v) || (only && v != obj))
    {
        return false;
    }
    omap_remove(&e->more, a, NULL);
    *size = take_size(&e->more_sz, a);
    return true;
}

static inline bool eph_any_in(const Eph *e, uintptr_t lo, uintptr_t hi)
{
    for (uint32_t i = 0; i < e->nsmall; i++)
    {
        if (e->small[i].a >= lo && e->small[i].a < hi)
        {
            return true;
        }
    }
    if (e->more.n == 0)
    {
        return false;
    }
    omap_it it = omap_lower_bound(&e->more, lo);
    uint64_t k;
    return omap_it_next(&it, &k, NULL) && k < hi;
}

/* Every (address, object) in [lo, hi), appended in no order. */
static inline void eph_range_into(const Eph *e, uintptr_t lo, uintptr_t hi, atvec *out)
{
    for (uint32_t i = 0; i < e->nsmall; i++)
    {
        if (e->small[i].a >= lo && e->small[i].a < hi)
        {
            atvec_push(out, e->small[i].a, e->small[i].obj);
        }
    }
    if (e->more.n)
    {
        uint64_t k, v;
        omap_it it = omap_lower_bound(&e->more, lo);
        while (omap_it_next(&it, &k, &v) && k < hi)
        {
            atvec_push(out, (uintptr_t)k, v);
        }
    }
}

bool reclaimed_get1(Reclaimed *m, uintptr_t a, uint64_t *obj)
{
    if (eph_get(&m->ephemeral, a, obj))
    {
        return true;
    }
    uint64_t *v = map_u64_get(&m->ordered_ix, a);
    if (v)
    {
        *obj = *v;
        return true;
    }
    return false;
}

void reclaimed_insert1(Reclaimed *m, uintptr_t a, uint64_t obj, uint64_t size)
{
    full_reclaimed_on();
    uint64_t was;
    if (eph_remove(&m->ephemeral, a, false, 0, &was))
    {
        reclaimed_pc(m->heap, a, was, false);
    }
    if (omap_insert(&m->ordered, a, obj, NULL))
    {
        reclaimed_pc(m->heap, a, take_size(&m->ordered_sz, a), false);
    }
    map_u64_insert(&m->ordered_ix, a, obj, NULL);
    map_u64_insert(&m->ordered_sz, a, size, NULL);
    reclaimed_pc(m->heap, a, size, true);
}

void reclaimed_insert_ephemeral1(Reclaimed *m, uintptr_t a, uint64_t obj, uint64_t size)
{
    full_reclaimed_on();
    uint64_t was;
    if (eph_insert(&m->ephemeral, a, obj, size, &was))
    {
        reclaimed_pc(m->heap, a, was, false);
    }
    reclaimed_pc(m->heap, a, size, true);
}

void reclaimed_remove1(Reclaimed *m, uintptr_t a)
{
    uint64_t was;
    if (eph_remove(&m->ephemeral, a, false, 0, &was))
    {
        reclaimed_pc(m->heap, a, was, false);
    }
    else if (omap_remove(&m->ordered, a, NULL))
    {
        map_u64_remove(&m->ordered_ix, a, NULL);
        reclaimed_pc(m->heap, a, take_size(&m->ordered_sz, a), false);
    }
}

void reclaimed_remove_if1(Reclaimed *m, uintptr_t a, uint64_t obj, bool ephemeral)
{
    if (ephemeral)
    {
        uint64_t was;
        if (eph_remove(&m->ephemeral, a, true, obj, &was))
        {
            reclaimed_pc(m->heap, a, was, false);
        }
        return;
    }
    uint64_t *v = map_u64_get(&m->ordered_ix, a);
    if (v && *v == obj)
    {
        /* The ordered entry itself: `get` would answer with an ephemeral
           entry at the same address and leave this one behind (CHG-0349). */
        omap_remove(&m->ordered, a, NULL);
        map_u64_remove(&m->ordered_ix, a, NULL);
        reclaimed_pc(m->heap, a, take_size(&m->ordered_sz, a), false);
    }
}

static bool omap_first_below(const omap *m, uintptr_t lo, uintptr_t hi)
{
    if (m->n == 0)
    {
        return false;
    }
    omap_it it = omap_lower_bound(m, lo);
    uint64_t k;
    return omap_it_next(&it, &k, NULL) && k < hi;
}

bool reclaimed_any_in1(const Reclaimed *m, uintptr_t lo, uintptr_t hi)
{
    return lo < hi && (omap_first_below(&m->ordered, lo, hi) || eph_any_in(&m->ephemeral, lo, hi));
}

/* Every (address, object) in [lo, hi), in address order, appended. */
void reclaimed_range1(const Reclaimed *m, uintptr_t lo, uintptr_t hi, atvec *out)
{
    size_t start = out->n;
    if (lo >= hi)
    {
        return;
    }
    uint64_t k, v;
    if (m->ordered.n)
    {
        omap_it it = omap_lower_bound(&m->ordered, lo);
        while (omap_it_next(&it, &k, &v) && k < hi)
        {
            atvec_push(out, (uintptr_t)k, v);
        }
    }
    size_t before = out->n;
    eph_range_into(&m->ephemeral, lo, hi, out);
    if (out->n > before)
    {
        atvec_sort_from(out, start);
    }
}

/* ---- HeapReclaimed: the reclaimed objects of every heap ---- */

static size_t reclaimed_holder(Rt *r, uintptr_t a)
{
    size_t c = cur_heap();
    uint64_t o;
    if (reclaimed_get1(&r->reclaimed.h[c], a, &o))
    {
        return c;
    }
    size_t n = heaps_len(&r->reclaimed.n);
    for (size_t h = 0; h < n; h++)
    {
        if (h != c && holds_heap(h) && reclaimed_get1(&r->reclaimed.h[h], a, &o))
        {
            return h;
        }
    }
    return NO_IDX;
}

bool reclaimed_get(Rt *r, uintptr_t a, uint64_t *obj)
{
    size_t h = sole_heap(heaps_one(&r->reclaimed.n));
    if (h != NO_IDX)
    {
        return reclaimed_get1(&r->reclaimed.h[h], a, obj);
    }
    h = reclaimed_holder(r, a);
    if (h == NO_IDX)
    {
        return false;
    }
    return reclaimed_get1(&r->reclaimed.h[h], a, obj);
}

void reclaimed_insert(Rt *r, uintptr_t a, uint64_t obj, uint64_t size)
{
    size_t h = sole_heap(heaps_one(&r->reclaimed.n));
    if (h != NO_IDX)
    {
        reclaimed_insert1(&r->reclaimed.h[h], a, obj, size);
        return;
    }
    size_t c = cur_heap();
    h = reclaimed_holder(r, a);
    if (h != NO_IDX && h != c)
    {
        reclaimed_remove1(&r->reclaimed.h[h], a);
    }
    reclaimed_insert1(&r->reclaimed.h[c], a, obj, size);
}

void reclaimed_insert_ephemeral(Rt *r, uintptr_t a, uint64_t obj, uint64_t size)
{
    reclaimed_insert_ephemeral1(&r->reclaimed.h[cur_heap()], a, obj, size);
}

void reclaimed_remove_if(Rt *r, uintptr_t a, uint64_t obj, bool ephemeral)
{
    size_t h = sole_heap(heaps_one(&r->reclaimed.n));
    if (h != NO_IDX)
    {
        reclaimed_remove_if1(&r->reclaimed.h[h], a, obj, ephemeral);
        return;
    }
    size_t n = heaps_len(&r->reclaimed.n);
    for (h = 0; h < n; h++)
    {
        if (holds_heap(h))
        {
            reclaimed_remove_if1(&r->reclaimed.h[h], a, obj, ephemeral);
        }
    }
}

bool reclaimed_any_in(Rt *r, uintptr_t lo, uintptr_t hi)
{
    size_t h = sole_heap(heaps_one(&r->reclaimed.n));
    if (h != NO_IDX)
    {
        return reclaimed_any_in1(&r->reclaimed.h[h], lo, hi);
    }
    size_t n = heaps_len(&r->reclaimed.n);
    for (h = 0; h < n; h++)
    {
        if (holds_heap(h) && reclaimed_any_in1(&r->reclaimed.h[h], lo, hi))
        {
            return true;
        }
    }
    return false;
}

atvec reclaimed_range(Rt *r, uintptr_t lo, uintptr_t hi)
{
    atvec out = {0};
    size_t h = sole_heap(heaps_one(&r->reclaimed.n));
    if (h != NO_IDX)
    {
        reclaimed_range1(&r->reclaimed.h[h], lo, hi, &out);
        return out;
    }
    size_t n = heaps_len(&r->reclaimed.n);
    for (h = 0; h < n; h++)
    {
        if (holds_heap(h))
        {
            reclaimed_range1(&r->reclaimed.h[h], lo, hi, &out);
        }
    }
    atvec_sort_from(&out, 0);
    return out;
}

bool reclaimed_heap_empty(Rt *r, size_t h)
{
    if (h >= heaps_len(&r->reclaimed.n))
    {
        return true;
    }
    return reclaimed_entries1(&r->reclaimed.h[h]) == 0;
}
