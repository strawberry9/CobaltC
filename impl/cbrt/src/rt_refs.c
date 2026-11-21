/* Allocation, fn values, borrows and access checks, reference data and
   slots, the notes of objectless locals, values in flight and the
   dual-body tests. Part of lib.c's translation unit. */

#include "rt.h"

#include <stdio.h>

/* ---- small helpers ---- */

static inline bool rf_u64s_has(const uint64_t *p, size_t n, uint64_t x)
{
    for (size_t i = 0; i < n; i++)
    {
        if (p[i] == x)
        {
            return true;
        }
    }
    return false;
}

static inline const CbProj *rf_cbproj_or_empty(const CbProj *p, size_t n)
{
    return n == 0 ? NULL : p;
}

/* One step of `Norm::of`: a range followed by an index folds into one
   absolute index. */
static void rf_norm_add(Norm *m, Proj x)
{
    const Proj *last = NULL;
    if (m->spill.n)
    {
        last = &m->spill.p[m->spill.n - 1];
    }
    else if (m->len)
    {
        last = &m->buf[m->len - 1];
    }
    if (last && last->k == P_RANGE && x.k == P_INDEX)
    {
        norm_set_last(m, proj_index(last->a + x.a));
    }
    else
    {
        norm_push(m, x);
    }
}

static void rf_norm_init(Norm *m)
{
    m->len = 0;
    m->spill.p = NULL;
    m->spill.n = 0;
    m->spill.cap = 0;
}

static inline u64vecvec *rf_pool_cur(Rt *r)
{
    return &r->pool[cur_heap()];
}

static inline projvecvec *rf_proj_pool_cur(Rt *r)
{
    return &r->proj_pool[cur_heap()];
}

static void rf_write_hex(char *buf, size_t n, uint64_t v)
{
    /* Rust's `{:#x}`: "0x" and lower-case digits, "0x0" for zero. */
    char t[17];
    int k = 0;
    do
    {
        t[k++] = "0123456789abcdef"[v & 15];
        v >>= 4;
    } while (v);
    size_t j = 0;
    if (n < 3 + (size_t)k)
    {
        if (n)
        {
            buf[0] = 0;
        }
        return;
    }
    buf[j++] = '0';
    buf[j++] = 'x';
    while (k)
    {
        buf[j++] = t[--k];
    }
    buf[j] = 0;
}

