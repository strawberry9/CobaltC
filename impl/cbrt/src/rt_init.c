/* The program's start and end, faults, library mode, the standard
   library's externs and the schedule: `cb_init` and the type table it
   builds; a fault's report, unwind and containment (`[Fault-Unwind]`,
   `[Fault-Contain]`, D-0204); `cb_exit` and `cb_terminate_ok`; D-0257's
   library entry points; the arguments, standard streams, number text,
   `$fmt` (D-0297), files, clocks, processes and sockets; and the schedules
   of D-0204 (`COBALTC_SCHEDULE_SEED`). Part of lib.c's translation unit. */

#include <setjmp.h>
#include <stdarg.h>
#include <stdio.h>

#include "rt.h"
#include "diagnostics.h"
#include "fmt.h"
#include "io.h"

/* ---- the stamp ----

   This runtime's sources, hashed: `cobc` finds the marker in `libcbrt.a`
   and compares it with the stamp it was built with. CBRT_STAMP is given by
   the build (see build.sh): tools/stamp.c's hash of the files SOURCES
   lists. */
#ifndef CBRT_STAMP
#define CBRT_STAMP "0000000000000000"
#endif
__attribute__((used)) const char CB_RUNTIME_STAMP[27] = "CBRT-STAMP:" CBRT_STAMP;

/* ---- statics ---- */

Rt RT;
bool RT_INIT;

/* D-0292: `COBALTC_RECORD=full` makes every root a recorded path (the
   oracle light tokens are compared with); `=check` aborts where a light
   reference reaches what the lowering should have recorded. D-0315: the
   program was lowered decided. */
_Atomic bool RECORD_FULL;
_Atomic bool RECORD_CHECK;
_Atomic bool DECIDED_PROGRAM;
/* D-0292: `COBALTC_RT_STATS` counters of the phase-3 runtime. */
_Atomic bool PH3_ON;
_Atomic uint64_t N_ROOT_LIGHT, N_ROOT_REC, N_PATHS, N_SLOW, N_PROJ_ADDR, N_TOK0_OWNER, N_LIGHT, N_HOLDERS, N_ARRIVALS;

/* D-0293: the marks `cbrt.h` tests inline, per heap. */
uint8_t *cb_marks[256];
uint8_t cb_inline_off;
size_t MARK_LEN[256];

/* How many entries `Reclaimed` holds (THREADS_ENTRIES while threads run),
   and the same by page: the generated code's gates. */
_Atomic size_t cb_reclaimed_entries;
_Atomic uint32_t cb_reclaimed_pages[RECLAIMED_PAGE_BUCKETS];

/* D-0249: 1 under a schedule: the compiled atomic operations call
   `cb_atomic_point`. */
_Atomic uint8_t cb_sched_flag;
_Atomic bool SCHED_ON;

/* How many threads are in a call outside the runtime (`outside`), and how
   many cancellations were ever asked (`cancel_probe` needs no lock while
   there are none). */
_Atomic size_t IN_IO;
_Atomic size_t CANCELS;

__thread bool SCHED_PENDING;

/* D-0315: the program is decided (cbrt.h, `CB_DECIDED`). */
void cb_set_decided(void)
{
    atomic_store_explicit(&DECIDED_PROGRAM, true, memory_order_relaxed);
}

/* ---- internal errors ---- */

void cbrt_internal(const char *msg)
{
    outbuf_flush();
    char line[512];
    int k = snprintf(line, sizeof line, "cbrt: internal: %s\n", msg);
    if (k > 0)
    {
        outbuf_write_err((const uint8_t *)line, (size_t)k < sizeof line ? (size_t)k : sizeof line - 1);
    }
    plat_exit(4);
}

void cbrt_internalf(const char *fmt, ...)
{
    char msg[400];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof msg, fmt, ap);
    va_end(ap);
    cbrt_internal(msg);
}

void cbrt_oom(size_t n)
{
    cbrt_internalf("memory allocation of %llu bytes failed", (unsigned long long)n);
}

/* ---- heaps for threads (D-0202) ---- */

/* The calling thread's heap (`cur_heap`): its new objects, slots and
   reclaimed entries are recorded there. */
void set_heap(Rt *r, uint8_t h)
{
    (void)r;
    HEAP = h;
}

/* A heap for a new thread: one no running thread uses and that holds
   nothing, else a new one; when MAX_HEAPS are in use, the one with fewest
   users but the main thread's, shared from then on. */
uint8_t claim_heap(Rt *r)
{
    size_t n = r->heap_users.n;
    size_t h = NO_IDX;
    for (size_t i = 1; i < n; i++)
    {
        if (r->heap_users.p[i] == 0 && objs_heap_empty(r, i) && paths_heap_empty(r, i) && slots_heap_empty(r, i) && reclaimed_heap_empty(r, i))
        {
            h = i;
            break;
        }
    }
    if (h == NO_IDX)
    {
        if (n < MAX_HEAPS)
        {
            u64vec_push(&r->heap_users, 0);
            atomic_store_explicit(&HEAPS_N, r->heap_users.n, memory_order_release);
            h = r->heap_users.n - 1;
        }
        else
        {
            /* `min_by_key`: the first of the least used. */
            h = 0;
            uint64_t best = UINT64_MAX;
            for (size_t i = 1; i < n; i++)
            {
                if (r->heap_users.p[i] < best)
                {
                    best = r->heap_users.p[i];
                    h = i;
                }
            }
            atomic_store_explicit(&HEAP_SHARED[h], true, memory_order_release);
        }
    }
    r->heap_users.p[h]++;
    /* `ensure`: heap h's places exist in every table (each made in order). */
    size_t need = h + 1;
    while (heaps_len(&r->objs.n) < need)
    {
        HEAPS_PUSH(&r->objs, a, a->heap = (uint8_t)k_);
    }
    while (heaps_len(&r->paths.n) < need)
    {
        HEAPS_PUSH(&r->paths, a, a->heap = (uint8_t)k_);
    }
    while (heaps_len(&r->slots.n) < need)
    {
        HEAPS_PUSH(&r->slots, m, {
            m->heap = (uint8_t)k_;
            slotmap_last_none(m);
        });
    }
    while (heaps_len(&r->reclaimed.n) < need)
    {
        HEAPS_PUSH(&r->reclaimed, m, m->heap = (uint8_t)k_);
    }
    /* pool, proj_pool, slot_buf: fixed arrays of MAX_HEAPS, zero = empty. */
    return (uint8_t)h;
}

/* ---- locations, output ---- */

/* The executing statement's location (`cb_at`, a C thread-local in the
   generated program). */
void get_at(uint32_t *file, uint32_t *line)
{
    uint32_t f = NOLOC, l = 0;
    cb_where(&f, &l);
    *file = f;
    *line = l;
}

void set_at(uint32_t file, uint32_t line)
{
    cb_set_where(file, line);
}

/* D-0178: standard output's buffer, written out before a fault's report, a
   read of standard input and the program's end. */
void flush_stdout(void)
{
    outbuf_flush();
}

/* The program's end, however it ends: standard output's buffer (D-0178)
   and every file's (D-0201) written out. */
void flush_at_end(void)
{
    replay_finish();
    outbuf_flush();
    fileio_flush_all();
}

/* Standard error, unbuffered. `outbuf_write_err` writes standard output's
   buffer out first; every report here follows a `flush_stdout`, so it
   finds that buffer empty. */
static void eprint(const char *s)
{
    (void)outbuf_write_err((const uint8_t *)s, strlen(s));
}

/* ---- the program's start ---- */

/* glibc's heap as a fresh process's: the chunks carved from its top at
   addresses 16 past a multiple of 32, so a tree's 32-byte nodes do not
   cross cache lines. Where the top is off, one block of 4104 bytes carved
   from it moves it back, and is kept; a block that did not come from the
   top is freed again. */
#if defined(__linux__) && defined(__GLIBC__)
#include <malloc.h>
#include <unistd.h>
static void align_heap(void)
{
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
    uintptr_t before = (uintptr_t)sbrk(0) - (uintptr_t)mallinfo().keepcost;
    if (before % 32 != 0)
    {
        return;
    }
    void *volatile p = malloc(4104);
    uintptr_t after = (uintptr_t)sbrk(0) - (uintptr_t)mallinfo().keepcost;
#pragma GCC diagnostic pop
    if (p && after == before)
    {
        free(p);
    }
}
#else
static void align_heap(void)
{
}
#endif

static char *dup_cstr_lossy(const char *s)
{
    return diag_lossy((const uint8_t *)s, strlen(s));
}

static bool type_holds_fn(TypeInfo *ts, TypeMap *m, uint32_t id)
{
    bool f;
    size_t i = typemap_get(m, id, &f);
    return f && ts[i].holds_fn;
}

/* The fixed reference offsets, as `ref_offsets` walks a value (a type
   nested too deep for it is left to the walk): false for None. */
static bool fixed(TypeInfo *ts, TypeMap *m, size_t i, uint32_t depth, u64vec *out)
{
    TypeInfo *t = &ts[i];
    if (!t->has_refs)
    {
        return true;
    }
    if (depth > 32)
    {
        return false;
    }
    switch (t->kind)
    {
    case K_REF:
    case K_GUARD:
        u64vec_push(out, 0);
        return true;
    case K_STRUCT:
        for (uint32_t k = 0; k < t->nfields; k++)
        {
            bool f;
            size_t fi = typemap_get(m, t->fields[k].ty, &f);
            if (!f)
            {
                return false;
            }
            u64vec inner = {0};
            if (!fixed(ts, m, fi, depth + 1, &inner))
            {
                u64vec_free(&inner);
                return false;
            }
            for (uint32_t j = 0; j < inner.n; j++)
            {
                u64vec_push(out, inner.p[j] + t->fields[k].off);
            }
            u64vec_free(&inner);
        }
        return true;
    case K_ARRAY:
    {
        bool f;
        size_t e = typemap_get(m, t->elem, &f);
        if (!f)
        {
            return false;
        }
        u64vec inner = {0};
        if (!fixed(ts, m, e, depth + 1, &inner))
        {
            u64vec_free(&inner);
            return false;
        }
        uint64_t esize = ts[e].size;
        for (uint64_t k = 0; k < t->count; k++)
        {
            for (uint32_t j = 0; j < inner.n; j++)
            {
                u64vec_push(out, inner.p[j] + k * esize);
            }
        }
        u64vec_free(&inner);
        return true;
    }
    case K_ENUM:
        return false;
    default:
        return true;
    }
}

typedef struct
{
    fieldref *p;
    uint32_t n;
    uint32_t cap;
} frvec;

static void frvec_push(frvec *v, uint64_t off, uint32_t ty)
{
    if (v->n == v->cap)
    {
        v->cap = v->cap ? v->cap * 2 : 4;
        v->p = (fieldref *)cb_xrealloc(v->p, v->cap * sizeof(fieldref));
    }
    v->p[v->n].off = off;
    v->p[v->n].ty = ty;
    v->n++;
}

