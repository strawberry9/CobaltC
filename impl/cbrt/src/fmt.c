/* `printf` and `String::appendf`'s format strings (`spec/21` §2f
   `rule.stdlib.format`, D-0038): parsing, and the text each specifier
   produces; `print`'s text of a float; and `parse<T>`'s scanner. The
   Rust standard library's float text (`{:e}`, `{:.*e}`, `{:.*}`) and
   `str::parse` are fltconv.c's exact algorithms. */

#include "fmt.h"
#include "fltconv.h"

#include <string.h>

/* A width or precision above this is not a format; a computed one counts
   as this. */
#define LIMIT 4096

/* ---- parsing ---- */

static void lit_put(fmt_pieces *ps, uint8_t c)
{
    if (ps->lit_n == ps->lit_cap)
    {
        ps->lit_cap = ps->lit_cap ? ps->lit_cap * 2 : 32;
        ps->lit = (uint8_t *)cb_xrealloc(ps->lit, ps->lit_cap);
    }
    ps->lit[ps->lit_n++] = c;
}

static void piece_push(fmt_pieces *ps, const fmt_piece *p)
{
    if (ps->n == ps->cap)
    {
        ps->cap = ps->cap ? ps->cap * 2 : 8;
        ps->v = (fmt_piece *)cb_xrealloc(ps->v, ps->cap * sizeof(fmt_piece));
    }
    ps->v[ps->n++] = *p;
}

/* The literal bytes since `start` as a piece, when there are any. */
static void lit_flush(fmt_pieces *ps, size_t *start)
{
    if (ps->lit_n > *start)
    {
        fmt_piece p;
        memset(&p, 0, sizeof p);
        p.lit_off = *start;
        p.lit_len = ps->lit_n - *start;
        piece_push(ps, &p);
        *start = ps->lit_n;
    }
}

void fmt_pieces_free(fmt_pieces *ps)
{
    free(ps->v);
    free(ps->lit);
    memset(ps, 0, sizeof *ps);
}

/* What a specifier accepts. */
enum
{
    KIND_INT,
    KIND_FLOAT,
    KIND_TEXT,
    KIND_VALUE
};

static int conv_kind(uint8_t c)
{
    switch (c)
    {
    case 'd':
    case 'i':
    case 'u':
    case 'x':
    case 'X':
    case 'o':
    case 'b':
        return KIND_INT;
    case 'f':
    case 'F':
    case 'e':
    case 'E':
    case 'g':
    case 'G':
        return KIND_FLOAT;
    case 'v':
        return KIND_VALUE;
    default:
        return KIND_TEXT;
    }
}

/* A run of digits at *i: its value, 0 for none; -1 when above LIMIT. */
static int64_t parse_digits(const uint8_t *f, size_t n, size_t *i)
{
    size_t start = *i;
    uint64_t v = 0;
    int big = 0;
    while (*i < n && f[*i] >= '0' && f[*i] <= '9')
    {
        if (v <= LIMIT)
        {
            v = v * 10 + (uint64_t)(f[*i] - '0');
        }
        else
        {
            big = 1;
        }
        (*i)++;
    }
    if (start == *i)
    {
        return 0;
    }
    if (big || v > LIMIT)
    {
        return -1;
    }
    return (int64_t)v;
}

/* The pieces of `f`, or -1 when it is not a format: an unknown
   conversion, a `%` at the end, a flag the conversion does not take. */