static void rf_dec(char *buf, uint64_t v)
{
    char t[21];
    int k = 0;
    do
    {
        t[k++] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    size_t j = 0;
    while (k)
    {
        buf[j++] = t[--k];
    }
    buf[j] = 0;
}

/* ---- allocation ----

   `allocate`'s blocks: plain `malloc` for an alignment `malloc` gives
   (rf_MALLOC_ALIGN), so that `cbrt.h`'s inline `cb_allocate_at_once` and
   `cb_reallocate_plain_fast` (`malloc`/`realloc` themselves) agree with
   this runtime; an over-aligned block is recorded by address, so that
   `deallocate` hands it back as it was made. `deallocate` of anything
   `allocate` did not return is a false trusted claim (`[Deallocate]`,
   spec/21 §0; `[Trusted-Claim-False]`). */

#define rf_MALLOC_ALIGN (sizeof(void *) == 8 ? 16u : 8u)

typedef struct
{
    size_t size;
    size_t align;
} rf_alloc_rec;

HMAP(rf_allocmap, rf_alloc_rec)

static plat_mutex rf_ALLOCS_LOCK = PLAT_MUTEX_INIT;
static rf_allocmap rf_ALLOCS;
_Atomic size_t OVER_ALIGNED;

/* `cbrt.h`'s `cb_allocate_at_once` calls `malloc` itself for a block
   `malloc` aligns unless this is set: while the replay trace records each
   allocation, every one comes here. */
uint8_t cb_alloc_traced;

/* Rust's `usize::next_power_of_two` in a release build (0 past the top). */
static inline size_t rf_next_pow2(size_t x)
{
    if (x <= 1)
    {
        return 1;
    }
    if (x > ((size_t)1 << (sizeof(size_t) * 8 - 1)))
    {
        return 0;
    }
    return (size_t)1 << (sizeof(size_t) * 8 - (size_t)__builtin_clzll((unsigned long long)(x - 1)));
}

void *cb_allocate(uint64_t size, uint64_t align)
{
    replay_out(me(), "raw storage");
    /* No runtime state but rf_ALLOCS, which has its own lock: `malloc` is
       thread-safe, so no hold on the runtime lock (D-0192). */
    if (size == 0)
    {
        return (void *)1;
    }
    size_t al = rf_next_pow2((size_t)(align ? align : 1));
    if (al <= rf_MALLOC_ALIGN)
    {
        return block_alloc((size_t)size);
    }
    /* `Layout::from_size_align`: the size rounded up to the alignment must
       not pass isize::MAX. */
    if ((size_t)size > (size_t)PTRDIFF_MAX - (al - 1))
    {
        return NULL;
    }
    void *p = plat_aligned_alloc((size_t)size, al);
    if (p)
    {
        plat_mutex_lock(&rf_ALLOCS_LOCK);
        rf_alloc_rec rec = {(size_t)size, al};
        rf_allocmap_insert(&rf_ALLOCS, (uint64_t)(uintptr_t)p, rec, NULL);
        plat_mutex_unlock(&rf_ALLOCS_LOCK);
        atomic_fetch_add_explicit(&OVER_ALIGNED, 1, memory_order_relaxed);
        /* From here on a free asks the map first, out of line. */
        alloc_gate(AL_GATE_ALIGNED);
    }
    return p;
}

void cb_deallocate(void *addr, uint64_t size, uint64_t align)
{
    (void)align;
    HELD_SCOPE e = enter_cells((uintptr_t)addr, size);
    release_cells(addr, size);
    free_block((uint8_t *)addr, size);
}

/* The block at `addr` handed back to the allocator (its cells released). */
void free_block(uint8_t *addr, uint64_t size)
{
    if (size == 0)
    {
        return;
    }
    if (atomic_load_explicit(&OVER_ALIGNED, memory_order_relaxed) > 0)
    {
        plat_mutex_lock(&rf_ALLOCS_LOCK);
        rf_alloc_rec rec;
        if (rf_allocmap_remove(&rf_ALLOCS, (uint64_t)(uintptr_t)addr, &rec))
        {
            atomic_fetch_sub_explicit(&OVER_ALIGNED, 1, memory_order_relaxed);
            plat_mutex_unlock(&rf_ALLOCS_LOCK);
            plat_aligned_free(addr);
            return;
        }
        plat_mutex_unlock(&rf_ALLOCS_LOCK);
    }
    block_free(addr, (size_t)size);
}

/* The same for the generated C (`cbrt.h`'s `cb_block_free`). */
void cb_free_block(void *addr, uint64_t size)
{
    free_block((uint8_t *)addr, size);
}

/* `allocate(new)`, `copy_raw` of the old contents and `deallocate(old)`,
   as `Vec`'s growth makes them, in one step for a buffer of plain
   elements: `realloc`, then the old cells released. NULL if the
   allocation fails, the old block then untouched. An over-aligned buffer
   takes the three steps. */
uint8_t *cb_reallocate_plain(uint8_t *addr, uint64_t old, uint64_t new_size, uint64_t align)
{
    if (old == 0)
    {
        return (uint8_t *)cb_allocate(new_size, align);
    }
    /* D-0202: one entry for the old cells (the helpers below enter none: a
       second reckoning of their heaps could differ from this one). */
    HELD_SCOPE e = enter_cells((uintptr_t)addr, old);
    if (new_size == 0)
    {
        release_cells(addr, old);
        free_block(addr, old);
        return (uint8_t *)1;
    }
    if (rf_next_pow2((size_t)(align ? align : 1)) > rf_MALLOC_ALIGN)
    {
        uint8_t *p = (uint8_t *)cb_allocate(new_size, align);
        if (!p)
        {
            return p;
        }
        memcpy(p, addr, (size_t)(old < new_size ? old : new_size));
        release_cells(addr, old);
        free_block(addr, old);
        return p;
    }
    /* The old block's address, kept as a number: after `realloc` it names
       cells, not storage the runtime reads. */
    uintptr_t was = (uintptr_t)addr;
    __asm__("" : "+r"(was));
    uint8_t *p = (uint8_t *)block_realloc(addr, (size_t)old, (size_t)new_size);
    if (p)
    {
        /* The old cells' objects end, whether the block moved or grew in
           place. */
        release_plain_cells((void *)was, old);
    }
    return p;
}

/* `cb_deallocate` for cells that never held a reference: no slot to
   forget, so only an object over the cells, if any could be there, ends
   first. */
void cb_deallocate_plain(uint8_t *addr, uint64_t size)
{
    if (size == 0)
    {
        return;
    }
    if (!quiet_cells((uintptr_t)addr, (size_t)size) || atomic_load_explicit(&OVER_ALIGNED, memory_order_relaxed) != 0)
    {
        cb_deallocate(addr, size, 1);
        return;
    }
    block_free(addr, (size_t)size);
}

void cb_copy_raw(void *dst, const void *src, uint64_t n)
{
    ENTERED_SCOPE e = enter();
    if (n == 0)
    {
        return;
    }
    memmove(dst, src, (size_t)n);
    /* The references the source cells hold get a holder in the copy: none
       where no reference was ever stored (FULL). */
    if (HOOK(FULL.copy_slots))
    {
        HOOK(FULL.copy_slots)((uintptr_t)dst, (uintptr_t)src, n);
    }
}

static void copy_slots_full(uintptr_t d, uintptr_t s, uint64_t n)
{
    Rt *r = rt();
    atvec moved = slots_range(r, s, s + (uintptr_t)n);
    for (size_t i = 0; i < moved.n; i++)
    {
        cb_store_ref((void *)(d + (moved.p[i].a - s)), moved.p[i].t);
    }
    atvec_free(&moved);
}

/* ---- fn values ---- */

void *cb_fn_item(void *code)
{
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    uint64_t *h = map_u64_get(&r->fn_items, (uint64_t)(uintptr_t)code);
    if (h)
    {
        return (void *)(uintptr_t)*h;
    }
    CbFnBox *b = (CbFnBox *)cb_xmalloc(sizeof(CbFnBox));
    b->code = code;
    b->data = NULL;
    b->root = 0;
    b->ty = NOTYPE;
    b->closure = 0;
    map_u64_insert(&r->fn_items, (uint64_t)(uintptr_t)code, (uint64_t)(uintptr_t)b, NULL);
    return b;
}

/* `cb_fn_item` for a function whose reference parameters are all pure
   direct: the handle is the same, and the code is recorded as one that
   keeps nothing of its arguments (`cb_fn_pure`). */
void *cb_fn_item_pure(void *code)
{
    void *h = cb_fn_item(code);
    ENTERED_SCOPE e = enter();
    hset_add(&rt()->pure_items, (uint64_t)(uintptr_t)code);
    return h;
}

/* Whether the `fn` value `h` is such a fn item (D-0191): a `Vec` call
   handed it may pass it bare element addresses, as a direct call would. */
uint8_t cb_fn_pure(void *h)
{
    if (!h)
    {
        return 0;
    }
    READ_SCOPE e = enter_read();
    const CbFnBox *b = (const CbFnBox *)h;
    return (uint8_t)hset_has(&rt()->pure_items, (uint64_t)(uintptr_t)b->code);
}

/* D-0192: a closure's code whose reference parameters are all direct and
   never handed on: recorded as `cb_fn_item_pure` records a function. */
void cb_fn_mark_pure(void *code)
{
    ENTERED_SCOPE e = enter();
    hset_add(&rt()->pure_items, (uint64_t)(uintptr_t)code);
}

/* A closure's storage (16-aligned, at least one byte): `malloc`'s, which
   is 16-aligned on x86_64 (glibc, MinGW), and freed with `free`
   (`drop_fn_handle`). */
void *box_storage(uint64_t size)
{
    return malloc((size_t)(size ? size : 1));
}

/* The closure object moves into a heap box the handle owns. */
void *cb_fn_box(uint64_t obj, void *code, uint32_t ty)
{
    replay_out(me(), "a closure");
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    uint64_t size = rt_typ(r, ty)->size;
    uint8_t *data = (uint8_t *)box_storage(size);
    if (!data)
    {
        cbrt_oom((size_t)size);
    }
    uintptr_t old = objs_at(r, obj)->addr;
    if (size > 0)
    {
        memcpy(data, (const void *)old, (size_t)size);
    }
    cb_move_to(obj, data);
    r = rt();
    detach(r, obj);
    uint64_t root = objs_at(r, obj)->root;
    if (root == 0)
    {
        root = make_root(r, obj);
        objs_at(r, obj)->root = root;
    }
    CbFnBox *b = (CbFnBox *)cb_xmalloc(sizeof(CbFnBox));
    b->code = code;
    b->data = data;
    b->root = root;
    b->ty = ty;
    b->closure = 1;
    map_u64_insert(&r->fn_boxes, (uint64_t)(uintptr_t)b, obj, NULL);
    return b;
}

/* `[Read]` of a fn value: an item handle copies; a closure box is cloned
   (its own object, its own reference data) -- unless the closure owns a
   resource, which cannot be read in value position. */
void *cb_fn_copy(void *handle, uint32_t file, uint32_t line)
{
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    if (!map_u64_get(&r->fn_boxes, (uint64_t)(uintptr_t)handle))
    {
        return handle;
    }
    const CbFnBox *b = (const CbFnBox *)handle;
    uint32_t ty = b->ty;
    if (rt_typ(r, ty)->is_resource)
    {
        fault("diag.read-of-resource", file, line);
    }
    uint64_t size = rt_typ(r, ty)->size;
    uint8_t *data = (uint8_t *)box_storage(size);
    if (!data)
    {
        cbrt_oom((size_t)size);
    }
    if (size > 0)
    {
        memcpy(data, b->data, (size_t)size);
    }
    uint64_t id = objs_reserve(r);
    ensure_mark(id);
    Obj *o = objs_insert(r, id);
    memset(o, 0, sizeof *o);
    o->addr = (uintptr_t)data;
    o->ty = ty;
    o->alive = true;
    o->init = true;
    o->frame = NO_IDX;
    o->scope = NO_IDX;
    uint64_t root = make_root(r, id);
    objs_at(r, id)->root = root;
    cb_copy_datum(data, b->data, ty);
    /* The `fn` values among the captures are copied in turn: each copy owns
       its own closures (a shared box would be dropped by whichever copy ends
       first, leaving the other's slot empty). */
    u64vec offs = {0};
    if (size > 0)
    {
        fn_offsets(rt(), (uintptr_t)data, ty, 0, &offs);
    }
    for (uint32_t i = 0; i < offs.n; i++)
    {
        void **slot = (void **)(data + offs.p[i]);
        if (*slot)
        {
            *slot = cb_fn_copy(*slot, file, line);
        }
    }
    u64vec_free(&offs);
    CbFnBox *nb = (CbFnBox *)cb_xmalloc(sizeof(CbFnBox));
    nb->code = b->code;
    nb->data = data;
    nb->root = root;
    nb->ty = ty;
    nb->closure = b->closure;
    map_u64_insert(&rt()->fn_boxes, (uint64_t)(uintptr_t)nb, id, NULL);
    return nb;
}

/* ---- aliasing ---- */

bool excluded(Rt *r, const uint64_t *excludes, size_t n, uint64_t tok)
{
    if (rf_u64s_has(excludes, n, tok))
    {
        return true;
    }
    for (size_t i = 0; i < n; i++)
    {
        PathRec *p = paths_get(r, excludes[i]);
        if (p && u64vec_contains(&p->ancestors, tok))
        {
            return true;
        }
    }
    return false;
}

bool solitary(Rt *r, uint64_t of, const uint64_t *excludes, size_t n)
{
    Obj *o = objs_get(r, of);
    if (!o)
    {
        return true;
    }
    for (uint32_t i = 0; i < o->toks.v.n; i++)
    {
        uint64_t t = o->toks.v.p[i];
        PathRec *p = paths_at(r, t);
        if (p->valid && !p->is_root && p->occ > 0 && !excluded(r, excludes, n, t))
        {
            return false;
        }
    }
    return true;
}

static uint64_t borrow_lone(uint64_t base, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line);
CB_ALWAYS void rf_access_affine(const PathView *rec, uint32_t file, uint32_t line);
static inline void rf_access_clash(Rt *r, uint64_t tok, const PathView *rec, const Obj *o, const CbProj *extra, size_t n, bool exclusive, const uint64_t *point, size_t npoint, bool is_point, bool *clash, uint32_t file,
                                   uint32_t line);
static void rf_access_clash_with(Rt *r, uint64_t tok, const PathView *rec, const Obj *o, const CbProj *extra, size_t n, bool exclusive, const uint64_t *point, size_t npoint, bool is_point, bool *clash, uint32_t file,
                                 uint32_t line);

static __attribute__((noinline)) uint64_t rf_borrow_general(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);

uint64_t cb_borrow(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line)
{
    if (base == 0)
    {
        return 0;
    }
    /* D-0292: in a decided function's frame a borrow from the owner's own
       binding, or from a light reference, is light. */
    if ((base & LIGHT) != 0 && !frame_rec())
    {
        return light_borrow(base, p, n, mode, file, line);
    }
    if (light_ref(base))
    {
        check_light("cb_borrow", base, file, line);
    }
    if (LIKELY(lone()))
    {
        return borrow_lone(base, rf_cbproj_or_empty(p, n), n, mode == 1, file, line);
    }
    return rf_borrow_general(base, p, n, mode, file, line);
}

/* `cb_borrow` with threads or under the trace (apart, so that the entry
   itself keeps no frame for what this holds). */
static __attribute__((noinline)) uint64_t rf_borrow_general(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line)
{
    uint64_t named = 0;
    if (replay_on())
    {
        u64vec st = replay_steps(rf_cbproj_or_empty(p, n), n);
        named = replay_borrow(me(), base, st.p, st.n, mode == 1);
        u64vec_free(&st);
    }
    /* D-0202: local when the base's object, and every path it was formed
       through (whose ancestors the new path's list closes over), are in this
       thread's heap. */
    HELD_SCOPE h = enter_on(heap_of(base));
    if (h == H_LOCAL && !ancestors_local(base))
    {
        held_drop(h);
        h = held_excl();
    }
    uint64_t t = borrow(rt(), base, rf_cbproj_or_empty(p, n), n, mode == 1, file, line);
    replay_minted(named, t);
    return t;
}

/* `&m e[lo .. hi]` (D-0070): a borrow of the elements `lo .. hi` of what
   `base ++ extra` reaches, or, when that is itself a slice's range, of its
   elements `lo .. hi` (the range made absolute). Checked as an access to
   that range. D-0292: `cont`/`cty`, the place and type of the container
   `base ++ p` reaches: a light base carries no projection, and the slice's
   path is reconstructed from them (`proj_of_addr`). */
uint64_t cb_borrow_range(uint64_t base, const void *cont, uint32_t cty, const CbProj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line)
{
    return borrow_range_in(base, cont, cty, p, n, lo, hi, mode, file, line, true);
}

/* `cb_borrow_range`'s checks with no path minted: a slice formed for a
   parameter that never reads its path (`cobc`'s pure direct slices). */
void cb_borrow_range_unminted(uint64_t base, const void *cont, uint32_t cty, const CbProj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line)
{
    borrow_range_in(base, cont, cty, p, n, lo, hi, mode, file, line, false);
}

/* Closes `anc` (base and base's ancestors already pushed) over each of
   those tokens' own ancestors (`record_token_ancestors`). */
static void rf_close_ancestors(Rt *r, u64vec *anc, uint64_t base, const uint64_t *banc, size_t nbanc)
{
    for (size_t k = 0; k <= nbanc; k++)
    {
        uint64_t e = k == 0 ? base : banc[k - 1];
        PathRec *pe = paths_get(r, e);
        if (pe)
        {
            for (uint32_t i = 0; i < pe->ancestors.n; i++)
            {
                uint64_t a = pe->ancestors.p[i];
                if (!u64vec_contains(anc, a))
                {
                    u64vec_push(anc, a);
                }
            }
        }
    }
}

uint64_t borrow_range_in(uint64_t base, const void *cont, uint32_t cty, const CbProj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line, bool minted)
{
    replay_out(me(), "a slice");
    /* No path (0): a range of a reference whose checks were proved (an
       objectless `String` lent to a `std` builder, D-0196) has none either. */
    if (base == 0)
    {
        return 0;
    }
    /* D-0204: as `cb_borrow`: local when the base's heap, and every path it
       was formed through, are this thread's to hold. */
    HELD_SCOPE h = enter_on(heap_of(base));
    if (h == H_LOCAL && !ancestors_local(base))
    {
        held_drop(h);
        h = held_excl();
    }
    const CbProj *extra = rf_cbproj_or_empty(p, n);
    Rt *r = rt();
    PathView rec;
    if (!view(r, base, &rec))
    {
        fault("diag.stale-binding", file, line);
    }
    /* The new path's projection and ancestors come from the runtime's pools,
       and the check compares through a stack buffer (`Norm`,
       `ranged_overlap_with`), as `borrow_in`'s do. D-0292: through a light
       reference the container's projection is read from its place; a light
       root's is the explicit one. */
    bool light = light_ref(base);
    projvec path;
    if (light)
    {
        if (!cont)
        {
            check_light("cb_borrow_range", base, file, line);
            path = take_proj(rf_proj_pool_cur(rt()));
        }
        else
        {
            path = proj_of_addr(rec.of, (uintptr_t)cont, cty);
        }
    }
    else
    {
        path = take_proj(rf_proj_pool_cur(rt()));
        projvec_extend(&path, rec.proj, rec.nproj);
        for (size_t i = 0; i < n; i++)
        {
            projvec_push(&path, conv(extra[i]));
        }
    }
    Proj range;
    if (path.n && path.p[path.n - 1].k == P_RANGE)
    {
        uint64_t pl = path.p[path.n - 1].a;
        path.n--;
        range = proj_range(pl + lo, pl + hi);
    }
    else
    {
        range = proj_range(lo, hi);
    }
    projvec_push(&path, range);
    /* The access check over the range: `base` itself (and its ancestors)
       excluded, as for any borrow through it. */
    r = rt();
    PathView brec;
    if (!view(r, base, &brec))
    {
        cbrt_internal("a path stale between two views");
    }
    uint64_t rec_of = brec.of;
    bool rec_root = brec.is_root;
    bool exclusive = mode == 1;
    /* A shared borrow can clash only with an exclusive path. */
    Obj *o = objs_get(r, rec_of);
    if (!o || !o->alive)
    {
        fault("diag.stale-binding", file, line);
    }
    const u64vec *toks = exclusive ? &o->toks.v : &o->xtoks.v;
    Norm mine;
    norm_of(&mine, path.p, path.n);
    size_t mlen;
    const Proj *mp = norm_get(&mine, &mlen);
    for (uint32_t i = 0; i < toks->n; i++)
    {
        uint64_t t = toks->p[i];
        PathRec *q = paths_at(r, t);
        if (!q->valid || q->is_root || q->occ == 0)
        {
            continue;
        }
        if (!rec_root && (t == base || rf_u64s_has(brec.ancestors, brec.nancestors, t)))
        {
            continue;
        }
        if (!ranged_overlap_with(mp, mlen, q->proj.p, q->proj.n))
        {
            continue;
        }
        if (q->has_lock_of != brec.has_lock_of && !(brec.has_lock_of ? q->exclusive : exclusive))
        {
            continue;
        }
        if (exclusive || q->exclusive)
        {
            fault("diag.aliasing-conflict", file, line);
        }
    }
    norm_free(&mine);
    if (!minted)
    {
        give_proj(rf_proj_pool_cur(rt()), path);
        return 0;
    }
    u64vec anc2 = {0};
    if (!(rec_root || light))
    {
        anc2 = take_vec(rf_pool_cur(rt()));
    }
    r = rt();
    if (!view(r, base, &brec))
    {
        cbrt_internal("a path stale between two views");
    }
    if (!rec_root && !light)
    {
        u64vec_push(&anc2, base);
        u64vec_extend(&anc2, brec.ancestors, brec.nancestors);
        rf_close_ancestors(r, &anc2, base, brec.ancestors, brec.nancestors);
    }
    projvec lock_of = {0};
    if (brec.has_lock_of)
    {
        projvec_extend(&lock_of, brec.lock_of, brec.nlock_of);
    }
    return mint_locked(rt(), rec_of, path, exclusive, false, anc2, brec.has_lock_of ? &lock_of : NULL);
}

uint64_t borrow(Rt *r, uint64_t base, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line)
{
    return borrow_in(r, base, extra, n, exclusive, file, line, true);
}

/* D-0292: `[Borrow]` in a frame that forms light references. From the
   owner's own binding (a light root): valid, and, where holders exist on
   the object, the new place checked against them. From a light reference:
   its object alive, and nothing else (DESIGN.md §2.1). The result is
   `LIGHT | obj`: no record, no stamp, nothing to retire. */
uint64_t light_borrow(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line)
{
    uint64_t obj = base & ~TOK_FLAGS;
    HELD_SCOPE e = enter_on(heap_of(base));
    Rt *r = rt();
    if ((base & ROOT) != 0)
    {
        Obj *o = objs_get(r, obj);
        if (!o || o->root != base)
        {
            fault("diag.stale-binding", file, line);
        }
        bool marked = o->toks.v.n != 0;
        if (marked)
        {
            ph3_count(&N_SLOW);
            check_access(r, base, rf_cbproj_or_empty(p, n), n, mode == 1, file, line);
        }
    }
    else
    {
        Obj *o = objs_get(r, obj);
        if (!o || !o->alive)
        {
            fault("diag.stale-binding", file, line);
        }
    }
    ph3_count(&N_LIGHT);
    return obj | LIGHT;
}

/* `stamp` false: the path is recorded in no statement scope; its taker
   ends it (`cb_call_done`) or its last holder does (`drop_occ`). */
CB_ALWAYS uint64_t borrow_core(bool L, Rt *r, uint64_t base, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line, bool stamp)
{
    /* The access check (`check_access`), whose view of the base serves the
       new path too: nothing below changes the base's record. */
    PathView rec;
    if (!view(r, base, &rec))
    {
        fault("diag.stale-binding", file, line);
    }
    rf_access_affine(&rec, file, line);
    Obj *o = objs_get(r, rec.of);
    if (!o || !o->alive)
    {
        fault("diag.stale-binding", file, line);
    }
    bool clash;
    rf_access_clash(r, base, &rec, o, extra, n, exclusive, NULL, 0, false, &clash, file, line);
    uint64_t of = rec.of;
    /* The two lists from the runtime's pools (`give_proj`, `give_vec`). */
    projvec proj = take_proj(&r->proj_pool[HEAP_(L)]);
    bool light = light_ref(base);
    u64vec anc = {0};
    /* D-0292: a light base reached a recording frame unrecorded: its place
       is unknown, so the new path is the whole object (never fewer clashes). */
    if (!light)
    {
        projvec_extend(&proj, rec.proj, rec.nproj);
        for (size_t i = 0; i < n; i++)
        {
            projvec_push(&proj, conv(extra[i]));
        }
    }
    /* `ancestors`: the chain this borrow was formed through, closed over
       those tokens' own ancestors. */
    if (!rec.is_root && !light)
    {
        anc = take_vec_inl(&r->pool[HEAP_(L)]);
        u64vec_push(&anc, base);
        u64vec_extend(&anc, rec.ancestors, rec.nancestors);
        rf_close_ancestors(r, &anc, base, rec.ancestors, rec.nancestors);
    }
    projvec lock_of = {0};
    if (rec.has_lock_of)
    {
        projvec_extend(&lock_of, rec.lock_of, rec.nlock_of);
    }
    if (L)
    {
        return mint_core(true, r, of, proj, exclusive, false, anc, rec.has_lock_of ? &lock_of : NULL, stamp);
    }
    return mint_in(r, of, proj, exclusive, false, anc, rec.has_lock_of ? &lock_of : NULL, stamp);
}

uint64_t borrow_in(Rt *r, uint64_t base, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line, bool stamp)
{
    return borrow_core(false, r, base, extra, n, exclusive, file, line, stamp);
}

/* `borrow` on the lone thread. */
static __attribute__((noinline)) uint64_t borrow_lone(uint64_t base, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line)
{
    return borrow_core(true, rt(), base, extra, n, exclusive, file, line, true);
}

/* A call through a closure's box: the exclusive borrow of its object that
   is the call's `self`, ended by `cb_call_done` once the call returns --
   unheld, no check could count it (D-0018). */
uint64_t cb_call_self(uint64_t root)
{
    replay_out(me(), "a closure");
    /* D-0292: light in a decided function's frame (`cb_call_done` of a
       light token ends nothing). */
    if ((root & LIGHT) != 0 && !frame_rec())
    {
        return light_borrow(root, NULL, 0, 1, NOLOC, 0);
    }
    ENTERED_SCOPE e = enter();
    return borrow_in(rt(), root, NULL, 0, true, NOLOC, 0, false);
}

/* A borrow recorded in no statement scope, for a callee that keeps
   nothing of it (`cobc`'s inline `Vec::push` handing `grow` its path);
   ended by `cb_call_done`. */
uint64_t cb_borrow_unstamped(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line)
{
    replay_out(me(), "a borrow in no statement");
    /* No path (0): a reference whose checks the compiler proved (D-0196)
       lends none on. */
    if (base == 0)
    {
        return 0;
    }
    if ((base & LIGHT) != 0 && !frame_rec())
    {
        return light_borrow(base, p, n, mode, file, line);
    }
    ENTERED_SCOPE e = enter();
    return borrow_in(rt(), base, rf_cbproj_or_empty(p, n), n, mode == 1, file, line, false);
}

void cb_call_done(uint64_t tok)
{
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    PathRec *p = paths_get(r, tok);
    if (p && p->occ == 0)
    {
        retire_token(r, tok);
    }
}

Proj conv(CbProj q)
{
    switch (q.kind)
    {
    case 0:
        return proj_field(q.idx);
    case 1:
        return proj_index(q.idx);
    default:
        return proj_payload();
    }
}

/* Element `i` of the projection `base ++ extra`, without building it. */
static inline Proj rf_proj_at(const Proj *base, size_t nbase, const CbProj *extra, size_t i)
{
    if (i < nbase)
    {
        return base[i];
    }
    return conv(extra[i - nbase]);
}

/* `check_access` through an object's root path with no projection: the
   root excludes no path, every path's projection extends the empty one,
   and a root carries no lock origin. */
void check_root_access(Rt *r, uint64_t of, bool exclusive, uint32_t file, uint32_t line)
{
    Obj *o = objs_get(r, of);
    if (!o || !o->alive)
    {
        fault("diag.stale-binding", file, line);
    }
    const u64vec *toks = exclusive ? &o->toks.v : &o->xtoks.v;
    for (uint32_t i = 0; i < toks->n; i++)
    {
        PathRec *p = paths_at(r, toks->p[i]);
        if (!p->valid || p->is_root || p->occ == 0)
        {
            continue;
        }
        if (!exclusive && p->has_lock_of)
        {
            continue;
        }
        if (exclusive || p->exclusive)
        {
            fault("diag.aliasing-conflict", file, line);
        }
    }
}

/* `[Read]`/`[Write]`'s path checks (stale binding, `clash`) for an access
   through `tok` narrowed by `extra`, as `path_target` + `clash` compute
   them, without allocating: this runs at every access. Returns the
   accessed object. */
uint64_t check_access(Rt *r, uint64_t tok, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line)
{
    bool clash;
    return check_access_with(r, tok, extra, n, exclusive, NULL, 0, false, &clash, file, line);
}

/* D-0283: a call's point check (`cobc`'s `emit_point_checks`): an access
   through argument path `tok` the callee's summary declares, checked at
   the call, before the callee holds its arguments. The call's other
   argument paths (`args`) are held by then, so they count even with no
   holder yet. */
void cb_point_check(const uint64_t *args, size_t nargs, uint64_t tok, const CbProj *p, size_t n, uint32_t exclusive, const uint8_t *msg, uint64_t msg_len, uint32_t file, uint32_t line)
{
    replay_out(me(), "a point check at a call");
    if (tok == 0)
    {
        return;
    }
    /* D-0292: a point check is made in a recording frame, whose references
       are all recorded; a light one here is a lowering hole (whole-object). */
    if (light_ref(tok))
    {
        check_light("cb_point_check", tok, file, line);
    }
    bool clash;
    {
        Held e = enter_on(heap_of(tok));
        check_access_with(rt(), tok, rf_cbproj_or_empty(p, n), n, exclusive != 0, nargs ? args : NULL, nargs, true, &clash, file, line);
        held_drop(e);
    }
    if (clash)
    {
        cb_fault_msg("diag.aliasing-conflict", msg, msg_len, file, line);
    }
}

/* D-0213: an exclusive path is its own thread's. */
CB_ALWAYS void rf_access_affine(const PathView *rec, uint32_t file, uint32_t line)
{
    if (UNLIKELY(rec->exclusive && !rec->is_root && atomic_load_explicit(&cb_live_threads, memory_order_relaxed) != 1))
    {
        if (rec->rec)
        {
            affine(rec->rec, file, line);
        }
    }
}

/* `clash` for an access through `tok` (its view `rec`, its live object `o`)
   narrowed by `extra`: the object's other paths that overlap it. A shared
   access can clash only with an exclusive path. With a point check
   (`is_point`: the call's argument paths), a clash is reported, not faulted
   on. */
static inline void rf_access_clash(Rt *r, uint64_t tok, const PathView *rec, const Obj *o, const CbProj *extra, size_t n, bool exclusive, const uint64_t *point, size_t npoint, bool is_point, bool *clash, uint32_t file,
                                   uint32_t line)
{
    *clash = false;
    /* No path that could clash: nothing to compare. */
    if ((exclusive ? o->toks.v.n : o->xtoks.v.n) != 0)
    {
        rf_access_clash_with(r, tok, rec, o, extra, n, exclusive, point, npoint, is_point, clash, file, line);
    }
}

static __attribute__((noinline)) void rf_access_clash_with(Rt *r, uint64_t tok, const PathView *rec, const Obj *o, const CbProj *extra, size_t n, bool exclusive, const uint64_t *point, size_t npoint, bool is_point, bool *clash,
                                                           uint32_t file, uint32_t line)
{
    const u64vec *toks = exclusive ? &o->toks.v : &o->xtoks.v;
    uint32_t ntoks = toks->n;
    size_t np = rec->nproj + n;
    /* Whether the accessing path has a range: found at the first path it
       is compared with. */
    int ranged = -1;
    /* The accessing path normalized, once per access, when some path
       involved has a range (built on first need). */
    Norm mine;
    bool have_mine = false;
    for (uint32_t k = 0; k < ntoks; k++)
    {
        uint64_t t = toks->p[k];
        /* `excluded`: the accessing path itself, before its record is
           read (the one path an exclusive reference's object often has). */
        if (t == tok && !rec->is_root)
        {
            continue;
        }
        PathRec *p = paths_at(r, t);
        if (!p->valid || p->is_root || (p->occ == 0 && !rf_u64s_has(point, npoint, t)))
        {
            continue;
        }
        if (is_point && (empty_range(rec->proj, rec->nproj) || empty_range(p->proj.p, p->proj.n)))
        {
            continue;
        }
        /* `excluded`: the accessing path's ancestors (a closed list: an
           ancestor's ancestors are in it). */
        if (!rec->is_root && rf_u64s_has(rec->ancestors, rec->nancestors, t))
        {
            continue;
        }
        if (ranged < 0)
        {
            ranged = has_range(rec->proj, rec->nproj);
        }
        if (ranged || has_range(p->proj.p, p->proj.n))
        {
            if (!have_mine)
            {
                rf_norm_init(&mine);
                for (size_t i = 0; i < np; i++)
                {
                    rf_norm_add(&mine, rf_proj_at(rec->proj, rec->nproj, extra, i));
                }
                have_mine = true;
            }
            size_t mlen;
            const Proj *mp = norm_get(&mine, &mlen);
            if (!ranged_overlap_with(mp, mlen, p->proj.p, p->proj.n))
            {
                continue;
            }
        }
        else
        {
            size_t m = np < p->proj.n ? np : p->proj.n;
            bool differ = false;
            for (size_t i = 0; i < m; i++)
            {
                if (!proj_eq(rf_proj_at(rec->proj, rec->nproj, extra, i), p->proj.p[i]))
                {
                    differ = true;
                    break;
                }
            }
            if (differ)
            {
                continue;
            }
        }
        /* `sync-exempt` (spec/19 §2, CHG-0072), both directions: a
           lock-derived path and a shared path that is not. */
        if (rec->has_lock_of != p->has_lock_of && !(rec->has_lock_of ? p->exclusive : exclusive))
        {
            continue;
        }
        if (exclusive || p->exclusive)
        {
            if (is_point)
            {
                if (have_mine)
                {
                    norm_free(&mine);
                }
                *clash = true;
                return;
            }
            fault("diag.aliasing-conflict", file, line);
        }
    }
    if (have_mine)
    {
        norm_free(&mine);
    }
}

uint64_t check_access_with(Rt *r, uint64_t tok, const CbProj *extra, size_t n, bool exclusive, const uint64_t *point, size_t npoint, bool is_point, bool *clash, uint32_t file, uint32_t line)
{
    PathView rec;
    if (!view(r, tok, &rec))
    {
        fault("diag.stale-binding", file, line);
    }
    rf_access_affine(&rec, file, line);
    Obj *o = objs_get(r, rec.of);
    if (!o || !o->alive)
    {
        fault("diag.stale-binding", file, line);
    }
    rf_access_clash(r, tok, &rec, o, extra, n, exclusive, point, npoint, is_point, clash, file, line);
    return rec.of;
}

/* D-0205 / CHG-0237: whether a frozen parameter's body may run (`cobc`'s
   `NAME__frz`): reads through `tok` cannot clash with anything now, and
   nothing the call does can change that. `tok` is valid, over a live,
   initialized object, not formed under a lock, and no valid exclusive path
   to that object overlaps it (held or not -- conservative); for a vector
   parameter (`n > 0`), no object lies over its buffer. 0, a path the
   caller already vouched for: yes. D-0209: an object over raw storage
   passes only while this thread is alone. */
uint8_t cb_frozen_ok(uint64_t tok, const void *lo, size_t n)
{
    /* D-0292: a light reference is vouched for as token 0 is. */
    if (tok == 0 || (light_ref(tok) && !frame_rec()))
    {
        return 1;
    }
    if (n > 0 && cb_live_in((void *)lo, (uint64_t)n) != 0)
    {
        return 0;
    }
    HELD_SCOPE e = enter_on(heap_of(tok));
    Rt *r = rt();
    PathView rec;
    if (!view(r, tok, &rec) || rec.has_lock_of)
    {
        return 0;
    }
    Obj *o = objs_get(r, rec.of);
    if (!o || !o->alive || !o->init)
    {
        return 0;
    }
    if (o->reclaimed && atomic_load_explicit(&cb_live_threads, memory_order_acquire) != 1)
    {
        return 0;
    }
    /* The normalised path is built only when an exclusive path is there to
       compare it with. */
    Norm mine;
    bool have_mine = false;
    uint8_t ok = 1;
    for (uint32_t k = 0; k < o->xtoks.v.n; k++)
    {
        uint64_t t = o->xtoks.v.p[k];
        if (t == tok || rf_u64s_has(rec.ancestors, rec.nancestors, t))
        {
            continue;
        }
        PathRec *q = paths_get(r, t);
        if (!q || !q->valid || q->is_root)
        {
            continue;
        }
        bool overlap;
        if (has_range(rec.proj, rec.nproj) || has_range(q->proj.p, q->proj.n))
        {
            if (!have_mine)
            {
                norm_of(&mine, rec.proj, rec.nproj);
                have_mine = true;
            }
            size_t mlen;
            const Proj *mp = norm_get(&mine, &mlen);
            overlap = ranged_overlap_with(mp, mlen, q->proj.p, q->proj.n);
        }
        else
        {
            size_t m = rec.nproj < q->proj.n ? rec.nproj : q->proj.n;
            overlap = true;
            for (size_t i = 0; i < m; i++)
            {
                if (!proj_eq(rec.proj[i], q->proj.p[i]))
                {
                    overlap = false;
                    break;
                }
            }
        }
        if (overlap)
        {
            ok = 0;
            break;
        }
    }
    if (have_mine)
    {
        norm_free(&mine);
    }
    return ok;
}

/* An access with nothing to decide: this thread alone and no trace kept, a
   recorded path that is valid, over a live object none of whose other
   paths that could clash (all of them for a write, the exclusive ones for
   a read) is held. true: the object, and no clash; false: the general
   check decides, from the start. It reads the tables and changes nothing. */
/* `rf_access_quick`'s look at the object's other paths: true when none of
   them is held (invalid, a root, or with no occurrence). */
static __attribute__((noinline)) bool rf_access_quick_rest(Rt *r, uint64_t base, const PathRec *rec, const u64vec *toks)
{
    for (uint32_t k = 0; k < toks->n; k++)
    {
        uint64_t t = toks->p[k];
        if (t == base)
        {
            continue;
        }
        PathRec *q = paths_get(r, t);
        if (UNLIKELY(!q))
        {
            return false;
        }
        if (q->valid && !q->is_root && q->occ != 0)
        {
            /* A held path the accessing one was formed through clashes
               with nothing of it (`excluded`, as the general check has
               it); any other held path is for the general check. */
            if (!rec->is_root && rf_u64s_has(rec->ancestors.p, rec->ancestors.n, t))
            {
                continue;
            }
            return false;
        }
    }
    return true;
}

/* `*prec`: the path's record once the quick path has the live object in
   hand (so that what follows need not look either up again), else NULL. */
CB_ALWAYS bool rf_access_quick(uint64_t base, bool exclusive, Obj **out, PathRec **prec)
{
    *prec = NULL;
    if (UNLIKELY(!lone()))
    {
        return false;
    }
    Rt *r = rt();
    PathRec *rec = paths_get_inl(r, base);
    if (UNLIKELY(!rec || !rec->valid))
    {
        return false;
    }
    Obj *o = objs_get_inl(r, rec->of);
    if (UNLIKELY(!o || !o->alive))
    {
        return false;
    }
    *out = o;
    *prec = rec;
    /* No path that could clash, or the accessing path alone (an element's
       object and its one path): decided here. */
    const u64vec *toks = exclusive ? &o->toks.v : &o->xtoks.v;
    uint32_t n = toks->n;
    return LIKELY(n == 0 || (n == 1 && toks->p[0] == base));
}

/* The object's other paths that could clash, looked at: true when none
   is held but those the accessing path was formed through. One or two are
   looked at here (a reference formed from an exclusive one, read through
   while that one is held, is the commonest access after the plain one). */
CB_ALWAYS bool rf_access_others(Rt *r, uint64_t base, const PathRec *rec, const u64vec *toks)
{
    uint32_t n = toks->n;
    if (n > 2)
    {
        return rf_access_quick_rest(r, base, rec, toks);
    }
    for (uint32_t k = 0; k < n; k++)
    {
        uint64_t t = toks->p[k];
        if (t == base)
        {
            continue;
        }
        PathRec *q = paths_get_inl(r, t);
        if (UNLIKELY(!q))
        {
            return false;
        }
        if (q->valid && !q->is_root && q->occ != 0)
        {
            if (rec->is_root || !rf_u64s_has(rec->ancestors.p, rec->ancestors.n, t))
            {
                return false;
            }
        }
    }
    return true;
}

static __attribute__((noinline)) void rf_read_general(uint64_t base, const CbProj *p, size_t n, uint32_t file, uint32_t line);

/* `cb_read` past its plain case. `rec`: the path's record when the quick
   path had the live object `o` in hand (its other paths are looked at,
   then the general check), else NULL (the general check from the start).
   `loc`: the file, and the line above it. */
static __attribute__((noinline)) void rf_read_scan(uint64_t base, const CbProj *p, size_t n, uint64_t loc, const PathRec *rec, const Obj *o)
{
    if (rec != NULL && rf_access_others(rt(), base, rec, &o->xtoks.v) && o->init)
    {
        return;
    }
    /* No path (0): a place whose checks were proven (D-0189). */
    if (base == 0)
    {
        return;
    }
    rf_read_general(base, p, n, (uint32_t)loc, (uint32_t)(loc >> 32));
}

void cb_read(uint64_t base, const CbProj *p, size_t n, uint32_t file, uint32_t line)
{
    /* Nothing is called here but in tail position: no frame. No path
       (0) has no record, and is `rf_read_scan`'s to return on. */
    Obj *o = NULL;
    PathRec *rec;
    if (LIKELY(rf_access_quick(base, false, &o, &rec)) && LIKELY(o->init))
    {
        return;
    }
    rf_read_scan(base, p, n, (uint64_t)file | ((uint64_t)line << 32), rec, o);
}

static void rf_read_general(uint64_t base, const CbProj *p, size_t n, uint32_t file, uint32_t line)
{
    /* D-0292: nothing through a light reference: its conflicts were decided
       at its formation (in a recording frame, the whole object stands in). */
    if (light_ref(base))
    {
        if (!frame_rec())
        {
            return;
        }
        check_light("cb_read", base, file, line);
    }
    HELD_SCOPE e = enter_on(heap_of(base));
    Rt *r = rt();
    const CbProj *extra = rf_cbproj_or_empty(p, n);
    if (replay_on())
    {
        u64vec st = replay_steps(extra, n);
        replay_read(me(), base, st.p, st.n);
        u64vec_free(&st);
    }
    /* Staleness first, then initialization, then `clash` -- the order
       `path_target` and the checks after it always had. The path and its
       object are looked up once. */
    PathView rec;
    Obj *o = NULL;
    if (view(r, base, &rec))
    {
        o = objs_get(r, rec.of);
    }
    if (UNLIKELY(!o || !o->alive))
    {
        fault("diag.stale-binding", file, line);
    }
    if (UNLIKELY(!o->init))
    {
        fault("diag.use-of-uninitialized", file, line);
    }
    rf_access_affine(&rec, file, line);
    bool clash;
    rf_access_clash(r, base, &rec, o, extra, n, false, NULL, 0, false, &clash, file, line);
}

/* D-0049: whether the value of type `ty` at `addr` owns a resource now
   (`live-resource-at`, spec/05): a `None` of an `Option<Box<T>>` does not.
   A type declared `resource` or with a destructor (`owner`), a handle and
   a guard always do. */
bool owns(Rt *r, uintptr_t addr, uint32_t ty)
{
    TypeInfo *info = rt_typ(r, ty);
    if (!info->is_resource)
    {
        return false;
    }
    if (info->owner || info->drop)
    {
        return true;
    }
    switch (info->kind)
    {
    case K_STRUCT:
        for (uint32_t i = 0; i < info->nfields; i++)
        {
            if (owns(r, addr + (uintptr_t)info->fields[i].off, info->fields[i].ty))
            {
                return true;
            }
        }
        return false;
    case K_ENUM:
    {
        size_t tag = enum_tag(info, addr);
        if (tag < info->nvariants)
        {
            uint32_t pty = info->variants[tag];
            if (pty != NOTYPE)
            {
                return owns(r, addr + (uintptr_t)info->payload_off, pty);
            }
            return false;
        }
        return true;
    }
    case K_ARRAY:
    {
        uint64_t esize = rt_typ(r, info->elem)->size;
        for (uint64_t i = 0; i < info->count; i++)
        {
            if (owns(r, addr + (uintptr_t)(i * esize), info->elem))
            {
                return true;
            }
        }
        return false;
    }
    default:
        return true;
    }
}

uint32_t cb_owns(const void *addr, uint32_t ty)
{
    ENTERED_SCOPE e = enter();
    return (uint32_t)owns(rt(), (uintptr_t)addr, ty);
}

/* D-0194: `[Write]` to a place of a quiet type: `cb_write`'s check, then,
   where the target holds a value (its object initialized) that owns
   something, that value is destroyed in place (`[Write-Quiet-Replace]`)
   instead of faulting; the caller stores the new value after. A place with
   no path (0) always holds its value. */
void cb_write_quiet(uint64_t base, const CbProj *p, size_t n, void *addr, uint32_t ty, uint32_t file, uint32_t line)
{
    replay_out(me(), "a quiet place written");
    ENTERED_SCOPE e = enter();
    bool init;
    if (base == 0 || (light_ref(base) && !frame_rec()))
    {
        init = true;
    }
    else
    {
        Rt *r = rt();
        uint64_t of = check_access(r, base, rf_cbproj_or_empty(p, n), n, true, file, line);
        Obj *o = objs_at(r, of);
        bool was = o->init;
        o->init = true;
        init = was;
    }
    if (init && owns(rt(), (uintptr_t)addr, ty))
    {
        drop_in_place((uintptr_t)addr, ty, 0);
    }
}

static __attribute__((noinline)) void rf_write_general(uint64_t base, const CbProj *p, size_t n, uint32_t target_resource, uint32_t file, uint32_t line);

/* `cb_write` past its plain case, as `rf_read_scan`; `n_res`: the count
   of projections, and `target_resource` above it. */
static __attribute__((noinline)) void rf_write_scan(uint64_t base, const CbProj *p, uint64_t n_res, uint64_t loc, const PathRec *rec, Obj *o)
{
    uint32_t target_resource = (uint32_t)(n_res >> 32);
    if (rec != NULL && rf_access_others(rt(), base, rec, &o->toks.v) && (target_resource == 0 || !o->init))
    {
        o->init = true;
        return;
    }
    /* No path (0), as for `cb_read` (D-0189). */
    if (base == 0)
    {
        return;
    }
    rf_write_general(base, p, (size_t)(uint32_t)n_res, target_resource, (uint32_t)loc, (uint32_t)(loc >> 32));
}

void cb_write(uint64_t base, const CbProj *p, size_t n, uint32_t target_resource, uint32_t file, uint32_t line)
{
    Obj *o = NULL;
    PathRec *rec;
    if (LIKELY(rf_access_quick(base, true, &o, &rec)) && LIKELY(target_resource == 0 || !o->init))
    {
        o->init = true;
        return;
    }
    /* A projection list longer than 32 bits can count is not one: the
       general check gets what it was given. */
    if (UNLIKELY(n > UINT32_MAX))
    {
        if (base != 0)
        {
            rf_write_general(base, p, n, target_resource, file, line);
        }
        return;
    }
    rf_write_scan(base, p, (uint64_t)n | ((uint64_t)target_resource << 32), (uint64_t)file | ((uint64_t)line << 32), rec, o);
}

static void rf_write_general(uint64_t base, const CbProj *p, size_t n, uint32_t target_resource, uint32_t file, uint32_t line)
{
    /* D-0292: a light reference: the place holds its value, so a resource
       target that owns something is overwritten live. */
    if (light_ref(base))
    {
        if (!frame_rec())
        {
            if (target_resource != 0)
            {
                fault("diag.overwrite-of-live-resource", file, line);
            }
            return;
        }
        check_light("cb_write", base, file, line);
    }
    const CbProj *extra = rf_cbproj_or_empty(p, n);
    /* D-0192, D-0202: all of it within the base's heap: local when that is
       this thread's. */
    HELD_SCOPE e = enter_on(heap_of(base));
    Rt *r = rt();
    if (replay_on())
    {
        u64vec st = replay_steps(extra, n);
        replay_write(me(), base, st.p, st.n);
        u64vec_free(&st);
    }
    PathView rec;
    if (UNLIKELY(!view(r, base, &rec)))
    {
        fault("diag.stale-binding", file, line);
    }
    rf_access_affine(&rec, file, line);
    Obj *o = objs_get(r, rec.of);
    if (UNLIKELY(!o || !o->alive))
    {
        fault("diag.stale-binding", file, line);
    }
    bool clash;
    rf_access_clash(r, base, &rec, o, extra, n, true, NULL, 0, false, &clash, file, line);
    if (target_resource != 0 && o->init)
    {
        fault("diag.overwrite-of-live-resource", file, line);
    }
    o->init = true;
}

/* ---- reference data ---- */

typedef struct
{
    uintptr_t slot;
    uint64_t tok;
} rf_store_ctx;

static void rf_store_plan(Plan *p, Rt *r, void *ctx)
{
    rf_store_ctx *c = (rf_store_ctx *)ctx;
    plan_store(p, r, c->slot, c->tok);
}

/* D-0292: `[Write]` of a reference value whose place `cobc` knows: a light
   reference stored into data becomes a holder -- a path on its object with
   the projection of its place (`proj_of_addr`), held by the slot alone
   (occurrence 1, no statement scope). A recorded reference is stored as
   `cb_store_ref` stores it. `mode`: `CB_EXCLUSIVE` for an exclusive
   reference. */
void cb_hold_ref(void *slot, uint64_t tok, const void *addr, uint32_t ty, uint32_t mode, uint32_t file, uint32_t line)
{
    if ((tok & LIGHT) == 0)
    {
        cb_store_ref(slot, tok);
        return;
    }
    replay_out(me(), "a light reference stored into data");
    uint64_t obj = tok & ~TOK_FLAGS;
    rf_store_ctx c = {(uintptr_t)slot, tok};
    HELD_SCOPE e = enter_planned(rf_store_plan, &c);
    Rt *r = rt();
    Obj *o = objs_get(r, obj);
    if (!o || !o->alive)
    {
        fault("diag.stale-binding", file, line);
    }
    projvec proj = proj_of_addr(obj, (uintptr_t)addr, ty);
    u64vec none = {0};
    uint64_t holder = mint_in(rt(), obj, proj, (mode & 1) == 1, false, none, NULL, false);
    ph3_count(&N_HOLDERS);
    r = rt();
    paths_at(r, holder)->occ = 1;
    uint64_t old = slots_insert(r, (uintptr_t)slot, holder);
    if (old)
    {
        drop_occ(r, old);
    }
}

/* D-0292: a reference arriving in a recording frame (a parameter, a call's
   result): recorded, so that the frame's checks see it. A light reference
   gets a path with the projection of its place; `kind` 1 holds it in the
   frame (a parameter, for the call), 0 stamps it into the statement (a
   result). A recorded reference, or none, is returned as it is. */
uint64_t cb_rec_ref(uint64_t tok, const void *addr, uint32_t ty, uint32_t mode, uint32_t kind, uint32_t file, uint32_t line)
{
    if ((tok & LIGHT) == 0)
    {
        /* D-0299 step 4: a parameter's reference is bound to the parameter
           (`[Call]`), which holds it for the call. */
        if (kind != 0 && tok != 0)
        {
            replay_param(me(), tok);
        }
        return tok;
    }
    uint64_t obj = tok & ~TOK_FLAGS;
    HELD_SCOPE e = enter_on(heap_of(tok));
    Rt *r = rt();
    Obj *o = objs_get(r, obj);
    if (!o || !o->alive)
    {
        fault("diag.stale-binding", file, line);
    }
    projvec proj = proj_of_addr(obj, (uintptr_t)addr, ty);
    u64vec none = {0};
    uint64_t t = mint_in(rt(), obj, proj, (mode & 1) == 1, false, none, NULL, kind == 0);
    ph3_count(&N_ARRIVALS);
    if (kind != 0)
    {
        r = rt();
        paths_at(r, t)->occ++;
        TState *s = ts();
        for (size_t i = s->stack.n; i-- > 0;)
        {
            if (s->stack.p[i].kind == E_FRAME)
            {
                owned_push(r, &s->stack.p[i].u.owned, t | HOLD);
                break;
            }
        }
    }
    return t;
}

/* D-0292: the projection of the place of type `ty` at `addr` inside the
   object `obj`, read from the owner's type: fields by offset, elements by
   size, an enum's payload by the tag in memory. A place the walk does not
   reach gives the deepest place found, which is at least as wide; under
   `COBALTC_RECORD=check` it is reported. */
projvec proj_of_addr(uint64_t obj, uintptr_t addr, uint32_t ty)
{
    ph3_count(&N_PROJ_ADDR);
    projvec proj = take_proj(rf_proj_pool_cur(rt()));
    Rt *r = rt();
    Obj *o = objs_get(r, obj);
    if (!o)
    {
        return proj;
    }
    uintptr_t cur = o->addr;
    uint32_t cty = o->ty;
    TypeInfo *wt = rt_typ_opt(r, ty);
    bool want_zero = wt && wt->size == 0;
    for (;;)
    {
        if (cty == ty && cur == addr)
        {
            return proj;
        }
        TypeInfo *info = rt_typ(r, cty);
        bool found = false;
        Proj step = proj_payload();
        uintptr_t na = 0;
        uint32_t nt = 0;
        switch (info->kind)
        {
        case K_STRUCT:
            for (uint32_t i = 0; i < info->nfields; i++)
            {
                uint64_t off = info->fields[i].off;
                uint32_t fty = info->fields[i].ty;
                uintptr_t size = (uintptr_t)rt_typ(r, fty)->size;
                uintptr_t lo = cur + (uintptr_t)off;
                bool hit = want_zero ? (size == 0 && addr == lo) : (size > 0 && addr >= lo && addr < lo + size);
                if (hit)
                {
                    found = true;
                    step = proj_field(i);
                    na = lo;
                    nt = fty;
                    break;
                }
            }
            break;
        case K_ARRAY:
        {
            uintptr_t es = (uintptr_t)rt_typ(r, info->elem)->size;
            if (es != 0 && addr >= cur)
            {
                uintptr_t i = (addr - cur) / es;
                if ((uint64_t)i < info->count)
                {
                    found = true;
                    step = proj_index(i);
                    na = cur + i * es;
                    nt = info->elem;
                }
            }
            break;
        }
        case K_ENUM:
        {
            size_t tag = enum_tag(info, cur);
            if (tag < info->nvariants)
            {
                uint32_t pty = info->variants[tag];
                if (pty != NOTYPE && addr >= cur + (uintptr_t)info->payload_off)
                {
                    found = true;
                    step = proj_payload();
                    na = cur + (uintptr_t)info->payload_off;
                    nt = pty;
                }
            }
            break;
        }
        default:
            break;
        }
        if (!found)
        {
            break;
        }
        projvec_push(&proj, step);
        cur = na;
        cty = nt;
    }
    if (atomic_load_explicit(&RECORD_CHECK, memory_order_relaxed) && !(cur == addr && cty == ty))
    {
        Obj *ob = objs_at(r, obj);
        TypeInfo *want = rt_typ_opt(r, ty);
        char a1[32], a2[32];
        rf_write_hex(a1, sizeof a1, (uint64_t)addr);
        rf_write_hex(a2, sizeof a2, (uint64_t)ob->addr);
        char d[24];
        rf_dec(d, obj);
        fprintf(stderr, "cbrt check: no place of type %s at %s in object %s (%s at %s): the deepest place found is used\n", want ? want->name : "?", a1, d, rt_typ(r, ob->ty)->name, a2);
        fflush(stderr);
        plat_abort();
    }
    return proj;
}

static __attribute__((noinline)) void rf_store_ref_general(void *slot, uint64_t tok);

void cb_store_ref(void *slot, uint64_t tok)
{
    /* The lone thread, one heap, a recorded reference into a slot on the
       page used last (a local's): stored where it is. */
    if (LIKELY(lone()) && LIKELY(tok != 0) && LIKELY((tok & LIGHT) == 0))
    {
        Rt *r = rt();
        SlotMap *m = &r->slots.h[0];
        uintptr_t x = slotmap_last_at(m, (uintptr_t)slot);
        if (LIKELY(x < 512) && LIKELY(m->last_ptr != &SLOT_NONE) && LIKELY(heaps_one(&r->slots.n)) && LIKELY(HOOK(cb_slots_used)))
        {
            SlotPage *pg = m->last_ptr;
            uint64_t old = pg->s[x];
            if (old == tok)
            {
                return;
            }
            pg->s[x] = tok;
            if (LIKELY(old == 0))
            {
                pg->n++;
            }
            else
            {
                drop_occ(r, old);
            }
            PathRec *p = paths_get_inl(r, tok);
            if (LIKELY(p != NULL))
            {
                p->occ++;
            }
            return;
        }
    }
    rf_store_ref_general(slot, tok);
}

static __attribute__((noinline)) void rf_store_ref_general(void *slot, uint64_t tok)
{
    /* D-0292: a light reference is stored with its place (`cb_hold_ref`);
       one that reaches here has lost it, and would hold nothing. */
    if ((tok & LIGHT) != 0)
    {
        char h[32];
        rf_write_hex(h, sizeof h, tok);
        cbrt_internalf("a light reference (%s) stored without its place", h);
    }
    rf_store_ctx c = {(uintptr_t)slot, tok};
    HELD_SCOPE e = enter_planned(rf_store_plan, &c);
    Rt *r = rt();
    if (replay_on())
    {
        replay_store(me(), (uintptr_t)slot, slots_get(r, (uintptr_t)slot), tok);
    }
    /* Token 0 (a proven reference): the slot stays empty, and
       `cb_load_ref` gives 0 back. */
    if (tok == 0)
    {
        uint64_t old = slots_remove(r, (uintptr_t)slot);
        if (old)
        {
            drop_occ(r, old);
        }
        return;
    }
    uint64_t old = slots_insert(r, (uintptr_t)slot, tok);
    if (old)
    {
        if (old == tok)
        {
            return;
        }
        drop_occ(r, old);
    }
    PathRec *p = paths_get(r, tok);
    if (p)
    {
        p->occ++;
    }
}

/* `[Swap-Places]`: each reference names a place that is still there
   (temporally valid, its object alive); no conflict check. */
void cb_valid(uint64_t tok, uint32_t file, uint32_t line)
{
    /* No path (0): a place whose checks were proven (D-0200). */
    if (tok == 0)
    {
        return;
    }
    ENTERED_SCOPE e = enter();
    /* D-0292: a light reference: its object is still there. */
    if (light_ref(tok))
    {
        Obj *o = objs_get(rt(), tok & ~TOK_FLAGS);
        if (!o || !o->alive)
        {
            fault("diag.stale-binding", file, line);
        }
        return;
    }
    /* A recorded path: valid, over a live object (what `path_target`
       faults on; its lists are not wanted here). */
    Rt *r = rt();
    PathView v;
    if (!view(r, tok, &v))
    {
        fault("diag.stale-binding", file, line);
    }
    Obj *o = objs_get(r, v.of);
    if (!o || !o->alive)
    {
        fault("diag.stale-binding", file, line);
    }
}

typedef struct
{
    uintptr_t lo;
    uintptr_t hi;
} rf_cells_ctx;

static void rf_cells_plan(Plan *p, Rt *r, void *ctx)
{
    (void)r;
    rf_cells_ctx *c = (rf_cells_ctx *)ctx;
    plan_cells(p, c->lo, c->hi);
}

static __attribute__((noinline)) uint64_t rf_load_ref_general(const void *slot)
{
    rf_cells_ctx c = {(uintptr_t)slot, (uintptr_t)slot + 8};
    HELD_SCOPE e = enter_planned(rf_cells_plan, &c);
    return slots_get(rt(), (uintptr_t)slot);
}

uint64_t cb_load_ref(const void *slot)
{
    /* The lone thread and the page used last (a local's): the slot is
       read where it is. An empty slot is empty in every heap only when
       there is one heap. */
    if (LIKELY(lone()))
    {
        Rt *r = rt();
        SlotMap *m = &r->slots.h[0];
        uintptr_t x = slotmap_last_at(m, (uintptr_t)slot);
        if (LIKELY(x < 512))
        {
            uint64_t t = m->last_ptr->s[x];
            if (LIKELY(t != 0) || heaps_one(&r->slots.n))
            {
                return t;
            }
        }
    }
    return rf_load_ref_general(slot);
}

/* The offsets of every reference slot inside a value of type `ty` (enums
   by the active variant, read from the value itself). */
void ref_offsets(Rt *r, uintptr_t addr, uint32_t ty, uint64_t base, u64vec *out)
{
    TypeInfo *info = rt_typ(r, ty);
    if (!info->has_refs)
    {
        return;
    }
    if (info->has_fixed_refs)
    {
        u64vec_reserve(out, info->nfixed_refs);
        for (uint32_t i = 0; i < info->nfixed_refs; i++)
        {
            out->p[out->n++] = base + info->fixed_refs[i];
        }
        return;
    }
    switch (info->kind)
    {
    case K_REF:
    case K_GUARD:
        u64vec_push(out, base);
        break;
    case K_STRUCT:
        for (uint32_t i = 0; i < info->nfields; i++)
        {
            ref_offsets(r, addr + (uintptr_t)info->fields[i].off, info->fields[i].ty, base + info->fields[i].off, out);
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
                ref_offsets(r, addr + (uintptr_t)info->payload_off, pty, base + info->payload_off, out);
            }
        }
        break;
    }
    case K_ARRAY:
    {
        uint64_t esize = rt_typ(r, info->elem)->size;
        for (uint64_t i = 0; i < info->count; i++)
        {
            ref_offsets(r, addr + (uintptr_t)(i * esize), info->elem, base + i * esize, out);
        }
        break;
    }
    default:
        break;
    }
}

