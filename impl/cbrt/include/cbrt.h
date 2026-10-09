/* cbrt -- the CobaltC runtime's C interface (impl/COBC-PLAN.md §2).
   Generated programs include this text verbatim; the definitions live
   in impl/cbrt/src/lib.rs. */
#ifndef CBRT_H
#define CBRT_H
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>

typedef struct { const uint8_t *p; uint64_t n; } cb_str;   /* [Sizeof-Str]: 16 */
typedef struct {} cb_unit;                                  /* [Sizeof-Unit]: 0 */
typedef __int128 cb_i128;
typedef unsigned __int128 cb_u128;

/* A reference in a register: the address and its access-path token.
   In memory a reference is the bare pointer ([Repr-Ref]); the token is
   kept by the runtime, keyed by the slot's address (cb_store_ref). */
typedef struct { void *p; uint64_t tok; } cb_ref;

typedef struct { uint32_t kind; uint64_t idx; } cb_proj;   /* a projection step */
#define CB_FIELD   0u
#define CB_INDEX   1u
#define CB_PAYLOAD 2u

#define CB_SHARED    0u
#define CB_EXCLUSIVE 1u

#define CB_NOLOC  0xFFFFFFFFu   /* a line inside the prelude: reported as "unknown location" */
#define CB_NOTYPE 0xFFFFFFFFu

/* Type descriptors: what the runtime needs to end and destroy values
   it never sees the source of (impl/COBC-PLAN.md §2). */
typedef struct { uint64_t off; uint32_t ty; } cb_field;
typedef struct {
    const char *name;
    uint64_t size;
    uint32_t kind;                 /* CB_K_* */
    uint8_t is_resource;
    uint8_t has_refs;
    void (*drop)(cb_ref);          /* the user destructor, or 0 */
    uint32_t nfields;  const cb_field *fields;     /* struct: declaration order */
    uint32_t nvariants; const uint32_t *variants;  /* enum: payload type per variant, CB_NOTYPE if none */
    uint64_t payload_off;
    uint32_t elem;  uint64_t count;                /* array */
    uint8_t owner;                 /* declared `resource`, or with a destructor (D-0049) */
} cb_type;
#define CB_K_SCALAR 0u
#define CB_K_STRUCT 1u
#define CB_K_ENUM   2u
#define CB_K_ARRAY  3u
#define CB_K_REF    4u
#define CB_K_OTHER  5u
#define CB_K_FN     6u
#define CB_K_HANDLE 7u   /* handle<T>: the thread id; destructor [Handle-Destructor] */
#define CB_K_GUARD  8u   /* guard<T>: the mutex interior's address + lock path in the slot table */

/* A `fn` value: an 8-byte handle to a callable box — one interned box
   per fn item, or a heap box owning a moved-in closure object, called
   through an exclusive borrow of that object ([Closure-Call]). */
typedef struct { void *code; void *data; uint64_t root; uint32_t ty; uint32_t closure; } cb_fnbox;
void *cb_fn_item(void *code);
void *cb_fn_item_pure(void *code);   /* a fn item whose reference parameters are all pure direct (D-0191) */
uint8_t cb_fn_pure(void *h);         /* whether h is such a fn item or closure */
void cb_fn_mark_pure(void *code);    /* a closure code that keeps nothing of its arguments (D-0192) */
void *cb_fn_box(uint64_t obj, void *code, uint32_t ty);
uint64_t cb_borrow_unstamped(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);   /* a borrow in no statement scope, ended by cb_call_done */
uint64_t cb_call_self(uint64_t root);   /* a closure call's `self`: an unstamped exclusive borrow */
void cb_call_done(uint64_t tok);   /* ... ended after the call when nothing holds it */
void *cb_fn_copy(void *handle, uint32_t file, uint32_t line);   /* [Read] of a fn value: clones a closure box */
void *cb_fn_read(void **slot, uint32_t file, uint32_t line);   /* D-0089: copy, or move out when the closure owns a resource */
void cb_fn_split(void *dst, void *src, uint32_t ty, uint32_t file, uint32_t line);   /* D-0089: the same for each fn slot of a value */
_Noreturn void cb_fn_moved(void);   /* D-0089: a call through an emptied fn slot */
void cb_drop_value(void *addr, uint32_t ty);   /* D-0089: a plain temporary holding fn values ends */

