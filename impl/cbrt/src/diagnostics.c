/* Renders a `diag.*` id into the report a fault prints (`render`, `caret`,
   `source_line`, `unhide_binders`, `for_stderr`, `paint`, `stderr_color`,
   `explain`). The registry's fields come from diag_table.h, generated from
   spec/registry/diagnostics.md by tools/gen_diag_table.py, which parses it
   as cobc's `parse_registry` does (impl/cobc/diags.cb); an id without an
   entry is reported as a gap, never given invented prose. */

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "common.h"
#include "diagnostics.h"
#include "diag_table.h"
#include "io.h"
#include "plat.h"

/* ---- strings ---- */

static void put(cb_buf *b, const char *s)
{
    cb_buf_put(b, s, strlen(s));
}

static void putn(cb_buf *b, const char *s, size_t n)
{
    cb_buf_put(b, s, n);
}

static void putf(cb_buf *b, const char *fmt, ...) CB_PRINTF(2, 3);
static void putf(cb_buf *b, const char *fmt, ...)
{
    char tmp[64];
    va_list ap;
    va_start(ap, fmt);
    int k = vsnprintf(tmp, sizeof tmp, fmt, ap);
    va_end(ap);
    if (k < 0)
    {
        return;
    }
    if ((size_t)k < sizeof tmp)
    {
        putn(b, tmp, (size_t)k);
        return;
    }
    char *big = (char *)cb_xmalloc((size_t)k + 1);
    va_start(ap, fmt);
    vsnprintf(big, (size_t)k + 1, fmt, ap);
    va_end(ap);
    putn(b, big, (size_t)k);
    free(big);
}

static void rep(cb_buf *b, char c, size_t n)
{
    cb_buf_fill(b, (uint8_t)c, n);
}

static char *finish(cb_buf *b)
{
    cb_buf_byte(b, 0);
    return (char *)b->p;
}

/* The length of the UTF-8 sequence starting at p[i] when it is well formed
   (Rust's `from_utf8` rules: no overlong forms, no surrogates, at most
   U+10FFFF), else 0. */
static size_t utf8_seq(const uint8_t *p, size_t n, size_t i)
{
    uint8_t c = p[i];
    if (c < 0x80)
    {
        return 1;
    }
    size_t need;
    uint8_t lo = 0x80, hi = 0xbf;
    if (c >= 0xc2 && c <= 0xdf)
    {
        need = 1;
    }
    else if (c == 0xe0)
    {
        need = 2;
        lo = 0xa0;
    }
    else if ((c >= 0xe1 && c <= 0xec) || c == 0xee || c == 0xef)
    {
        need = 2;
    }
    else if (c == 0xed)
    {
        need = 2;
        hi = 0x9f;
    }
    else if (c == 0xf0)
    {
        need = 3;
        lo = 0x90;
    }
    else if (c >= 0xf1 && c <= 0xf3)
    {
        need = 3;
    }
    else if (c == 0xf4)
    {
        need = 3;
        hi = 0x8f;
    }
    else
    {
        return 0;
    }
    if (i + 1 >= n || p[i + 1] < lo || p[i + 1] > hi)
    {
        return 0;
    }
    for (size_t k = 2; k <= need; k++)
    {
        if (i + k >= n || p[i + k] < 0x80 || p[i + k] > 0xbf)
        {
            return 0;
        }
    }
    return need + 1;
}

/* How many bytes of an invalid sequence at p[i] one U+FFFD replaces: the
   maximal prefix of a well-formed sequence, at least one byte (Rust's
   `from_utf8_lossy`). */
static size_t utf8_bad_len(const uint8_t *p, size_t n, size_t i)
{
    uint8_t c = p[i];
    size_t need;
    uint8_t lo = 0x80, hi = 0xbf;
    if (c >= 0xc2 && c <= 0xdf)
    {
        need = 1;
    }
    else if (c == 0xe0)
    {
        need = 2;
        lo = 0xa0;
    }
    else if ((c >= 0xe1 && c <= 0xec) || c == 0xee || c == 0xef)
    {
        need = 2;
    }
    else if (c == 0xed)
    {
        need = 2;
        hi = 0x9f;
    }
    else if (c == 0xf0)
    {
        need = 3;
        lo = 0x90;
    }
    else if (c >= 0xf1 && c <= 0xf3)
    {
        need = 3;
    }
    else if (c == 0xf4)
    {
        need = 3;
        hi = 0x8f;
    }
    else
    {
        return 1;
    }
    size_t k = 1;
    if (i + 1 < n && p[i + 1] >= lo && p[i + 1] <= hi)
    {
        k = 2;
        while (k <= need && i + k < n && p[i + k] >= 0x80 && p[i + k] <= 0xbf)
        {
            k++;
        }
    }
    return k;
}

