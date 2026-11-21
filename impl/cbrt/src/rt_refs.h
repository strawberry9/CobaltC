/* Declarations of rt_refs.c (see rt.h): allocation, fn values, borrows and
   access checks, reference data and slots, the notes of objectless locals,
   values in flight, the dual-body tests. */

#ifndef CBRT_RT_REFS_H
#define CBRT_RT_REFS_H

/* ---- defined in rt_refs.c ---- */

/* Over-aligned blocks `cb_allocate` made (address -> (size, align)), and
   how many there are; `free_block` routes by them. */
extern _Atomic size_t OVER_ALIGNED;
void free_block(uint8_t *addr, uint64_t size);
/* A closure box's storage (16-aligned): freed with `free`. */
void *box_storage(uint64_t size);

bool excluded(Rt *r, const uint64_t *excludes, size_t n, uint64_t tok);
bool solitary(Rt *r, uint64_t of, const uint64_t *excludes, size_t n);
uint64_t borrow_range_in(uint64_t base, const void *cont, uint32_t cty, const CbProj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line, bool minted);
uint64_t borrow(Rt *r, uint64_t base, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line);
uint64_t light_borrow(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);
uint64_t borrow_in(Rt *r, uint64_t base, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line, bool stamp);
Proj conv(CbProj q);
void check_root_access(Rt *r, uint64_t of, bool exclusive, uint32_t file, uint32_t line);
uint64_t check_access(Rt *r, uint64_t tok, const CbProj *extra, size_t n, bool exclusive, uint32_t file, uint32_t line);
/* `point`: the call's argument paths (NULL: no point check); *clash: a
   clash reported rather than faulted on (point checks only). Returns the
   accessed object. */
uint64_t check_access_with(Rt *r, uint64_t tok, const CbProj *extra, size_t n, bool exclusive, const uint64_t *point, size_t npoint, bool is_point, bool *clash, uint32_t file, uint32_t line);
bool owns(Rt *r, uintptr_t addr, uint32_t ty);
projvec proj_of_addr(uint64_t obj, uintptr_t addr, uint32_t ty);
void ref_offsets(Rt *r, uintptr_t addr, uint32_t ty, uint64_t base, u64vec *out);
void fn_offsets(Rt *r, uintptr_t addr, uint32_t ty, uint64_t base, u64vec *out);
bool fn_owns_resource(Rt *r, void *h);
void end_note_table(uintptr_t addr, uint32_t n);
void fold_frames_and_notes(void);
bool fn_exclusive_capture(void *handle, uint32_t depth);
bool fn_self_writing(void *handle, uint32_t depth);
uint64_t migrate(Rt *r, uint64_t obj);
void unname_sent(Rt *r, uintptr_t a);
bool access_clashes(Rt *r, uint64_t tok, bool exclusive);

/* The exported entries (cbrt.h's prototypes). */
struct cb_note;
void *cb_allocate(uint64_t size, uint64_t align);
void cb_deallocate(void *addr, uint64_t size, uint64_t align);
uint8_t *cb_reallocate_plain(uint8_t *addr, uint64_t old, uint64_t new_size, uint64_t align);
void cb_deallocate_plain(uint8_t *addr, uint64_t size);
void cb_copy_raw(void *dst, const void *src, uint64_t n);
void *cb_fn_item(void *code);
void *cb_fn_item_pure(void *code);
uint8_t cb_fn_pure(void *h);
void cb_fn_mark_pure(void *code);
void *cb_fn_box(uint64_t obj, void *code, uint32_t ty);
void *cb_fn_copy(void *handle, uint32_t file, uint32_t line);
uint64_t cb_borrow(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);
uint64_t cb_borrow_range(uint64_t base, const void *cont, uint32_t cty, const CbProj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line);
void cb_borrow_range_unminted(uint64_t base, const void *cont, uint32_t cty, const CbProj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line);
uint64_t cb_call_self(uint64_t root);
uint64_t cb_borrow_unstamped(uint64_t base, const CbProj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);
void cb_call_done(uint64_t tok);
void cb_point_check(const uint64_t *args, size_t nargs, uint64_t tok, const CbProj *p, size_t n, uint32_t exclusive, const uint8_t *msg, uint64_t msg_len, uint32_t file, uint32_t line);
uint8_t cb_frozen_ok(uint64_t tok, const void *lo, size_t n);
void cb_read(uint64_t base, const CbProj *p, size_t n, uint32_t file, uint32_t line);
uint32_t cb_owns(const void *addr, uint32_t ty);
void cb_write_quiet(uint64_t base, const CbProj *p, size_t n, void *addr, uint32_t ty, uint32_t file, uint32_t line);
void cb_write(uint64_t base, const CbProj *p, size_t n, uint32_t target_resource, uint32_t file, uint32_t line);
void cb_hold_ref(void *slot, uint64_t tok, const void *addr, uint32_t ty, uint32_t mode, uint32_t file, uint32_t line);
uint64_t cb_rec_ref(uint64_t tok, const void *addr, uint32_t ty, uint32_t mode, uint32_t kind, uint32_t file, uint32_t line);
void cb_store_ref(void *slot, uint64_t tok);
void cb_valid(uint64_t tok, uint32_t file, uint32_t line);
uint64_t cb_load_ref(const void *slot);
void *cb_fn_read(void **slot, uint32_t file, uint32_t line);
void cb_fn_split(void *dst, void *src, uint32_t ty, uint32_t file, uint32_t line);
void cb_forget_obj(uint64_t obj);
void cb_notes_push(struct cb_note *t, uint32_t n);
void cb_notes_pop(void);
void cb_notes_drop(struct cb_note *t);
void cb_join_plain(uint64_t tid, void *dst);
void cb_drop_local(void *addr, uint32_t ty);
void cb_drop_value(void *addr, uint32_t ty);
void cb_fn_moved(void) CB_NORET;
void cb_copy_datum(void *dst, const void *src, uint32_t ty);
void cb_lend(void *addr, uint64_t size, uint32_t file, uint32_t line);
void cb_lend_fn(void *handle, uint32_t file, uint32_t line);
void cb_fn_shared(void *handle, uint32_t file, uint32_t line);
void cb_fn_shared_value(const void *addr, uint32_t ty, uint32_t file, uint32_t line);
void cb_send(uint64_t obj);
uint64_t cb_recv(void *addr);
void cb_send_ref(uint64_t tok);
void cb_recv_ref(void);
void cb_result_ref(uint64_t tok);
void cb_send_datum(const void *src, uint32_t ty);
uint64_t cb_elem_borrow_datum(void *addr, uint32_t ty, uint32_t mode);
void cb_send_tok_datum(uint64_t tok);
uint64_t cb_peek_datum_tok(size_t k, size_t n);
uint8_t cb_paths_disjoint(size_t n, const uint64_t *toks, const uint8_t *excl);
uint8_t cb_paths_arrive(size_t n, const uint64_t *toks, const uint8_t *excl);
uint8_t cb_direct_ok(size_t n, const uint64_t *toks);
void cb_send_datum_none(void);
uint64_t cb_recv_datum_tok(void);
void cb_recv_datum(void *dst, uint32_t ty);