/* program */
void cb_end_binding(uint64_t root);                  /* D-0111: a reference binding ends at its last use */
void cb_init(const char *const *files, size_t nfiles, const cb_type *types, size_t ntypes, const uint32_t *ids);
_Noreturn void cb_fault(const char *diag, uint32_t file, uint32_t line);
_Noreturn void cb_exit(uint8_t status);                                   /* std::exit (D-0150): unwinds this thread, ends with status */
_Noreturn void cb_fault_msg(const char *diag, const uint8_t *msg, uint64_t n, uint32_t file, uint32_t line);   /* assert (D-0065) */
_Noreturn void cb_terminate_ok(uint8_t status);        /* ok(s): main's value, or 0 */
void cb_set_args(int argc, char **argv);                /* the program's arguments (rule.stdlib.args) */
uint64_t cb_frame_here(void);                           /* D-0033 (1): a binding's frame, for cb_rebind */
uint64_t cb_rebind(uint64_t root, void *addr, uint32_t ty, uint64_t frame, uint32_t file, uint32_t line);
/* The statement now executing, for faults raised without a location of
   their own (by pops, destructors): a C thread-local the generated
   program defines, set inline at each statement, read by the runtime
   through cb_where. */
extern __thread uint32_t cb_at_file, cb_at_line;
static inline void cb_at(uint32_t file, uint32_t line) { cb_at_file = file; cb_at_line = line; }
void cb_where(uint32_t *file, uint32_t *line);       /* defined by the generated program */
void cb_set_where(uint32_t file, uint32_t line);     /* likewise */
/* D-0077: this thread's stack floor (set by the runtime when the program
   and each thread start; 0 while unknown), checked at each function's
   entry: a call below it is `diag.stack-exhausted`, not a crash. */