/* D-0089: the offsets of the `fn` slots in the value at `addr`. */
void fn_offsets(Rt *r, uintptr_t addr, uint32_t ty, uint64_t base, u64vec *out)
{
    TypeInfo *info = rt_typ(r, ty);
    if (!info->holds_fn)
    {
        return;
    }
    switch (info->kind)
    {
    case K_FN:
        u64vec_push(out, base);
        break;
    case K_STRUCT:
        for (uint32_t i = 0; i < info->nfields; i++)
        {
            fn_offsets(r, addr + (uintptr_t)info->fields[i].off, info->fields[i].ty, base + info->fields[i].off, out);
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
                fn_offsets(r, addr + (uintptr_t)info->payload_off, pty, base + info->payload_off, out);
            }
        }
        break;
    }
    case K_ARRAY:
    {
        uint64_t esize = rt_typ(r, info->elem)->size;
        for (uint64_t i = 0; i < info->count; i++)
        {
            fn_offsets(r, addr + (uintptr_t)(i * esize), info->elem, base + i * esize, out);
        }
        break;
    }
    default:
        break;
    }
}

/* D-0089: whether the closure a handle names owns a resource. */
bool fn_owns_resource(Rt *r, void *h)
{
    if (!h || !map_u64_get(&r->fn_boxes, (uint64_t)(uintptr_t)h))
    {
        return false;
    }
    const CbFnBox *b = (const CbFnBox *)h;
    if (rt_typ(r, b->ty)->is_resource)
    {
        return true;
    }
    /* A closure holding a `fn` value whose closure owns a resource owns it
       too (its capture's type, `fn(…)`, cannot say so). */
    u64vec offs = {0};
    fn_offsets(r, (uintptr_t)b->data, b->ty, 0, &offs);
    bool any = false;
    for (uint32_t i = 0; i < offs.n && !any; i++)
    {
        any = fn_owns_resource(r, *(void **)((uint8_t *)b->data + offs.p[i]));
    }
    u64vec_free(&offs);
    return any;
}

