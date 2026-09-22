/* cbrt -- the CobaltC runtime's C interface (impl/COBC-PLAN.md §2).
   Generated programs include this text verbatim; the definitions live
   in impl/cbrt/src/lib.rs. */
#ifndef CBRT_H
#define CBRT_H
#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <math.h>

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
void *cb_fn_box(uint64_t obj, void *code, uint32_t ty);
void *cb_fn_copy(void *handle, uint32_t file, uint32_t line);   /* [Read] of a fn value: clones a closure box */

/* program */
void cb_init(const char *const *files, size_t nfiles, const cb_type *types, size_t ntypes);
_Noreturn void cb_fault(const char *diag, uint32_t file, uint32_t line);
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
int64_t cb_write_out(const void *p, uint64_t n);   /* the `write` extern */
int64_t cb_write_err(const void *p, uint64_t n);   /* `std`'s `write_err` (eprintf) */
int64_t cb_read_in(void *p, uint64_t n);          /* the `read` extern */
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
typedef struct { uint32_t kind; uint32_t width; uint64_t hi, lo; double f; const uint8_t *p; uint64_t n; } cb_fmt_arg;
void cb_format(const uint8_t *f, uint64_t fn, const cb_fmt_arg *args, uint64_t n, cb_str *out);   /* `$fmt` (rule.stdlib.format) */
int64_t cb_file_read(const void *path, uint64_t path_len, void *buf, uint64_t cap);   /* `read_file` (rule.stdlib.file) */
int64_t cb_file_write(const void *path, uint64_t path_len, const void *buf, uint64_t len);
int64_t cb_file_op(uint64_t op, uint64_t h, void *buf, uint64_t n);  /* `File` (rule.stdlib.file-handle) */
int64_t cb_file_at(uint64_t op, uint64_t h, uint64_t pos);           /* `File::seek`, `File::len` */
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
void cb_stmt_push(void);
void cb_stmt_pop(void);

/* objects (state.objects, state.holder, state.init, obligations) */
uint64_t cb_new(void *addr, uint32_t ty, uint32_t file, uint32_t line);         /* a temporary of this statement */
uint64_t cb_new_uninit(void *addr, uint32_t ty, uint32_t file, uint32_t line);  /* [Let-Uninit] */
uint64_t cb_bind(uint64_t obj);                     /* adopt into the current frame; the root token */
void     cb_move_to(uint64_t obj, void *addr);      /* the same object, now at this address */
uint64_t cb_temp_path(uint64_t obj);                /* an owner-like path to a temporary (calling a closure temporary) */
uint64_t cb_take(uint64_t root, uint32_t file, uint32_t line);   /* [Transfer] out of a binding */
void     cb_absorb(uint64_t obj);                   /* [Relocate-In]: now part of a container */
void     cb_result(uint64_t obj);                   /* a statement's result: re-stamped outward */
void     cb_destroy(uint64_t root, uint32_t file, uint32_t line);   /* drop(x) */
void     cb_destroy_via(uint64_t tok, uint32_t file, uint32_t line); /* drop(*r): through a reference */
void     cb_consume(uint64_t obj, uint32_t file, uint32_t line);    /* destroy a temporary now */
void     cb_end_moved_out(uint64_t obj);            /* end a temporary whose contents relocated out */

/* access paths (state.access-paths; spec/08, spec/10, spec/16) */
uint64_t cb_borrow(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line);
void     cb_read(uint64_t base, const cb_proj *p, size_t n, uint32_t file, uint32_t line);
void     cb_write(uint64_t base, const cb_proj *p, size_t n, uint32_t target_resource, uint32_t file, uint32_t line);
uint32_t cb_owns(const void *addr, uint32_t ty);   /* D-0049: the value there owns a resource now */

/* reference data (plan D5) */
void     cb_store_ref(void *slot, uint64_t tok);
uint64_t cb_load_ref(const void *slot);
void     cb_valid(uint64_t tok, uint32_t file, uint32_t line);   /* [Swap-Places]: temporally valid, no conflict check */
void     cb_copy_datum(void *dst, const void *src, uint32_t ty);

/* raw storage (spec/20 §2, spec/21 §0): reclaimed objects live at raw
   addresses, owned by no frame; `reclaim` attaches or re-attaches one */
uint64_t cb_reclaim(void *addr, uint32_t ty);                    /* [Reclaim]: the object's root token */
uint64_t cb_elem_borrow(void *addr, uint32_t ty, uint32_t mode);
uint64_t cb_elem_borrow_here(void *addr, uint32_t ty, uint32_t mode);  /* the same, recorded in the current scope (a `…_pre` body) */
void cb_elem_access(void *addr, uint32_t ty, uint32_t mode, uint32_t write, uint32_t file, uint32_t line);   /* *Vec::index_*(...) read or written at once */
void     cb_borrow_check(uint64_t base, const cb_proj *p, size_t n, uint32_t mode, uint32_t file, uint32_t line); /* `&v` straight into Vec::index_*: checks only */  /* Vec::index_*: [Reclaim] + [Borrow] + send (spec/21 §0) */
void     cb_vec_drop_plain(void *addr, uint64_t len, uint64_t size);   /* Vec::drop element loop, plain T (spec/21 §0) */
void     cb_raw_move_in(uint64_t obj, void *addr);               /* `*p = v`, v a resource: [Rawptr-Move-In] */
void     cb_raw_move_out(void *addr, void *dst, uint32_t ty);    /* `*p` read of a resource into dst: [Rawptr-Move-Out] */
void     cb_release(void *addr, uint64_t n);                     /* [Release]: end reclaimed objects starting in the range */
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

/* threads and mutexes (spec/19; impl/COBC-PLAN.md §9) */
void    *cb_env_alloc(uint64_t size);                              /* a spawned thread's argument block, zeroed */
uint64_t cb_spawn(void (*body)(void *), void *env, uint64_t env_size, uint32_t res_ty);   /* [Spawn]: the thread id */
void     cb_hold(uint64_t obj);                                    /* a resource argument waiting in a block */
void     cb_thread_done(uint64_t res_obj);                         /* [Thread-Body-Done]: the result is in the block */
void     cb_env_forget(void *addr, uint64_t size);                 /* the block's references stop being occurrences */
void     cb_drop_fn(void *handle);                                 /* destroy a fn value (a closure box) */
uint64_t cb_join(uint64_t tid, void *dst);                         /* [Join]: the result's object, or 0 */
uint64_t cb_lock(uint64_t tok, void *m, uint32_t file, uint32_t line);   /* [Lock]: the lock path's token */

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

static inline uint8_t cb_bytes_eq(const void *a, uint64_t an, const void *b, uint64_t bn)
{
    return an == bn && (an == 0 || memcmp(a, b, an) == 0);
}

#endif
