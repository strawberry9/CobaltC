/* Exact float <-> decimal conversions (fltconv.h): what fmt.c needs of
   Rust's `core::num::flt2dec` and `dec2flt`. */

#include "fltconv.h"

#include <string.h>

typedef unsigned __int128 u128;

#include "fltconv_tables.h"

static inline uint64_t f64_bits(double v)
{
    uint64_t b;
    memcpy(&b, &v, 8);
    return b;
}

static inline double f64_of(uint64_t b)
{
    double v;
    memcpy(&v, &b, 8);
    return v;
}

static inline float f32_of(uint32_t b)
{
    float v;
    memcpy(&v, &b, 4);
    return v;
}

/* ---- the exact decimal (fl_dec) ----

   The decimal-shifting arithmetic of Go's `strconv/decimal.go` (and of
   Rust's `dec2flt::decimal_seq`): a number is a digit string and a point,
   multiplied or divided by 2^k one shift of at most 60 bits at a time,
   exactly while the digits fit. */

#define DEC_MAX_SHIFT 60

static void dec_trim(fl_dec *a)
{
    while (a->nd > 0 && a->d[a->nd - 1] == 0)
    {
        a->nd--;
    }
    if (a->nd == 0)
    {
        a->dp = 0;
    }
}

static void dec_assign(fl_dec *a, uint64_t v)
{
    uint8_t buf[24];
    int n = 0;
    while (v > 0)
    {
        uint64_t q = v / 10;
        buf[n++] = (uint8_t)(v - 10 * q);
        v = q;
    }
    a->nd = 0;
    a->trunc = 0;
    for (int i = n - 1; i >= 0; i--)
    {
        a->d[a->nd++] = buf[i];
    }
    a->dp = a->nd;
    dec_trim(a);
}

/* a * 2^k, 0 < k <= DEC_MAX_SHIFT. */
static void dec_lshift(fl_dec *a, unsigned k)
{
    uint8_t tmp[FL_DEC_CAP + 24];
    int w = (int)sizeof tmp;
    uint64_t n = 0;
    for (int r = a->nd - 1; r >= 0; r--)
    {
        n += (uint64_t)a->d[r] << k;
        uint64_t q = n / 10;
        tmp[--w] = (uint8_t)(n - 10 * q);
        n = q;
    }
    while (n > 0)
    {
        uint64_t q = n / 10;
        tmp[--w] = (uint8_t)(n - 10 * q);
        n = q;
    }
    int len = (int)sizeof tmp - w;
    int delta = len - a->nd;
    int keep = len < FL_DEC_CAP ? len : FL_DEC_CAP;
    for (int i = keep; i < len; i++)
    {
        if (tmp[w + i] != 0)
        {
            a->trunc = 1;
        }
    }
    memcpy(a->d, tmp + w, (size_t)keep);
    a->nd = keep;
    a->dp += delta;
    dec_trim(a);
}

/* a / 2^k, 0 < k <= DEC_MAX_SHIFT. */
static void dec_rshift(fl_dec *a, unsigned k)
{
    int r = 0;
    int w = 0;
    uint64_t n = 0;
    for (; (n >> k) == 0; r++)
    {
        if (r >= a->nd)
        {
            if (n == 0)
            {
                a->nd = 0;
                a->dp = 0;
                return;
            }
            while ((n >> k) == 0)
            {
                n = n * 10;
                r++;
            }
            break;
        }
        n = n * 10 + a->d[r];
    }
    a->dp -= r - 1;
    uint64_t mask = ((uint64_t)1 << k) - 1;
    for (; r < a->nd; r++)
    {
        uint64_t c = a->d[r];
        a->d[w++] = (uint8_t)(n >> k);
        n &= mask;
        n = n * 10 + c;
    }
    while (n > 0)
    {
        uint64_t dig = n >> k;
        n &= mask;
        if (w < FL_DEC_CAP)
        {
            a->d[w++] = (uint8_t)dig;
        }
        else if (dig > 0)
        {
            a->trunc = 1;
        }
        n = n * 10;
    }
    a->nd = w;
    dec_trim(a);
}

