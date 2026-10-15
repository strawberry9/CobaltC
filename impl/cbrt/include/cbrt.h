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
void cb_fn_shared(void *handle, uint32_t file, uint32_t line);   /* D-0218: a fn value read through a shared reference holds no exclusive capture */
void cb_fn_shared_value(const void *addr, uint32_t ty, uint32_t file, uint32_t line);   /* ... the same for each fn slot of a value */

/* program */
void cb_end_binding(uint64_t root);                  /* D-0111: a reference binding ends at its last use — [Ref-Binding-Last-Use] (D-0111): a reference binding's object and path end at its last use */
void cb_init(const char *const *files, size_t nfiles, const cb_type *types, size_t ntypes, const uint32_t *ids);   /* [Program]: Σ_0 — the files, the type descriptors and the static objects; internal to the start-up text */
_Noreturn void cb_fault(const char *diag, uint32_t file, uint32_t line);   /* [Fault-Unwind]: the diagnostic is raised at file:line; every open scope and frame of this thread is unwound ([Stmt-Exit], [Block-Exit], destructors run), then the report is written and the program ends with status 1; [Fault-Contain] in a supervised thread */
_Noreturn void cb_exit(uint8_t status);                                   /* std::exit (D-0150): unwinds this thread, ends with status — `exit` (D-0150): unwinds this thread as [Fault-Unwind] does, reporting nothing, and ends with the status */
_Noreturn void cb_fault_msg(const char *diag, const uint8_t *msg, uint64_t n, uint32_t file, uint32_t line);   /* assert (D-0065) — [Fault-Unwind] with a message (assert, D-0065) */
_Noreturn void cb_terminate_ok(uint8_t status);        /* ok(s): main's value, or 0 — [Program]'s end: every frame has exited; the output is written out and the process ends with main's value */
void cb_set_args(int argc, char **argv);                /* the program's arguments (rule.stdlib.args) — [Arg-Bytes]'s source: the program's arguments (rule.stdlib.args) */
uint64_t cb_frame_here(void);                           /* D-0033 (1): a binding's frame, for cb_rebind — internal: the current frame's id, for cb_rebind */
uint64_t cb_rebind(uint64_t root, void *addr, uint32_t ty, uint64_t frame, uint32_t file, uint32_t line);   /* [Let]'s re-initialisation of a moved-from binding (D-0033 (1)): a new object at `addr`, bound in the binding's own frame */
/* The statement now executing, for faults raised without a location of
   their own (by pops, destructors): a C thread-local the generated
   program defines, set inline at each statement, read by the runtime
   through cb_where. */
extern __thread uint32_t cb_at_file, cb_at_line;
static inline void cb_at(uint32_t file, uint32_t line) { cb_at_file = file; cb_at_line = line; }   /* internal: the statement now executing, for faults raised without a location */
void cb_where(uint32_t *file, uint32_t *line);       /* defined by the generated program — internal: reads cb_at's location (defined by the generated program) */
void cb_set_where(uint32_t file, uint32_t line);     /* likewise — internal: sets it */
/* D-0077: this thread's stack floor (set by the runtime when the program
   and each thread start; 0 while unknown), checked at each function's
   entry: a call below it is `diag.stack-exhausted`, not a crash. */
extern __thread uintptr_t cb_stack_floor;   /* internal: this thread's stack floor (D-0077) */
void cb_set_stack_floor(uintptr_t floor);            /* defined by the generated program — internal: set by the runtime when the thread starts */
_Noreturn void cb_stack_exhausted(void);   /* [Call]'s stack limit (D-0077): `diag.stack-exhausted` raised as [Fault-Unwind] */
_Noreturn void cb_run_main(void (*start)(void));   /* runs the program on a thread with a large stack — [Program]: runs main on a thread with a large stack (ℓ_0) */
/* D-0257: library mode (`cobc --lib`): an entry point's stub starts the
   runtime once and registers its thread (enter), writes out what it
   printed (leave); a `_try` stub sets its jump buffer and reads the
   contained fault's code and report. */