extern __thread uintptr_t cb_stack_floor;
void cb_set_stack_floor(uintptr_t floor);            /* defined by the generated program */
_Noreturn void cb_stack_exhausted(void);
_Noreturn void cb_run_main(void (*start)(void));   /* runs the program on a thread with a large stack */
static inline void cb_stack_check(void)
{
    /* The frame's address, not a local's: taking a local's address made
       GCC add a stack-protector canary to every function. */
    if (__builtin_expect((uintptr_t)__builtin_frame_address(0) < cb_stack_floor, 0)) cb_stack_exhausted();
}
int64_t cb_write_out(const void *p, uint64_t n);   /* the `stdout_write` extern */
int64_t cb_write_err(const void *p, uint64_t n);   /* `std`'s private `stderr_write` (eprintf) */
int64_t cb_flush_out(void);                         /* `std`'s private `stdout_flush` (D-0178) */
int64_t cb_read_in(void *p, uint64_t n);          /* the `stdin_read` extern */
int64_t cb_read_line_in(void *p, uint64_t n);     /* `std`'s private `stdin_read_line` (read_line) */
int64_t cb_arg_bytes(uint64_t i, void *p, uint64_t n);   /* the `arg_bytes` extern */
void cb_print_int(uint64_t hi, uint64_t lo, uint32_t is_signed);   /* `print` (rule.stdlib.print) */
void cb_print_f64(double v);
void cb_print_f32(float v);
void cb_print_bool(uint32_t b);
uint64_t cb_text_int(uint64_t hi, uint64_t lo, uint32_t is_signed, void *buf);   /* `String::append` (rule.stdlib.text) */
uint64_t cb_text_f64(double v, void *buf);
uint64_t cb_text_f32(float v, void *buf);
uint64_t cb_text_bool(uint32_t b, void *buf);
int64_t cb_parse_int(const void *p, uint64_t n, uint32_t is_signed, uint32_t bits, uint64_t *out);   /* `parse` */
int64_t cb_parse_float(const void *p, uint64_t n, uint32_t is_f32, double *out);
int64_t cb_parse_bool(const void *p, uint64_t n, uint8_t *out);                /* D-0181 */
typedef struct { uint32_t kind; uint32_t width; uint64_t hi, lo; double f; const uint8_t *p; uint64_t n; } cb_fmt_arg;
void cb_format(const uint8_t *f, uint64_t fn, const cb_fmt_arg *args, uint64_t n, cb_str *out);   /* `$fmt` (rule.stdlib.format) */
int64_t cb_file_read(const void *path, uint64_t path_len, void *buf, uint64_t cap);   /* `read_file` (rule.stdlib.file) */
int64_t cb_file_write(const void *path, uint64_t path_len, const void *buf, uint64_t len);
int64_t cb_file_op(uint64_t op, uint64_t h, void *buf, uint64_t n);  /* `File` (rule.stdlib.file-handle) */
int64_t cb_file_at(uint64_t op, uint64_t h, uint64_t pos);           /* `File::seek`, `File::len` */
int64_t cb_fs_op(uint64_t op, const void *a, uint64_t an, const void *b, uint64_t bn);   /* filesystem (D-0061) */
int64_t cb_fs_query(uint64_t op, const void *a, uint64_t an, void *buf, uint64_t cap);
int64_t cb_clock_read(uint64_t which);
int64_t cb_tz_offset(int64_t moment);                                                                                              /* local time (D-0147); `unix` is a C macro */
int64_t cb_sleep_ns(uint64_t ns);                                /* D-0124 */
int64_t cb_path_op(uint64_t op, uint64_t h, const void *a, uint64_t an, const void *b, uint64_t bn, void *out, uint64_t cap);   /* paths (D-0141) */
int64_t cb_proc_op(uint64_t op, uint64_t h, const void *a, uint64_t an, const void *b, uint64_t bn, void *out, uint64_t cap);   /* processes (D-0143) */
int64_t cb_net_op(uint64_t op, uint64_t h, const void *a, uint64_t an, const void *b, uint64_t bn, void *out, uint64_t cap);    /* networking (D-0144) */
int64_t cb_os_random(void *out, uint64_t n);                                                                                      /* D-0142 */
/* D-0123: bit counting on a value's unsigned image of `w` bits, and rotation. */
static inline uint32_t cb_popcount64(uint64_t v) { return (uint32_t)__builtin_popcountll(v); }
static inline uint32_t cb_clz(uint64_t v, uint32_t w) { return v == 0 ? w : (uint32_t)__builtin_clzll(v) - (64u - w); }
static inline uint32_t cb_ctz(uint64_t v, uint32_t w) { return v == 0 ? w : (uint32_t)__builtin_ctzll(v); }
static inline uint64_t cb_rotl(uint64_t v, uint32_t k, uint32_t w) { uint64_t m = w == 64 ? ~0ull : ((1ull << w) - 1); v &= m; k %= w; return k == 0 ? v : ((v << k) | (v >> (w - k))) & m; }
static inline uint64_t cb_rotr(uint64_t v, uint32_t k, uint32_t w) { uint64_t m = w == 64 ? ~0ull : ((1ull << w) - 1); v &= m; k %= w; return k == 0 ? v : ((v >> k) | (v << (w - k))) & m; }
/* Rotations at a fixed width, in the form C compilers make one instruction of. */
#define CB_ROT(W, T) \
    static inline T cb_rotl##W(T v, uint32_t k) { k %= W; return (T)((v << k) | (v >> ((W - k) % W))); } \
    static inline T cb_rotr##W(T v, uint32_t k) { k %= W; return (T)((v >> k) | (v << ((W - k) % W))); }