/* D-0089: `[Read]` of a `fn` value by value. A closure that owns a
   resource moves out, leaving the slot empty (a call through it is
   `diag.stale-binding`); any other is copied. */
void *cb_fn_read(void **slot, uint32_t file, uint32_t line)
{
    void *h = *slot;
    bool own;
    {
        Entered e = enter();
        own = fn_owns_resource(rt(), h);
        entered_drop(e);
    }
    if (own)
    {
        *slot = NULL;
        return h;
    }
    if (!h)
    {
        return h;
    }
    return cb_fn_copy(h, file, line);
}

/* D-0089: after `*dst = *src` (a value holding `fn` slots read by value):
   each slot's closure is copied into `dst`, or, when it owns a resource,
   moved there and emptied in `src`. */
void cb_fn_split(void *dst, void *src, uint32_t ty, uint32_t file, uint32_t line)
{
    u64vec offs = {0};
    bool *ownv;
    {
        Entered e = enter();
        Rt *r = rt();
        fn_offsets(r, (uintptr_t)src, ty, 0, &offs);
        ownv = (bool *)cb_xmalloc((size_t)offs.n * sizeof(bool) + 1);
        for (uint32_t i = 0; i < offs.n; i++)
        {
            ownv[i] = fn_owns_resource(r, *(void **)((uint8_t *)src + offs.p[i]));
        }
        entered_drop(e);
    }
    for (uint32_t i = 0; i < offs.n; i++)
    {
        void **s = (void **)((uint8_t *)src + offs.p[i]);
        void **d = (void **)((uint8_t *)dst + offs.p[i]);
        if (ownv[i])
        {
            *s = NULL;
        }
        else if (*s)
        {
            *d = cb_fn_copy(*s, file, line);
        }
    }
    free(ownv);
    u64vec_free(&offs);
}