int fmt_parse(const uint8_t *f, size_t n, fmt_pieces *out)
{
    fmt_pieces *ps = out;
    ps->n = 0;
    ps->lit_n = 0;
    size_t start = 0;
    size_t i = 0;
    while (i < n)
    {
        if (f[i] != '%')
        {
            lit_put(ps, f[i]);
            i++;
            continue;
        }
        i++;
        if (i < n && f[i] == '%')
        {
            lit_put(ps, '%');
            i++;
            continue;
        }
        fmt_piece s;
        memset(&s, 0, sizeof s);
        s.is_spec = 1;
        while (i < n)
        {
            uint8_t c = f[i];
            if (c == '-')
            {
                s.left = 1;
            }
            else if (c == '0')
            {
                s.zero = 1;
            }
            else if (c == '+')
            {
                s.plus = 1;
            }
            else if (c == ' ')
            {
                s.space = 1;
            }
            else if (c == '#')
            {
                s.alt = 1;
            }
            else
            {
                break;
            }
            i++;
        }
        if (i < n && f[i] == '*')
        {
            s.width_star = 1;
            i++;
        }
        else
        {
            int64_t w = parse_digits(f, n, &i);
            if (w < 0)
            {
                goto fail;
            }
            s.width = (size_t)w;
        }
        if (i < n && f[i] == '.')
        {
            i++;
            if (i < n && f[i] == '*')
            {
                s.prec_star = 1;
                s.has_prec = 1;
                s.prec = 0;
                i++;
            }
            else
            {
                int64_t p = parse_digits(f, n, &i);
                if (p < 0)
                {
                    goto fail;
                }
                s.has_prec = 1;
                s.prec = (size_t)p;
            }
        }
        if (i >= n)
        {
            goto fail;
        }
        uint8_t conv = f[i];
        i++;
        if (conv == '*' || !memchr("diuxXobfFeEgGsv", conv, 15))
        {
            goto fail;
        }
        s.conv = conv;
        int ok;
        switch (conv_kind(conv))
        {
        case KIND_INT:
            ok = (conv == 'd' || conv == 'i' || conv == 'u') ? !s.alt : (!s.plus && !s.space);
            break;
        case KIND_FLOAT:
            ok = 1;
            break;
        case KIND_TEXT:
            ok = !s.zero && !s.plus && !s.space && !s.alt;
            break;
        default:
            ok = !s.zero && !s.plus && !s.space && !s.alt && !s.has_prec;
            break;
        }
        if (!ok)
        {
            goto fail;
        }
        lit_flush(ps, &start);
        piece_push(ps, &s);
    }
    lit_flush(ps, &start);
    return 0;
fail:
    ps->n = 0;
    ps->lit_n = 0;
    return -1;
}

/* ---- numbers as text ---- */

/* The decimal digits of v, written backwards ending at `end`; returns
   their count. */
static size_t u128_dec(cb_u128 v, char *end)
{
    char *p = end;
    while ((uint64_t)(v >> 64) != 0)
    {
        const uint64_t e19 = 10000000000000000000ull;
        uint64_t r = (uint64_t)(v % e19);
        v /= e19;
        for (int k = 0; k < 19; k++)
        {
            *--p = (char)('0' + r % 10);
            r /= 10;
        }
    }
    uint64_t x = (uint64_t)v;
    do
    {
        *--p = (char)('0' + x % 10);
        x /= 10;
    } while (x != 0);
    return (size_t)(end - p);
}

size_t fmt_int_text(uint64_t hi, uint64_t lo, int is_signed, char *buf)
{
    cb_u128 bits = ((cb_u128)hi << 64) | lo;
    char tmp[48];
    size_t k = 0;
    if (is_signed && (hi >> 63) != 0)
    {
        buf[k++] = '-';
        bits = ~bits + 1;
    }
    size_t n = u128_dec(bits, tmp + sizeof tmp);
    memcpy(buf + k, tmp + sizeof tmp - n, n);
    return k + n;
}

size_t fmt_format_float(double v, int is_f32, char *buf)
{
    if (v != v)
    {
        memcpy(buf, "NaN", 3);
        return 3;
    }
    if (v == __builtin_inf())
    {
        memcpy(buf, "inf", 3);
        return 3;
    }
    if (v == -__builtin_inf())
    {
        memcpy(buf, "-inf", 4);
        return 4;
    }
    uint64_t bits;
    int neg;
    if (is_f32)
    {
        float f = (float)v;
        uint32_t b;
        memcpy(&b, &f, 4);
        bits = b;
        neg = (b >> 31) != 0;
        bits &= 0x7fffffffu;
        if ((uint32_t)bits == 0x7f800000u)
        {
            /* A finite f64 beyond f32's range: `v as f32` is infinite. */
            return neg ? (memcpy(buf, "-inf", 4), 4) : (memcpy(buf, "inf", 3), 3);
        }
    }
    else
    {
        memcpy(&bits, &v, 8);
        neg = (bits >> 63) != 0;
        bits &= ~((uint64_t)1 << 63);
    }
    char dg[24];
    int n;
    int32_t k;
    if (bits == 0)
    {
        dg[0] = '0';
        n = 1;
        k = 0;
    }
    else
    {
        uint64_t d;
        int32_t e10;
        n = fl_shortest(bits, is_f32, &d, &e10);
        for (int i = n - 1; i >= 0; i--)
        {
            dg[i] = (char)('0' + d % 10);
            d /= 10;
        }
        k = e10 + n - 1;
    }
    size_t o = 0;
    if (neg)
    {
        buf[o++] = '-';
    }
    if (-7 < k && k < 21)
    {
        if (k >= 0)
        {
            int int_len = k + 1;
            if (n <= int_len)
            {
                memcpy(buf + o, dg, (size_t)n);
                o += (size_t)n;
                memset(buf + o, '0', (size_t)(int_len - n));
                o += (size_t)(int_len - n);
                buf[o++] = '.';
                buf[o++] = '0';
            }
            else
            {
                memcpy(buf + o, dg, (size_t)int_len);
                o += (size_t)int_len;
                buf[o++] = '.';
                memcpy(buf + o, dg + int_len, (size_t)(n - int_len));
                o += (size_t)(n - int_len);
            }
        }
        else
        {
            buf[o++] = '0';
            buf[o++] = '.';
            memset(buf + o, '0', (size_t)(-k - 1));
            o += (size_t)(-k - 1);
            memcpy(buf + o, dg, (size_t)n);
            o += (size_t)n;
        }
    }
    else
    {
        buf[o++] = dg[0];
        buf[o++] = '.';
        if (n > 1)
        {
            memcpy(buf + o, dg + 1, (size_t)(n - 1));
            o += (size_t)(n - 1);
        }
        else
        {
            buf[o++] = '0';
        }
        buf[o++] = 'e';
        int32_t kk = k;
        if (kk < 0)
        {
            buf[o++] = '-';
            kk = -kk;
        }
        char t[8];
        int m = 0;
        do
        {
            t[m++] = (char)('0' + kk % 10);
            kk /= 10;
        } while (kk != 0);
        while (m > 0)
        {
            buf[o++] = t[--m];
        }
    }
    return o;
}