CB_ROT(8, uint8_t)
CB_ROT(16, uint16_t)
CB_ROT(32, uint32_t)
CB_ROT(64, uint64_t)
#undef CB_ROT
static inline uint32_t cb_popcount128(cb_u128 v) { return cb_popcount64((uint64_t)v) + cb_popcount64((uint64_t)(v >> 64)); }
static inline uint32_t cb_clz128(cb_u128 v) { uint64_t hi = (uint64_t)(v >> 64); return hi ? cb_clz(hi, 64) : 64 + cb_clz((uint64_t)v, 64); }
static inline uint32_t cb_ctz128(cb_u128 v) { uint64_t lo = (uint64_t)v; return lo ? cb_ctz(lo, 64) : 64 + cb_ctz((uint64_t)(v >> 64), 64); }
static inline cb_u128 cb_rotl128(cb_u128 v, uint32_t k) { k %= 128; return k == 0 ? v : (v << k) | (v >> (128 - k)); }
static inline cb_u128 cb_rotr128(cb_u128 v, uint32_t k) { k %= 128; return k == 0 ? v : (v >> k) | (v << (128 - k)); }
uint64_t cb_event_op(uint64_t op, uint64_t id, uint64_t seen);   /* channels' event counts (D-0063) */
uint8_t cb_bytes_less(const void *a, uint64_t an, const void *b, uint64_t bn);   /* key_less on text (D-0062) */
/* The same, inline, for a native sort's inner loop: byte order, a prefix first. */
static inline uint8_t cb_bytes_lt(const void *a, uint64_t an, const void *b, uint64_t bn)
{
    int c = memcmp(a, b, an < bn ? an : bn);
    return (uint8_t)(c < 0 || (c == 0 && an < bn));
}
void    cb_sort_plain(void *p, uint64_t n, uint32_t kind);   /* Vec::sort of integers or bools (D-0062) */
/* Inline so that `cc` sees the comparison and drops it where the
   program has already proven it (a loop guard `i < n`); the fault is
   cb_fault's, as it would be from inside the runtime. */
static inline void cb_index_check(uint64_t i, uint64_t n, uint32_t file, uint32_t line)
{
    if (__builtin_expect(i >= n, 0)) cb_fault("diag.index-out-of-bounds", file, line);
}

/* frames and statement scopes (spec/14 §1, §1a) */
void cb_frame_push(void);
void cb_frame_pop(void);
void cb_frame_push_at(uint32_t file, uint32_t line);
void cb_stmt_push(void);
void cb_stmt_push_at(uint32_t file, uint32_t line);
void cb_stmt_pop(void);

/* objects (state.objects, state.holder, state.init, obligations) */
uint64_t cb_new(void *addr, uint32_t ty, uint32_t file, uint32_t line);         /* a temporary of this statement */
uint64_t cb_new_uninit(void *addr, uint32_t ty, uint32_t file, uint32_t line);  /* [Let-Uninit] */
uint64_t cb_bind(uint64_t obj);                     /* adopt into the current frame; the root token */
void     cb_move_to(uint64_t obj, void *addr);      /* the same object, now at this address */
uint64_t cb_temp_path(uint64_t obj);                /* an owner-like path to a temporary (calling a closure temporary) */
uint64_t cb_take(uint64_t root, uint32_t file, uint32_t line);   /* [Transfer] out of a binding */
void     cb_absorb(uint64_t obj);                   /* [Relocate-In]: now part of a container */
void     cb_absorb_refs(uint64_t obj);              /* the same for plain data holding references, moved in first: its slots stay */
void     cb_result(uint64_t obj);                   /* a statement's result: re-stamped outward */
void     cb_destroy(uint64_t root, uint32_t file, uint32_t line);   /* drop(x) */
void     cb_destroy_via(uint64_t tok, uint32_t file, uint32_t line); /* drop(*r): through a reference */
void     cb_consume(uint64_t obj, uint32_t file, uint32_t line);    /* destroy a temporary now */
void     cb_end_moved_out(uint64_t obj);            /* end a temporary whose contents relocated out */

