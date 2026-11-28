/* CBMC harnesses over the runtime's own data structures (D-0261), each at
   the unwind bound run.sh gives it: ids stable until removed (the arena),
   sets exact (HeapSet), places filled before read (Heaps), the path fold
   streamed as built (Norm / NormIter, short and spilled), overlap symmetric
   and reflexive. Run by run.sh, one `--function` each. */

#include "../../src/rt.h"
#include "../../src/rt_tables.c"

/* Under CBMC the runtime's thread-locals and externals are plain. */
__thread uint32_t LHELD;
__thread HeapSet LSET;
__thread size_t LSOLE;
__thread uint8_t HEAP;
_Atomic bool MULTI;
_Atomic size_t HEAPS_N;
_Atomic size_t cb_reclaimed_entries;
_Atomic uint32_t cb_reclaimed_pages[RECLAIMED_PAGE_BUCKETS];
uint8_t *cb_marks[256];
size_t MARK_LEN[256];

/* What CBMC 5.12 has no model for, given exactly: the bit scans (with no
   body CBMC takes their result as any value), and the runtime's internal
   error, which ends the process (with no body CBMC returns from it and goes
   on in a state the runtime never reaches). Reaching an internal error is a
   failure of the harness. */
int __builtin_ctzll(unsigned long long x)
{
    __CPROVER_assert(x != 0, "ctzll of a nonzero word");
    int n = 0;
    if (!(x & 0xffffffffULL)) { n += 32; x >>= 32; }
    if (!(x & 0xffffULL)) { n += 16; x >>= 16; }
    if (!(x & 0xffULL)) { n += 8; x >>= 8; }
    if (!(x & 0xfULL)) { n += 4; x >>= 4; }
    if (!(x & 0x3ULL)) { n += 2; x >>= 2; }
    if (!(x & 0x1ULL)) { n += 1; }
    return n;
}

int __builtin_clzll(unsigned long long x)
{
    __CPROVER_assert(x != 0, "clzll of a nonzero word");
    int n = 0;
    if (!(x & 0xffffffff00000000ULL)) { n += 32; x <<= 32; }
    if (!(x & 0xffff000000000000ULL)) { n += 16; x <<= 16; }
    if (!(x & 0xff00000000000000ULL)) { n += 8; x <<= 8; }
    if (!(x & 0xf000000000000000ULL)) { n += 4; x <<= 4; }
    if (!(x & 0xc000000000000000ULL)) { n += 2; x <<= 2; }
    if (!(x & 0x8000000000000000ULL)) { n += 1; }
    return n;
}

/* `realloc` as C defines it, in CBMC's terms (5.12's own model loses the old
   contents here: the property fails under CBMC and holds when the harness is
   run as a program, every choice of the symbolic steps, under ASan). */
void *realloc(void *p, size_t n)
{
    void *q = malloc(n);
    if (p)
    {
        size_t old = __CPROVER_OBJECT_SIZE(p);
        if (n >= old)
        {
            __CPROVER_array_copy((char *)q, (char *)p);
        }
        else
        {
            memcpy(q, p, n);
        }
        free(p);
    }
    return q;
}

void cbrt_internal(const char *msg)
{
    (void)msg;
    __CPROVER_assert(0, "no internal error");
    __CPROVER_assume(0);
}

uint8_t nondet_u8(void);
size_t nondet_size(void);

/* An arena id names its entry until the entry is removed; a removed id
   names nothing, and the slot's reuse gets a new generation. */
ARENA(U8Arena, uint8_t)

void arena_ids_are_stable_until_removed(void)
{
    U8Arena a;
    memset(&a, 0, sizeof a);
    a.heap = nondet_u8();
    uint64_t k1 = U8Arena_reserve(&a);
    *U8Arena_insert(&a, k1) = 1;
    uint64_t k2 = U8Arena_reserve(&a);
    *U8Arena_insert(&a, k2) = 2;
    __CPROVER_assert(U8Arena_get(&a, k1) && *U8Arena_get(&a, k1) == 1, "k1 names 1");
    __CPROVER_assert(U8Arena_get(&a, k2) && *U8Arena_get(&a, k2) == 2, "k2 names 2");
    __CPROVER_assert(k1 != k2, "distinct ids");
    uint8_t out = 0;
    __CPROVER_assert(U8Arena_remove(&a, k1, &out) && out == 1, "remove gives 1");
    __CPROVER_assert(U8Arena_get(&a, k1) == NULL, "k1 dead");
    __CPROVER_assert(U8Arena_get(&a, k2) && *U8Arena_get(&a, k2) == 2, "k2 still 2");
    uint64_t k3 = U8Arena_reserve(&a);
    *U8Arena_insert(&a, k3) = 3;
    __CPROVER_assert(k3 != k1, "reuse has a new generation");
    __CPROVER_assert(U8Arena_get(&a, k1) == NULL, "k1 stays dead");
    __CPROVER_assert(U8Arena_get(&a, k3) && *U8Arena_get(&a, k3) == 3, "k3 names 3");
    __CPROVER_assert((size_t)(k3 >> HEAP_SHIFT) == a.heap, "heap_of(k3)");
}

