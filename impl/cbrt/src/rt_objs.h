/* Declarations of rt_objs.c (see rt.h): frames and statement scopes,
   objects and their roots, destruction (`[Destroy]`, `[Object-End]`, Box and
   tree teardown), raw storage (`[Reclaim]`, element borrows, `[Release]`). */

#ifndef CBRT_RT_OBJS_H
#define CBRT_RT_OBJS_H

/* ---- exported (cbrt.h) ---- */

void cb_frame_push(void);
void cb_frame_push_fn(void);
void cb_frame_push_rec(void);
void cb_frame_push_at(uint32_t file, uint32_t line);
void cb_frame_pop(void);
void cb_frame_hold(uint64_t tok);
void cb_frame_unhold(uint64_t tok);
void cb_stmt_push(void);
void cb_stmt_push_at(uint32_t file, uint32_t line);
void cb_stmt_pop(void);
uint64_t cb_new(void *addr, uint32_t ty, uint32_t file, uint32_t line);
uint64_t cb_new_uninit(void *addr, uint32_t ty, uint32_t file, uint32_t line);
uint64_t cb_bind(uint64_t obj);
uint64_t cb_frame_here(void);
uint64_t cb_rebind(uint64_t root, void *addr, uint32_t ty, uint64_t frame, uint32_t file, uint32_t line);
void cb_move_to(uint64_t obj, void *addr);
uint64_t cb_temp_path(uint64_t obj);
uint64_t cb_take(uint64_t root, uint32_t file, uint32_t line);
void cb_absorb(uint64_t obj);
void cb_absorb_refs(uint64_t obj);
void cb_result(uint64_t obj);
void cb_destroy(uint64_t root, uint32_t file, uint32_t line);
void cb_destroy_via(uint64_t tok, const void *addr, uint32_t ty, uint32_t file, uint32_t line);
void cb_end_binding(uint64_t root);
void cb_consume(uint64_t obj, uint32_t file, uint32_t line);
void cb_end_moved_out(uint64_t obj);
uint64_t cb_reclaim(void *addr, uint32_t ty);
void cb_borrow_unminted(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);
void cb_borrow_check(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);
uint64_t cb_elem_borrow(void *addr, uint32_t ty, uint32_t mode);
uint64_t cb_elem_borrow_here(void *addr, uint32_t ty, uint32_t mode);
void cb_elem_access_slow(void *addr, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line);
void cb_vec_drop_plain(void *addr, uint64_t len, uint64_t size);
uint8_t cb_live_in(void *addr, uint64_t n);
uint8_t cb_cells_held(void *addr, uint64_t n);
uint8_t cb_live_in2(void *a, void *b, uint64_t n);
uint8_t cb_live_at(void *addr);
void cb_raw_move_in(uint64_t obj, void *addr);
void cb_raw_move_out(void *addr, void *dst, uint32_t ty);
void cb_release(void *addr, uint64_t n);
void cb_release_plain(uint8_t *addr, uint64_t n);

/* ---- internal ---- */

void frame_push_with(bool rec);
/* D-0292: whether the running frame records every reference it forms. */
static inline bool frame_rec(void)
{
    if (atomic_load_explicit(&RECORD_FULL, memory_order_relaxed))
    {
        return true;
    }
    return top_rec(ts_raw());
}
/* A light reference into an object (`LIGHT | obj`): no record of its own. */
static inline bool light_ref(uint64_t tok)
{
    return (tok & LIGHT) != 0 && (tok & ROOT) == 0;
}
void check_light(const char *what, uint64_t tok, uint32_t file, uint32_t line);

void end_scope(Scope s);
void invalidate(Rt *r, uint64_t tok);
void drop_occ(Rt *r, uint64_t tok);
void retire_token(Rt *r, uint64_t t);
u64vec *scope_toks(Rt *r, size_t i); /* NULL: none */
void end_owned(u64vec owned);
void end_temps(const uint64_t *temps, size_t n);
size_t top_frame(Rt *r);  /* NO_IDX: none */
size_t scope_below(size_t i);
size_t top_scope(Rt *r);
void detach(Rt *r, uint64_t obj);
void stamp_temp(Rt *r, uint64_t obj, size_t scope);
void detach_tok(Rt *r, uint64_t tok);
void stamp_tok(Rt *r, uint64_t tok, size_t scope);

