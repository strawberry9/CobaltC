/* Frames, statement scopes and objects: the frame/scope stack
   (`[Block-Exit]`, `[Stmt-Exit]`), paths' ends (`invalidate`, `drop_occ`,
   D-0018), objects and their roots (D-0292's light roots), destruction
   (`[Destroy]`, `[Run-Destructor]`, `[Destroy-Composite]`, the Box chain's
   and the tree's teardowns in constant stack, D-0298), `[Object-End]`, and
   raw storage: `[Reclaim]`, element borrows, the live checks and
   `[Release]` (D-0202's local entries by address). Part of lib.c's
   translation unit. */

#include <inttypes.h>
#include <stdio.h>

#include "rt.h"

void (*HANDLE_END)(uint64_t tid);
void (*THREADS_AT_END)(void);
void (*GUARD_END)(uintptr_t addr, uint64_t cells_off);

/* ---- small helpers ---- */

/* `Vec::remove(k)`: the order of the rest kept. */
static inline void u64vec_remove_at(u64vec *v, uint32_t k)
{
    memmove(&v->p[k], &v->p[k + 1], (size_t)(v->n - k - 1) * sizeof(uint64_t));
    v->n--;
}

/* `rposition(|&x| x == t)`: the last place of t, or -1. */
static inline int64_t u64vec_rposition(const u64vec *v, uint64_t t)
{
    for (uint32_t i = v->n; i-- > 0;)
    {
        if (v->p[i] == t)
        {
            return (int64_t)i;
        }
    }
    return -1;
}

static inline void toklist_free(toklist *l)
{
    u64vec_free(&l->v);
    if (l->at)
    {
        map_u64_free(l->at);
        free(l->at);
        l->at = NULL;
    }
}

static inline u64vecvec *cur_pool(Rt *r)
{
    return &r->pool[cur_heap()];
}

static inline projvecvec *cur_proj_pool(Rt *r)
{
    return &r->proj_pool[cur_heap()];
}

/* A fresh object's record (`Obj { .. }`), written into its arena place. */
static inline void obj_init(Obj *o, uintptr_t addr, uint32_t ty, bool init, bool is_resource, bool reclaimed, bool ephemeral)
{
    Obj v = {.addr = addr, .ty = ty, .alive = true, .init = init, .is_resource = is_resource, .reclaimed = reclaimed, .ephemeral = ephemeral, .frame = NO_IDX, .scope = NO_IDX};
    *o = v;
}

/* One line on standard error, written at once (Rust's `eprintln!`: the
   standard output's buffer is not written out first). */
static void eprint_line(const char *a, const char *b)
{
    size_t na = strlen(a), nb = strlen(b);
    char stackbuf[256];
    char *buf = na + nb + 1 <= sizeof stackbuf ? stackbuf : (char *)cb_xmalloc(na + nb + 1);
    memcpy(buf, a, na);
    memcpy(buf + na, b, nb);
    buf[na + nb] = '\n';
    fwrite(buf, 1, na + nb + 1, stderr);
    fflush(stderr);
    if (buf != stackbuf)
    {
        free(buf);
    }
}

/* The cores (below): each is inlined into an entry's lone path (`L` a
   constant true: rt.h) and once into the general function of its name. */
static void end_identity_rec(Rt *r, Obj *o, uint64_t obj, TypeInfo *ti, bool forget);
CB_ALWAYS void end_identity_core(bool L, Rt *r, Obj *o, uint64_t obj, TypeInfo *ti, bool forget);
CB_ALWAYS void retire_core(bool L, Rt *r, PathRec *p, uint64_t t);
CB_ALWAYS void destroy_core(bool L, Rt *r, Obj *o, uint64_t obj, uint64_t root, bool tail);
CB_ALWAYS void end_owned_core(bool L, u64vec owned);
CB_ALWAYS void end_temps_core(bool L, const uint64_t *temps, size_t n);
static void retire_lone(Rt *r, PathRec *p, uint64_t t);
static void destroy_lone(Rt *r, Obj *o, uint64_t obj, uint64_t root);
CB_ALWAYS void end_scope_core(bool L, Scope s);
static inline void list_take(u64vec *v, uint64_t x);
static uint64_t elem_borrow_lone(void *addr, uint32_t ty, uint32_t mode, bool stamp);

/* ---- frames and scopes ---- */

/* A frame on top of this thread's stack: one entry, which owns nothing yet
   (D-0202: the thread's own stack: no lock). */
static inline void frame_fill(Entry *e, bool rec, uint32_t file, uint32_t line)
{
    e->kind = E_FRAME;
    memset(&e->u.owned, 0, sizeof e->u.owned);
    e->u.floc.f = file;
    e->u.floc.l = line;
    e->rec = rec;
}

/* `block`: a block's frame, which records what its function's does
   (D-0292): `rec` is then the innermost frame's. */
static __attribute__((noinline)) void frame_push_general(bool block, bool rec, uint32_t file, uint32_t line)
{
    if (UNLIKELY(replay_on()))
    {
        replay_frame_push(me());
    }
    HELD_SCOPE h = own_heap();
    if (block)
    {
        rec = top_rec(ts_raw());
    }
    frame_fill(entry_push(ts()), rec, file, line);
}

CB_ALWAYS void frame_push_at(bool block, bool rec, uint32_t file, uint32_t line)
{
    /* The lone thread, room on the stack. */
    if (LIKELY(lone()))
    {
        TState *t = &MAIN_TS;
        if (t->pend.n != 0)
        {
            make_pending(t);
        }
        if (LIKELY(t->stack.n < t->stack.cap))
        {
            if (block)
            {
                rec = top_rec(t);
            }
            frame_fill(&t->stack.p[t->stack.n++], rec, file, line);
            return;
        }
    }
    frame_push_general(block, rec, file, line);
}

void cb_frame_push(void)
{
    frame_push_at(true, false, NOLOC, 0);
}

/* D-0292: a function's frame. `cb_frame_push_rec`: the static pass left a
   conflict of this body to the run, so every reference it forms, and every
   one that arrives in it, is recorded (`cb_rec_ref`). `cb_frame_push_fn`:
   a decided body, whose references are light. */
void cb_frame_push_fn(void)
{
    frame_push_at(false, false, NOLOC, 0);
}

void cb_frame_push_rec(void)
{
    frame_push_at(false, true, NOLOC, 0);
}

void frame_push_with(bool rec)
{
    frame_push_general(false, rec, NOLOC, 0);
}

/* `COBALTC_RECORD=check`: a light reference where a recorded one was due. */
CB_COLD void check_light(const char *what, uint64_t tok, uint32_t file, uint32_t line)
{
    if (atomic_load_explicit(&RECORD_CHECK, memory_order_relaxed))
    {
        Rt *r = rt();
        if (file < r->nfiles)
        {
            fprintf(stderr, "cbrt check: a light reference (0x%" PRIx64 ") reached %s in a recording frame at %s:%" PRIu32 "\n", tok, what, r->files[file], line);
        }
        else
        {
            fprintf(stderr, "cbrt check: a light reference (0x%" PRIx64 ") reached %s in a recording frame at ?:%" PRIu32 "\n", tok, what, line);
        }
        fflush(stderr);
        plat_abort();
    }
}

/* A block's frame whose end reports `file:line` for a destruction fault. */
void cb_frame_push_at(uint32_t file, uint32_t line)
{
    frame_push_at(true, false, file, line);
}

static void frame_pop_general(void);

void cb_frame_pop(void)
{
    /* The lone thread's frame that never owned anything. */
    if (LIKELY(lone()))
    {
        TState *t = &MAIN_TS;
        if (LIKELY(t->pend.n == 0) && LIKELY(t->stack.n != 0))
        {
            Entry *last = &t->stack.p[t->stack.n - 1];
            if (LIKELY(last->kind == E_FRAME))
            {
                if (LIKELY(last->u.owned.n == 0))
                {
                    t->stack.n--;
                    if (UNLIKELY(last->u.owned.cap != 0))
                    {
                        give_vec(&RT.pool[0], last->u.owned);
                    }
                    return;
                }
                /* It owns something: code that filled FULL gave it that. */
                if (LIKELY(HOOK(FULL.frame_pop_owning) != NULL))
                {
                    HOOK(FULL.frame_pop_owning)();
                    return;
                }
            }
        }
    }
    frame_pop_general();
}

static __attribute__((noinline)) void frame_pop_general(void)
{
    if (UNLIKELY(replay_on()))
    {
        replay_frame_pop(me());
    }
    /* D-0202: a frame that owns nothing ends nothing: no lock. */
    TState *t = ts();
    Entry *last = entryvec_last(&t->stack);
    if (LIKELY(last && last->kind == E_FRAME && last->u.owned.n == 0))
    {
        HELD_SCOPE h = own_heap();
        t->stack.n--;
        if (last->u.owned.cap)
        {
            give_vec(cur_pool(rt()), last->u.owned);
        }
        return;
    }
    /* A frame that owns something was given it by code that filled FULL. */
    if (HOOK(FULL.frame_pop_owning))
    {
        HOOK(FULL.frame_pop_owning)();
        return;
    }
    cbrt_internal(t->stack.n == 0 ? "frame pop on an empty stack" : "frame pop closed a statement scope");
}

/* `cb_frame_pop` for a frame that owns something. */
CB_ALWAYS void frame_pop_owning_core(bool L)
{
    TState *t = ts_(L);
    if (t->stack.n == 0)
    {
        cbrt_internal("frame pop on an empty stack");
    }
    Entry en = t->stack.p[--t->stack.n];
    if (en.kind != E_FRAME)
    {
        cbrt_internal("frame pop closed a statement scope");
    }
    loc_t saved = t->dloc;
    t->dloc = en.u.floc;
    if (L)
    {
        end_owned_core(true, en.u.owned);
    }
    else
    {
        end_owned(en.u.owned);
    }
    t->dloc = saved;
}

static void frame_pop_owning(void)
{
    if (LIKELY(lone()))
    {
        frame_pop_owning_core(true);
        return;
    }
    HELD_SCOPE e = enter_frame_end();
    frame_pop_owning_core(false);
}

/* A reference binding with no object (`cobc`'s held locals): the frame
   holds its path, as the binding's object would, until the frame ends or
   the binding ends early (`cb_frame_unhold`, D-0111). */
void cb_frame_hold(uint64_t tok)
{
    /* No path (a pathless reference, CHG-0237; a light token, D-0292):
       nothing to hold. */
    if (tok == 0 || (tok & LIGHT) != 0)
    {
        return;
    }
    replay_frame_hold(me(), tok);
    /* D-0204: the path's record and this thread's own frame: local. */
    HELD_SCOPE e = enter_on(heap_of(tok));
    Rt *r = rt();
    PathRec *p = paths_get(r, tok);
    if (!p)
    {
        return;
    }
    p->occ++;
    TState *t = ts();
    for (size_t i = t->stack.n; i-- > 0;)
    {
        if (t->stack.p[i].kind == E_FRAME)
        {
            owned_push(r, &t->stack.p[i].u.owned, tok | HOLD);
            break;
        }
    }
}

static void unhold_plan(Plan *p, Rt *r, void *ctx)
{
    plan_drop_occ(p, r, *(const uint64_t *)ctx);
}

void cb_frame_unhold(uint64_t tok)
{
    /* No path (a pathless reference; a static's or a constant's, D-0258; a
       light token, D-0292): nothing was held. */
    if (tok == 0 || (tok & LIGHT) != 0)
    {
        return;
    }
    replay_frame_unhold(me(), tok);
    HELD_SCOPE e = enter_planned(unhold_plan, &tok);
    TState *t = ts();
    for (size_t i = t->stack.n; i-- > 0;)
    {
        Entry *en = &t->stack.p[i];
        if (en->kind == E_FRAME)
        {
            int64_t k = u64vec_rposition(&en->u.owned, tok | HOLD);
            if (k >= 0)
            {
                u64vec_remove_at(&en->u.owned, (uint32_t)k);
                drop_occ(rt(), tok);
                return;
            }
        }
    }
}

void cb_stmt_push(void)
{
    if (UNLIKELY(replay_on()))
    {
        replay_stmt_push(me());
    }
    /* Pending until the runtime next looks (`ts`): this thread's own state
       only, so no lock (D-0196). */
    loc_t l = {NOLOC, 0};
    locvec_push(&ts_raw()->pend, l);
}

static __attribute__((noinline)) void stmt_push_general(loc_t l)
{
    if (UNLIKELY(replay_on()))
    {
        replay_stmt_push(me());
    }
    locvec_push(&ts_raw()->pend, l);
}

/* A statement scope whose end reports `file:line` for a destruction fault,
   as `cb_frame_push_at` does for a frame. */
void cb_stmt_push_at(uint32_t file, uint32_t line)
{
    loc_t l = {file, line};
    if (LIKELY(lone()) && LIKELY(MAIN_TS.pend.n < MAIN_TS.pend.cap))
    {
        MAIN_TS.pend.p[MAIN_TS.pend.n++] = l;
        return;
    }
    stmt_push_general(l);
}

static void stmt_pop_general(void);

/* The lone thread's made scope ends: one that holds nothing, or its
   temporaries and paths end (`end_scope`). */