static void dec_shift(fl_dec *a, int64_t k)
{
    if (a->nd == 0)
    {
        return;
    }
    if (k > 0)
    {
        while (k > DEC_MAX_SHIFT)
        {
            dec_lshift(a, DEC_MAX_SHIFT);
            k -= DEC_MAX_SHIFT;
        }
        if (k > 0)
        {
            dec_lshift(a, (unsigned)k);
        }
    }
    else if (k < 0)
    {
        while (k < -DEC_MAX_SHIFT)
        {
            dec_rshift(a, DEC_MAX_SHIFT);
            k += DEC_MAX_SHIFT;
        }
        if (k < 0)
        {
            dec_rshift(a, (unsigned)-k);
        }
    }
}

/* Whether rounding to `nd` digits (0 <= nd < a->nd) goes up: past half,
   or exactly half and the kept digit odd (half to even); a truncated tail
   is above half. */
static int dec_round_up(const fl_dec *a, int32_t nd)
{
    if (a->d[nd] == 5 && nd + 1 == a->nd)
    {
        if (a->trunc)
        {
            return 1;
        }
        return nd > 0 && (a->d[nd - 1] & 1) == 1;
    }
    return a->d[nd] >= 5;
}

void fl_dec_round(fl_dec *a, int64_t nd)
{
    if (nd >= a->nd)
    {
        return;
    }
    if (nd < 0)
    {
        /* Below half a unit of the place rounded at: zero. */
        a->nd = 0;
        a->dp = 0;
        return;
    }
    int32_t k = (int32_t)nd;
    if (dec_round_up(a, k))
    {
        for (int32_t i = k - 1; i >= 0; i--)
        {
            if (a->d[i] < 9)
            {
                a->d[i]++;
                a->nd = i + 1;
                return;
            }
        }
        /* All nines (or rounding at the first digit): 1 at the next place. */
        a->d[0] = 1;
        a->nd = 1;
        a->dp++;
    }
    else
    {
        a->nd = k;
        dec_trim(a);
    }
}

/* The integer part of a, rounded half to even (a < 2^64). */
static uint64_t dec_rounded_integer(const fl_dec *a)
{
    if (a->dp > 20)
    {
        return UINT64_MAX;
    }
    int32_t i = 0;
    uint64_t n = 0;
    for (; i < a->dp && i < a->nd; i++)
    {
        n = n * 10 + a->d[i];
    }
    for (; i < a->dp; i++)
    {
        n *= 10;
    }
    if (a->dp >= 0 && a->dp < a->nd && dec_round_up(a, a->dp))
    {
        n++;
    }
    return n;
}

static void split_double(double v, uint64_t *m, int32_t *e2);

void fl_dec_of_double(fl_dec *a, double v)
{
    uint64_t m;
    int32_t e2;
    split_double(v, &m, &e2);
    dec_assign(a, m);
    dec_shift(a, e2);
}

/* ---- a value scaled by a power of ten, rounded (the fast path) ---- */

static const uint64_t P5_U64[28] = {1ull, 5ull, 25ull, 125ull, 625ull, 3125ull, 15625ull, 78125ull,
                                    390625ull, 1953125ull, 9765625ull, 48828125ull, 244140625ull,
                                    1220703125ull, 6103515625ull, 30517578125ull, 152587890625ull,
                                    762939453125ull, 3814697265625ull, 19073486328125ull,
                                    95367431640625ull, 476837158203125ull, 2384185791015625ull,
                                    11920928955078125ull, 59604644775390625ull, 298023223876953125ull,
                                    1490116119384765625ull, 7450580596923828125ull};

static void split_double(double v, uint64_t *m, int32_t *e2)
{
    uint64_t b = f64_bits(v);
    uint64_t frac = b & (((uint64_t)1 << 52) - 1);
    int32_t ex = (int32_t)((b >> 52) & 0x7ff);
    if (ex == 0)
    {
        *m = frac;
        *e2 = 1 - 1075;
    }
    else
    {
        *m = frac | ((uint64_t)1 << 52);
        *e2 = ex - 1075;
    }
}

