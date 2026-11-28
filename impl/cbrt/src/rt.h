/* cbrt in C -- the internal header of the runtime's core (D-0326 phase B).
   The core is one translation unit (lib.c includes the parts rt_*.c), so
   its hot paths sit together and inline across parts (D-0202).

   The design: objects and paths in arenas indexed by their ids, heaps one
   per running thread (D-0202), reference slots in pages, reclaimed entries
   in ordered maps with page buckets, per-thread state (`TState`), the
   runtime lock made reentrant over an RwLock, heap spin locks, local
   entries planned before they lock (`Plan`). Conventions, so that the
   parts agree:
   - a function of a type `Type` is `type_name` (lower case);
   - an optional id is the id or 0 (no id is 0); an optional index is the
     index or NO_IDX;
   - the runtime's state is `Rt *r`; `rt()` is the one state;
   - a guard (`Held`, `Entered`, `ReadEntered`, a mutex guard) is released
     explicitly, or by GNU C's `cleanup` attribute (HELD_SCOPE etc.) where
     it is held to the end of the block;
   - a state that cannot happen: cbrt_internal (exit status 4). */

#ifndef CBRT_RT_H
#define CBRT_RT_H

#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "common.h"
#include "ds.h"
#include "plat.h"

#define CB_ALWAYS static inline __attribute__((always_inline))
#define CB_COLD __attribute__((cold, noinline))
#define CB_NORET __attribute__((noreturn))
#define LIKELY(x) __builtin_expect(!!(x), 1)
#define UNLIKELY(x) __builtin_expect(!!(x), 0)

#define NO_IDX SIZE_MAX
#define NOLOC 0xFFFFFFFFu
#define NOTYPE 0xFFFFFFFFu

#define K_STRUCT 1u
#define K_ENUM 2u
#define K_ARRAY 3u
#define K_REF 4u
#define K_FN 6u
#define K_HANDLE 7u
#define K_GUARD 8u

/* D-0202: ids carry their heap in bits 55..62; D-0292: bits 53, 54 name a
   token's kind (LIGHT, ROOT). */
#define HEAP_SHIFT 55
#define MAX_HEAPS 256
#define GEN_MASK 0x001fffffu
#define LIGHT (1ull << 53)
#define ROOT (1ull << 54)
#define TOK_FLAGS (LIGHT | ROOT)
/* An id's bits 32..53, its generation and LIGHT above it (a light token is
   no id): an arena place's first word is its generation, with SLOT_SOME
   when it is filled. */
#define ID_GEN_BITS 0x003fffffull
#define SLOT_SOME (1ull << 32)
/* A frame marks a held path's token with the top bit. */
#define HOLD (1ull << 63)
/* D-0213: an exclusive path lent and not yet accessed. */
#define TRANSIT UINT64_MAX

#define SLOT_PAGE 12
#define RECLAIMED_PAGE_BUCKETS 4096
#define THREADS_ENTRIES ((size_t)1 << 40)
#define TOK_INDEX_AT 32
#define NORM_INLINE 12

/* ---- the C interface's structs (cbrt.h) ---- */

typedef struct
{
    void *p;
    uint64_t tok;
} CbRef;

typedef struct
{
    uint32_t kind;
    uint64_t idx;
} CbProj;

typedef struct
{
    uint64_t off;
    uint32_t ty;
} CbField;

typedef struct
{
    const char *name;
    uint64_t size;
    uint32_t kind;
    uint8_t is_resource;
    uint8_t has_refs;
    void (*drop)(CbRef);
    uint32_t nfields;
    const CbField *fields;
    uint32_t nvariants;
    const uint32_t *variants;
    uint64_t payload_off;
    uint32_t elem;
    uint64_t count;
    uint8_t owner;
} CbType;

typedef struct
{
    void *code;
    void *data;
    uint64_t root;
    uint32_t ty;
    /* 0: a fn item; 1: a closure, called with an exclusive borrow of its
       object as `self`; 2: one that captures nothing (no borrow formed). */
    uint32_t closure;
    /* Reports 148, 155: the copies sharing this box beyond the first (a
       decided program's copy of a closure that owns no resource and whose
       closures write none of their captures is the same box), and what is
       known of it (FN_KNOWN and its bits, `fn_flags`). */
    uint32_t refs;
    uint32_t flags;
} CbFnBox;

#define FN_KNOWN 1u
#define FN_OWNS 2u    /* it, or a closure among its captures, owns a resource */
#define FN_WRITES 4u  /* it, or a closure among its captures, writes its captures */
#define FN_REFS 8u    /* its captures, or those of a closure among them, hold a reference */

/* ---- projections ---- */

enum
{
    P_FIELD = 0,
    P_INDEX = 1,
    P_PAYLOAD = 2,
    /* D-0070: the elements lo .. hi a slice borrows. */
    P_RANGE = 3
};

typedef struct
{
    uint32_t k;
    uint64_t a; /* the field or index; a range's lo */
    uint64_t b; /* a range's hi; 0 otherwise */
} Proj;

static inline Proj proj_field(uint64_t i)
{
    Proj p = {P_FIELD, i, 0};
    return p;
}

static inline Proj proj_index(uint64_t i)
{
    Proj p = {P_INDEX, i, 0};
    return p;
}

static inline Proj proj_payload(void)
{
    Proj p = {P_PAYLOAD, 0, 0};
    return p;
}

static inline Proj proj_range(uint64_t l, uint64_t h)
{
    Proj p = {P_RANGE, l, h};
    return p;
}

static inline bool proj_eq(Proj x, Proj y)
{
    return x.k == y.k && x.a == y.a && x.b == y.b;
}

typedef struct
{
    Proj *p;
    uint32_t n;
    uint32_t cap;
} projvec;

void projvec_grow(projvec *v, size_t need);

static inline void projvec_push(projvec *v, Proj x)
{
    if (UNLIKELY(v->n == v->cap))
    {
        projvec_grow(v, (size_t)v->n + 1);
    }
    v->p[v->n++] = x;
}

static inline void projvec_extend(projvec *v, const Proj *src, size_t n)
{
    if (n)
    {
        if ((size_t)v->cap - v->n < n)
        {
            projvec_grow(v, (size_t)v->n + n);
        }
        memcpy(v->p + v->n, src, n * sizeof(Proj));
        v->n += (uint32_t)n;
    }
}

static inline projvec projvec_clone(const projvec *v)
{
    projvec c = {0};
    projvec_extend(&c, v->p, v->n);
    return c;
}

static inline void projvec_free(projvec *v)
{
    if (v->p)
    {
        free(v->p);
    }
    v->p = NULL;
    v->n = 0;
    v->cap = 0;
}

static inline bool has_range(const Proj *p, size_t n)
{
    for (size_t i = 0; i < n; i++)
    {
        if (p[i].k == P_RANGE)
        {
            return true;
        }
    }
    return false;
}

/* A path holding an empty range (`s[k .. k]`) reaches nothing (D-0283). */
static inline bool empty_range(const Proj *p, size_t n)
{
    for (size_t i = 0; i < n; i++)
    {
        if (p[i].k == P_RANGE && p[i].a >= p[i].b)
        {
            return true;
        }
    }
    return false;
}

/* A path with each range that an index follows folded into that index,
   made absolute, into a stack buffer: a path of a few steps is compared
   without allocating (`Norm`). */
typedef struct
{
    Proj buf[NORM_INLINE];
    size_t len;
    projvec spill;
} Norm;

void norm_push(Norm *n, Proj x);
void norm_set_last(Norm *n, Proj x);
static inline const Proj *norm_get(const Norm *n, size_t *len)
{
    if (n->spill.n == 0)
    {
        *len = n->len;
        return n->buf;
    }
    *len = n->spill.n;
    return n->spill.p;
}
/* `Norm::of` over a list. */
void norm_of(Norm *n, const Proj *p, size_t len);
static inline void norm_free(Norm *n)
{
    projvec_free(&n->spill);
}
typedef struct
{
    const Proj *s;
    size_t n;
    size_t i;
} NormIter;
bool norm_iter_next(NormIter *it, Proj *out);
bool ranged_overlap_with(const Proj *a, size_t an, const Proj *b, size_t bn);