static __attribute__((noinline)) void stmt_pop_lone_made(TState *t)
{
    if (UNLIKELY(t->stack.n == 0) || UNLIKELY(t->scopes.n == 0) || UNLIKELY(t->stack.p[t->stack.n - 1].kind != E_SCOPE))
    {
        stmt_pop_general();
        return;
    }
    Scope s = t->stack.p[--t->stack.n].u.s;
    t->scopes.n--;
    if (s.temps.n == 0 && s.toks.n == 0)
    {
        if (s.temps.cap)
        {
            give_vec_inl(&RT.pool[0], s.temps);
        }
        if (s.toks.cap)
        {
            give_vec_inl(&RT.pool[0], s.toks);
        }
        return;
    }
    loc_t saved = t->dloc;
    if (s.loc.f != NOLOC)
    {
        t->dloc = s.loc;
    }
    end_scope_core(true, s);
    t->dloc = saved;
}

void cb_stmt_pop(void)
{
    if (LIKELY(lone()) && LIKELY(atomic_load_explicit(&VERIFY_STRIDE, memory_order_relaxed) == 0))
    {
        /* A scope never made held nothing: popping it ends nothing. */
        TState *t = &MAIN_TS;
        if (LIKELY(t->pend.n != 0))
        {
            t->pend.n--;
            return;
        }
        /* A made scope that never held anything (a block's frame was
           pushed inside its statement): the entry goes. */
        if (LIKELY(t->stack.n != 0) && LIKELY(t->scopes.n != 0))
        {
            Entry *e = &t->stack.p[t->stack.n - 1];
            if (e->kind == E_SCOPE && (e->u.s.temps.n | e->u.s.toks.n | e->u.s.temps.cap | e->u.s.toks.cap) == 0)
            {
                t->stack.n--;
                t->scopes.n--;
                return;
            }
        }
        stmt_pop_lone_made(t);
        return;
    }
    stmt_pop_general();
}

static __attribute__((noinline)) void stmt_pop_general(void)
{
    if (UNLIKELY(replay_on()))
    {
        replay_stmt_pop(me());
    }
    /* D-0261: with `COBALTC_VERIFY`, the whole state is checked at every
       statement's end. */
    if (UNLIKELY(atomic_load_explicit(&VERIFY_STRIDE, memory_order_relaxed) != 0) && verify_on())
    {
        verify_state();
    }
    /* A scope never made held nothing: popping it ends nothing. */
    TState *t = ts_raw();
    if (t->pend.n)
    {
        t->pend.n--;
        return;
    }
    /* D-0202: a made scope that holds nothing ends nothing: no lock. */
    Entry *last = entryvec_last(&t->stack);
    if (last && last->kind == E_SCOPE && last->u.s.temps.n == 0 && last->u.s.toks.n == 0)
    {
        HELD_SCOPE h = own_heap();
        if (t->scopes.n)
        {
            t->scopes.n--;
        }
        t->stack.n--;
        if (last->u.s.temps.cap)
        {
            give_vec(cur_pool(rt()), last->u.s.temps);
        }
        if (last->u.s.toks.cap)
        {
            give_vec(cur_pool(rt()), last->u.s.toks);
        }
        return;
    }
    HELD_SCOPE e = enter_scope_end();
    t = ts();
    if (t->stack.n == 0)
    {
        cbrt_internal("statement pop on an empty stack");
    }
    Entry en = t->stack.p[--t->stack.n];
    if (en.kind != E_SCOPE)
    {
        cbrt_internal("statement pop closed a frame");
    }
    if (t->scopes.n)
    {
        t->scopes.n--;
    }
    if (en.u.s.loc.f == NOLOC)
    {
        end_scope(en.u.s);
    }
    else
    {
        loc_t saved = t->dloc;
        t->dloc = en.u.s.loc;
        end_scope(en.u.s);
        ts()->dloc = saved;
    }
}

/* `[Stmt-Exit]`: the scope's temporaries end, and every path formed in it
   that no object holds ends too (`Unheld`); a held path lives on with its
   holder. */
CB_ALWAYS void end_scope_core(bool L, Scope s)
{
    end_temps_core(L, s.temps.p, s.temps.n);
    Rt *r = rt();
    for (uint32_t i = 0; i < s.toks.n; i++)
    {
        uint64_t t = s.toks.p[i];
        PathRec *p = paths_get_inl(r, t);
        if (!p)
        {
            continue;
        }
        if (p->occ != 0)
        {
            p->scope = NO_IDX;
        }
        else if (L)
        {
            retire_lone(r, p, t);
        }
        else
        {
            retire_token(r, t);
        }
    }
    if (s.temps.cap)
    {
        give_vec_inl(&r->pool[HEAP_(L)], s.temps);
    }
    if (s.toks.cap)
    {
        give_vec_inl(&r->pool[HEAP_(L)], s.toks);
    }
}

void end_scope(Scope s)
{
    end_scope_core(false, s);
}

/* A path stops being valid. One that nothing holds is removed at once (any
   later use is `diag.stale-binding` either way, and keeping dead paths made
   every check on a long-lived object scan all of them); a held one is
   removed when its last holder lets go (`drop_occ`). */
void invalidate(Rt *r, uint64_t tok)
{
    PathRec *p = paths_get(r, tok);
    if (!p)
    {
        return;
    }
    if (p->occ == 0)
    {
        retire_token(r, tok);
    }
    else
    {
        p->valid = false;
    }
}

/* One fewer slot holds `tok`. A path its last holder let go of, owned by no
   statement scope and not in flight, is unreachable: it ends now (D-0018). */
void drop_occ(Rt *r, uint64_t tok)
{
    PathRec *p = paths_get(r, tok);
    if (!p)
    {
        return;
    }
    if (p->occ)
    {
        p->occ--;
    }
    if (p->occ == 0 && (!p->valid || (p->scope == NO_IDX && !p->is_root)))
    {
        if (UNLIKELY(replay_on()))
        {
            replay_dead(tok);
        }
        retire_token(r, tok);
    }
}

/* `retire_token` with the path's record in hand. */
CB_ALWAYS void retire_core(bool L, Rt *r, PathRec *p, uint64_t t)
{
    uint64_t of = p->of;
    bool exclusive = p->exclusive, is_root = p->is_root;
    if (p->proj.cap)
    {
        give_proj(&r->proj_pool[HEAP_(L)], p->proj);
    }
    if (p->ancestors.cap)
    {
        give_vec_inl(&r->pool[HEAP_(L)], p->ancestors);
    }
    projvec_free(&p->lock_of);
    PathArena_drop(&r->paths.h[t >> HEAP_SHIFT], t);
    Obj *o = objs_get_inl(r, of);
    if (!o)
    {
        return;
    }
    toklist_remove(&o->toks, t);
    if (exclusive && !is_root)
    {
        toklist_remove(&o->xtoks, t);
    }
    uint32_t left = o->toks.v.n;
    if (left == 0)
    {
        set_mark(of, false);
    }
    /* Its root ended: the object keeps no name for a dead path, so a
       later `[Reclaim]` of its cells mints a new root (`cb_reclaim`). */
    if (o->root == t)
    {
        o->root = 0;
    }
    /* An ephemeral element object left with only its root path is
       unobservable: it ends now rather than at `[Release]`. This keeps a
       loop over a Vec from holding one object per element it touched. Its
       type holds no reference: no slot to forget. */
    if (!is_root && o->ephemeral)
    {
        bool unobserved;
        if (o->root == 0 || (o->root & LIGHT) != 0)
        {
            unobserved = left == 0;
        }
        else
        {
            unobserved = left == 1 && o->toks.v.p[0] == o->root;
        }
        if (unobserved)
        {
            if (L)
            {
                end_identity_core(true, r, o, of, rt_typ(r, o->ty), false);
            }
            else
            {
                end_identity_rec(r, o, of, rt_typ(r, o->ty), false);
            }
        }
    }
}

void retire_token(Rt *r, uint64_t t)
{
    PathRec *p = paths_get(r, t);
    if (p)
    {
        retire_core(false, r, p, t);
    }
}

static __attribute__((noinline)) void retire_lone(Rt *r, PathRec *p, uint64_t t)
{
    retire_core(true, r, p, t);
}

u64vec *scope_toks(Rt *r, size_t i)
{
    (void)r;
    TState *t = ts();
    if (i < t->stack.n && t->stack.p[i].kind == E_SCOPE)
    {
        return &t->stack.p[i].u.s.toks;
    }
    return NULL;
}

/* `[Block-Exit]`: owned objects, last established first. A frame's list
   holds objects and, marked by `HOLD`, paths it holds for a reference
   binding with no object (`cb_frame_hold`). */
CB_ALWAYS void end_owned_core(bool L, u64vec owned)
{
    Rt *r = rt();
    for (uint32_t i = owned.n; i-- > 0;)
    {
        uint64_t obj = owned.p[i];
        if (obj & HOLD)
        {
            drop_occ(r, obj & ~HOLD);
            continue;
        }
        Obj *o = objs_get_inl(r, obj);
        if (!o || !o->alive)
        {
            continue;
        }
        if (L)
        {
            destroy_lone(r, o, obj, o->root);
        }
        else
        {
            destroy_obj_tail(obj, o->root, false);
        }
    }
    if (owned.cap)
    {
        give_vec_inl(&rt()->pool[HEAP_(L)], owned);
    }
}

void end_owned(u64vec owned)
{
    end_owned_core(false, owned);
}

/* `[Stmt-Exit]`: temporaries not adopted, sent, or re-stamped outward. */
CB_ALWAYS void end_temps_core(bool L, const uint64_t *temps, size_t n)
{
    for (size_t i = n; i-- > 0;)
    {
        uint64_t obj = temps[i];
        Rt *r = rt();
        Obj *o = objs_get_inl(r, obj);
        if (o && o->alive && o->frame == NO_IDX && o->root == 0)
        {
            if (L)
            {
                destroy_lone(r, o, obj, 0);
            }
            else
            {
                destroy_obj_tail(obj, 0, false);
            }
        }
    }
}

void end_temps(const uint64_t *temps, size_t n)
{
    end_temps_core(false, temps, n);
}

size_t top_frame(Rt *r)
{
    (void)r;
    TState *t = ts();
    for (size_t i = t->stack.n; i-- > 0;)
    {
        if (t->stack.p[i].kind == E_FRAME)
        {
            return i;
        }
    }
    return NO_IDX;
}

/* The statement scope nearest below position `i` of the stack. */
size_t scope_below(size_t i)
{
    sizevec *sc = &ts()->scopes;
    size_t lo = 0, hi = sc->n;
    while (lo < hi)
    {
        size_t m = lo + (hi - lo) / 2;
        if (sc->p[m] < i)
        {
            lo = m + 1;
        }
        else
        {
            hi = m;
        }
    }
    return lo == 0 ? NO_IDX : sc->p[lo - 1];
}

size_t top_scope(Rt *r)
{
    (void)r;
    TState *t = ts();
    return t->scopes.n ? t->scopes.p[t->scopes.n - 1] : NO_IDX;
}

/* `detach` with the object's record in hand. */
CB_ALWAYS void detach_core(bool L, Obj *o, uint64_t obj)
{
    if (o->frame == NO_IDX && o->scope == NO_IDX)
    {
        return;
    }
    /* Only the owning thread edits its own stack; an index into another
       thread's stack is never followed. */
    if (o->tid == ME_(L))
    {
        TState *t = ts_(L);
        size_t frame = o->frame, scope = o->scope;
        if (frame != NO_IDX && frame < t->stack.n && t->stack.p[frame].kind == E_FRAME)
        {
            list_take(&t->stack.p[frame].u.owned, obj);
        }
        if (scope != NO_IDX && scope < t->stack.n && t->stack.p[scope].kind == E_SCOPE)
        {
            list_take(&t->stack.p[scope].u.s.temps, obj);
        }
    }
    o->frame = NO_IDX;
    o->scope = NO_IDX;
}

static void detach_obj(Obj *o, uint64_t obj)
{
    detach_core(false, o, obj);
}

void detach(Rt *r, uint64_t obj)
{
    Obj *o = objs_get(r, obj);
    if (o)
    {
        detach_obj(o, obj);
    }
}

void stamp_temp(Rt *r, uint64_t obj, size_t scope)
{
    if (scope != NO_IDX)
    {
        TState *t = ts();
        if (scope < t->stack.n && t->stack.p[scope].kind == E_SCOPE)
        {
            Scope *s = &t->stack.p[scope].u.s;
            if (s->temps.cap == 0)
            {
                s->temps = take_vec(cur_pool(r));
            }
            u64vec_push(&s->temps, obj);
        }
    }
    Obj *o = objs_at(r, obj);
    o->scope = scope;
    if (scope != NO_IDX)
    {
        o->tid = me();
    }
}

/* `x` off a frame's or a scope's list: it is there once, most often last. */
static inline void list_take(u64vec *v, uint64_t x)
{
    if (LIKELY(v->n != 0) && LIKELY(v->p[v->n - 1] == x))
    {
        v->n--;
        return;
    }
    int64_t k = u64vec_rposition(v, x);
    if (k >= 0)
    {
        u64vec_remove_at(v, (uint32_t)k);
    }
}

void detach_tok(Rt *r, uint64_t tok)
{
    PathRec *p = paths_get(r, tok);
    if (!p)
    {
        return;
    }
    size_t scope = p->scope;
    if (scope != NO_IDX)
    {
        u64vec *v = scope_toks(r, scope);
        if (v)
        {
            list_take(v, tok);
        }
    }
    p->scope = NO_IDX;
}