int32_t fl_log2(double v)
{
    uint64_t m;
    int32_t e2;
    split_double(v, &m, &e2);
    return e2 + 63 - __builtin_clzll(m);
}

int fl_scaled_round(double v, int32_t p, u128 *r)
{
    if (p < 0 || p > 27)
    {
        return 0;
    }
    uint64_t m;
    int32_t e2;
    split_double(v, &m, &e2);
    if (m == 0)
    {
        *r = 0;
        return 1;
    }
    /* v x 10^p = m x 5^p x 2^(e2 + p), m x 5^p < 2^53 x 2^63. */
    u128 n = (u128)m * P5_U64[p];
    int32_t s = e2 + p;
    if (s >= 0)
    {
        uint64_t hi = (uint64_t)(n >> 64);
        int bl = hi ? 128 - __builtin_clzll(hi) : 64 - __builtin_clzll((uint64_t)n);
        if (s > 127 - bl)
        {
            return 0;
        }
        *r = n << s;
        return 1;
    }
    int32_t t = -s;
    if (t >= 120)
    {
        /* n < 2^116: below one half. */
        *r = 0;
        return 1;
    }
    u128 q = n >> t;
    u128 rem = n - (q << t);
    u128 half = (u128)1 << (t - 1);
    if (rem > half || (rem == half && (q & 1) != 0))
    {
        q++;
    }
    *r = q;
    return 1;
}

/* ---- shortest digits (Ryu) ---- */

static inline uint32_t pow5bits(int32_t e)
{
    return (uint32_t)(((e * 1217359) >> 19) + 1);
}

static inline uint32_t log10pow2(int32_t e)
{
    return (uint32_t)((e * 78913) >> 18);
}

static inline uint32_t log10pow5(int32_t e)
{
    return (uint32_t)((e * 732923) >> 20);
}

static inline uint32_t pow5factor(uint64_t v)
{
    uint32_t c = 0;
    for (;;)
    {
        uint64_t q = v / 5;
        if (v - 5 * q != 0)
        {
            break;
        }
        v = q;
        c++;
    }
    return c;
}

static inline int mult_pow5(uint64_t v, uint32_t p)
{
    return pow5factor(v) >= p;
}

static inline int mult_pow2(uint64_t v, uint32_t p)
{
    return (v & (((uint64_t)1 << p) - 1)) == 0;
}

static inline uint64_t mulshift64(uint64_t m, const uint64_t *mul, int32_t j)
{
    u128 b0 = (u128)m * mul[0];
    u128 b2 = (u128)m * mul[1];
    return (uint64_t)(((b0 >> 64) + b2) >> (j - 64));
}

