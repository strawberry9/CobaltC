# The items both tools build in

These exported items of `std` have no CobaltC body: `coby` and `cobc` each realize them directly, because
they take a variable number of arguments (`printf` and its family, `assert`), end the program (`exit`),
are generic over a function type the surface language cannot yet spell as a body (`Result::map_err`), or
are the platform's floating-point library (`std::math`). Every other exported item carries its
documentation as `///` comments in its own source (D-0229); these carry theirs here, in the same words
`spec/21` uses, and `impl/tools/cobdoc.py --check` fails when an item the tools build in has no entry
below, or an entry names no such item. The tools' lists are `impl/src/modres.rs`'s `native(…)`
registrations and `impl/src/typecheck.rs`'s `FLOAT_FNS` and `FLOAT2_FNS`.

An entry is a heading naming the item and its module, a `Signature:` line, and Markdown text whose first
sentence is the summary. `a1, …, an` stands for any number of further arguments. `T` in `std::math`
stands for `f32` or `f64`, the same for every operand.

## `Result::map_err` in `std::core`
Signature: `fn Result::map_err<T, E1, E2>(Result<T, E1> r, fn(E1) : E2 f) : Result<T, E2>`

Keeps an `Ok` as it is and maps an `Err` through `f`: `match (r) { Ok(v) : Ok(v), Err(e) : Err(f(e)) }`.
It is the step that turns a library's error into a program's own before `?` passes it on. `r` is
consumed; `f` runs only for an `Err`.

## `printf` in `std::io`
Signature: `fn printf(str f, a1, …, an)`

