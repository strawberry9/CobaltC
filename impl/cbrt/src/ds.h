/* The runtime's containers, in C: the counterparts of Rust's standard
   library's `Vec<u64>`, `Vec<Proj>`, `HashMap`/`HashSet` with an id hasher,
   `BTreeMap<usize, u64>` and `VecDeque`. Each is a plain struct a zeroed
   value of which is empty, as Rust's `Default`.

   The hash tables hash with `id_hash` (a multiplicative hash: the tables
   are keyed by sequential ids and by addresses, never by hostile keys) and
   probe linearly, removing by backward shift (no tombstones). Their
   iteration order is their slot order: every place where the runtime
   iterates a hash table and the order could be observed is listed in
   LOG.md ("Hash-table iteration order") and given a defined order there
   instead. */

#ifndef CBRT_DS_H
#define CBRT_DS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "common.h"

/* The id hash: the hash of a `u64` or `usize` key. */
static inline uint64_t id_hash(uint64_t x)
{
    uint64_t h = x * 0x9E3779B97F4A7C15ull;
    return h ^ (h >> 29);
}

/* ---- Vec<u64> ---- */

typedef struct
{
    uint64_t *p;
    uint32_t n;
    uint32_t cap;
} u64vec;

void u64vec_grow(u64vec *v, size_t need);

static inline void u64vec_push(u64vec *v, uint64_t x)
{
    if (__builtin_expect(v->n == v->cap, 0))
    {
        u64vec_grow(v, (size_t)v->n + 1);
    }
    v->p[v->n++] = x;
}

/* `u64vec_push` in line whatever the compiler thinks of it: for the few
   places a checked program reaches at every object and every path. */
static inline __attribute__((always_inline)) void u64vec_push_inl(u64vec *v, uint64_t x)
{
    if (__builtin_expect(v->n == v->cap, 0))
    {
        u64vec_grow(v, (size_t)v->n + 1);
    }
    v->p[v->n++] = x;
}

static inline void u64vec_reserve(u64vec *v, size_t extra)
{
    if ((size_t)v->cap - v->n < extra)
    {
        u64vec_grow(v, (size_t)v->n + extra);
    }
}

static inline void u64vec_extend(u64vec *v, const uint64_t *src, size_t n)
{
    if (n)
    {
        u64vec_reserve(v, n);
        memcpy(v->p + v->n, src, n * sizeof(uint64_t));
        v->n += (uint32_t)n;
    }
}

static inline void u64vec_free(u64vec *v)
{
    /* An empty list, the usual one here, has no block: no call. */
    if (v->p)
    {
        free(v->p);
    }
    v->p = NULL;
    v->n = 0;
    v->cap = 0;
}

static inline bool u64vec_contains(const u64vec *v, uint64_t x)
{
    for (uint32_t i = 0; i < v->n; i++)
    {
        if (v->p[i] == x)
        {
            return true;
        }
    }
    return false;
}

/* `retain(|m| *m != x)`: every occurrence removed, order kept. */
static inline void u64vec_remove_all(u64vec *v, uint64_t x)
{
    uint32_t j = 0;
    for (uint32_t i = 0; i < v->n; i++)
    {
        if (v->p[i] != x)
        {
            v->p[j++] = v->p[i];
        }
    }
    v->n = j;
}

static inline u64vec u64vec_clone(const u64vec *v)
{
    u64vec c = {0};
    u64vec_extend(&c, v->p, v->n);
    return c;
}

/* ---- Vec<(usize, u64)> ---- */

typedef struct
{
    uintptr_t a;
    uint64_t t;
} addr_tok;

typedef struct
{
    addr_tok *p;
    size_t n;
    size_t cap;
} atvec;

