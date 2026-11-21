/* boot -- builds CobaltC on a machine that has nothing but a C compiler
   (D-0326). From the repository's root:

       gcc impl/bootstrap/boot.c -o boot && ./boot            (Linux)
       gcc impl\bootstrap\boot.c -o boot.exe && boot.exe      (Windows, MinGW-w64)

   It builds the runtime (impl/cbrt, C), unpacks the seed (the compiler's
   own C, impl/bootstrap/cobc-seed.c.gz), compiles it into
   impl/target/release/cobc, proves the result by having that compiler write
   the C of its own source again, which must be the seed byte for byte, then
   builds the formatter and the model with it, and tries them.

       boot [--cc COMPILER] [--no-verify] [--runtime-only] [--runtime DIR] [--clean]

   --cc names the C compiler (default: $CC, else the first of gcc, cc and
   clang that runs); --no-verify skips the proof; --runtime-only builds the
   runtime and stops; --runtime DIR takes the libcbrt.a in DIR instead of
   building one (for testing); --clean removes impl/target/boot and
   impl/target/stage0 first. Needs: a C compiler for x86-64 that accepts GNU
   C11 (GCC 10 or later, Clang 14 or later, MinGW-w64's GCC on Windows), its
   `ar`, and about 3 GB of memory for the one large compile. Nothing else:
   no shell, no Python, no archive tool. */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#define EXE ".exe"
#define SEP "\\"
#define NULL_DEV "NUL"
#else
#define EXE ""
#define SEP "/"
#define NULL_DEV "/dev/null"
#endif

static const char *cc = NULL;
static const char *ar = NULL;

/* ---- small helpers ---- */

static void die(const char *what)
{
    fprintf(stderr, "boot: %s\n", what);
    exit(1);
}

static void step(const char *what)
{
    printf("== %s\n", what);
    fflush(stdout);
}

/* A command run through the system's shell, its status 0 or not. */
static int run(const char *cmd)
{
    fflush(stdout);
    fflush(stderr);
    return system(cmd) == 0;
}

static int run_quiet(const char *cmd)
{
    char buf[4096];
    snprintf(buf, sizeof buf, "%s > %s 2>&1", cmd, NULL_DEV);
    return run(buf);
}

/* A path for the shell to run: on Windows `cmd` needs backslashes in a
   program's own path. */
static void native_path(char *out, size_t n, const char *path)
{
    snprintf(out, n, "%s", path);
#ifdef _WIN32
    for (char *p = out; *p; p++)
    {
        if (*p == '/')
        {
            *p = '\\';
        }
    }
#endif
}

static void make_dir(const char *path)
{
    char p[1024];
    char cmd[1200];
    native_path(p, sizeof p, path);
#ifdef _WIN32
    snprintf(cmd, sizeof cmd, "if not exist %s mkdir %s", p, p);
#else
    snprintf(cmd, sizeof cmd, "mkdir -p %s", p);
#endif
    if (!run(cmd))
    {
        die("cannot make a directory");
    }
}

static int exists(const char *path)
{
    FILE *f = fopen(path, "rb");
    if (f)
    {
        fclose(f);
        return 1;
    }
    return 0;
}

/* The whole of a file, its length in *n; NULL when it cannot be read. */
static unsigned char *read_all(const char *path, size_t *n)
{
    FILE *f = fopen(path, "rb");
    if (!f)
    {
        return NULL;
    }
    size_t cap = 1 << 20;
    size_t len = 0;
    unsigned char *buf = malloc(cap);
    if (!buf)
    {
        die("out of memory");
    }
    for (;;)
    {
        if (len == cap)
        {
            cap *= 2;
            buf = realloc(buf, cap);
            if (!buf)
            {
                die("out of memory");
            }
        }
        size_t got = fread(buf + len, 1, cap - len, f);
        if (got == 0)
        {
            break;
        }
        len += got;
    }
    fclose(f);
    *n = len;
    return buf;
}

