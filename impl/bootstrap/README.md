# The bootstrap seed

CobaltC's compiler, `cobc`, is written in CobaltC, so it needs a compiler to build it. This directory holds
the way out of that circle (D-0326): the compiler's own source translated to C once, so that a machine with
nothing but a C compiler can build it.

| File | What it is |
|---|---|
| `cobc-seed.c.gz` | the **seed**: the C that `cobc` writes for its own source (`impl/cobc/main.cb` and everything it loads), without `#line` directives, gzipped |
| `SEED` | the seed's length and CRC-32, so that a damaged or mismatched seed is noticed before it is built |
| `boot.c` | **boot**, one C file that builds everything from the seed: the runtime, the compiler, the formatter and the model |
| `mkseed.sh` | writes the seed and `SEED` again from the current source (for developers, after the compiler's source changes) |

The quick way, from the repository's root:

```sh
gcc impl/bootstrap/boot.c -o boot && ./boot                 # Linux
gcc impl\bootstrap\boot.c -o boot.exe && boot.exe           # Windows, MinGW-w64
```

boot's options: `--cc COMPILER`, `--no-verify` (skip step 5), `--runtime-only` (stop after step 2),
`--runtime DIR` (use the `libcbrt.a` in DIR) and `--clean` (remove `impl/target/boot` and `impl/target/stage0`
first). Everything below is what boot does, step by step, so that you can do it yourself,
see where a step fails, or check a step boot took. The commands are for Linux with GCC from the repository's
root; on Windows put `.exe` after each program's name and link the libraries given in step 4.

## What boot does, and how to do it by hand

**1. Find a C compiler.** boot takes `--cc`, else `$CC`, else the first of `gcc`, `cc` and `clang` that answers
`--version`, and compiles a small probe to check that it accepts GNU C11 and targets x86-64. It needs GCC 10 or
later, Clang 14 or later, or MinGW-w64's GCC, the matching `ar`, and about 3 GB of memory for step 4.

```sh
mkdir -p impl/target/boot impl/target/stage0 impl/target/release
gcc --version
```

**2. Build the runtime, `libcbrt.a`.** The runtime (`impl/cbrt`) is plain C. Its compiler options are in
`impl/cbrt/FLAGS`, its files in `impl/cbrt/SOURCES`. Every program `cobc` builds links it, and `cobc` refuses a
runtime built from other sources than its own, by a *stamp*: a 64-bit hash of every file `SOURCES` lists
(`impl/cbrt/tools/stamp.c` computes it; boot computes the same hash itself). Each listed `.c` file is
compiled with the flags and the stamp, except `src/rt_*.c` and `src/replay.c`, which `src/lib.c` includes;
the objects are joined into one, which keeps only the names in `impl/cbrt/tools/exports.txt` global (where
`ld` and `objcopy` exist), and `ar` makes the library.

```sh
gcc -O1 -o impl/target/boot/stamp impl/cbrt/tools/stamp.c
STAMP=$(impl/target/boot/stamp impl/cbrt)
FLAGS=$(cat impl/cbrt/FLAGS)
objs=
for f in $(grep -v '^#' impl/cbrt/SOURCES | grep '\.c$' | grep -v -e '^src/rt_' -e '^src/replay\.c$'); do
    o=impl/target/boot/cbrt-$(basename $f .c).o
    gcc $FLAGS -DCBRT_STAMP=\"$STAMP\" -c -o $o impl/cbrt/$f && objs="$objs $o"
done
ld -r -o impl/target/boot/cbrt.o $objs
objcopy --keep-global-symbols=impl/cbrt/tools/exports.txt impl/target/boot/cbrt.o
rm -f impl/target/release/libcbrt.a
ar rcs impl/target/release/libcbrt.a impl/target/boot/cbrt.o
```

With a shell at hand, `impl/cbrt/build.sh --out impl/target/release` does the same.

**3. Unpack the seed and check it.** boot has its own gzip decoder, so it needs no `gzip`; by hand:

```sh
gzip -dc impl/bootstrap/cobc-seed.c.gz > impl/target/stage0/cobc.c
wc -c < impl/target/stage0/cobc.c        # must be SEED's `length`
python3 -c "import zlib,sys; print('%08x' % (zlib.crc32(open(sys.argv[1],'rb').read()) & 0xffffffff))" impl/target/stage0/cobc.c
                                         # must be SEED's `crc32`
```

A length or CRC that differs means the seed is damaged, or is not the one `SEED` was written with: build
nothing from it.

**4. Compile the compiler from the seed.** One large C file, with `impl/cobc/host.c` (the few functions the
compiler takes from C) and the runtime. This is the step that needs the memory and a few minutes.

```sh
gcc -std=gnu11 -ffp-contract=off -fno-optimize-sibling-calls -w -O2 -o impl/target/release/cobc \
    impl/target/stage0/cobc.c impl/cobc/host.c -Limpl/target/release -lcbrt -lpthread -ldl -lm
```

On Windows the libraries are `-lkernel32 -ladvapi32 -lbcrypt -lntdll -luserenv -lws2_32` in place of
`-lpthread -ldl -lm`. The options before `-O2` are the ones `cobc` gives the C compiler for every program
(`impl/cobc/cc.cb`, `cc_flags_base`); keep them as they are.

**5. The proof.** The new compiler compiles its own source back to C, in the compiler's directory as
`mkseed.sh` does (the C names each module file by its path from there). Without its `#line` lines that C must
be the seed, byte for byte. If it is, the compiler you built is the one the source describes, whatever C
compiler built it; if not, either the seed is not this tree's (run `mkseed.sh`, below) or the C compiler
miscompiled it.

```sh
(cd impl/cobc && ../target/release/cobc --emit-c --c-name cobc.c -o ../target/stage0/again.c main.cb)
grep -v '^#line ' impl/target/stage0/again.c | cmp - impl/target/stage0/cobc.c && echo "the same, byte for byte"
```

**6. Build the formatter and the model with the new compiler.**

```sh
impl/target/release/cobc --cc gcc -O 2 -o impl/target/release/cobfmt impl/cobfmt/main.cb
impl/target/release/cobc --cc gcc -O 2 -o impl/target/release/cobmodel impl/model/src/main.cb
```

**7. Try them** on a first program:

```sh
printf 'import std;\n\nfn main()\n{\n    printf("hello from CobaltC: %%v\\n", 6 * 7);\n}\n' > impl/target/boot/hello.cb
impl/target/release/cobc --cc gcc --run impl/target/boot/hello.cb      # hello from CobaltC: 42
impl/target/release/cobfmt --check impl/target/boot/hello.cb
```

The tools are then in `impl/target/release`: `cobc`, `cobfmt`, `cobmodel` and `libcbrt.a`. From there on,
`impl/build.sh` rebuilds them from the source with the compiler you have (README, "Building").

## What mkseed.sh does, and how to do it by hand

The seed must be the C of the committed compiler source, so whoever changes the compiler (its front end, back
end, driver, or `std`, which is compiled into it) writes the seed again before committing; the gate's build
unit, and `impl/tools/pregate.py` before the gate starts, check that it was done. `mkseed.sh` uses the compiler
in `impl/target/release` (or `COBC`), so build that from the current source first (`impl/build.sh`).

1. It checks that `impl/cobc/build_info.cb` is in its committed form: no runtime stamp and no runtime
   directory. `impl/build.sh` writes both in only while it builds, so the seed is the neutral build.
2. It writes the compiler's C, in the compiler's own directory, and takes out the `#line` lines.
3. It records that C's length and CRC-32 in `SEED`, and gzips the C (`-9n`: no name or time in the file, so
   the same C always gives the same bytes) into `cobc-seed.c.gz`.