/* `stamp_tok` with the path's record in hand (NULL: none). */
static inline void stamp_tok_rec(Rt *r, PathRec *p, uint64_t tok, size_t scope)
{
    if (scope != NO_IDX)
    {
        TState *t = ts();
        if (scope < t->stack.n && t->stack.p[scope].kind == E_SCOPE)
        {
            u64vec *v = &t->stack.p[scope].u.s.toks;
            if (v->cap == 0)
            {
                *v = take_vec(cur_pool(r));
            }
            u64vec_push_inl(v, tok);
        }
    }
    if (p)
    {
        p->scope = scope;
    }
}

void stamp_tok(Rt *r, uint64_t tok, size_t scope)
{
    stamp_tok_rec(r, paths_get(r, tok), tok, scope);
}

/* ---- objects ---- */

CB_ALWAYS uint64_t new_obj_core(bool L, uintptr_t addr, uint32_t ty, bool init)
{
    Rt *r = rt();
    full_on();
    uint64_t id = ObjArena_reserve(&r->objs.h[HEAP_(L)]);
    if (UNLIKELY((size_t)(uint32_t)id - 1 >= MARK_LEN[(id >> HEAP_SHIFT) & 0xff]))
    {
        ensure_mark(id);
    }
    TypeInfo *ti = rt_typ_opt(r, ty);
    bool is_resource = ti && ti->is_resource;
    Obj *o = objs_insert(r, id);
    obj_init(o, addr, ty, init, is_resource, false, false);
    /* A temporary of the innermost statement scope (`stamp_temp`). */
    TState *t = ts_(L);
    if (t->scopes.n)
    {
        size_t scope = t->scopes.p[t->scopes.n - 1];
        if (scope < t->stack.n && t->stack.p[scope].kind == E_SCOPE)
        {
            Scope *s = &t->stack.p[scope].u.s;
            if (s->temps.cap == 0)
            {
                s->temps = take_vec_inl(&r->pool[HEAP_(L)]);
            }
            u64vec_push_inl(&s->temps, id);
        }
        o->scope = scope;
        o->tid = ME_(L);
    }
    if (TRACED_(L))
    {
        replay_establish(me(), id, is_resource, addr, ti ? ti->size : 0);
    }
    return id;
}

uint64_t new_obj(uintptr_t addr, uint32_t ty, bool init)
{
    return new_obj_core(false, addr, ty, init);
}

/* `cb_new` with threads or under the trace (apart, so that the entry
   itself keeps no frame for what this holds). */
static __attribute__((noinline)) uint64_t new_general(void *addr, uint32_t ty)
{
    /* D-0202: a new object in this thread's heap, in its own scope. */
    HELD_SCOPE h = enter_on(cur_heap());
    return new_obj((uintptr_t)addr, ty, true);
}

uint64_t cb_new(void *addr, uint32_t ty, uint32_t file, uint32_t line)
{
    (void)file;
    (void)line;
    if (LIKELY(lone()))
    {
        return new_obj_core(true, (uintptr_t)addr, ty, true);
    }
    return new_general(addr, ty);
}

uint64_t cb_new_uninit(void *addr, uint32_t ty, uint32_t file, uint32_t line)
{
    (void)file;
    (void)line;
    HELD_SCOPE h = enter_on(cur_heap());
    return new_obj((uintptr_t)addr, ty, false);
}

uint64_t mint(Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors)
{
    return mint_locked(r, of, proj, exclusive, is_root, ancestors, NULL);
}

uint64_t mint_locked(Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors, projvec *lock_of)
{
    return mint_in(r, of, proj, exclusive, is_root, ancestors, lock_of, !is_root);
}

/* `stamp`: record a derived path in the current statement scope. A path
   that leaves for the caller at once (`cb_elem_borrow`) is not stamped:
   `cb_recv_ref` stamps it into the caller's scope. */
CB_ALWAYS uint64_t mint_core(bool L, Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors, projvec *lock_of, bool stamp)
{
    ph3_count(is_root ? &N_ROOT_REC : &N_PATHS);
    /* D-0202: a path lives in its object's heap. */
    uint64_t tok = paths_reserve_in(r, (size_t)(of >> HEAP_SHIFT));
    PathRec *p = paths_insert(r, tok);
    p->of = of;
    p->proj = proj;
    p->exclusive = exclusive;
    p->valid = true;
    p->is_root = is_root;
    p->ancestors = ancestors;
    p->occ = 0;
    p->scope = NO_IDX;
    if (lock_of)
    {
        p->has_lock_of = true;
        p->lock_of = *lock_of;
    }
    else
    {
        p->has_lock_of = false;
        memset(&p->lock_of, 0, sizeof p->lock_of);
    }
    atomic_init(&p->thread, ME_(L));
    Obj *o = objs_get_inl(r, of);
    if (o)
    {
        if (o->toks.v.cap == 0 && !o->toks.at)
        {
            o->toks = toklist_from_vec(take_vec_inl(&r->pool[HEAP_(L)]));
        }
        toklist_push(&o->toks, tok);
        if (exclusive && !is_root)
        {
            if (o->xtoks.v.cap == 0 && !o->xtoks.at)
            {
                o->xtoks = toklist_from_vec(take_vec_inl(&r->pool[HEAP_(L)]));
            }
            toklist_push(&o->xtoks, tok);
        }
        set_mark(of, true);
    }
    if (stamp)
    {
        /* Into the innermost statement scope (`stamp_tok`). */
        TState *t = ts_(L);
        size_t scope = t->scopes.n ? t->scopes.p[t->scopes.n - 1] : NO_IDX;
        if (scope != NO_IDX && scope < t->stack.n && t->stack.p[scope].kind == E_SCOPE)
        {
            u64vec *v = &t->stack.p[scope].u.s.toks;
            if (v->cap == 0)
            {
                *v = take_vec_inl(&r->pool[HEAP_(L)]);
            }
            u64vec_push_inl(v, tok);
        }
        p->scope = scope;
    }
    return tok;
}

uint64_t mint_in(Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors, projvec *lock_of, bool stamp)
{
    return mint_core(false, r, of, proj, exclusive, is_root, ancestors, lock_of, stamp);
}

/* D-0292: an object's root token: light (`LIGHT | ROOT | obj`, no path
   record; valid while it is the object's `root`) unless
   `COBALTC_RECORD=full` asks for the recorded root path of before phase 3. */
uint64_t make_root(Rt *r, uint64_t obj)
{
    if (atomic_load_explicit(&RECORD_FULL, memory_order_relaxed))
    {
        projvec np = {0};
        u64vec na = {0};
        return mint(r, obj, np, true, true, na);
    }
    ph3_count(&N_ROOT_LIGHT);
    return obj | LIGHT | ROOT;
}

CB_ALWAYS uint64_t bind_core(bool L, uint64_t obj)
{
    Rt *r = rt();
    Obj *o = objs_get_inl(r, obj);
    if (o)
    {
        detach_core(L, o, obj);
    }
    TState *t = ts_(L);
    size_t fi = t->stack.n;
    do
    {
        if (fi == 0)
        {
            cbrt_internal("a binding outside any frame");
        }
        fi--;
    } while (t->stack.p[fi].kind != E_FRAME);
    u64vec *owned = &t->stack.p[fi].u.owned;
    if (UNLIKELY(owned->cap == 0))
    {
        *owned = take_vec_inl(&r->pool[HEAP_(L)]);
    }
    u64vec_push_inl(owned, obj);
    uint64_t tok = make_root(r, obj);
    if (UNLIKELY(!o))
    {
        cbrt_internal("no entry under this id");
    }
    o->frame = fi;
    o->tid = ME_(L);
    o->root = tok;
    if (TRACED_(L))
    {
        replay_bind(me(), obj, tok);
    }
    return tok;
}

static __attribute__((noinline)) uint64_t bind_general(uint64_t obj)
{
    /* D-0202: the object's records and its root path (in its heap), and
       this thread's frame. */
    HELD_SCOPE h = enter_on(heap_of(obj));
    return bind_core(false, obj);
}

uint64_t cb_bind(uint64_t obj)
{
    if (LIKELY(lone()))
    {
        return bind_core(true, obj);
    }
    return bind_general(obj);
}

/* D-0033 (1): the frame the calling block binds into (`cb_bind`), recorded
   for a binding the program may give a new value after its old one moved
   away or ended. */
uint64_t cb_frame_here(void)
{
    ENTERED_SCOPE e = enter();
    size_t f = top_frame(rt());
    if (f == NO_IDX)
    {
        cbrt_internal("a binding outside any frame");
    }
    return (uint64_t)f;
}

static bool rebind_live(Rt *r, uint64_t root, uintptr_t addr, bool need_held)
{
    PathView v;
    if (!view(r, root, &v))
    {
        return false;
    }
    if (need_held && !holds_heap(heap_of(v.of)))
    {
        return false;
    }
    Obj *o = objs_get(r, v.of);
    return o && o->alive && (v.rec != NULL || o->addr == addr);
}

/* `x = e` for a whole binding: while `root` still reaches a live object,
   `root` itself; otherwise a new, uninitialized object at `addr`, bound in
   `frame` (x's own, from `cb_frame_here`) as `cb_bind` binds one, and its
   root. D-0292: a light root is the object's current root, so the binding
   still owns its object only while the object is at the binding's own
   address. */
uint64_t cb_rebind(uint64_t root, void *addr, uint32_t ty, uint64_t frame, uint32_t file, uint32_t line)
{
    (void)file;
    (void)line;
    replay_out(me(), "a binding re-established after a move");
    /* D-0204: the common case asked under a local hold of the root's heap;
       only a new object's binding enters exclusively. */
    if (multi())
    {
        HELD_SCOPE h = enter_on(heap_of(root));
        if (rebind_live(rt(), root, (uintptr_t)addr, true))
        {
            return root;
        }
    }
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    if (rebind_live(r, root, (uintptr_t)addr, false))
    {
        return root;
    }
    uint64_t obj = new_obj((uintptr_t)addr, ty, false);
    r = rt();
    detach(r, obj);
    size_t fr = (size_t)frame;
    TState *t = ts();
    if (fr >= t->stack.n)
    {
        cbrt_internal("a rebinding into a frame that is not on the stack");
    }
    if (t->stack.p[fr].kind == E_FRAME)
    {
        owned_push(r, &t->stack.p[fr].u.owned, obj);
    }
    uint64_t tok = make_root(r, obj);
    Obj *o = objs_at(r, obj);
    o->frame = fr;
    o->tid = me();
    o->root = tok;
    return tok;
}

/* Re-keys the slots inside an object's old extent to its new address. */
void rekey_slots(Rt *r, uintptr_t old, uintptr_t new_, uint64_t size)
{
    if (old == new_ || size == 0)
    {
        return;
    }
    /* Into the buffer `forget_slots` keeps; a nested use, through
       `drop_occ`, takes a new one. */
    size_t h = cur_heap();
    atvec moved = r->slot_buf[h];
    memset(&r->slot_buf[h], 0, sizeof(atvec));
    slots_scan_into(r, old, old + (uintptr_t)size, true, &moved);
    for (size_t i = 0; i < moved.n; i++)
    {
        uintptr_t a = moved.p[i].a;
        uint64_t t = moved.p[i].t;
        /* A slot already at the destination stops holding what it held:
           overwriting it silently left that path counted as held forever. */
        uint64_t prev = slots_insert(r, new_ + (a - old), t);
        if (prev)
        {
            drop_occ(r, prev);
        }
    }
    moved.n = 0;
    r = rt();
    h = cur_heap();
    if (r->slot_buf[h].cap < moved.cap)
    {
        atvec_free(&r->slot_buf[h]);
        r->slot_buf[h] = moved;
    }
    else
    {
        atvec_free(&moved);
    }
}

typedef struct
{
    uint64_t obj;
    uintptr_t to;
} move_ctx;

static void move_plan(Plan *p, Rt *r, void *ctx)
{
    move_ctx *m = (move_ctx *)ctx;
    plan_move_obj(p, r, m->obj, m->to);
}

void cb_move_to(uint64_t obj, void *addr)
{
    replay_moved(obj, (uintptr_t)addr);
    move_ctx mc = {obj, (uintptr_t)addr};
    HELD_SCOPE e = enter_planned(move_plan, &mc);
    Rt *r = rt();
    Obj *o = objs_get(r, obj);
    if (!o)
    {
        return;
    }
    TypeInfo *t = rt_typ(r, o->ty);
    uintptr_t old = o->addr;
    uint64_t size = t->size;
    bool refs = t->has_refs, reclaimed = o->reclaimed, ephemeral = o->ephemeral;
    /* Only a reference slot is stored: a type that holds none has none to
       move. */
    if (refs)
    {
        rekey_slots(r, old, (uintptr_t)addr, size);
    }
    /* CHG-0349: an object reclaimed from raw cells no longer lies over
       them, so its entry there goes. `cb_raw_move_in` enters the object at
       its new cells itself. */
    if (reclaimed && old != (uintptr_t)addr)
    {
        reclaimed_remove_if(r, old, obj, ephemeral);
    }
    objs_at(r, obj)->addr = (uintptr_t)addr;
}

path_target_ret path_target(Rt *r, uint64_t tok, uint32_t file, uint32_t line)
{
    path_target_locked_ret x = path_target_locked(r, tok, file, line);
    projvec_free(&x.lock_of);
    path_target_ret y = {x.of, x.proj, x.excludes};
    return y;
}