static void write_all(const char *path, const unsigned char *b, size_t n)
{
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(b, 1, n, f) != n || fclose(f) != 0)
    {
        fprintf(stderr, "boot: cannot write %s\n", path);
        exit(1);
    }
}

/* ---- CRC-32 (the gzip trailer's) ---- */

static uint32_t crc_table[256];

static void crc_init(void)
{
    for (uint32_t i = 0; i < 256; i++)
    {
        uint32_t c = i;
        for (int k = 0; k < 8; k++)
        {
            c = (c & 1) ? 0xEDB88320u ^ (c >> 1) : c >> 1;
        }
        crc_table[i] = c;
    }
}

static uint32_t crc32_of(const unsigned char *b, size_t n)
{
    uint32_t c = 0xFFFFFFFFu;
    for (size_t i = 0; i < n; i++)
    {
        c = crc_table[(c ^ b[i]) & 0xFF] ^ (c >> 8);
    }
    return c ^ 0xFFFFFFFFu;
}

/* ---- inflate (RFC 1951), for the gzip file (RFC 1952) ---- */

struct bits
{
    const unsigned char *in;
    size_t len;
    size_t pos;
    uint32_t buf;
    int count;
};

static int getbit(struct bits *s)
{
    if (s->count == 0)
    {
        if (s->pos >= s->len)
        {
            die("the seed is cut short");
        }
        s->buf = s->in[s->pos++];
        s->count = 8;
    }
    int b = s->buf & 1;
    s->buf >>= 1;
    s->count--;
    return b;
}

static uint32_t getbits(struct bits *s, int n)
{
    uint32_t v = 0;
    for (int i = 0; i < n; i++)
    {
        v |= (uint32_t)getbit(s) << i;
    }
    return v;
}

/* A canonical Huffman code: how many codes of each length, and the symbols
   in code order. */
struct huff
{
    uint16_t count[16];
    uint16_t symbol[320];
};

static void huff_build(struct huff *h, const uint8_t *lengths, int n)
{
    uint16_t offs[16];
    memset(h->count, 0, sizeof h->count);
    for (int i = 0; i < n; i++)
    {
        h->count[lengths[i]]++;
    }
    h->count[0] = 0;
    offs[1] = 0;
    for (int i = 1; i < 15; i++)
    {
        offs[i + 1] = offs[i] + h->count[i];
    }
    for (int i = 0; i < n; i++)
    {
        if (lengths[i])
        {
            h->symbol[offs[lengths[i]]++] = (uint16_t)i;
        }
    }
}

static int huff_decode(struct bits *s, const struct huff *h)
{
    int code = 0;
    int first = 0;
    int index = 0;
    for (int len = 1; len < 16; len++)
    {
        code |= getbit(s);
        int count = h->count[len];
        if (code - count < first)
        {
            return h->symbol[index + (code - first)];
        }
        index += count;
        first += count;
        first <<= 1;
        code <<= 1;
    }
    die("the seed is not a valid gzip file (a bad code)");
    return -1;
}

struct out
{
    unsigned char *b;
    size_t len;
    size_t cap;
};

static void put(struct out *o, unsigned char c)
{
    if (o->len == o->cap)
    {
        o->cap = o->cap ? o->cap * 2 : (1 << 24);
        o->b = realloc(o->b, o->cap);
        if (!o->b)
        {
            die("out of memory");
        }
    }
    o->b[o->len++] = c;
}

static const uint16_t len_base[29] = {3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258};
static const uint8_t len_extra[29] = {0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0};
static const uint16_t dist_base[30] = {1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577};
static const uint8_t dist_extra[30] = {0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13};

