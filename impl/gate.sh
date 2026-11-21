#!/usr/bin/env bash
# The gate (D-0307): every check the project runs, as units with declared
# inputs, on a snapshot of the tree; a unit's green verdict is kept under
# the hash of its inputs and never run again on the same inputs.
#
#   impl/gate.sh                  # the gate profile on HEAD: what a commit and a push need
#   impl/gate.sh release          # the gate and the release profile's slow units (the owner says when)
#   impl/gate.sh all              # every unit of both profiles, the kept verdicts ignored
#   impl/gate.sh UNIT...          # those units only
#   impl/gate.sh --dirty ...      # the working tree as it is, not HEAD
#   impl/gate.sh --list [gate]    # the units and whether a green verdict is kept for this tree
#
# impl/tools/gate.py has the unit table and says where the snapshot, the
# store and the runs are.
exec python3 "$(cd "$(dirname "$0")" && pwd)/tools/gate.py" "$@"