int diag_utf8_ok(const uint8_t *p, size_t n)
{
    size_t i = 0;
    while (i < n)
    {
        size_t k = utf8_seq(p, n, i);
        if (!k)
        {
            return 0;
        }
        i += k;
    }
    return 1;
}

char *diag_lossy(const uint8_t *p, size_t n)
{
    cb_buf b = {0};
    size_t i = 0;
    while (i < n)
    {
        size_t k = utf8_seq(p, n, i);
        if (k)
        {
            putn(&b, (const char *)p + i, k);
            i += k;
        }
        else
        {
            putn(&b, "\xef\xbf\xbd", 3);
            i += utf8_bad_len(p, n, i);
        }
    }
    return finish(&b);
}

/* ---- the registry ---- */

/* An entry's field, or NULL where the entry has none. */
static inline const char *dg(const uint16_t *e, int field)
{
    return e[field] ? DIAG_TEXT + e[field] : NULL;
}

static const uint16_t *lookup(const char *id)
{
    size_t lo = 0, hi = DIAG_TABLE_LEN;
    while (lo < hi)
    {
        size_t m = (lo + hi) / 2;
        int c = strcmp(DIAG_TEXT + DIAG_TABLE[m][DG_ID], id);
        if (c == 0)
        {
            return DIAG_TABLE[m];
        }
        if (c < 0)
        {
            lo = m + 1;
        }
        else
        {
            hi = m;
        }
    }
    return NULL;
}

/* Whether a diagnostic is shown with its full registry entry
   (`COBALTC_EXPLAIN` set to anything but empty or `0`). */
static int explain(void)
{
    static int done, on;
    if (!done)
    {
        const char *v = plat_getenv("COBALTC_EXPLAIN");
        on = v && v[0] && strcmp(v, "0") != 0;
        done = 1;
    }
    return on;
}

/* Whether diagnostics written to stderr are colored: only on a terminal,
   never when `NO_COLOR` is set (no-color.org), and on Windows only once
   the console accepts ANSI sequences. */
static int stderr_color(void)
{
    static int done, on;
    if (!done)
    {
        const char *v = plat_getenv("NO_COLOR");
        on = (!v || !v[0]) && plat_stderr_is_tty() && plat_enable_ansi();
        done = 1;
    }
    return on;
}

/* ---- the report ---- */

/* The text of line `line` (1-based) of `file`; NULL for a file that cannot
   be read (or `std`'s, which has no file of its own beside the program). */
static char *source_line(const char *file, size_t line)
{
    if (strncmp(file, "<std>/", 6) == 0 || line == 0)
    {
        return NULL;
    }
    cb_buf raw = {0};
    if (fileio_read((const uint8_t *)file, strlen(file), &raw) != 0)
    {
        cb_buf_free(&raw);
        return NULL;
    }
    char *text = diag_lossy(raw.p, raw.n);
    cb_buf_free(&raw);
    const char *s = text;
    for (size_t k = 1; k < line; k++)
    {
        const char *nl = strchr(s, '\n');
        if (!nl)
        {
            free(text);
            return NULL;
        }
        s = nl + 1;
    }
    const char *e = strchr(s, '\n');
    size_t n = e ? (size_t)(e - s) : strlen(s);
    if (n > 0 && s[n - 1] == '\r')
    {
        n--;
    }
    char *out = (char *)cb_xmalloc(n + 1);
    memcpy(out, s, n);
    out[n] = 0;
    free(text);
    return out;
}

static int is_cont(uint8_t c)
{
    return (c & 0xc0) == 0x80;
}

static size_t char_count(const char *s, size_t n)
{
    size_t c = 0;
    for (size_t i = 0; i < n; i++)
    {
        if (!is_cont((uint8_t)s[i]))
        {
            c++;
        }
    }
    return c;
}

static int word(uint8_t c)
{
    return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
}

/* Where on `src` the caret goes and how far it reaches, for the byte column
   `col` (1-based; 0 puts it at the line's first token): the excerpt's
   padding, the column as characters, the caret's length. */