path_target_locked_ret path_target_locked(Rt *r, uint64_t tok, uint32_t file, uint32_t line)
{
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
    path_target_locked_ret out;
    memset(&out, 0, sizeof out);
    out.of = v.of;
    if (!v.is_root)
    {
        u64vec_push(&out.excludes, tok);
        u64vec_extend(&out.excludes, v.ancestors, v.nancestors);
    }
    projvec_extend(&out.proj, v.proj, v.nproj);
    if (v.has_lock_of)
    {
        out.has_lock_of = true;
        projvec_extend(&out.lock_of, v.lock_of, v.nlock_of);
    }
    return out;
}

/* D-0292: what a token names, read the same way whatever its kind: a light
   root (`LIGHT | ROOT | obj`) is the object's own path with no projection,
   exclusive, no ancestors and no lock origin, valid while it is the
   object's current root; a light reference stands for the whole object
   (more clashes than the place has, never fewer); a path id is its valid
   record. false: stale. */
bool view(Rt *r, uint64_t tok, PathView *out)
{
    if (tok & LIGHT)
    {
        uint64_t obj = tok & ~TOK_FLAGS;
        Obj *o = objs_get(r, obj);
        if (!o)
        {
            return false;
        }
        if (tok & ROOT)
        {
            if (o->root != tok)
            {
                return false;
            }
        }
        else if (!o->alive)
        {
            return false;
        }
        memset(out, 0, sizeof *out);
        out->of = obj;
        out->exclusive = true;
        out->is_root = (tok & ROOT) != 0;
        out->rec = NULL;
        return true;
    }
    PathRec *p = paths_get(r, tok);
    if (!p || !p->valid)
    {
        return false;
    }
    out->of = p->of;
    out->proj = p->proj.p;
    out->nproj = p->proj.n;
    out->exclusive = p->exclusive;
    out->is_root = p->is_root;
    out->ancestors = p->ancestors.p;
    out->nancestors = p->ancestors.n;
    out->has_lock_of = p->has_lock_of;
    out->lock_of = p->lock_of.p;
    out->nlock_of = p->has_lock_of ? p->lock_of.n : 0;
    out->rec = p;
    return true;
}

uint64_t tok_obj(Rt *r, uint64_t tok)
{
    if (tok & LIGHT)
    {
        uint64_t obj = tok & ~TOK_FLAGS;
        return objs_get(r, obj) ? obj : 0;
    }
    PathRec *p = paths_get(r, tok);
    return p ? p->of : 0;
}

/* A path to a temporary for an access that needs one (calling a closure
   temporary): owner-like, like a root, but the object stays a temporary of
   its statement and ends there. */
uint64_t cb_temp_path(uint64_t obj)
{
    replay_out(me(), "a temporary's root path (a closure called)");
    /* D-0204: the object's records (its heap): local. */
    HELD_SCOPE e = enter_on(heap_of(obj));
    projvec np = {0};
    u64vec na = {0};
    return mint(rt(), obj, np, true, true, na);
}

uint64_t cb_take(uint64_t root, uint32_t file, uint32_t line)
{
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    path_target_ret pt = path_target(r, root, file, line);
    uint64_t obj = pt.of;
    projvec_free(&pt.proj);
    u64vec_free(&pt.excludes);
    /* [Authority-Transfer-Aliased]: nothing but the owner reaches it. */
    if (!solitary(r, obj, &root, 1))
    {
        fault("diag.move-while-aliased", file, line);
    }
    replay_take(me(), obj, root);
    invalidate(r, root);
    detach(r, obj);
    objs_at(r, obj)->root = 0;
    size_t scope = top_scope(r);
    stamp_temp(r, obj, scope);
    return obj;
}

static void end_obj_plan(Plan *p, Rt *r, void *ctx)
{
    plan_end_obj(p, r, *(const uint64_t *)ctx);
}

void cb_absorb(uint64_t obj)
{
    replay_out(me(), "a relocation into or out of a container");
    HELD_SCOPE e = enter_planned(end_obj_plan, &obj);
    Rt *r = rt();
    detach(r, obj);
    end_identity(r, obj);
}

/* `cb_absorb` for plain data holding references that has just moved into
   the container being built: the temporary's record ends, and the
   references it held stay in their slots, now the container's (`cobc`'s
   `store_part`). The temporary is fresh: no path reaches it. */
void cb_absorb_refs(uint64_t obj)
{
    replay_out(me(), "a relocation into or out of a container");
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    if (!objs_get(r, obj))
    {
        return;
    }
    detach(r, obj);
    end_identity_in(r, obj, false);
}

void cb_result(uint64_t obj)
{
    replay_out(me(), "a statement's result object re-stamped outward");
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    Obj *o = objs_get(r, obj);
    if (!o || !o->alive)
    {
        return;
    }
    size_t cur = o->scope;
    detach(r, obj);
    size_t parent = cur != NO_IDX ? scope_below(cur) : NO_IDX;
    stamp_temp(r, obj, parent);
}

void cb_destroy(uint64_t root, uint32_t file, uint32_t line)
{
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    path_target_ret pt = path_target(r, root, file, line);
    uint64_t obj = pt.of;
    projvec_free(&pt.proj);
    u64vec_free(&pt.excludes);
    /* A `std` body gives no location: a fault in the destruction is the
       caller's, where the location already is. */
    if (!(file == NOLOC && line == 0))
    {
        set_at(file, line);
    }
    Obj *o = objs_get(r, obj);
    replay_end(me(), obj, o ? o->is_resource : false);
    destroy_obj(obj, root);
}

/* `drop(*r)`: `[Destroy]` reached through a reference. The owner's own root
   is another valid path on the object, so it is never solitary while
   owned; an unowned temporary is destroyed. */
void cb_destroy_via(uint64_t tok, const void *addr, uint32_t ty, uint32_t file, uint32_t line)
{
    replay_out(me(), "a destruction through a reference");
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    /* D-0292: through a light reference: the referent's place, from its
       address and type. */
    if (light_ref(tok))
    {
        uint64_t obj = tok & ~TOK_FLAGS;
        Obj *o = objs_get(r, obj);
        if (!o || !o->alive)
        {
            fault("diag.stale-binding", file, line);
        }
        projvec proj = proj_of_addr(obj, (uintptr_t)addr, ty);
        bool whole = proj.n == 0;
        give_proj(cur_proj_pool(rt()), proj);
        if (!whole)
        {
            fault("diag.move-out-of-field", file, line);
        }
        r = rt();
        if (objs_at(r, obj)->root != 0 || !solitary(r, obj, NULL, 0))
        {
            fault("diag.destroy-while-aliased", file, line);
        }
        set_at(file, line);
        destroy_obj(obj, tok);
        return;
    }
    path_target_ret pt = path_target(r, tok, file, line);
    uint64_t obj = pt.of;
    bool field = pt.proj.n != 0;
    projvec_free(&pt.proj);
    if (field)
    {
        u64vec_free(&pt.excludes);
        fault("diag.move-out-of-field", file, line);
    }
    u64vec_push(&pt.excludes, tok);
    bool bad = objs_at(r, obj)->root != 0 || !solitary(r, obj, pt.excludes.p, pt.excludes.n);
    u64vec_free(&pt.excludes);
    if (bad)
    {
        fault("diag.destroy-while-aliased", file, line);
    }
    set_at(file, line);
    destroy_obj(obj, tok);
}

/* D-0111: a local reference binding ends after its last use, as at its
   block's exit: its object ends, and with it the path it holds. */
void cb_end_binding(uint64_t root)
{
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    uint64_t obj = tok_obj(r, root);
    if (!obj)
    {
        return;
    }
    Obj *o = objs_get(r, obj);
    replay_end(me(), obj, o ? o->is_resource : false);
    destroy_obj(obj, root);
}

void cb_consume(uint64_t obj, uint32_t file, uint32_t line)
{
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    Obj *o = objs_get(r, obj);
    if (o && o->alive)
    {
        set_at(file, line);
        o = objs_get(r, obj);
        replay_end(me(), obj, o ? o->is_resource : false);
        /* D-0339 (report 337): a value consumed by `std::Rc`'s destructor
           itself (its `overwrite` of the box's value) is in tail position,
           as `drop_in_place` takes it in a decided program. */
        destroy_obj_tail(obj, 0, ts_raw()->chain_top);
    }
}

void cb_end_moved_out(uint64_t obj)
{
    replay_out(me(), "a relocation into or out of a container");
    HELD_SCOPE e = enter_planned(end_obj_plan, &obj);
    Rt *r = rt();
    detach(r, obj);
    end_identity(r, obj);
}

/* `[Destroy]` for a resource, `[Object-End]` for a plain object. `root`:
   the root to exclude from the solitary check (0: none). */
void destroy_obj(uint64_t obj, uint64_t root)
{
    destroy_obj_tail(obj, root, false);
}

/* `destroy_obj`; `tail`: nothing observable follows it in the Box teardown
   that asked for it. */
CB_ALWAYS void destroy_core(bool L, Rt *r, Obj *o, uint64_t obj, uint64_t root, bool tail)
{
    TypeInfo *ti = rt_typ(r, o->ty);
    /* D-0327: an object never written (`[Let-Uninit]`'s, before its first
       assignment) holds no value: no destructor runs on it and nothing in
       it is destroyed; it ends as a plain object does (`[Block-Exit]`,
       `[Object-End]`). */
    if (o->init && (o->is_resource || ti->holds_fn))
    {
        uintptr_t addr = o->addr;
        uint32_t ty = o->ty;
        if (o->is_resource && !solitary(r, obj, &root, root ? 1 : 0))
        {
            /* Report 319 (`[Terminate-Exit]`): an `exit`'s unwind meets a
               value another thread still holds a reference to (lent by
               `spawn`): the other threads take no further steps, so the
               value is left as it is (its memory is the process's until
               it ends) and the status is the one `exit` gave. */
            if (EXIT_UNWINDING && multi())
            {
                return;
            }
            loc_t d = ts()->dloc;
            fault("diag.destroy-while-aliased", d.f, d.l);
        }
        /* A resource; or plain, but its `fn` values own their closures
           (D-0089). What it runs may move the tables. */
        drop_in_place_tail(addr, ty, obj, tail);
        r = rt();
        o = objs_get(r, obj);
        if (!o)
        {
            return;
        }
        ti = rt_typ(r, o->ty);
    }
    if (L)
    {
        detach_core(true, o, obj);
        end_identity_core(true, r, o, obj, ti, true);
    }
    else
    {
        detach_obj(o, obj);
        end_identity_rec(r, o, obj, ti, true);
    }
}

void destroy_obj_tail(uint64_t obj, uint64_t root, bool tail)
{
    Rt *r = rt();
    Obj *o = objs_get(r, obj);
    if (o && o->alive)
    {
        destroy_core(false, r, o, obj, root, tail);
    }
}

/* A live object's end on the lone thread. */
static __attribute__((noinline)) void destroy_lone(Rt *r, Obj *o, uint64_t obj, uint64_t root)
{
    destroy_core(true, r, o, obj, root, false);
}

/* The variant index of the enum value at `addr`: its discriminant, or for
   an enum laid out as its Box payload (D-0298) which of its two variants
   the pointer's being null or not names. */
size_t enum_tag(TypeInfo *info, uintptr_t addr)
{
    if (info->has_niche)
    {
        return *(const uintptr_t *)addr != 0 ? info->niche_some : info->niche_none;
    }
    return (size_t)*(const uint32_t *)addr;
}

/* Runs the destructor and destroys sub-resources of the value at `addr` of
   type `ty` (`[Run-Destructor]`, `[Destroy-Composite]`). `owner` is the
   object being destroyed, or 0 for a field. */
void drop_in_place(uintptr_t addr, uint32_t ty, uint64_t owner)
{
    /* Report 337: a value ended by `std::Rc`'s destructor itself (not by a
       destructor it runs) is in tail position: the destructor does nothing
       observable after it. */
    drop_in_place_tail(addr, ty, owner, ts_raw()->chain_top);
}

/* Report 337: the `Rc`s that waited for the destructor whose list started
   at `base`, each run in turn; one that waits in its run joins the list,
   so a chain of any length is destroyed in constant stack, in the same
   order as nested runs (each after what its enclosing destructor did
   last, which is unobservable). */
static void chain_drain(size_t base)
{
    for (;;)
    {
        TState *st = ts_raw();
        if (st->nchain <= base)
        {
            break;
        }
        chain_item_t it = st->chain[--st->nchain];
        st->chain_flat_next = true;
        drop_in_place_tail((uintptr_t)it.bytes, it.ty, 0, false);
        ts_raw()->chain_flat_next = false;
    }
    TState *st = ts_raw();
    if (base == 0 && st->chain != NULL)
    {
        free(st->chain);
        st->chain = NULL;
        st->capchain = 0;
    }
}

/* `drop_in_place`; `tail` as for `destroy_obj_tail`: what this destroys
   last is then in tail position too. */
