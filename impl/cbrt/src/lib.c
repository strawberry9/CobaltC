/* cbrt in C: the runtime's core, one translation unit (D-0202): the parts
   are included here. */

#include "rt.h"

#include "rt_alloc.c"
#include "rt_tables.c"
#include "rt_lock.c"
#include "rt_init.c"
#include "rt_threads.c"
#include "replay.c"
#include "rt_objs.c"
#include "rt_refs.c"