static int caret(const char *src, size_t col, cb_buf *pad, size_t *chars, size_t *len)
{
    size_t n = strlen(src);
    size_t k;
    if (col == 0)
    {
        size_t i = 0;
        while (i < n && (src[i] == ' ' || src[i] == '\t'))
        {
            i++;
        }
        if (i == n)
        {
            return 0;
        }
        k = i;
    }
    else
    {
        k = col - 1;
    }
    if (k > n)
    {
        k = n;
    }
    while (k > 0 && k < n && is_cont((uint8_t)src[k]))
    {
        k--;
    }
    for (size_t i = 0; i < k; i++)
    {
        if (!is_cont((uint8_t)src[i]))
        {
            cb_buf_byte(pad, src[i] == '\t' ? '\t' : ' ');
        }
    }
    *chars = char_count(src, k) + 1;
    const char *rest = src + k;
    size_t rn = n - k;
    if (rn == 0)
    {
        *len = 1;
    }
    else if (word((uint8_t)rest[0]))
    {
        size_t i = 0;
        while (i < rn && word((uint8_t)rest[i]))
        {
            i++;
        }
        *len = i;
    }
    else if (rest[0] == '"')
    {
        size_t c = 1;
        int esc = 0;
        size_t i = 1;
        while (i < rn)
        {
            /* one character */
            size_t w = 1;
            while (i + w < rn && is_cont((uint8_t)rest[i + w]))
            {
                w++;
            }
            char ch = rest[i];
            c++;
            i += w;
            if (esc)
            {
                esc = 0;
            }
            else if (ch == '\\')
            {
                esc = 1;
            }
            else if (ch == '"')
            {
                break;
            }
        }
        *len = c;
    }
    else
    {
        *len = 1;
    }
    return 1;
}

/* D-0301: the hidden binder of `V(*x)` is `$box<n>_x`; a message names it
   as the `x` the program wrote. */
static void unhide_binders(cb_buf *out, const char *m)
{
    const char *rest = m;
    for (;;)
    {
        const char *k = strstr(rest, "$box");
        if (!k)
        {
            break;
        }
        putn(out, rest, (size_t)(k - rest));
        const char *after = k + 4;
        size_t digits = 0;
        while (after[digits] >= '0' && after[digits] <= '9')
        {
            digits++;
        }
        if (digits > 0 && after[digits] == '_')
        {
            rest = after + digits + 1;
        }
        else
        {
            put(out, "$box");
            rest = after;
        }
    }
    put(out, rest);
}

static size_t dec_len(size_t v)
{
    size_t n = 1;
    while (v >= 10)
    {
        v /= 10;
        n++;
    }
    return n;
}

char *diag_render(const char *id, const char *phase, const char *file, size_t line, size_t col, const char *msg)
{
    const uint16_t *info = lookup(id);
    int dynamic = strcmp(phase, "dynamic") == 0;
    const char *kind = dynamic ? "fault" : "error";
    cb_buf out = {0};
    putf(&out, "%s[%s]: ", kind, id);
    if (msg)
    {
        unhide_binders(&out, msg);
    }
    else if (info && dg(info, DG_SUMMARY))
    {
        put(&out, dg(info, DG_SUMMARY));
    }
    else
    {
        put(&out, "(no summary)");
    }
    put(&out, "\n");
    /* A dynamic diagnostic is located by its line alone, its caret under the
       line's first token (D-0228). */
    if (file && dynamic)
    {
        col = 0;
    }
    char *src = file ? source_line(file, line) : NULL;
    size_t width = file ? dec_len(line) : 1;
    if (file && src)
    {
        cb_buf pad = {0};
        size_t chars, len;
        if (caret(src, col, &pad, &chars, &len))
        {
            rep(&out, ' ', width);
            putf(&out, "--> %s:%llu:%llu\n", file, (unsigned long long)line, (unsigned long long)chars);
            rep(&out, ' ', width + 1);
            put(&out, "|\n");
            putf(&out, "%llu | %s\n", (unsigned long long)line, src);
            rep(&out, ' ', width + 1);
            put(&out, "| ");
            putn(&out, (const char *)pad.p, pad.n);
            rep(&out, '^', len);
            put(&out, "\n");
        }
        else
        {
            rep(&out, ' ', width);
            putf(&out, "--> %s:%llu\n", file, (unsigned long long)line);
        }
        cb_buf_free(&pad);
    }
    else if (file)
    {
        rep(&out, ' ', width);
        if (col == 0)
        {
            putf(&out, "--> %s:%llu\n", file, (unsigned long long)line);
        }
        else
        {
            putf(&out, "--> %s:%llu:%llu\n", file, (unsigned long long)line, (unsigned long long)col);
        }
    }
    else
    {
        rep(&out, ' ', width);
        put(&out, "--> unknown location\n");
    }
    free(src);
    if (!info)
    {
        rep(&out, ' ', width + 1);
        if (strncmp(id, "unsupported", 11) == 0)
        {
            /* A limit of this implementation, not a diagnostic of the
               language: no registry entry is due (session finding 001-C7). */
            put(&out, "= note: a limit of this tool, not a fault of the program; the other implementation may run it\n");
            return finish(&out);
        }
        putf(&out, "= note: no entry for `%s` in spec/registry/diagnostics.md -- a gap in the registry or in this tool's diag id, not a fabricated explanation\n", id);
        return finish(&out);
    }
    if (dg(info, DG_REPAIR))
    {
        rep(&out, ' ', width + 1);
        putf(&out, "= repair: %s\n", dg(info, DG_REPAIR));
    }
    if (!explain())
    {
        return finish(&out);
    }
    put(&out, "\n");
    if (dg(info, DG_RULE))
    {
        putf(&out, "  rule:      %s\n", dg(info, DG_RULE));
    }
    if (dg(info, DG_INVARIANT))
    {
        putf(&out, "  invariant: %s\n", dg(info, DG_INVARIANT));
    }
    if (dg(info, DG_REQUIRED))
    {
        putf(&out, "  required:  %s\n", dg(info, DG_REQUIRED));
    }
    if (dg(info, DG_OBSERVED))
    {
        putf(&out, "  observed:  %s\n", dg(info, DG_OBSERVED));
    }
    if (dg(info, DG_PROVENANCE))
    {
        putf(&out, "  provenance: %s\n", dg(info, DG_PROVENANCE));
    }
    if (dg(info, DG_RELATED))
    {
        putf(&out, "\n  see: spec/examples.md -> %s\n", dg(info, DG_RELATED));
        put(&out, "       spec/registry/diagnostics.md\n");
    }
    else
    {
        put(&out, "\n  see: spec/registry/diagnostics.md\n");
    }
    return finish(&out);
}