void drop_in_place_tail(uintptr_t addr, uint32_t ty, uint64_t owner, bool tail)
{
    Rt *r = rt();
    size_t pos = rt_typ_pos(r, ty);
    TypeInfo *info = &r->types[pos];
    if (info->tree && !multi())
    {
        if (info->is_resource)
        {
            tree_drop(addr, pos);
        }
        return;
    }
    if (info->has_box_of)
    {
        box_drop(addr, info->box_of, tail);
        return;
    }
    if (info->kind == K_FN)
    {
        /* A fn-typed slot owns the closure box its handle names, if any. */
        drop_fn_handle(*(const uintptr_t *)addr);
        return;
    }
    if (!info->is_resource && !info->holds_fn)
    {
        return;
    }
    /* A thread handle is made by `cb_spawn` alone and a guard by `cb_lock`
       alone, and each names its value's end there (HANDLE_END, GUARD_END):
       a program that spawns nothing, or locks nothing, links neither. */
    if (info->kind == K_HANDLE)
    {
        HOOK(HANDLE_END)(*(const uint64_t *)addr);
        return;
    }
    if (info->kind == K_GUARD)
    {
        /* Report 315: a guard's descriptor carries the offset of its
           mutex's state cells (`payload_off`), for a guard ended by a
           thread other than the one that locked it. */
        HOOK(GUARD_END)(addr, info->payload_off);
        return;
    }
    if (info->has_vec_plain && info->drop)
    {
        /* cobc's native `Vec::drop` for a plain element: its checked read of
           `self.len` through the root just minted for it cannot fail, so
           that root and its object are not made; the element checks and the
           buffer's release are as the body makes them. */
        uint64_t esz = info->vec_plain;
        uintptr_t po = (uintptr_t)info->fields[0].off, lo = (uintptr_t)info->fields[1].off, co = (uintptr_t)info->fields[2].off;
        uint8_t *ptr = *(uint8_t *const *)(addr + po);
        uint64_t len = *(const uint64_t *)(addr + lo);
        uint64_t cap = *(const uint64_t *)(addr + co);
        if (len > 0)
        {
            cb_vec_drop_plain(ptr, len, esz);
        }
        if (cap > 0)
        {
            cb_deallocate(ptr, cap * esz, 1);
        }
        return;
    }
    if (info->drop)
    {
        /* `self`: a fresh exclusive root into the value. D-0315: in a decided
           program none, for a value with no object: no check of the
           destructor's body reads its token. */
        bool bare = owner == 0 && atomic_load_explicit(&DECIDED_PROGRAM, memory_order_relaxed) && !atomic_load_explicit(&RECORD_FULL, memory_order_relaxed);
        /* Report 337: an `Rc` in tail position of what an `Rc` destructor
           ends waits, as its bytes, for that destructor's return (`chain`):
           what the destructor does after it is unobservable. With no object
           over it (a decided program) its bytes are all it is; D-0339: so is
           a field's under `--all-checks` (`owner` 0), whose root and object
           are made when its turn comes, as for any value with none. */
        TState *cst = ts_raw();
        bool chain = info->chain_drop;
        bool flat = cst->chain_flat_next;
        cst->chain_flat_next = false;
        if (chain && owner == 0 && tail && cst->chain_top && info->size <= CHAIN_ITEM_BYTES && !rt()->unwinding)
        {
            if (cst->nchain == cst->capchain)
            {
                cst->capchain = cst->capchain ? cst->capchain * 2 : 16;
                cst->chain = (chain_item_t *)cb_xrealloc(cst->chain, cst->capchain * sizeof(chain_item_t));
            }
            chain_item_t *ci = &cst->chain[cst->nchain++];
            memset(ci, 0, sizeof *ci);
            memcpy(ci->bytes, (const void *)addr, (size_t)info->size);
            ci->ty = ty;
            return;
        }
        size_t chain_base = cst->nchain;
        bool chain_saved = cst->chain_top;
        cst->chain_top = chain;
        replay_out(me(), "a destructor runs");
        if (trace_events_on() && strncmp(info->name, "std", 3) != 0)
        {
            eprint_line("EV drop ", info->name);
        }
        uint64_t of = (owner != 0 || bare) ? owner : new_anonymous(r, addr, ty);
        uint64_t tok = 0;
        if (!bare)
        {
            projvec np = {0};
            u64vec na = {0};
            tok = mint(r, of, np, true, true, na);
        }
        bool counted = !multi();
        if (counted)
        {
            CALLBACK_DEPTH++;
        }
        /* The destructor is code of its own, run in the middle of the
           caller's transfers: it gets an empty in-flight queue, and the
           caller's is put back when it returns. */
        infq saved = ts_raw()->inflight;
        memset(&ts_raw()->inflight, 0, sizeof(infq));
        CbRef self = {(void *)addr, tok};
        /* Report 170: during the program's unwind (a fault's, an `exit`'s) a
           fault of the destructor is kept and
           the unwind goes on. */
        DROPS_RUNNING++;
        if (UNLIKELY(rt()->unwinding))
        {
            run_drop_guarded(info->drop, self);
        }
        else
        {
            info->drop(self);
        }
        DROPS_RUNNING--;
        infq leftover = ts_raw()->inflight;
        ts_raw()->inflight = saved;
        {
            Inflight it;
            while (infq_pop_front(&leftover, &it))
            {
                if (it.kind == IF_DATUM)
                {
                    u64vec_free(&it.datum);
                }
            }
            free(leftover.p);
        }
        if (counted)
        {
            CALLBACK_DEPTH--;
        }
        ts_raw()->chain_top = chain_saved;
        r = rt();
        if (!bare)
        {
            invalidate(r, tok);
            if (owner == 0)
            {
                end_identity(r, of);
            }
        }
        /* Report 337: the `Rc`s that waited for this destructor, unless a
           loop runs it, which takes them. */
        if (chain && !flat)
        {
            chain_drain(chain_base);
        }
    }
    r = rt();
    info = &r->types[pos];
    switch (info->kind)
    {
    case K_STRUCT:
    {
        /* The owning fields, last declared first, the first declared last;
           read by index each time. A field is in tail position when those
           destroyed after it free memory only (`quiet`): what its
           destruction hands on waits for them, unobservably (report 022, a
           chain whose nodes own a `String` beside the link). */
        uint32_t n = info->nowning;
        uint32_t nq = info->nquiet_owning;
        for (uint32_t i = n; i-- > 0;)
        {
            fieldref f = rt()->types[pos].owning[i];
            drop_in_place_tail(addr + (uintptr_t)f.off, f.ty, 0, tail && i <= nq);
        }
        break;
    }
    case K_ENUM:
    {
        size_t tag = enum_tag(info, addr);
        uintptr_t off = (uintptr_t)info->payload_off;
        if (tag < info->nvariants)
        {
            uint32_t pty = info->variants[tag];
            if (pty != NOTYPE)
            {
                drop_in_place_tail(addr + off, pty, 0, tail);
            }
        }
        break;
    }
    case K_ARRAY:
    {
        uint32_t elem = info->elem;
        uint64_t count = info->count;
        TypeInfo *ei = rt_typ(r, elem);
        uintptr_t esize = (uintptr_t)ei->size;
        bool eq = ei->quiet;
        for (uint64_t i = count; i-- > 0;)
        {
            drop_in_place_tail(addr + (uintptr_t)i * esize, elem, 0, tail && (i == 0 || eq));
        }
        break;
    }
    default:
        break;
    }
}

/* A `Box<T>` at `addr` destroyed as `std`'s `Box::drop` does it, natively.
   Its referent's destruction is its last observable effect, so a Box in
   tail position hands it to the nearest enclosing teardown's loop: a
   Box-linked chain of any length is destroyed in constant stack, in the
   same order. D-0298: a Box is one pointer; null marks one whose value
   `into_inner` took. (`box_next_t`: root, object, the cells' address in
   `cells`, their size in `size`, the type.) */
void box_drop(uintptr_t addr, uint32_t t, bool tail)
{
    uint8_t *p = *(uint8_t *const *)addr;
    if (!p)
    {
        return;
    }
    uint64_t sz = rt_typ(rt(), t)->size;
    uint64_t cells = sz ? sz : 1;
    /* With no live object over the contents, `[Reclaim]` would establish a
       fresh one which the destruction ends at once: unobservable, so the
       contents are destroyed in place (root and object 0). */
    bool none_over = quiet_cells((uintptr_t)p, (size_t)cells) || live_reclaimed(rt(), (uintptr_t)p) == 0;
    uint64_t root = 0, obj = 0;
    if (!none_over)
    {
        root = cb_reclaim(p, t);
        loc_t d = ts()->dloc;
        path_target_ret pt = path_target(rt(), root, d.f, d.l);
        obj = pt.of;
        projvec_free(&pt.proj);
        u64vec_free(&pt.excludes);
    }
    TState *st = ts();
    if (tail && st->box_draining)
    {
        box_next_t it = {root, obj, (uintptr_t)p, cells, t};
        if (!st->has_box_next)
        {
            st->has_box_next = true;
            st->box_next = it;
            return;
        }
        /* A second Box in tail position before the loop took the first:
           one of a tree's, or of a value whose fields after its link free
           memory only (`TypeInfo::quiet`). It waits on the thread's list
           rather than nesting a loop of its own, so a tree of any depth is
           destroyed in constant stack too (report 022). */
        if (st->nbox_more == st->capbox_more)
        {
            st->capbox_more = st->capbox_more ? st->capbox_more * 2 : 16;
            st->box_more = (box_next_t *)cb_xrealloc(st->box_more, st->capbox_more * sizeof(box_next_t));
        }
        st->box_more[st->nbox_more++] = it;
        return;
    }
    bool was = st->box_draining;
    st->box_draining = true;
    bool had_saved = st->has_box_next;
    box_next_t saved = st->box_next;
    st->has_box_next = false;
    /* The waiting Boxes below `base` are the enclosing loops'. */
    size_t base = st->nbox_more;
    box_next_t next = {root, obj, (uintptr_t)p, cells, t};
    bool has_next = true;
    while (has_next)
    {
        has_next = false;
        box_next_t c = next;
        if (c.obj == 0)
        {
            drop_in_place_tail(c.cells, c.ty, 0, true);
        }
        else
        {
            destroy_obj_tail(c.obj, c.root, true);
        }
        cb_deallocate((void *)c.cells, c.size, 0);
        TState *s2 = ts();
        if (s2->has_box_next)
        {
            next = s2->box_next;
            s2->has_box_next = false;
            has_next = true;
        }
        else if (s2->nbox_more > base)
        {
            next = s2->box_more[--s2->nbox_more];
            has_next = true;
        }
    }
    st = ts();
    st->has_box_next = had_saved;
    st->box_next = saved;
    st->box_draining = was;
    if (!was && st->nbox_more == 0 && st->box_more != NULL)
    {
        free(st->box_more);
        st->box_more = NULL;
        st->capbox_more = 0;
    }
}

/* The pending Boxes of a tree's walk (`tree_drop`): the first 64 on the
   stack, the rest on the heap. A buffer the walk allocates and frees among
   the nodes it frees would be the one block among them too large for
   glibc's fast bins (`tree` 1.07 of Rust's time). */
#define WALKSTACK(NAME, T)                                                \
    typedef struct                                                        \
    {                                                                     \
        T inl[64];                                                       \
        size_t n;                                                         \
        T *spill;                                                           \
        size_t nspill;                                                      \
        size_t capspill;                                                    \
    } NAME;                                                               \
    static inline void NAME##_push(NAME *w, T x)                          \
    {                                                                     \
        if (w->n < 64)                                                    \
        {                                                                 \
            w->inl[w->n++] = x;                                          \
            return;                                                       \
        }                                                                 \
        if (w->nspill == w->capspill)                                         \
        {                                                                 \
            w->capspill = w->capspill ? w->capspill * 2 : 4;                    \
            w->spill = (T *)cb_xrealloc(w->spill, w->capspill * sizeof(T));     \
        }                                                                 \
        w->spill[w->nspill++] = x;                                            \
    }                                                                     \
    static inline bool NAME##_pop(NAME *w, T *x)                          \
    {                                                                     \
        if (w->nspill)                                                      \
        {                                                                 \
            *x = w->spill[--w->nspill];                                       \
            return true;                                                  \
        }                                                                 \
        if (w->n == 0)                                                    \
        {                                                                 \
            return false;                                                 \
        }                                                                 \
        *x = w->inl[--w->n];                                             \
        return true;                                                      \
    }

typedef struct
{
    uintptr_t p;
    size_t q;
} walk_item;

WALKSTACK(walk_addr, uintptr_t)
WALKSTACK(walk_pair, walk_item)

static inline void walk_queue(uintptr_t base, const fieldref *plan, uint32_t n, bool *has_next, walk_item *next, walk_pair *more)
{
    for (uint32_t i = 0; i < n; i++)
    {
        uintptr_t p = *(const uintptr_t *)(base + (uintptr_t)plan[i].off);
        if (p != 0)
        {
            walk_item it = {p, plan[i].ty};
            if (!*has_next)
            {
                *has_next = true;
                *next = it;
            }
            else
            {
                walk_pair_push(more, it);
            }
        }
    }
}

static void tree_visit(TypeInfo *tys, uintptr_t addr, size_t pos, bool *has_next, walk_item *next, walk_pair *more);

/* A struct's owning fields, last declared first, or an array's elements,
   last first, as `drop_in_place_tail` visits them. */
static void tree_parts_drop(TypeInfo *tys, uintptr_t addr, TypeInfo *t, bool *has_next, walk_item *next, walk_pair *more)
{
    switch (t->kind)
    {
    case K_STRUCT:
        for (uint32_t i = t->ntree_parts; i-- > 0;)
        {
            tree_visit(tys, addr + (uintptr_t)t->tree_parts[i].off, t->tree_parts[i].ty, has_next, next, more);
        }
        break;
    case K_ARRAY:
    {
        size_t e = t->tree_parts[0].ty;
        uintptr_t esize = (uintptr_t)tys[e].size;
        for (uint64_t i = t->count; i-- > 0;)
        {
            tree_visit(tys, addr + (uintptr_t)i * esize, e, has_next, next, more);
        }
        break;
    }
    default:
        break;
    }
}

