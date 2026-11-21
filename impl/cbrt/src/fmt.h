/* `printf` and `String::appendf`'s format strings (`spec/21` §2f
   `rule.stdlib.format`, D-0038), `print`'s text of a number
   (`rule.stdlib.print`), and `parse<T>`'s scanner (`rule.stdlib.text`
   `[Parse]`). The rules and the recorded runs fix the text to the byte. */

#ifndef CBRT_FMT_H
#define CBRT_FMT_H

#include <stddef.h>
#include <stdint.h>

#include "common.h"

/* A piece of a parsed format: literal bytes (`lit_off`, `lit_len` into the
   list's `lit`) or a specifier. */
typedef struct
{
    uint8_t is_spec;
    uint8_t left;
    uint8_t zero;
    uint8_t plus;
    uint8_t space;
    uint8_t alt;
    /* D-0100: `%*d` / `%.*s`, the width or precision is the next argument. */
    uint8_t width_star;
    uint8_t prec_star;
    uint8_t has_prec;
    uint8_t conv;
    size_t width;
    size_t prec;
    size_t lit_off;
    size_t lit_len;
} fmt_piece;

typedef struct fmt_pieces
{
    fmt_piece *v;
    size_t n;
    size_t cap;
    uint8_t *lit;
    size_t lit_n;
    size_t lit_cap;
} fmt_pieces;

/* An argument as the formatter sees it. */
#define FMT_ARG_INT 0
#define FMT_ARG_FLOAT 1
#define FMT_ARG_TEXT 2
typedef struct
{
    uint32_t kind;
    /* Int: the bit pattern in its own width, and whether signed. */
    uint32_t is_signed;
    uint32_t width;
    cb_u128 bits;
    /* Float: the value, and whether its type is f32 (`%v`'s digits). */
    double f;
    uint32_t is_f32;
    /* Text: bytes valid while the render runs. */
    const uint8_t *p;
    size_t n;
} fmt_arg;

/* The pieces of `f`; 0 when it is a format, else -1 and `out` empty. `out`
   must be zeroed or freed before. */
int fmt_parse(const uint8_t *f, size_t n, fmt_pieces *out);
void fmt_pieces_free(fmt_pieces *ps);

/* The buffer this thread's renders kept, freed: called when the thread ends. */
void fmt_thread_end(void);

/* The text of the pieces with `args`, one per specifier (a `*` takes one
   before its value), appended to `out`. */
void fmt_render_into(const fmt_pieces *ps, const fmt_arg *args, size_t nargs, cb_buf *out);

/* `print`'s text of a float: the shortest digits that read back in its own
   type (f32 when `is_f32`; `v` is then converted to f32 first). Writes at
   most FMT_FLOAT_MAX bytes; returns the length. */
#define FMT_FLOAT_MAX 32
size_t fmt_format_float(double v, int is_f32, char *buf);

/* The decimal text of a 128-bit value (`i128` when `is_signed`), at most 40
   bytes; returns the length. */
size_t fmt_int_text(uint64_t hi, uint64_t lo, int is_signed, char *buf);

/* `numtext`: -1 on success with the value in `*out` (when `out` is not
   NULL), else the code: NUMTEXT_EMPTY, NUMTEXT_OUT_OF_RANGE, or k >= 0
   (`Invalid(k)`, the longest prefix that can still begin a number). */
#define NUMTEXT_EMPTY (-2)
#define NUMTEXT_OUT_OF_RANGE (-3)
int64_t numtext_parse_int(const uint8_t *b, size_t n, int is_signed, uint32_t bits, cb_u128 *out);
int64_t numtext_parse_float(const uint8_t *b, size_t n, int is_f32, double *out);
int64_t numtext_parse_bool(const uint8_t *b, size_t n, uint8_t *out);

#endif