static void rf_end_obj_plan(Plan *p, Rt *r, void *ctx)
{
    plan_end_obj(p, r, *(uint64_t *)ctx);
}

/* An object whose value now lives in a C local that no path can ever reach
   but through checks the compiler proved (`cobc`'s objectless `String`
   locals): its record ends, the value is not dropped -- the local owns it,
   and `cb_drop_local` drops it where the binding ends. */
void cb_forget_obj(uint64_t obj)
{
    HELD_SCOPE e = enter_planned(rf_end_obj_plan, &obj);
    Rt *r = rt();
    if (!objs_get(r, obj))
    {
        return;
    }
    detach(r, obj);
    end_identity(r, obj);
}

/* D-0269: an entry of a function's table of objectless locals (D-0200), as
   the generated C lays it out (`struct cb_note` in `cbrt.h`): the local's
   address, its moved flag's address (NULL for a local the body never moves
   away), its type, and whether it is bound now. */
struct cb_note
{
    void *p;
    const uint8_t *moved;
    uint32_t tid;
    uint8_t live;
};

/* D-0269: a function that binds objectless locals enters, while threads
   run: its table is noted for a contained fault's unwind. No hold on the
   runtime: the thread's own state. */
void cb_notes_push(struct cb_note *t, uint32_t n)
{
    full_on();
    TState *s = ts_raw();
    note_t e = {(uintptr_t)t, n, s->stack.n};
    notevec_push(&s->notes, e);
}