/* One part of a `tree` value: a Box and an enum's payload handled here, in
   the caller's loop, a struct's or an array's parts by `tree_parts_drop`. */
static inline __attribute__((always_inline)) void tree_visit(TypeInfo *tys, uintptr_t addr, size_t pos, bool *has_next, walk_item *next, walk_pair *more)
{
    TypeInfo *t = &tys[pos];
    if (t->kind == K_ENUM && !t->has_box_of && (t->is_resource || t->holds_fn))
    {
        size_t tag = enum_tag(t, addr);
        if (tag < t->ntree_parts && t->tree_parts[tag].ty != NOTYPE)
        {
            addr += (uintptr_t)t->payload_off;
            t = &tys[t->tree_parts[tag].ty];
        }
        else
        {
            return;
        }
    }
    if (!t->is_resource && !t->holds_fn)
    {
        return;
    }
    if (t->has_box_of)
    {
        uint32_t id = t->box_of;
        size_t inner = t->tree_parts[0].ty;
        uintptr_t p = *(const uintptr_t *)addr;
        if (p == 0)
        {
            return;
        }
        uint64_t sz = tys[inner].size;
        uint64_t cells = sz ? sz : 1;
        if (quiet_cells(p, (size_t)cells) || live_reclaimed(rt(), p) == 0)
        {
            walk_item it = {p, inner};
            if (!*has_next)
            {
                *has_next = true;
                *next = it;
            }
            else
            {
                walk_pair_push(more, it);
            }
        }
        else
        {
            /* An object over its cells (a `Box::get` reclaimed them, as the
               checked forms record): `box_drop`'s, handed in tail position,
               so that inside an enclosing Box's destruction it waits for
               that Box's loop rather than nesting a loop of its own. A
               chain whose every link was borrowed (walked, cloned) is then
               destroyed in constant stack, as an unborrowed one is
               (CHG-0475, report 022: a million links exhausted the stack in
               the bootstrap form). A tree value has nothing to observe in
               its destruction but the cells freed, so the order this gives
               is as good as any. */
            box_drop(addr, id, true);
        }
        return;
    }
    tree_parts_drop(tys, addr, t, has_next, next, more);
}

/* The destruction of a `tree` value at `addr` (`TypeInfo::tree`), alone on
   the runtime: what `drop_in_place_tail` and `box_drop` do for it, with
   nothing to observe but the cells freed, and those freed in another
   order. Each Box found over quiet cells is queued and its referent
   destroyed in turn, in constant stack; one with an object over its cells
   is `box_drop`'s. */
void tree_drop(uintptr_t addr, size_t pos)
{
    /* The descriptors, fixed since `cb_init`. */
    TypeInfo *tys = rt()->types;
    /* Quiet: no object over any raw cells, no reference slot held, no
       over-aligned block. Then every Box is freed as it is, found by the
       types' plans with no descriptor read on the way. */
    if (tys[pos].has_box_plan)
    {
        const fieldref *plan = tys[pos].box_plan;
        uint32_t nplan = tys[pos].nbox_plan;
        if (atomic_load_explicit(&cb_reclaimed_entries, memory_order_relaxed) == 0 && atomic_load_explicit(&OVER_ALIGNED, memory_order_relaxed) == 0 && slots_all_empty(rt()))
        {
            /* A tree of one node type: every Box the plan finds is of the
               type whose plan it is, so the walk reads no descriptor and the
               offsets stay fixed. */
            if (nplan > 0)
            {
                uint32_t u = plan[0].ty;
                const fieldref *up = tys[u].has_box_plan ? tys[u].box_plan : NULL;
                uint32_t nup = tys[u].has_box_plan ? tys[u].nbox_plan : 0;
                bool uniform = nup != 0 && nup <= 4;
                for (uint32_t i = 0; uniform && i < nplan; i++)
                {
                    uniform = plan[i].ty == u;
                }
                for (uint32_t i = 0; uniform && i < nup; i++)
                {
                    uniform = up[i].ty == u;
                }
                if (uniform)
                {
                    /* Each node's block, by its size (a node with a Box
                       in it is no empty type). */
                    uint64_t usz = tys[u].size;
                    uintptr_t offs[4] = {0, 0, 0, 0};
                    for (uint32_t i = 0; i < nup; i++)
                    {
                        offs[i] = (uintptr_t)up[i].off;
                    }
                    walk_addr more;
                    more.n = 0;
                    more.spill = NULL;
                    more.nspill = 0;
                    more.capspill = 0;
                    for (uint32_t i = 0; i < nplan; i++)
                    {
                        uintptr_t c = *(const uintptr_t *)(addr + (uintptr_t)plan[i].off);
                        if (c != 0)
                        {
                            walk_addr_push(&more, c);
                        }
                    }
                    uintptr_t p;
                    bool has = walk_addr_pop(&more, &p);
                    while (has)
                    {
                        bool has_next = false;
                        uintptr_t nx = 0;
                        for (uint32_t i = 0; i < nup; i++)
                        {
                            uintptr_t c = *(const uintptr_t *)(p + offs[i]);
                            if (c != 0)
                            {
                                /* Its cells asked for now: the load waits
                                   while `p` is freed. */
                                __builtin_prefetch((const void *)c, 0, 3);
                                if (!has_next)
                                {
                                    has_next = true;
                                    nx = c;
                                }
                                else
                                {
                                    walk_addr_push(&more, c);
                                }
                            }
                        }
                        block_free((void *)p, (size_t)usz);
                        if (has_next)
                        {
                            p = nx;
                            has = true;
                        }
                        else
                        {
                            has = walk_addr_pop(&more, &p);
                        }
                    }
                    free(more.spill);
                    return;
                }
            }
            bool has_next = false;
            walk_item next = {0, 0};
            walk_pair more;
            more.n = 0;
            more.spill = NULL;
            more.nspill = 0;
            more.capspill = 0;
            walk_queue(addr, plan, nplan, &has_next, &next, &more);
            for (;;)
            {
                walk_item c;
                if (has_next)
                {
                    c = next;
                    has_next = false;
                }
                else if (!walk_pair_pop(&more, &c))
                {
                    break;
                }
                TypeInfo *q = &tys[c.q];
                if (q->has_box_plan)
                {
                    walk_queue(c.p, q->box_plan, q->nbox_plan, &has_next, &next, &more);
                }
                if (q->size != 0)
                {
                    block_free((void *)c.p, (size_t)q->size);
                }
            }
            free(more.spill);
            return;
        }
    }
    bool has_next = false;
    walk_item next = {0, 0};
    walk_pair more;
    more.n = 0;
    more.spill = NULL;
    more.nspill = 0;
    more.capspill = 0;
    tree_visit(tys, addr, pos, &has_next, &next, &more);
    for (;;)
    {
        walk_item c;
        if (has_next)
        {
            c = next;
            has_next = false;
        }
        else if (!walk_pair_pop(&more, &c))
        {
            break;
        }
        tree_visit(tys, c.p, c.q, &has_next, &next, &more);
        uint64_t sz = tys[c.q].size;
        tree_free(c.p, sz ? sz : 1);
    }
    free(more.spill);
}

void tree_free(uintptr_t p, uint64_t cells)
{
    if (quiet_cells(p, (size_t)cells) && slots_all_empty(rt()))
    {
        free_block((uint8_t *)p, cells);
    }
    else
    {
        cb_deallocate((void *)p, cells, 0);
    }
}

void drop_fn_handle(uintptr_t handle)
{
    Rt *r = rt();
    uint64_t obj;
    /* Report 155: a box its copies share ends with the last of them. */
    if (handle != 0 && map_u64_get(&r->fn_boxes, handle))
    {
        CbFnBox *sb = (CbFnBox *)handle;
        uint32_t n = __atomic_load_n(&sb->refs, __ATOMIC_ACQUIRE);
        while (n > 0)
        {
            if (__atomic_compare_exchange_n(&sb->refs, &n, n - 1, 0, __ATOMIC_ACQ_REL, __ATOMIC_ACQUIRE))
            {
                return;
            }
        }
    }
    if (map_u64_remove(&r->fn_boxes, handle, &obj))
    {
        Obj *o = objs_get(r, obj);
        uint64_t root = o ? o->root : 0;
        destroy_obj(obj, root);
        CbFnBox *b = (CbFnBox *)handle;
        free(b->data);
        free(b);
    }
}

/* A field being destroyed needs an identity for its destructor's `self`
   path; it exists only for the duration of the call. */
uint64_t new_anonymous(Rt *r, uintptr_t addr, uint32_t ty)
{
    uint64_t id = objs_reserve(r);
    ensure_mark(id);
    obj_init(objs_insert(r, id), addr, ty, true, true, false, false);
    return id;
}

/* `[Object-End]`: every path into the object dies, every reference its
   storage held stops being an occurrence, the identity is gone. */
void end_identity(Rt *r, uint64_t obj)
{
    end_identity_in(r, obj, true);
}

/* `forget`: the references stored in the object's storage stop being
   occurrences (every end but `cb_absorb_refs`, whose storage now belongs to
   the container it moved into). */
void end_identity_in(Rt *r, uint64_t obj, bool forget)
{
    Obj *o = objs_get(r, obj);
    if (!o)
    {
        return;
    }
    end_identity_rec(r, o, obj, rt_typ(r, o->ty), forget);
}

/* Every path into the ending object `o` ends with it. */
static __attribute__((noinline)) void end_paths_of(Rt *r, Obj *o)
{
    u64vec toks = toklist_into_vec(&o->toks);
    for (uint32_t i = 0; i < toks.n; i++)
    {
        PathRec p;
        if (paths_remove(r, toks.p[i], &p))
        {
            give_proj(cur_proj_pool(r), p.proj);
            give_vec(cur_pool(r), p.ancestors);
            projvec_free(&p.lock_of);
        }
    }
    give_vec(cur_pool(r), toks);
}

/* `end_identity_in` with the object's record and its type in hand. */
CB_ALWAYS void end_identity_core(bool L, Rt *r, Obj *o, uint64_t obj, TypeInfo *ti, bool forget)
{
    uintptr_t addr = o->addr;
    bool ephemeral = o->ephemeral;
    if (o->xtoks.v.cap || o->xtoks.at)
    {
        give_vec_inl(&r->pool[HEAP_(L)], toklist_into_vec(&o->xtoks));
    }
    /* Every path into the object is dead; a token that turns up later is
       reported stale by its absence. */
    if (o->toks.v.n != 0 || o->toks.at)
    {
        end_paths_of(r, o);
    }
    else if (o->toks.v.cap)
    {
        give_vec_inl(&r->pool[HEAP_(L)], o->toks.v);
        memset(&o->toks.v, 0, sizeof o->toks.v);
    }
    /* An ephemeral object's type holds no reference, so no slot lies in its
       storage; nor does any other object's whose type holds none: only a
       reference slot is stored. Letting go of a slot's path may end other
       objects, so this one's record is looked up again after it. */
    if (forget && !ephemeral && ti->has_refs)
    {
        forget_slots(r, addr, ti->size);
        o = objs_get_inl(r, obj);
        if (!o)
        {
            cbrt_internal("an object's end found no object");
        }
    }
    set_mark(obj, false);
    if (TRACED_(L))
    {
        replay_forget(obj);
    }
    bool reclaimed = o->reclaimed;
    toklist_free(&o->toks);
    toklist_free(&o->xtoks);
    ObjArena_drop(&r->objs.h[obj >> HEAP_SHIFT], obj);
    if (reclaimed)
    {
        if (L && heaps_one(&r->reclaimed.n))
        {
            reclaimed_remove_if1(&r->reclaimed.h[0], addr, obj, ephemeral);
        }
        else
        {
            reclaimed_remove_if(r, addr, obj, ephemeral);
        }
    }
}

static void end_identity_rec(Rt *r, Obj *o, uint64_t obj, TypeInfo *ti, bool forget)
{
    end_identity_core(false, r, o, obj, ti, forget);
}

/* The slots in [addr, addr + size), in address order, when those are a few
   cells on one page and there is one heap (a local, a small value): read
   where they are, and taken out if `take`. SIZE_MAX: not such a range. */
#define SLOTS_FEW 8
size_t slots_few(Rt *r, uintptr_t addr, uint64_t size, bool take, addr_tok *out)
{
    uintptr_t page = addr >> SLOT_PAGE;
    if (size > 8 * SLOTS_FEW || ((addr + (uintptr_t)size - 1) >> SLOT_PAGE) != page || !heaps_one(&r->slots.n) || r->slots.h[0].odd.n != 0)
    {
        return SIZE_MAX;
    }
    SlotMap *m = &r->slots.h[0];
    SlotPage *pg = slotmap_page(m, page);
    if (!pg || pg->n == 0)
    {
        return 0;
    }
    size_t k = 0;
    for (uintptr_t a = (addr + 7) & ~(uintptr_t)7; a < addr + (uintptr_t)size; a += 8)
    {
        uint64_t *e = &pg->s[(a >> 3) & 511];
        if (*e != 0)
        {
            out[k].a = a;
            out[k].t = *e;
            k++;
            if (take)
            {
                *e = 0;
                pg->n--;
            }
        }
    }
    if (take && k && pg->n == 0)
    {
        slotmap_emptied(m, page, pg);
    }
    return k;
}