/* A set holds exactly what was added, and each visits it in order. */
void heapset_add_has_each(void)
{
    size_t a = nondet_size(), b = nondet_size();
    __CPROVER_assume(a < MAX_HEAPS && b < MAX_HEAPS);
    HeapSet s = HEAPSET_EMPTY;
    heapset_add(&s, a);
    __CPROVER_assert(heapset_has(&s, a), "has a");
    __CPROVER_assert(heapset_has(&s, b) == (a == b), "has b iff a == b");
    heapset_add(&s, b);
    size_t seen = 0, last = 0;
    HEAPSET_EACH(s, h, {
        __CPROVER_assert(h == a || h == b, "each visits members");
        __CPROVER_assert(seen == 0 || h > last, "ascending");
        last = h;
        seen++;
    });
    __CPROVER_assert(seen == (a == b ? 1u : 2u), "each visits each once");
    HeapSet e = HEAPSET_EMPTY;
    __CPROVER_assert(heapset_covers(&s, &e), "covers empty");
    HeapSet t = HEAPSET_EMPTY;
    heapset_add(&t, a);
    __CPROVER_assert(heapset_covers(&s, &t), "covers {a}");
}

/* The per-heap places: a place is read only once filled, len counts the
   filled ones, nothing beyond them is reachable. The model has 8 heaps. */
typedef struct
{
    uint32_t h[8];
    _Atomic size_t n;
} U32Heaps;

void heaps_places_are_filled_before_read(void)
{
    U32Heaps hs;
    memset(&hs, 0, sizeof hs);
    hs.h[0] = 7;
    size_t k = nondet_size();
    __CPROVER_assume(k <= 3);
    for (size_t i = 0; i < k; i++)
    {
        HEAPS_PUSH(&hs, p, *p = 10 + (uint32_t)i);
    }
    __CPROVER_assert(heaps_len(&hs.n) == k + 1, "len");
    __CPROVER_assert(heaps_one(&hs.n) == (k == 0), "one");
    size_t q = nondet_size();
    __CPROVER_assume(q <= 4);
    if (heaps_has(&hs.n, q))
    {
        if (q == 0)
        {
            __CPROVER_assert(hs.h[0] == 7, "heap 0");
        }
        else
        {
            __CPROVER_assert(q - 1 < k, "filled");
            __CPROVER_assert(hs.h[q] == 10 + (uint32_t)(q - 1), "its value");
        }
    }
    else
    {
        __CPROVER_assert(q > k, "unfilled is beyond");
    }
    size_t n = 0;
    for (size_t i = 0; i < heaps_len(&hs.n); i++)
    {
        __CPROVER_assert(hs.h[i] == (i == 0 ? 7u : 10u + (uint32_t)(i - 1)), "iter");
        n++;
    }
    __CPROVER_assert(n == k + 1, "iter count");
}

/* A symbolic step whose numbers are small. */
static Proj any_proj(void)
{
    uint8_t tag = nondet_u8(), a = nondet_u8(), b = nondet_u8();
    switch (tag % 4)
    {
    case 0:
        return proj_field(a);
    case 1:
        return proj_index(a);
    case 2:
        return proj_payload();
    default:
        return proj_range(a, b);
    }
}

/* `built` is exactly what NormIter streams of p. */
static void assert_streams_as(const Norm *built, const Proj *p, size_t n)
{
    size_t gl;
    const Proj *got = norm_get(built, &gl);
    NormIter it = {p, n, 0};
    Proj x;
    size_t k = 0;
    while (norm_iter_next(&it, &x))
    {
        __CPROVER_assert(k < gl, "no more streamed than built");
        __CPROVER_assert(proj_eq(got[k], x), "same step");
        k++;
    }
    __CPROVER_assert(k == gl, "as many streamed as built");
}

void norm_of_agrees_with_norm_iter(void)
{
    Proj p[3] = {any_proj(), any_proj(), any_proj()};
    __CPROVER_assume(!(p[0].k == P_RANGE && p[1].k == P_RANGE));
    __CPROVER_assume(!(p[1].k == P_RANGE && p[2].k == P_RANGE));
    Norm built;
    norm_of(&built, p, 3);
    assert_streams_as(&built, p, 3);
    size_t gl;
    const Proj *got = norm_get(&built, &gl);
    __CPROVER_assert(gl <= 3, "no longer");
    for (size_t i = 1; i < gl; i++)
    {
        __CPROVER_assert(!(got[i - 1].k == P_RANGE && got[i].k == P_INDEX), "no range before an index");
    }
}

void norm_spill_agrees_with_norm_iter(void)
{
    Proj arr[NORM_INLINE + 2];
    for (size_t i = 0; i < NORM_INLINE + 2; i++)
    {
        arr[i] = proj_payload();
    }
    for (size_t i = 0; i < NORM_INLINE; i++)
    {
        arr[i] = i % 3 == 2 ? proj_range(i, i + 2) : proj_field(i);
    }
    arr[NORM_INLINE] = proj_index(3);
    arr[NORM_INLINE - 2] = any_proj();
    arr[NORM_INLINE + 1] = any_proj();
    Norm built;
    norm_of(&built, arr, NORM_INLINE + 2);
    assert_streams_as(&built, arr, NORM_INLINE + 2);
    size_t gl;
    norm_get(&built, &gl);
    __CPROVER_assert(gl >= NORM_INLINE, "spilled length");
}

void ranged_overlap_is_symmetric_and_reflexive(void)
{
    Proj a[3] = {any_proj(), any_proj(), any_proj()};
    Proj b[2] = {any_proj(), any_proj()};
    Norm na, nb;
    norm_of(&na, a, 3);
    norm_of(&nb, b, 2);
    size_t la, lb;
    const Proj *ga = norm_get(&na, &la);
    const Proj *gb = norm_get(&nb, &lb);
    __CPROVER_assert(ranged_overlap_with(ga, la, b, 2) == ranged_overlap_with(gb, lb, a, 3), "symmetric");
    bool nonempty = true;
    for (int i = 0; i < 3; i++)
    {
        if (a[i].k == P_RANGE && a[i].a >= a[i].b)
        {
            nonempty = false;
        }
    }
    if (nonempty)
    {
        __CPROVER_assert(ranged_overlap_with(ga, la, a, 3), "reflexive");
    }
}