/* ---- floats under a precision (`fixed`, `exp`, `general`) ---- */

/* 10^k for k <= 38. */
static cb_u128 pow10_u128(int k)
{
    cb_u128 r = 1;
    while (k-- > 0)
    {
        r *= 10;
    }
    return r;
}

/* `a` (finite, > 0) rounded to `nsig` significant digits, half to even,
   when the 128-bit fast path applies: 1 with the digits as an integer of
   exactly `nsig` digits in *r and the decimal exponent of the first in *x;
   else 0. */
static int sig_round_fast(double a, int64_t nsig, cb_u128 *r, int32_t *x)
{
    if (nsig < 1 || nsig > 37)
    {
        return 0;
    }
    /* floor(log10 a) is x0 or x0 + 1 (or x0 - 1, below). */
    int32_t x0 = (fl_log2(a) * 78913) >> 18;
    int32_t p = (int32_t)nsig - 1 - x0;
    cb_u128 v;
    if (!fl_scaled_round(a, p, &v))
    {
        return 0;
    }
    cb_u128 top = pow10_u128((int)nsig);
    if (v > top)
    {
        /* The exponent is x0 + 1: one digit fewer after the point. */
        x0++;
        p--;
        if (!fl_scaled_round(a, p, &v))
        {
            return 0;
        }
    }
    else if (v < top / 10)
    {
        /* The estimate was one too high (possible below 1, where the
           integer factor 78913/2^18 rounds e x log10(2) up). */
        x0--;
        p++;
        if (!fl_scaled_round(a, p, &v) || v < top / 10 || v > top)
        {
            return 0;
        }
    }
    if (v == top)
    {
        /* Rounded up to the next power of ten. */
        v = top / 10;
        x0++;
    }
    *r = v;
    *x = x0;
    return 1;
}

/* `format!("{:.*}", p, a)` for finite a >= 0, then `#`'s point when p is
   0: `a` with `p` digits after the point, correctly rounded. */
static void float_fixed(cb_buf *o, double a, size_t p, int alt)
{
    cb_u128 r;
    if (p <= 27 && fl_scaled_round(a, (int32_t)p, &r))
    {
        char tmp[48];
        size_t n = u128_dec(r, tmp + sizeof tmp);
        const char *dg = tmp + sizeof tmp - n;
        cb_buf_reserve(o, (n > p ? n : p + 1) + 2);
        if (n <= p)
        {
            o->p[o->n++] = '0';
            if (p > 0)
            {
                o->p[o->n++] = '.';
                memset(o->p + o->n, '0', p - n);
                o->n += p - n;
                memcpy(o->p + o->n, dg, n);
                o->n += n;
            }
        }
        else
        {
            memcpy(o->p + o->n, dg, n - p);
            o->n += n - p;
            if (p > 0)
            {
                o->p[o->n++] = '.';
                memcpy(o->p + o->n, dg + n - p, p);
                o->n += p;
            }
        }
        if (p == 0 && alt)
        {
            o->p[o->n++] = '.';
        }
        return;
    }
    fl_dec d;
    fl_dec_of_double(&d, a);
    if (d.nd > 0)
    {
        fl_dec_round(&d, (int64_t)d.dp + (int64_t)p);
    }
    if (d.nd == 0 || d.dp <= 0)
    {
        cb_buf_byte(o, '0');
    }
    else
    {
        int32_t k = d.dp < d.nd ? d.dp : d.nd;
        cb_buf_reserve(o, (size_t)d.dp);
        for (int32_t i = 0; i < k; i++)
        {
            o->p[o->n++] = (uint8_t)('0' + d.d[i]);
        }
        if (d.dp > d.nd)
        {
            cb_buf_fill(o, '0', (size_t)(d.dp - d.nd));
        }
    }
    if (p > 0)
    {
        cb_buf_reserve(o, p + 1);
        o->p[o->n++] = '.';
        for (size_t i = 0; i < p; i++)
        {
            int64_t idx = (int64_t)d.dp + (int64_t)i;
            o->p[o->n++] = (uint8_t)('0' + ((d.nd > 0 && idx >= 0 && idx < d.nd) ? d.d[idx] : 0));
        }
    }
    else if (alt)
    {
        cb_buf_byte(o, '.');
    }
}

