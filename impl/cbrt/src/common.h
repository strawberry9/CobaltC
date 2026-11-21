/* What every part of the C runtime shares: a growable byte buffer (Rust's
   `Vec<u8>`, the answers of the I/O calls and the formatter's text) and the
   internal error, for a state that "cannot happen": one line on standard
   error, `cbrt: internal: ...`, and exit status 4, never undefined
   behaviour. */

#ifndef CBRT_COMMON_H
#define CBRT_COMMON_H

/* MinGW: the C99 printf family of MinGW-w64 itself, not msvcrt's (whose
   formats Windows 7 does not all have). */
#if defined(_WIN32) && !defined(__USE_MINGW_ANSI_STDIO)
#define __USE_MINGW_ANSI_STDIO 1
#endif

/* The format archetype of the runtime's own printf-like functions. */
#ifdef _WIN32
#define CB_PRINTF(a, b) __attribute__((format(gnu_printf, a, b)))
#else
#define CB_PRINTF(a, b) __attribute__((format(printf, a, b)))
#endif

#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>

typedef unsigned __int128 cb_u128;
typedef __int128 cb_i128;

/* A byte vector: `p[0..n)` used, room for `cap`. A zeroed struct is an
   empty buffer. */
typedef struct
{
    uint8_t *p;
    size_t n;
    size_t cap;
} cb_buf;

/* Internal errors: the message after `cbrt: internal: `. */
void cbrt_internal(const char *msg) __attribute__((noreturn, cold));
void cbrt_internalf(const char *fmt, ...) __attribute__((noreturn, cold)) CB_PRINTF(1, 2);
/* Out of memory, as Rust's allocation failure (`memory allocation of N
   bytes failed`, then abort): here an internal error. */
void cbrt_oom(size_t n) __attribute__((noreturn, cold));

static inline void *cb_xmalloc(size_t n)
{
    void *p = malloc(n ? n : 1);
    if (!p)
    {
        cbrt_oom(n);
    }
    return p;
}

static inline void *cb_xcalloc(size_t k, size_t n)
{
    void *p = calloc(k ? k : 1, n ? n : 1);
    if (!p)
    {
        cbrt_oom(k * n);
    }
    return p;
}

static inline void *cb_xrealloc(void *q, size_t n)
{
    void *p = realloc(q, n ? n : 1);
    if (!p)
    {
        cbrt_oom(n);
    }
    return p;
}

/* Room for `extra` more bytes. */
static inline void cb_buf_reserve(cb_buf *b, size_t extra)
{
    if (b->cap - b->n < extra)
    {
        size_t want = b->n + extra;
        size_t c = b->cap ? b->cap * 2 : 64;
        if (c < want)
        {
            c = want;
        }
        b->p = (uint8_t *)cb_xrealloc(b->p, c);
        b->cap = c;
    }
}

static inline void cb_buf_put(cb_buf *b, const void *src, size_t n)
{
    if (n)
    {
        cb_buf_reserve(b, n);
        memcpy(b->p + b->n, src, n);
        b->n += n;
    }
}

static inline void cb_buf_byte(cb_buf *b, uint8_t c)
{
    cb_buf_reserve(b, 1);
    b->p[b->n++] = c;
}

static inline void cb_buf_str(cb_buf *b, const char *s)
{
    cb_buf_put(b, s, strlen(s));
}

static inline void cb_buf_fill(cb_buf *b, uint8_t c, size_t n)
{
    if (n)
    {
        cb_buf_reserve(b, n);
        memset(b->p + b->n, c, n);
        b->n += n;
    }
}

static inline void cb_buf_clear(cb_buf *b)
{
    b->n = 0;
}

static inline void cb_buf_free(cb_buf *b)
{
    free(b->p);
    b->p = NULL;
    b->n = 0;
    b->cap = 0;
}

#endif