/* Each `tree` type's Box pointers (`box_plan`): false for None. */
static bool local_plan(TypeInfo *ts, size_t i, uint32_t depth, frvec *v)
{
    TypeInfo *t = &ts[i];
    if (!t->is_resource && !t->holds_fn)
    {
        return true;
    }
    if (depth > 8 || !t->tree)
    {
        return false;
    }
    if (t->has_box_of)
    {
        if (t->ntree_parts == 0)
        {
            return false;
        }
        uint32_t inner = t->tree_parts[0].ty;
        if (inner == NOTYPE || ts[inner].size == 0)
        {
            return false;
        }
        frvec_push(v, 0, inner);
        return true;
    }
    switch (t->kind)
    {
    case K_STRUCT:
        for (uint32_t k = 0; k < t->ntree_parts; k++)
        {
            frvec sub = {0};
            if (!local_plan(ts, t->tree_parts[k].ty, depth + 1, &sub))
            {
                free(sub.p);
                return false;
            }
            for (uint32_t j = 0; j < sub.n; j++)
            {
                frvec_push(v, t->tree_parts[k].off + sub.p[j].off, sub.p[j].ty);
            }
            free(sub.p);
        }
        return true;
    case K_ENUM:
    {
        /* Laid out as its one Box (D-0298): a null pointer is the other
           variant, which holds nothing. */
        if (!t->has_niche)
        {
            return false;
        }
        size_t some = t->niche_some, none = t->niche_none;
        if (t->payload_off != 0 || t->ntree_parts != 2 || none >= t->ntree_parts || t->tree_parts[none].ty != NOTYPE)
        {
            return false;
        }
        if (some >= t->ntree_parts)
        {
            return false;
        }
        uint32_t vv = t->tree_parts[some].ty;
        if (vv == NOTYPE || !ts[vv].has_box_of)
        {
            return false;
        }
        return local_plan(ts, vv, depth + 1, v);
    }
    case K_ARRAY:
    {
        if (t->ntree_parts == 0)
        {
            return false;
        }
        uint32_t e = t->tree_parts[0].ty;
        frvec ep = {0};
        if (!local_plan(ts, e, depth + 1, &ep))
        {
            free(ep.p);
            return false;
        }
        if (ep.n == 0)
        {
            free(ep.p);
            return true;
        }
        if (t->count > 64)
        {
            free(ep.p);
            return false;
        }
        uint64_t es = ts[e].size;
        for (uint64_t k = 0; k < t->count; k++)
        {
            for (uint32_t j = 0; j < ep.n; j++)
            {
                frvec_push(v, k * es + ep.p[j].off, ep.p[j].ty);
            }
        }
        free(ep.p);
        return true;
    }
    default:
        return false;
    }
}

void cb_init(const char *const *files, size_t nfiles, const CbType *types, size_t ntypes, const uint32_t *ids)
{
    const char *record = plat_getenv("COBALTC_RECORD");
    atomic_store_explicit(&RECORD_FULL, record && strcmp(record, "full") == 0, memory_order_relaxed);
    atomic_store_explicit(&RECORD_CHECK, record && strcmp(record, "check") == 0, memory_order_relaxed);
    /* D-0293: under either oracle every entry is the runtime's. */
    cb_inline_off = record != NULL;
    atomic_store_explicit(&PH3_ON, plat_getenv("COBALTC_RT_STATS") != NULL, memory_order_relaxed);
    netio_set_cancel_probe(cancel_probe);
    netio_wake_init();
    sched_init();
    set_stack_floor();
    outbuf_prepare();
    cb_alloc_traced = replay_on() ? 1 : 0;
    if (cb_alloc_traced)
    {
        alloc_gate(AL_GATE_SLOW);
    }
    Rt *r = rt();
    /* The type map: open addressing, a power-of-two size, the empty slot
       NOTYPE (never a valid id). */
    size_t cap = 16;
    while (cap < ntypes * 2)
    {
        cap *= 2;
    }
    TypeMap *m = &r->type_map;
    m->ids = (uint32_t *)cb_xmalloc(cap * sizeof(uint32_t));
    m->pos = (uint32_t *)cb_xcalloc(cap, sizeof(uint32_t));
    for (size_t i = 0; i < cap; i++)
    {
        m->ids[i] = NOTYPE;
    }
    m->mask = cap - 1;
    for (size_t i = 0; i < ntypes; i++)
    {
        uint32_t id = ids[i];
        size_t at = (size_t)(id * 0x9E3779B1u) & m->mask;
        for (;;)
        {
            if (m->ids[at] == NOTYPE)
            {
                m->ids[at] = id;
                m->pos[at] = (uint32_t)i;
                break;
            }
            if (m->ids[at] == id)
            {
                cbrt_internalf("two type descriptors share the id %u", id);
            }
            at = (at + 1) & m->mask;
        }
    }
    r->files = (char **)cb_xcalloc(nfiles, sizeof(char *));
    r->nfiles = nfiles;
    for (size_t i = 0; i < nfiles; i++)
    {
        r->files[i] = dup_cstr_lossy(files[i]);
    }
    TypeInfo *ts = (TypeInfo *)cb_xcalloc(ntypes, sizeof(TypeInfo));
    for (size_t i = 0; i < ntypes; i++)
    {
        const CbType *t = &types[i];
        TypeInfo *x = &ts[i];
        x->name = t->name ? dup_cstr_lossy(t->name) : dup_cstr_lossy("");
        x->size = t->size;
        x->kind = t->kind;
        x->is_resource = t->is_resource != 0;
        x->has_refs = t->has_refs != 0;
        x->drop = t->drop;
        x->nfields = t->nfields;
        x->fields = (fieldref *)cb_xcalloc(t->nfields, sizeof(fieldref));
        for (uint32_t k = 0; k < t->nfields; k++)
        {
            x->fields[k].off = t->fields[k].off;
            x->fields[k].ty = t->fields[k].ty;
        }
        x->nvariants = t->nvariants;
        x->variants = (uint32_t *)cb_xcalloc(t->nvariants, sizeof(uint32_t));
        for (uint32_t k = 0; k < t->nvariants; k++)
        {
            x->variants[k] = t->variants[k];
        }
        x->payload_off = t->payload_off;
        x->elem = t->elem;
        x->count = t->count;
        x->owner = t->owner != 0;
        x->holds_fn = t->kind == K_FN;
        x->has_box_of = t->kind == K_STRUCT && t->elem != 0;
        x->box_of = x->has_box_of ? t->elem - 1 : 0;
        x->has_vec_plain = t->kind == K_STRUCT && t->count != 0 && t->nfields == 3;
        x->vec_plain = x->has_vec_plain ? t->count - 1 : 0;
        x->has_niche = t->kind == K_ENUM && t->elem != 0;
        if (x->has_niche)
        {
            x->niche_some = (size_t)t->elem - 1;
            x->niche_none = t->count > 0 ? (size_t)t->count - 1 : 0;
        }
    }
    /* D-0089: `holds_fn` through fields, elements and payloads. */
    for (;;)
    {
        bool changed = false;
        for (size_t i = 0; i < ntypes; i++)
        {
            TypeInfo *t = &ts[i];
            if (t->holds_fn)
            {
                continue;
            }
            bool inner = false;
            for (uint32_t k = 0; k < t->nfields && !inner; k++)
            {
                inner = type_holds_fn(ts, m, t->fields[k].ty);
            }
            for (uint32_t k = 0; k < t->nvariants && !inner; k++)
            {
                inner = t->variants[k] != NOTYPE && type_holds_fn(ts, m, t->variants[k]);
            }
            if (!inner && t->kind == K_ARRAY)
            {
                inner = type_holds_fn(ts, m, t->elem);
            }
            if (inner)
            {
                t->holds_fn = true;
                changed = true;
            }
        }
        if (!changed)
        {
            break;
        }
    }
    for (size_t i = 0; i < ntypes; i++)
    {
        u64vec f = {0};
        if (fixed(ts, m, i, 0, &f))
        {
            ts[i].has_fixed_refs = true;
            ts[i].fixed_refs = f.p;
            ts[i].nfixed_refs = f.n;
        }
        else
        {
            u64vec_free(&f);
        }
    }
    for (size_t i = 0; i < ntypes; i++)
    {
        TypeInfo *t = &ts[i];
        if (t->kind != K_STRUCT)
        {
            continue;
        }
        frvec v = {0};
        for (uint32_t k = 0; k < t->nfields; k++)
        {
            bool f;
            size_t fi = typemap_get(m, t->fields[k].ty, &f);
            if (f && (ts[fi].is_resource || ts[fi].holds_fn))
            {
                frvec_push(&v, t->fields[k].off, t->fields[k].ty);
            }
        }
        t->owning = v.p;
        t->nowning = v.n;
    }
    /* `tree`: the greatest set closed under its parts (a Box's referent may
       be the type itself), every type that may be one first, then each with
       a part outside struck out until none changes. */
    for (size_t i = 0; i < ntypes; i++)
    {
        TypeInfo *t = &ts[i];
        bool ok = true;
        frvec parts = {0};
        bool f;
        if (!t->is_resource && !t->holds_fn)
        {
            ok = true;
        }
        else if (t->has_box_of)
        {
            size_t p = typemap_get(m, t->box_of, &f);
            if (f)
            {
                frvec_push(&parts, 0, (uint32_t)p);
            }
            else
            {
                ok = false;
            }
        }
        else if (t->drop || t->has_vec_plain)
        {
            ok = false;
        }
        else
        {
            switch (t->kind)
            {
            case K_STRUCT:
                for (uint32_t k = 0; k < t->nowning && ok; k++)
                {
                    size_t p = typemap_get(m, t->owning[k].ty, &f);
                    if (f)
                    {
                        frvec_push(&parts, t->owning[k].off, (uint32_t)p);
                    }
                    else
                    {
                        ok = false;
                    }
                }
                break;
            case K_ENUM:
                for (uint32_t k = 0; k < t->nvariants && ok; k++)
                {
                    if (t->variants[k] == NOTYPE)
                    {
                        frvec_push(&parts, 0, NOTYPE);
                        continue;
                    }
                    size_t p = typemap_get(m, t->variants[k], &f);
                    if (f)
                    {
                        frvec_push(&parts, 0, (uint32_t)p);
                    }
                    else
                    {
                        ok = false;
                    }
                }
                break;
            case K_ARRAY:
            {
                size_t p = typemap_get(m, t->elem, &f);
                if (f)
                {
                    frvec_push(&parts, 0, (uint32_t)p);
                }
                else
                {
                    ok = false;
                }
                break;
            }
            default:
                ok = false;
                break;
            }
        }
        if (ok)
        {
            t->tree = true;
            t->tree_parts = parts.p;
            t->ntree_parts = parts.n;
        }
        else
        {
            free(parts.p);
        }
    }
    for (;;)
    {
        bool changed = false;
        for (size_t i = 0; i < ntypes; i++)
        {
            TypeInfo *t = &ts[i];
            if (!t->tree)
            {
                continue;
            }
            bool out = false;
            for (uint32_t k = 0; k < t->ntree_parts; k++)
            {
                uint32_t p = t->tree_parts[k].ty;
                if (p != NOTYPE && !ts[p].tree)
                {
                    out = true;
                    break;
                }
            }
            if (out)
            {
                t->tree = false;
                free(t->tree_parts);
                t->tree_parts = NULL;
                t->ntree_parts = 0;
                changed = true;
            }
        }
        if (!changed)
        {
            break;
        }
    }
    for (size_t i = 0; i < ntypes; i++)
    {
        if (ts[i].tree && ts[i].is_resource)
        {
            frvec v = {0};
            if (local_plan(ts, i, 0, &v))
            {
                ts[i].has_box_plan = true;
                ts[i].box_plan = v.p;
                ts[i].nbox_plan = v.n;
            }
            else
            {
                free(v.p);
            }
        }
    }
    for (;;)
    {
        bool changed = false;
        for (size_t i = 0; i < ntypes; i++)
        {
            TypeInfo *t = &ts[i];
            if (!t->has_box_plan)
            {
                continue;
            }
            bool strike = false;
            for (uint32_t k = 0; k < t->nbox_plan; k++)
            {
                TypeInfo *q = &ts[t->box_plan[k].ty];
                if (!q->has_box_plan && (q->is_resource || q->holds_fn))
                {
                    strike = true;
                    break;
                }
            }
            if (strike)
            {
                t->has_box_plan = false;
                free(t->box_plan);
                t->box_plan = NULL;
                t->nbox_plan = 0;
                changed = true;
            }
        }
        if (!changed)
        {
            break;
        }
    }
    r->types = ts;
    r->ntypes = ntypes;
    r->objs.h[0].heap = 0;
    r->paths.h[0].heap = 0;
    r->slots.h[0].heap = 0;
    slotmap_last_none(&r->slots.h[0]);
    r->reclaimed.h[0].heap = 0;
    r->unwinding = false;
    r->fault_report = NULL;
    u64vec_push(&r->heap_users, 1);
    RT_INIT = true;
    align_heap();
}