static inline void atvec_push(atvec *v, uintptr_t a, uint64_t t)
{
    if (v->n == v->cap)
    {
        size_t c = v->cap ? v->cap * 2 : 8;
        v->p = (addr_tok *)cb_xrealloc(v->p, c * sizeof(addr_tok));
        v->cap = c;
    }
    v->p[v->n].a = a;
    v->p[v->n].t = t;
    v->n++;
}

static inline void atvec_free(atvec *v)
{
    /* An empty vector allocated nothing (most of an object's ends hold no
       slot): no call into the C library for it. */
    if (v->p)
    {
        free(v->p);
    }
    v->p = NULL;
    v->n = 0;
    v->cap = 0;
}

/* `sort_unstable_by_key(|e| e.0)` / `sort_unstable()` of `v[from..]`: by
   address, then token (addresses are unique within one table; across
   tables an address is held in one heap only). */
void atvec_sort_from(atvec *v, size_t from);

/* ---- HashMap<u64, V> / HashSet<u64> ---- */

/* HMAP(NAME, V): a table from `uint64_t` keys to `V` values.
     V *NAME_get(const NAME *m, uint64_t k)        NULL when absent
     V *NAME_slot(NAME *m, uint64_t k, bool *had)  the value's place, made
                                                   (uninitialised) if absent
     bool NAME_remove(NAME *m, uint64_t k, V *old) old may be NULL
     void NAME_clear(NAME *m), NAME_free(NAME *m)
   Iterate with `for (size_t i = 0; i < m->cap; i++) if (m->used[i])`,
   key `m->e[i].k`, value `m->e[i].v`. */
