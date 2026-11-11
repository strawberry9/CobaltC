#!/usr/bin/env bash
# Compares cbparse (impl/selfhost/front, coby's front end written in
# CobaltC) with coby's, byte for byte, on every `.cb` file git tracks:
#
#   dump     `--dump-ast FILE`: the program's own part of the parse, or the error
#   resolved `--dump-resolved FILE`: that part once names are resolved, or the
#            diagnostic with its message
#   folded   `--dump-folded FILE`: that part once the constants are folded and
#            the text literals viewed too, what the type checker reads
#   checked  `--dump-checked FILE`: `ok` once the whole static pass accepts
#            the program, or the diagnostic with its message
#   syntax   `--check-syntax FILE`, against `cobc --check-syntax`
#   mutants  seven mutated copies of each file (mutate.py), on standard
#            input with `--origin FILE`: almost all are errors, so the two
#            parsers' messages and positions are compared
#
# and that impl/selfhost/front/prelude.cb is what `coby --prelude-summary`
# gives now. cbparse is impl/target/selfhost/cbparse (impl/selfhost/build.sh).
#
#   impl/selfhost/compare_parser.sh [-j JOBS] [-m MODE,...] [FILE...]
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
coby="$root/impl/target/release/coby"
cobc="$root/impl/target/release/cobc"
cb="${CBPARSE:-$root/impl/target/selfhost/cbparse}"
jobs=3
modes="dump,resolved,folded,syntax,mutants"
while getopts "j:m:" o; do
    case $o in
        j) jobs=$OPTARG ;;
        m) modes=$OPTARG ;;
        *) exit 2 ;;
    esac
done
shift $((OPTIND - 1))
for t in "$coby" "$cobc" "$cb"; do
    [ -x "$t" ] || { echo "compare_parser.sh: $t is missing; build it first" >&2; exit 2; }
done
OUT=${OUT:-$root/stress/selfhost/parser-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$OUT/run"
ulimit -t 600
if [ $# -gt 0 ]; then
    printf '%s\n' "$@" > "$OUT/files"
else
    (cd "$root" && git ls-files '*.cb') > "$OUT/files"
fi
: > "$OUT/results"
if "$coby" --prelude-summary | cmp -s - "$here/front/prelude.cb"; then
    echo "same prelude-summary" >> "$OUT/results"
else
    echo "DIFF prelude-summary (regenerate: coby --prelude-summary > impl/selfhost/front/prelude.cb)" >> "$OUT/results"
fi

# Both tools on one input; `$1` names the case, the rest are the arguments
# (a `<` FILE as the second-to-last pair feeds standard input).
both() {
    local tag=$1 a b
    shift
    tag=$(printf '%s' "$tag" | tr '/' '_')
    a="$OUT/run/$tag.coby" b="$OUT/run/$tag.cb"
    local tool_a=$1 input=$2
    shift 2
    if [ -n "$input" ]; then
        (cd "$root" && timeout -k 5 600 "$tool_a" "$@" < "$input" > "$a" 2>&1; echo "exit $?" >> "$a")
        (cd "$root" && timeout -k 5 600 "$cb" "$@" < "$input" > "$b" 2>&1; echo "exit $?" >> "$b")
    else
        (cd "$root" && timeout -k 5 600 "$tool_a" "$@" > "$a" 2>&1; echo "exit $?" >> "$a")
        (cd "$root" && timeout -k 5 600 "$cb" "$@" > "$b" 2>&1; echo "exit $?" >> "$b")
    fi
    # A message about reading the input names its tool, by design.
    sed -i -E 's/^(coby|cobc): /TOOL: /' "$a"
    sed -i -E 's/^cbparse: /TOOL: /' "$b"
    if cmp -s "$a" "$b"; then
        rm -f "$a" "$b"
        echo "same $tag"
    else
        echo "DIFF $tag"
    fi
}

one() {
    local mode=$1 f=$2
    case $mode in
        dump)   both "dump:$f" "$coby" "" --dump-ast "$f" ;;
        resolved) both "resolved:$f" "$coby" "" --dump-resolved "$f" ;;
        folded) both "folded:$f" "$coby" "" --dump-folded "$f" ;;
        checked) both "checked:$f" "$coby" "" --dump-checked "$f" ;;
        syntax) both "syntax:$f" "$cobc" "" --check-syntax "$f" ;;
        mutants)
            local dir
            dir=$(mktemp -d "$OUT/run/m.XXXXXX")
            python3 -I "$here/mutate.py" "$root/$f" "$dir" > "$dir/list"
            local n=0
            while read -r m; do
                both "mutant$n:$f" "$coby" "$m" --dump-ast --origin "$f" -
                n=$((n + 1))
            done < "$dir/list"
            rm -rf "$dir"
            ;;
    esac
}
export -f one both
export OUT root coby cobc cb here
for m in ${modes//,/ }; do
    sed "s|^|$m |" "$OUT/files"
done | xargs -P "$jobs" -L 1 bash -c 'one "$0" "$1"' >> "$OUT/results"

same=$(grep -c '^same' "$OUT/results")
diffs=$(grep -c '^DIFF' "$OUT/results")
echo "compare_parser.sh: $same same, $diffs different (results in $OUT)"
[ "$diffs" -eq 0 ] && echo PASSED > "$OUT/STATUS" && exit 0
grep '^DIFF' "$OUT/results" | head -20
echo FAILED > "$OUT/STATUS"
exit 1
