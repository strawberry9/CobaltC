/* The program's small blocks (`[Allocate]`, `[Deallocate]`): 1 to AL_MAX
   bytes at an alignment `malloc` gives. `[Deallocate]` is given the size
   `[Allocate]` was (its side-condition, spec/21), so a block carries no
   header: its size names its class, 16 bytes apart, and a heap is one list
   of free blocks per class. A block is carved from a slab `malloc` gave, a
   batch at a time in address order, and goes back on its class's list when
   it is freed; a slab is never returned.

   One thread: the lone heap, whose lists `cbrt.h` exports (`cb_bins`) so
   that the generated C pops and pushes in line while `cb_alloc_gate` is 0.
   Threads: each its own heap, found through one thread-local pointer, here;
   the first thread to ask keeps the lone heap, a heap whose thread ended
   goes whole to the next thread, and a list longer than its limit moves to
   a central list, which an empty list is filled from before a slab is
   carved (one thread freeing what another allocates stays bounded).

   Under a sanitizer every block is `malloc`'s, so that the sanitizer sees
   each one: when the runtime itself is built under one (AL_SYSTEM), and
   when the program is (its C says so before `main`, `cbrt.h`: the gate's
   AL_GATE_SYSTEM). `cb_alloc_gate` is then never 0.
   Part of lib.c's translation unit. */

#include "rt.h"

#define AL_MAX 1024u
#define AL_CLASSES 65u /* a block of n bytes: class (n + 15) >> 4, 1..64 */
#define AL_SLAB ((size_t)64 * 1024)

#if defined(__SANITIZE_ADDRESS__) || defined(__SANITIZE_THREAD__) || defined(CBRT_SYSTEM_ALLOC)
#define AL_SYSTEM 1
#elif defined(__has_feature)
#if __has_feature(address_sanitizer) || __has_feature(thread_sanitizer) || __has_feature(memory_sanitizer)
#define AL_SYSTEM 1
#endif
#endif
#ifndef AL_SYSTEM
#define AL_SYSTEM 0
#endif

/* Why the generated C's inline paths are off (`cb_alloc_gate`). */
#define AL_GATE_SLOW 1u    /* a thread was spawned, or the trace records allocations */
#define AL_GATE_ALIGNED 2u /* an over-aligned block was made: a free asks the map */
#define AL_GATE_SYSTEM 8u  /* every block is malloc's */

void *cb_bins[AL_CLASSES];
uint8_t cb_alloc_gate = AL_SYSTEM ? AL_GATE_SYSTEM : 0;

typedef struct AlHeap
{
    void **bin;               /* the lists, by class */
    uint8_t *cur;             /* the slab being carved */
    uint8_t *end;
    struct AlHeap *next;      /* among the heaps whose thread ended */
    uint32_t cnt[AL_CLASSES]; /* with threads: blocks freed into each list */
    void *own[AL_CLASSES];    /* the lists of a heap that is not the lone one */
} AlHeap;

static AlHeap AL_HEAP0 = {.bin = cb_bins};
static __thread AlHeap *AL_MINE;
/* More than one thread may allocate: set before a second one can (a spawn;
   a library, whose entry points any thread may call, from its start). */
static _Atomic bool AL_THREADS;
static _Atomic bool AL_HEAP0_TAKEN;
static plat_mutex AL_LOCK = PLAT_MUTEX_INIT;
static AlHeap *AL_ENDED;            /* under AL_LOCK */
static void *AL_CENTRAL[AL_CLASSES]; /* written under AL_LOCK; read without it as a hint */

/* Whether every block is `malloc`'s. */
CB_ALWAYS bool al_system(void)
{
    return AL_SYSTEM || (__atomic_load_n(&cb_alloc_gate, __ATOMIC_RELAXED) & AL_GATE_SYSTEM) != 0;
}

void alloc_gate(uint8_t why)
{
    __atomic_or_fetch(&cb_alloc_gate, why, __ATOMIC_RELAXED);
}

/* From here on each thread allocates from its own heap, out of line. Called
   by the one thread there is, before another exists. */
void alloc_threads_on(void)
{
    if (!atomic_load_explicit(&AL_THREADS, memory_order_relaxed))
    {
        atomic_store_explicit(&AL_THREADS, true, memory_order_release);
        alloc_gate(AL_GATE_SLOW);
    }
}

static inline uint32_t al_limit(unsigned c)
{
    uint32_t n = 32768u / ((uint32_t)c << 4);
    return n < 8 ? 8 : n;
}

CB_COLD static AlHeap *al_attach(void)
{
    AlHeap *h = NULL;
    if (!atomic_exchange_explicit(&AL_HEAP0_TAKEN, true, memory_order_acq_rel))
    {
        h = &AL_HEAP0;
    }
    else
    {
        plat_mutex_lock(&AL_LOCK);
        h = AL_ENDED;
        if (h)
        {
            AL_ENDED = h->next;
        }
        plat_mutex_unlock(&AL_LOCK);
        if (!h)
        {
            h = (AlHeap *)cb_xcalloc(1, sizeof(AlHeap));
            h->bin = h->own;
        }
    }
    h->next = NULL;
    AL_MINE = h;
    return h;
}

/* A heap whose thread ended: with its free blocks, it waits for the next
   thread. */
void alloc_heap_end(void *heap)
{
    AlHeap *h = (AlHeap *)heap;
    if (!h || h == &AL_HEAP0)
    {
        return;
    }
    plat_mutex_lock(&AL_LOCK);
    h->next = AL_ENDED;
    AL_ENDED = h;
    plat_mutex_unlock(&AL_LOCK);
}