/* `forget_slots` for up to four cells of page `pg` that hold a slot: taken
   where they are, then let go of in address order (letting go can end
   other objects, and a page). */
static __attribute__((noinline)) void forget_cells(SlotMap *m, uintptr_t page, SlotPage *pg, uint64_t *e, size_t cells)
{
    uint64_t t0 = e[0], t1 = 0, t2 = 0, t3 = 0;
    e[0] = 0;
    if (cells > 1)
    {
        t1 = e[1];
        e[1] = 0;
    }
    if (cells > 2)
    {
        t2 = e[2];
        e[2] = 0;
    }
    if (cells > 3)
    {
        t3 = e[3];
        e[3] = 0;
    }
    pg->n -= (uint32_t)((t0 != 0) + (t1 != 0) + (t2 != 0) + (t3 != 0));
    if (pg->n == 0)
    {
        slotmap_emptied(m, page, pg);
    }
    Rt *r = rt();
    if (t0 != 0)
    {
        drop_occ(r, t0);
    }
    if (t1 != 0)
    {
        drop_occ(r, t1);
    }
    if (t2 != 0)
    {
        drop_occ(r, t2);
    }
    if (t3 != 0)
    {
        drop_occ(r, t3);
    }
}

static __attribute__((noinline)) void forget_slots_more(Rt *r, uintptr_t addr, uint64_t size);

/* The references stored in [addr, addr + size) stop being occurrences. */
void forget_slots(Rt *r, uintptr_t addr, uint64_t size)
{
    /* Up to four cells on one page, one heap (a local that holds a
       reference, a slice, a view), most often with no slot in them:
       looked at where they are, and nothing is called but at the end. */
    if (size - 1 < 32 && (addr & 7) == 0 && heaps_one(&r->slots.n) && r->slots.h[0].odd.n == 0)
    {
        uintptr_t page = addr >> SLOT_PAGE;
        size_t cells = (size_t)((size + 7) >> 3);
        if (((addr + 8 * (cells - 1)) >> SLOT_PAGE) == page)
        {
            SlotMap *m = &r->slots.h[0];
            SlotPage *pg = slotmap_page(m, page);
            if (!pg || pg->n == 0)
            {
                return;
            }
            uint64_t *e = &pg->s[(addr >> 3) & 511];
            uint64_t any = 0;
            for (size_t i = 0; i < cells; i++)
            {
                any |= e[i];
            }
            if (any != 0)
            {
                forget_cells(m, page, pg, e, cells);
            }
            return;
        }
    }
    if (size != 0)
    {
        forget_slots_more(r, addr, size);
    }
}

static __attribute__((noinline)) void forget_slots_more(Rt *r, uintptr_t addr, uint64_t size)
{
    /* A few cells on one page, one heap: taken where they are. */
    addr_tok held8[SLOTS_FEW];
    size_t k = slots_few(r, addr, size, true, held8);
    if (k != SIZE_MAX)
    {
        for (size_t i = 0; i < k; i++)
        {
            drop_occ(r, held8[i].t);
        }
        return;
    }
    /* Into a buffer kept for this (an object's end is the commonest runtime
       event); a nested end, through `drop_occ`, takes a new one. */
    size_t h = cur_heap();
    atvec held = r->slot_buf[h];
    memset(&r->slot_buf[h], 0, sizeof(atvec));
    slots_scan_into(r, addr, addr + (uintptr_t)size, true, &held);
    for (size_t i = 0; i < held.n; i++)
    {
        drop_occ(r, held.p[i].t);
    }
    held.n = 0;
    r = rt();
    h = cur_heap();
    if (r->slot_buf[h].cap < held.cap)
    {
        atvec_free(&r->slot_buf[h]);
        r->slot_buf[h] = held;
    }
    else
    {
        atvec_free(&held);
    }
}

/* ---- raw storage ---- */

uint64_t cb_reclaim(void *addr, uint32_t ty)
{
    replay_out(me(), "raw storage");
    /* D-0204: as the element entries: the heaps of the cells' pages, and an
       object found there in one of them. */
    uintptr_t a = (uintptr_t)addr;
    HELD_SCOPE e = enter_elem(a, ty);
    Rt *r = rt();
    uint64_t obj = live_reclaimed(r, a);
    if (obj)
    {
        /* Its root token is about to be in the program's hands. */
        Obj *o = objs_at(r, obj);
        if (o->ephemeral)
        {
            o->ephemeral = false;
            uint64_t size = rt_typ(r, o->ty)->size;
            reclaimed_insert(r, a, obj, size);
        }
        o = objs_at(r, obj);
        if (o->root)
        {
            return o->root;
        }
        uint64_t root = make_root(r, obj);
        objs_at(r, obj)->root = root;
        return root;
    }
    return establish_reclaimed(r, a, ty, false).tok;
}

/* `[Reclaim]`'s fresh object over the cells at `a`: its root token (0 for
   an ephemeral object, which has none) and its id. */
/* An ephemeral object over the cells at `a`, of the type `ti` describes:
   no root path (nothing outside the runtime ever holds one; `cb_reclaim`
   mints it if the program re-attaches the object). */
CB_ALWAYS uint64_t establish_ephemeral_core(bool L, Rt *r, uintptr_t a, uint32_t ty, TypeInfo *ti)
{
    full_on();
    uint64_t id = ObjArena_reserve(&r->objs.h[HEAP_(L)]);
    if (UNLIKELY((size_t)(uint32_t)id - 1 >= MARK_LEN[(id >> HEAP_SHIFT) & 0xff]))
    {
        ensure_mark(id);
    }
    obj_init(objs_insert(r, id), a, ty, true, ti->is_resource, true, true);
    reclaimed_insert_ephemeral1(&r->reclaimed.h[HEAP_(L)], a, id, ti->size);
    return id;
}

establish_ret establish_reclaimed(Rt *r, uintptr_t a, uint32_t ty, bool ephemeral)
{
    TypeInfo *ti = rt_typ(r, ty);
    establish_ret out;
    if (ephemeral)
    {
        out.tok = 0;
        out.id = establish_ephemeral_core(false, r, a, ty, ti);
        return out;
    }
    uint64_t id = objs_reserve(r);
    if (UNLIKELY((size_t)(uint32_t)id - 1 >= MARK_LEN[(id >> HEAP_SHIFT) & 0xff]))
    {
        ensure_mark(id);
    }
    obj_init(objs_insert(r, id), a, ty, true, ti->is_resource, true, false);
    reclaimed_insert(r, a, id, ti->size);
    uint64_t tok = make_root(r, id);
    objs_at(r, id)->root = tok;
    out.tok = tok;
    out.id = id;
    return out;
}

/* `&place` / `&mut place` passed to a parameter that never reads the
   reference's path (`cobc`'s pure direct parameters): `[Borrow]`'s check
   alone -- stale binding, then `clash` -- with no path minted. Token 0
   names no path: a check through it is proven. */
void cb_borrow_unminted(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line)
{
    replay_out(me(), "a borrow checked and not formed");
    /* D-0292: through a light reference nothing is checked (`light_borrow`). */
    if (base == 0 || (light_ref(base) && !frame_rec()))
    {
        return;
    }
    HELD_SCOPE e = enter_on(heap_of(base));
    check_access(rt(), base, n ? p : NULL, n, mode == 1, file, line);
}

/* `&place` / `&mut place` passed straight to the prelude's `Vec::index_*`:
   the borrow's own check, then the checked read of `v.len` the body
   performs first -- which can fail only for initialization. No token is
   minted (D-0018: unobservable). */
void cb_borrow_check(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line)
{
    replay_out(me(), "a vector element");
    if (base == 0 || (light_ref(base) && !frame_rec()))
    {
        return;
    }
    HELD_SCOPE e = enter_on(heap_of(base));
    Rt *r = rt();
    uint64_t of = check_access(r, base, n ? p : NULL, n, mode == 1, file, line);
    if (!objs_at(r, of)->init)
    {
        /* The body's read reports no location (`spec/21`'s own text). */
        set_at(NOLOC, 0);
        fault("diag.use-of-uninitialized", NOLOC, 0);
    }
}

/* `Vec::index_shared`/`Vec::index_exclusive` (`spec/21` §1) realized
   natively (§0 permits it): `[Reclaim]` of the element's cells, `[Borrow]`
   of it in `mode`, and the result's return to the caller (`cb_send_ref`). */
uint64_t cb_elem_borrow(void *addr, uint32_t ty, uint32_t mode)
{
    replay_out(me(), "a vector element");
    HELD_SCOPE e = enter_elem((uintptr_t)addr, ty);
    uint64_t tok = elem_borrow(addr, ty, mode, false, true);
    PathRec *p = paths_get(rt(), tok);
    if (p)
    {
        p->occ++;
    }
    Inflight f;
    memset(&f, 0, sizeof f);
    f.kind = IF_REF;
    f.x = tok;
    infq_push_back(&ts_raw()->inflight, f);
    return tok;
}

/* The same for a `…_pre` body, whose caller receives the reference in the
   statement scope that is current: the path is recorded there directly. */
uint64_t cb_elem_borrow_here(void *addr, uint32_t ty, uint32_t mode)
{
    if (LIKELY(lone()))
    {
        return elem_borrow_lone(addr, ty, mode, true);
    }
    replay_out(me(), "a vector element");
    HELD_SCOPE e = enter_elem((uintptr_t)addr, ty);
    return elem_borrow(addr, ty, mode, true, true);
}

/* D-0202: entering for the object over an element's cells at `a`: a local
   hold on the heaps whose page buckets name its pages and this thread's,
   when the object found there, if any, is in one of them. */
CB_COLD Held enter_elem_multi(uintptr_t a, uint32_t ty)
{
    uint64_t sz = rt_typ(rt(), ty)->size;
    Held h = enter_set_multi(addr_heaps(a, a + (uintptr_t)(sz ? sz : 1)));
    if (h != H_LOCAL)
    {
        return h;
    }
    uint64_t o;
    if (!reclaimed_get(rt(), a, &o) || holds_heap(heap_of(o)))
    {
        return h;
    }
    held_drop(h);
    return held_excl();
}

/* `*Vec::index_*(&v, i)` read, or assigned a stateless value, at once, for a
   plain element type: where no live object lies over the element's cells,
   nothing is done (the borrow's ephemeral object would end unseen);
   otherwise the same checks are made, and the path, unheld, ends now
   (D-0018, D-0023). */
void cb_elem_access_slow(void *addr, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line)
{
    HELD_SCOPE e = enter_elem((uintptr_t)addr, ty);
    Rt *r = rt();
    if (live_reclaimed(r, (uintptr_t)addr) == 0)
    {
        return;
    }
    /* A clash faults in `elem_borrow`, which takes the current location:
       the caller's, when it gives one. */
    if (!(file == NOLOC && line == 0))
    {
        set_at(file, line);
    }
    uint64_t tok = elem_borrow(addr, ty, mode, false, false);
    if (write != 0)
    {
        cb_write(tok, NULL, 0, 0, file, line);
    }
    else
    {
        cb_read(tok, NULL, 0, file, line);
    }
    retire_token(rt(), tok);
}

/* The live object over the cells at `a`, if any: one with a root, an
   ephemeral one, or a value `cb_raw_move_in` placed there. Establishing a
   second object over them instead left the first alive and unreachable. */
uint64_t live_reclaimed(Rt *r, uintptr_t a)
{
    uint64_t obj;
    if (!reclaimed_get(r, a, &obj))
    {
        return 0;
    }
    Obj *o = objs_get(r, obj);
    if (o && o->alive && (o->root != 0 || o->ephemeral || (o->reclaimed && o->frame == NO_IDX && o->scope == NO_IDX)))
    {
        return obj;
    }
    return 0;
}

/* `light`: the borrow may be a light reference (D-0295) where the frame
   forms light references and the element's object persists. The checks are
   the recorded borrow's (`check_root_access`); the result, `LIGHT | obj`,
   is recorded nowhere. An ephemeral object is left out: it ends when its
   last recorded path does. */
CB_ALWAYS uint64_t elem_borrow_core(bool L, void *addr, uint32_t ty, uint32_t mode, bool stamp, bool light)
{
    Rt *r = rt();
    uintptr_t a = (uintptr_t)addr;
    bool exclusive = mode == 1;
    /* The object over these cells, if one is alive (`live_reclaimed`). */
    uint64_t existing = 0;
    Obj *eo = NULL;
    {
        uint64_t x;
        bool found = L && heaps_one(&r->reclaimed.n) ? reclaimed_get1(&r->reclaimed.h[0], a, &x) : reclaimed_get(r, a, &x);
        if (found)
        {
            eo = objs_get_inl(r, x);
            if (eo && eo->alive && (eo->root != 0 || eo->ephemeral || (eo->reclaimed && eo->frame == NO_IDX && eo->scope == NO_IDX)))
            {
                existing = x;
            }
        }
    }
    if (light && !atomic_load_explicit(&RECORD_FULL, memory_order_relaxed))
    {
        light = !top_rec(L ? &MAIN_TS : ts_raw());
    }
    else
    {
        light = false;
    }
    bool persists;
    uint64_t obj;
    if (existing)
    {
        persists = light && !eo->ephemeral;
        obj = existing;
        /* `[Borrow]` from the object's root path. */
        check_root_access(r, obj, exclusive, NOLOC, 0);
    }
    else
    {
        /* Ephemeral unless the element holds references or is a function
           value (a resource element qualifies too: `cb_reclaim`, which
           gives the program its root, makes it persistent). A new object
           has no path yet: its root path's access clashes with nothing. */
        TypeInfo *t = rt_typ(r, ty);
        bool plain = !t->has_refs && t->kind != K_FN;
        persists = light && !plain;
        obj = L && plain ? establish_ephemeral_core(true, r, a, ty, t) : establish_reclaimed(r, a, ty, plain).id;
    }
    if (persists)
    {
        ph3_count(&N_LIGHT);
        return obj | LIGHT;
    }
    projvec np = {0};
    u64vec na = {0};
    if (L)
    {
        return mint_core(true, r, obj, np, exclusive, false, na, NULL, stamp);
    }
    return mint_in(r, obj, np, exclusive, false, na, NULL, stamp);
}