/* ---- faults ---- */

/* The diagnostic's name is borrowed from the C string (a literal in the
   generated C), not copied: a contained fault leaves this frame by a jump
   that drops nothing (CHG-0338). */
void cb_fault(const char *diag, uint32_t file, uint32_t line)
{
    (void)enter();
    const char *id = diag_utf8_ok((const uint8_t *)diag, strlen(diag)) ? diag : "diag.internal-error";
    fault(id, file, line);
}

/* A fault carrying the program's message (`assert`, D-0065). */
void cb_fault_msg(const char *diag, const uint8_t *msg, uint64_t n, uint32_t file, uint32_t line)
{
    (void)enter();
    const char *id = diag_utf8_ok((const uint8_t *)diag, strlen(diag)) ? diag : "diag.internal-error";
    cb_buf b = {0};
    cb_buf_str(&b, id);
    cb_buf_byte(&b, DETAIL_SEP);
    if (n)
    {
        char *m = diag_lossy(msg, (size_t)n);
        cb_buf_str(&b, m);
        free(m);
    }
    cb_buf_byte(&b, 0);
    fault_owned((char *)b.p, file, line);
}

/* D-0264: whether `cb_exit` wrote the `EV exit` event (then no `EV end`). */
static _Atomic bool EXIT_EVENT;

/* `std::exit(status)` (D-0150, `[Terminate-Exit]`): the calling thread's
   scopes and frames are folded exactly as a fault folds them, every
   destructor running, and the program ends with `status`, reporting
   nothing. */
void cb_exit(uint8_t status)
{
    replay_out(me(), "the program exits");
    replay_finish();
    (void)enter();
    Rt *r = rt();
    flush_stdout();
    if (trace_events_on() && !r->unwinding)
    {
        char line[32];
        snprintf(line, sizeof line, "EV exit %u\n", (unsigned)status);
        eprint(line);
        atomic_store_explicit(&EXIT_EVENT, true, memory_order_relaxed);
    }
    if (r->unwinding)
    {
        /* `exit` from a destructor already unwinding: the outcome is the
           ending in progress. */
        if (r->fault_report)
        {
            char *rep = r->fault_report;
            r->fault_report = NULL;
            char *s = diag_for_stderr(rep);
            eprint(s);
            plat_exit(1);
        }
        plat_exit(status);
    }
    r->unwinding = true;
    fold_frames_and_notes();
    flush_at_end();
    plat_exit(outbuf_end_status(status));
}

/* D-0257: the `_try` entry stub's jump buffer while one is active (0:
   none), and the report of the last fault this thread's `_try` contained. */
static __thread uintptr_t LIB_JMP;
static __thread char *LIB_FAULT;

/* A fault's report, as the program prints it: located by its line
   (D-0228). The text is "id" or "id\x01message". */
char *render_fault(const char *id, uint32_t file, uint32_t line)
{
    Rt *r = rt();
    const char *f = NULL;
    if (file != NOLOC && file < r->nfiles)
    {
        f = r->files[file];
    }
    const char *sep = strchr(id, DETAIL_SEP);
    if (sep)
    {
        size_t k = (size_t)(sep - id);
        char *i = (char *)cb_xmalloc(k + 1);
        memcpy(i, id, k);
        i[k] = 0;
        char *out = diag_render(i, "dynamic", f, line, 0, sep + 1);
        free(i);
        return out;
    }
    return diag_render(id, "dynamic", f, line, 0, NULL);
}

/* The fault itself, its text borrowed (`owned` false) or the runtime's own
   (moved into a contained failure's record, CHG-0338). */
static void fault_impl(const char *id, bool owned, uint32_t file, uint32_t line) CB_NORET;

void fault(const char *id, uint32_t file, uint32_t line)
{
    fault_impl(id, false, file, line);
}

void fault_owned(char *id, uint32_t file, uint32_t line)
{
    fault_impl(id, true, file, line);
}

static void fault_impl(const char *id, bool owned, uint32_t file, uint32_t line)
{
    /* No lock is held, and none exists, where no thread was ever started. */
    void (*lock_now)(void) = HOOK(MT_fault_lock);
    if (lock_now)
    {
        lock_now();
    }
    replay_fault(me(), id);
    replay_finish();
    if (file == NOLOC && line == 0)
    {
        get_at(&file, &line);
    }
    /* D-0204 `[Fault-Contain]`: a spawned thread's fault ends that thread
       alone, unless it was lent an exclusive reference, its trampoline did
       not register, or the program is ending. */
    uint64_t m = me();
    /* D-0257: a fault inside a library's `_try` entry point ends that call
       alone. */
    uintptr_t lj = LIB_JMP;
    if (lj != 0 && !rt()->unwinding && !ts_raw()->cunwind)
    {
        /* LIB_JMP is set by a library's `_try` entry, after `cb_lib_enter`. */
        HOOK(LIB_contained_fault)(id, file, line, m, lj);
    }
    if (m != 1 && !rt()->unwinding && !ts_raw()->cunwind && ts_raw()->jmp != 0)
    {
        Book *b = book();
        bool lent = hset_has(&b->lent, m);
        book_unlock();
        if (!lent)
        {
            char *raw;
            if (owned)
            {
                raw = (char *)id;
            }
            else
            {
                size_t k = strlen(id);
                raw = (char *)cb_xmalloc(k + 1);
                memcpy(raw, id, k + 1);
            }
            /* A spawned thread (m != 1): `go_multi` ran. */
            HOOK(MT_contained_fault)(raw, file, line, m);
        }
    }
    /* The runtime is never released again: `[Fault-Unwind]` -- other threads
       take no further steps and their frames are not unwound. */
    Rt *r = rt();
    flush_stdout();
    if (r->unwinding)
    {
        /* A destructor faulting during the unwind: the program's outcome is
           the first fault; during an `exit`'s unwind (no report kept), this
           fault. */
        char *rep = r->fault_report;
        r->fault_report = NULL;
        if (!rep)
        {
            rep = render_fault(id, file, line);
        }
        char *s = diag_for_stderr(rep);
        eprint(s);
        plat_exit(1);
    }
    r->fault_report = render_fault(id, file, line);
    if (trace_events_on())
    {
        const char *sep = strchr(id, DETAIL_SEP);
        size_t k = sep ? (size_t)(sep - id) : strlen(id);
        cb_buf b = {0};
        cb_buf_str(&b, "EV fault ");
        cb_buf_put(&b, id, k);
        cb_buf_str(&b, "\n");
        cb_buf_byte(&b, 0);
        eprint((const char *)b.p);
        cb_buf_free(&b);
    }
    r->unwinding = true;
    /* [Fault-Unwind]: fold every open scope and frame, innermost first. */
    fold_frames_and_notes();
    flush_at_end();
    if (rt()->fault_report)
    {
        char *rep = rt()->fault_report;
        rt()->fault_report = NULL;
        char *s = diag_for_stderr(rep);
        eprint(s);
    }
    plat_exit(1);
}

/* The keys of the mutexes thread `m` holds, removed from the book and
   their state cells poisoned (D-0310). Hash-table order (LOG.md): taken in
   key order; the order is not observable (each is poisoned and signalled). */
static u64vec release_locks_of(uint64_t m)
{
    u64vec ks = {0};
    Book *b = book();
    for (size_t i = 0; i < b->locks.cap; i++)
    {
        if (b->locks.used[i] && b->locks.e[i].v.holder == m)
        {
            u64vec_push(&ks, b->locks.e[i].k);
        }
    }
    for (uint32_t i = 1; i < ks.n; i++)
    {
        uint64_t x = ks.p[i];
        uint32_t j = i;
        while (j > 0 && ks.p[j - 1] > x)
        {
            ks.p[j] = ks.p[j - 1];
            j--;
        }
        ks.p[j] = x;
    }
    for (uint32_t i = 0; i < ks.n; i++)
    {
        lockrec h;
        if (lockmap_remove(&b->locks, ks.p[i], &h))
        {
            poison_cells(h.cells);
        }
    }
    book_unlock();
    return ks;
}

static void clear_thread_state(TState *t)
{
    t->scopes.n = 0;
    t->pend.n = 0;
    Inflight x;
    while (infq_pop_front(&t->inflight, &x))
    {
        if (x.kind == IF_DATUM)
        {
            u64vec_free(&x.datum);
        }
    }
}

/* D-0204 `[Fault-Contain]`: the thread's mutexes released and poisoned,
   its scopes and frames folded as `[Fault-Unwind]` folds them, the failure
   recorded, every hold on the runtime given up, and control back to the
   trampoline's `setjmp` (`cb_thread_jmp`), which ends the thread
   (`cb_thread_failed`). `id` is moved into the failure's record. */
void contained_fault(char *id, uint32_t file, uint32_t line, uint64_t m)
{
    char *text = render_fault(id, file, line);
    ts()->cunwind = true;
    u64vec keys = release_locks_of(m);
    for (uint32_t i = 0; i < keys.n; i++)
    {
        signal_key(mutex_key((uintptr_t)keys.p[i]));
    }
    u64vec_free(&keys);
    /* D-0269, D-0315: the frames and the noted locals, which the C
       statements this jump leaves behind would have ended, folded together. */
    fold_frames_and_notes();
    TState *t = ts();
    clear_thread_state(t);
    uint64_t fid;
    {
        Book *b = book();
        Failure f = {id, file, line, text};
        failvec_push(&b->failures, f);
        fid = b->failures.n - 1;
        book_unlock();
    }
    t->has_failed = true;
    t->failed = fid;
    t->cunwind = false;
    if (atomic_load_explicit(&OWNER, memory_order_relaxed) == m)
    {
        (void)lock_release_all();
    }
    longjmp(*(jmp_buf *)t->jmp, 1);
}

/* ---- D-0257: library mode (`cobc --lib`) ----

   A library has no `main`: C calls its entry points, on threads the runtime
   did not start. The first call starts the runtime once; the first thread
   to enter is the runtime's main thread (id 1, MAIN_TS); every other
   foreign thread is registered on its first call as a spawned thread is,
   and released when it ends, by a thread-exit hook. */
static plat_mutex LIB_INIT_LOCK = PLAT_MUTEX_INIT;
static _Atomic bool LIB_INIT_DONE;
static _Atomic bool LIB_FIRST;
static _Atomic bool LIB_MODE;

typedef struct
{
    uint64_t tid;
    uint8_t heap;
    TState *own;
    void *blocks; /* its heap of small blocks (`alloc_heap_mine`) */
} LibReg;

static __thread bool LIB_REGISTERED;
static __thread LibReg LIB_REG;

static void register_foreign(void);

/* The foreign thread ends: as a spawned thread's end, from a thread-exit
   hook, when the thread's other locals may already be gone: nothing here
   touches one. The runtime's write lock and every heap's are taken
   directly; the thread's state and heap are released; its id leaves the
   book. */