int fl_shortest(uint64_t bits, int is_f32, uint64_t *digits, int32_t *exp10)
{
    uint64_t ieee_m;
    uint32_t ieee_e;
    int32_t e2;
    uint64_t m2;
    if (is_f32)
    {
        ieee_m = bits & ((1u << 23) - 1);
        ieee_e = (uint32_t)((bits >> 23) & 0xff);
        if (ieee_e == 0)
        {
            e2 = 1 - 127 - 23 - 2;
            m2 = ieee_m;
        }
        else
        {
            e2 = (int32_t)ieee_e - 127 - 23 - 2;
            m2 = ((uint64_t)1 << 23) | ieee_m;
        }
    }
    else
    {
        ieee_m = bits & (((uint64_t)1 << 52) - 1);
        ieee_e = (uint32_t)((bits >> 52) & 0x7ff);
        if (ieee_e == 0)
        {
            e2 = 1 - 1023 - 52 - 2;
            m2 = ieee_m;
        }
        else
        {
            e2 = (int32_t)ieee_e - 1023 - 52 - 2;
            m2 = ((uint64_t)1 << 52) | ieee_m;
        }
    }
    int even = (m2 & 1) == 0;
    int accept = even;
    uint64_t mv = 4 * m2;
    uint32_t mmshift = ieee_m != 0 || ieee_e <= 1;
    uint64_t vr, vp, vm;
    int32_t e10;
    int vm_tz = 0;
    int vr_tz = 0;
    if (e2 >= 0)
    {
        uint32_t q = log10pow2(e2) - (e2 > 3);
        e10 = (int32_t)q;
        int32_t k = RYU_POW5_INV_BITCOUNT + (int32_t)pow5bits((int32_t)q) - 1;
        int32_t i = -e2 + (int32_t)q + k;
        vr = mulshift64(4 * m2, RYU_POW5_INV[q], i);
        vp = mulshift64(4 * m2 + 2, RYU_POW5_INV[q], i);
        vm = mulshift64(4 * m2 - 1 - mmshift, RYU_POW5_INV[q], i);
        if (q <= 21)
        {
            uint32_t mv_mod5 = (uint32_t)(mv % 5);
            if (mv_mod5 == 0)
            {
                vr_tz = mult_pow5(mv, q);
            }
            else if (accept)
            {
                vm_tz = mult_pow5(mv - 1 - mmshift, q);
            }
            else
            {
                vp -= (uint64_t)mult_pow5(mv + 2, q);
            }
        }
    }
    else
    {
        uint32_t q = log10pow5(-e2) - (-e2 > 1);
        e10 = (int32_t)q + e2;
        int32_t i = -e2 - (int32_t)q;
        int32_t k = (int32_t)pow5bits(i) - RYU_POW5_BITCOUNT;
        int32_t j = (int32_t)q - k;
        vr = mulshift64(4 * m2, RYU_POW5[i], j);
        vp = mulshift64(4 * m2 + 2, RYU_POW5[i], j);
        vm = mulshift64(4 * m2 - 1 - mmshift, RYU_POW5[i], j);
        if (q <= 1)
        {
            vr_tz = 1;
            if (accept)
            {
                vm_tz = mmshift == 1;
            }
            else
            {
                --vp;
            }
        }
        else if (q < 63)
        {
            vr_tz = mult_pow2(mv, q);
        }
    }
    int32_t removed = 0;
    uint8_t last = 0;
    uint64_t out;
    if (vm_tz || vr_tz)
    {
        for (;;)
        {
            uint64_t vp10 = vp / 10;
            uint64_t vm10 = vm / 10;
            if (vp10 <= vm10)
            {
                break;
            }
            uint32_t vm_mod = (uint32_t)(vm - 10 * vm10);
            uint64_t vr10 = vr / 10;
            uint32_t vr_mod = (uint32_t)(vr - 10 * vr10);
            vm_tz &= vm_mod == 0;
            vr_tz &= last == 0;
            last = (uint8_t)vr_mod;
            vr = vr10;
            vp = vp10;
            vm = vm10;
            removed++;
        }
        if (vm_tz)
        {
            for (;;)
            {
                uint64_t vm10 = vm / 10;
                uint32_t vm_mod = (uint32_t)(vm - 10 * vm10);
                if (vm_mod != 0)
                {
                    break;
                }
                uint64_t vp10 = vp / 10;
                uint64_t vr10 = vr / 10;
                uint32_t vr_mod = (uint32_t)(vr - 10 * vr10);
                vr_tz &= last == 0;
                last = (uint8_t)vr_mod;
                vr = vr10;
                vp = vp10;
                vm = vm10;
                removed++;
            }
        }
        if (vr_tz && last == 5 && vr % 2 == 0)
        {
            /* Exactly half: round to even. */
            last = 4;
        }
        out = vr + (uint64_t)((vr == vm && (!accept || !vm_tz)) || last >= 5);
    }
    else
    {
        int up = 0;
        uint64_t vp100 = vp / 100;
        uint64_t vm100 = vm / 100;
        if (vp100 > vm100)
        {
            uint64_t vr100 = vr / 100;
            uint32_t vr_mod = (uint32_t)(vr - 100 * vr100);
            up = vr_mod >= 50;
            vr = vr100;
            vp = vp100;
            vm = vm100;
            removed += 2;
        }
        for (;;)
        {
            uint64_t vp10 = vp / 10;
            uint64_t vm10 = vm / 10;
            if (vp10 <= vm10)
            {
                break;
            }
            uint64_t vr10 = vr / 10;
            uint32_t vr_mod = (uint32_t)(vr - 10 * vr10);
            up = vr_mod >= 5;
            vr = vr10;
            vp = vp10;
            vm = vm10;
            removed++;
        }
        out = vr + (uint64_t)(vr == vm || up);
    }
    int n = 1;
    for (uint64_t t = out; t >= 10; t /= 10)
    {
        n++;
    }
    *digits = out;
    *exp10 = e10 + removed;
    return n;
}