uint64_t elem_borrow(void *addr, uint32_t ty, uint32_t mode, bool stamp, bool light)
{
    return elem_borrow_core(false, addr, ty, mode, stamp, light);
}

/* An element borrow on the lone thread, stamped or not. */
static __attribute__((noinline)) uint64_t elem_borrow_lone(void *addr, uint32_t ty, uint32_t mode, bool stamp)
{
    return elem_borrow_core(true, addr, ty, mode, stamp, true);
}

/* `Vec::drop`'s element loop for a plain, reference-free element type,
   realized natively: only the elements that already have an object are
   checked (`[Reclaim]` and a checked read), in index order. */
void cb_vec_drop_plain(void *addr, uint64_t len, uint64_t size)
{
    /* No object was ever made: none lies over the elements. */
    if (HOOK(FULL.vec_drop_plain))
    {
        HOOK(FULL.vec_drop_plain)(addr, len, size);
    }
}

static void vec_drop_plain_full(void *addr, uint64_t len, uint64_t size)
{
    uintptr_t a = (uintptr_t)addr;
    uintptr_t n = (uintptr_t)(len * size);
    /* D-0202: local while no object lies over the elements. */
    HELD_SCOPE e = enter_live_empty(a, a + (n ? n : 1));
    Rt *r = rt();
    if (!reclaimed_any_in(r, a, a + n))
    {
        return;
    }
    atvec rg = reclaimed_range(r, a, a + n);
    atvec existing = {0};
    for (size_t i = 0; i < rg.n; i++)
    {
        uintptr_t at = rg.p[i].a;
        if (size != 0 && (at - a) % size != 0)
        {
            continue;
        }
        Obj *o = objs_get(r, rg.p[i].t);
        if (o)
        {
            atvec_push(&existing, at, o->ty);
        }
    }
    atvec_free(&rg);
    for (size_t i = 0; i < existing.n; i++)
    {
        uint64_t root = cb_reclaim((void *)existing.p[i].a, (uint32_t)existing.p[i].t);
        cb_read(root, NULL, 0, NOLOC, 0);
    }
    atvec_free(&existing);
}

/* Whether a live object starts over any of the `n` cells from `addr`: a
   native body over a run of elements takes its general path only when one
   of them has an object a path may hold. */
uint8_t cb_live_in(void *addr, uint64_t n)
{
    if (n == 0)
    {
        return 0;
    }
    /* D-0202: a local hold on the heaps whose page buckets name these
       cells, when every object registered over them is in one of those. */
    uintptr_t a = (uintptr_t)addr;
    HELD_SCOPE e = enter_live(a, a + (uintptr_t)n);
    if (quiet_cells(a, (size_t)n))
    {
        return 0;
    }
    Rt *r = rt();
    if (!reclaimed_any_in(r, a, a + (uintptr_t)n))
    {
        return 0;
    }
    atvec rg = reclaimed_range(r, a, a + (uintptr_t)n);
    uint8_t any = 0;
    for (size_t i = 0; i < rg.n; i++)
    {
        if (live_reclaimed(r, rg.p[i].a) != 0)
        {
            any = 1;
            break;
        }
    }
    atvec_free(&rg);
    return any;
}

/* CHG-0427: whether any entry lies over the `n` cells from `addr`, live or
   not (`release_cells`'s test, exactly). */
uint8_t cb_cells_held(void *addr, uint64_t n)
{
    if (n == 0)
    {
        return 0;
    }
    uintptr_t a = (uintptr_t)addr;
    HELD_SCOPE e = enter_live(a, a + (uintptr_t)n);
    if (quiet_cells(a, (size_t)n))
    {
        return 0;
    }
    return reclaimed_any_in(rt(), a, a + (uintptr_t)n) ? 1 : 0;
}

/* `cb_live_in` of two runs of `n` cells, in one call (`Vec::swap`'s two
   elements); each run under its own hold. */
uint8_t cb_live_in2(void *a, void *b, uint64_t n)
{
    return (cb_live_in(a, n) != 0 || cb_live_in(b, n) != 0) ? 1 : 0;
}

/* As `enter_live`, but local only when no reclaimed entry lies over the
   cells at all. */
CB_COLD Held enter_live_empty_multi(uintptr_t lo, uintptr_t hi)
{
    Held h = enter_set_multi(addr_heaps(lo, hi));
    if (h != H_LOCAL || quiet_cells(lo, (size_t)(hi - lo)) || !reclaimed_any_in(rt(), lo, hi))
    {
        return h;
    }
    held_drop(h);
    return held_excl();
}

CB_COLD Held enter_live_multi(uintptr_t lo, uintptr_t hi)
{
    Held h = enter_set_multi(addr_heaps(lo, hi));
    if (h != H_LOCAL)
    {
        return h;
    }
    Rt *r = rt();
    if (quiet_cells(lo, (size_t)(hi - lo)) || !reclaimed_any_in(r, lo, hi))
    {
        return h;
    }
    atvec rg = reclaimed_range(r, lo, hi);
    bool all = true;
    for (size_t i = 0; i < rg.n; i++)
    {
        if (!holds_heap(heap_of(rg.p[i].t)))
        {
            all = false;
            break;
        }
    }
    atvec_free(&rg);
    if (all)
    {
        return h;
    }
    held_drop(h);
    return held_excl();
}

/* Whether a live object lies over the cells at `addr`: for a native body
   that must take the general path only for such an element (`cobc`'s
   native `Vec<String>::drop`). */
uint8_t cb_live_at(void *addr)
{
    HELD_SCOPE e = enter_live((uintptr_t)addr, (uintptr_t)addr + 1);
    return live_reclaimed(rt(), (uintptr_t)addr) != 0 ? 1 : 0;
}

void cb_raw_move_in(uint64_t obj, void *addr)
{
    replay_out(me(), "raw storage");
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    uintptr_t a = (uintptr_t)addr;
    uint64_t old;
    if (reclaimed_get(r, a, &old) && old != obj)
    {
        check_not_foreign(r, &old, 1);
        end_identity(r, old);
    }
    cb_move_to(obj, addr);
    r = rt();
    detach(r, obj);
    Obj *o = objs_at(r, obj);
    o->reclaimed = true;
    uint32_t ty = o->ty;
    if (o->root)
    {
        uint64_t root = o->root;
        o->root = 0;
        invalidate(r, root);
    }
    /* A value holding no reference and no function, its root ended by the
       move, has no path left: it ends now instead of lasting until
       `[Release]` (one per element a `Vec<String>` was ever pushed). */
    TypeInfo *t = rt_typ(r, ty);
    uint64_t size = t->size;
    o = objs_get(r, obj);
    if (!t->has_refs && t->kind != K_FN && o && o->toks.v.n == 0)
    {
        end_identity(r, obj);
        return;
    }
    reclaimed_insert(r, a, obj, size);
}

void cb_raw_move_out(void *addr, void *dst, uint32_t ty)
{
    replay_out(me(), "raw storage");
    ENTERED_SCOPE e = enter();
    Rt *r = rt();
    /* `[Rawptr-Move-Out]` re-keys held-by to the new object: the references
       the value holds move with its bytes (already copied to `dst`), and
       only then does the reclaimed identity at `addr` end. */
    uint64_t size = rt_typ(r, ty)->size;
    uint64_t obj;
    if (reclaimed_get(r, (uintptr_t)addr, &obj))
    {
        check_not_foreign(r, &obj, 1);
    }
    rekey_slots(r, (uintptr_t)addr, (uintptr_t)dst, size);
    if (reclaimed_get(r, (uintptr_t)addr, &obj))
    {
        end_identity(r, obj);
    }
}

void cb_release(void *addr, uint64_t n)
{
    HELD_SCOPE e = enter_cells((uintptr_t)addr, n);
    release_cells(addr, n);
}

/* D-0202: entering to end what lies over the cells [a, a + n): a local hold
   on the heaps whose page buckets name those pages, when nothing there
   reaches further -- no reference slot, and only reclaimed objects of held
   heaps that hold no references. Otherwise exclusive, decided before
   anything changes. */
CB_COLD Held enter_cells_multi(uintptr_t a, uint64_t n)
{
    uintptr_t hi = a + (uintptr_t)(n ? n : 1);
    Held h = enter_set_multi(addr_heaps(a, hi));
    if (h != H_LOCAL || cells_local(a, hi))
    {
        return h;
    }
    held_drop(h);
    return held_excl();
}

bool cells_local(uintptr_t a, uintptr_t hi)
{
    Rt *r = rt();
    if (slots_any_in(r, a, hi))
    {
        return false;
    }
    if (quiet_cells(a, (size_t)(hi - a)) || !reclaimed_any_in(r, a, hi))
    {
        return true;
    }
    atvec rg = reclaimed_range(r, a, hi);
    bool all = true;
    for (size_t i = 0; i < rg.n && all; i++)
    {
        uint64_t o = rg.p[i].t;
        if (!holds_heap(heap_of(o)))
        {
            all = false;
            break;
        }
        Obj *x = objs_get(r, o);
        if (x && rt_typ(r, x->ty)->has_refs)
        {
            all = false;
        }
    }
    atvec_free(&rg);
    return all;
}

/* D-0214: whether another thread holds an exclusive path to `obj` -- one
   that is another running thread's, or lent and not yet taken (`affine`).
   Only while another thread runs. */
bool foreign_held(Rt *r, uint64_t obj)
{
    if (atomic_load_explicit(&cb_live_threads, memory_order_relaxed) == 1)
    {
        return false;
    }
    Obj *o = objs_get(r, obj);
    if (!o)
    {
        return false;
    }
    uint64_t m = me();
    for (uint32_t i = 0; i < o->xtoks.v.n; i++)
    {
        PathRec *p = paths_get(r, o->xtoks.v.p[i]);
        if (!p)
        {
            continue;
        }
        uint64_t th = atomic_load_explicit(&p->thread, memory_order_acquire);
        if (p->valid && p->occ > 0 && th != m && (th == TRANSIT || thread_running(th)))
        {
            return true;
        }
    }
    return false;
}

/* D-0214 `[Release]`, `[Rawptr-Move-Out]`: a reclaimed object another thread
   holds an exclusive path to is not ended under it. */
void check_not_foreign(Rt *r, const uint64_t *objs, size_t n)
{
    if (atomic_load_explicit(&cb_live_threads, memory_order_relaxed) == 1)
    {
        return;
    }
    for (size_t i = 0; i < n; i++)
    {
        if (foreign_held(r, objs[i]))
        {
            uint32_t f, l;
            get_at(&f, &l);
            fault("diag.aliasing-conflict", f, l);
        }
    }
}

static void end_stale_over(Rt *r, uintptr_t a, uint64_t n)
{
    /* No entry on these pages (`quiet_cells`): no object over these cells. */
    if (!quiet_cells(a, (size_t)n) && reclaimed_any_in(r, a, a + (uintptr_t)n))
    {
        atvec rg = reclaimed_range(r, a, a + (uintptr_t)n);
        u64vec stale = {0};
        for (size_t i = 0; i < rg.n; i++)
        {
            u64vec_push(&stale, rg.p[i].t);
        }
        atvec_free(&rg);
        check_not_foreign(r, stale.p, stale.n);
        for (uint32_t i = 0; i < stale.n; i++)
        {
            end_identity(r, stale.p[i]);
        }
        u64vec_free(&stale);
    }
}

/* `cb_release`'s work, inside an entry. */
void release_cells(void *addr, uint64_t n)
{
    Rt *r = rt();
    uintptr_t a = (uintptr_t)addr;
    /* Through FULL: no object over raw cells and no stored reference was
       ever made where its pointers are null. */
    if (!quiet_cells(a, (size_t)n) && HOOK(FULL.end_stale_over))
    {
        HOOK(FULL.end_stale_over)(r, a, n);
    }
    /* References written into these cells were held by the object over
       them (`[Rawptr-Write]`, `CHG-0031`), reclaimed or not: none is held
       once the cells are released. */
    if (HOOK(FULL.forget_slots))
    {
        HOOK(FULL.forget_slots)(r, a, n);
    }
}

/* `cb_release` for cells that never hold a reference: a `Vec` of plain
   elements. The objects over the cells end as `cb_release` ends them; its
   scan for references held by the cells, which could find none, is not
   made. */
void cb_release_plain(uint8_t *addr, uint64_t n)
{
    HELD_SCOPE e = enter_cells((uintptr_t)addr, n);
    release_plain_cells(addr, n);
}

/* `cb_release_plain`'s work, inside an entry. */
void release_plain_cells(void *addr, uint64_t n)
{
    if (!quiet_cells((uintptr_t)addr, (size_t)n) && HOOK(FULL.end_stale_over))
    {
        HOOK(FULL.end_stale_over)(rt(), (uintptr_t)addr, n);
    }
}