/* This OS thread ends. */
void alloc_thread_end(void)
{
    AlHeap *h = AL_MINE;
    AL_MINE = NULL;
    alloc_heap_end(h);
}

/* This thread's heap, made now: for a thread whose end is told through
   `alloc_heap_end`, when its thread-locals may be gone. */
void *alloc_heap_mine(void)
{
    AlHeap *h = AL_MINE;
    return h ? h : al_attach();
}

CB_ALWAYS bool al_threads(void)
{
    return atomic_load_explicit(&AL_THREADS, memory_order_relaxed);
}

CB_ALWAYS AlHeap *al_heap(bool threads)
{
    if (LIKELY(!threads))
    {
        return &AL_HEAP0;
    }
    AlHeap *h = AL_MINE;
    return LIKELY(h != NULL) ? h : al_attach();
}

/* A block of class `c` when its list is empty: the central list's blocks,
   with threads; else a batch carved from the slab, in address order, the
   first returned and the rest on the list. NULL: no memory. */
static __attribute__((noinline)) void *al_refill(AlHeap *h, unsigned c, bool threads)
{
    if (threads && __atomic_load_n(&AL_CENTRAL[c], __ATOMIC_RELAXED) != NULL)
    {
        plat_mutex_lock(&AL_LOCK);
        void *p = AL_CENTRAL[c];
        AL_CENTRAL[c] = NULL;
        plat_mutex_unlock(&AL_LOCK);
        if (p)
        {
            h->bin[c] = *(void **)p;
            return p;
        }
    }
    size_t sz = (size_t)c << 4;
    size_t have = (size_t)(h->end - h->cur);
    if (have < sz)
    {
        /* What is left of the slab is one block of a smaller class. */
        if (have)
        {
            void **b = &h->bin[have >> 4];
            *(void **)h->cur = *b;
            *b = h->cur;
        }
        uint8_t *s = (uint8_t *)malloc(AL_SLAB);
        if (!s)
        {
            h->cur = NULL;
            h->end = NULL;
            return NULL;
        }
        h->cur = s;
        h->end = s + AL_SLAB;
        have = AL_SLAB;
    }
    size_t n = 4096 / sz;
    if (n > 32)
    {
        n = 32;
    }
    if (n > have / sz)
    {
        n = have / sz;
    }
    uint8_t *first = h->cur;
    h->cur += n * sz;
    void *head = NULL;
    for (size_t i = n - 1; i >= 1; i--)
    {
        uint8_t *b = first + i * sz;
        *(void **)b = head;
        head = b;
    }
    h->bin[c] = head;
    return first;
}

/* A list past its limit, with threads: all of it to the central list. */
static __attribute__((noinline)) void al_spill(AlHeap *h, unsigned c)
{
    void *head = h->bin[c];
    void *tail = head;
    while (*(void **)tail)
    {
        tail = *(void **)tail;
    }
    h->bin[c] = NULL;
    h->cnt[c] = 0;
    plat_mutex_lock(&AL_LOCK);
    *(void **)tail = AL_CENTRAL[c];
    __atomic_store_n(&AL_CENTRAL[c], head, __ATOMIC_RELAXED);
    plat_mutex_unlock(&AL_LOCK);
}

/* A block of `size` bytes, 1..AL_MAX; NULL: no memory. */
static inline void *al_alloc(size_t size)
{
    if (UNLIKELY(al_system()))
    {
        return malloc(size);
    }
    unsigned c = (unsigned)((size + 15) >> 4);
    bool threads = al_threads();
    AlHeap *h = al_heap(threads);
    void *p = h->bin[c];
    if (LIKELY(p != NULL))
    {
        h->bin[c] = *(void **)p;
        if (threads && h->cnt[c])
        {
            h->cnt[c]--;
        }
        return p;
    }
    return al_refill(h, c, threads);
}

/* The block of `size` bytes, 1..AL_MAX, at `p` is free. */
static inline void al_free(void *p, size_t size)
{
    if (UNLIKELY(al_system()))
    {
        free(p);
        return;
    }
    unsigned c = (unsigned)((size + 15) >> 4);
    bool threads = al_threads();
    AlHeap *h = al_heap(threads);
    *(void **)p = h->bin[c];
    h->bin[c] = p;
    if (threads && ++h->cnt[c] > al_limit(c))
    {
        al_spill(h, c);
    }
}

/* Whether blocks of `a` and of `b` bytes are of one class (a block of one
   size serves as the other). */
static inline bool al_same(size_t a, size_t b)
{
    return !al_system() && a <= AL_MAX && b <= AL_MAX && ((a + 15) >> 4) == ((b + 15) >> 4);
}

/* A block of `size` bytes, > 0, at an alignment `malloc` gives. */
static inline void *block_alloc(size_t size)
{
    return size <= AL_MAX ? al_alloc(size) : malloc(size);
}

/* The same handed back, given its size. */
static inline void block_free(void *p, size_t size)
{
    if (size <= AL_MAX)
    {
        al_free(p, size);
    }
    else
    {
        free(p);
    }
}

/* `realloc` for such a block, from `old` to `size` bytes, both > 0: NULL
   and the old block untouched when there is no memory. */
static inline void *block_realloc(void *p, size_t old, size_t size)
{
    if (al_system() || (old > AL_MAX && size > AL_MAX))
    {
        return realloc(p, size);
    }
    if (al_same(old, size))
    {
        return p;
    }
    void *q = block_alloc(size);
    if (q)
    {
        memcpy(q, p, old < size ? old : size);
        block_free(p, old);
    }
    return q;
}