static void inflate_codes(struct bits *s, struct out *o, const struct huff *lit, const struct huff *dist)
{
    for (;;)
    {
        int sym = huff_decode(s, lit);
        if (sym < 256)
        {
            put(o, (unsigned char)sym);
        }
        else if (sym == 256)
        {
            return;
        }
        else
        {
            sym -= 257;
            if (sym >= 29)
            {
                die("the seed is not a valid gzip file (a bad length)");
            }
            size_t len = len_base[sym] + getbits(s, len_extra[sym]);
            int d = huff_decode(s, dist);
            if (d >= 30)
            {
                die("the seed is not a valid gzip file (a bad distance)");
            }
            size_t back = dist_base[d] + getbits(s, dist_extra[d]);
            if (back > o->len)
            {
                die("the seed is not a valid gzip file (a distance too far)");
            }
            for (size_t i = 0; i < len; i++)
            {
                put(o, o->b[o->len - back]);
            }
        }
    }
}

static void inflate(struct bits *s, struct out *o)
{
    int last;
    do
    {
        last = getbit(s);
        int type = (int)getbits(s, 2);
        if (type == 0)
        {
            s->count = 0;
            if (s->pos + 4 > s->len)
            {
                die("the seed is cut short");
            }
            unsigned n = s->in[s->pos] | (s->in[s->pos + 1] << 8);
            s->pos += 4;
            if (s->pos + n > s->len)
            {
                die("the seed is cut short");
            }
            for (unsigned i = 0; i < n; i++)
            {
                put(o, s->in[s->pos++]);
            }
        }
        else if (type == 1)
        {
            static struct huff lit, dist;
            static int made = 0;
            if (!made)
            {
                uint8_t l[288];
                int i = 0;
                for (; i < 144; i++)
                {
                    l[i] = 8;
                }
                for (; i < 256; i++)
                {
                    l[i] = 9;
                }
                for (; i < 280; i++)
                {
                    l[i] = 7;
                }
                for (; i < 288; i++)
                {
                    l[i] = 8;
                }
                huff_build(&lit, l, 288);
                for (i = 0; i < 30; i++)
                {
                    l[i] = 5;
                }
                huff_build(&dist, l, 30);
                made = 1;
            }
            inflate_codes(s, o, &lit, &dist);
        }
        else if (type == 2)
        {
            static const uint8_t order[19] = {16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15};
            int nlen = (int)getbits(s, 5) + 257;
            int ndist = (int)getbits(s, 5) + 1;
            int ncode = (int)getbits(s, 4) + 4;
            uint8_t lengths[320];
            memset(lengths, 0, sizeof lengths);
            for (int i = 0; i < ncode; i++)
            {
                lengths[order[i]] = (uint8_t)getbits(s, 3);
            }
            struct huff codes;
            huff_build(&codes, lengths, 19);
            int i = 0;
            while (i < nlen + ndist)
            {
                int sym = huff_decode(s, &codes);
                if (sym < 16)
                {
                    lengths[i++] = (uint8_t)sym;
                }
                else
                {
                    int rep;
                    uint8_t v = 0;
                    if (sym == 16)
                    {
                        if (i == 0)
                        {
                            die("the seed is not a valid gzip file (a repeat first)");
                        }
                        v = lengths[i - 1];
                        rep = 3 + (int)getbits(s, 2);
                    }
                    else if (sym == 17)
                    {
                        rep = 3 + (int)getbits(s, 3);
                    }
                    else
                    {
                        rep = 11 + (int)getbits(s, 7);
                    }
                    if (i + rep > nlen + ndist)
                    {
                        die("the seed is not a valid gzip file (too many lengths)");
                    }
                    while (rep--)
                    {
                        lengths[i++] = v;
                    }
                }
            }
            struct huff lit, dist;
            huff_build(&lit, lengths, nlen);
            huff_build(&dist, lengths + nlen, ndist);
            inflate_codes(s, o, &lit, &dist);
        }
        else
        {
            die("the seed is not a valid gzip file (block type 3)");
        }
    } while (!last);
}