void cb_lib_enter(void (*init)(void));   /* [Program] for a library (D-0257): starts the runtime once and registers the calling C thread */
void cb_lib_leave(void);   /* internal: writes out what the call printed */
void cb_lib_shutdown(void);   /* internal: cobalt_shutdown */
void cb_lib_jmp(void *jb);   /* [Fault-Contain] for a library entry's `_try` form (D-0257): the stub's jump buffer */
int cb_lib_fault_code(void);   /* internal: the contained fault's status, after the jump */
int cb_lib_last_fault(char *buf, size_t n);   /* internal: cobalt_last_fault's report */
static inline void cb_stack_check(void)   /* [Call]'s stack-limit check at a function's entry (D-0077): below the floor is `diag.stack-exhausted` */
{
    /* The frame's address, not a local's: taking a local's address made
       GCC add a stack-protector canary to every function. */
    if (__builtin_expect((uintptr_t)__builtin_frame_address(0) < cb_stack_floor, 0)) cb_stack_exhausted();
}
/* standard library externs: each an instance of [Extern-Call] implementing the std rule its comment names */
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
/* bit operations (D-0123): on a value's unsigned image of `w` bits, and rotation */
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
/* `round`, half away from zero, from `trunc` (exact: the dropped fraction
   is representable), not the C library's: MinGW's over msvcrt gives 1 for
   0.49999999999999994 (D-0255). coby's `round_f64` is the same. */
static inline double cb_round(double x)
{
    double t = trunc(x);
    return fabs(x - t) >= 0.5 ? t + copysign(1.0, x) : t;
}
static inline float cb_roundf(float x)
{
    float t = truncf(x);
    return fabsf(x - t) >= 0.5f ? t + copysignf(1.0f, x) : t;
}
static inline void cb_index_check(uint64_t i, uint64_t n, uint32_t file, uint32_t line)
{
    if (__builtin_expect(i >= n, 0)) cb_fault("diag.index-out-of-bounds", file, line);
}

/* frames and statement scopes (spec/14 §1, §1a) */
void cb_frame_push(void);   /* [Block-Enter], [Call]: pushes a frame (state.frame-stack); its pop ends what it owns */
void cb_frame_pop(void);   /* [Block-Exit]: ends the frame's owned objects last bound first, [Destroy] for a resource (its destructor, [Run-Destructor], [Destroy-Composite]), [Object-End] for the rest; drops the frame's holds */
void cb_frame_push_at(uint32_t file, uint32_t line);   /* [Block-Enter] with the block's location, for a destruction fault */
void cb_stmt_push(void);   /* [Stmt-Enter]: a statement scope for this statement's temporaries and unheld paths */
void cb_stmt_push_at(uint32_t file, uint32_t line);   /* [Stmt-Enter] with the statement's location */
void cb_stmt_pop(void);   /* [Stmt-Exit]: ends the scope's temporaries ([Destroy]/[Object-End]) and its unheld paths; the result was re-stamped out by cb_result */

/* objects (state.objects, state.holder, state.init, obligations) */
uint64_t cb_new(void *addr, uint32_t ty, uint32_t file, uint32_t line);         /* a temporary of this statement — [Object-Establish-Plain]/[Object-Establish-Resource] over `addr` (storage the C provides, not the runtime's) with init valid, a temporary of the current statement; assumes `addr` holds a value of `ty` */
uint64_t cb_new_uninit(void *addr, uint32_t ty, uint32_t file, uint32_t line);  /* [Let-Uninit] — [Let-Uninit]: [Object-Establish-*] with init uninitialized; assumes a whole-object [Write] before any read (definite assignment) */
uint64_t cb_bind(uint64_t obj);                     /* adopt into the current frame; the root token — [Binding-Form] and [Store-Binding-Temp]: the current frame adopts the temporary (state.holder); the root path's token is returned for the binding's accesses */
void     cb_move_to(uint64_t obj, void *addr);      /* the same object, now at this address — internal: the same object, its cells now at `addr` (the C moved the bytes; the record follows). Assumes no path's target was derived from the old address */
uint64_t cb_temp_path(uint64_t obj);                /* an owner-like path to a temporary (calling a closure temporary) — [Temp-Root]: an owner-like root path to a temporary (a closure temporary called) */
uint64_t cb_take(uint64_t root, uint32_t file, uint32_t line);   /* [Authority-Transfer] out of a binding — [Store-Binding-Place-Transfer]/[Authority-Transfer] out of a binding: solitary and authority checked; the root path ends, the object is in flight */
void     cb_absorb(uint64_t obj);                   /* [Relocate-In]: now part of a container — [Relocate-In]: the temporary's contents are now a sub-resource of a container; its record ends without destruction, its obligations move */
void     cb_absorb_refs(uint64_t obj);              /* the same for plain data holding references, moved in first: its slots stay — [Relocate-In] for plain data holding references, moved in first: its slots' holds stay */
void     cb_result(uint64_t obj);                   /* a statement's result: re-stamped outward — [Stmt-Exit]'s re-stamp: the statement's result object belongs to the parent scope */
void     cb_destroy(uint64_t root, uint32_t file, uint32_t line);   /* drop(x) — [Destroy] through the binding's root path: temporally valid, solitary, authority held; [Run-Destructor], [Destroy-Composite], [Object-End] */
void     cb_destroy_via(uint64_t tok, uint32_t file, uint32_t line); /* drop(*r): through a reference — [Destroy] through a reference (drop(*r)): the same premises on the referent */
void     cb_consume(uint64_t obj, uint32_t file, uint32_t line);    /* destroy a temporary now — [Destroy] of a temporary now, as [Stmt-Exit] would at the statement's end */
void     cb_end_moved_out(uint64_t obj);            /* end a temporary whose contents relocated out — [Relocate-Out]'s source after a destructuring: the container ends ([Object-End]) without destroying the parts that moved */