Writes the arguments to standard output as the format `f` says (`spec/21` §2f, `rule.stdlib.format`).
`f` is a string literal, so the format and the arguments are checked when the program is compiled and
nothing about it can fail at run time. The conversions are C's: `%d` for any integer type (there is no
`%ld`), `%u`, `%x`, `%X`, `%o`, `%b` (binary) for an integer's bits in its own width, `%f`, `%e`, `%g` for
`f32` and `f64`, `%s` for a `str`, a `ref<String, shared>` or a `bool`, and `%v` for any printable value
or enum "as CobaltC writes it" (an enum's variant name; a float that reads back as the same value). Width
and precision may be `*`, taking the next argument (a `usize`). A reference to a number, a `bool` or a
`str` prints as what it refers to, so `foreach (x in &v) { printf("%d\n", x); }` prints the elements.
Output is buffered (D-0178) and written out when the buffer fills, at `flush_stdout()`, and when the
program ends.

## `eprintf` in `std::io`
Signature: `fn eprintf(str f, a1, …, an)`

Writes the arguments to standard error as the format `f` says, with `printf`'s conversions. For
messages that are not the program's output: errors, warnings, progress. Standard error is written at
once, with nothing held back, so its text and standard output's interleave in the order the calls ran
(after a `flush_stdout()`, exactly).

## `sprintf` in `std::text`
Signature: `fn sprintf(str f, a1, …, an) : String`

The text `printf` would write for the same arguments, as a new `String` instead of output. The format is
checked when the program is compiled, as `printf`'s is.

## `String::appendf` in `std::text`
Signature: `fn String::appendf(ref<String, exclusive> s, str f, a1, …, an)`

Appends to `*s` the text `printf` would write for the same arguments; `String::append` of `sprintf`'s
result, without the intermediate `String`. The common way to build a line piece by piece before printing
or keeping it once.

## `File::printf` in `std::io`
Signature: `fn File::printf(ref<File, exclusive> f, str format, a1, …, an) : Result<void, FileError>`

Writes the formatted text to the open file `*f`, as `printf` would to standard output (D-0059); the
format is checked when the program is compiled. A write that fails is the `Err`, as for `File::write`.
`File` itself is `std::fs`'s type; this function is registered with the `printf` family in `std::io`,
and `import std;` brings in both.

## `TcpStream::printf` in `std::net`
Signature: `fn TcpStream::printf(ref<TcpStream, exclusive> s, str format, a1, …, an) : Result<void, NetError>`

Writes the formatted text to the connected stream `*s`, as `File::printf` does to a file; the whole text
is written or the `Err` says why (`[Stream-Write]`). A line protocol's reply in one call:
`TcpStream::printf(&mut c, "%d %s\r\n", code, text)?`.

## `assert` in `std::core`
Signature: `fn assert(bool c)`, `fn assert(bool c, str f, a1, …, an)`

Checks `c` and, if it is false, faults with `diag.assert-failed`, reporting the formatted message if one
is given (`spec/18` §1a, `rule.fail.assert`, D-0065). The message's arguments are evaluated only when the
assertion fails. An assertion is part of the program's meaning: there is no build in which it is left
out, and `--check` sees its format as it sees `printf`'s. The fault unwinds like any other, so
destructors run and a supervised task reports it to its parent.

## `exit` in `std::process`
Signature: `fn exit(u8 status) : never`

Ends the program with exit status `status` (`[Exit]`, `spec/21` §2k, D-0150). The calling thread unwinds
first, so its destructors run and standard output is written out; other threads are not waited for. The
result type is `never`, so a branch that calls `exit` needs no value of its own. `fn main() : u8`'s return
value is the ordinary way to set the status; `exit` is for deep inside a program.

## `sqrt` in `std::math`
Signature: `fn sqrt(T x) : T`

The square root of `x`, correctly rounded (`[Float-Fn]`, `spec/21` §3d, D-0091): every conforming
implementation gives the same bits. A NaN for a negative `x`; `-0.0` for `-0.0`; a NaN in gives a NaN out.

## `floor` in `std::math`
Signature: `fn floor(T x) : T`

`x` rounded to an integral value toward negative infinity, exactly (`[Float-Fn]`). A NaN or an infinity is
returned as it is.

## `ceil` in `std::math`
Signature: `fn ceil(T x) : T`

`x` rounded to an integral value toward positive infinity, exactly (`[Float-Fn]`).

## `round` in `std::math`
Signature: `fn round(T x) : T`

`x` rounded to the nearest integral value, halfway cases away from zero (`2.5` to `3.0`, `-2.5` to
`-3.0`), exactly (`[Float-Fn]`). Convert the result with `rule.arith.convert` when an integer is wanted.

## `trunc` in `std::math`
Signature: `fn trunc(T x) : T`

`x` rounded to an integral value toward zero, exactly (`[Float-Fn]`): the fractional part dropped.

## `ln` in `std::math`
Signature: `fn ln(T x) : T`

The natural logarithm of `x`, within one unit in the last place of the mathematical value
(`[Float-Transcendental]`, D-0101): the platform's C library computes it, so `coby` and `cobc` agree on one
platform and may differ in the last bit between platforms. A NaN for a negative `x`; negative infinity for
`±0`.

## `exp` in `std::math`
Signature: `fn exp(T x) : T`

`e` to the power `x`, within one unit in the last place (`[Float-Transcendental]`). Overflows to infinity
as IEEE-754 says.

## `log2` in `std::math`
Signature: `fn log2(T x) : T`

The base-2 logarithm of `x`, within one unit in the last place (`[Float-Transcendental]`); a NaN for a
negative `x`, negative infinity for `±0`. For an integer's bit length, `integer` arithmetic is exact and
cheaper.

## `log10` in `std::math`
Signature: `fn log10(T x) : T`

The base-10 logarithm of `x`, within one unit in the last place (`[Float-Transcendental]`); a NaN for a
negative `x`, negative infinity for `±0`.

## `sin` in `std::math`
Signature: `fn sin(T x) : T`

The sine of `x`, an angle in radians, within one unit in the last place (`[Float-Transcendental]`).

## `cos` in `std::math`
Signature: `fn cos(T x) : T`

The cosine of `x`, an angle in radians, within one unit in the last place (`[Float-Transcendental]`).

## `tan` in `std::math`
Signature: `fn tan(T x) : T`

The tangent of `x`, an angle in radians, within one unit in the last place (`[Float-Transcendental]`).

## `asin` in `std::math`
Signature: `fn asin(T x) : T`

The arc sine of `x`, in radians in `[-π/2, π/2]`, within one unit in the last place
(`[Float-Transcendental]`, D-0181); a NaN for `x` outside `[-1, 1]`.

## `acos` in `std::math`
Signature: `fn acos(T x) : T`

The arc cosine of `x`, in radians in `[0, π]`, within one unit in the last place
(`[Float-Transcendental]`, D-0181); a NaN for `x` outside `[-1, 1]`.

## `atan` in `std::math`
Signature: `fn atan(T x) : T`

The arc tangent of `x`, in radians in `(-π/2, π/2)`, within one unit in the last place
(`[Float-Transcendental]`, D-0181). For an angle from two coordinates use `atan2`, which knows the quadrant.

## `atan2` in `std::math`
Signature: `fn atan2(T y, T x) : T`

The angle of the point `(x, y)` from the positive x axis, in radians in `(-π, π]`, within one unit in the
last place (`[Float-Transcendental-2]`); C's `atan2`. Both operands have the same type.

## `powf` in `std::math`
Signature: `fn powf(T x, T y) : T`

`x` to the power `y` for floating-point operands, within one unit in the last place
(`[Float-Transcendental-2]`); C's `pow`. For an integer exponent, `pow` (an ordinary `std::math` function)
is exact and checked for overflow.