/* ---- text to float ---- */

/* The float's parameters: explicit significand bits, the biased exponent
   of infinity, the decimal exponents beyond which the value is zero or
   infinite (Eisel-Lemire's), and where ties can occur in its product. */
typedef struct
{
    int sig_bits;
    int inf_power;
    int exp_min; /* the least normal exponent, unbiased */
    int64_t smallest_p10;
    int64_t largest_p10;
    int64_t min_rte;
    int64_t max_rte;
    int bias;
    int exp_bits;
} fl_kind;

static const fl_kind K64 = {52, 0x7ff, -1022, -342, 308, -4, 23, -1023, 11};
static const fl_kind K32 = {23, 0xff, -126, -65, 38, -17, 10, -127, 8};

/* A biased float: significand without the hidden bit, biased exponent;
   e < 0 when undecided. */
typedef struct
{
    uint64_t m;
    int32_t e;
} fl_biased;

static inline int32_t lemire_pow2(int32_t q)
{
    /* (q * (152_170 + 65536)) >> 16, an arithmetic shift, plus 63. */
    return ((q * (152170 + 65536)) >> 16) + 63;
}

static fl_biased lemire(const fl_kind *k, int64_t q, uint64_t w)
{
    fl_biased zero = {0, 0};
    fl_biased inf = {0, k->inf_power};
    fl_biased err = {0, -1};
    if (w == 0 || q < k->smallest_p10)
    {
        return zero;
    }
    if (q > k->largest_p10)
    {
        return inf;
    }
    int lz = __builtin_clzll(w);
    w <<= lz;
    int precision = k->sig_bits + 3;
    uint64_t mask = UINT64_MAX >> precision;
    const uint64_t *p5 = LEMIRE_POW5[q + 342];
    /* p5[1] is the high half. */
    u128 first = (u128)w * p5[1];
    uint64_t lo = (uint64_t)first;
    uint64_t hi = (uint64_t)(first >> 64);
    if ((hi & mask) == mask)
    {
        u128 second = (u128)w * p5[0];
        uint64_t second_hi = (uint64_t)(second >> 64);
        lo += second_hi;
        if (second_hi > lo)
        {
            hi++;
        }
    }
    if (lo == UINT64_MAX)
    {
        if (!(q >= -27 && q <= 55))
        {
            return err;
        }
    }
    int upperbit = (int)(hi >> 63);
    int sh = upperbit + 64 - k->sig_bits - 3;
    uint64_t mant = hi >> sh;
    int32_t power2 = lemire_pow2((int32_t)q) + upperbit - lz - k->exp_min + 1;
    if (power2 <= 0)
    {
        if (-power2 + 1 >= 64)
        {
            return zero;
        }
        mant >>= -power2 + 1;
        mant += mant & 1;
        mant >>= 1;
        fl_biased r = {mant, mant >= ((uint64_t)1 << k->sig_bits)};
        return r;
    }
    if (lo <= 1 && q >= k->min_rte && q <= k->max_rte && (mant & 3) == 1 && (mant << sh) == hi)
    {
        mant &= ~(uint64_t)1;
    }
    mant += mant & 1;
    mant >>= 1;
    if (mant >= ((uint64_t)2 << k->sig_bits))
    {
        mant = (uint64_t)1 << k->sig_bits;
        power2++;
    }
    mant &= ~((uint64_t)1 << k->sig_bits);
    if (power2 >= k->inf_power)
    {
        return inf;
    }
    fl_biased r = {mant, power2};
    return r;
}

