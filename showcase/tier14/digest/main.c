/* A C program using the CobaltC library beside it: `digest.h` is what
   `cobc --lib` wrote, and the link line is its first line. The library's
   pointer parameters are `uint8_t *`, so C's text is cast to pass. */
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <pthread.h>
#include "digest.h"

#define BYTES(s) ((uint8_t *)(s)), (strlen(s))

static void show(const char *label, struct Digest d)
{
    char hex[65];
    digest_hex(d, (uint8_t *)hex, sizeof hex);
    printf("%-8s %s\n", label, hex);
}

/* Four threads hashing at once: the library keeps nothing between calls,
   and registers each C thread on its first call. */
static void *worker(void *arg)
{
    const char *text = arg;
    struct Digest first = digest_sha256(BYTES(text));
    for (int i = 0; i < 500; i++) {
        if (!digest_equal(digest_sha256(BYTES(text)), first)) return "differs";
    }
    return NULL;
}

int main(void)
{
    struct Digest abc = digest_sha256(BYTES("abc"));
    show("sha256", abc);
    show("empty", digest_sha256(BYTES("")));
    /* RFC 4231, test case 2. */
    show("hmac", digest_hmac(BYTES("Jefe"), BYTES("what do ya want for nothing?")));
    printf("crc32    %08x\n", digest_crc32(BYTES("The quick brown fox jumps over the lazy dog")));

    char b64[40];
    size_t n = digest_base64(BYTES("Many hands make light work."), (uint8_t *)b64, sizeof b64);
    printf("base64   %s (%zu)\n", b64, n);

    printf("equal    %d %d\n", digest_equal(abc, digest_sha256(BYTES("abc"))), digest_equal(abc, digest_sha256(BYTES("abd"))));
    printf("check    %d %d %d\n",
           digest_check_hex(abc, (uint8_t *)"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
           digest_check_hex(abc, (uint8_t *)"0000000000000000000000000000000000000000000000000000000000000000"),
           digest_check_hex(abc, (uint8_t *)"not a digest"));

    /* A buffer too small for 64 digits and a NUL: in C that is an overrun;
       here it is a fault at the library's assert, and the _try form
       contains it: 1 back, the result untouched, the report kept. */
    char small[16];
    size_t written = 99;
    int rc = digest_hex_try(abc, (uint8_t *)small, sizeof small, &written);
    char report[400];
    int len = cobalt_last_fault(report, sizeof report);
    /* The report's first line names the fault; the rest shows the source. */
    printf("try      rc=%d written=%zu report=%d\n  %.*s\n", rc, written, len > 0, (int)strcspn(report, "\n"), report);
    /* The library is still usable after a contained fault. */
    show("after", abc);

    pthread_t t[4];
    const char *texts[4] = { "one", "two", "three", "four" };
    for (int i = 0; i < 4; i++) pthread_create(&t[i], NULL, worker, (void *)texts[i]);
    const char *failed = NULL;
    for (int i = 0; i < 4; i++) {
        void *r;
        pthread_join(t[i], &r);
        if (r) failed = r;
    }
    printf("threads  %s\n", failed ? failed : "agree");

    cobalt_shutdown();
    printf("done\n");
    return 0;
}