/* ---- small vectors ---- */

typedef struct
{
    uint32_t f;
    uint32_t l;
} loc_t;

#define VEC(NAME, T)                                                  \
    typedef struct                                                    \
    {                                                                 \
        T *p;                                                         \
        size_t n;                                                     \
        size_t cap;                                                   \
    } NAME;                                                           \
    static inline void NAME##_push(NAME *v, T x)                      \
    {                                                                 \
        if (UNLIKELY(v->n == v->cap))                                 \
        {                                                             \
            size_t c = v->cap ? v->cap * 2 : 8;                       \
            v->p = (T *)cb_xrealloc(v->p, c * sizeof(T));             \
            v->cap = c;                                               \
        }                                                             \
        v->p[v->n++] = x;                                             \
    }                                                                 \
    static inline T *NAME##_last(const NAME *v)                       \
    {                                                                 \
        return v->n ? &v->p[v->n - 1] : NULL;                         \
    }                                                                 \
    static inline void NAME##_free(NAME *v)                           \
    {                                                                 \
        free(v->p);                                                   \
        v->p = NULL;                                                  \
        v->n = 0;                                                     \
        v->cap = 0;                                                   \
    }

VEC(locvec, loc_t)
VEC(sizevec, size_t)
VEC(boolvec, bool)
VEC(u64vecvec, u64vec)
VEC(projvecvec, projvec)

/* ---- TokList: an object's paths, indexed once long ---- */

typedef struct
{
    u64vec v;
    map_u64 *at; /* NULL while short; else each token's position */
} toklist;

void toklist_push_more(toklist *l, uint64_t t);
void toklist_remove_more(toklist *l, uint64_t t);
/* A short list with room, and a list of the one token: in line (an
   element's object and its one path, at every element access). */
static inline void toklist_push(toklist *l, uint64_t t)
{
    if (LIKELY(!l->at && l->v.n < l->v.cap && l->v.n < TOK_INDEX_AT))
    {
        l->v.p[l->v.n++] = t;
        return;
    }
    toklist_push_more(l, t);
}
static inline void toklist_remove(toklist *l, uint64_t t)
{
    if (LIKELY(!l->at && l->v.n == 1 && l->v.p[0] == t))
    {
        l->v.n = 0;
        return;
    }
    toklist_remove_more(l, t);
}
static inline u64vec toklist_into_vec(toklist *l)
{
    u64vec v = l->v;
    if (l->at)
    {
        map_u64_free(l->at);
        free(l->at);
        l->at = NULL;
    }
    l->v.p = NULL;
    l->v.n = 0;
    l->v.cap = 0;
    return v;
}
static inline toklist toklist_from_vec(u64vec v)
{
    toklist l;
    l.v = v;
    l.at = NULL;
    return l;
}

/* ---- slot pages ---- */

typedef struct
{
    uint64_t s[512];
    uint32_t n;
} SlotPage;

HMAP(pagemap, SlotPage *)

typedef struct
{
    uint8_t heap;
    pagemap pages;
    omap odd;
    SlotPage **spare;
    size_t nspare;
    size_t spare_cap;
    /* The page used last: `last_p` its number (UINTPTR_MAX: none),
       `last_base` its first address (0: none), `last_ptr` the page (none:
       `SLOT_NONE`, which holds nothing and is never written). */
    uintptr_t last_p;
    uintptr_t last_base;
    SlotPage *last_ptr;
    /* The page emptied last stays in `pages` (a frame's one reference
       local empties and fills its page at every call): NULL: none. */
    SlotPage *kept;
    uintptr_t kept_p;
} SlotMap;

extern SlotPage SLOT_NONE;

static inline void slotmap_last(SlotMap *m, uintptr_t p, SlotPage *pg)
{
    m->last_p = p;
    m->last_base = p << SLOT_PAGE;
    m->last_ptr = pg;
}

static inline void slotmap_last_none(SlotMap *m)
{
    m->last_p = UINTPTR_MAX;
    m->last_base = 0;
    m->last_ptr = &SLOT_NONE;
}

/* `a`'s position on the page used last, when it is an aligned address on
   it (one compare for both: a low bit set, rotated to the top, is past
   any position); 512 or more: not on it. On no page, an aligned address
   below 4096 reads `SLOT_NONE`: no slot, as it is. */
static inline uintptr_t slotmap_last_at(const SlotMap *m, uintptr_t a)
{
    uintptr_t x = a - m->last_base;
    return (x >> 3) | (x << 61);
}

uint64_t slotmap_get(SlotMap *m, uintptr_t a);
uint64_t slotmap_remove(SlotMap *m, uintptr_t a);

/* The slot at `a` in `m`, where there is one heap: read on the page used
   last where it is on it. */
static inline uint64_t slotmap_get_near(SlotMap *m, uintptr_t a)
{
    uintptr_t x = slotmap_last_at(m, a);
    if (LIKELY(x < 512))
    {
        return m->last_ptr->s[x];
    }
    return slotmap_get(m, a);
}

/* ---- type info ---- */

typedef struct
{
    uint64_t off;
    uint32_t ty;
} fieldref;

typedef struct
{
    char *name;
    uint64_t size;
    uint32_t kind;
    bool is_resource;
    bool has_refs;
    void (*drop)(CbRef);
    fieldref *fields;
    uint32_t nfields;
    uint32_t *variants;
    uint32_t nvariants;
    uint64_t payload_off;
    uint32_t elem;
    uint64_t count;
    bool owner;
    /* Report 337: `std::Rc` (its descriptor's `owner` 2): its destructor does
       nothing observable after ending its value, so an `Rc` destroyed last
       in that value waits for the destructor's return (`drop_in_place_tail`). */
    bool chain_drop;
    bool holds_fn;
    bool has_box_of;
    uint32_t box_of;
    bool has_vec_plain;
    uint64_t vec_plain;
    fieldref *owning;
    uint32_t nowning;
    bool has_fixed_refs;
    uint64_t *fixed_refs;
    uint32_t nfixed_refs;
    bool tree;
    /* Its destruction frees memory and does nothing else (`box_drop`'s
       tail position): no destructor of its own or of a part but `Box`'s and
       a plain `Vec`'s, no `fn` value. `nquiet_owning`: how many of a
       struct's owning fields, from the first declared, are so. */
    bool quiet;
    uint32_t nquiet_owning;
    bool has_niche;
    size_t niche_some;
    size_t niche_none;
    fieldref *tree_parts;
    uint32_t ntree_parts;
    bool has_box_plan;
    fieldref *box_plan;
    uint32_t nbox_plan;
} TypeInfo;

typedef struct
{
    uint32_t *ids;
    uint32_t *pos;
    size_t mask;
} TypeMap;

/* ---- objects and paths ---- */

typedef struct
{
    uintptr_t addr;
    uint32_t ty;
    bool alive;
    bool init;
    bool is_resource;
    bool reclaimed;
    bool ephemeral;
    uint64_t root;   /* 0: none */
    size_t frame;    /* NO_IDX: none */
    size_t scope;    /* NO_IDX: none */
    uint64_t tid;
    toklist toks;
    toklist xtoks;
} Obj;

typedef struct
{
    uint64_t of;
    projvec proj;
    bool exclusive;
    bool valid;
    bool is_root;
    bool has_lock_of;
    uint32_t occ;
    size_t scope; /* NO_IDX: none */
    u64vec ancestors;
    projvec lock_of;
    _Atomic uint64_t thread;
} PathRec;