#define HMAP(NAME, V)                                                                  \
    typedef struct                                                                     \
    {                                                                                  \
        uint64_t k;                                                                    \
        V v;                                                                           \
    } NAME##_ent;                                                                      \
    typedef struct                                                                     \
    {                                                                                  \
        NAME##_ent *e;                                                                 \
        uint8_t *used;                                                                 \
        size_t cap;                                                                    \
        size_t n;                                                                      \
    } NAME;                                                                            \
    static inline V *NAME##_get(const NAME *m, uint64_t k)                             \
    {                                                                                  \
        if (m->n == 0)                                                                 \
        {                                                                              \
            return NULL;                                                               \
        }                                                                              \
        size_t mask = m->cap - 1;                                                      \
        size_t i = (size_t)id_hash(k) & mask;                                          \
        while (m->used[i])                                                             \
        {                                                                              \
            if (m->e[i].k == k)                                                        \
            {                                                                          \
                return &m->e[i].v;                                                     \
            }                                                                          \
            i = (i + 1) & mask;                                                        \
        }                                                                              \
        return NULL;                                                                   \
    }                                                                                  \
    static inline bool NAME##_has(const NAME *m, uint64_t k)                           \
    {                                                                                  \
        return NAME##_get(m, k) != NULL;                                               \
    }                                                                                  \
    static void NAME##_rehash(NAME *m, size_t cap)                                     \
    {                                                                                  \
        NAME##_ent *oe = m->e;                                                         \
        uint8_t *ou = m->used;                                                         \
        size_t oc = m->cap;                                                            \
        m->e = (NAME##_ent *)cb_xmalloc(cap * sizeof(NAME##_ent));                     \
        m->used = (uint8_t *)cb_xcalloc(cap, 1);                                       \
        m->cap = cap;                                                                  \
        size_t mask = cap - 1;                                                         \
        for (size_t j = 0; j < oc; j++)                                                \
        {                                                                              \
            if (ou[j])                                                                 \
            {                                                                          \
                size_t i = (size_t)id_hash(oe[j].k) & mask;                            \
                while (m->used[i])                                                     \
                {                                                                      \
                    i = (i + 1) & mask;                                                \
                }                                                                      \
                m->used[i] = 1;                                                        \
                m->e[i] = oe[j];                                                       \
            }                                                                          \
        }                                                                              \
        free(oe);                                                                      \
        free(ou);                                                                      \
    }                                                                                  \
    static inline V *NAME##_slot(NAME *m, uint64_t k, bool *had)                       \
    {                                                                                  \
        if (__builtin_expect((m->n + 1) * 8 > m->cap * 7, 0))                          \
        {                                                                              \
            NAME##_rehash(m, m->cap ? m->cap * 2 : 8);                                 \
        }                                                                              \
        size_t mask = m->cap - 1;                                                      \
        size_t i = (size_t)id_hash(k) & mask;                                          \
        while (m->used[i])                                                             \
        {                                                                              \
            if (m->e[i].k == k)                                                        \
            {                                                                          \
                if (had)                                                               \
                {                                                                      \
                    *had = true;                                                       \
                }                                                                      \
                return &m->e[i].v;                                                     \
            }                                                                          \
            i = (i + 1) & mask;                                                        \
        }                                                                              \
        m->used[i] = 1;                                                                \
        m->e[i].k = k;                                                                 \
        m->n++;                                                                        \
        if (had)                                                                       \
        {                                                                              \
            *had = false;                                                              \
        }                                                                              \
        return &m->e[i].v;                                                             \
    }                                                                                  \
    /* `insert`: true when the key was there (its old value in *old). */             \
    static inline bool NAME##_insert(NAME *m, uint64_t k, V v, V *old)                 \
    {                                                                                  \
        bool had;                                                                      \
        V *p = NAME##_slot(m, k, &had);                                                \
        if (had && old)                                                                \
        {                                                                              \
            *old = *p;                                                                 \
        }                                                                              \
        *p = v;                                                                        \
        return had;                                                                    \
    }                                                                                  \
    static inline bool NAME##_remove(NAME *m, uint64_t k, V *old)                      \
    {                                                                                  \
        if (m->n == 0)                                                                 \
        {                                                                              \
            return false;                                                              \
        }                                                                              \
        size_t mask = m->cap - 1;                                                      \
        size_t i = (size_t)id_hash(k) & mask;                                          \
        while (m->used[i])                                                             \
        {                                                                              \
            if (m->e[i].k == k)                                                        \
            {                                                                          \
                if (old)                                                               \
                {                                                                      \
                    *old = m->e[i].v;                                                  \
                }                                                                      \
                /* Backward shift: later entries of the run move up into the      \
                   hole when their home is at or before it. */                       \
                size_t j = i;                                                          \
                for (;;)                                                               \
                {                                                                      \
                    j = (j + 1) & mask;                                                \
                    if (!m->used[j])                                                   \
                    {                                                                  \
                        break;                                                         \
                    }                                                                  \
                    size_t h = (size_t)id_hash(m->e[j].k) & mask;                      \
                    if (((j - h) & mask) >= ((j - i) & mask))                          \
                    {                                                                  \
                        m->e[i] = m->e[j];                                             \
                        i = j;                                                         \
                    }                                                                  \
                }                                                                      \
                m->used[i] = 0;                                                        \
                m->n--;                                                                \
                return true;                                                           \
            }                                                                          \
            i = (i + 1) & mask;                                                        \
        }                                                                              \
        return false;                                                                  \
    }                                                                                  \
    static inline void NAME##_clear(NAME *m)                                           \
    {                                                                                  \
        if (m->n)                                                                      \
        {                                                                              \
            memset(m->used, 0, m->cap);                                                \
            m->n = 0;                                                                  \
        }                                                                              \
    }                                                                                  \
    static inline void NAME##_free(NAME *m)                                            \
    {                                                                                  \
        free(m->e);                                                                    \
        free(m->used);                                                                 \
        m->e = NULL;                                                                   \
        m->used = NULL;                                                                \
        m->cap = 0;                                                                    \
        m->n = 0;                                                                      \
    }

/* HashSet<u64>: a map with an empty value. */
typedef struct
{
    char unused;
} hset_unit;
HMAP(hset, hset_unit)

static inline bool hset_add(hset *s, uint64_t k)
{
    bool had;
    hset_slot(s, k, &had);
    return !had;
}

/* Common tables. */
HMAP(map_u64, uint64_t)
HMAP(map_ptr, void *)

/* ---- BTreeMap<usize, u64> ---- */

/* An ordered map from addresses to tokens or objects, with range queries
   in address order: a B+ tree whose leaves are linked in order. */
typedef struct omap_node omap_node;
typedef struct
{
    omap_node *root;
    size_t n;
    int depth;
} omap;

bool omap_get(const omap *m, uint64_t k, uint64_t *v);
/* `insert`: true when the key was there (its old value in *old). */
bool omap_insert(omap *m, uint64_t k, uint64_t v, uint64_t *old);
bool omap_remove(omap *m, uint64_t k, uint64_t *old);
void omap_clear(omap *m);

/* A position in the map: the first entry with key >= `lo`. */
typedef struct
{
    const omap_node *leaf;
    int i;
} omap_it;
omap_it omap_lower_bound(const omap *m, uint64_t lo);
static inline bool omap_it_valid(omap_it it)
{
    return it.leaf != NULL;
}
bool omap_it_next(omap_it *it, uint64_t *k, uint64_t *v);

/* ---- VecDeque<T> ---- */

/* DEQUE(NAME, T): a ring buffer of `T`, a power-of-two capacity. */
#define DEQUE(NAME, T)                                                     \
    typedef struct                                                         \
    {                                                                      \
        T *p;                                                              \
        size_t head;                                                       \
        size_t n;                                                          \
        size_t cap;                                                        \
    } NAME;                                                                \
    static void NAME##_grow(NAME *d)                                       \
    {                                                                      \
        size_t c = d->cap ? d->cap * 2 : 8;                                \
        T *p = (T *)cb_xmalloc(c * sizeof(T));                             \
        for (size_t i = 0; i < d->n; i++)                                  \
        {                                                                  \
            p[i] = d->p[(d->head + i) & (d->cap - 1)];                     \
        }                                                                  \
        free(d->p);                                                        \
        d->p = p;                                                          \
        d->head = 0;                                                       \
        d->cap = c;                                                        \
    }                                                                      \
    static inline void NAME##_push_back(NAME *d, T x)                      \
    {                                                                      \
        if (d->n == d->cap)                                                \
        {                                                                  \
            NAME##_grow(d);                                                \
        }                                                                  \
        d->p[(d->head + d->n) & (d->cap - 1)] = x;                         \
        d->n++;                                                            \
    }                                                                      \
    static inline void NAME##_push_front(NAME *d, T x)                     \
    {                                                                      \
        if (d->n == d->cap)                                                \
        {                                                                  \
            NAME##_grow(d);                                                \
        }                                                                  \
        d->head = (d->head - 1) & (d->cap - 1);                            \
        d->p[d->head] = x;                                                 \
        d->n++;                                                            \
    }                                                                      \
    static inline T *NAME##_at(const NAME *d, size_t i)                    \
    {                                                                      \
        return &d->p[(d->head + i) & (d->cap - 1)];                        \
    }                                                                      \
    static inline bool NAME##_pop_front(NAME *d, T *out)                   \
    {                                                                      \
        if (d->n == 0)                                                     \
        {                                                                  \
            return false;                                                  \
        }                                                                  \
        *out = d->p[d->head];                                              \
        d->head = (d->head + 1) & (d->cap - 1);                            \
        d->n--;                                                            \
        return true;                                                       \
    }                                                                      \
    static inline bool NAME##_pop_back(NAME *d, T *out)                    \
    {                                                                      \
        if (d->n == 0)                                                     \
        {                                                                  \
            return false;                                                  \
        }                                                                  \
        *out = d->p[(d->head + d->n - 1) & (d->cap - 1)];                  \
        d->n--;                                                            \
        return true;                                                       \
    }

#endif