/* access paths (state.access-paths; spec/08, spec/10, spec/16) */
uint64_t cb_borrow(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);   /* [Borrow] from `base` through the projections `p` ([Field-Access], [Index-Checked] steps): temporally valid, mode no stronger than the source, ¬clash, [Thread-Affine]; mints the path, held by this statement until stored */
uint64_t cb_borrow_range(uint64_t base, const cb_proj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line);   /* a slice's range (D-0070) — [Slice-Form]: [Borrow] whose last step is the range lo..hi (D-0070) */
void     cb_read(uint64_t base, const cb_proj *p, size_t n, uint32_t file, uint32_t line);   /* [Read]'s premises on the place: temporally valid, ¬clash(shared), [Thread-Affine]; the C reads the value itself */
void     cb_write(uint64_t base, const cb_proj *p, size_t n, uint32_t target_resource, uint32_t file, uint32_t line);   /* [Write]'s premises: temporally valid, mode exclusive, ¬clash(exclusive), ¬live-resource-at when `target_resource`; the C stores the value itself */
void     cb_write_quiet(uint64_t base, const cb_proj *p, size_t n, void *addr, uint32_t ty, uint32_t file, uint32_t line); /* D-0194: a quiet place's old value destroyed by the write — [Write-Quiet-Replace] (D-0194): the old quiet value at `addr` is destroyed ([Destroy]) and the place written */
uint32_t cb_owns(const void *addr, uint32_t ty);   /* D-0049: the value there owns a resource now — `owns(τ, v)` of [Write]'s live-resource-at (D-0049): whether the value at `addr` owns a resource now */

/* reference data (plan D5) */
void     cb_store_ref(void *slot, uint64_t tok);   /* [Write] of a reference value: the slot's holder gains the path (held-by), keyed by the slot's address */
uint64_t cb_load_ref(const void *slot);   /* [Read] of a reference value: the token the slot holds */
void     cb_valid(uint64_t tok, uint32_t file, uint32_t line);   /* [Swap-Places]: temporally valid, no conflict check */
void     cb_copy_datum(void *dst, const void *src, uint32_t ty);   /* [Read] then [Write] of a value holding references: the bytes and each slot's hold are copied */
void     cb_lend(void *addr, uint64_t size, uint32_t file, uint32_t line);   /* D-0213 — [Thread-Lend]: every exclusive path in the value at `addr` is lent (transit) to the thread that receives it */
void     cb_lend_fn(void *h, uint32_t file, uint32_t line);                 /* D-0213 — [Thread-Lend] for a closure's captures */