/* ---- color ---- */

#define RESET "\x1b[0m"

static int starts(const char *s, size_t n, const char *p)
{
    size_t k = strlen(p);
    return n >= k && memcmp(s, p, k) == 0;
}

/* Colors a rendered diagnostic: `error`/`fault` bold red and the rest of
   its first line bold, the `-->` location and the source excerpt's gutter
   bold blue, the caret bold red, `= repair:` bold cyan. */
static char *paint(const char *out)
{
    cb_buf s = {0};
    int have_width = 0;
    size_t w = 0;
    const char *line = out;
    size_t i = 0;
    while (*line)
    {
        const char *nl = strchr(line, '\n');
        size_t n = nl ? (size_t)(nl - line) : strlen(line);
        const char *body = line;
        size_t lead = 0;
        while (lead < n && body[lead] == ' ')
        {
            lead++;
        }
        if (i == 0 && (starts(body, n, "error[") || starts(body, n, "fault[")))
        {
            put(&s, "\x1b[1;31m");
            putn(&s, body, 5);
            put(&s, RESET "\x1b[1m");
            putn(&s, body + 5, n - 5);
            put(&s, RESET);
        }
        else if (!have_width && starts(body + lead, n - lead, "--> "))
        {
            have_width = 1;
            w = lead;
            putn(&s, body, lead);
            put(&s, "\x1b[1;34m-->" RESET);
            putn(&s, body + lead + 3, n - lead - 3);
        }
        else if (have_width)
        {
            size_t g = w + 1;
            int gut_spaces = n >= g;
            for (size_t k = 0; gut_spaces && k < g; k++)
            {
                if (body[k] != ' ')
                {
                    gut_spaces = 0;
                }
            }
            if (gut_spaces && n == g + 1 && body[g] == '|')
            {
                put(&s, "\x1b[1;34m");
                putn(&s, body, n);
                put(&s, RESET);
            }
            else if (gut_spaces && starts(body + g, n - g, "= repair: "))
            {
                putn(&s, body, g);
                put(&s, "\x1b[1;36m= repair:" RESET " ");
                putn(&s, body + g + 10, n - g - 10);
            }
            else if (gut_spaces && starts(body + g, n - g, "| "))
            {
                const char *rest = body + g + 2;
                size_t rn = n - g - 2;
                size_t carets = 0;
                while (carets < rn && rest[rn - 1 - carets] == '^')
                {
                    carets++;
                }
                put(&s, "\x1b[1;34m");
                putn(&s, body, g);
                put(&s, "|" RESET " ");
                putn(&s, rest, rn - carets);
                put(&s, "\x1b[1;31m");
                putn(&s, rest + rn - carets, carets);
                put(&s, RESET);
            }
            else
            {
                int digits = n > w + 2;
                for (size_t k = 0; digits && k < w; k++)
                {
                    if (body[k] < '0' || body[k] > '9')
                    {
                        digits = 0;
                    }
                }
                if (digits && starts(body + w, n - w, " | "))
                {
                    put(&s, "\x1b[1;34m");
                    putn(&s, body, w + 2);
                    put(&s, RESET);
                    putn(&s, body + w + 2, n - w - 2);
                }
                else
                {
                    putn(&s, body, n);
                }
            }
        }
        else
        {
            putn(&s, body, n);
        }
        if (nl)
        {
            put(&s, "\n");
            line = nl + 1;
        }
        else
        {
            line += n;
        }
        i++;
    }
    return finish(&s);
}

char *diag_for_stderr(const char *out)
{
    if (stderr_color())
    {
        return paint(out);
    }
    size_t n = strlen(out);
    char *c = (char *)cb_xmalloc(n + 1);
    memcpy(c, out, n + 1);
    return c;
}