static void lib_reg_drop(void *p)
{
    LibReg reg = *(LibReg *)p;
    free(p);
    if (reg.tid == 1)
    {
        return;
    }
    lock_write_spinning();
    size_t n = atomic_load_explicit(&HEAPS_N, memory_order_acquire);
    for (size_t h = 0; h < n; h++)
    {
        hlock(h);
    }
    atomic_fetch_sub_explicit(&cb_live_threads, 1, memory_order_acq_rel);
    size_t h = reg.heap;
    if (h != 0)
    {
        Rt *r = rt();
        if (r->heap_users.p[h] > 0)
        {
            r->heap_users.p[h]--;
        }
        if (r->heap_users.p[h] <= 1)
        {
            atomic_store_explicit(&HEAP_SHARED[h], false, memory_order_release);
        }
    }
    if (atomic_load_explicit(&cb_live_threads, memory_order_acquire) == 1)
    {
        atomic_store_explicit(&SOLO_PENDING, true, memory_order_relaxed);
    }
    for (size_t k = n; k-- > 0;)
    {
        hunlock(k);
    }
    plat_rw_write_unlock(&RTLOCK);
    free(reg.own);
    alloc_heap_end(reg.blocks);
    Book *b = book();
    map_u64_remove(&b->os, reg.tid, NULL);
    book_unlock();
}

/* A library entry point begins: the runtime started once (`init` is the
   program's `cb_init` call), this thread registered. */
void cb_lib_enter(void (*init)(void))
{
    if (!atomic_load_explicit(&LIB_INIT_DONE, memory_order_acquire))
    {
        plat_mutex_lock(&LIB_INIT_LOCK);
        if (!atomic_load_explicit(&LIB_INIT_DONE, memory_order_relaxed))
        {
            atomic_store_explicit(&LIB_MODE, true, memory_order_release);
            HOOK_SET(LIB_contained_fault, lib_contained_fault);
            /* Any thread may call a library: each its own heap of small
               blocks, from the start. */
            alloc_threads_on();
            init();
            atomic_store_explicit(&LIB_INIT_DONE, true, memory_order_release);
        }
        plat_mutex_unlock(&LIB_INIT_LOCK);
    }
    if (LIB_REGISTERED)
    {
        return;
    }
    if (!atomic_exchange_explicit(&LIB_FIRST, true, memory_order_acq_rel))
    {
        /* The main thread: `cb_init` set its stack floor if this thread ran
           it; otherwise set it now. */
        set_stack_floor();
        LIB_REG.tid = 1;
        LIB_REG.heap = 0;
        LIB_REG.own = NULL;
        LIB_REG.blocks = NULL;
        LIB_REGISTERED = true;
        return;
    }
    register_foreign();
}

/* A C thread the runtime did not start, on its first call: given an id, a
   record in the book, a heap and a state of its own, and counted among the
   running threads, as a spawned thread is (`run_job`); released by a
   thread-exit hook (`lib_reg_drop`). */
static void register_foreign(void)
{
    if (atomic_load_explicit(&CB_ENTRIES_ON, memory_order_acquire))
    {
        /* D-0269: from here on a function notes its objectless locals, as
           after a spawn. */
        atomic_store_explicit(&cb_threaded, 1, memory_order_release);
    }
    Entered e = multi() ? ENT_GONE : enter();
    uint64_t tid = atomic_fetch_add_explicit(&NEXT_TID, 1, memory_order_acq_rel);
    TID = tid;
    {
        Book *b = book();
        map_u64_insert(&b->os, tid, netio_os_thread(), NULL);
        book_unlock();
    }
    atomic_fetch_add_explicit(&cb_live_threads, 1, memory_order_acq_rel);
    go_multi();
    entered_drop(e);
    uint8_t heap;
    {
        Entered e2 = enter();
        uint8_t h = claim_heap(rt());
        HEAP = h;
        set_heap(rt(), h);
        heap = h;
        entered_drop(e2);
    }
    TState *own = (TState *)cb_xcalloc(1, sizeof(TState));
    own->dloc.f = NOLOC;
    own->dloc.l = 0;
    TSP = own;
    set_at(NOLOC, 0);
    set_stack_floor();
    LIB_REG.tid = tid;
    LIB_REG.heap = heap;
    LIB_REG.own = own;
    LIB_REG.blocks = alloc_heap_mine();
    LIB_REGISTERED = true;
    LibReg *keep = (LibReg *)cb_xmalloc(sizeof(LibReg));
    *keep = LIB_REG;
    plat_thread_at_exit(lib_reg_drop, keep);
}

/* [Extern-Entry] in a program with `main` (CHG-0475, 001): `main` runs this
   after `cb_init` when the program has entry points. A C thread the program
   did not start may call one at any moment, so the runtime is made ready for
   a second thread now, while `main`'s is the only one: each thread allocates
   from its own heap, and locking is on (`go_multi`) and stays on
   (`go_solo` keeps it while `CB_ENTRIES_ON`), so no thread is ever inside
   the runtime unlocked when another arrives. The main thread is known. */
_Atomic bool CB_ENTRIES_ON;

void cb_entries_on(void)
{
    LIB_REG.tid = 1;
    LIB_REG.heap = 0;
    LIB_REG.own = NULL;
    LIB_REG.blocks = NULL;
    LIB_REGISTERED = true;
    atomic_store_explicit(&LIB_FIRST, true, memory_order_release);
    atomic_store_explicit(&CB_ENTRIES_ON, true, memory_order_release);
    alloc_threads_on();
    Entered e = enter();
    go_multi();
    entered_drop(e);
}

/* An entry point of a program begins: a thread the runtime knows (the main
   one, a spawned one, one registered before) goes on; any other is
   registered as a library's foreign thread is. */
void cb_entry_enter(void)
{
    if (LIKELY(LIB_REGISTERED))
    {
        return;
    }
    register_foreign();
}

/* A thread the runtime started (`worker`): its calls of entry points need no
   registration. */
void thread_known(void)
{
    LIB_REGISTERED = true;
}

/* A library entry point returns to C: what it printed is written out, so
   that C's own output keeps its order with it. */
void cb_lib_leave(void)
{
    flush_stdout();
}

/* `cobalt_shutdown`: every buffered output written. */
void cb_lib_shutdown(void)
{
    if (atomic_load_explicit(&LIB_INIT_DONE, memory_order_acquire))
    {
        flush_at_end();
    }
}

/* The `_try` stub's jump buffer (0 when the call is over). */
/* The buffer a generated `setjmp` filled, made a plain jump's. On 64-bit
   Windows `setjmp` records the frame, and `longjmp` then unwinds the stack
   frame by frame to it (`RtlUnwindEx`), reading each function's unwind
   table. Nothing between a fault and the `setjmp` that contains it needs
   that (C frames, the runtime's holds given up before the jump), and it
   is not safe: for a function with a frame pointer and saved XMM registers
   GCC's table places those registers above the function's frame, so the
   unwinder reads past the end of a thread's stack when that function runs
   near its top (an access violation in about a third of the runs of
   `conf.discarded-noted-results-end-with-statement` on Windows, CHG-0460).
   With no frame recorded `longjmp` restores the registers and jumps. */
void jump_plainly(void *jb)
{
#if defined(_WIN64)
    ((_JUMP_BUFFER *)jb)->Frame = 0;
#else
    (void)jb;
#endif
}

void cb_lib_jmp(void *jb)
{
    jump_plainly(jb);
    LIB_JMP = (uintptr_t)jb;
}

/* The code of the fault a `_try` call just contained: 1 (a fault), 0. */
int cb_lib_fault_code(void)
{
    /* Bug 032: a stack overflow's fault lifted the floor. */
    restore_stack_floor();
    return LIB_FAULT ? 1 : 0;
}

/* `cobalt_last_fault(buf, n)`: the last contained fault's report, at most
   `n - 1` bytes of it, NUL-terminated; the whole report's length, or 0. */
int cb_lib_last_fault(char *buf, size_t n)
{
    if (!LIB_FAULT)
    {
        return 0;
    }
    size_t len = strlen(LIB_FAULT);
    if (n > 0 && buf)
    {
        size_t k = len < n - 1 ? len : n - 1;
        memcpy(buf, LIB_FAULT, k);
        buf[k] = 0;
    }
    return (int)len;
}

/* A fault inside a `_try` entry point: the thread's frames unwind, the
   report is kept, and the stub's `setjmp` resumes. */
void lib_contained_fault(const char *id, uint32_t file, uint32_t line, uint64_t m, uintptr_t jb)
{
    char *text = render_fault(id, file, line);
    ts()->cunwind = true;
    u64vec keys = release_locks_of(m);
    for (uint32_t i = 0; i < keys.n; i++)
    {
        signal_key(mutex_key((uintptr_t)keys.p[i]));
    }
    u64vec_free(&keys);
    fold_frames_and_notes();
    TState *t = ts();
    clear_thread_state(t);
    t->cunwind = false;
    free(LIB_FAULT);
    LIB_FAULT = diag_for_stderr(text);
    free(text);
    LIB_JMP = 0;
    flush_stdout();
    if (atomic_load_explicit(&OWNER, memory_order_relaxed) == m)
    {
        (void)lock_release_all();
    }
    longjmp(*(jmp_buf *)jb, 1);
}

/* `[Terminate-Ok]`: `ok(s)`, reported as exit status `s`. */
void cb_terminate_ok(uint8_t status)
{
    flush_at_end();
    /* D-0264: a normal end; an `exit` wrote its own event. */
    if (trace_events_on() && !atomic_load_explicit(&EXIT_EVENT, memory_order_relaxed))
    {
        char line[32];
        snprintf(line, sizeof line, "EV end %u\n", (unsigned)status);
        eprint(line);
    }
    /* A test hook (CHG-0093): what the runtime still records once the
       program has ended normally. */
    if (plat_getenv("COBALTC_RT_STATS"))
    {
        Rt *r = rt();
        uint64_t objects = 0, paths = 0;
        size_t hn = heaps_len(&r->objs.n);
        for (size_t h = 0; h < hn; h++)
        {
            ObjArena *a = &r->objs.h[h];
            for (size_t i = 0; i < a->n; i++)
            {
                if (a->slots[i].some && a->slots[i].v.alive)
                {
                    objects++;
                }
            }
        }
        hn = heaps_len(&r->paths.n);
        for (size_t h = 0; h < hn; h++)
        {
            PathArena *a = &r->paths.h[h];
            for (size_t i = 0; i < a->n; i++)
            {
                if (a->slots[i].some)
                {
                    paths++;
                }
            }
        }
        char buf[1024];
        snprintf(buf, sizeof buf, "cbrt: live at exit: %llu objects, %llu paths\n", (unsigned long long)objects, (unsigned long long)paths);
        eprint(buf);
        snprintf(buf, sizeof buf, "cbrt: entries while threads ran: %llu local, %llu exclusive; %llu objects migrated\n",
                 (unsigned long long)atomic_load(&N_LOCAL), (unsigned long long)atomic_load(&N_EXCL), (unsigned long long)atomic_load(&N_MIGRATED));
        eprint(buf);
        snprintf(buf, sizeof buf,
                 "cbrt: roots: %llu light, %llu recorded; light references: %llu; paths minted: %llu (%llu holders at stores, %llu recorded on arrival); marked-owner checks: %llu; projections from addresses: %llu; owners from token 0: %llu\n",
                 (unsigned long long)atomic_load(&N_ROOT_LIGHT), (unsigned long long)atomic_load(&N_ROOT_REC), (unsigned long long)atomic_load(&N_LIGHT),
                 (unsigned long long)atomic_load(&N_PATHS), (unsigned long long)atomic_load(&N_HOLDERS), (unsigned long long)atomic_load(&N_ARRIVALS),
                 (unsigned long long)atomic_load(&N_SLOW), (unsigned long long)atomic_load(&N_PROJ_ADDR), (unsigned long long)atomic_load(&N_TOK0_OWNER));
        eprint(buf);
    }
    plat_exit(outbuf_end_status(status));
}

/* ---- the program's arguments and standard streams ---- */

/* `argv` without the program's own name, copied once at startup. */
static uint8_t **ARGS;
static size_t *ARG_LENS;
static size_t NARGS;
static bool ARGS_SET;

/* The program's arguments, and the runtime's start-up: a write to a pipe
   whose reader is gone must fail with `Err(Io)`, not end the program by
   SIGPIPE (D-0143). */