/* D-0269: that function returns: its table leaves. */
void cb_notes_pop(void)
{
    TState *s = ts_raw();
    if (s->notes.n)
    {
        s->notes.n--;
    }
}

/* D-0315 (step 4): `[Join]` of a result that has no object: `cb_join`,
   which then stamps nothing into the statement. */
void cb_join_plain(uint64_t tid, void *dst)
{
    (void)cb_join(tid, dst);
}

/* D-0315: a statement's temporary whose type has a destructor of its own
   ends where its statement ends: its own registration leaves, though a
   value bound later in the statement registered after it. */
void cb_notes_drop(struct cb_note *t)
{
    notevec *notes = &ts_raw()->notes;
    for (size_t i = notes->n; i-- > 0;)
    {
        if (notes->p[i].addr == (uintptr_t)t)
        {
            memmove(&notes->p[i], &notes->p[i + 1], (notes->n - i - 1) * sizeof(note_t));
            notes->n--;
            return;
        }
    }
}

/* D-0269: the noted objectless locals of a table end, the live entries not
   moved away, the function's last bound first (`[Block-Exit]`'s order). */
void end_note_table(uintptr_t addr, uint32_t n)
{
    for (size_t i = n; i-- > 0;)
    {
        struct cb_note *e = (struct cb_note *)addr + i;
        if (e->live != 0 && (!e->moved || *e->moved == 0))
        {
            /* D-0315 (step 4): ended once, though both its own registration
               and the body's whole table name it. */
            e->live = 0;
            drop_in_place((uintptr_t)e->p, e->tid, 0);
        }
    }
}

/* D-0315 (step 4): this thread's open scopes, frames and tables folded,
   innermost first (`[Fault-Unwind]`, `[Block-Exit]`'s order): the entries
   above a table's depth, then the table. */
void fold_frames_and_notes(void)
{
    if (HOOK(FULL.fold))
    {
        HOOK(FULL.fold)();
        return;
    }
    /* No object, path or note was ever made: the open frames and scopes
       own nothing, and are dropped as they are. */
    for (;;)
    {
        TState *s = ts();
        if (s->stack.n == 0)
        {
            break;
        }
        Entry e = s->stack.p[--s->stack.n];
        Rt *r = rt();
        if (e.kind == E_FRAME)
        {
            give_vec(rf_pool_cur(r), e.u.owned);
        }
        else
        {
            if (s->scopes.n)
            {
                s->scopes.n--;
            }
            give_vec(rf_pool_cur(r), e.u.s.temps);
            give_vec(rf_pool_cur(r), e.u.s.toks);
        }
    }
}

static void fold_full(void)
{
    for (;;)
    {
        TState *t = ts_raw();
        size_t depth = t->stack.n;
        if (t->notes.n)
        {
            note_t nt = t->notes.p[t->notes.n - 1];
            if (nt.depth >= depth)
            {
                t->notes.n--;
                end_note_table(nt.addr, nt.n);
                continue;
            }
        }
        TState *s = ts();
        if (s->stack.n == 0)
        {
            break;
        }
        Entry e = s->stack.p[--s->stack.n];
        if (e.kind == E_FRAME)
        {
            end_owned(e.u.owned);
        }
        else
        {
            if (s->scopes.n)
            {
                s->scopes.n--;
            }
            end_scope(e.u.s);
        }
    }
    for (;;)
    {
        TState *t = ts_raw();
        if (t->notes.n == 0)
        {
            break;
        }
        note_t nt = t->notes.p[--t->notes.n];
        end_note_table(nt.addr, nt.n);
    }
}

/* The value at `addr` of a reference-free resource type with no object
   (`cb_forget_obj`) ends: `[Destroy]` with no record to end. */
void cb_drop_local(void *addr, uint32_t ty)
{
    ENTERED_SCOPE e = enter();
    drop_in_place((uintptr_t)addr, ty, 0);
}

/* D-0089: a plain value holding `fn` values ends (a discarded or dropped
   temporary with no object of its own): its closures end. */
void cb_drop_value(void *addr, uint32_t ty)
{
    ENTERED_SCOPE e = enter();
    drop_in_place((uintptr_t)addr, ty, 0);
}

/* D-0089: a call through an emptied `fn` slot. */
void cb_fn_moved(void)
{
    fault("diag.stale-binding", NOLOC, 0);
}

typedef struct
{
    uintptr_t dst;
    uintptr_t src;
    uint32_t ty;
} rf_copy_ctx;

static void rf_copy_plan(Plan *p, Rt *r, void *ctx)
{
    rf_copy_ctx *c = (rf_copy_ctx *)ctx;
    uint64_t sz = rt_typ(r, c->ty)->size;
    uintptr_t size = (uintptr_t)(sz ? sz : 1);
    plan_forget(p, r, c->dst, c->dst + size);
    plan_slots_of(p, r, c->src, c->ty);
}

/* Where a value of type `ty` at `addr` has its reference slots: the
   type's own list where the places do not depend on the value (nothing is
   built), else a list from the pool in `*buf`, which the caller gives
   back when `buf->cap`. */
static inline const uint64_t *rf_offs(Rt *r, uintptr_t addr, uint32_t ty, u64vec *buf, uint32_t *n)
{
    TypeInfo *info = rt_typ(r, ty);
    if (LIKELY(info->has_fixed_refs))
    {
        *n = info->nfixed_refs;
        return info->fixed_refs;
    }
    *buf = take_vec(rf_pool_cur(r));
    ref_offsets(r, addr, ty, 0, buf);
    *n = buf->n;
    return buf->p;
}

/* Heap 0's slots when the lone thread runs and there is one heap (a slot
   is then read where it is, `slotmap_get_near`), else NULL. */
static inline SlotMap *rf_slots_lone(Rt *r)
{
    return lone() && heaps_one(&r->slots.n) ? &r->slots.h[0] : NULL;
}

void cb_copy_datum(void *dst, const void *src, uint32_t ty)
{
    rf_copy_ctx c = {(uintptr_t)dst, (uintptr_t)src, ty};
    HELD_SCOPE e = enter_planned(rf_copy_plan, &c);
    Rt *r = rt();
    /* The offsets and the slots found are kept in the runtime's reusable
       lists (`pool`, `slot_buf`): a copy of a value holding references is
       one of a program's commonest runtime events. */
    u64vec obuf = {0};
    uint32_t noffs;
    const uint64_t *offs = rf_offs(r, (uintptr_t)src, ty, &obuf, &noffs);
    /* `dst`'s bytes are now the new value's: a reference slot the old value
       had where the new one has none holds nothing any more, and its path
       one holder fewer. */
    uintptr_t d = (uintptr_t)dst;
    uintptr_t size = (uintptr_t)rt_typ(r, ty)->size;
    size_t h = cur_heap();
    /* A small value: its few slots read where they are. */
    addr_tok few[8];
    size_t nfew = slots_few(r, d, size, false, few);
    if (nfew != SIZE_MAX)
    {
        for (size_t i = 0; i < nfew; i++)
        {
            uintptr_t a = few[i].a;
            bool kept = false;
            for (uint32_t k = 0; k < noffs; k++)
            {
                if (d + (uintptr_t)offs[k] == a)
                {
                    kept = true;
                    break;
                }
            }
            if (!kept)
            {
                Rt *r2 = rt();
                slots_remove(r2, a);
                drop_occ(r2, few[i].t);
            }
        }
        for (uint32_t k = 0; k < noffs; k++)
        {
            uint64_t off = offs[k];
            SlotMap *m0 = rf_slots_lone(rt());
            uintptr_t sa = (uintptr_t)src + (uintptr_t)off;
            uint64_t t = m0 ? slotmap_get_near(m0, sa) : slots_get(rt(), sa);
            if (t)
            {
                cb_store_ref((uint8_t *)dst + off, t);
            }
        }
        if (obuf.cap)
        {
            give_vec(rf_pool_cur(rt()), obuf);
        }
        return;
    }
    atvec held = r->slot_buf[h];
    memset(&r->slot_buf[h], 0, sizeof(atvec));
    slots_scan_into(r, d, d + size, false, &held);
    for (size_t i = 0; i < held.n; i++)
    {
        uintptr_t a = held.p[i].a;
        bool kept = false;
        for (uint32_t k = 0; k < noffs; k++)
        {
            if (d + (uintptr_t)offs[k] == a)
            {
                kept = true;
                break;
            }
        }
        if (!kept)
        {
            Rt *r2 = rt();
            slots_remove(r2, a);
            drop_occ(r2, held.p[i].t);
        }
    }
    held.n = 0;
    r = rt();
    if (r->slot_buf[h].cap < held.cap)
    {
        atvec_free(&r->slot_buf[h]);
        r->slot_buf[h] = held;
    }
    else
    {
        atvec_free(&held);
    }
    for (uint32_t k = 0; k < noffs; k++)
    {
        uint64_t off = offs[k];
        uint64_t t = slots_get(rt(), (uintptr_t)src + (uintptr_t)off);
        if (t)
        {
            cb_store_ref((uint8_t *)dst + off, t);
        }
    }
    if (obuf.cap)
    {
        give_vec(rf_pool_cur(rt()), obuf);
    }
}

/* D-0213 `[Thread-Lend]` (spec/19 §1, §3): every exclusive reference stored
   in `[addr, addr + size)` is replaced by a path borrowed from it, held
   where it was and in transit to whichever thread first accesses through
   it. The sender's own copies of the reference stay valid and now clash
   with the new path while it lives. */
void cb_lend(void *addr, uint64_t size, uint32_t file, uint32_t line)
{
    replay_out(me(), "threads");
    if (size == 0 || !addr)
    {
        return;
    }
    ENTERED_SCOPE e = enter();
    uintptr_t lo = (uintptr_t)addr;
    atvec held = {0};
    slots_scan_into(rt(), lo, lo + (uintptr_t)size, false, &held);
    for (size_t i = 0; i < held.n; i++)
    {
        uintptr_t a = held.p[i].a;
        uint64_t t = held.p[i].t;
        Rt *r = rt();
        PathRec *p = paths_get(r, t);
        if (!(p && p->exclusive && !p->is_root && !p->has_lock_of))
        {
            continue;
        }
        uint64_t c = borrow_in(r, t, NULL, 0, true, file, line, false);
        atomic_store_explicit(&paths_at(r, c)->thread, TRANSIT, memory_order_release);
        cb_store_ref((void *)a, c);
    }
    atvec_free(&held);
}

/* `cb_lend` over a closure's captures (a `move` closure handed to `spawn`),
   and over the closures among them. */
void cb_lend_fn(void *handle, uint32_t file, uint32_t line)
{
    if (!handle)
    {
        return;
    }
    void *data;
    uint32_t ty;
    {
        Entered e = enter();
        Rt *r = rt();
        if (!map_u64_get(&r->fn_boxes, (uint64_t)(uintptr_t)handle))
        {
            entered_drop(e);
            return;
        }
        const CbFnBox *b = (const CbFnBox *)handle;
        data = b->data;
        ty = b->ty;
        entered_drop(e);
    }
    uint64_t size;
    {
        Entered e = enter();
        size = rt_typ(rt(), ty)->size;
        entered_drop(e);
    }
    if (size == 0)
    {
        return;
    }
    cb_lend(data, size, file, line);
    u64vec offs = {0};
    {
        Entered e = enter();
        fn_offsets(rt(), (uintptr_t)data, ty, 0, &offs);
        entered_drop(e);
    }
    for (uint32_t i = 0; i < offs.n; i++)
    {
        void **slot = (void **)((uint8_t *)data + offs.p[i]);
        if (*slot)
        {
            cb_lend_fn(*slot, file, line);
        }
    }
    u64vec_free(&offs);
}

/* D-0218 `[Read-Fn-Through-Shared]`: a `fn` value reached through a shared
   reference is read or called only when no closure in it holds an
   exclusive capture (its body would write through it). */
bool fn_exclusive_capture(void *handle, uint32_t depth)
{
    if (!handle || depth > 64)
    {
        return false;
    }
    void *data;
    uint32_t ty;
    uint64_t size;
    {
        Entered e = enter();
        Rt *r = rt();
        if (!map_u64_get(&r->fn_boxes, (uint64_t)(uintptr_t)handle))
        {
            entered_drop(e);
            return false;
        }
        const CbFnBox *b = (const CbFnBox *)handle;
        /* D-0239: a closure that writes its own captures (`cobc` marks its
           box 3) is not called through a shared reference either. */
        if (b->closure == 3)
        {
            entered_drop(e);
            return true;
        }
        data = b->data;
        ty = b->ty;
        size = rt_typ(r, b->ty)->size;
        entered_drop(e);
    }
    if (size == 0)
    {
        return false;
    }
    u64vec offs = {0};
    {
        Entered e = enter();
        uintptr_t lo = (uintptr_t)data;
        atvec held = {0};
        Rt *r = rt();
        slots_scan_into(r, lo, lo + (uintptr_t)size, false, &held);
        bool any = false;
        for (size_t i = 0; i < held.n; i++)
        {
            PathRec *p = paths_get(r, held.p[i].t);
            if (p && p->exclusive && !p->is_root && !p->has_lock_of)
            {
                any = true;
                break;
            }
        }
        atvec_free(&held);
        if (any)
        {
            entered_drop(e);
            return true;
        }
        fn_offsets(r, lo, ty, 0, &offs);
        entered_drop(e);
    }
    bool any = false;
    for (uint32_t i = 0; i < offs.n && !any; i++)
    {
        any = fn_exclusive_capture(*(void **)((uint8_t *)data + offs.p[i]), depth + 1);
    }
    u64vec_free(&offs);
    return any;
}