```sh
grep -q '^export const str CBRT_STAMP = "";' impl/cobc/build_info.cb && \
grep -q '^export const str CBRT_DIR = "";' impl/cobc/build_info.cb || echo "build_info.cb is not committed form"
(cd impl/cobc && ../target/release/cobc --emit-c --c-name cobc.c -o /tmp/full.c main.cb)
grep -v '^#line ' /tmp/full.c > /tmp/cobc.c
python3 - <<'EOF'
import zlib
data = open("/tmp/cobc.c", "rb").read()
with open("impl/bootstrap/SEED", "w") as f:
    f.write("# The bootstrap seed, impl/bootstrap/cobc-seed.c.gz (D-0326): written by\n")
    f.write("# impl/bootstrap/mkseed.sh; checked by impl/bootstrap/boot.c. Do not edit.\n")
    f.write("length %d\n" % len(data))
    f.write("crc32 %08x\n" % (zlib.crc32(data) & 0xffffffff))
EOF
gzip -9n -c /tmp/cobc.c > impl/bootstrap/cobc-seed.c.gz
```

`impl/bootstrap/mkseed.sh --check` writes nothing: it exits 1 unless the committed seed and `SEED` are what
those steps would write. Every build of the compiler writes the same C for the same source, whichever C
compiler built it (the gate's `bootstrap` unit checks that with GCC and with Clang), so it does not matter
which build of `cobc` writes the seed, as long as its source is the committed one.