/* access paths (state.access-paths; spec/08, spec/10, spec/16) */
uint64_t cb_borrow(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);
uint64_t cb_borrow_range(uint64_t base, const cb_proj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line);   /* a slice's range (D-0070) */
void     cb_read(uint64_t base, const cb_proj *p, size_t n, uint32_t file, uint32_t line);
void     cb_write(uint64_t base, const cb_proj *p, size_t n, uint32_t target_resource, uint32_t file, uint32_t line);
void     cb_write_quiet(uint64_t base, const cb_proj *p, size_t n, void *addr, uint32_t ty, uint32_t file, uint32_t line); /* D-0194: a quiet place's old value destroyed by the write */
uint32_t cb_owns(const void *addr, uint32_t ty);   /* D-0049: the value there owns a resource now */

/* reference data (plan D5) */
void     cb_store_ref(void *slot, uint64_t tok);
uint64_t cb_load_ref(const void *slot);
void     cb_valid(uint64_t tok, uint32_t file, uint32_t line);   /* [Swap-Places]: temporally valid, no conflict check */
void     cb_copy_datum(void *dst, const void *src, uint32_t ty);

/* The first position of the `nn` bytes at `n` in the `hn` bytes at `h`,
   or UINT64_MAX (`StringView::find`'s answer natively; an empty needle
   is found at 0). Pure. */
static inline uint64_t cb_find_bytes(const uint8_t *h, uint64_t hn, const uint8_t *n, uint64_t nn)
{
    if (nn == 0) return 0;
    if (nn > hn) return UINT64_MAX;
    const uint8_t *p = h, *last = h + (hn - nn);
    while (p <= last) {
        p = (const uint8_t *)memchr(p, n[0], (size_t)(last - p) + 1);
        if (!p) return UINT64_MAX;
        if (memcmp(p, n, (size_t)nn) == 0) return (uint64_t)(p - h);
        p++;
    }
    return UINT64_MAX;
}
/* `StringView::boundary`: `i` at either end, or on a byte that does not
   continue a UTF-8 sequence. Pure. */
static inline int cb_char_boundary(const uint8_t *d, uint64_t n, uint64_t i)
{
    return i == 0 || i == n || (d[i] & 0xC0u) != 0x80u;
}

/* raw storage (spec/20 §2, spec/21 §0): reclaimed objects live at raw
   addresses, owned by no frame; `reclaim` attaches or re-attaches one */
uint64_t cb_reclaim(void *addr, uint32_t ty);                    /* [Reclaim]: the object's root token */
uint64_t cb_elem_borrow(void *addr, uint32_t ty, uint32_t mode);
uint64_t cb_elem_borrow_here(void *addr, uint32_t ty, uint32_t mode);  /* the same, recorded in the current scope (a `…_pre` body) */
void cb_deallocate_plain(uint8_t *addr, uint64_t size);   /* cells that never held a reference */
void cb_elem_access_slow(void *addr, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line);
extern uintptr_t cb_reclaimed_entries;   /* entries over raw cells (an AtomicUsize); 0: no object over any element */
extern uint32_t cb_reclaimed_pages[4096];   /* the same by page, `(a >> 12) & 4095` (AtomicU32s) */
/* Whether an entry may lie over the `size` bytes at `addr`: an element of
   at most a page lies on at most two, and a larger one is always asked. */
static inline int cb_reclaimed_near(const void *addr, uint64_t size)
{
    uintptr_t a = (uintptr_t)addr;
    if (size > 4096u) return 1;
    if (size == 0) size = 1;
    return __atomic_load_n(&cb_reclaimed_pages[(a >> 12) & 4095u], __ATOMIC_RELAXED) != 0
        || __atomic_load_n(&cb_reclaimed_pages[((a + size - 1) >> 12) & 4095u], __ATOMIC_RELAXED) != 0;
}
/* *Vec::index_*(...) or a Box's contents read or written at once, `size`
   the element's: checked only where an object lies over the element,
   and none does while the runtime holds no entry on its pages. */
