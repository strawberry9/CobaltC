/* A fault's report: the registry entry of a `diag.*` id rendered in the
   short form (D-0228), its full form under `COBALTC_EXPLAIN`, colored for a
   terminal. The registry is the generated table diag_table.h
   (tools/gen_diag_table.py). Every text here is UTF-8; results are malloc'd
   and NUL-terminated. */

#ifndef CBRT_DIAGNOSTICS_H
#define CBRT_DIAGNOSTICS_H

#include <stddef.h>
#include <stdint.h>

/* Separates a diagnostic's id from a message the program supplied
   (D-0052): "diag.x\x01message". */
#define DETAIL_SEP '\x01'

/* `render(id, phase_actual, at, message)`: `file` NULL for no location;
   `msg` NULL for none (the registry's summary is used). */
char *diag_render(const char *id, const char *phase, const char *file, size_t line, size_t col, const char *msg);
/* `for_stderr`: colored when stderr is a terminal and NO_COLOR is unset. */
char *diag_for_stderr(const char *out);
/* `String::from_utf8_lossy` of `n` bytes, NUL-terminated. */
char *diag_lossy(const uint8_t *p, size_t n);
/* Whether `n` bytes are valid UTF-8 (`str::from_utf8(..).is_ok()`). */
int diag_utf8_ok(const uint8_t *p, size_t n);

#endif
