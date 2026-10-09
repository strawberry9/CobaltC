#!/usr/bin/env bash
# Runs every showcase under both implementations -- `cobc` once per C
# compiler, GCC and Clang by default -- and compares each run's standard
# output (and exit status) with the checked-in expectation (`<name>.out`
# beside `<name>.cb`). Exits non-zero if anything differs.
#
#   ./run_all.sh                  # uses ../impl/target/release/{coby,cobc}
#   COBY=... COBC=... ./run_all.sh
#   COBC_CCS="gcc" ./run_all.sh   # the C compilers to build with (default: gcc clang)
#   ./run_all.sh --regenerate     # rewrite the .out files from coby (review the diff!)
#
# A showcase with a `<name>.args` file beside it is a command-line tool:
# it runs once per line of that file (blank lines and `#` lines are
# skipped; `(none)` is a run with no arguments), with the line's words
# as its arguments (shell quoting), and
# its expectation is each run's `$ name args` line, output and exit
# status, in order. `$TMP` in a line is a new, empty directory for that
# run (removed afterwards), for a tool that writes files; the `$ name
# args` line shows it as written. A line may end with `< FILE` (Tier 8):
# that run's standard input reads FILE, relative to the showcase's
# directory; without it, standard input is empty. A line may begin with
# NAME=value words (Tier 9): that run's environment variables, as in a
# shell (`$ NAME=value name args` in the expectation).
set -u
# No program may run away: every process this script starts gets at most
# LIMIT seconds of CPU (`ulimit -t`, inherited by what it starts), a
# compiled program the same through `cobc` (COBALTC_CPU_LIMIT), and each
# run is stopped after LIMIT seconds of wall time with everything it
# started (`timeout` signals the whole process group).
LIMIT="${LIMIT:-600}"
ulimit -t "$((LIMIT + 60))"
export COBALTC_CPU_LIMIT="$LIMIT"
here="$(cd "$(dirname "$0")" && pwd)"
COBY="${COBY:-$here/../impl/target/release/coby}"
COBC="${COBC:-$here/../impl/target/release/cobc}"
COBC_CCS="${COBC_CCS:-gcc clang}"
regen=0
[ "${1:-}" = "--regenerate" ] && regen=1

fail=0
pass=0

# One showcase's runs through `$@` (the tool and its options): once, or
# once per line of its `.args` file.
runs() {
    local f="$1"; shift
    local dir base argsfile line
    dir="$(dirname "$f")"; base="$(basename "$f")"; argsfile="${f%.cb}.args"
    if [ ! -f "$argsfile" ]; then
        (cd "$dir" && timeout -k 10 "$LIMIT" "$@" "$base" 2>&1; echo "[exit $?]")
        return
    fi
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in ''|'#'*) continue ;; esac
        [ "$line" = "(none)" ] && line=""
        # Leading NAME=value words (Tier 9) are the run's environment, as
        # in a shell: a value is a plain word, "quoted", or $'escaped'.
        envs=""
        while [[ "$line" =~ ^([A-Za-z_][A-Za-z0-9_]*=([^ \"\']*|\"[^\"]*\"|\$\'[^\']*\'))( +(.*))?$ ]]; do
            envs="$envs ${BASH_REMATCH[1]}"
            line="${BASH_REMATCH[4]}"
        done
        envs="${envs# }"
        echo "\$ ${envs:+$envs }${base%.cb}${line:+ $line}"
        # "$@" (the tool) and $base are expanded by eval, so only the
        # line's own words are split and unquoted.
        TMP="$(mktemp -d)"
        (cd "$dir" && export TMP && eval "$envs timeout -k 10 \"\$LIMIT\" \"\$@\" \"\$base\" $line" 2>&1 </dev/null; echo "[exit $?]")
        rm -rf "$TMP"
    done < "$argsfile"
}

for f in "$here"/tier*/*.cb; do
    name="${f#$here/}"
    expected="${f%.cb}.out"
    a="$(runs "$f" "$COBY")"
    if [ $regen = 1 ]; then
        printf '%s\n' "$a" > "$expected"
    fi
    want="$(cat "$expected" 2>/dev/null)"
    bad=""
    [ "$a" != "$want" ] && bad="$bad coby"
    for cc in $COBC_CCS; do
        b="$(runs "$f" "$COBC" --cc "$cc" --run)"
        [ "$b" != "$want" ] && bad="$bad cobc/$cc"
    done
    if [ -z "$bad" ]; then
        pass=$((pass + 1))
        echo "ok    $name"
    else
        fail=$((fail + 1))
        echo "FAIL  $name (${bad# })"
    fi
done
echo
echo "$pass showcases identical under coby and cobc ($COBC_CCS), $fail failures"
[ $fail = 0 ]