/* The exponent `e` as text: `e`, its sign, at least two digits. */
static void put_exp10(cb_buf *o, int32_t e)
{
    cb_buf_reserve(o, 8);
    o->p[o->n++] = 'e';
    o->p[o->n++] = e < 0 ? '-' : '+';
    int32_t ae = e < 0 ? -e : e;
    char t[8];
    int m = 0;
    do
    {
        t[m++] = (char)('0' + ae % 10);
        ae /= 10;
    } while (ae != 0);
    if (m < 2)
    {
        t[m++] = '0';
    }
    while (m > 0)
    {
        o->p[o->n++] = (uint8_t)t[--m];
    }
}

/* `d.ddde±XX`: `format!("{:.*e}", p, a)`'s digits, `p` after the point
   (and `#`'s point when p is 0), an exponent of at least two digits. */
static void float_exp(cb_buf *o, double a, size_t p, int alt)
{
    cb_u128 r;
    int32_t x;
    if (a != 0.0 && p < 37 && sig_round_fast(a, (int64_t)p + 1, &r, &x))
    {
        char tmp[48];
        size_t n = u128_dec(r, tmp + sizeof tmp);
        const char *dg = tmp + sizeof tmp - n;
        cb_buf_reserve(o, p + 3);
        o->p[o->n++] = (uint8_t)dg[0];
        if (p > 0)
        {
            o->p[o->n++] = '.';
            memcpy(o->p + o->n, dg + 1, p);
            o->n += p;
        }
        else if (alt)
        {
            o->p[o->n++] = '.';
        }
        put_exp10(o, x);
        return;
    }
    fl_dec d;
    fl_dec_of_double(&d, a);
    int32_t e = 0;
    if (d.nd > 0)
    {
        fl_dec_round(&d, (int64_t)p + 1);
        e = d.dp - 1;
    }
    cb_buf_reserve(o, p + 3);
    o->p[o->n++] = (uint8_t)('0' + (d.nd > 0 ? d.d[0] : 0));
    if (p > 0)
    {
        o->p[o->n++] = '.';
        for (size_t i = 1; i <= p; i++)
        {
            o->p[o->n++] = (uint8_t)('0' + ((int64_t)i < d.nd ? d.d[i] : 0));
        }
    }
    else if (alt)
    {
        o->p[o->n++] = '.';
    }
    put_exp10(o, e);
}

/* C's `%g`: `p` significant digits (0 means 1), in `%e` form when the
   exponent is below -4 or at least `p`, else `%f` form; trailing zeros
   (and a trailing point) removed unless `#`. */
static void float_general(cb_buf *o, double a, size_t p, int alt)
{
    if (p < 1)
    {
        p = 1;
    }
    int64_t x = 0;
    if (a != 0.0)
    {
        cb_u128 r;
        int32_t xf;
        if (sig_round_fast(a, (int64_t)p, &r, &xf))
        {
            x = xf;
        }
        else
        {
            fl_dec d;
            fl_dec_of_double(&d, a);
            fl_dec_round(&d, (int64_t)p);
            x = (int64_t)d.dp - 1;
        }
    }
    size_t start = o->n;
    int is_exp = x < -4 || x >= (int64_t)p;
    if (is_exp)
    {
        float_exp(o, a, p - 1, alt);
    }
    else
    {
        float_fixed(o, a, (size_t)((int64_t)p - 1 - x), alt);
    }
    if (alt)
    {
        return;
    }
    /* The mantissa: up to an `e`, if any. */
    size_t mend = o->n;
    for (size_t i = start; i < o->n; i++)
    {
        if (o->p[i] == 'e')
        {
            mend = i;
            break;
        }
    }
    int dot = memchr(o->p + start, '.', mend - start) != NULL;
    if (!dot)
    {
        return;
    }
    size_t cut = mend;
    while (cut > start && o->p[cut - 1] == '0')
    {
        cut--;
    }
    while (cut > start && o->p[cut - 1] == '.')
    {
        cut--;
    }
    if (cut < mend)
    {
        memmove(o->p + cut, o->p + mend, o->n - mend);
        o->n -= mend - cut;
    }
}

