/* The runtime's source stamp (D-0326): a hash of every file SOURCES lists,
   in its order, which the build bakes into libcbrt.a (`CBRT-STAMP:` and
   16 hex digits, src/rt_init.c) and into the compilers, so that a compiler
   refuses a runtime built from other sources (a stale libcbrt.a).

       stamp CBRT_DIR        prints the 16 hex digits

   The hash is 64-bit FNV-1a over, for each file: its path as SOURCES
   writes it, a 0 byte, its length as 8 bytes (least significant first),
   then its bytes; a carriage return is skipped wherever it occurs, so a
   checkout that ends its lines with CR LF stamps as one that ends them
   with LF. A missing file counts as empty. Lines of SOURCES that are empty
   or start with `#` are not paths. impl/bootstrap/boot.c computes the same
   value. Standard C only. */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t h = 0xcbf29ce484222325ULL;

static void mix(const unsigned char *p, size_t n)
{
    for (size_t i = 0; i < n; i++)
    {
        h ^= p[i];
        h *= 0x100000001b3ULL;
    }
}

/* The file's bytes without carriage returns, in a buffer the caller frees;
   its length in *n. NULL and *n == 0 for a missing file. */
static unsigned char *slurp(const char *path, size_t *n)
{
    *n = 0;
    FILE *f = fopen(path, "rb");
    if (!f)
    {
        return NULL;
    }
    size_t cap = 1 << 16, len = 0;
    unsigned char *buf = malloc(cap);
    if (!buf)
    {
        fprintf(stderr, "stamp: out of memory\n");
        exit(4);
    }
    int c;
    while ((c = fgetc(f)) != EOF)
    {
        if (c == '\r')
        {
            continue;
        }
        if (len == cap)
        {
            cap *= 2;
            buf = realloc(buf, cap);
            if (!buf)
            {
                fprintf(stderr, "stamp: out of memory\n");
                exit(4);
            }
        }
        buf[len++] = (unsigned char)c;
    }
    fclose(f);
    *n = len;
    return buf;
}

int main(int argc, char **argv)
{
    if (argc != 2)
    {
        fprintf(stderr, "usage: stamp CBRT_DIR\n");
        return 2;
    }
    char path[4096];
    snprintf(path, sizeof path, "%s/SOURCES", argv[1]);
    FILE *list = fopen(path, "rb");
    if (!list)
    {
        fprintf(stderr, "stamp: cannot read %s\n", path);
        return 1;
    }
    char line[1024];
    while (fgets(line, sizeof line, list))
    {
        size_t k = strlen(line);
        while (k > 0 && (line[k - 1] == '\n' || line[k - 1] == '\r' || line[k - 1] == ' '))
        {
            line[--k] = 0;
        }
        if (k == 0 || line[0] == '#')
        {
            continue;
        }
        snprintf(path, sizeof path, "%s/%s", argv[1], line);
        size_t n;
        unsigned char *bytes = slurp(path, &n);
        mix((const unsigned char *)line, k + 1);
        unsigned char le[8];
        for (int i = 0; i < 8; i++)
        {
            le[i] = (unsigned char)((uint64_t)n >> (8 * i));
        }
        mix(le, 8);
        if (bytes)
        {
            mix(bytes, n);
            free(bytes);
        }
    }
    fclose(list);
    printf("%016llx\n", (unsigned long long)h);
    return 0;
}
