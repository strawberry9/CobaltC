#!/usr/bin/env bash
# Compares cobfmt-cb (impl/selfhost/cobfmt, cobfmt written in CobaltC) with
# the Rust cobfmt, byte for byte: standard output, standard error and the
# exit status, on every `.cb` file git tracks, in five ways:
#
#   plain    --stdout FILE                (the files as they are: formatted)
#   nowrap   --no-wrap --stdout FILE
#   narrow   --width 40 --stdout FILE     (breaking lists, far more often)
#   flat     every line's indentation removed, on standard input
#   messy    tabs for indentation, spaces doubled, CRLF line ends, on standard input
#
# and on the options and errors of the command line. Both formatters must
# exist: impl/target/release/cobfmt (cargo build --release -p cobfmt) and
# impl/target/selfhost/cobfmt-cb (impl/selfhost/build.sh; `COBFMT_CB` names
# another). Run from
# anywhere; a mismatch is listed and kept in OUT (default a fresh
# directory under stress/selfhost/), and the exit status is 1.
#
#   impl/selfhost/compare.sh [-j JOBS] [-m MODE,...] [FILE...]
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
rust="$root/impl/target/release/cobfmt"
cb="${COBFMT_CB:-$root/impl/target/selfhost/cobfmt-cb}"
jobs=3
modes="plain,nowrap,narrow,flat,messy"
while getopts "j:m:" o; do
    case $o in
        j) jobs=$OPTARG ;;
        m) modes=$OPTARG ;;
        *) exit 2 ;;
    esac
done
shift $((OPTIND - 1))
for t in "$rust" "$cb"; do
    [ -x "$t" ] || { echo "compare.sh: $t is missing; build it first" >&2; exit 2; }
