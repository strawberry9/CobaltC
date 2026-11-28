#!/usr/bin/env bash
# D-0307: the targeted suites behind the "tested" evidence tier are the
# gate's profile now (impl/gate.sh), which runs them on a snapshot of the
# tree and keeps each green verdict under the hash of its inputs.
exec "$(cd "$(dirname "$0")/.." && pwd)/gate.sh" "$@"