/* The gzip file `path` unpacked; its trailer's length and CRC-32 checked. */
static unsigned char *gunzip(const char *path, size_t *n)
{
    size_t zlen;
    unsigned char *z = read_all(path, &zlen);
    if (!z)
    {
        fprintf(stderr, "boot: cannot read %s\n", path);
        exit(1);
    }
    if (zlen < 18 || z[0] != 0x1f || z[1] != 0x8b || z[2] != 8)
    {
        die("the seed is not a gzip file");
    }
    int flags = z[3];
    size_t pos = 10;
    if (flags & 4)
    {
        pos += 2 + (z[pos] | (z[pos + 1] << 8));
    }
    if (flags & 8)
    {
        while (pos < zlen && z[pos])
        {
            pos++;
        }
        pos++;
    }
    if (flags & 16)
    {
        while (pos < zlen && z[pos])
        {
            pos++;
        }
        pos++;
    }
    if (flags & 2)
    {
        pos += 2;
    }
    struct bits s = {z, zlen - 8, pos, 0, 0};
    struct out o = {NULL, 0, 0};
    inflate(&s, &o);
    const unsigned char *t = z + zlen - 8;
    uint32_t crc = t[0] | (t[1] << 8) | (t[2] << 16) | ((uint32_t)t[3] << 24);
    uint32_t size = t[4] | (t[5] << 8) | (t[6] << 16) | ((uint32_t)t[7] << 24);
    if ((uint32_t)o.len != size || crc32_of(o.b, o.len) != crc)
    {
        die("the seed's checksum is wrong (the file is damaged)");
    }
    free(z);
    *n = o.len;
    return o.b;
}

/* ---- the seed's record (impl/bootstrap/SEED) ---- */

/* `length N` and `crc32 HEX` lines; 0 when absent. */
static void seed_record(size_t *len, uint32_t *crc)
{
    size_t n;
    unsigned char *t = read_all("impl/bootstrap/SEED", &n);
    *len = 0;
    *crc = 0;
    if (!t)
    {
        die("no impl/bootstrap/SEED (run boot from the repository's root)");
    }
    t = realloc(t, n + 1);
    t[n] = 0;
    char *p = (char *)t;
    while (*p)
    {
        unsigned long long v;
        unsigned x;
        if (sscanf(p, "length %llu", &v) == 1)
        {
            *len = (size_t)v;
        }
        else if (sscanf(p, "crc32 %x", &x) == 1)
        {
            *crc = x;
        }
        char *nl = strchr(p, '\n');
        if (!nl)
        {
            break;
        }
        p = nl + 1;
    }
    free(t);
}

/* `text` without its `#line` lines (the seed is the compiler's C without
   them: mkseed.sh). */
static unsigned char *strip_line_directives(const unsigned char *b, size_t n, size_t *out_n)
{
    unsigned char *o = malloc(n + 1);
    if (!o)
    {
        die("out of memory");
    }
    size_t k = 0;
    size_t i = 0;
    while (i < n)
    {
        size_t j = i;
        while (j < n && b[j] != '\n')
        {
            j++;
        }
        size_t end = j < n ? j + 1 : j;
        if (!(end - i >= 6 && memcmp(b + i, "#line ", 6) == 0))
        {
            memcpy(o + k, b + i, end - i);
            k += end - i;
        }
        i = end;
    }
    *out_n = k;
    return o;
}

/* ---- the C compiler ---- */

static int compiler_runs(const char *c)
{
    char cmd[512];
    snprintf(cmd, sizeof cmd, "%s --version", c);
    return run_quiet(cmd);
}