static inline void cb_elem_access(void *addr, uint64_t size, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line)
{
    if (__builtin_expect(__atomic_load_n(&cb_reclaimed_entries, __ATOMIC_RELAXED) != 0, 0) && cb_reclaimed_near(addr, size)) cb_elem_access_slow(addr, ty, mode, write, file, line);
}
void cb_drop_local(void *addr, uint32_t ty);
/* D-0200: an objectless `String` or `Vec` of plain elements at `self` ends:
   its buffer (`bytes` bytes at `buf`, never holding a reference, never
   over-aligned) freed at once while the runtime holds no entry on its
   pages; otherwise `cb_drop_local`'s general path. */
static inline void cb_drop_plain_buf(void *self, uint32_t ty, void *buf, uint64_t bytes)
{
    if (bytes == 0) return;
    if (__builtin_expect(__atomic_load_n(&cb_reclaimed_entries, __ATOMIC_RELAXED) != 0, 0) && cb_reclaimed_near(buf, bytes)) { cb_drop_local(self, ty); return; }
    free(buf);
}
/* An element of a direct slice: checked only where the slice's entry
   test (`cb_live_in`) found an object over one of its elements. */
static inline void cb_elem_access_if(uint8_t live, void *addr, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line)
{
    if (__builtin_expect(live != 0, 0)) cb_elem_access_slow(addr, ty, mode, write, file, line);
}
void     cb_borrow_range_unminted(uint64_t base, const cb_proj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line); /* `&e[lo..hi]` for a pure direct slice: the checks alone */
void     cb_borrow_unminted(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line); /* `&p` to a pure direct parameter: the check alone */
void     cb_borrow_check(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line); /* `&v` straight into Vec::index_*: checks only */  /* Vec::index_*: [Reclaim] + [Borrow] + send (spec/21 §0) */
void     cb_vec_drop_plain(void *addr, uint64_t len, uint64_t size);   /* Vec::drop element loop, plain T (spec/21 §0) */
void     cb_raw_move_in(uint64_t obj, void *addr);               /* `*p = v`, v a resource: [Rawptr-Move-In] */
uint8_t  cb_live_at(void *addr);
uint8_t  cb_live_in(void *addr, uint64_t n);                     /* a live object over any of n cells: a native body takes its general path */
uint8_t  cb_frozen_ok(uint64_t tok, const void *lo, size_t n);   /* CHG-0237: a frozen parameter's body may run */
uint8_t  cb_live_in2(void *a, void *b, uint64_t n);             /* cb_live_in of two runs of n cells */
uint64_t cb_elem_borrow_datum(void *addr, uint32_t ty, uint32_t mode); /* Some(&elem) returned as reference data */
uint64_t cb_peek_datum_tok(size_t k, size_t n);               /* dual-body dispatch: the k-th of n data's token, left in flight */
uint8_t  cb_paths_disjoint(size_t n, const uint64_t *toks, const uint8_t *excl); /* dual-body dispatch: valid and pairwise disjoint */
void     cb_send_tok_datum(uint64_t tok);                       /* a path formed in a native body, sent as one-reference data */
void     cb_send_datum_none(void);                               /* None returned as reference data */
void     cb_frame_hold(uint64_t tok);                            /* a reference local with no object: the frame holds its path */
void     cb_frame_unhold(uint64_t tok);                          /* … until it ends early (D-0111) */                                 /* a live object over these cells? (native drops) */
void     cb_raw_move_out(void *addr, void *dst, uint32_t ty);    /* `*p` read of a resource into dst: [Rawptr-Move-Out] */
void     cb_release(void *addr, uint64_t n);                     /* [Release]: end reclaimed objects starting in the range */
void     cb_release_plain(uint8_t *addr, uint64_t n);             /* cb_release for cells that never hold a reference */
uint8_t *cb_reallocate_plain(uint8_t *addr, uint64_t old, uint64_t new_size, uint64_t align); /* Vec growth of plain elements */
void    *cb_allocate(uint64_t size, uint64_t align);             /* 0 on failure */
void     cb_deallocate(void *addr, uint64_t size, uint64_t align);
void     cb_copy_raw(void *dst, const void *src, uint64_t n);