void cb_fn_shared(void *handle, uint32_t file, uint32_t line)
{
    if (fn_exclusive_capture(handle, 0))
    {
        ENTERED_SCOPE e = enter();
        if (fn_self_writing(handle, 0))
        {
            fault("diag.borrow-exceeds-source\001this closure, reached through a shared reference, changes a value it captured by `move` (D-0239); pass its owner by `ref<_, exclusive>`", file, line);
        }
        fault("diag.borrow-exceeds-source\001this closure, reached through a shared reference, writes through an exclusive reference it captured (D-0218); pass its owner by `ref<_, exclusive>`", file, line);
    }
}

/* D-0239: whether the closure in `handle`, or one in a `fn` value it
   captured, writes its own captures; asked only once `fn_exclusive_capture`
   has answered yes, to name the reason. The runtime's lock is held. */
bool fn_self_writing(void *handle, uint32_t depth)
{
    if (!handle || depth > 64)
    {
        return false;
    }
    Rt *r = rt();
    if (!map_u64_get(&r->fn_boxes, (uint64_t)(uintptr_t)handle))
    {
        return false;
    }
    const CbFnBox *b = (const CbFnBox *)handle;
    if (b->closure == 3)
    {
        return true;
    }
    uint64_t size = rt_typ(r, b->ty)->size;
    if (size == 0)
    {
        return false;
    }
    u64vec offs = {0};
    fn_offsets(r, (uintptr_t)b->data, b->ty, 0, &offs);
    bool any = false;
    for (uint32_t i = 0; i < offs.n && !any; i++)
    {
        any = fn_self_writing(*(void **)((uint8_t *)b->data + offs.p[i]), depth + 1);
    }
    u64vec_free(&offs);
    return any;
}

/* The same for each `fn` slot of a value of type `ty` at `addr`. */
void cb_fn_shared_value(const void *addr, uint32_t ty, uint32_t file, uint32_t line)
{
    u64vec offs = {0};
    {
        Entered e = enter();
        fn_offsets(rt(), (uintptr_t)addr, ty, 0, &offs);
        entered_drop(e);
    }
    for (uint32_t i = 0; i < offs.n; i++)
    {
        cb_fn_shared(*(void *const *)((const uint8_t *)addr + offs.p[i]), file, line);
    }
    u64vec_free(&offs);
}

/* ---- values in flight ---- */

static inline void rf_inflight_push(uint32_t kind, uint64_t x, u64vec datum)
{
    Inflight f;
    f.kind = kind;
    f.x = x;
    f.datum = datum;
    infq_push_back(&ts_raw()->inflight, f);
}

static inline u64vec rf_no_datum(void)
{
    u64vec v = {0};
    return v;
}

void cb_send(uint64_t obj)
{
    HELD_SCOPE h = enter_on(heap_of(obj));
    detach(rt(), obj);
    rf_inflight_push(IF_OBJ, obj, rf_no_datum());
}

typedef struct
{
    uint64_t obj;
    uintptr_t to;
} rf_move_ctx;

static void rf_move_plan(Plan *p, Rt *r, void *ctx)
{
    rf_move_ctx *c = (rf_move_ctx *)ctx;
    plan_move_obj(p, r, c->obj, c->to);
}

uint64_t cb_recv(void *addr)
{
    /* D-0202: local for an object of this thread's heap whose type holds no
       reference (its move rekeys no slot). */
    HELD_SCOPE e;
    if (!multi())
    {
        e = H_SOLO;
    }
    else
    {
        infq *q = &ts_raw()->inflight;
        uint64_t front = (q->n && infq_at(q, 0)->kind == IF_OBJ) ? infq_at(q, 0)->x : 0;
        rf_move_ctx c = {front, (uintptr_t)addr};
        e = enter_planned(rf_move_plan, &c);
    }
    Inflight f;
    if (!infq_pop_front(&ts_raw()->inflight, &f) || f.kind != IF_OBJ)
    {
        cbrt_internal("receive of an object that was not sent");
    }
    uint64_t obj = migrate(rt(), f.x);
    cb_move_to(obj, addr);
    Rt *r = rt();
    size_t scope = top_scope(r);
    stamp_temp(r, obj, scope);
    return obj;
}

/* D-0202: an object received from another heap (a spawn's argument, a
   join's result) moves into this thread's heap under a new id, so its new
   owner's work on it stays local. Only a value nothing else names:
   detached, with no path at all, not over raw cells, and holding no
   reference. The caller holds both heaps. The old id is stale (its
   generation moved on). */
uint64_t migrate(Rt *r, uint64_t obj)
{
    if (!multi() || heap_of(obj) == cur_heap())
    {
        return obj;
    }
    Obj *o = objs_get(r, obj);
    bool movable = o && o->alive && o->toks.v.n == 0 && o->root == 0 && !o->reclaimed && !o->ephemeral && o->frame == NO_IDX && o->scope == NO_IDX && !rt_typ(r, o->ty)->has_refs;
    if (!movable)
    {
        return obj;
    }
    if (atomic_load_explicit(&STATS_ON, memory_order_relaxed))
    {
        atomic_fetch_add_explicit(&N_MIGRATED, 1, memory_order_relaxed);
    }
    set_mark(obj, false);
    Obj ob;
    if (!objs_remove(r, obj, &ob))
    {
        cbrt_internal("no entry under this id");
    }
    bool marked = ob.toks.v.n != 0;
    uint64_t id = objs_reserve(r);
    ensure_mark(id);
    *objs_insert(r, id) = ob;
    set_mark(id, marked);
    return id;
}

/* A reference leaving a function: out of the callee's scopes, into the
   caller's current statement -- the same journey an object makes. */
void cb_send_ref(uint64_t tok)
{
    /* D-0202: the path's record and this thread's stack: local. */
    HELD_SCOPE e = enter_on(heap_of(tok));
    Rt *r = rt();
    if (paths_get(r, tok))
    {
        replay_restamp(me(), tok, false);
        detach_tok(r, tok);
        /* The flight holds it until `cb_recv_ref` (`cb_send_datum`'s rule). */
        paths_at(r, tok)->occ++;
        rf_inflight_push(IF_REF, tok, rf_no_datum());
    }
    else if ((tok & LIGHT) != 0)
    {
        /* D-0292: a light token travels as it is; nothing holds it. */
        rf_inflight_push(IF_REF, tok, rf_no_datum());
    }
    else
    {
        rf_inflight_push(IF_REF, 0, rf_no_datum());
    }
}

void cb_recv_ref(void)
{
    HELD_SCOPE e;
    if (!multi())
    {
        e = H_SOLO;
    }
    else
    {
        infq *q = &ts_raw()->inflight;
        uint64_t front = (q->n && infq_at(q, 0)->kind == IF_REF) ? infq_at(q, 0)->x : 0;
        e = enter_on(heap_of(front));
    }
    Rt *r = rt();
    Inflight f;
    if (!infq_pop_front(&ts_raw()->inflight, &f) || f.kind != IF_REF)
    {
        cbrt_internal("receive of a reference that was not sent");
    }
    uint64_t tok = f.x;
    if (tok != 0 && paths_get(r, tok))
    {
        replay_flight_in(me(), tok);
        size_t scope = top_scope(r);
        stamp_tok(r, tok, scope);
        /* The flight's hold ends; the statement's scope has it now. */
        PathRec *p = paths_at(r, tok);
        if (p->occ)
        {
            p->occ--;
        }
    }
}

/* A reference that is a statement's result: re-stamped outward. One that
   no statement owns (formed long before, and held by some slot) is stamped
   to the statement around the current one. */
void cb_result_ref(uint64_t tok)
{
    HELD_SCOPE e = enter_on(heap_of(tok));
    Rt *r = rt();
    PathRec *p = paths_get(r, tok);
    if (!p)
    {
        return;
    }
    size_t cur = p->scope;
    size_t i;
    if (cur != NO_IDX)
    {
        detach_tok(r, tok);
        i = cur;
    }
    else
    {
        i = top_scope(r);
        if (i == NO_IDX)
        {
            return;
        }
    }
    size_t parent = scope_below(i);
    replay_restamp(me(), tok, true);
    stamp_tok(r, tok, parent);
}

typedef struct
{
    uintptr_t src;
    uint32_t ty;
} rf_send_ctx;

static void rf_send_datum_plan(Plan *p, Rt *r, void *ctx)
{
    rf_send_ctx *c = (rf_send_ctx *)ctx;
    plan_slots_of(p, r, c->src, c->ty);
    /* `unname_sent`'s temporary: this thread's, in its own heap. */
    plan_heap(p, cur_heap());
}

void cb_send_datum(const void *src, uint32_t ty)
{
    rf_send_ctx c = {(uintptr_t)src, ty};
    HELD_SCOPE e = enter_planned(rf_send_datum_plan, &c);
    Rt *r = rt();
    u64vec obuf = {0};
    uint32_t noffs;
    const uint64_t *offs = rf_offs(r, (uintptr_t)src, ty, &obuf, &noffs);
    u64vec toks = take_vec(rf_pool_cur(r));
    u64vec_reserve(&toks, noffs);
    SlotMap *m0 = rf_slots_lone(r);
    for (uint32_t i = 0; i < noffs; i++)
    {
        uintptr_t a = (uintptr_t)src + (uintptr_t)offs[i];
        toks.p[toks.n++] = m0 ? slotmap_get_near(m0, a) : slots_get(r, a);
    }
    /* In flight, as `cb_send_ref`'s: a path formed in the sending statement
       must not end with that statement, and the sender's copy no longer
       holds it. `cb_recv_datum` stores it, held again, at the receiver;
       until then the flight itself holds it. */
    for (uint32_t i = 0; i < noffs; i++)
    {
        uint64_t t = toks.p[i];
        if (t != 0)
        {
            if (UNLIKELY(replay_on()))
            {
                replay_restamp(me(), t, false);
            }
            detach_tok(r, t);
            uintptr_t a = (uintptr_t)src + (uintptr_t)offs[i];
            if ((m0 ? slotmap_remove(m0, a) : slots_remove(r, a)) == 0)
            {
                PathRec *p = paths_get(r, t);
                if (p)
                {
                    p->occ++;
                }
            }
        }
    }
    if (obuf.cap)
    {
        give_vec(rf_pool_cur(r), obuf);
    }
    unname_sent(r, (uintptr_t)src);
    rf_inflight_push(IF_DATUM, 0, toks);
}

/* CHG-0372. The temporary the sender made for the datum it sends, if any:
   it holds none of the datum's slots now, but it lasts until that
   statement ends, and the storage it names may end first (a callee's local
   inlined into the caller, D-0277). So it no longer names that storage,
   and its end forgets nothing there. Plain data only. */
void unname_sent(Rt *r, uintptr_t a)
{
    TState *t = ts();
    if (t->scopes.n == 0)
    {
        return;
    }
    size_t si = t->scopes.p[t->scopes.n - 1];
    if (si >= t->stack.n || t->stack.p[si].kind != E_SCOPE)
    {
        return;
    }
    Scope *s = &t->stack.p[si].u.s;
    for (uint32_t i = s->temps.n; i-- > 0;)
    {
        Obj *ob = objs_get_inl(r, s->temps.p[i]);
        if (ob && ob->alive && ob->addr == a && !ob->is_resource && !ob->reclaimed)
        {
            ob->addr = 0;
            return;
        }
    }
}

/* A native body returning `Some(r)` for an element reference `r` (`cobc`'s
   native `HashMap::get`/`get_mut`): the element's `[Reclaim]` and
   `[Borrow]` as `cb_elem_borrow` makes them, the path sent as the datum's
   one reference. No temporary object holds it on the way. */
uint64_t cb_elem_borrow_datum(void *addr, uint32_t ty, uint32_t mode)
{
    replay_out(me(), "a vector element");
    HELD_SCOPE e = enter_elem((uintptr_t)addr, ty);
    uint64_t tok = elem_borrow(addr, ty, mode, false, false);
    Rt *r = rt();
    PathRec *p = paths_get(r, tok);
    if (p)
    {
        p->occ++;
    }
    u64vec toks = take_vec(rf_pool_cur(r));
    u64vec_push(&toks, tok);
    rf_inflight_push(IF_DATUM, 0, toks);
    return tok;
}

/* A native body returning reference data with one reference, `tok`, a path
   it formed just now and stored in no slot (`cobc`'s native
   `StringView::sub`): sent as `cb_send_datum` sends such a datum. */
void cb_send_tok_datum(uint64_t tok)
{
    /* No path (a `str`'s view): nothing of the runtime's to touch. */
    if (tok == 0 && ts_raw()->pend.n == 0)
    {
        u64vec v = {0};
        u64vec_push(&v, 0);
        rf_inflight_push(IF_DATUM, 0, v);
        return;
    }
    HELD_SCOPE e = enter_on(heap_of(tok));
    Rt *r = rt();
    if (paths_get(r, tok))
    {
        detach_tok(r, tok);
        PathRec *p = paths_get(r, tok);
        if (p)
        {
            p->occ++;
        }
    }
    u64vec toks = take_vec(rf_pool_cur(r));
    u64vec_push(&toks, tok);
    rf_inflight_push(IF_DATUM, 0, toks);
}

/* The token of the `k`-th of the `n` data in flight to this thread (0:
   none), left in flight: `cobc`'s dual-body dispatcher reads a slice
   argument's path before the body it chooses receives it. A queue of any
   other shape is the compiler's error, not the program's. */