done
OUT=${OUT:-$root/stress/selfhost/compare-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$OUT"
ulimit -t 600
if [ $# -gt 0 ]; then
    printf '%s\n' "$@" > "$OUT/files"
else
    (cd "$root" && git ls-files '*.cb') > "$OUT/files"
fi

# One file in one mode: both tools, everything they print, compared.
one() {
    local mode=$1 f=$2 tag
    tag=$(printf '%s' "$mode:$f" | tr '/' '_')
    local a="$OUT/run/$tag.rust" b="$OUT/run/$tag.cb" input=""
    case $mode in
        plain)  args=(--stdout "$f") ;;
        nowrap) args=(--no-wrap --stdout "$f") ;;
        narrow) args=(--width 40 --stdout "$f") ;;
        flat)   args=(-); input=$(mktemp "$OUT/run/in.XXXXXX"); sed 's/^[ \t]*//' "$f" > "$input" ;;
        messy)  args=(-); input=$(mktemp "$OUT/run/in.XXXXXX")
                sed -e 's/^    /\t/' -e 's/ = / =  /g' -e 's/, /,  /g' -e 's/$/\r/' "$f" > "$input" ;;
    esac
    if [ -n "$input" ]; then
        timeout -k 5 120 "$rust" "${args[@]}" < "$input" > "$a" 2>&1; echo "exit $?" >> "$a"
        timeout -k 5 120 "$cb" "${args[@]}" < "$input" > "$b" 2>&1; echo "exit $?" >> "$b"
        rm -f "$input"
    else
        (cd "$root" && timeout -k 5 120 "$rust" "${args[@]}" > "$a" 2>&1; echo "exit $?" >> "$a")
        (cd "$root" && timeout -k 5 120 "$cb" "${args[@]}" > "$b" 2>&1; echo "exit $?" >> "$b")
    fi
    if cmp -s "$a" "$b"; then
        rm -f "$a" "$b"
        echo "same $mode $f"
    else
        echo "DIFF $mode $f"
    fi
}
export -f one
export OUT root rust cb
mkdir -p "$OUT/run"
for m in ${modes//,/ }; do
    sed "s|^|$m |" "$OUT/files"
done | xargs -P "$jobs" -L 1 bash -c 'one "$0" "$1"' > "$OUT/results"

# The command line: options, usage errors, missing files, --check, --diff.
cli() {
    local name=$1; shift
    local a="$OUT/run/cli-$name.rust" b="$OUT/run/cli-$name.cb"
    (cd "$root" && timeout -k 5 120 "$rust" "$@" > "$a" 2>&1; echo "exit $?" >> "$a")
    (cd "$root" && timeout -k 5 120 "$cb" "$@" > "$b" 2>&1; echo "exit $?" >> "$b")
    # The usage text and the program's own name differ by design: with
    # no arguments both print their usage, and only the status is compared.
    sed -i 's/cobfmt-cb/cobfmt/g' "$b"
    if [ $# -eq 0 ]; then
        tail -1 "$a" > "$a.st" && mv "$a.st" "$a"
        tail -1 "$b" > "$b.st" && mv "$b.st" "$b"
    fi
    if cmp -s "$a" "$b"; then rm -f "$a" "$b"; echo "same cli $name"; else echo "DIFF cli $name"; fi
}
scratch="$OUT/cli"
mkdir -p "$scratch"
printf 'fn main()\n{\nx=1;\n}\n' > "$scratch/messy.cb"
printf 'fn main()\n{\n    x = 1;\n}\n' > "$scratch/tidy.cb"
printf 'fn main()\n{\n    x = ;\n}\n' > "$scratch/broken.cb"
printf '\xff\xfe\n' > "$scratch/binary.cb"
{
    cli unknown --frobnicate x.cb
    cli nofile
    cli width-missing --width
    cli width-small --width 5 x.cb
    cli width-eq --width=abc x.cb
    cli check-diff --check --diff x.cb
    cli stdout-check --stdout --check x.cb
    cli stdin-two - x.cb
    cli missing stress/selfhost/no-such-file.cb
    cli check "${scratch#$root/}/messy.cb" "${scratch#$root/}/tidy.cb" --check
    cli diff --diff "${scratch#$root/}/messy.cb"
    cli broken "${scratch#$root/}/broken.cb"
    cli binary "${scratch#$root/}/binary.cb"
    cli dir --check "${scratch#$root/}"
} >> "$OUT/results"
# Writing in place, with a backup: the files and their backups compared.
for t in rust cb; do
    mkdir -p "$scratch/w-$t"
    cp "$scratch/messy.cb" "$scratch/tidy.cb" "$scratch/w-$t/"
    # A file with permissions of its own keeps them (D-0272).
    chmod 750 "$scratch/w-$t/messy.cb"
done
(cd "$scratch/w-rust" && "$rust" --backup . > ../w-rust.log 2>&1; echo "exit $?" >> ../w-rust.log)
(cd "$scratch/w-cb" && "$cb" --backup . > ../w-cb.log 2>&1; echo "exit $?" >> ../w-cb.log)
modes() { (cd "$1" && for f in *; do printf '%s %s\n' "$f" "$(stat -c %a "$f")"; done); }
if cmp -s "$scratch/w-rust.log" "$scratch/w-cb.log" && diff -r "$scratch/w-rust" "$scratch/w-cb" > /dev/null \
    && [ "$(modes "$scratch/w-rust")" = "$(modes "$scratch/w-cb")" ] && [ "$(stat -c %a "$scratch/w-cb/messy.cb")" = 750 ]; then
    echo "same cli write-backup" >> "$OUT/results"
else
    echo "DIFF cli write-backup" >> "$OUT/results"
fi

same=$(grep -c '^same' "$OUT/results")
diffs=$(grep -c '^DIFF' "$OUT/results")
echo "compare.sh: $same same, $diffs different (results in $OUT)"
[ "$diffs" -eq 0 ] && echo PASSED > "$OUT/STATUS" && exit 0
grep '^DIFF' "$OUT/results" | head -20
echo FAILED > "$OUT/STATUS"
exit 1