static void find_tools(const char *given)
{
    if (given)
    {
        cc = given;
    }
    else if (getenv("CC") && getenv("CC")[0])
    {
        cc = getenv("CC");
    }
    else
    {
        static const char *tries[] = {"gcc", "cc", "clang"};
        for (int i = 0; i < 3 && !cc; i++)
        {
            if (compiler_runs(tries[i]))
            {
                cc = tries[i];
            }
        }
    }
    if (!cc || !compiler_runs(cc))
    {
        die("no C compiler found: install GCC or Clang (on Windows MinGW-w64's GCC), or name one with --cc");
    }
    if (getenv("AR") && getenv("AR")[0])
    {
        ar = getenv("AR");
    }
    else
    {
        static const char *tries[] = {"ar", "gcc-ar", "llvm-ar"};
        for (int i = 0; i < 3 && !ar; i++)
        {
            char cmd[256];
            snprintf(cmd, sizeof cmd, "%s --version", tries[i]);
            if (run_quiet(cmd))
            {
                ar = tries[i];
            }
        }
    }
    if (!ar)
    {
        die("no `ar` found (it comes with GCC's binutils, or llvm-ar with Clang); name one with AR=");
    }
    printf("C compiler: %s; archiver: %s\n", cc, ar);
}

static const char *PROBE =
    "#include <stdint.h>\n"
    "#if !defined(__x86_64__)\n#error CobaltC compiles for x86-64 only\n#endif\n"
    "int main(void)\n"
    "{\n"
    "    __int128 a = 1;\n"
    "    long r;\n"
    "    int over = __builtin_add_overflow(1L, 2L, &r);\n"
    "    int v = ({ int t = (int)a; t + over; });\n"
    "    return v == 1 && r == 3 ? 0 : 1;\n"
    "}\n";

static void probe(void)
{
    write_all("impl/target/boot/probe.c", (const unsigned char *)PROBE, strlen(PROBE));
    char cmd[1024];
    snprintf(cmd, sizeof cmd, "%s -std=gnu11 -o impl/target/boot/probe%s impl/target/boot/probe.c", cc, EXE);
    if (!run(cmd))
    {
        die("the C compiler cannot compile what CobaltC writes: it needs x86-64, GNU C11, __int128, statement "
            "expressions and __builtin_add_overflow (GCC 10 or later, Clang 14 or later, MinGW-w64's GCC)");
    }
    char exe[256];
    native_path(exe, sizeof exe, "impl/target/boot/probe" EXE);
    if (!run(exe))
    {
        die("a program the C compiler built does not run here");
    }
}

/* ---- the runtime (impl/cbrt: FLAGS, SOURCES) ---- */

/* The steps of impl/cbrt/build.sh: every `.c` SOURCES lists, but the parts
   of the core (src/rt_*.c, src/replay.c, which src/lib.c includes), compiled
   with FLAGS and the stamp; one relocatable object whose only global symbols
   are tools/exports.txt's (where `ld` and `objcopy` are at hand: MinGW-w64
   and every GCC have them; without them the objects go into the archive as
   they are); `ar` makes libcbrt.a. */
static char runtime_libs[512];

/* tools/stamp.c's hash (see there): 64-bit FNV-1a over each SOURCES file's
   path, a 0 byte, its length (8 bytes, least significant first) and its
   bytes, carriage returns skipped. */
static uint64_t stamp_h = 0xcbf29ce484222325ULL;

static void stamp_mix(const unsigned char *p, size_t n)
{
    for (size_t i = 0; i < n; i++)
    {
        stamp_h ^= p[i];
        stamp_h *= 0x100000001b3ULL;
    }
}

static void stamp_file(const char *name)
{
    char path[600];
    snprintf(path, sizeof path, "impl/cbrt/%s", name);
    size_t n = 0;
    unsigned char *b = read_all(path, &n);
    size_t k = 0;
    if (b)
    {
        for (size_t i = 0; i < n; i++)
        {
            if (b[i] != '\r')
            {
                b[k++] = b[i];
            }
        }
    }
    stamp_mix((const unsigned char *)name, strlen(name) + 1);
    unsigned char le[8];
    for (int i = 0; i < 8; i++)
    {
        le[i] = (unsigned char)((uint64_t)k >> (8 * i));
    }
    stamp_mix(le, 8);
    if (b)
    {
        stamp_mix(b, k);
        free(b);
    }
}