void cb_set_args(int argc, char **argv)
{
    procio_ignore_sigpipe();
    if (ARGS_SET)
    {
        return;
    }
    plat_args(argc, (const char *const *)argv, &ARGS, &ARG_LENS, &NARGS);
    ARGS_SET = true;
}

/* `std::arg_bytes`: up to `n` bytes of argument `i` into `p`; the
   argument's whole length, or -1 when there is no argument `i`. */
int64_t cb_arg_bytes(uint64_t i, void *p, uint64_t n)
{
    if (!ARGS_SET || i >= NARGS)
    {
        return -1;
    }
    size_t len = ARG_LENS[i];
    size_t k = len < n ? len : (size_t)n;
    if (k)
    {
        memcpy(p, ARGS[i], k);
    }
    return (int64_t)len;
}

int64_t cb_write_out(const void *p, uint64_t n)
{
    return outbuf_write((const uint8_t *)p, (size_t)n) == 0 ? (int64_t)n : -1;
}

/* `std::stdout_flush` (D-0178). */
int64_t cb_flush_out(void)
{
    return outbuf_flush() == 0 ? 0 : -1;
}

/* `std::stderr_write` (D-0040): to standard error, unbuffered; standard
   output's buffer written out first (D-0178). */
int64_t cb_write_err(const void *p, uint64_t n)
{
    return outbuf_write_err((const uint8_t *)p, (size_t)n) == 0 ? (int64_t)n : -1;
}

/* `outside`: a call that may wait outside the runtime (D-0204): while it
   lasts, no deadlock is declared; under a schedule it is made out of turn
   (unless this thread holds the runtime, a destructor's call). */
static bool outside_enter(void)
{
    if (!multi())
    {
        return false;
    }
    uint64_t m = TID;
    if (sched_on() && HELD == 0 && LHELD == 0 && atomic_load_explicit(&OWNER, memory_order_relaxed) != m)
    {
        sched_io(m, true);
        atomic_fetch_add_explicit(&IN_IO, 1, memory_order_acq_rel);
        return true;
    }
    /* A count, not the book: every socket operation of every thread passes
       here. */
    atomic_fetch_add_explicit(&IN_IO, 1, memory_order_acq_rel);
    return false;
}

static void outside_leave(bool m_on, bool sched)
{
    if (!m_on)
    {
        return;
    }
    atomic_fetch_sub_explicit(&IN_IO, 1, memory_order_acq_rel);
    if (sched)
    {
        sched_io(TID, false);
    }
}

#define OUTSIDE(EXPR)                         \
    ({                                        \
        bool on_ = multi();                   \
        bool sch_ = outside_enter();          \
        __typeof__(EXPR) r_ = (EXPR);         \
        outside_leave(on_, sch_);             \
        r_;                                   \
    })

/* `std::stdin_read`: up to `n` bytes of standard input; 0 at its end, -1 on
   an error. Standard output's buffer is written out first, so a prompt is
   on the screen before the program waits. */
int64_t cb_read_in(void *p, uint64_t n)
{
    flush_stdout();
    if (n == 0)
    {
        return 0;
    }
    return OUTSIDE(io_stdin_read((uint8_t *)p, (size_t)n));
}

/* `std::stdin_read_line`: up to `n` bytes, through the next `\n`. */
int64_t cb_read_line_in(void *p, uint64_t n)
{
    if (n == 0)
    {
        return 0;
    }
    return OUTSIDE(fileio_read_line_chunk((uint8_t *)p, (size_t)n));
}

/* ---- print and text (`rule.stdlib.print`, `rule.stdlib.text`) ---- */

static void print_text(const char *t, size_t n)
{
    (void)outbuf_write((const uint8_t *)t, n);
}

void cb_print_int(uint64_t hi, uint64_t lo, uint32_t is_signed)
{
    char b[48];
    size_t n = fmt_int_text(hi, lo, is_signed != 0, b);
    print_text(b, n);
}

void cb_print_f64(double v)
{
    char b[64];
    size_t n = fmt_format_float(v, 0, b);
    print_text(b, n);
}

void cb_print_f32(float v)
{
    char b[64];
    size_t n = fmt_format_float((double)v, 1, b);
    print_text(b, n);
}

void cb_print_bool(uint32_t b)
{
    if (b)
    {
        print_text("true", 4);
    }
    else
    {
        print_text("false", 5);
    }
}

/* `String::append`'s `text(v)`: written to `buf`, which holds 64 bytes. */
uint64_t cb_text_int(uint64_t hi, uint64_t lo, uint32_t is_signed, void *buf)
{
    return fmt_int_text(hi, lo, is_signed != 0, (char *)buf);
}

uint64_t cb_text_f64(double v, void *buf)
{
    return fmt_format_float(v, 0, (char *)buf);
}

uint64_t cb_text_f32(float v, void *buf)
{
    return fmt_format_float((double)v, 1, (char *)buf);
}

uint64_t cb_text_bool(uint32_t b, void *buf)
{
    if (b)
    {
        memcpy(buf, "true", 4);
        return 4;
    }
    memcpy(buf, "false", 5);
    return 5;
}

/* ---- `$fmt` (`rule.stdlib.format`) ---- */

typedef struct
{
    uint32_t kind; /* 0 unsigned integer, 1 signed integer, 2 float, 3 text */
    uint32_t width;
    uint64_t hi;
    uint64_t lo;
    double f;
    const uint8_t *p;
    uint64_t n;
} CbFmtArg;

typedef struct
{
    const uint8_t *p;
    uint64_t n;
} CbStr;

typedef struct
{
    uint8_t *key;
    size_t keyn;
    fmt_pieces pieces;
} fmt_cached;

/* `cb_format`'s state, one per thread: the last `$fmt` text (valid until
   its next `$fmt`), the parsed formats (most recent last, at most 64), and
   the arguments copied out of the call, each text argument's bytes in its
   slot's own buffer, kept between calls. */
typedef struct
{
    cb_buf out;
    fmt_cached cache[64];
    size_t ncache;
    fmt_arg *args;
    cb_buf *texts;
    size_t args_cap;
    cb_buf space; /* D-0297: a literal format's text, apart from `out` */
} FmtState;

static __thread FmtState *FMT;

static FmtState *fmt_state(void)
{
    if (UNLIKELY(!FMT))
    {
        FMT = (FmtState *)cb_xcalloc(1, sizeof(FmtState));
    }
    return FMT;
}

void cb_format(const uint8_t *f, uint64_t fn, const CbFmtArg *cargs, uint64_t n, CbStr *out)
{
    FmtState *st = fmt_state();
    size_t na = (size_t)n;
    if (na > st->args_cap)
    {
        size_t c = st->args_cap ? st->args_cap : 4;
        while (c < na)
        {
            c *= 2;
        }
        st->args = (fmt_arg *)cb_xrealloc(st->args, c * sizeof(fmt_arg));
        st->texts = (cb_buf *)cb_xrealloc(st->texts, c * sizeof(cb_buf));
        memset(st->texts + st->args_cap, 0, (c - st->args_cap) * sizeof(cb_buf));
        st->args_cap = c;
    }
    for (size_t i = 0; i < na; i++)
    {
        const CbFmtArg *a = &cargs[i];
        fmt_arg *v = &st->args[i];
        memset(v, 0, sizeof *v);
        if (a->kind == 0 || a->kind == 1)
        {
            v->kind = FMT_ARG_INT;
            v->bits = ((cb_u128)a->hi << 64) | a->lo;
            v->is_signed = a->kind == 1;
            v->width = a->width;
        }
        else if (a->kind == 2)
        {
            v->kind = FMT_ARG_FLOAT;
            v->f = a->f;
            v->is_f32 = a->width == 32;
        }
        else
        {
            /* The bytes copied into the slot's own buffer: a text argument
               that was the previous result is not overwritten under it. */
            cb_buf *t = &st->texts[i];
            cb_buf_clear(t);
            cb_buf_put(t, a->p, (size_t)a->n);
            v->kind = FMT_ARG_TEXT;
            v->p = t->p;
            v->n = t->n;
        }
    }
    /* The format's pieces, parsed once per distinct format (by its bytes),
       the most recent first. */
    size_t fl = (size_t)fn;
    size_t at = SIZE_MAX;
    for (size_t i = st->ncache; i-- > 0;)
    {
        if (st->cache[i].keyn == fl && (fl == 0 || memcmp(st->cache[i].key, f, fl) == 0))
        {
            at = i;
            break;
        }
    }
    if (at == SIZE_MAX)
    {
        if (st->ncache >= 64)
        {
            free(st->cache[0].key);
            fmt_pieces_free(&st->cache[0].pieces);
            memmove(&st->cache[0], &st->cache[1], 63 * sizeof(fmt_cached));
            st->ncache--;
        }
        fmt_cached *c = &st->cache[st->ncache];
        c->key = (uint8_t *)cb_xmalloc(fl ? fl : 1);
        if (fl)
        {
            memcpy(c->key, f, fl);
        }
        c->keyn = fl;
        memset(&c->pieces, 0, sizeof c->pieces);
        (void)fmt_parse(f, fl, &c->pieces);
        at = st->ncache++;
    }
    cb_buf_clear(&st->out);
    fmt_render_into(&st->cache[at].pieces, st->args, na, &st->out);
    out->p = st->out.p;
    out->n = st->out.n;
}

/* D-0297: room for `n` bytes of a literal format's text, this thread's,
   valid until its next call. */
uint8_t *cb_fmt_space(uint64_t n)
{
    FmtState *st = fmt_state();
    cb_buf_clear(&st->space);
    cb_buf_reserve(&st->space, (size_t)n ? (size_t)n : 1);
    return st->space.p;
}

/* ---- files (`rule.stdlib.file`, `rule.stdlib.file-handle`) ---- */

int64_t cb_file_read(const void *path, uint64_t path_len, void *buf, uint64_t cap)
{
    cb_buf data = {0};
    int64_t c = fileio_read(path, (size_t)path_len, &data);
    if (c != 0)
    {
        cb_buf_free(&data);
        return c;
    }
    size_t k = data.n < cap ? data.n : (size_t)cap;
    if (k)
    {
        memcpy(buf, data.p, k);
    }
    int64_t len = (int64_t)data.n;
    cb_buf_free(&data);
    return len;
}

int64_t cb_file_write(const void *path, uint64_t path_len, const void *buf, uint64_t len)
{
    return fileio_write(path, (size_t)path_len, buf, (size_t)len);
}

/* What a file operation gave back, this thread's, kept between calls. */
static __thread cb_buf FILE_OUT;
static __thread cb_buf FILE_AT_OUT;

int64_t cb_file_op(uint64_t op, uint64_t h, void *buf, uint64_t n)
{
    cb_buf *out = &FILE_OUT;
    cb_buf_clear(out);
    int takes = fileio_takes_input(op);
    int64_t r = fileio_call(op, h, takes ? buf : NULL, takes ? (size_t)n : 0, n, out);
    if (out->n)
    {
        memcpy(buf, out->p, out->n);
    }
    return r;
}

/* An OS thread of the runtime ends (`worker`): what its thread-local
   pointers and buffers hold is freed: the formatting state (`cb_format`,
   `cb_fmt_space`), the renderer's buffer (`fmt.c`) and the file
   operations' buffers. (The scheduler's parker, made only under
   `COBALTC_SCHEDULE_SEED`, stays.) */
void thread_locals_free(void)
{
    FmtState *st = FMT;
    if (st)
    {
        cb_buf_free(&st->out);
        for (size_t i = 0; i < st->ncache; i++)
        {
            free(st->cache[i].key);
            fmt_pieces_free(&st->cache[i].pieces);
        }
        for (size_t i = 0; i < st->args_cap; i++)
        {
            cb_buf_free(&st->texts[i]);
        }
        free(st->args);
        free(st->texts);
        cb_buf_free(&st->space);
        free(st);
        FMT = NULL;
    }
    cb_buf_free(&FILE_OUT);
    cb_buf_free(&FILE_AT_OUT);
    fmt_thread_end();
}