uint64_t new_obj(uintptr_t addr, uint32_t ty, bool init);
/* `mint*`: `proj` and `ancestors` are moved in; `lock_of` NULL is None,
   else its contents are moved in. */
uint64_t mint(Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors);
uint64_t mint_locked(Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors, projvec *lock_of);
uint64_t mint_in(Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors, projvec *lock_of, bool stamp);
uint64_t make_root(Rt *r, uint64_t obj);
void rekey_slots(Rt *r, uintptr_t old, uintptr_t new_, uint64_t size);

/* `path_target`: (object, projection, excludes) -- owned copies the caller
   frees; faults `diag.stale-binding`. `path_target_locked` adds the lock
   origin (`has_lock_of`, `lock_of`). */
typedef struct
{
    uint64_t of;
    projvec proj;
    u64vec excludes;
} path_target_ret;
typedef struct
{
    uint64_t of;
    projvec proj;
    u64vec excludes;
    bool has_lock_of;
    projvec lock_of;
} path_target_locked_ret;
path_target_ret path_target(Rt *r, uint64_t tok, uint32_t file, uint32_t line);
path_target_locked_ret path_target_locked(Rt *r, uint64_t tok, uint32_t file, uint32_t line);

/* D-0292: what a token names (`PathView`), borrowed from the tables. */
typedef struct
{
    uint64_t of;
    const Proj *proj;
    size_t nproj;
    bool exclusive;
    bool is_root;
    const uint64_t *ancestors;
    size_t nancestors;
    bool has_lock_of;
    const Proj *lock_of;
    size_t nlock_of;
    PathRec *rec; /* NULL: none */
} PathView;
/* false: stale. */
bool view(Rt *r, uint64_t tok, PathView *out);
/* The object a token names, valid or not (0: no record at all). */
uint64_t tok_obj(Rt *r, uint64_t tok);

void destroy_obj(uint64_t obj, uint64_t root);
void destroy_obj_tail(uint64_t obj, uint64_t root, bool tail);
size_t enum_tag(TypeInfo *info, uintptr_t addr);
void drop_in_place(uintptr_t addr, uint32_t ty, uint64_t owner);
void drop_in_place_tail(uintptr_t addr, uint32_t ty, uint64_t owner, bool tail);
void box_drop(uintptr_t addr, uint32_t t, bool tail);
void tree_drop(uintptr_t addr, size_t pos);
void tree_free(uintptr_t p, uint64_t cells);
void drop_fn_handle(uintptr_t handle);
uint64_t new_anonymous(Rt *r, uintptr_t addr, uint32_t ty);
void end_identity(Rt *r, uint64_t obj);
void end_identity_in(Rt *r, uint64_t obj, bool forget);
void forget_slots(Rt *r, uintptr_t addr, uint64_t size);
size_t slots_few(Rt *r, uintptr_t addr, uint64_t size, bool take, addr_tok *out);

typedef struct
{
    uint64_t tok;
    uint64_t id;
} establish_ret;
establish_ret establish_reclaimed(Rt *r, uintptr_t a, uint32_t ty, bool ephemeral);

/* D-0202: the entries by address (local holds on the heaps whose page
   buckets name the cells), inline so that one thread pays a load and a
   branch. */
