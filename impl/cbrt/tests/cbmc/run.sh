#!/usr/bin/env bash
# Runs the CBMC harnesses (harness.c), each at its unwind bound. Logs to
# results.txt.
set -u
cd "$(dirname "$0")"
ulimit -t 1800
out=results.txt
: > "$out"
fail=0
run() {
    local fn="$1" unwind="$2"
    local t0=$(date +%s)
    if timeout -k 10 1800 cbmc harness.c --function "$fn" --unwind "$unwind" --unwinding-assertions --bounds-check --pointer-check --signed-overflow-check --unsigned-overflow-check --div-by-zero-check -D__CPROVER__ > "log_$fn.txt" 2>&1; then
        r=PASS
    else
        r=FAIL; fail=1
    fi
    echo "$r $fn (unwind $unwind, $(( $(date +%s) - t0 )) s): $(grep -E '^\*\* [0-9]+ of [0-9]+ failed|VERIFICATION' "log_$fn.txt" | tr '\n' ' ')" | tee -a "$out"
}
run arena_ids_are_stable_until_removed 5
run heapset_add_has_each 6
run heaps_places_are_filled_before_read 10
run norm_of_agrees_with_norm_iter 5
run norm_spill_agrees_with_norm_iter 16
run ranged_overlap_is_symmetric_and_reflexive 5
exit $fail