/* ---- text: UTF-8 as `String::from_utf8_lossy` reads it ---- */

/* The sequence at p[0..n): its length, and whether it is a character (1)
   or an invalid sequence one U+FFFD replaces (0): the maximal prefix of a
   valid sequence, as Rust's `Utf8Chunks` (and Unicode's recommended
   practice) cut it. */
static size_t utf8_step(const uint8_t *p, size_t n, int *valid)
{
    uint8_t b = p[0];
    if (b < 0x80)
    {
        *valid = 1;
        return 1;
    }
    *valid = 0;
    if (b >= 0xc2 && b <= 0xdf)
    {
        if (n >= 2 && (p[1] & 0xc0) == 0x80)
        {
            *valid = 1;
            return 2;
        }
        return 1;
    }
    if (b >= 0xe0 && b <= 0xef)
    {
        if (n < 2)
        {
            return 1;
        }
        uint8_t c = p[1];
        int ok = (b == 0xe0 && c >= 0xa0 && c <= 0xbf) || (b >= 0xe1 && b <= 0xec && c >= 0x80 && c <= 0xbf) ||
                 (b == 0xed && c >= 0x80 && c <= 0x9f) || (b >= 0xee && b <= 0xef && c >= 0x80 && c <= 0xbf);
        if (!ok)
        {
            return 1;
        }
        if (n < 3 || (p[2] & 0xc0) != 0x80)
        {
            return 2;
        }
        *valid = 1;
        return 3;
    }
    if (b >= 0xf0 && b <= 0xf4)
    {
        if (n < 2)
        {
            return 1;
        }
        uint8_t c = p[1];
        int ok = (b == 0xf0 && c >= 0x90 && c <= 0xbf) || (b >= 0xf1 && b <= 0xf3 && c >= 0x80 && c <= 0xbf) ||
                 (b == 0xf4 && c >= 0x80 && c <= 0x8f);
        if (!ok)
        {
            return 1;
        }
        if (n < 3 || (p[2] & 0xc0) != 0x80)
        {
            return 2;
        }
        if (n < 4 || (p[3] & 0xc0) != 0x80)
        {
            return 3;
        }
        *valid = 1;
        return 4;
    }
    return 1;
}

static int utf8_valid(const uint8_t *p, size_t n)
{
    size_t i = 0;
    while (i < n)
    {
        if (p[i] < 0x80)
        {
            i++;
            continue;
        }
        int v;
        size_t k = utf8_step(p + i, n - i, &v);
        if (!v)
        {
            return 0;
        }
        i += k;
    }
    return 1;
}

/* ---- one specifier ---- */

static __thread cb_buf BODY;

/* This thread's buffer freed (`thread_locals_free`). */
void fmt_thread_end(void)
{
    cb_buf_free(&BODY);
}

/* The commonest specifiers -- an integer under `%v`, `%d`, `%i` or `%u`,
   or valid UTF-8 text, with no flag, width or precision -- written
   straight into `out`: the same bytes the general path makes. */
static int fast(const fmt_piece *s, const fmt_arg *a, cb_buf *out)
{
    if (s->left || s->zero || s->plus || s->space || s->alt || s->width != 0 || s->has_prec)
    {
        return 0;
    }
    if (a->kind == FMT_ARG_INT)
    {
        if (!(s->conv == 'v' || s->conv == 'd' || s->conv == 'i' || s->conv == 'u'))
        {
            return 0;
        }
        uint32_t w = a->width;
        cb_u128 mask = w >= 128 ? ~(cb_u128)0 : (((cb_u128)1 << w) - 1);
        cb_u128 b = a->bits & mask;
        /* Report 279: `%u` prints the bits unsigned, as `int_text` does. */
        int neg = s->conv != 'u' && a->is_signed && w > 0 && ((b >> ((w - 1) & 127)) & 1) == 1;
        cb_u128 mag = neg ? ((~b + 1) & mask) : b;
        char tmp[48];
        size_t n = u128_dec(mag, tmp + sizeof tmp);
        cb_buf_reserve(out, n + 1);
        if (neg)
        {
            out->p[out->n++] = '-';
        }
        memcpy(out->p + out->n, tmp + sizeof tmp - n, n);
        out->n += n;
        return 1;
    }
    if (a->kind == FMT_ARG_TEXT && utf8_valid(a->p, a->n))
    {
        cb_buf_put(out, a->p, a->n);
        return 1;
    }
    return 0;
}