/* text and hashing helpers */
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
uint64_t cb_elem_borrow(void *addr, uint32_t ty, uint32_t mode);   /* [Reclaim] of the element's cells and [Borrow] of it (Vec::index_*): a path over raw storage */
uint64_t cb_elem_borrow_here(void *addr, uint32_t ty, uint32_t mode);  /* the same, recorded in the current scope (a `…_pre` body) — [Reclaim] and [Borrow] of an element, the path recorded in the current scope (a `…_pre` body) */
void cb_deallocate_plain(uint8_t *addr, uint64_t size);   /* cells that never held a reference — [Deallocate] of cells that never held a reference: no [Release] of reference holders is needed */
void cb_elem_access_slow(void *addr, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line);   /* [Read]/[Write] premises on an element over raw cells where an object lies (otherwise no object, no path, no check) */
extern uintptr_t cb_reclaimed_entries;   /* entries over raw cells (an AtomicUsize); 0: no object over any element — internal: the count of reclaimed objects, the fast path's gate (0: no element has an object) */
extern uint64_t  cb_live_threads;        /* threads running, the main one included (an AtomicU64); 1: this thread is alone (D-0209) — internal: the running threads, the gate of [Thread-Affine] checks (1: alone) */
extern uint8_t cb_sched_flag;            /* 1 under a schedule (`--explore`, `--schedule-seed`); an AtomicU8 — internal: whether a schedule runs the threads ([Thread-Step] under --explore) */
void cb_atomic_point(void);              /* D-0249: an atomic operation, a point where a schedule may pass the turn — [Thread-Step] under a schedule: a point where the turn may pass */
uint64_t cb_atomic_wait(uint64_t op, const void *addr, uint64_t seen); /* D-0253: an atomic's wake count: read, wait, notify — [Atomic-Wake-Read] (op 0), [Atomic-Wait] (1), [Atomic-Notify] (2, 3) on the atomic at `addr` */
void cb_spin_hint(void);                 /* D-0253: the processor's pause — [Spin-Hint] */
extern uint32_t cb_reclaimed_pages[4096];   /* the same by page, `(a >> 12) & 4095` (AtomicU32s) — internal: cb_reclaimed_entries by page, the gate of cb_reclaimed_near */
/* Whether an entry may lie over the `size` bytes at `addr`: an element of
   at most a page lies on at most two, and a larger one is always asked. */
static inline int cb_reclaimed_near(const void *addr, uint64_t size)   /* internal: whether a reclaimed object may lie over the cells */
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
static inline void cb_elem_access(void *addr, uint64_t size, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line)   /* [Read]/[Write] on an element: checked (cb_elem_access_slow) only where an object may lie over it; otherwise no object exists there and no rule applies */
{
    if (__builtin_expect(__atomic_load_n(&cb_reclaimed_entries, __ATOMIC_RELAXED) != 0, 0) && cb_reclaimed_near(addr, size)) cb_elem_access_slow(addr, ty, mode, write, file, line);
}
void cb_drop_local(void *addr, uint32_t ty);   /* [Destroy]/[Object-End] of an objectless local's value (D-0200): the value is destroyed as its binding's end would */
/* D-0200: an objectless `String` or `Vec` of plain elements at `self` ends:
   its buffer (`bytes` bytes at `buf`, never holding a reference, never
   over-aligned) freed at once while the runtime holds no entry on its
   pages; otherwise `cb_drop_local`'s general path. */
static inline void cb_drop_plain_buf(void *self, uint32_t ty, void *buf, uint64_t bytes)   /* [Deallocate] of an objectless String's or Vec's buffer at once when no object lies over it, else cb_drop_local */
{
    if (bytes == 0) return;
    if (__builtin_expect(__atomic_load_n(&cb_reclaimed_entries, __ATOMIC_RELAXED) != 0, 0) && cb_reclaimed_near(buf, bytes)) { cb_drop_local(self, ty); return; }
    free(buf);
}
/* An element of a direct slice: checked only where the slice's entry
   test (`cb_live_in`) found an object over one of its elements. */