static int tool_runs(const char *t)
{
    char cmd[600];
    snprintf(cmd, sizeof cmd, "%s --version", t);
    return run_quiet(cmd);
}

static void build_runtime(void)
{
    size_t n;
    unsigned char *t = read_all("impl/cbrt/SOURCES", &n);
    if (!t)
    {
        die("no impl/cbrt/SOURCES (run boot from the repository's root)");
    }
    t = realloc(t, n + 1);
    t[n] = 0;
    size_t fn;
    unsigned char *f = read_all("impl/cbrt/FLAGS", &fn);
    if (!f)
    {
        die("no impl/cbrt/FLAGS");
    }
    char flags[1024];
    size_t fl = fn < sizeof flags - 1 ? fn : sizeof flags - 1;
    memcpy(flags, f, fl);
    flags[fl] = 0;
    free(f);
    for (size_t i = 0; i < fl; i++)
    {
        if (flags[i] == '\n' || flags[i] == '\r')
        {
            flags[i] = ' ';
        }
    }
    /* First the stamp, over every listed file. */
    char *names[256];
    int count = 0;
    char *line = (char *)t;
    while (line && *line)
    {
        char *nl = strchr(line, '\n');
        if (nl)
        {
            *nl = 0;
        }
        size_t L = strlen(line);
        while (L && (line[L - 1] == '\r' || line[L - 1] == ' '))
        {
            line[--L] = 0;
        }
        if (L && line[0] != '#')
        {
            if (count == 256)
            {
                die("impl/cbrt/SOURCES lists too many files");
            }
            stamp_file(line);
            names[count++] = line;
        }
        line = nl ? nl + 1 : NULL;
    }
    char stamp[17];
    snprintf(stamp, sizeof stamp, "%016llx", (unsigned long long)stamp_h);
    printf("   stamp %s\n", stamp);
    char objects[16384] = "";
    int built = 0;
    for (int i = 0; i < count; i++)
    {
        const char *p = names[i];
        size_t L = strlen(p);
        if (L < 2 || strcmp(p + L - 2, ".c") != 0 || strncmp(p, "src/rt_", 7) == 0 || strcmp(p, "src/replay.c") == 0)
        {
            continue;
        }
        char obj[600];
        char cmd[4096];
        snprintf(obj, sizeof obj, "impl/target/boot/cbrt-%d.o", built++);
        snprintf(cmd, sizeof cmd, "%s %s -DCBRT_STAMP=\\\"%s\\\" -c -o %s impl/cbrt/%s", cc, flags, stamp, obj, p);
        printf("   %s\n", p);
        if (!run(cmd))
        {
            die("the runtime does not compile (the C compiler's message is above)");
        }
        strncat(objects, " ", sizeof objects - strlen(objects) - 1);
        strncat(objects, obj, sizeof objects - strlen(objects) - 1);
    }
    free(t);
    if (built == 0)
    {
        die("impl/cbrt/SOURCES names no C file");
    }
    char cmd[20000];
    const char *members = objects;
    if (tool_runs("ld") && tool_runs("objcopy"))
    {
        snprintf(cmd, sizeof cmd, "ld -r -o impl/target/boot/cbrt.o%s", objects);
        if (!run(cmd))
        {
            die("ld could not join the runtime's objects");
        }
        if (!run("objcopy --keep-global-symbols=impl/cbrt/tools/exports.txt impl/target/boot/cbrt.o"))
        {
            die("objcopy could not hide the runtime's own names");
        }
        members = " impl/target/boot/cbrt.o";
    }
    else
    {
        printf("   (no ld and objcopy: the runtime's own names stay visible to a program's C)\n");
    }
    remove("impl/target/release/libcbrt.a");
    snprintf(cmd, sizeof cmd, "%s rcs impl/target/release/libcbrt.a%s", ar, members);
    if (!run(cmd))
    {
        die("ar could not make libcbrt.a");
    }
#ifdef _WIN32
    snprintf(runtime_libs, sizeof runtime_libs, "-lkernel32 -ladvapi32 -lbcrypt -lntdll -luserenv -lws2_32");
#else
    snprintf(runtime_libs, sizeof runtime_libs, "-lpthread -ldl -lm");
#endif
}