/* An integer's text (`int_text`): its sign and its prefix go to the
   arrays, its digits to `body`. */
static void int_text(const fmt_piece *s, cb_u128 bits, int is_signed, uint32_t width, const char **sign,
                     const char **prefix, cb_buf *body)
{
    cb_u128 mask = width >= 128 ? ~(cb_u128)0 : (((cb_u128)1 << width) - 1);
    bits &= mask;
    int neg = is_signed && width > 0 && ((bits >> ((width - 1) & 127)) & 1) == 1;
    char tmp[136];
    char *end = tmp + sizeof tmp;
    size_t n;
    *sign = "";
    *prefix = "";
    uint8_t c = s->conv;
    if (c == 'u')
    {
        /* Report 126: as C, the value's bits in its own width, unsigned
           (`-1: i32` prints 4294967295), as `%x` prints them. */
        n = u128_dec(bits, end);
    }
    else if (c == 'd' || c == 'i')
    {
        cb_u128 mag = neg ? ((~bits + 1) & mask) : bits;
        *sign = neg ? "-" : s->plus ? "+" : s->space ? " " : "";
        n = u128_dec(mag, end);
    }
    else
    {
        /* The bits of the value in its type's width. */
        const char *pre;
        unsigned shift;
        const char *digs = "0123456789abcdef";
        if (c == 'x')
        {
            shift = 4;
            pre = "0x";
        }
        else if (c == 'X')
        {
            shift = 4;
            pre = "0X";
            digs = "0123456789ABCDEF";
        }
        else if (c == 'o')
        {
            shift = 3;
            pre = "0o";
        }
        else
        {
            shift = 1;
            pre = "0b";
        }
        cb_u128 v = bits;
        cb_u128 m = ((cb_u128)1 << shift) - 1;
        char *p = end;
        do
        {
            *--p = digs[(unsigned)(v & m)];
            v >>= shift;
        } while (v != 0);
        n = (size_t)(end - p);
        /* Report 235: as C, no prefix for 0 (`%#x` of 0 is `0`). */
        if (s->alt && bits != 0)
        {
            *prefix = pre;
        }
    }
    const char *dg = end - n;
    /* A precision is a minimum number of digits; `.0` with 0 prints none
       -- but `%#.0o`, whose `#` asks for a leading 0, prints `0` (C). */
    if (s->has_prec && s->prec == 0 && n == 1 && dg[0] == '0' && !(s->alt && c == 'o'))
    {
        return;
    }
    if (s->has_prec && n < s->prec)
    {
        cb_buf_fill(body, '0', s->prec - n);
    }
    cb_buf_put(body, dg, n);
}