static inline void cb_elem_access_if(uint8_t live, void *addr, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line)   /* [Read]/[Write] on an element of a direct slice: checked where cb_live_in found an object */
{
    if (__builtin_expect(live != 0, 0)) cb_elem_access_slow(addr, ty, mode, write, file, line);
}
void     cb_borrow_range_unminted(uint64_t base, const cb_proj *p, size_t n, uint64_t lo, uint64_t hi, uint32_t mode, uint32_t file, uint32_t line); /* `&e[lo..hi]` for a pure direct slice: the checks alone — [Slice-Form]'s premises without minting a path: a direct parameter's slice (D-0209) */
void     cb_borrow_unminted(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line); /* `&p` to a pure direct parameter: the check alone — [Borrow]'s premises without minting: `&p` to a pure direct parameter (D-0191) */
void     cb_borrow_check(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line); /* `&v` straight into Vec::index_*: checks only */  /* Vec::index_*: [Reclaim] + [Borrow] + send (spec/21 §0) — [Borrow]'s premises without minting: `&v` straight into Vec::index_* */
void     cb_vec_drop_plain(void *addr, uint64_t len, uint64_t size);   /* Vec::drop element loop, plain T (spec/21 §0) — [Object-End] of every element object over the buffer, as Vec::drop's loop does for plain elements; [Deallocate] follows */
void     cb_raw_move_in(uint64_t obj, void *addr);               /* `*p = v`, v a resource: [Rawptr-Move-In] */
uint8_t  cb_live_at(void *addr);   /* internal: whether a live object lies at `addr` (a native body's gate) */
uint8_t  cb_live_in(void *addr, uint64_t n);                     /* a live object over any of n cells: a native body takes its general path — internal: whether a live object lies over any of the n cells */
uint8_t  cb_frozen_ok(uint64_t tok, const void *lo, size_t n);   /* CHG-0237: a frozen parameter's body may run — [Read]'s premises for a frozen parameter's whole body (D-0205, CHG-0237): valid, initialized, not lock-derived, no exclusive path over it */
uint8_t  cb_live_in2(void *a, void *b, uint64_t n);             /* cb_live_in of two runs of n cells — internal: cb_live_in of two runs */
uint64_t cb_elem_borrow_datum(void *addr, uint32_t ty, uint32_t mode); /* Some(&elem) returned as reference data — [Reclaim] and [Borrow] of an element returned as reference data (Some(&elem)) */
uint64_t cb_peek_datum_tok(size_t k, size_t n);               /* dual-body dispatch: the k-th of n data's token, left in flight — internal: dual-body dispatch (D-0188): the k-th in-flight datum's token */
uint8_t  cb_paths_disjoint(size_t n, const uint64_t *toks, const uint8_t *excl); /* dual-body dispatch: valid and pairwise disjoint — internal: dual-body dispatch: the arguments' paths valid and pairwise disjoint, so the direct body may run */
uint8_t  cb_paths_arrive(size_t n, const uint64_t *toks, const uint8_t *excl); /* D-0219: and each clash-free as it arrives — [Borrow]'s ¬clash for each argument as it arrives (D-0219), beside cb_paths_disjoint */
uint8_t  cb_direct_ok(size_t n, const uint64_t *toks);        /* D-0209: direct parameters' paths valid, over fresh storage — [Read]/[Write] premises for direct parameters' paths (D-0209): valid, over fresh storage */
void     cb_send_tok_datum(uint64_t tok);                       /* a path formed in a native body, sent as one-reference data — internal: a path a native body formed, sent as one-reference data */
void     cb_send_datum_none(void);                               /* None returned as reference data — internal: None returned as reference data */
void     cb_frame_hold(uint64_t tok);                            /* a reference local with no object: the frame holds its path — a reference local without an object: the frame holds its path (an occurrence) until [Block-Exit] or [Ref-Binding-Last-Use] (D-0111) */
void     cb_frame_unhold(uint64_t tok);                          /* … until it ends early (D-0111) */                                 /* a live object over these cells? (native drops) — [Ref-Binding-Last-Use] (D-0111): the frame's hold ends early */
void     cb_raw_move_out(void *addr, void *dst, uint32_t ty);    /* `*p` read of a resource into dst: [Rawptr-Move-Out] */
void     cb_release(void *addr, uint64_t n);                     /* [Release]: end reclaimed objects starting in the range */
void     cb_release_plain(uint8_t *addr, uint64_t n);             /* cb_release for cells that never hold a reference — [Release] for cells that never hold a reference */
uint8_t *cb_reallocate_plain(uint8_t *addr, uint64_t old, uint64_t new_size, uint64_t align); /* Vec growth of plain elements — Vec::grow of plain elements: [Allocate], [Copy-Raw], [Deallocate] of the old cells */
void    *cb_allocate(uint64_t size, uint64_t align);             /* 0 on failure — [Allocate]: 0 on failure (Err) */
void     cb_deallocate(void *addr, uint64_t size, uint64_t align);   /* [Deallocate]: [Release] of the objects over the cells, then the cells leave storage */
void     cb_copy_raw(void *dst, const void *src, uint64_t n);   /* [Copy-Raw] */

