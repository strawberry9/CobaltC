/* Exact conversions between binary floats and decimal text, in place of the
   Rust standard library's `flt2dec` and `dec2flt`, for fmt.c's float text
   and `parse<T>` (`{:e}`, `{:.*e}`, `{:.*}`, `str::parse::<f64>` and
   `::<f32>`). The C library's `printf` and `strtod` are not used: Windows
   7's msvcrt rounds neither correctly. Every function here gives the text
   or value Rust gives, byte for byte (tools/floatcheck compares them).

   - The shortest digits that read back (`{:e}`): Ryu.
   - Digits correctly rounded to a precision (`{:.*e}`, `{:.*}`): the
     exact decimal expansion of the binary value (`fl_dec`), rounded half
     to even.
   - Text to a float, correctly rounded: Clinger's fast path, then
     Eisel-Lemire, then the exact decimal (`fl_dec`) when those cannot
     decide, as `core::num::dec2flt` does. */

#ifndef CBRT_FLTCONV_H
#define CBRT_FLTCONV_H

#include <stddef.h>
#include <stdint.h>

/* A decimal number 0.d[0]d[1]...d[nd-1] x 10^dp, digits 0..9 (not ASCII),
   no trailing zeros; nd == 0 is zero. `trunc`: digits beyond the capacity
   were dropped and some of them were not zero (text input only; the exact
   expansion of a double, at most 767 significant digits, always fits). */
#define FL_DEC_CAP 800
typedef struct
{
    int32_t nd;
    int32_t dp;
    int trunc;
    uint8_t d[FL_DEC_CAP];
} fl_dec;

/* The exact decimal value of |v| (finite). */
void fl_dec_of_double(fl_dec *a, double v);
/* Rounds to `nd` significant digits, half to even (`nd` may be 0 or less:
   rounding at a place left of the first digit). */
void fl_dec_round(fl_dec *a, int64_t nd);

/* round(|v| x 10^p), half to even, exactly, for finite v and 0 <= p <=
   27, when the product fits in 128 bits: 1 with the integer in *r, else 0
   (the caller takes the exact decimal). The fast path of `{:.*}` and
   `{:.*e}` at the precisions programs print with. */
int fl_scaled_round(double v, int32_t p, unsigned __int128 *r);
/* floor(log2 |v|) for finite nonzero v. */
int32_t fl_log2(double v);

/* The shortest digits that read back as the finite, nonzero value with
   these bits (an f64's, or when `is_f32` an f32's in the low 32 bits),
   the nearest of them when several: |v| = *digits x 10^*exp10. Returns the
   number of digits. */
int fl_shortest(uint64_t bits, int is_f32, uint64_t *digits, int32_t *exp10);

/* Text in `numtext`'s grammar (sign? digit+ ('.' digit+)? (('e'|'E')
   sign? digit+)?, already checked) to the nearest f64, or the nearest f32
   when `is_f32` (rounded once, in f32, then widened, which is exact). */
double fl_parse(const uint8_t *s, size_t n, int is_f32);

#endif