static void one_general(const fmt_piece *s, const fmt_arg *a, cb_buf *out)
{
    const char *sign = "";
    const char *prefix = "";
    cb_buf *body = &BODY;
    body->n = 0;
    size_t body_chars;
    int text = 0;
    if (a->kind == FMT_ARG_INT)
    {
        if (s->conv == 'v')
        {
            /* `%v`: the value as it is. */
            fmt_piece d;
            memset(&d, 0, sizeof d);
            d.conv = 'd';
            int_text(&d, a->bits, a->is_signed, a->width, &sign, &prefix, body);
        }
        else
        {
            int_text(s, a->bits, a->is_signed, a->width, &sign, &prefix, body);
        }
        body_chars = body->n;
    }
    else if (a->kind == FMT_ARG_FLOAT)
    {
        double v = a->f;
        if (s->conv == 'v')
        {
            /* A float in its shortest exact form. */
            char t[FMT_FLOAT_MAX];
            size_t n = fmt_format_float(v, a->is_f32 != 0, t);
            cb_buf_put(body, t, n);
        }
        else
        {
            int upper = s->conv >= 'A' && s->conv <= 'Z';
            uint64_t vb;
            memcpy(&vb, &v, 8);
            int isnan = v != v;
            /* Report 235: a NaN with its sign bit set prints `-nan`, as C. */
            if ((vb >> 63) != 0)
            {
                sign = "-";
            }
            else if (s->plus)
            {
                sign = "+";
            }
            else if (s->space)
            {
                sign = " ";
            }
            double av = __builtin_fabs(v);
            if (isnan)
            {
                cb_buf_str(body, "nan");
            }
            else if (av == __builtin_inf())
            {
                cb_buf_str(body, "inf");
            }
            else
            {
                size_t p = s->has_prec ? s->prec : 6;
                uint8_t lc = (uint8_t)(upper ? s->conv + 32 : s->conv);
                if (lc == 'f')
                {
                    float_fixed(body, av, p, s->alt);
                }
                else if (lc == 'e')
                {
                    float_exp(body, av, p, s->alt);
                }
                else
                {
                    float_general(body, av, p, s->alt);
                }
            }
            if (upper)
            {
                for (size_t i = 0; i < body->n; i++)
                {
                    uint8_t c = body->p[i];
                    if (c >= 'a' && c <= 'z')
                    {
                        body->p[i] = (uint8_t)(c - 32);
                    }
                }
            }
        }
        body_chars = body->n;
    }
    else
    {
        /* Text, lossily decoded; a precision keeps that many characters. */
        text = 1;
        size_t i = 0;
        size_t chars = 0;
        size_t lim = s->has_prec ? s->prec : SIZE_MAX;
        while (i < a->n && chars < lim)
        {
            int v;
            size_t k = utf8_step(a->p + i, a->n - i, &v);
            if (v)
            {
                cb_buf_put(body, a->p + i, k);
            }
            else
            {
                cb_buf_put(body, "\xef\xbf\xbd", 3);
            }
            i += k;
            chars++;
        }
        body_chars = chars;
    }
    (void)text;
    /* Width counts characters. `0` pads after the sign and prefix, for a
       finite number with no precision on an integer; otherwise spaces. */
    size_t sl = strlen(sign);
    size_t pl = strlen(prefix);
    size_t len = sl + pl + body_chars;
    size_t pad = s->width > len ? s->width - len : 0;
    int finite = !(a->kind == FMT_ARG_FLOAT && !(__builtin_fabs(a->f) < __builtin_inf()));
    int zero_ok = s->zero && !s->left && finite && !(conv_kind(s->conv) == KIND_INT && s->has_prec);
    cb_buf_reserve(out, pad + sl + pl + body->n);
    if (s->left)
    {
        cb_buf_put(out, sign, sl);
        cb_buf_put(out, prefix, pl);
        cb_buf_put(out, body->p, body->n);
        cb_buf_fill(out, ' ', pad);
    }
    else if (zero_ok)
    {
        cb_buf_put(out, sign, sl);
        cb_buf_put(out, prefix, pl);
        cb_buf_fill(out, '0', pad);
        cb_buf_put(out, body->p, body->n);
    }
    else
    {
        cb_buf_fill(out, ' ', pad);
        cb_buf_put(out, sign, sl);
        cb_buf_put(out, prefix, pl);
        cb_buf_put(out, body->p, body->n);
    }
}

void fmt_render_into(const fmt_pieces *ps, const fmt_arg *args, size_t nargs, cb_buf *out)
{
    size_t k = 0;
    for (size_t pi = 0; pi < ps->n; pi++)
    {
        const fmt_piece *p = &ps->v[pi];
        if (!p->is_spec)
        {
            cb_buf_put(out, ps->lit + p->lit_off, p->lit_len);
            continue;
        }
        fmt_piece s = *p;
        /* A computed width or precision above LIMIT counts as LIMIT. */
        if (s.width_star)
        {
            s.width = (k < nargs && args[k].kind == FMT_ARG_INT) ? (args[k].bits > LIMIT ? LIMIT : (size_t)args[k].bits) : 0;
            k++;
        }
        if (s.prec_star)
        {
            s.prec = (k < nargs && args[k].kind == FMT_ARG_INT) ? (args[k].bits > LIMIT ? LIMIT : (size_t)args[k].bits) : 0;
            s.has_prec = 1;
            k++;
        }
        if (k < nargs)
        {
            if (!fast(&s, &args[k], out))
            {
                one_general(&s, &args[k], out);
            }
        }
        k++;
    }
}

/* ---- parse<T> ---- */

/* The end of the digit run from i. */
static size_t digits_end(const uint8_t *b, size_t n, size_t i)
{
    while (i < n && b[i] >= '0' && b[i] <= '9')
    {
        i++;
    }
    return i;
}

/* An integer of `bits` bits, as its two's-complement pattern in a u128.
   `number(T) := sign? digit+` (no `-` when unsigned). */