/* The exact decimal's float (Go's `decimal.floatBits`, Rust's
   `dec2flt::slow::parse_long_mantissa`): scaled by powers of two into
   [1/2, 1), then the significand's bits taken, rounded half to even. */
static fl_biased dec_to_float(const fl_kind *k, fl_dec *d)
{
    static const uint8_t powtab[] = {1, 3, 6, 9, 13, 16, 19, 23, 26};
    fl_biased zero = {0, 0};
    fl_biased inf = {0, k->inf_power};
    if (d->nd == 0)
    {
        return zero;
    }
    if (d->dp > 310)
    {
        return inf;
    }
    if (d->dp < -330)
    {
        return zero;
    }
    int64_t ex = 0;
    while (d->dp > 0)
    {
        int n = d->dp >= 9 ? 27 : powtab[d->dp];
        dec_shift(d, -n);
        ex += n;
    }
    while (d->dp < 0 || (d->dp == 0 && d->d[0] < 5))
    {
        int n = -d->dp >= 9 ? 27 : powtab[-d->dp];
        dec_shift(d, n);
        ex -= n;
    }
    ex--;
    if (ex < k->bias + 1)
    {
        int64_t n = k->bias + 1 - ex;
        dec_shift(d, -n);
        ex += n;
    }
    if (ex - k->bias >= (1 << k->exp_bits) - 1)
    {
        return inf;
    }
    dec_shift(d, 1 + k->sig_bits);
    uint64_t mant = dec_rounded_integer(d);
    if (mant == ((uint64_t)2 << k->sig_bits))
    {
        mant >>= 1;
        ex++;
        if (ex - k->bias >= (1 << k->exp_bits) - 1)
        {
            return inf;
        }
    }
    if ((mant & ((uint64_t)1 << k->sig_bits)) == 0)
    {
        ex = k->bias;
    }
    fl_biased r = {mant & (((uint64_t)1 << k->sig_bits) - 1), (int32_t)(ex - k->bias)};
    return r;
}

static const double P10_64[23] = {1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11,
                                  1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22};
static const float P10_32[11] = {1e0f, 1e1f, 1e2f, 1e3f, 1e4f, 1e5f, 1e6f, 1e7f, 1e8f, 1e9f, 1e10f};
static const uint64_t P10_U64[20] = {1ull, 10ull, 100ull, 1000ull, 10000ull, 100000ull, 1000000ull,
                                     10000000ull, 100000000ull, 1000000000ull, 10000000000ull,
                                     100000000000ull, 1000000000000ull, 10000000000000ull,
                                     100000000000000ull, 1000000000000000ull, 10000000000000000ull,
                                     100000000000000000ull, 1000000000000000000ull,
                                     10000000000000000000ull};