/* `std::file_at`: `seek` and `len`, whose number is a position. */
int64_t cb_file_at(uint64_t op, uint64_t h, uint64_t pos)
{
    cb_buf *out = &FILE_AT_OUT;
    cb_buf_clear(out);
    return fileio_call(op, h, NULL, 0, pos, out);
}

/* ---- sorts (D-0062, D-0297) ---- */

#define SORT_PLAIN(NAME, T)                                    \
    static void NAME##_ins(T *a, size_t n)                     \
    {                                                          \
        for (size_t i = 1; i < n; i++)                         \
        {                                                      \
            T x = a[i];                                        \
            size_t j = i;                                      \
            while (j > 0 && a[j - 1] > x)                      \
            {                                                  \
                a[j] = a[j - 1];                               \
                j--;                                           \
            }                                                  \
            a[j] = x;                                          \
        }                                                      \
    }                                                          \
    static void NAME##_heap(T *a, size_t n)                    \
    {                                                          \
        for (size_t s = n / 2; s-- > 0;)                       \
        {                                                      \
            size_t r = s;                                      \
            for (;;)                                           \
            {                                                  \
                size_t c = 2 * r + 1;                          \
                if (c >= n)                                    \
                {                                              \
                    break;                                     \
                }                                              \
                if (c + 1 < n && a[c] < a[c + 1])              \
                {                                              \
                    c++;                                       \
                }                                              \
                if (!(a[r] < a[c]))                            \
                {                                              \
                    break;                                     \
                }                                              \
                T t = a[r];                                    \
                a[r] = a[c];                                   \
                a[c] = t;                                      \
                r = c;                                         \
            }                                                  \
        }                                                      \
        for (size_t e = n; e-- > 1;)                           \
        {                                                      \
            T t = a[0];                                        \
            a[0] = a[e];                                       \
            a[e] = t;                                          \
            size_t r = 0;                                      \
            for (;;)                                           \
            {                                                  \
                size_t c = 2 * r + 1;                          \
                if (c >= e)                                    \
                {                                              \
                    break;                                     \
                }                                              \
                if (c + 1 < e && a[c] < a[c + 1])              \
                {                                              \
                    c++;                                       \
                }                                              \
                if (!(a[r] < a[c]))                            \
                {                                              \
                    break;                                     \
                }                                              \
                T u = a[r];                                    \
                a[r] = a[c];                                   \
                a[c] = u;                                      \
                r = c;                                         \
            }                                                  \
        }                                                      \
    }                                                          \
    static void NAME##_q(T *a, size_t n, int depth)            \
    {                                                          \
        while (n > 24)                                         \
        {                                                      \
            if (depth-- == 0)                                  \
            {                                                  \
                NAME##_heap(a, n);                             \
                return;                                        \
            }                                                  \
            T x = a[0], y = a[n / 2], z = a[n - 1], p;         \
            if (x < y)                                         \
            {                                                  \
                p = y < z ? y : (x < z ? z : x);               \
            }                                                  \
            else                                               \
            {                                                  \
                p = x < z ? x : (y < z ? z : y);               \
            }                                                  \
            size_t i = 0, j = n - 1;                           \
            for (;;)                                           \
            {                                                  \
                while (a[i] < p)                               \
                {                                              \
                    i++;                                       \
                }                                              \
                while (p < a[j])                               \
                {                                              \
                    j--;                                       \
                }                                              \
                if (i >= j)                                    \
                {                                              \
                    break;                                     \
                }                                              \
                T t = a[i];                                    \
                a[i] = a[j];                                   \
                a[j] = t;                                      \
                i++;                                           \
                j--;                                           \
            }                                                  \
            size_t k = j + 1;                                  \
            if (k < n - k)                                     \
            {                                                  \
                NAME##_q(a, k, depth);                         \
                a += k;                                        \
                n -= k;                                        \
            }                                                  \
            else                                               \
            {                                                  \
                NAME##_q(a + k, n - k, depth);                 \
                n = k;                                         \
            }                                                  \
        }                                                      \
        NAME##_ins(a, n);                                      \
    }                                                          \
    static void NAME(T *a, size_t n)                           \
    {                                                          \
        int d = 0;                                             \
        for (size_t m = n; m > 1; m >>= 1)                     \
        {                                                      \
            d += 2;                                            \
        }                                                      \
        NAME##_q(a, n, d);                                     \
    }

SORT_PLAIN(sort_u8, uint8_t)
SORT_PLAIN(sort_u16, uint16_t)
SORT_PLAIN(sort_u32, uint32_t)
SORT_PLAIN(sort_u64, uint64_t)
SORT_PLAIN(sort_u128, cb_u128)
SORT_PLAIN(sort_i8, int8_t)
SORT_PLAIN(sort_i16, int16_t)
SORT_PLAIN(sort_i32, int32_t)
SORT_PLAIN(sort_i64, int64_t)
SORT_PLAIN(sort_i128, cb_i128)

/* `Vec::sort` (D-0062) of a plain integer or `bool` element type: the
   elements sorted in place. Equal elements of a plain type cannot be told
   apart, so a stable sort and this unstable one leave the same bytes.
   `kind`: the element's width in bytes, plus 256 when it is signed; 0 for
   `bool`. */
void cb_sort_plain(void *p, uint64_t n, uint32_t kind)
{
    size_t k = (size_t)n;
    switch (kind)
    {
    case 0:
    case 1:
        sort_u8((uint8_t *)p, k);
        break;
    case 2:
        sort_u16((uint16_t *)p, k);
        break;
    case 4:
        sort_u32((uint32_t *)p, k);
        break;
    case 8:
        sort_u64((uint64_t *)p, k);
        break;
    case 16:
        sort_u128((cb_u128 *)p, k);
        break;
    case 257:
        sort_i8((int8_t *)p, k);
        break;
    case 258:
        sort_i16((int16_t *)p, k);
        break;
    case 260:
        sort_i32((int32_t *)p, k);
        break;
    case 264:
        sort_i64((int64_t *)p, k);
        break;
    case 272:
        sort_i128((cb_i128 *)p, k);
        break;
    default:
        break;
    }
}

/* Byte order, a prefix first (`[u8]`'s `Ord`). */
static inline int bytes_cmp(const uint8_t *a, uint64_t an, const uint8_t *b, uint64_t bn)
{
    uint64_t m = an < bn ? an : bn;
    /* The first eight bytes as one big-endian word each: texts that differ
       there (most of a sort's comparisons) need no call to `memcmp`. */
    if (m >= 8)
    {
        uint64_t x, y;
        memcpy(&x, a, 8);
        memcpy(&y, b, 8);
        if (x != y)
        {
            x = __builtin_bswap64(x);
            y = __builtin_bswap64(y);
            return x < y ? -1 : 1;
        }
    }
    else
    {
        /* Short texts byte by byte, with no call. */
        for (uint64_t i = 0; i < m; i++)
        {
            if (a[i] != b[i])
            {
                return a[i] < b[i] ? -1 : 1;
            }
        }
        return an < bn ? -1 : (an > bn ? 1 : 0);
    }
    int c = m ? memcmp(a, b, (size_t)m) : 0;
    if (c != 0)
    {
        return c;
    }
    return an < bn ? -1 : (an > bn ? 1 : 0);
}

/* A stable merge sort of `n` records of W words (2: `str`, 3: `String`),
   keyed by the text at words 0 (pointer) and 1 (length). One copy for each
   width, so a record moves as a fixed-size struct the C compiler inlines
   (a `memcpy` of a width known only at run time is a call per move). */