int64_t numtext_parse_int(const uint8_t *b, size_t n, int is_signed, uint32_t bits, cb_u128 *out)
{
    if (n == 0)
    {
        return NUMTEXT_EMPTY;
    }
    size_t i = 0;
    int neg = b[0] == '-';
    if (b[0] == '+' || (neg && is_signed))
    {
        i = 1;
    }
    /* One pass: the digits, and their value in a u64 while there are at
       most 19 (10^19 - 1 < 2^64); a longer run takes the checked u128
       path, digit by digit. */
    size_t end = i;
    uint64_t m = 0;
    while (end < n)
    {
        uint8_t d = (uint8_t)(b[end] - '0');
        if (d > 9)
        {
            break;
        }
        m = m * 10 + d;
        end++;
    }
    if (end == i || end < n)
    {
        return (int64_t)end;
    }
    cb_u128 mag = m;
    if (end - i > 19)
    {
        mag = 0;
        const cb_u128 max = ~(cb_u128)0;
        for (size_t j = i; j < end; j++)
        {
            unsigned d = (unsigned)(b[j] - '0');
            if (mag > max / 10)
            {
                return NUMTEXT_OUT_OF_RANGE;
            }
            mag *= 10;
            if (mag > max - d)
            {
                return NUMTEXT_OUT_OF_RANGE;
            }
            mag += d;
        }
    }
    cb_u128 limit;
    if (!is_signed)
    {
        limit = bits == 128 ? ~(cb_u128)0 : (((cb_u128)1 << (bits & 127)) - 1);
    }
    else if (!neg)
    {
        limit = ((cb_u128)1 << ((bits - 1) & 127)) - 1;
    }
    else
    {
        limit = (cb_u128)1 << ((bits - 1) & 127);
    }
    if (mag > limit)
    {
        return NUMTEXT_OUT_OF_RANGE;
    }
    if (out)
    {
        *out = neg ? (~mag + 1) : mag;
    }
    return -1;
}

/* A float, correctly rounded in f32 when `is_f32` (then widened, which is
   exact). `number(T) := sign? (digit+ ('.' digit+)? (('e'|'E') sign?
   digit+)? | 'inf') | 'NaN'`. */
int64_t numtext_parse_float(const uint8_t *b, size_t n, int is_f32, double *out)
{
    if (n == 0)
    {
        return NUMTEXT_EMPTY;
    }
    if (n == 3 && memcmp(b, "NaN", 3) == 0)
    {
        if (out)
        {
            *out = __builtin_nan("");
        }
        return -1;
    }
    size_t i = 0;
    if (b[0] == '+' || b[0] == '-')
    {
        i = 1;
    }
    const uint8_t *rest = b + i;
    size_t rn = n - i;
    if (rn > 0 && rest[0] == 'i')
    {
        /* `inf`: as far as it matches. */
        size_t k = 0;
        while (k < 3 && k < rn && rest[k] == "inf"[k])
        {
            k++;
        }
        if (k == 3 && rn == 3)
        {
            if (out)
            {
                *out = b[0] == '-' ? -__builtin_inf() : __builtin_inf();
            }
            return -1;
        }
        return (int64_t)(i + k);
    }
    if (i == 0 && rn > 0 && rest[0] == 'N')
    {
        /* `NaN`, but not all of it (the whole text was checked above). */
        size_t k = 0;
        while (k < 3 && k < rn && rest[k] == "NaN"[k])
        {
            k++;
        }
        return (int64_t)k;
    }
    size_t j = digits_end(b, n, i);
    if (j == i)
    {
        return (int64_t)j;
    }
    if (j < n && b[j] == '.')
    {
        size_t k = digits_end(b, n, j + 1);
        if (k == j + 1)
        {
            return (int64_t)k;
        }
        j = k;
    }
    if (j < n && (b[j] == 'e' || b[j] == 'E'))
    {
        size_t k = j + 1;
        if (k < n && (b[k] == '+' || b[k] == '-'))
        {
            k++;
        }
        size_t e = digits_end(b, n, k);
        if (e == k)
        {
            return (int64_t)e;
        }
        j = e;
    }
    if (j < n)
    {
        return (int64_t)j;
    }
    double v = fl_parse(b, n, is_f32);
    if (__builtin_fabs(v) == __builtin_inf())
    {
        return NUMTEXT_OUT_OF_RANGE;
    }
    if (out)
    {
        *out = v;
    }
    return -1;
}

/* D-0181: `bool` text: exactly `true` or `false`; otherwise the length of
   the longest prefix of either. */
int64_t numtext_parse_bool(const uint8_t *b, size_t n, uint8_t *out)
{
    if (n == 0)
    {
        return NUMTEXT_EMPTY;
    }
    if (n == 4 && memcmp(b, "true", 4) == 0)
    {
        if (out)
        {
            *out = 1;
        }
        return -1;
    }
    if (n == 5 && memcmp(b, "false", 5) == 0)
    {
        if (out)
        {
            *out = 0;
        }
        return -1;
    }
    size_t kt = 0;
    while (kt < 4 && kt < n && b[kt] == "true"[kt])
    {
        kt++;
    }
    size_t kf = 0;
    while (kf < 5 && kf < n && b[kf] == "false"[kf])
    {
        kf++;
    }
    return (int64_t)(kt > kf ? kt : kf);
}
