#!/usr/bin/env bash
# Checks the book: every example program under examples/ runs under both
# implementations -- `cobc` once per C compiler, GCC and Clang by default --
# and its output (standard output and standard error together, then the
# exit status) matches the checked-in expectation `<name>.out` beside it;
# every example's text appears verbatim in a chapter under src/; and every
# example is formatted as `cobfmt` would format it. Exits non-zero if
# anything differs.
#
#   ./check.sh                    # uses ../impl/target/release/{coby,cobc,cobfmt}
#   COBY=... COBC=... ./check.sh
#   COBC_CCS="gcc" ./check.sh     # the C compilers to build with (default: gcc clang)
#   ./check.sh --regenerate       # rewrite the .out files from coby (review the diff!)
#
# An example with a `<name>.args` file beside it is a command-line tool:
# it runs once per line of that file (blank lines and `#` lines are
# skipped; `(none)` is a run with no arguments), with the line's words as
# its arguments, and its expectation is each run's `$ name args` line,
# output and exit status, in order. `$TMP` in a line is an empty directory
# made for that `.args` file and shared by its runs, so a tool that keeps
# a file between runs (chapter 15) sees its own earlier work; it is removed
# afterwards. A line may end
# with `< FILE`: that run's standard input reads FILE, relative to the
# example's directory; without it, standard input is empty. An example
# with a `<name>.in` file and no `.args` file reads that file as its
# standard input. A `<name>.opts` file holds options for the tool itself,
# placed before the file name (`--unittest`, say). A `<name>.coby` file
# marks an example whose output is checked under `coby` only, because the
# text it prints names the tool (`--explore`'s report). An example with a directory `<name>/` beside it has its
# module files there (chapter 12).
#
# The conventions are the showcase runner's (showcase/run_all.sh), so a
# reader who knows one knows the other.
set -u
LIMIT="${LIMIT:-300}"
ulimit -t "$((LIMIT + 60))"
export COBALTC_CPU_LIMIT="$LIMIT"
here="$(cd "$(dirname "$0")" && pwd)"
COBY="${COBY:-$here/../impl/target/release/coby}"
COBC="${COBC:-$here/../impl/target/release/cobc}"
COBFMT="${COBFMT:-$here/../impl/target/release/cobfmt}"
COBC_CCS="${COBC_CCS:-gcc clang}"
regen=0
[ "${1:-}" = "--regenerate" ] && regen=1

fail=0
pass=0

runs() {
    local f="$1"; shift
    local dir base argsfile infile line
    dir="$(dirname "$f")"; base="$(basename "$f")"; argsfile="${f%.cb}.args"; infile="${f%.cb}.in"
    local opts=""
    [ -f "${f%.cb}.opts" ] && opts="$(cat "${f%.cb}.opts")"
    if [ ! -f "$argsfile" ]; then
        if [ -f "$infile" ]; then
            (cd "$dir" && timeout -k 10 "$LIMIT" "$@" $opts "$base" < "$infile" 2>&1; echo "[exit $?]")
        else
            (cd "$dir" && timeout -k 10 "$LIMIT" "$@" $opts "$base" < /dev/null 2>&1; echo "[exit $?]")
        fi
        return
    fi
    TMP="$(mktemp -d)"
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in ''|'#'*) continue ;; esac
        [ "$line" = "(none)" ] && line=""
        echo "\$ ${base%.cb}${line:+ $line}"
        (cd "$dir" && export TMP && eval "timeout -k 10 \"\$LIMIT\" \"\$@\" $opts \"\$base\" $line" 2>&1 </dev/null; echo "[exit $?]")
    done < "$argsfile"
    rm -rf "$TMP"
}

for f in "$here"/examples/ch*/*.cb; do
    name="${f#$here/}"
    expected="${f%.cb}.out"
    a="$(runs "$f" "$COBY")"
    if [ $regen = 1 ]; then
        printf '%s\n' "$a" > "$expected"
        echo "wrote $name"
        continue
    fi
    if [ ! -f "$expected" ]; then
        echo "FAIL no expectation: $expected"; fail=$((fail + 1)); continue
    fi
    e="$(cat "$expected")"
    if [ "$a" != "$e" ]; then
        echo "DIFF coby $name"; diff <(printf '%s\n' "$e") <(printf '%s\n' "$a") | head -n 20; fail=$((fail + 1)); continue
    fi
    ok=1
    if [ -f "${f%.cb}.coby" ]; then pass=$((pass + 1)); continue; fi   # its output names the tool
    for cc in $COBC_CCS; do
        b="$(runs "$f" "$COBC" --cc "$cc" --run)"
        if [ "$a" != "$b" ]; then
            echo "DIFF cobc/$cc $name"; diff <(printf '%s\n' "$a") <(printf '%s\n' "$b") | head -n 20; fail=$((fail + 1)); ok=0; break
        fi
    done
    [ $ok = 1 ] && pass=$((pass + 1))
done
[ $regen = 1 ] && exit 0

# Every example's text is in a chapter, verbatim.
if ! python3 -I "$here/embedded.py" "$here"; then fail=$((fail + 1)); fi

# Every example is formatted.
if [ -x "$COBFMT" ]; then
    unformatted="$("$COBFMT" --check "$here/examples" 2>&1)"
    if [ -n "$unformatted" ]; then echo "FAIL cobfmt would change:"; echo "$unformatted"; fail=$((fail + 1)); fi
fi

echo "book: $pass examples, identical under coby and cobc ($COBC_CCS); $fail problems"
[ $fail = 0 ]