/* values in flight across a call boundary, in argument order */
void     cb_send(uint64_t obj);
uint64_t cb_recv(void *addr);
void     cb_send_ref(uint64_t tok);   void cb_recv_ref(void);
void     cb_result_ref(uint64_t tok);                     /* a statement's reference result: re-stamped outward */
void     cb_send_datum(const void *src, uint32_t ty);
void     cb_recv_datum(void *dst, uint32_t ty);
uint64_t cb_recv_datum_tok(void);                         /* one reference's data, unheld: its token (0: none) */
void     cb_forget_obj(uint64_t obj);                       /* an objectless local's received object: its record ends, its value is kept */
void     cb_drop_local(void *addr, uint32_t ty);            /* that local ends: its value is destroyed */

/* threads and mutexes (spec/19; impl/COBC-PLAN.md §9) */
void    *cb_env_alloc(uint64_t size);                              /* a spawned thread's argument block, zeroed */
uint64_t cb_spawn(void (*body)(void *), void *env, uint64_t env_size, uint32_t res_ty);   /* [Spawn]: the thread id */
void     cb_hold(uint64_t obj);                                    /* a resource argument waiting in a block */
void     cb_thread_done(uint64_t res_obj);                         /* [Thread-Body-Done]: the result is in the block */
void     cb_env_forget(void *addr, uint64_t size);                 /* the block's references stop being occurrences */
void     cb_drop_fn(void *handle);                                 /* destroy a fn value (a closure box) */
/* D-0204: supervised threads -- a contained fault's unwind ends at the
   trampoline's setjmp; join_status (std::try_join), cancel, a failure's text */
void     cb_thread_jmp(void *jb);
void     cb_thread_start(void *env, uint64_t size, void *h);
void     cb_thread_failed(void);
void     cb_thread_lent(void);
uint64_t cb_join_status(uint64_t tid, uint8_t mode);
void     cb_cancel(uint64_t tid);
int64_t  cb_task_op(uint64_t op, uint64_t id, void *out, uint64_t cap);
uint64_t cb_join(uint64_t tid, void *dst);                         /* [Join]: the result's object, or 0 */
uint64_t cb_lock(uint64_t tok, void *m, uint32_t file, uint32_t line);   /* [Lock]: the lock path's token */
void     cb_lock_bare(void *m, uint32_t file, uint32_t line);   /* D-0200: lock without a guard */
void     cb_unlock_bare(void *m);
void     cb_unlock_signal(void *m, uint64_t id);   /* D-0200: event_op(3, id) then cb_unlock_bare(m) */

static inline uint8_t cb_str_eq(cb_str a, cb_str b)
{
    return a.n == b.n && (a.n == 0 || memcmp(a.p, b.p, a.n) == 0);
}

/* A hash-table key's hash and equality (spec/21 rule.stdlib.hashmap):
   FNV-1a, 64-bit, over the key's bytes. */
static inline uint64_t cb_hash_bytes(const void *p, uint64_t n)
{
    const uint8_t *b = (const uint8_t *)p;
    uint64_t h = 14695981039346656037ull;
    for (uint64_t i = 0; i < n; i++)
    {
        h = (h ^ b[i]) * 1099511628211ull;
    }
    return h;
}

/* D-0110: FNV-1a continued from h over n more bytes (a composite key) */
static inline uint64_t cb_fnv_more(uint64_t h, const void *p, uint64_t n)
{
    const uint8_t *b = (const uint8_t *)p;
    for (uint64_t i = 0; i < n; i++)
    {
        h = (h ^ b[i]) * 1099511628211ull;
    }
    return h;
}

/* D-0108: text in byte order; negative, zero or positive */
static inline int cb_bytes_cmp(const void *a, uint64_t an, const void *b, uint64_t bn)
{
    uint64_t n = an < bn ? an : bn;
    int c = n == 0 ? 0 : memcmp(a, b, n);
    return c != 0 ? c : (an < bn ? -1 : an > bn ? 1 : 0);
}

static inline uint8_t cb_bytes_eq(const void *a, uint64_t an, const void *b, uint64_t bn)
{
    return an == bn && (an == 0 || memcmp(a, b, an) == 0);
}

#endif