/* ---- expected from rt_objs.c ---- */
/* frame_rec, light_ref (static inline), view, PathView: rt_objs.h */
void check_light(const char *what, uint64_t tok, uint32_t file, uint32_t line);
uint64_t mint_in(Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors, projvec *lock_of, bool stamp);
uint64_t mint_locked(Rt *r, uint64_t of, projvec proj, bool exclusive, bool is_root, u64vec ancestors, projvec *lock_of);
uint64_t make_root(Rt *r, uint64_t obj);
void detach(Rt *r, uint64_t obj);
void detach_tok(Rt *r, uint64_t tok);
void stamp_temp(Rt *r, uint64_t obj, size_t scope);
void stamp_tok(Rt *r, uint64_t tok, size_t scope);
size_t top_scope(Rt *r);
size_t scope_below(size_t i);
void retire_token(Rt *r, uint64_t t);
void drop_occ(Rt *r, uint64_t tok);
void end_owned(u64vec owned);
void end_scope(Scope s);
size_t enum_tag(TypeInfo *info, uintptr_t addr);
void drop_in_place(uintptr_t addr, uint32_t ty, uint64_t owner);
void end_identity(Rt *r, uint64_t obj);
Held enter_cells(uintptr_t a, uint64_t n);
void release_cells(void *addr, uint64_t n);
void release_plain_cells(void *addr, uint64_t n);
Held enter_elem(uintptr_t a, uint32_t ty);
uint64_t elem_borrow(void *addr, uint32_t ty, uint32_t mode, bool stamp, bool light);
void cb_move_to(uint64_t obj, void *addr);
uint8_t cb_live_in(void *addr, uint64_t n);

/* plat_aligned_alloc / plat_aligned_free: plat.h. */

/* ---- expected from rt_init.c ---- */
void cb_fault_msg(const char *diag, const uint8_t *msg, uint64_t n, uint32_t file, uint32_t line) CB_NORET;
/* diagnostics' DETAIL_SEP, '\x01' */

/* ---- expected from rt_threads.c / replay.c ---- */
uint64_t cb_join(uint64_t tid, void *dst);
void replay_out(uint64_t me, const char *why);
uint64_t replay_borrow(uint64_t me, uint64_t base, const uint64_t *steps, size_t n, bool exclusive);
void replay_minted(uint64_t named, uint64_t tok);
void replay_read(uint64_t me, uint64_t base, const uint64_t *steps, size_t n);
void replay_write(uint64_t me, uint64_t base, const uint64_t *steps, size_t n);
void replay_store(uint64_t me, uintptr_t slot, uint64_t old, uint64_t tok);
void replay_param(uint64_t me, uint64_t tok);
void replay_restamp(uint64_t me, uint64_t tok, bool parent);
void replay_flight_in(uint64_t me, uint64_t tok);

#endif