uint64_t cb_peek_datum_tok(size_t k, size_t n)
{
    ENTERED_SCOPE e = enter();
    infq *q = &ts_raw()->inflight;
    if (q->n != n || k >= n)
    {
        char a[24], b[24];
        rf_dec(a, q->n);
        rf_dec(b, n);
        cbrt_internalf("dual-body dispatch found %s data in flight, expected %s", a, b);
    }
    Inflight *f = infq_at(q, k);
    if (f->kind != IF_DATUM)
    {
        cbrt_internal("dual-body dispatch found something other than reference data in flight");
    }
    if (f->datum.n == 1)
    {
        return f->datum.p[0];
    }
    if (f->datum.n == 0)
    {
        return 0;
    }
    cbrt_internal("dual-body dispatch found reference data with more than one reference");
}

/* Whether a set of paths over one object overlaps where it matters:
   `a` normalized when either has a range, else step by step over the
   shorter. */
static bool rf_paths_overlap(const Proj *a, size_t an, const Proj *b, size_t bn)
{
    if (has_range(a, an) || has_range(b, bn))
    {
        Norm na;
        norm_of(&na, a, an);
        size_t len;
        const Proj *np = norm_get(&na, &len);
        bool o = ranged_overlap_with(np, len, b, bn);
        norm_free(&na);
        return o;
    }
    size_t m = an < bn ? an : bn;
    for (size_t i = 0; i < m; i++)
    {
        if (!proj_eq(a[i], b[i]))
        {
            return false;
        }
    }
    return true;
}

/* Whether the `n` paths `toks` (each exclusive where `excl` says so) are
   all valid, over live objects that are not reclaimed cells, not
   lock-derived, and pairwise disjoint wherever one of a pair is exclusive:
   then `cobc`'s dual-body dispatcher may run the body that reads them
   unchecked. Anything it cannot vouch for answers no. */
uint8_t cb_paths_disjoint(size_t n, const uint64_t *toks, const uint8_t *excl)
{
    /* D-0202: local when every path is in this thread's heap. */
    size_t c = cur_heap();
    bool all = true;
    for (size_t i = 0; i < n; i++)
    {
        if (heap_of(toks[i]) != c)
        {
            all = false;
            break;
        }
    }
    HELD_SCOPE e = all ? enter_on(c) : held_excl();
    Rt *r = rt();
    PathView small[8];
    bool small_has[8];
    PathView *recs = n <= 8 ? small : (PathView *)cb_xmalloc(n * sizeof(PathView));
    bool *has = n <= 8 ? small_has : (bool *)cb_xmalloc(n * sizeof(bool));
    uint8_t ok = 1;
    for (size_t i = 0; i < n && ok; i++)
    {
        uint64_t t = toks[i];
        /* D-0292: a light reference is vouched for (as token 0 would be): it
           takes part in no pair. */
        if (light_ref(t) && !frame_rec())
        {
            has[i] = false;
            continue;
        }
        /* Not over a reclaimed object when there are pairs to compare; a
           single reclaimed path only while this thread is alone (D-0209,
           spec/08 §4 H5). */
        PathView v;
        Obj *o = NULL;
        bool good = view(r, t, &v) && !v.has_lock_of;
        if (good)
        {
            o = objs_get(r, v.of);
            good = o && o->alive && o->init && (!o->reclaimed || (n == 1 && atomic_load_explicit(&cb_live_threads, memory_order_acquire) == 1));
        }
        if (!good)
        {
            ok = 0;
            break;
        }
        recs[i] = v;
        has[i] = true;
    }
    for (size_t i = 0; i < n && ok; i++)
    {
        for (size_t j = i + 1; j < n && ok; j++)
        {
            if (!has[i] || !has[j])
            {
                continue;
            }
            if (!(excl[i] != 0 || excl[j] != 0) || recs[i].of != recs[j].of)
            {
                continue;
            }
            if (rf_paths_overlap(recs[i].proj, recs[i].nproj, recs[j].proj, recs[j].nproj))
            {
                ok = 0;
            }
        }
    }
    if (recs != small)
    {
        free(recs);
        free(has);
    }
    return ok;
}

/* D-0219: `cb_paths_disjoint`, and each path clash-free as it arrives
   (spec/08 §4 `[Standing-Local]` H3), for an argument that is an existing
   reference passed on. */
uint8_t cb_paths_arrive(size_t n, const uint64_t *toks, const uint8_t *excl)
{
    if (cb_paths_disjoint(n, toks, excl) == 0)
    {
        return 0;
    }
    size_t c = cur_heap();
    bool all = true;
    for (size_t i = 0; i < n; i++)
    {
        if (heap_of(toks[i]) != c)
        {
            all = false;
            break;
        }
    }
    HELD_SCOPE e = all ? enter_on(c) : held_excl();
    Rt *r = rt();
    for (size_t i = 0; i < n; i++)
    {
        uint64_t t = toks[i];
        if (light_ref(t) && !frame_rec())
        {
            continue;
        }
        if (access_clashes(r, t, excl[i] != 0))
        {
            return 0;
        }
    }
    return 1;
}

/* Whether an access through `tok` in its own mode would meet `clash`, as
   `check_access` computes it, without faulting (D-0219). */
bool access_clashes(Rt *r, uint64_t tok, bool exclusive)
{
    PathView rec;
    if (!view(r, tok, &rec))
    {
        return true;
    }
    Obj *o = objs_get(r, rec.of);
    if (!o || !o->alive)
    {
        return true;
    }
    const u64vec *toks = exclusive ? &o->toks.v : &o->xtoks.v;
    bool ranged = has_range(rec.proj, rec.nproj);
    Norm mine;
    bool have_mine = false;
    bool clash = false;
    for (uint32_t k = 0; k < toks->n; k++)
    {
        uint64_t t = toks->p[k];
        PathRec *p = paths_at(r, t);
        if (!p->valid || p->is_root || p->occ == 0)
        {
            continue;
        }
        if (!rec.is_root && (t == tok || rf_u64s_has(rec.ancestors, rec.nancestors, t)))
        {
            continue;
        }
        if (ranged || has_range(p->proj.p, p->proj.n))
        {
            if (!have_mine)
            {
                norm_of(&mine, rec.proj, rec.nproj);
                have_mine = true;
            }
            size_t mlen;
            const Proj *mp = norm_get(&mine, &mlen);
            if (!ranged_overlap_with(mp, mlen, p->proj.p, p->proj.n))
            {
                continue;
            }
        }
        else
        {
            size_t m = rec.nproj < p->proj.n ? rec.nproj : p->proj.n;
            bool differ = false;
            for (size_t i = 0; i < m; i++)
            {
                if (!proj_eq(rec.proj[i], p->proj.p[i]))
                {
                    differ = true;
                    break;
                }
            }
            if (differ)
            {
                continue;
            }
        }
        if (rec.has_lock_of != p->has_lock_of && !(rec.has_lock_of ? p->exclusive : exclusive))
        {
            continue;
        }
        if (exclusive || p->exclusive)
        {
            clash = true;
            break;
        }
    }
    if (have_mine)
    {
        norm_free(&mine);
    }
    return clash;
}

/* D-0209: whether a function's direct reference parameters (`cobc`'s
   `NAME__dir`, D-0188) may run: each token valid, not lock-derived, over a
   live, initialized object, and clash-free as it arrives (D-0219); while
   other threads run, also not reclaimed cells (H5) and an exclusive path of
   this thread's own (D-0213). A 0 token passes. */
uint8_t cb_direct_ok(size_t n, const uint64_t *toks)
{
    size_t c = cur_heap();
    bool all = true;
    for (size_t i = 0; i < n; i++)
    {
        if (!(toks[i] == 0 || heap_of(toks[i]) == c))
        {
            all = false;
            break;
        }
    }
    HELD_SCOPE e = all ? enter_on(c) : held_excl();
    Rt *r = rt();
    bool others = atomic_load_explicit(&cb_live_threads, memory_order_relaxed) != 1;
    uint64_t m = me();
    for (size_t i = 0; i < n; i++)
    {
        uint64_t t = toks[i];
        if (t == 0 || (light_ref(t) && !frame_rec()))
        {
            continue;
        }
        PathView p;
        Obj *o = NULL;
        bool good = view(r, t, &p) && !p.has_lock_of;
        if (good)
        {
            o = objs_get(r, p.of);
            good = o && o->alive && o->init && (!others || !o->reclaimed);
        }
        if (!good)
        {
            return 0;
        }
        if (others && p.exclusive && !p.is_root && p.rec && atomic_load_explicit(&p.rec->thread, memory_order_acquire) != m)
        {
            return 0;
        }
        if (access_clashes(r, t, p.exclusive))
        {
            return 0;
        }
    }
    return 1;
}

/* The same body returning `None`: a datum with no reference in flight. */
void cb_send_datum_none(void)
{
    ENTERED_SCOPE e = enter();
    rf_inflight_push(IF_DATUM, 0, rf_no_datum());
}

/* Reference data with at most one reference (`Option<ref<T>>`) received
   with no slot to hold it (`cobc`'s match on a call's result): the path
   back in the statement's scope, as `cb_recv_ref` puts one, and its token
   returned (0: none sent). It ends with the statement unless a binder holds
   it (`cb_frame_hold`). */
uint64_t cb_recv_datum_tok(void)
{
    /* D-0204: the one path's records (its heap, peeked at in this thread's
       own queue) and this thread's scopes: local. */
    infq *q = &ts_raw()->inflight;
    uint64_t front = 0;
    if (q->n)
    {
        Inflight *f = infq_at(q, 0);
        if (f->kind == IF_DATUM && f->datum.n == 1)
        {
            front = f->datum.p[0];
        }
    }
    HELD_SCOPE e = enter_on(heap_of(front));
    Rt *r = rt();
    Inflight f;
    if (!infq_pop_front(&ts_raw()->inflight, &f) || f.kind != IF_DATUM)
    {
        cbrt_internal("receive of reference data that was not sent");
    }
    uint64_t tok;
    if (f.datum.n == 0)
    {
        tok = 0;
    }
    else if (f.datum.n == 1)
    {
        tok = f.datum.p[0];
    }
    else
    {
        cbrt_internal("reference data with more than one reference received as one");
    }
    give_vec(rf_pool_cur(r), f.datum);
    if (tok == 0)
    {
        return 0;
    }
    if (paths_get(r, tok))
    {
        size_t scope = top_scope(r);
        stamp_tok(r, tok, scope);
        /* The flight's hold ends; the statement's scope has it now. */
        PathRec *p = paths_at(r, tok);
        if (p->occ)
        {
            p->occ--;
        }
    }
    return tok;
}

typedef struct
{
    uintptr_t dst;
    uint32_t ty;
} rf_recv_ctx;

static void rf_recv_datum_plan(Plan *p, Rt *r, void *ctx)
{
    rf_recv_ctx *c = (rf_recv_ctx *)ctx;
    infq *q = &ts_raw()->inflight;
    if (q->n)
    {
        Inflight *f = infq_at(q, 0);
        if (f->kind == IF_DATUM)
        {
            for (uint32_t i = 0; i < f->datum.n; i++)
            {
                if (f->datum.p[i] != 0)
                {
                    plan_drop_occ(p, r, f->datum.p[i]);
                }
            }
        }
    }
    uint64_t sz = rt_typ(r, c->ty)->size;
    uintptr_t size = (uintptr_t)(sz ? sz : 1);
    plan_forget(p, r, c->dst, c->dst + size);
}

void cb_recv_datum(void *dst, uint32_t ty)
{
    rf_recv_ctx c = {(uintptr_t)dst, ty};
    HELD_SCOPE e = enter_planned(rf_recv_datum_plan, &c);
    Rt *r = rt();
    Inflight f;
    if (!infq_pop_front(&ts_raw()->inflight, &f) || f.kind != IF_DATUM)
    {
        cbrt_internal("receive of reference data that was not sent");
    }
    u64vec toks = f.datum;
    u64vec obuf = {0};
    uint32_t noffs;
    const uint64_t *offs = rf_offs(r, (uintptr_t)dst, ty, &obuf, &noffs);
    size_t scope = top_scope(r);
    SlotMap *m0 = rf_slots_lone(r);
    uint32_t k = noffs < toks.n ? noffs : toks.n;
    for (uint32_t i = 0; i < k; i++)
    {
        uint64_t off = offs[i];
        uint64_t t = toks.p[i];
        if (t != 0)
        {
            /* Back in a statement's scope, as `cb_recv_ref` puts one: it ends
               with that statement unless something holds it then. */
            PathRec *p = paths_get(rt(), t);
            if (p)
            {
                if (UNLIKELY(replay_on()))
                {
                    replay_flight_in(me(), t);
                }
                stamp_tok_rec(rt(), p, t, scope);
            }
            /* Into an empty slot on the page used last, the path held (or
               gone): the store's one holder more and the flight's one
               fewer are no change, and the path does not end. */
            if (m0 && (p == NULL || p->occ != 0))
            {
                uintptr_t x = slotmap_last_at(m0, (uintptr_t)dst + (uintptr_t)off);
                if (x < 512 && m0->last_ptr != &SLOT_NONE && m0->last_ptr->s[x] == 0 && HOOK(cb_slots_used))
                {
                    m0->last_ptr->s[x] = t;
                    m0->last_ptr->n++;
                    continue;
                }
            }
            cb_store_ref((uint8_t *)dst + off, t);
            /* The flight's hold (`cb_send_datum`) ends: the slot holds it. */
            drop_occ(rt(), t);
        }
    }
    r = rt();
    if (obuf.cap)
    {
        give_vec(rf_pool_cur(r), obuf);
    }
    give_vec(rf_pool_cur(r), toks);
}

/* ---- FULL (rt.h) ---- */

FullOps FULL;

CB_COLD void full_objects(void)
{
    HOOK_SET(FULL.frame_pop_owning, frame_pop_owning);
    HOOK_SET(FULL.fold, fold_full);
    HOOK_SET(FULL.on, true);
}

CB_COLD void full_reclaimed(void)
{
    HOOK_SET(FULL.end_stale_over, end_stale_over);
    HOOK_SET(FULL.vec_drop_plain, vec_drop_plain_full);
    HOOK_SET(FULL.reclaimed, true);
}

CB_COLD void full_slots(void)
{
    HOOK_SET(FULL.forget_slots, forget_slots);
    HOOK_SET(FULL.copy_slots, copy_slots_full);
}
