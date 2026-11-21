/* Declarations of rt_init.c (see rt.h): start, faults, library mode,
   the standard library's externs, the schedule. */

#ifndef CBRT_RT_INIT_H
#define CBRT_RT_INIT_H

extern __thread bool SCHED_PENDING;

void cbrt_internal(const char *msg) __attribute__((noreturn, cold));
void set_heap(Rt *r, uint8_t h);
uint8_t claim_heap(Rt *r);
char *render_fault(const char *id, uint32_t file, uint32_t line);
/* `id` is moved into the failure's record. */
void contained_fault(char *id, uint32_t file, uint32_t line, uint64_t me) CB_NORET;
void lib_contained_fault(const char *id, uint32_t file, uint32_t line, uint64_t me, uintptr_t jb) CB_NORET;
int cancel_probe(void);

struct SchedSt;
struct SchedSt *sched_lock(void);
/* Takes the schedule's lock as held, and gives it up. */
void sched_wait(uint64_t me, struct SchedSt *g);

#endif