double fl_parse(const uint8_t *s, size_t n, int is_f32)
{
    const fl_kind *k = is_f32 ? &K32 : &K64;
    size_t i = 0;
    int neg = 0;
    if (i < n && (s[i] == '+' || s[i] == '-'))
    {
        neg = s[i] == '-';
        i++;
    }
    /* The significant digits: the first 19 in w, how many in all (ns, from
       the first nonzero digit, trailing zeros not counted), and the
       decimal point's place relative to them. */
    uint64_t w = 0;
    int64_t ns = 0;        /* significant digits up to the last nonzero one */
    int64_t seen = 0;      /* significant digits so far, zeros included */
    int64_t int_sig = 0;   /* significant digits before the point */
    int many = 0;
    size_t first_sig = 0;
    int started = 0;
    size_t int_end;
    for (; i < n && s[i] >= '0' && s[i] <= '9'; i++)
    {
        if (!started && s[i] == '0')
        {
            continue;
        }
        if (!started)
        {
            started = 1;
            first_sig = i;
        }
        seen++;
        if (s[i] != '0')
        {
            ns = seen;
        }
    }
    int_end = i;
    int_sig = seen;
    int64_t lead_frac_zeros = 0;
    if (i < n && s[i] == '.')
    {
        i++;
        for (; i < n && s[i] >= '0' && s[i] <= '9'; i++)
        {
            if (!started && s[i] == '0')
            {
                lead_frac_zeros++;
                continue;
            }
            if (!started)
            {
                started = 1;
                first_sig = i;
            }
            seen++;
            if (s[i] != '0')
            {
                ns = seen;
            }
        }
    }
    size_t digits_end = i;
    int64_t e = 0;
    if (i < n && (s[i] == 'e' || s[i] == 'E'))
    {
        i++;
        int eneg = 0;
        if (i < n && (s[i] == '+' || s[i] == '-'))
        {
            eneg = s[i] == '-';
            i++;
        }
        for (; i < n; i++)
        {
            if (e < 0x10000000)
            {
                e = e * 10 + (s[i] - '0');
            }
        }
        if (eneg)
        {
            e = -e;
        }
    }
    double r;
    if (ns == 0)
    {
        r = 0.0;
        return neg ? -r : r;
    }
    /* The value is 0.(the ns digits) x 10^dp. */
    int64_t dp = (started && first_sig < int_end ? int_sig : -lead_frac_zeros) + e;
    /* The first min(ns, 19) significant digits. */
    int64_t take = ns < 19 ? ns : 19;
    many = ns > 19;
    {
        int64_t got = 0;
        for (size_t j = first_sig; j < digits_end && got < take; j++)
        {
            if (s[j] == '.')
            {
                continue;
            }
            w = w * 10 + (uint64_t)(s[j] - '0');
            got++;
        }
    }
    int64_t q = dp - take;
    /* Clinger's fast path: w and 10^|q| both exact, one rounding. */
    if (!many)
    {
        if (is_f32)
        {
            if (w <= ((uint64_t)1 << 24) && q >= -10 && q <= 10 + 7)
            {
                uint64_t ww = w;
                int64_t qq = q;
                int ok = 1;
                if (qq > 10)
                {
                    uint64_t p = P10_U64[qq - 10];
                    ok = w <= (((uint64_t)1 << 24) / p);
                    ww = w * p;
                    qq = 10;
                }
                if (ok)
                {
                    float f = (float)ww;
                    f = qq >= 0 ? f * P10_32[qq] : f / P10_32[-qq];
                    r = (double)f;
                    return neg ? -r : r;
                }
            }
        }
        else
        {
            if (w <= ((uint64_t)1 << 53) && q >= -22 && q <= 22 + 15)
            {
                uint64_t ww = w;
                int64_t qq = q;
                int ok = 1;
                if (qq > 22)
                {
                    /* "Disguised": move the excess powers into w. */
                    uint64_t p = P10_U64[qq - 22];
                    ok = w <= (((uint64_t)1 << 53) / p);
                    ww = w * p;
                    qq = 22;
                }
                if (ok)
                {
                    double f = (double)ww;
                    f = qq >= 0 ? f * P10_64[qq] : f / P10_64[-qq];
                    return neg ? -f : f;
                }
            }
        }
    }
    fl_biased fp = lemire(k, q, w);
    if (many && fp.e >= 0)
    {
        fl_biased f2 = lemire(k, q, w + 1);
        if (f2.e != fp.e || f2.m != fp.m)
        {
            fp.e = -1;
        }
    }
    if (fp.e < 0)
    {
        static __thread fl_dec d;
        d.nd = 0;
        d.trunc = 0;
        for (size_t j = first_sig; j < digits_end; j++)
        {
            if (s[j] == '.')
            {
                continue;
            }
            if (d.nd < FL_DEC_CAP)
            {
                d.d[d.nd++] = (uint8_t)(s[j] - '0');
            }
            else if (s[j] != '0')
            {
                d.trunc = 1;
            }
        }
        if (dp > 100000)
        {
            dp = 100000;
        }
        if (dp < -100000)
        {
            dp = -100000;
        }
        d.dp = (int32_t)dp;
        dec_trim(&d);
        fp = dec_to_float(k, &d);
    }
    if (is_f32)
    {
        uint32_t b = (uint32_t)fp.m | ((uint32_t)fp.e << 23);
        r = (double)f32_of(b);
    }
    else
    {
        r = f64_of(fp.m | ((uint64_t)fp.e << 52));
    }
    return neg ? -r : r;
}
