#!/usr/bin/env bash
# D-0307: the battery is the gate's release profile now (impl/gate.sh):
# the gate's units and the slow ones (the self-hosted fixed point, the
# whole-corpus formatter and parser comparisons, --all-checks, the debug
# profile, the `slow:` cases, the verify modes, the stress programs and
# sanitizers, the fuzzer, the external servers). The former stages are in
# impl/tools/battery_stages.sh.
exec "$(cd "$(dirname "$0")" && pwd)/gate.sh" release "$@"