/* values in flight across a call boundary, in argument order */
void     cb_send(uint64_t obj);   /* [Call]'s argument passing: the object is in flight from the caller to the callee's [Store-Binding-Temp] */
uint64_t cb_recv(void *addr);   /* [Store-Binding-Temp] side of a call: the in-flight object, now at `addr`, bound by the callee */
void     cb_send_ref(uint64_t tok);   void cb_recv_ref(void);   /* [Call]: a reference argument in flight, pending until [Store-Binding-Value] binds it (D-0107) */
void     cb_result_ref(uint64_t tok);                     /* a statement's reference result: re-stamped outward — [Stmt-Exit]'s re-stamp for a reference result */
void     cb_send_datum(const void *src, uint32_t ty);   /* [Call]: a value holding references in flight; its slots' holds move with it */
void     cb_recv_datum(void *dst, uint32_t ty);   /* [Store-Binding-Value] of in-flight reference data into `dst` */
uint64_t cb_recv_datum_tok(void);                         /* one reference's data, unheld: its token (0: none) — internal: one reference's in-flight data, unheld: its token */
void     cb_forget_obj(uint64_t obj);                       /* an objectless local's received object: its record ends, its value is kept — internal: an objectless local's received object (D-0200): its record ends, its value is kept by the C */
void     cb_drop_local(void *addr, uint32_t ty);            /* that local ends: its value is destroyed — internal: declared again above, [Destroy]/[Object-End] of an objectless local (D-0200) */

/* threads and mutexes (spec/19; impl/COBC-PLAN.md §9) */
void    *cb_env_alloc(uint64_t size, uint64_t align);              /* a spawned thread's argument block, zeroed, aligned as its struct — [Spawn]: the argument block, storage outside every object */
uint64_t cb_spawn(void (*body)(void *), void *env, uint64_t env_size, uint32_t res_ty);   /* [Spawn]: the thread id */
void     cb_hold(uint64_t obj);                                    /* a resource argument waiting in a block — [Spawn]: a resource argument held in the block until the new thread's [Store-Binding-Temp] */
void     cb_thread_done(uint64_t res_obj);                         /* [Thread-Body-Done]: the result is in the block */
void     cb_env_forget(void *addr, uint64_t size);                 /* the block's references stop being occurrences — [Spawn]: the block's reference cells stop counting as holders once the parameters are bound */
void     cb_drop_fn(void *handle);                                 /* destroy a fn value (a closure box) — [Destroy] of a closure box: the closure object it owns ends (D-0089) */
/* D-0204: supervised threads -- a contained fault's unwind ends at the
   trampoline's setjmp; join_status (std::try_join), cancel, a failure's text */
void     cb_thread_jmp(void *jb);   /* [Fault-Contain] (D-0204): the trampoline's jump buffer, where a contained fault's unwind ends */
void     cb_thread_start(void *env, uint64_t size, void *h);   /* [Spawn] in the new thread: registers it and binds its parameters from the block */
void     cb_thread_failed(void);   /* [Fault-Contain]: the thread's failure is recorded (threads(ℓ).value := failed) */
void     cb_thread_lent(void);   /* [Thread-Lend]'s consequence for containment: a thread lent an exclusive reference is never contained */
uint64_t cb_join_status(uint64_t tid, uint8_t mode);   /* [Try-Join] (mode 0, waits) and [Is-Finished] (mode 1) */
void     cb_cancel(uint64_t tid);   /* [Cancel] (D-0204) */
int64_t  cb_task_op(uint64_t op, uint64_t id, void *out, uint64_t cap);   /* [Failure-Text] and the supervision operations' queries */
uint64_t cb_join(uint64_t tid, void *dst);                         /* [Join]: the result's object, or 0 */
uint64_t cb_lock(uint64_t tok, void *m, uint32_t file, uint32_t line);   /* [Lock]: the lock path's token */
void     cb_lock_bare(void *m, uint32_t file, uint32_t line);   /* D-0200: lock without a guard — [Lock] without a guard object (D-0200, std's own bodies): sync(o) := locked-by; unlocked by cb_unlock_bare */
void     cb_unlock_bare(void *m);   /* [Guard-Drop] without a guard object */
void     cb_unlock_signal(void *m, uint64_t id);   /* D-0200: event_op(3, id) then cb_unlock_bare(m) — [Atomic-Notify]-like event signal (event_op(3, id)) then [Guard-Drop] without a guard, as one step */

/* text and hashing helpers */
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