#define TEXT_MERGE(NAME, W)                                                                                    \
    typedef struct                                                                                             \
    {                                                                                                          \
        uint64_t w[W];                                                                                         \
    } NAME##_rec;                                                                                              \
    static inline int NAME##_cmp(const NAME##_rec *x, const NAME##_rec *y)                                     \
    {                                                                                                          \
        return bytes_cmp((const uint8_t *)(uintptr_t)x->w[0], x->w[1], (const uint8_t *)(uintptr_t)y->w[0], y->w[1]); \
    }                                                                                                          \
    static void NAME(NAME##_rec *a, NAME##_rec *tmp, size_t n)                                                 \
    {                                                                                                          \
        if (n < 2)                                                                                             \
        {                                                                                                      \
            return;                                                                                            \
        }                                                                                                      \
        if (n <= 12)                                                                                           \
        {                                                                                                      \
            for (size_t i = 1; i < n; i++)                                                                     \
            {                                                                                                  \
                NAME##_rec x = a[i];                                                                           \
                size_t j = i;                                                                                  \
                while (j > 0 && NAME##_cmp(&a[j - 1], &x) > 0)                                                 \
                {                                                                                              \
                    a[j] = a[j - 1];                                                                           \
                    j--;                                                                                       \
                }                                                                                              \
                a[j] = x;                                                                                      \
            }                                                                                                  \
            return;                                                                                            \
        }                                                                                                      \
        size_t h = n / 2;                                                                                      \
        NAME(a, tmp, h);                                                                                       \
        NAME(a + h, tmp, n - h);                                                                               \
        /* Already in order: nothing to merge. */                                                              \
        if (NAME##_cmp(&a[h - 1], &a[h]) <= 0)                                                                 \
        {                                                                                                      \
            return;                                                                                            \
        }                                                                                                      \
        memcpy(tmp, a, h * sizeof(NAME##_rec));                                                                \
        size_t i = 0, j = h, k = 0;                                                                            \
        while (i < h && j < n)                                                                                 \
        {                                                                                                      \
            if (NAME##_cmp(&a[j], &tmp[i]) < 0)                                                                \
            {                                                                                                  \
                a[k++] = a[j++];                                                                               \
            }                                                                                                  \
            else                                                                                               \
            {                                                                                                  \
                a[k++] = tmp[i++];                                                                             \
            }                                                                                                  \
        }                                                                                                      \
        while (i < h)                                                                                          \
        {                                                                                                      \
            a[k++] = tmp[i++];                                                                                 \
        }                                                                                                      \
    }

TEXT_MERGE(text_merge2, 2)
TEXT_MERGE(text_merge3, 3)

/* A sort by keys, for TEXT_KEYED_MIN texts or more. Each text gets a key
   of two words: eight of its bytes as one big-endian word (zero after the
   text's end), and a tag holding the count of bytes from there on (nine
   for nine or more) above the record's index. Keys order as their texts
   do by the word, then by the count: where the words are the same and a
   count is under nine, the shorter text is a prefix of the other (the
   word's zeros are its end, or the other's own zero bytes), and comes
   first. So a merge reads the keys alone, in order, and picks without a
   jump; the records move once, at the end.

   Texts that agree in the eight bytes and both go on (the same word, count
   nine) end next to one another, in the order they came in: a tied run,
   which gets keys from its next bytes and is sorted the same way, until no
   run is left.

   The order is `bytes_cmp`'s, and the sort is stable. */
typedef struct
{
    uint64_t word;
    uint64_t tag;
} text_key;

#define TEXT_KEYED_MIN 32
#define TEXT_KEY_SHIFT 60
#define TEXT_KEY_INDEX ((UINT64_C(1) << TEXT_KEY_SHIFT) - 1)
/* A tied run shorter than this is sorted by comparing its texts. */
#define TEXT_RUN_MIN 12
/* Keys are made again, past the bytes all the texts share, when they share
   this many of a key's eight. */
#define TEXT_KEY_SHARED 3

typedef struct
{
    const uint64_t *recs; /* the records, `width` words each: pointer, length, … */
    size_t width;
} text_recs;

/* A tied run still to sort: keys `lo` up to `hi`, alike in their texts'
   first `skip` bytes. */
typedef struct
{
    size_t lo;
    size_t hi;
    uint64_t skip;
} text_run;

static inline const uint8_t *text_of(const text_recs *t, const text_key *k, uint64_t *len)
{
    const uint64_t *r = t->recs + (size_t)(k->tag & TEXT_KEY_INDEX) * t->width;
    *len = r[1];
    return (const uint8_t *)(uintptr_t)r[0];
}

/* Whether `x` comes before `y`: the word, then the count, then the index,
   as one 128-bit comparison (a subtraction with borrow, no jump). The
   index keeps keys alike in word and count in the order they came in. */
static inline size_t text_key_less(const text_key *x, const text_key *y)
{
    return (size_t)((((unsigned __int128)x->word << 64) | x->tag) < (((unsigned __int128)y->word << 64) | y->tag));
}

static void text_key_sort(text_key *a, text_key *tmp, size_t n)
{
    if (n <= 8)
    {
        for (size_t i = 1; i < n; i++)
        {
            text_key x = a[i];
            size_t j = i;
            while (j > 0 && text_key_less(&x, &a[j - 1]))
            {
                a[j] = a[j - 1];
                j--;
            }
            a[j] = x;
        }
        return;
    }
    size_t h = n / 2;
    text_key_sort(a, tmp, h);
    text_key_sort(a + h, tmp, n - h);
    /* Already in order: nothing to merge. */
    if (!text_key_less(&a[h], &a[h - 1]))
    {
        return;
    }
    memcpy(tmp, a, h * sizeof(text_key));
    size_t i = 0, j = h, k = 0;
    while (i < h && j < n)
    {
        /* The right one when it comes first; the choice is arithmetic on
           the two addresses, not a jump the processor has to guess. */
        size_t right = text_key_less(&a[j], &tmp[i]);
        uintptr_t left_at = (uintptr_t)&tmp[i], right_at = (uintptr_t)&a[j];
        const text_key *from = (const text_key *)(left_at ^ ((left_at ^ right_at) & ((uintptr_t)0 - right)));
        a[k++] = *from;
        j += right;
        i += 1 - right;
    }
    while (i < h)
    {
        a[k++] = tmp[i++];
    }
}

/* The bytes the texts of the `n` keys share after their first `skip`. */
static uint64_t text_keys_common(const text_recs *t, const text_key *keys, size_t n, uint64_t skip)
{
    uint64_t len;
    const uint8_t *first = text_of(t, &keys[0], &len) + skip;
    uint64_t common = len - skip;
    for (size_t r = 1; r < n && common > 0; r++)
    {
        const uint8_t *p = text_of(t, &keys[r], &len) + skip;
        len -= skip;
        if (len >= common && memcmp(p, first, (size_t)common) == 0)
        {
            continue;
        }
        uint64_t m = len < common ? len : common;
        uint64_t i = 0;
        while (i < m && p[i] == first[i])
        {
            i++;
        }
        common = i;
    }
    return common;
}

/* The keys of `n` texts from their byte `skip` on; each keeps its index.
   The result is how many of the eight bytes all the texts share (eight
   when every text has them and they are the same). */
static uint64_t text_keys_make(const text_recs *t, text_key *keys, size_t n, uint64_t skip)
{
    uint64_t differ = 0, shortest = UINT64_MAX, first = 0;
    for (size_t r = 0; r < n; r++)
    {
        uint64_t len;
        const uint8_t *p = text_of(t, &keys[r], &len) + skip;
        len -= skip;
        uint64_t word = 0;
        if (len >= 8)
        {
            memcpy(&word, p, 8);
            word = __builtin_bswap64(word);
        }
        else
        {
            for (uint64_t i = 0; i < len; i++)
            {
                word |= (uint64_t)p[i] << (56 - 8 * i);
            }
        }
        if (r == 0)
        {
            first = word;
        }
        differ |= word ^ first;
        shortest = len < shortest ? len : shortest;
        keys[r].word = word;
        keys[r].tag = ((len < 9 ? len : 9) << TEXT_KEY_SHIFT) | (keys[r].tag & TEXT_KEY_INDEX);
    }
    uint64_t shared = differ ? (uint64_t)__builtin_clzll(differ) / 8 : 8;
    return shared < shortest ? shared : shortest;
}

/* A short tied run, by its texts from their byte `skip` on. */
static void text_keys_insertion(const text_recs *t, text_key *keys, size_t n, uint64_t skip)
{
    for (size_t i = 1; i < n; i++)
    {
        text_key x = keys[i];
        uint64_t xn;
        const uint8_t *xp = text_of(t, &x, &xn);
        size_t j = i;
        while (j > 0)
        {
            uint64_t yn;
            const uint8_t *yp = text_of(t, &keys[j - 1], &yn);
            if (bytes_cmp(yp + skip, yn - skip, xp + skip, xn - skip) <= 0)
            {
                break;
            }
            keys[j] = keys[j - 1];
            j--;
        }
        keys[j] = x;
    }
}

/* Puts the `n` keys, each holding its record's index, in their texts'
   order. `tmp` has room for half of them. */
static void text_keys_order(const text_recs *t, text_key *keys, text_key *tmp, size_t n)
{
    text_run *runs = NULL;
    size_t nruns = 0, cap = 0;
    text_run cur = {0, n, 0};
    for (;;)
    {
        text_key *run = keys + cur.lo;
        size_t m = cur.hi - cur.lo;
        /* Keys from where the run's texts are known to be alike; again from
           further on when most of a key's bytes turn out to be shared. */
        uint64_t skip = cur.skip;
        uint64_t shared = text_keys_make(t, run, m, skip);
        if (shared >= TEXT_KEY_SHARED)
        {
            skip += shared == 8 ? 8 + text_keys_common(t, run, m, skip + 8) : shared;
            text_keys_make(t, run, m, skip);
        }
        text_key_sort(run, tmp, m);
        for (size_t i = 0; i < m;)
        {
            size_t j = i + 1;
            if ((run[i].tag >> TEXT_KEY_SHIFT) == 9)
            {
                while (j < m && run[j].word == run[i].word && (run[j].tag >> TEXT_KEY_SHIFT) == 9)
                {
                    j++;
                }
                if (j - i >= TEXT_RUN_MIN)
                {
                    if (nruns == cap)
                    {
                        cap = cap ? cap * 2 : 16;
                        runs = (text_run *)cb_xrealloc(runs, cap * sizeof(text_run));
                    }
                    runs[nruns].lo = cur.lo + i;
                    runs[nruns].hi = cur.lo + j;
                    runs[nruns].skip = skip + 8;
                    nruns++;
                }
                else if (j - i > 1)
                {
                    text_keys_insertion(t, run + i, j - i, skip + 8);
                }
            }
            i = j;
        }
        if (nruns == 0)
        {
            break;
        }
        cur = runs[--nruns];
    }
    free(runs);
}

/* `NAME##_keyed` orders the keys of `n` records, then writes each record
   to its place from a copy: the reads are scattered but do not wait for
   one another, as the steps of a walk round the permutation's cycles
   would. Records already in place at the front (all of them, when the
   texts came in order) are left. */
#define TEXT_KEYED(NAME)                                                                                       \
    static void NAME##_keyed(NAME##_rec *a, size_t n)                                                          \
    {                                                                                                          \
        text_key *keys = (text_key *)cb_xmalloc(n * sizeof(text_key));                                         \
        text_key *tmp = (text_key *)cb_xmalloc((n / 2 + 1) * sizeof(text_key));                                \
        text_recs t;                                                                                           \
        t.recs = (const uint64_t *)a;                                                                          \
        t.width = sizeof(NAME##_rec) / sizeof(uint64_t);                                                       \
        for (size_t i = 0; i < n; i++)                                                                         \
        {                                                                                                      \
            keys[i].tag = (uint64_t)i;                                                                         \
        }                                                                                                      \
        text_keys_order(&t, keys, tmp, n);                                                                     \
        free(tmp);                                                                                             \
        size_t placed = 0;                                                                                     \
        while (placed < n && (size_t)(keys[placed].tag & TEXT_KEY_INDEX) == placed)                            \
        {                                                                                                      \
            placed++;                                                                                          \
        }                                                                                                      \
        if (placed < n)                                                                                        \
        {                                                                                                      \
            NAME##_rec *copy = (NAME##_rec *)cb_xmalloc((n - placed) * sizeof(NAME##_rec));                    \
            memcpy(copy, a + placed, (n - placed) * sizeof(NAME##_rec));                                       \
            for (size_t i = placed; i < n; i++)                                                                \
            {                                                                                                  \
                a[i] = copy[(size_t)(keys[i].tag & TEXT_KEY_INDEX) - placed];                                  \
            }                                                                                                  \
            free(copy);                                                                                        \
        }                                                                                                      \
        free(keys);                                                                                            \
    }

TEXT_KEYED(text_merge2)
TEXT_KEYED(text_merge3)

/* D-0297: `Vec::sort` of `String` (`kind` 0: pointer, length, capacity) or
   `str` (1: pointer and length) holding no reference, stably in byte
   order. */
void cb_sort_text(void *p, uint64_t n, uint32_t kind)
{
    size_t k = (size_t)n;
    if (k < 2)
    {
        return;
    }
    if (kind == 0)
    {
        if (k >= TEXT_KEYED_MIN)
        {
            text_merge3_keyed((text_merge3_rec *)p, k);
            return;
        }
        text_merge3_rec *tmp = (text_merge3_rec *)cb_xmalloc((k / 2 + 1) * sizeof(text_merge3_rec));
        text_merge3((text_merge3_rec *)p, tmp, k);
        free(tmp);
    }
    else
    {
        if (k >= TEXT_KEYED_MIN)
        {
            text_merge2_keyed((text_merge2_rec *)p, k);
            return;
        }
        text_merge2_rec *tmp = (text_merge2_rec *)cb_xmalloc((k / 2 + 1) * sizeof(text_merge2_rec));
        text_merge2((text_merge2_rec *)p, tmp, k);
        free(tmp);
    }
}

/* `key_less` (D-0062) on text: byte order, a prefix first. */
uint8_t cb_bytes_less(const void *a, uint64_t an, const void *b, uint64_t bn)
{
    return bytes_cmp(a, an, b, bn) < 0;
}

/* ---- the filesystem, the environment and the clocks (D-0061) ---- */

int64_t cb_fs_op(uint64_t op, const void *a, uint64_t an, const void *b, uint64_t bn)
{
    return fileio_fs_op(op, a, (size_t)an, b, (size_t)bn);
}

int64_t cb_fs_query(uint64_t op, const void *a, uint64_t an, void *buf, uint64_t cap)
{
    cb_buf out = {0};
    int64_t c = fileio_fs_query(op, a, (size_t)an, &out);
    if (c != 0)
    {
        cb_buf_free(&out);
        return c;
    }
    size_t k = out.n < cap ? out.n : (size_t)cap;
    if (k)
    {
        memcpy(buf, out.p, k);
    }
    int64_t len = (int64_t)out.n;
    cb_buf_free(&out);
    return len;
}

int64_t cb_clock_read(uint64_t which)
{
    return fileio_clock_read(which);
}

/* `std::tz_offset` (D-0147). */
int64_t cb_tz_offset(int64_t t)
{
    return fileio_tz_offset(t);
}

static bool timer_ready(Book *b, void *ctx)
{
    (void)b;
    return plat_now_ns() >= *(uint64_t *)ctx;
}

/* `std::sleep_ns` (D-0124). D-0204: while threads run, a wait on the
   clock, which `cancel` ends; it never counts as stuck. */
int64_t cb_sleep_ns(uint64_t ns)
{
    if (!multi())
    {
        fileio_sleep_ns(ns);
        return 0;
    }
    uint64_t now = plat_now_ns();
    uint64_t deadline = now + ns < now ? UINT64_MAX : now + ns;
    uint64_t m = me();
    HAS_TIMER_DEADLINE = true;
    TIMER_DEADLINE = deadline;
    Wait w = {W_TIMER, 0, 0};
    wait_until(m | (3ull << 62), w, timer_ready, &deadline);
    HAS_TIMER_DEADLINE = false;
    return 0;
}

/* ---- paths, processes, sockets (D-0141, D-0143, D-0144) ---- */

static int64_t byte_call(io_byte_call f, uint64_t op, uint64_t h, const uint8_t *a, uint64_t an, const uint8_t *b, uint64_t bn, uint8_t *out, uint64_t cap)
{
    cb_buf v = {0};
    int64_t r = f(op, h, an ? a : NULL, (size_t)an, bn ? b : NULL, (size_t)bn, cap, &v);
    size_t k = v.n < cap ? v.n : (size_t)cap;
    if (k)
    {
        memcpy(out, v.p, k);
    }
    cb_buf_free(&v);
    return r;
}

int64_t cb_path_op(uint64_t op, uint64_t h, const void *a, uint64_t an, const void *b, uint64_t bn, void *out, uint64_t cap)
{
    return byte_call(fileio_path_call, op, h, a, an, b, bn, out, cap);
}

int64_t cb_proc_op(uint64_t op, uint64_t h, const void *a, uint64_t an, const void *b, uint64_t bn, void *out, uint64_t cap)
{
    return OUTSIDE(byte_call(procio_call, op, h, a, an, b, bn, out, cap));
}

int64_t cb_net_op(uint64_t op, uint64_t h, const void *a, uint64_t an, const void *b, uint64_t bn, void *out, uint64_t cap)
{
    int64_t r = OUTSIDE(byte_call(netio_call, op, h, a, an, b, bn, out, cap));
    /* D-0204: a thread asked to stop leaves its socket's wait. */
    if (r == NETIO_CANCELLED)
    {
        fault("diag.thread-cancelled", NOLOC, 0);
    }
    return r;
}

/* D-0204: `netio`'s question while a socket waits: has this thread been
   asked to stop (and can it be)? */
int cancel_probe(void)
{
    if (atomic_load_explicit(&CANCELS, memory_order_acquire) == 0 || !multi() || ts_raw()->cunwind)
    {
        return 0;
    }
    uint64_t m = me();
    Book *b = book();
    int r = hset_has(&b->cancel, m) && !hset_has(&b->lent, m);
    book_unlock();
    return r;
}

/* `std::os_random` (D-0142). */
int64_t cb_os_random(void *out, uint64_t n)
{
    if (n == 0)
    {
        return 0;
    }
    return osrand_fill(out, (size_t)n);
}

/* ---- parse (`rule.stdlib.text` `[Parse]`) ---- */

int64_t cb_parse_int(const void *p, uint64_t n, uint32_t is_signed, uint32_t bits, uint64_t *out)
{
    cb_u128 v;
    int64_t c = numtext_parse_int(p, (size_t)n, is_signed != 0, bits, &v);
    if (c == -1 && out)
    {
        out[0] = (uint64_t)(v >> 64);
        out[1] = (uint64_t)v;
    }
    return c;
}

int64_t cb_parse_float(const void *p, uint64_t n, uint32_t is_f32, double *out)
{
    double v;
    int64_t c = numtext_parse_float(p, (size_t)n, is_f32 != 0, &v);
    if (c == -1 && out)
    {
        *out = v;
    }
    return c;
}

/* D-0181: `parse_check`/`parse_value` for `bool`. */
int64_t cb_parse_bool(const void *p, uint64_t n, uint8_t *out)
{
    uint8_t v;
    int64_t c = numtext_parse_bool(p, (size_t)n, &v);
    if (c == -1 && out)
    {
        *out = v;
    }
    return c;
}

/* ---- schedules (D-0204: `cobc --explore`, `--schedule-seed`) ----

   With `COBALTC_SCHEDULE_SEED` set, one program thread runs at a time: the
   one holding the turn. The turn passes, to a thread the seed's sequence
   chooses, wherever a thread could be preempted without holding the
   runtime: a `spawn`, every wait, a thread's end, a call outside the
   runtime, after a mutex is released, and now and then at any such entry.
   One seed gives one order. */
/* `std::thread::park`/`unpark`: a flag under a lock, a condition
   variable to sleep on; an unpark before the park is kept. */
typedef struct
{
    plat_mutex m;
    plat_cond c;
    bool token;
} plat_parker;

static plat_parker *plat_parker_new(void)
{
    plat_parker *p = (plat_parker *)cb_xcalloc(1, sizeof(plat_parker));
    plat_mutex_init(&p->m);
    plat_cond_init(&p->c);
    return p;
}

static void plat_park(plat_parker *p)
{
    plat_mutex_lock(&p->m);
    while (!p->token)
    {
        plat_cond_wait(&p->c, &p->m);
    }
    p->token = false;
    plat_mutex_unlock(&p->m);
}

static void plat_unpark(plat_parker *p)
{
    plat_mutex_lock(&p->m);
    p->token = true;
    plat_cond_signal(&p->c);
    plat_mutex_unlock(&p->m);
}

typedef struct
{
    uint64_t id;
    plat_parker *t;
} parked_t;

VEC(parkedvec, parked_t)

typedef struct SchedSt
{
    uint64_t turn;
    u64vec members;
    u64vec io;
    uint64_t rng;
    /* Each waiting thread's parker, to wake the one whose turn it is. */
    parkedvec parked;
} SchedSt;

static plat_mutex SCHED_LOCK = PLAT_MUTEX_INIT;
static SchedSt SCHED = {.turn = 1};

SchedSt *sched_lock(void)
{
    plat_mutex_lock(&SCHED_LOCK);
    return &SCHED;
}

static void sched_unlock(void)
{
    plat_mutex_unlock(&SCHED_LOCK);
}

/* splitmix64 */
static uint64_t sched_next(SchedSt *s)
{
    s->rng += 0x9e3779b97f4a7c15ull;
    uint64_t z = s->rng;
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
    return z ^ (z >> 31);
}

static void sched_wake(SchedSt *s)
{
    for (size_t i = 0; i < s->parked.n; i++)
    {
        if (s->parked.p[i].id == s->turn)
        {
            plat_unpark(s->parked.p[i].t);
            return;
        }
    }
}

/* The next turn: a member not outside the runtime, or none (0) when every
   one is; the first back then takes it. */
static void sched_pick(SchedSt *s)
{
    size_t n = 0;
    for (uint32_t i = 0; i < s->members.n; i++)
    {
        if (!u64vec_contains(&s->io, s->members.p[i]))
        {
            n++;
        }
    }
    if (n == 0)
    {
        s->turn = 0;
        return;
    }
    size_t k = (size_t)(sched_next(s) % n);
    for (uint32_t i = 0; i < s->members.n; i++)
    {
        if (!u64vec_contains(&s->io, s->members.p[i]))
        {
            if (k == 0)
            {
                s->turn = s->members.p[i];
                break;
            }
            k--;
        }
    }
    sched_wake(s);
}

void sched_init(void)
{
    const char *v = plat_getenv("COBALTC_SCHEDULE_SEED");
    uint64_t seed = 0;
    if (v)
    {
        /* `trim().parse::<u64>()`: decimal digits, an optional `+`. */
        size_t n = strlen(v), a = 0, b = n;
        while (a < b && (v[a] == ' ' || (v[a] >= '\t' && v[a] <= '\r')))
        {
            a++;
        }
        while (b > a && (v[b - 1] == ' ' || (v[b - 1] >= '\t' && v[b - 1] <= '\r')))
        {
            b--;
        }
        if (a < b && v[a] == '+')
        {
            a++;
        }
        bool ok = a < b;
        uint64_t x = 0;
        for (size_t i = a; i < b && ok; i++)
        {
            if (v[i] < '0' || v[i] > '9')
            {
                ok = false;
                break;
            }
            if (__builtin_mul_overflow(x, 10, &x) || __builtin_add_overflow(x, (uint64_t)(v[i] - '0'), &x))
            {
                ok = false;
            }
        }
        seed = ok ? x : 0;
    }
    if (seed != 0)
    {
        SchedSt *g = sched_lock();
        g->rng = seed;
        g->members.n = 0;
        u64vec_push(&g->members, 1);
        g->turn = 1;
        sched_unlock();
        atomic_store_explicit(&SCHED_ON, true, memory_order_relaxed);
        atomic_store_explicit(&cb_sched_flag, 1, memory_order_relaxed);
    }
}

static __thread plat_parker *MY_PARKER;

/* Waits for this thread's turn (taking it when no one has it); called with
   the schedule's lock held, returns with it released. */
void sched_wait(uint64_t m, SchedSt *g)
{
    if (g->turn != m && g->turn != 0)
    {
        if (!MY_PARKER)
        {
            MY_PARKER = plat_parker_new();
        }
        parked_t pk = {m, MY_PARKER};
        parkedvec_push(&g->parked, pk);
        while (g->turn != m && g->turn != 0)
        {
            sched_unlock();
            plat_park(MY_PARKER);
            g = sched_lock();
        }
        size_t j = 0;
        for (size_t i = 0; i < g->parked.n; i++)
        {
            if (g->parked.p[i].id != m)
            {
                g->parked.p[j++] = g->parked.p[i];
            }
        }
        g->parked.n = j;
    }
    g->turn = m;
    sched_unlock();
}

/* Gives the turn to the seed's choice (maybe this thread again) and waits
   for it to come back. Only where this thread holds nothing. */
void rt_sched_yield(void)
{
    SCHED_PENDING = false;
    uint64_t m = TID;
    SchedSt *g = sched_lock();
    sched_pick(g);
    sched_wait(m, g);
}

/* At an entry: the switch a release asked for, or one in 32 by the seed,
   when this thread holds nothing. */
CB_COLD void sched_entry(void)
{
    uint64_t m = TID;
    if (HELD != 0 || LHELD != 0 || atomic_load_explicit(&OWNER, memory_order_relaxed) == m)
    {
        return;
    }
    bool pending = SCHED_PENDING;
    bool go = pending;
    if (!go)
    {
        SchedSt *g = sched_lock();
        go = sched_next(g) % 32 == 0;
        sched_unlock();
    }
    if (go)
    {
        rt_sched_yield();
    }
}

/* D-0249: an atomic operation is a synchronization point; under a schedule
   the turn may pass at each one. */
void cb_atomic_point(void)
{
    if (!sched_on())
    {
        return;
    }
    uint64_t m = TID;
    if (HELD != 0 || LHELD != 0 || atomic_load_explicit(&OWNER, memory_order_relaxed) == m)
    {
        return;
    }
    SchedSt *g = sched_lock();
    bool go = sched_next(g) % 2 == 0;
    sched_unlock();
    if (go)
    {
        rt_sched_yield();
    }
}

bool sched_alone(uint64_t m)
{
    SchedSt *g = sched_lock();
    bool other = false;
    for (uint32_t i = 0; i < g->members.n; i++)
    {
        if (g->members.p[i] != m && !u64vec_contains(&g->io, g->members.p[i]))
        {
            other = true;
            break;
        }
    }
    sched_unlock();
    return !other;
}

void sched_add(uint64_t t)
{
    SchedSt *g = sched_lock();
    u64vec_push(&g->members, t);
    sched_unlock();
}

void sched_leave(uint64_t m)
{
    SchedSt *g = sched_lock();
    u64vec_remove_all(&g->members, m);
    if (g->turn == m)
    {
        sched_pick(g);
    }
    sched_unlock();
}

void sched_io(uint64_t m, bool on)
{
    SchedSt *g = sched_lock();
    if (on)
    {
        u64vec_push(&g->io, m);
        if (g->turn == m)
        {
            sched_pick(g);
        }
        sched_unlock();
    }
    else
    {
        u64vec_remove_all(&g->io, m);
        sched_wait(m, g);
    }
}