Held enter_elem_multi(uintptr_t a, uint32_t ty);
Held enter_live_empty_multi(uintptr_t lo, uintptr_t hi);
Held enter_live_multi(uintptr_t lo, uintptr_t hi);
Held enter_cells_multi(uintptr_t a, uint64_t n);
extern Held (*MT_enter_elem)(uintptr_t a, uint32_t ty);
extern Held (*MT_enter_live)(uintptr_t lo, uintptr_t hi);
extern Held (*MT_enter_live_empty)(uintptr_t lo, uintptr_t hi);
extern Held (*MT_enter_cells)(uintptr_t a, uint64_t n);
CB_ALWAYS Held enter_elem(uintptr_t a, uint32_t ty)
{
    if (!multi())
    {
        return H_SOLO;
    }
    return HOOK(MT_enter_elem)(a, ty);
}
CB_ALWAYS Held enter_live(uintptr_t lo, uintptr_t hi)
{
    if (!multi())
    {
        return H_SOLO;
    }
    return HOOK(MT_enter_live)(lo, hi);
}
CB_ALWAYS Held enter_live_empty(uintptr_t lo, uintptr_t hi)
{
    if (!multi())
    {
        return H_SOLO;
    }
    return HOOK(MT_enter_live_empty)(lo, hi);
}
CB_ALWAYS Held enter_cells(uintptr_t a, uint64_t n)
{
    if (!multi())
    {
        return H_SOLO;
    }
    return HOOK(MT_enter_cells)(a, n);
}
uint64_t live_reclaimed(Rt *r, uintptr_t a); /* 0: none */
uint64_t elem_borrow(void *addr, uint32_t ty, uint32_t mode, bool stamp, bool light);
bool cells_local(uintptr_t a, uintptr_t hi);
bool foreign_held(Rt *r, uint64_t obj);
void check_not_foreign(Rt *r, const uint64_t *objs, size_t n);
void release_cells(void *addr, uint64_t n);
void release_plain_cells(void *addr, uint64_t n);

/* ---- expected from rt_refs.c ---- */

bool solitary(Rt *r, uint64_t of, const uint64_t *excludes, size_t n);
void check_root_access(Rt *r, uint64_t of, bool exclusive, uint32_t file, uint32_t line);
uint64_t check_access(Rt *r, uint64_t tok, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line);
projvec proj_of_addr(uint64_t obj, uintptr_t addr, uint32_t ty);
void free_block(uint8_t *addr, uint64_t size);
void cb_deallocate(void *addr, uint64_t size, uint64_t align);
void cb_read(uint64_t base, const CbProj *p, size_t n, uint32_t file, uint32_t line);
void cb_write(uint64_t base, const CbProj *p, size_t n, uint32_t target_resource, uint32_t file, uint32_t line);
/* How many blocks are over-aligned (not plain `malloc`'s). */
extern _Atomic size_t OVER_ALIGNED;
/* NOTE for rt_refs.c: `drop_fn_handle` frees a closure box with `free(b)`
   and its data with `free(b->data)`: `cb_fn_box` must allocate both with
   `malloc` (glibc's and MinGW's are 16-aligned on x86_64, as
   `box_storage` needs), and `cb_fn_copy` likewise. */

/* ---- expected from rt_threads.c ---- */

void handle_destructor(uint64_t tid);
void guard_drop(uintptr_t addr, uint64_t cells_off);
/* What ends a thread handle and a lock guard: set by what makes one. */
extern void (*HANDLE_END)(uint64_t tid);
/* Report 134: set by the first spawn: at the program's end, the threads
   still running are waited for. */
extern void (*THREADS_AT_END)(void);
extern void (*GUARD_END)(uintptr_t addr, uint64_t cells_off);
bool trace_events_on(void);
bool verify_on(void);
void verify_state(void);
void replay_out(uint64_t me, const char *why);
void replay_frame_push(uint64_t me);
void replay_frame_pop(uint64_t me);
void replay_stmt_push(uint64_t me);
void replay_stmt_pop(uint64_t me);
void replay_establish(uint64_t me, uint64_t obj, bool resource, uintptr_t addr, uint64_t size);
void replay_forget(uint64_t obj);
void replay_moved(uint64_t obj, uintptr_t addr);
void replay_bind(uint64_t me, uint64_t obj, uint64_t root);
void replay_take(uint64_t me, uint64_t obj, uint64_t root);
void replay_end(uint64_t me, uint64_t obj, bool resource);
void replay_frame_hold(uint64_t me, uint64_t tok);
void replay_frame_unhold(uint64_t me, uint64_t tok);
void replay_dead(uint64_t tok);

#endif