/* An arena: entries by `slot + 1` in an id's low 32 bits, a generation in
   the next 21, the heap above. */
#define ARENA(NAME, T)                                                                \
    typedef struct                                                                    \
    {                                                                                 \
        uint32_t g;                                                                   \
        uint32_t some; /* 0 or 1 */                                                   \
        T v;                                                                          \
    } NAME##_slot;                                                                    \
    typedef struct                                                                    \
    {                                                                                 \
        NAME##_slot *slots;                                                           \
        size_t n;                                                                     \
        size_t cap;                                                                   \
        uint32_t *free;                                                               \
        size_t nfree;                                                                 \
        size_t free_cap;                                                              \
        uint8_t heap;                                                                 \
    } NAME;                                                                           \
    static inline uint64_t NAME##_reserve(NAME *a)                                    \
    {                                                                                 \
        size_t i;                                                                     \
        if (a->nfree)                                                                 \
        {                                                                             \
            i = a->free[--a->nfree];                                                  \
        }                                                                             \
        else                                                                          \
        {                                                                             \
            if (a->n == a->cap)                                                       \
            {                                                                         \
                size_t c = a->cap ? a->cap * 2 : 64;                                  \
                a->slots = (NAME##_slot *)cb_xrealloc(a->slots, c * sizeof(NAME##_slot)); \
                a->cap = c;                                                           \
            }                                                                         \
            a->slots[a->n].g = 1;                                                     \
            a->slots[a->n].some = false;                                              \
            i = a->n++;                                                               \
        }                                                                             \
        return ((uint64_t)a->heap << HEAP_SHIFT) | ((uint64_t)a->slots[i].g << 32) | (uint64_t)(i + 1); \
    }                                                                                 \
    /* The entry an id names, or NULL: no id (0: place -1), a place past the  \
       end, an emptied place, another generation, or a token that is not an   \
       id (LIGHT, ROOT). The generation and `some` are read as one word. */   \
    CB_ALWAYS T *NAME##_get(const NAME *a, uint64_t k)                                \
    {                                                                                 \
        size_t i = (size_t)(uint32_t)k - 1;                                           \
        if (LIKELY(i < a->n))                                                         \
        {                                                                             \
            uint64_t have;                                                            \
            memcpy(&have, &a->slots[i], sizeof have);                                 \
            if (LIKELY(have == (((k >> 32) & ID_GEN_BITS) | SLOT_SOME)))              \
            {                                                                         \
                return &a->slots[i].v;                                                \
            }                                                                         \
        }                                                                             \
        return NULL;                                                                  \
    }                                                                                 \
    /* `insert` into the place a reserved id names; the value is written    \
       through the pointer returned. */                                              \
    static inline T *NAME##_insert(NAME *a, uint64_t k)                               \
    {                                                                                 \
        size_t i = (size_t)(uint32_t)k - 1;                                           \
        a->slots[i].some = true;                                                      \
        return &a->slots[i].v;                                                        \
    }                                                                                 \
    static inline bool NAME##_remove(NAME *a, uint64_t k, T *out)                     \
    {                                                                                 \
        size_t i = (size_t)(uint32_t)k - 1;                                           \
        uint32_t g = (uint32_t)(k >> 32) & GEN_MASK;                                  \
        if (i < a->n && a->slots[i].some && a->slots[i].g == g)                       \
        {                                                                             \
            if (out)                                                                  \
            {                                                                         \
                *out = a->slots[i].v;                                                 \
            }                                                                         \
            a->slots[i].some = false;                                                 \
            uint32_t ng = (a->slots[i].g + 1) & GEN_MASK;                             \
            a->slots[i].g = ng ? ng : 1;                                              \
            if (a->nfree == a->free_cap)                                              \
            {                                                                         \
                size_t c = a->free_cap ? a->free_cap * 2 : 64;                        \
                a->free = (uint32_t *)cb_xrealloc(a->free, c * sizeof(uint32_t));     \
                a->free_cap = c;                                                      \
            }                                                                         \
            a->free[a->nfree++] = (uint32_t)i;                                        \
            return true;                                                              \
        }                                                                             \
        return false;                                                                 \
    }                                                                                 \
    /* `remove` of an entry whose record the caller holds. */                \
    CB_ALWAYS void NAME##_drop(NAME *a, uint64_t k)                                   \
    {                                                                                 \
        size_t i = (size_t)(uint32_t)k - 1;                                           \
        a->slots[i].some = false;                                                     \
        uint32_t ng = (a->slots[i].g + 1) & GEN_MASK;                                 \
        a->slots[i].g = ng ? ng : 1;                                                  \
        if (UNLIKELY(a->nfree == a->free_cap))                                        \
        {                                                                             \
            size_t c = a->free_cap ? a->free_cap * 2 : 64;                            \
            a->free = (uint32_t *)cb_xrealloc(a->free, c * sizeof(uint32_t));         \
            a->free_cap = c;                                                          \
        }                                                                             \
        a->free[a->nfree++] = (uint32_t)i;                                            \
    }                                                                                 \
    static inline uint64_t NAME##_id_at(const NAME *a, size_t i)                      \
    {                                                                                 \
        return ((uint64_t)a->heap << HEAP_SHIFT) | ((uint64_t)a->slots[i].g << 32) | (uint64_t)(i + 1); \
    }

ARENA(ObjArena, Obj)
ARENA(PathArena, PathRec)

/* D-0202: one value per heap. Room for every heap is in the struct (the
   unused places are zero pages nobody touches), so a place never moves; `n`
   counts the heaps made besides heap 0 (atomic: read by every thread while
   a spawn fills the next place under the runtime lock). */
typedef struct
{
    ObjArena h[MAX_HEAPS];
    _Atomic size_t n;
} HeapObjs;

typedef struct
{
    PathArena h[MAX_HEAPS];
    _Atomic size_t n;
} HeapPaths;

typedef struct
{
    SlotMap h[MAX_HEAPS];
    _Atomic size_t n;
} HeapSlots;

/* The ephemeral element objects of a heap, by address, with each one's
   extent: one is made and ended at every element or `Box` access, and few
   are alive at once (the accesses a statement nests), so the first
   EPH_SMALL are a short unordered list, searched from its start; more go to
   an ordered map. */
#define EPH_SMALL 16
typedef struct
{
    uintptr_t a;
    uint64_t obj;
    uint64_t size;
} EphEnt;

typedef struct
{
    EphEnt *small; /* EPH_SMALL places; NULL until the first object */
    uint32_t nsmall;
    omap more;
    map_u64 more_sz;
} Eph;

static inline size_t eph_len(const Eph *e)
{
    return e->nsmall + e->more.n;
}

/* Raw address -> the reclaimed object there (`Reclaimed`). */
typedef struct
{
    uint8_t heap;
    omap ordered;
    map_u64 ordered_ix;
    Eph ephemeral;
    map_u64 ordered_sz;
} Reclaimed;

typedef struct
{
    Reclaimed h[MAX_HEAPS];
    _Atomic size_t n;
} HeapReclaimed;

static inline size_t heaps_len(_Atomic size_t *n)
{
    return 1 + atomic_load_explicit(n, memory_order_acquire);
}

/* Whether heap h's place is filled (heap 0's always is; the others once
   `n` has passed them, release/acquire). */
static inline bool heaps_has(_Atomic size_t *n, size_t h)
{
    return h == 0 || h - 1 < atomic_load_explicit(n, memory_order_acquire);
}

/* The next place (index heaps_len) filled by INIT, a statement over `P`,
   a pointer to it, before `n` passes it. */
#define HEAPS_PUSH(T, P, INIT)                                              \
    do                                                                      \
    {                                                                       \
        size_t k_ = heaps_len(&(T)->n);                                     \
        __typeof__(&(T)->h[0]) P = &(T)->h[k_];                             \
        memset(P, 0, sizeof *P);                                            \
        INIT;                                                               \
        atomic_store_explicit(&(T)->n, k_, memory_order_release);           \
    } while (0)

/* ---- frames, scopes, in-flight data ---- */

typedef struct
{
    u64vec temps;
    u64vec toks;
    loc_t loc;
} Scope;

enum
{
    E_FRAME = 0,
    E_SCOPE = 1
};

typedef struct
{
    uint32_t kind;
    /* D-0292: the frame records every reference it forms; a scope's entry
       has its frame's, so the top entry answers for the running frame. */
    bool rec;
    union
    {
        struct /* E_FRAME */
        {
            u64vec owned; /* no block until the frame owns something */
            loc_t floc;   /* where a destruction at its end is reported */
        };
        Scope s; /* E_SCOPE */
    } u;
} Entry;

VEC(entryvec, Entry)

enum
{
    IF_OBJ = 0,
    IF_REF = 1,
    IF_DATUM = 2
};

typedef struct
{
    uint32_t kind;
    uint64_t x;    /* IF_OBJ, IF_REF */
    u64vec datum;  /* IF_DATUM: one token per reference slot, 0 where none */
} Inflight;

DEQUE(infq, Inflight)

typedef struct
{
    uintptr_t addr;
    uint32_t n;
    size_t depth;
} note_t;

VEC(notevec, note_t)

typedef struct
{
    uint64_t root;
    uint64_t obj;
    uintptr_t cells;
    uint64_t size;
    uint32_t ty;
} box_next_t;

/* Report 315: a thread's list of the mutexes it locked, in chunks (the
   first inside its state); `key` 0 is an entry cleared. */
#define HELD_CHUNK 8
typedef struct
{
    uintptr_t key;
    uintptr_t cells;
} HeldMx;
typedef struct HeldChunk
{
    HeldMx e[HELD_CHUNK];
    struct HeldChunk *next;
} HeldChunk;

/* Report 337: an `Rc` waiting for its enclosing `Rc` destructor's return:
   its bytes (an `Rc` is one pointer) and its type. */
#define CHAIN_ITEM_BYTES 16
typedef struct
{
    uint64_t bytes[CHAIN_ITEM_BYTES / 8];
    uint32_t ty;
} chain_item_t;

/* A thread's own part of the state. */
typedef struct
{
    entryvec stack;
    sizevec scopes;
    infq inflight;
    loc_t dloc;
    bool box_draining;
    bool has_box_next;
    box_next_t box_next;
    /* The Boxes handed in tail position while `box_next` was taken, the
       innermost loop's above the length it found (`box_drop`). */
    box_next_t *box_more;
    size_t nbox_more;
    size_t capbox_more;
    /* Report 337: whether the innermost destructor running is `std::Rc`'s
       (`TypeInfo::chain_drop`); the `Rc`s that waited for its return, as
       their bytes; and whether the next one run is a waiting one, whose
       own waiting `Rc`s the loop that runs it takes. */
    /* Report 315: the mutexes this thread locked (`held_push`). */
    HeldChunk held;
    uint32_t nheld;
    bool chain_top;
    bool chain_flat_next;
    chain_item_t *chain;
    size_t nchain;
    size_t capchain;
    locvec pend;
    bool cunwind;
    uintptr_t jmp;
    bool has_failed;
    uint64_t failed;
    /* Report 312: the thread whose handle a `join` or `try_join` of this
       thread consumed and now waits for (0: none); a contained fault that
       ends the wait (a `cancel`) cancels and waits for it, as the handle's
       end would have. */
    uint64_t joining;
    notevec notes;
    /* D-0257: the report of the last fault a library's `_try` entry
       contained on this thread (`cobalt_last_fault`); freed with the
       state, so a C thread that ends leaves nothing (report 141). */
    char *lib_fault;
} TState;

/* ---- threads: the book ---- */

enum
{
    ST_RUNNING = 0,
    ST_DONE = 1,
    ST_FAILED = 2,
    ST_TAKEN = 3
};

typedef struct
{
    uint32_t status;
    uint64_t failed; /* ST_FAILED: the failure's index */
    uintptr_t env;
    uint64_t env_size;
    uint32_t res_ty;
    uint64_t res_obj;
    /* Report 134: its handle was destroyed on the thread itself (held
       through an `Arc` the thread owned): nobody waits for it but the
       program's end. */
    bool self_owned;
} ThreadRec;

enum
{
    W_JOIN = 0,
    W_LOCK = 1,
    W_EVENT = 2,
    W_ATOMIC = 3,
    W_TIMER = 4
};

typedef struct
{
    uint32_t k;
    uint64_t a;
    uint64_t b;
} Wait;

static inline bool wait_eq(Wait x, Wait y)
{
    return x.k == y.k && x.a == y.a && x.b == y.b;
}

typedef struct
{
    char *raw;
    uint32_t file;
    uint32_t line;
    char *text;
} Failure;

VEC(failvec, Failure)

HMAP(threadmap, ThreadRec)
HMAP(waitmap, Wait)

typedef struct
{
    threadmap threads;
    map_u64 atomic_counts;
    hset cancel;
    hset lent;
    failvec failures;
    waitmap waits;
    hset deadlocked;
    /* Report 321: the deadlocked thread that was the only end of the
       chains of waits when the deadlock was found (`stop_reason`). */
    hset lone_waiter;
    map_u64 os;
    map_u64 wake;
    /* Report 238: the failures' indices freed (`failure_free`), for the
       next failure to reuse. */
    u64vec free_failures;
    /* Report 070: the mutexes a contained fault poisoned, by their state
       cells, and that fault in a few words (`fault_brief`). */
    map_ptr poisoned_by;
    /* Report 315: thread id -> its state while it runs, for clearing the
       entry of a mutex whose guard another thread ends (`release_moved`). */
    map_ptr held_lists;
} Book;

Book *book(void);
void book_unlock(void);

bool book_still_waits(const Book *b, const Wait *w);
bool book_closes_cycle(const Book *b, uint64_t me, const Wait *kind);
void book_find_deadlock(Book *b);

/* ---- the runtime's state ---- */

HMAP(map_sz, size_t)

typedef struct
{
    char **files;
    size_t nfiles;
    TypeInfo *types;
    size_t ntypes;
    TypeMap type_map;
    HeapObjs objs;
    HeapPaths paths;
    HeapSlots slots;
    atvec slot_buf[MAX_HEAPS];
    HeapReclaimed reclaimed;
    map_u64 fn_items;  /* code address -> its interned box */
    hset pure_items;
    map_u64 fn_boxes;  /* closure box handle -> the closure object it owns */
    bool unwinding;
    char *fault_report; /* NULL: none */
    u64vecvec pool[MAX_HEAPS];
    u64vec heap_users; /* u32 counts, kept as u64 */
    projvecvec proj_pool[MAX_HEAPS];
} Rt;

extern Rt RT;
extern bool RT_INIT;

/* ---- what a program that forms no object does not link ----

   The ends of objects, paths, reference slots and noted locals are reached
   from entries every program has (a frame's end, a fault's unwind,
   `[Deallocate]`, `[Copy-Raw]`, a plain vector's drop) only when such state
   exists. The code that makes the first of each kind fills its part of FULL
   -- an object, a path or a note (`full_on`); an object over raw cells
   (`full_reclaimed_on`); a stored reference (`full_slots`) -- and those
   entries call through it: where a pointer is still null, there is no such
   state and nothing to do. A program that makes none of a kind links none
   of that kind's code. */
typedef struct
{
    bool on;        /* an object, a path or a note was made */
    bool reclaimed; /* an object over raw cells was made */
    void (*frame_pop_owning)(void);
    void (*fold)(void);
    void (*end_stale_over)(Rt *r, uintptr_t a, uint64_t n);
    void (*vec_drop_plain)(void *addr, uint64_t len, uint64_t size);
    void (*forget_slots)(Rt *r, uintptr_t addr, uint64_t size);
    void (*copy_slots)(uintptr_t d, uintptr_t s, uint64_t n);
} FullOps;
extern FullOps FULL;
/* A pointer or a flag of this kind is written by whichever thread first
   needs it and read by all, every writer writing the same value: relaxed
   atomic accesses, so that no read and write of it race. */
#define HOOK(x) __atomic_load_n(&(x), __ATOMIC_RELAXED)
#define HOOK_SET(x, v) __atomic_store_n(&(x), (v), __ATOMIC_RELAXED)
void full_objects(void);
void full_reclaimed(void);
void full_slots(void);
CB_ALWAYS void full_on(void)
{
    if (UNLIKELY(!HOOK(FULL.on)))
    {
        full_objects();
    }
}
CB_ALWAYS void full_reclaimed_on(void)
{
    if (UNLIKELY(!HOOK(FULL.reclaimed)))
    {
        full_reclaimed();
    }
}

static inline Rt *rt(void)
{
    return &RT;
}

TypeInfo *rt_typ_slow(Rt *r, uint32_t id) CB_NORET;

static inline size_t typemap_get(const TypeMap *m, uint32_t id, bool *found)
{
    size_t at = (size_t)(id * 0x9E3779B1u) & m->mask;
    for (;;)
    {
        uint32_t k = m->ids[at];
        if (k == id)
        {
            *found = true;
            return m->pos[at];
        }
        if (k == NOTYPE)
        {
            *found = false;
            return 0;
        }
        at = (at + 1) & m->mask;
    }
}

static inline TypeInfo *rt_typ(Rt *r, uint32_t id)
{
    bool f;
    size_t i = typemap_get(&r->type_map, id, &f);
    if (UNLIKELY(!f))
    {
        cbrt_internalf("no type descriptor has the id %u", id);
    }
    return &r->types[i];
}

static inline size_t rt_typ_pos(Rt *r, uint32_t id)
{
    bool f;
    size_t i = typemap_get(&r->type_map, id, &f);
    if (UNLIKELY(!f))
    {
        cbrt_internalf("no type descriptor has the id %u", id);
    }
    return i;
}

static inline TypeInfo *rt_typ_opt(Rt *r, uint32_t id)
{
    bool f;
    size_t i = typemap_get(&r->type_map, id, &f);
    return f ? &r->types[i] : NULL;
}

/* ---- globals ---- */

extern _Atomic bool RECORD_FULL;
extern _Atomic bool RECORD_CHECK;
extern _Atomic bool DECIDED_PROGRAM;
extern _Atomic bool PH3_ON;
extern _Atomic uint64_t N_ROOT_LIGHT, N_ROOT_REC, N_PATHS, N_SLOW, N_PROJ_ADDR, N_TOK0_OWNER, N_LIGHT, N_HOLDERS, N_ARRIVALS;
extern _Atomic uint64_t N_LOCAL, N_EXCL, N_MIGRATED;
extern _Atomic bool STATS_ON;

CB_ALWAYS void ph3_count(_Atomic uint64_t *c)
{
    if (atomic_load_explicit(&PH3_ON, memory_order_relaxed))
    {
        atomic_fetch_add_explicit(c, 1, memory_order_relaxed);
    }
}

/* The exported data (cbrt.h). */
extern uint8_t *cb_marks[256];
extern uint8_t cb_inline_off;
extern size_t MARK_LEN[256];
extern _Atomic size_t cb_reclaimed_entries;
extern _Atomic uint32_t cb_reclaimed_pages[RECLAIMED_PAGE_BUCKETS];
extern _Atomic uint64_t cb_live_threads;
extern _Atomic uint8_t cb_threaded;
extern _Atomic uint8_t cb_sched_flag;
extern uint8_t cb_alloc_traced;

extern _Atomic bool MULTI;
extern _Atomic bool SOLO_PENDING;
extern plat_rwlock RTLOCK;
extern _Atomic uint64_t OWNER;
extern _Atomic size_t HEAPS_N;
extern _Atomic bool HEAP_SHARED[MAX_HEAPS];
extern _Atomic bool SCHED_ON;
extern _Atomic size_t IN_IO;
extern _Atomic size_t CANCELS;

/* The thread-locals. */
typedef struct
{
    uint64_t w[4];
} HeapSet;

extern __thread uint64_t TID;
/* Report 337: how many destructors the runtime runs on this thread now
   (`drop_in_place_tail`): a stack exhausted inside one ends the program
   without the unwind, whose destructors would end those values again. */
extern __thread uint32_t DROPS_RUNNING;
/* Report 319: `cb_exit`'s unwind is running. */
extern bool EXIT_UNWINDING;
extern __thread uint8_t HEAP;
extern __thread TState *TSP;
extern __thread uint32_t HELD;
extern __thread bool GUARD;  /* this thread holds RTLOCK for writing */
extern __thread bool RGUARD; /* ... for reading */
extern __thread uint32_t RHELD;
extern __thread uint32_t LHELD;
extern __thread HeapSet LSET;
extern __thread size_t LSOLE;
extern __thread size_t XN;
extern __thread bool SCHED_PENDING;
extern __thread bool HAS_TIMER_DEADLINE;
extern __thread uint64_t TIMER_DEADLINE;

extern TState MAIN_TS;
extern uint32_t CALLBACK_DEPTH;

#define NO_SOLE SIZE_MAX

CB_ALWAYS bool multi(void)
{
    return atomic_load_explicit(&MULTI, memory_order_relaxed);
}

static inline uint64_t me(void)
{
    return multi() ? TID : 1;
}

CB_ALWAYS size_t cur_heap(void)
{
    return HEAP;
}

CB_ALWAYS TState *ts_raw(void)
{
    TState *p = multi() ? TSP : NULL;
    return p ? p : &MAIN_TS;
}

void make_pending(TState *t);
void make_pending_one(TState *t);

/* `COBALTC_VERIFY`'s stride: -1 not yet read, 0 off (rt_threads.c). */
extern _Atomic int64_t VERIFY_STRIDE;

/* The trace (D-0299): 0: not yet read; 1: off; 2: on; 3: on and stopped. */
extern _Atomic uint8_t RP_STATE;
extern _Atomic bool LONE;
bool replay_init(void);
static inline bool replay_on(void)
{
    uint8_t s = atomic_load_explicit(&RP_STATE, memory_order_relaxed);
    if (LIKELY(s == 1))
    {
        return false;
    }
    return s == 0 ? replay_init() : s == 2;
}

CB_ALWAYS TState *ts(void)
{
    TState *t = ts_raw();
    if (UNLIKELY(t->pend.n != 0))
    {
        make_pending(t);
    }
    return t;
}

/* Whether the innermost frame on `t`'s stack records (D-0292): every
   entry has its frame's flag. */
static inline bool top_rec(const TState *t)
{
    return t->stack.n != 0 && t->stack.p[t->stack.n - 1].rec;
}

/* A new place on top of `t`'s stack, to be filled. */
static inline Entry *entry_push(TState *t)
{
    entryvec *v = &t->stack;
    if (UNLIKELY(v->n == v->cap))
    {
        size_t c = v->cap ? v->cap * 2 : 16;
        v->p = (Entry *)cb_xrealloc(v->p, c * sizeof(Entry));
        v->cap = c;
    }
    return &v->p[v->n++];
}

/* The lone thread, untraced: this is thread 1 on heap 0 with the main
   state (`go_solo` runs on thread 1 alone), no entry takes a lock and no
   step is recorded. The entries that run at every statement of a program
   in the checked forms do their common case under it, in line. */
CB_ALWAYS bool lone(void)
{
    return LIKELY(atomic_load_explicit(&LONE, memory_order_relaxed));
}

/* `LONE` is `!multi()` and the trace read and off, kept as one flag for
   `lone()`: written again wherever either changes (`replay_init`,
   `go_multi`, `go_solo`). */
static inline void lone_set(void)
{
    atomic_store_explicit(&LONE, !multi() && atomic_load_explicit(&RP_STATE, memory_order_relaxed) == 1, memory_order_relaxed);
}

/* What a core knows of the thread it runs on. `L`: the lone thread, a
   constant where the core is inlined into an entry's lone path, false in
   its general one: thread 1, heap 0, the main state, no trace. */
#define ME_(L) ((L) ? (uint64_t)1 : me())
#define HEAP_(L) ((L) ? (size_t)0 : cur_heap())
#define TRACED_(L) (!(L) && UNLIKELY(replay_on()))
CB_ALWAYS TState *ts_(bool L)
{
    TState *t = L ? &MAIN_TS : ts_raw();
    if (UNLIKELY(t->pend.n != 0))
    {
        if (L)
        {
            make_pending_one(t);
        }
        else
        {
            make_pending(t);
        }
    }
    return t;
}

/* ---- heap sets ---- */

static const HeapSet HEAPSET_EMPTY = {{0, 0, 0, 0}};

static inline void heapset_add(HeapSet *s, size_t h)
{
    if (h < 256)
    {
        s->w[h >> 6] |= 1ull << (h & 63);
    }
}

static inline bool heapset_has(const HeapSet *s, size_t h)
{
    return h < 256 && (s->w[h >> 6] & (1ull << (h & 63))) != 0;
}

static inline bool heapset_covers(const HeapSet *s, const HeapSet *o)
{
    for (int i = 0; i < 4; i++)
    {
        if (o->w[i] & ~s->w[i])
        {
            return false;
        }
    }
    return true;
}

#define HEAPSET_EACH(SET, H, BODY)                         \
    do                                                     \
    {                                                      \
        for (int hs_i_ = 0; hs_i_ < 4; hs_i_++)            \
        {                                                  \
            uint64_t hs_w_ = (SET).w[hs_i_];               \
            while (hs_w_ != 0)                             \
            {                                              \
                size_t H = (size_t)hs_i_ * 64 + (size_t)__builtin_ctzll(hs_w_); \
                hs_w_ &= hs_w_ - 1;                        \
                BODY;                                      \
            }                                              \
        }                                                  \
    } while (0)

static inline HeapSet heapset_cur_and(size_t h)
{
    HeapSet s = HEAPSET_EMPTY;
    heapset_add(&s, cur_heap());
    if (h < atomic_load_explicit(&HEAPS_N, memory_order_acquire))
    {
        heapset_add(&s, h);
    }
    return s;
}

/* ---- holds ---- */

/* Whether the entry being run may touch heap h's records: any heap under
   an exclusive hold or with one thread, else the heaps its local hold
   locked. */
CB_ALWAYS bool holds_heap(size_t h)
{
    return LHELD == 0 || heapset_has(&LSET, h);
}

bool held_strict(size_t h);

/* The one heap the address tables need consult, or NO_IDX. */
CB_ALWAYS size_t sole_heap(bool one)
{
    if (one)
    {
        return 0;
    }
    if (LHELD == 0)
    {
        return NO_IDX;
    }
    return LSOLE == NO_SOLE ? NO_IDX : LSOLE;
}

/* The heap an object's or path's id names; no id (0): this thread's own. */
CB_ALWAYS size_t heap_of(uint64_t id)
{
    if (id == 0)
    {
        return cur_heap();
    }
    return (size_t)(id >> HEAP_SHIFT);
}

/* How an entry holds the runtime (`Held`). */
typedef uint8_t Held;
#define H_SOLO 0
#define H_LOCAL 1
#define H_EXCL 2
#define H_NESTED 3
#define H_GONE 255 /* dropped already */

void held_release(uint8_t tag);
Held held_excl(void);
Held enter_set_multi(HeapSet set);

/* ---- what a program that starts no thread does not link ----

   The entries below take the runtime's locks only while threads run
   (`multi`). What they call then is reached through the MT_ pointers,
   which `go_multi` fills before it turns `multi` on (`mt_on`): a program
   in which no thread is ever started links none of the locking. */
extern void (*MT_held_release)(uint8_t tag);
extern Held (*MT_enter_set)(HeapSet set);

CB_ALWAYS void held_drop(Held h)
{
    if (h == H_GONE)
    {
        return;
    }
    if (h == H_SOLO && !multi())
    {
        return;
    }
    HOOK(MT_held_release)(h);
}

static inline void held_cleanup(Held *h)
{
    held_drop(*h);
}

/* `drop(h)` before the end of the block. */
static inline void held_end(Held *h)
{
    held_drop(*h);
    *h = H_GONE;
}

#define HELD_SCOPE __attribute__((cleanup(held_cleanup), unused)) Held

CB_ALWAYS Held enter_on(size_t h)
{
    if (!multi())
    {
        return H_SOLO;
    }
    return HOOK(MT_enter_set)(heapset_cur_and(h));
}

/* `Entered`: 0 / 1, a bool; ENT_GONE when forgotten or dropped. */
typedef uint8_t Entered;
#define ENT_GONE 255
void enter_multi(void);
void lock_release(void);
extern void (*MT_enter)(void);
extern void (*MT_lock_release)(void);

CB_ALWAYS Entered enter(void)
{
    bool m = multi();
    if (m)
    {
        HOOK(MT_enter)();
    }
    return (Entered)m;
}

CB_ALWAYS void entered_drop(Entered e)
{
    if (e == ENT_GONE)
    {
        return;
    }
    if (e || multi())
    {
        HOOK(MT_lock_release)();
    }
}

static inline void entered_cleanup(Entered *e)
{
    entered_drop(*e);
}

static inline void entered_end(Entered *e)
{
    entered_drop(*e);
    *e = ENT_GONE;
}

#define ENTERED_SCOPE __attribute__((cleanup(entered_cleanup), unused)) Entered

/* `ReadEntered`. */
typedef uint8_t ReadEntered;
ReadEntered enter_read_multi(void);
extern ReadEntered (*MT_enter_read)(void);

CB_ALWAYS ReadEntered enter_read(void)
{
    if (!multi())
    {
        return 0;
    }
    return HOOK(MT_enter_read)();
}

void read_entered_release(void);
extern void (*MT_read_release)(void);

CB_ALWAYS void read_entered_drop(ReadEntered e)
{
    if (e == 1)
    {
        HOOK(MT_read_release)();
    }
}

static inline void read_entered_cleanup(ReadEntered *e)
{
    read_entered_drop(*e);
}

#define READ_SCOPE __attribute__((cleanup(read_entered_cleanup), unused)) ReadEntered

void lock_acquire(uint32_t count);
uint32_t lock_release_all(void);
void unlock_heaps(void);
void unlock_local(void);
void lock_set(const HeapSet *set);
void fault_lock(void);
void go_solo(void);
void go_multi(void);
/* A program with entry points (`cb_entries_on`, rt_init.c): locking stays on. */
extern _Atomic bool CB_ENTRIES_ON;
void thread_known(void);
void hlock(size_t h);
void hunlock(size_t h);

/* The hold a lock-free step on this thread's own stack and pools needs:
   none (H_GONE), unless this thread's heap is shared. */
Held own_heap_shared(void);
extern Held (*MT_own_heap_shared)(void);
CB_ALWAYS Held own_heap(void)
{
    if (multi() && atomic_load_explicit(&HEAP_SHARED[cur_heap()], memory_order_relaxed))
    {
        return HOOK(MT_own_heap_shared)();
    }
    return H_GONE;
}

/* ---- planned entries (D-0202) ---- */

typedef struct
{
    HeapSet need;
    bool ok;
} Plan;

Plan plan_new(void);
bool plan_heap(Plan *p, size_t h);
bool plan_cells(Plan *p, uintptr_t lo, uintptr_t hi);
void plan_drop_occ(Plan *p, Rt *r, uint64_t tok);
void plan_retire(Plan *p, Rt *r, uint64_t tok);
void plan_invalidate(Plan *p, Rt *r, uint64_t tok);
void plan_forget(Plan *p, Rt *r, uintptr_t lo, uintptr_t hi);
void plan_end_obj(Plan *p, Rt *r, uint64_t obj);
void plan_move_obj(Plan *p, Rt *r, uint64_t obj, uintptr_t to);
void plan_lock_path(Plan *p, Rt *r, uint64_t tok);
void plan_store(Plan *p, Rt *r, uintptr_t slot, uint64_t tok);
void plan_slots_of(Plan *p, Rt *r, uintptr_t addr, uint32_t ty);
void plan_destroy(Plan *p, Rt *r, uint64_t obj);
void plan_drop_value(Plan *p, Rt *r, uintptr_t addr, uint32_t ty);

typedef void (*plan_fn)(Plan *p, Rt *r, void *ctx);
Held enter_planned_multi(plan_fn f, void *ctx);
extern Held (*MT_enter_planned)(plan_fn f, void *ctx);
CB_ALWAYS Held enter_planned(plan_fn f, void *ctx)
{
    if (!multi())
    {
        return H_SOLO;
    }
    return HOOK(MT_enter_planned)(f, ctx);
}
Held enter_scope_end(void);
Held enter_frame_end(void);
bool ancestors_local(uint64_t tok);

/* ---- tables ---- */

/* Objects and paths: `HeapArena` over every heap. */
/* `objs_get_inl`, `paths_get_inl`: the lookup in line, for the entries
   that run at every access; `objs_get`, `paths_get` are the same out of
   line where the compiler judges so. */
CB_ALWAYS Obj *objs_get_inl(Rt *r, uint64_t k)
{
    size_t h = (size_t)(k >> HEAP_SHIFT);
    /* Heap 0, the lone thread's, at a fixed place. */
    if (LIKELY(h == 0))
    {
        return ObjArena_get(&r->objs.h[0], k);
    }
    if (!heaps_has(&r->objs.n, h))
    {
        return NULL;
    }
    return ObjArena_get(&r->objs.h[h], k);
}

CB_ALWAYS PathRec *paths_get_inl(Rt *r, uint64_t k)
{
    size_t h = (size_t)(k >> HEAP_SHIFT);
    if (LIKELY(h == 0))
    {
        return PathArena_get(&r->paths.h[0], k);
    }
    if (!heaps_has(&r->paths.n, h))
    {
        return NULL;
    }
    return PathArena_get(&r->paths.h[h], k);
}

static inline Obj *objs_get(Rt *r, uint64_t k)
{
    size_t h = (size_t)(k >> HEAP_SHIFT);
    if (UNLIKELY(h != 0) && !heaps_has(&r->objs.n, h))
    {
        return NULL;
    }
    return ObjArena_get(&r->objs.h[h], k);
}

static inline PathRec *paths_get(Rt *r, uint64_t k)
{
    size_t h = (size_t)(k >> HEAP_SHIFT);
    if (UNLIKELY(h != 0) && !heaps_has(&r->paths.n, h))
    {
        return NULL;
    }
    return PathArena_get(&r->paths.h[h], k);
}

/* `objs[&k]`: the entry, which must exist. */
static inline Obj *objs_at(Rt *r, uint64_t k)
{
    Obj *o = objs_get(r, k);
    if (UNLIKELY(!o))
    {
        cbrt_internal("no entry under this id");
    }
    return o;
}

static inline PathRec *paths_at(Rt *r, uint64_t k)
{
    PathRec *p = paths_get(r, k);
    if (UNLIKELY(!p))
    {
        cbrt_internal("no entry under this id");
    }
    return p;
}

static inline uint64_t objs_reserve(Rt *r)
{
    full_on();
    return ObjArena_reserve(&r->objs.h[cur_heap()]);
}

static inline uint64_t objs_reserve_in(Rt *r, size_t h)
{
    full_on();
    return ObjArena_reserve(&r->objs.h[h]);
}

static inline uint64_t paths_reserve_in(Rt *r, size_t h)
{
    full_on();
    return PathArena_reserve(&r->paths.h[h]);
}

static inline Obj *objs_insert(Rt *r, uint64_t k)
{
    return ObjArena_insert(&r->objs.h[k >> HEAP_SHIFT], k);
}

static inline PathRec *paths_insert(Rt *r, uint64_t k)
{
    return PathArena_insert(&r->paths.h[k >> HEAP_SHIFT], k);
}

static inline bool objs_remove(Rt *r, uint64_t k, Obj *out)
{
    if (k == 0 || (k & LIGHT) != 0)
    {
        return false;
    }
    size_t h = (size_t)(k >> HEAP_SHIFT);
    if (h != 0 && h - 1 >= atomic_load_explicit(&r->objs.n, memory_order_acquire))
    {
        return false;
    }
    return ObjArena_remove(&r->objs.h[h], k, out);
}

static inline bool paths_remove(Rt *r, uint64_t k, PathRec *out)
{
    if (k == 0 || (k & LIGHT) != 0)
    {
        return false;
    }
    size_t h = (size_t)(k >> HEAP_SHIFT);
    if (h != 0 && h - 1 >= atomic_load_explicit(&r->paths.n, memory_order_acquire))
    {
        return false;
    }
    return PathArena_remove(&r->paths.h[h], k, out);
}

bool objs_heap_empty(Rt *r, size_t h);
bool paths_heap_empty(Rt *r, size_t h);

/* Slot maps. */
uint64_t slotmap_get(SlotMap *m, uintptr_t a);   /* 0: none */
uint64_t slotmap_insert(SlotMap *m, uintptr_t a, uint64_t t); /* the old, or 0 */
uint64_t slotmap_remove(SlotMap *m, uintptr_t a); /* the old, or 0 */
typedef void (*tok_fn)(uint64_t t, void *ctx);
void slotmap_each_in(SlotMap *m, uintptr_t lo, uintptr_t hi, tok_fn f, void *ctx);
bool slotmap_any_in(SlotMap *m, uintptr_t lo, uintptr_t hi);
void slotmap_scan_into(SlotMap *m, uintptr_t lo, uintptr_t hi, bool take, atvec *out);

static inline bool slotmap_is_empty(const SlotMap *m)
{
    return m->odd.n == 0 && (m->pages.n == 0 || (m->pages.n == 1 && m->kept && m->kept->n == 0));
}

/* HeapSlots */
static inline bool heaps_one(_Atomic size_t *n)
{
    return atomic_load_explicit(n, memory_order_acquire) == 0;
}

size_t slots_holder(Rt *r, uintptr_t a); /* NO_IDX: none */
uint64_t slots_get(Rt *r, uintptr_t a);
uint64_t slots_insert(Rt *r, uintptr_t a, uint64_t t);
uint64_t slots_remove(Rt *r, uintptr_t a);
void slots_each_in(Rt *r, uintptr_t lo, uintptr_t hi, tok_fn f, void *ctx);
bool slots_any_in(Rt *r, uintptr_t lo, uintptr_t hi);
void slots_scan_into(Rt *r, uintptr_t lo, uintptr_t hi, bool take, atvec *out);
atvec slots_range(Rt *r, uintptr_t lo, uintptr_t hi);
bool slots_heap_empty(Rt *r, size_t h);
bool slots_all_empty(Rt *r);

/* Reclaimed */
bool reclaimed_get1(Reclaimed *m, uintptr_t a, uint64_t *obj);
void reclaimed_insert1(Reclaimed *m, uintptr_t a, uint64_t obj, uint64_t size);
void reclaimed_insert_ephemeral1(Reclaimed *m, uintptr_t a, uint64_t obj, uint64_t size);
void reclaimed_remove1(Reclaimed *m, uintptr_t a);
void reclaimed_remove_if1(Reclaimed *m, uintptr_t a, uint64_t obj, bool ephemeral);
bool reclaimed_any_in1(const Reclaimed *m, uintptr_t lo, uintptr_t hi);
void reclaimed_range1(const Reclaimed *m, uintptr_t lo, uintptr_t hi, atvec *out);
static inline size_t reclaimed_entries1(const Reclaimed *m)
{
    return m->ordered.n + eph_len(&m->ephemeral);
}

/* HeapReclaimed */
bool reclaimed_get(Rt *r, uintptr_t a, uint64_t *obj);
void reclaimed_insert(Rt *r, uintptr_t a, uint64_t obj, uint64_t size);
void reclaimed_insert_ephemeral(Rt *r, uintptr_t a, uint64_t obj, uint64_t size);
void reclaimed_remove_if(Rt *r, uintptr_t a, uint64_t obj, bool ephemeral);
bool reclaimed_any_in(Rt *r, uintptr_t lo, uintptr_t hi);
atvec reclaimed_range(Rt *r, uintptr_t lo, uintptr_t hi);
bool reclaimed_heap_empty(Rt *r, size_t h);

/* Page buckets. */
bool quiet_pages(uintptr_t a, size_t n);
CB_ALWAYS bool quiet_cells(uintptr_t a, size_t n)
{
    return atomic_load_explicit(&cb_reclaimed_entries, memory_order_relaxed) == 0 || quiet_pages(a, n);
}
void heap_page_count_multi(size_t h, uintptr_t a, uint64_t size, bool up);
extern void (*MT_heap_page_count)(size_t h, uintptr_t a, uint64_t size, bool up);
CB_ALWAYS void heap_page_count(size_t h, uintptr_t a, uint64_t size, bool up)
{
    if (multi())
    {
        HOOK(MT_heap_page_count)(h, a, size, up);
    }
}
void rebuild_heap_pages(Rt *r);
HeapSet addr_heaps(uintptr_t lo, uintptr_t hi);
void page_count(uintptr_t a, uint64_t size, bool up);

/* Marks (D-0293). */
void ensure_mark(uint64_t id);
static inline void set_mark(uint64_t id, bool on)
{
    size_t h = (size_t)((id >> HEAP_SHIFT) & 0xff);
    size_t i = (size_t)(uint32_t)id - 1;
    uint8_t *p = __atomic_load_n(&cb_marks[h], __ATOMIC_RELAXED);
    __atomic_store_n(&p[i], (uint8_t)on, __ATOMIC_RELAXED);
}

/* Pools. */
/* `take_vec_inl`, `give_vec_inl`: in line, for the cores; `take_vec`,
   `give_vec`: where the compiler judges. */
CB_ALWAYS u64vec take_vec_inl(u64vecvec *pool)
{
    if (pool->n)
    {
        return pool->p[--pool->n];
    }
    u64vec v = {0};
    return v;
}

static inline u64vec take_vec(u64vecvec *pool)
{
    return take_vec_inl(pool);
}

void give_vec_more(u64vecvec *pool, u64vec v);
CB_ALWAYS void give_vec_inl(u64vecvec *pool, u64vec v)
{
    if (LIKELY(v.cap > 0 && pool->n < pool->cap && pool->n < 4096))
    {
        v.n = 0;
        pool->p[pool->n++] = v;
        return;
    }
    give_vec_more(pool, v);
}

static inline void give_vec(u64vecvec *pool, u64vec v)
{
    give_vec_inl(pool, v);
}

/* `x` onto a frame's list, which takes its block from the pool at the
   first one. */
static inline void owned_push(Rt *r, u64vec *owned, uint64_t x)
{
    if (UNLIKELY(owned->cap == 0))
    {
        *owned = take_vec(&r->pool[cur_heap()]);
    }
    u64vec_push_inl(owned, x);
}

static inline void give_proj(projvecvec *pool, projvec v)
{
    if (v.cap > 0 && pool->n < 4096)
    {
        v.n = 0;
        projvecvec_push(pool, v);
    }
    else
    {
        projvec_free(&v);
    }
}

static inline projvec take_proj(projvecvec *pool)
{
    if (pool->n)
    {
        return pool->p[--pool->n];
    }
    projvec v = {0};
    return v;
}

/* ---- waits, the schedule ---- */

uint64_t mutex_key(uintptr_t addr);

/* Report 132: the event counts are kept in stripes, each with its own
   lock, apart from the book: a send or a receive that need not wait takes
   only its stripe's lock, so threads using separate channels do not
   serialise (report 315: a mutex's lock is in its own state cells). The book
   is taken only on the paths that wait or wake a waiter. Lock order: the
   book, then a stripe; never the book while holding a stripe. */
typedef struct
{
    plat_mutex m;
    map_u64 events; /* event count id -> its count */
    map_u64 ewait;  /* event count id -> threads waiting on it */
} Stripe;

Stripe *stripe_of(uint64_t k);
void stripe_unlock(Stripe *s);
/* The event count `id`, if it exists. Takes the stripe. */
bool stripe_event_get(uint64_t id, uint64_t *v);
/* A new event count's id. */
uint64_t stripe_event_new(void);
/* `n` threads more (or fewer) wait on a stripe's key. Under the stripe. */
void stripe_count(map_u64 *m, uint64_t k, int d);
void signal_key(uint64_t key);
typedef bool (*ready_fn)(Book *b, void *ctx);
void wait_until(uint64_t key, Wait kind, ready_fn ready, void *ctx);
const char *stop_reason(Book *b, uint64_t me);
bool thread_running(uint64_t t);
void affine_slow(PathRec *rec, uint64_t owner, uint64_t me, uint32_t file, uint32_t line);
static inline void affine(PathRec *rec, uint32_t file, uint32_t line)
{
    uint64_t owner = atomic_load_explicit(&rec->thread, memory_order_acquire);
    uint64_t m = me();
    if (owner != m)
    {
        affine_slow(rec, owner, m, file, line);
    }
}

static inline bool sched_on(void)
{
    return atomic_load_explicit(&SCHED_ON, memory_order_relaxed);
}
void sched_entry(void);
void rt_sched_yield(void);
bool sched_alone(uint64_t me);
void sched_add(uint64_t t);
void sched_leave(uint64_t me);
void sched_io(uint64_t me, bool on);
void sched_init(void);

/* ---- faults, locations, output ---- */

void get_at(uint32_t *file, uint32_t *line);
void set_at(uint32_t file, uint32_t line);
void fault(const char *id, uint32_t file, uint32_t line) CB_NORET;
void fault_without_unwind(const char *id, uint32_t file, uint32_t line) CB_NORET;
/* `fault_owned`: the text is the runtime's (freed or kept by the fault). */
void fault_owned(char *id, uint32_t file, uint32_t line) CB_NORET;
void flush_stdout(void);
void flush_at_end(void);

/* The generated program's (cbrt.h). */
void cb_where(uint32_t *file, uint32_t *line);
void cb_set_where(uint32_t file, uint32_t line);
void cb_set_stack_floor(uintptr_t floor);
void cbrt_request_stack(size_t bytes);
void jump_plainly(void *jb);
void thread_locals_free(void);

/* D-0299 step 4: a projection's steps as the replay's fields (a field, an
   index or a payload is a field of the mechanised fragment). */
static inline u64vec replay_steps(const CbProj *extra, size_t n)
{
    u64vec v = {0};
    u64vec_reserve(&v, n);
    for (size_t i = 0; i < n; i++)
    {
        v.p[v.n++] = extra[i].kind == 2 ? 0 : extra[i].idx;
    }
    return v;
}

/* Each part's own declarations (prototypes, shared types, globals). */
#include "rt_init.h"
#include "rt_objs.h"
#include "rt_refs.h"
#include "rt_threads.h"

#endif