/* ---- main ---- */

int main(int argc, char **argv)
{
    const char *given_cc = NULL;
    const char *runtime_dir = NULL;
    int verify = 1;
    int runtime_only = 0;
    int clean = 0;
    for (int i = 1; i < argc; i++)
    {
        if (strcmp(argv[i], "--cc") == 0 && i + 1 < argc)
        {
            given_cc = argv[++i];
        }
        else if (strcmp(argv[i], "--runtime") == 0 && i + 1 < argc)
        {
            runtime_dir = argv[++i];
        }
        else if (strcmp(argv[i], "--no-verify") == 0)
        {
            verify = 0;
        }
        else if (strcmp(argv[i], "--runtime-only") == 0)
        {
            runtime_only = 1;
        }
        else if (strcmp(argv[i], "--clean") == 0)
        {
            clean = 1;
        }
        else
        {
            fprintf(stderr, "usage: boot [--cc COMPILER] [--no-verify] [--runtime-only] [--runtime DIR] [--clean]\n");
            return 2;
        }
    }
    if (!exists("impl/bootstrap/boot.c"))
    {
        die("run boot from the repository's root (where impl/ is)");
    }
    crc_init();
    if (clean)
    {
#ifdef _WIN32
        run_quiet("if exist impl\\target\\boot rmdir /s /q impl\\target\\boot");
        run_quiet("if exist impl\\target\\stage0 rmdir /s /q impl\\target\\stage0");
#else
        run_quiet("rm -rf impl/target/boot impl/target/stage0");
#endif
    }
    make_dir("impl/target/boot");
    make_dir("impl/target/stage0");
    make_dir("impl/target/release");

    step("the C compiler");
    find_tools(given_cc);
    probe();

    step("the runtime (impl/cbrt)");
    if (runtime_dir)
    {
        char cmd[1024];
        char from[600];
        snprintf(from, sizeof from, "%s/libcbrt.a", runtime_dir);
        size_t n;
        unsigned char *lib = read_all(from, &n);
        if (!lib)
        {
            die("no libcbrt.a in the --runtime directory");
        }
        write_all("impl/target/release/libcbrt.a", lib, n);
        free(lib);
        (void)cmd;
#ifdef _WIN32
        snprintf(runtime_libs, sizeof runtime_libs, "-lkernel32 -ladvapi32 -lbcrypt -lntdll -luserenv -lws2_32");
#else
        snprintf(runtime_libs, sizeof runtime_libs, "-lpthread -ldl -lm");
#endif
        printf("   taken from %s\n", from);
    }
    else
    {
        build_runtime();
    }
    if (runtime_only)
    {
        printf("boot: impl/target/release/libcbrt.a is built\n");
        return 0;
    }

    step("the seed (impl/bootstrap/cobc-seed.c.gz)");
    size_t seed_len;
    unsigned char *seed = gunzip("impl/bootstrap/cobc-seed.c.gz", &seed_len);
    size_t want_len;
    uint32_t want_crc;
    seed_record(&want_len, &want_crc);
    if (seed_len != want_len || crc32_of(seed, seed_len) != want_crc)
    {
        die("the seed is not the one impl/bootstrap/SEED records");
    }
    write_all("impl/target/stage0/cobc.c", seed, seed_len);
    printf("   %zu bytes of C\n", seed_len);

    step("the compiler, from the seed (a few minutes; about 3 GB of memory)");
    {
        char cmd[4096];
        snprintf(cmd, sizeof cmd,
                 "%s -std=gnu11 -ffp-contract=off -fno-optimize-sibling-calls -w -O2 -o impl/target/release/cobc%s "
                 "impl/target/stage0/cobc.c impl/cobc/host.c -Limpl/target/release -lcbrt %s",
                 cc, EXE, runtime_libs);
        if (!run(cmd))
        {
            die("the C compiler could not build the compiler from the seed");
        }
    }
    char cobc[256];
    native_path(cobc, sizeof cobc, "impl/target/release/cobc" EXE);

    if (verify)
    {
        step("the proof: the compiler writes its own C again, which must be the seed (a few minutes)");
        /* In the compiler's own directory, as impl/bootstrap/mkseed.sh makes the seed: the C names each
           module file by its path from there. */
        char dir[256];
        char exe[256];
        char cmd[2048];
        native_path(dir, sizeof dir, "impl/cobc");
        native_path(exe, sizeof exe, "../target/release/cobc" EXE);
        snprintf(cmd, sizeof cmd, "cd %s && %s --emit-c --c-name cobc.c -o ../target/stage0/again.c main.cb", dir, exe);
        if (!run(cmd))
        {
            die("the new compiler could not compile its own source");
        }
        size_t again_len;
        unsigned char *again = read_all("impl/target/stage0/again.c", &again_len);
        if (!again)
        {
            die("the new compiler wrote no C");
        }
        size_t stripped_len;
        unsigned char *stripped = strip_line_directives(again, again_len, &stripped_len);
        if (stripped_len != seed_len || memcmp(stripped, seed, seed_len) != 0)
        {
            die("the compiler built from the seed writes C that is not the seed: the seed does not match this "
                "tree's compiler source (impl/bootstrap/mkseed.sh writes it), or the C compiler miscompiled it");
        }
        free(again);
        free(stripped);
        printf("   the same, byte for byte (%zu bytes)\n", seed_len);
    }
    free(seed);

    step("the formatter (impl/cobfmt)");
    {
        char cmd[2048];
        snprintf(cmd, sizeof cmd, "%s --cc %s -O 2 -o impl/target/release/cobfmt%s impl/cobfmt/main.cb", cobc, cc, EXE);
        if (!run(cmd))
        {
            die("the formatter did not build");
        }
    }
    if (exists("impl/model/src/main.cb"))
    {
        step("the model (impl/model)");
        char cmd[2048];
        snprintf(cmd, sizeof cmd, "%s --cc %s -O 2 -o impl/target/release/cobmodel%s impl/model/src/main.cb", cobc, cc, EXE);
        if (!run(cmd))
        {
            die("the model did not build");
        }
    }

    step("a first program");
    {
        static const char *HELLO =
            "import std;\n"
            "\n"
            "fn main()\n"
            "{\n"
            "    printf(\"hello from CobaltC: %v\\n\", 6 * 7);\n"
            "}\n";
        write_all("impl/target/boot/hello.cb", (const unsigned char *)HELLO, strlen(HELLO));
        char cmd[2048];
        snprintf(cmd, sizeof cmd, "%s --cc %s --run impl/target/boot/hello.cb", cobc, cc);
        if (!run(cmd))
        {
            die("the first program did not compile and run");
        }
        char fmt[256];
        native_path(fmt, sizeof fmt, "impl/target/release/cobfmt" EXE);
        snprintf(cmd, sizeof cmd, "%s --check impl/target/boot/hello.cb", fmt);
        if (!run(cmd))
        {
            die("the formatter did not accept the first program");
        }
    }

    printf("\nboot: done. The tools are in impl/target/release: cobc, cobfmt%s, and libcbrt.a.\n",
           exists("impl/target/release/cobmodel" EXE) ? ", cobmodel" : "");
#ifdef _WIN32
    printf("Put them on PATH:   set PATH=%%CD%%\\impl\\target\\release;%%PATH%%\n");
#else
    printf("Put them on PATH:   export PATH=\"$PWD/impl/target/release:$PATH\"\n");
#endif
    printf("Then:               cobc --run impl/cobaltc_examples/12_collatz.cb\n");
    return 0;
}
