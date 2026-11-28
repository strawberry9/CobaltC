/* Declarations of rt_threads.c (see rt.h): the stack floor and the
   program's own stack (D-0077), spawned threads and their pool (D-0202,
   D-0204), joins, cancellation, channels' event counts (D-0063), atomics'
   waits (D-0253), mutexes and guards (spec/19 §2, D-0200, D-0310), the
   trace events (D-0264) and the whole-state check (D-0261); and of
   replay.c (D-0299 step 4, `COBALTC_REPLAY_TRACE`). */

#ifndef CBRT_RT_THREADS_H
#define CBRT_RT_THREADS_H

/* ---- the stack (D-0077) ---- */

uintptr_t find_stack_floor(void);
uintptr_t floor_above(uintptr_t base, size_t size);
void set_stack_floor(void);
void restore_stack_floor(void);

/* ---- threads ---- */

extern _Atomic uint64_t NEXT_TID;

/* A fresh thread state (`TState` with its location unset), and its end:
   every list it holds freed. */
TState *tstate_new(void);
void tstate_free(TState *t);

void wait_done(uint64_t tid);
void free_env(uint64_t tid);
void handle_destructor(uint64_t tid);
void reraise(uint64_t fid) CB_NORET;
/* Report 238: a failure nothing names any more. Under the book's lock. */
void failure_free(Book *b, uint64_t fid);
void failure_brief(cb_buf *b, Book *bk, uint64_t fid);
void fault_brief(cb_buf *b, const char *raw, uint32_t file, uint32_t line);
void request_cancel(uint64_t tid);

/* ---- event counts, atomics ---- */

uint64_t event_key(uint64_t id);

/* ---- mutexes ---- */

/* Report 315: a mutex's state cells (`[Sizeof-Mutex]`'s `state`, D-0310)
   are the whole of its lock: the holder's thread id in the low bits (0:
   free), a bit set by a thread that registered to wait for it (its release
   then wakes the waiters), and the poison bit. An uncontended lock and its
   release are one compare-and-swap each; the deadlock finder and `[Lock-
   Reentrant]` read the holder from the cells, and each thread keeps the
   list of the mutexes it locked, which `[Fault-Contain]` poisons. */
#define MX_HOLDER 0x000000FFFFFFFFFFull
#define MX_WAITED (1ull << 62)
#define MX_POISON (1ull << 63)
void claim_mutex(uintptr_t key, uintptr_t cells, uint64_t me, uint32_t file, uint32_t line);
uintptr_t state_cells(void *m, uint32_t ty);
/* The holder of the mutex whose state cells are at `cells` (0: free). */
uint64_t mutex_holder(uintptr_t cells);
/* The mutexes this thread `m` holds, poisoned and let go: their keys and
   state cells (`[Fault-Contain]`). */
void poison_held(uint64_t m, u64vec *keys, u64vec *cells);
void held_register(uint64_t tid, TState *t);
void held_unregister(uint64_t tid);
void held_free(TState *t);
void guard_drop(uintptr_t addr, uint64_t cells_off);
void guard_release(uintptr_t addr, uint64_t cells_off);

/* ---- D-0264, D-0261 ---- */

bool trace_events_on(void);
uint64_t verify_stride(void);
bool verify_on(void);
void verify_state(void);

/* ---- the replay trace (replay.c, D-0299 step 4) ----

   Every hook is one relaxed load while `COBALTC_REPLAY_TRACE` is unset.
   `me` is the runtime's thread id (`me()`); steps are a projection's steps
   as the replay's fields (`replay_steps`). */

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
uint64_t replay_borrow(uint64_t me, uint64_t base, const uint64_t *steps, size_t n, bool exclusive);
void replay_minted(uint64_t named, uint64_t tok);
void replay_read(uint64_t me, uint64_t base, const uint64_t *steps, size_t n);
void replay_write(uint64_t me, uint64_t base, const uint64_t *steps, size_t n);
void replay_store(uint64_t me, uintptr_t slot, uint64_t old, uint64_t tok);
void replay_frame_hold(uint64_t me, uint64_t tok);
void replay_param(uint64_t me, uint64_t tok);
void replay_frame_unhold(uint64_t me, uint64_t tok);
void replay_restamp(uint64_t me, uint64_t tok, bool parent);
void replay_flight_in(uint64_t me, uint64_t tok);
void replay_dead(uint64_t tok);
void replay_fault(uint64_t me, const char *id);
void replay_finish(void);

/* ---- expected from rt_init.c ---- */

uint8_t claim_heap(Rt *r);
void set_heap(Rt *r, uint8_t h);
/* The schedule's state under its lock (`sched_lock`), and `sched_wait`,
   which takes the guard and gives it up (Rust's `MutexGuard` moved in). */
struct SchedSt;
struct SchedSt *sched_lock(void);
void sched_wait(uint64_t me, struct SchedSt *g);

/* ---- expected from rt_objs.c (declared there: detach, forget_slots,
   drop_fn_handle, top_scope, stamp_temp, rekey_slots, destroy_obj,
   drop_in_place, light_ref, invalidate, mint_locked, path_target,
   proj_of_addr, cb_move_to) ---- */

/* ---- expected from rt_refs.c ---- */

uint64_t migrate(Rt *r, uint64_t obj);

#endif
