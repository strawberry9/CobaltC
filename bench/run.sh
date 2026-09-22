#!/usr/bin/env bash
# Builds each workload with `cobc` (once per C compiler, GCC and Clang by
# default), with `rustc -O`, and with `rustc -O` and overflow checks on;
# checks that every build prints the same answer; runs each RUNS times;
# and reports the median wall-clock time with the range (fastest to
# slowest). Where `perf` can count them, it also reports the instructions
# each program executes and its branch mispredictions, figures that barely
# vary from run to run.
#
# The results are printed and written to RESULTS.md, the one place they
# are kept: other documents refer to it rather than copying numbers.
#
#   ./run.sh                         # uses ../impl/target/release/cobc and rustc
#   RUNS=15 ./run.sh                 # more runs per program (default 9)
#   COBC_CCS="gcc" ./run.sh          # C compilers for cobc (default: gcc clang)
#   COBC=... RUSTC=... PERF=... RESULTS=... ./run.sh
#
# Build `cobc` first, with its runtime (`cd impl && cargo build --release
# -p cbrt -p cobc`). Executables go to ./build/, which git ignores.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
COBC="${COBC:-$here/../impl/target/release/cobc}"
RUSTC="${RUSTC:-rustc}"
RUNS="${RUNS:-9}"
COBC_CCS="${COBC_CCS:-gcc clang}"
RESULTS="${RESULTS:-$here/RESULTS.md}"
build="$here/build"
mkdir -p "$build"
workloads="bench sieve sieve_fn collatz collatz_wrapping"

# A `perf` that can count user-space events here, or nothing. The `perf`
# wrapper may not match the running kernel, so a versioned binary is tried
# too. Counting needs kernel.perf_event_paranoid <= 2 and working hardware
# counters: in a VM without virtualized counters, `perf` exits successfully
# but reports `<not supported>` or 0, so a real count is required.
find_perf() {
    local c n
    for c in ${PERF:-} perf /usr/lib/linux-tools/*/perf; do
        [ -n "$c" ] || continue
        n="$("$c" stat -x, -e instructions:u true 2>&1 > /dev/null | awk -F, '/instructions/ { print $1 }')" || true
        if [[ "$n" =~ ^[0-9]+$ ]] && [ "$n" -gt 0 ]; then
            echo "$c"
            return
        fi
    done
}
perf_bin="$(find_perf)"

# Median, fastest and slowest of RUNS wall-clock times, in seconds.
times_of() {
    local t0 t1
    for _ in $(seq "$RUNS"); do
        t0=$(date +%s.%N)
        "$1" > /dev/null
        t1=$(date +%s.%N)
        awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.3f\n", b - a }'
    done | sort -n | awk '{ t[NR] = $1 } END {
        m = (NR % 2) ? t[(NR + 1) / 2] : (t[NR / 2] + t[NR / 2 + 1]) / 2
        printf "%.3f %.3f %.3f", m, t[1], t[NR] }'
}

# User-space instructions and branch mispredictions of one run, with
# thousands separators; `-` for each when perf cannot count.
counts_of() {
    if [ -z "$perf_bin" ]; then
        echo "- -"
        return
    fi
    "$perf_bin" stat -x, -e instructions:u,branch-misses:u "$1" 2>&1 > /dev/null \
        | awk -F, 'function sep(n,   s) { s = ""
                       while (length(n) > 3) { s = "," substr(n, length(n) - 2) s; n = substr(n, 1, length(n) - 3) }
                       return n s }
                   $3 ~ /^instructions/ && $1 ~ /^[0-9]+$/ { i = sep($1) }
                   $3 ~ /^branch-misses/ && $1 ~ /^[0-9]+$/ { b = sep($1) }
                   END { printf "%s %s", (i == "" ? "-" : i), (b == "" ? "-" : b) }'
}

builds=""
for cc in $COBC_CCS; do
    builds="$builds cobc-$cc"
done
builds="$builds rust rust-checked"

label() {
    case "$1" in
        cobc-*) echo "\`cobc\` (${1#cobc-})" ;;
        rust) echo "Rust" ;;
        rust-checked) echo "Rust, overflow checks" ;;
    esac
}

cd "$here"
declare -A TIME RANGE INSNS MISSES
echo "$RUNS runs per program; times in seconds: median (fastest - slowest)"
if [ -z "$perf_bin" ]; then
    echo "instructions, branch misses: not counted (no working hardware counters for perf here; see README)"
fi
echo
printf '%-17s %-14s %7s %17s %16s %13s\n' workload build median range instructions branch-misses
for w in $workloads; do
    for cc in $COBC_CCS; do
        "$COBC" --cc "$cc" -o "$build/$w.cobc-$cc" "$w.cb"
    done
    "$RUSTC" -O -o "$build/$w.rust" "$w.rs"
    "$RUSTC" -O -C overflow-checks=on -o "$build/$w.rust-checked" "$w.rs"
    want="$("$build/$w.rust")"
    for v in $builds; do
        if [ "$("$build/$w.$v")" != "$want" ]; then
            echo "error: $w.$v prints something other than $w.rust" >&2
            exit 1
        fi
    done
    for v in $builds; do
        read -r med lo hi <<< "$(times_of "$build/$w.$v")"
        read -r ins mis <<< "$(counts_of "$build/$w.$v")"
        TIME[$w,$v]="$med"
        RANGE[$w,$v]="$lo–$hi"
        INSNS[$w,$v]="$ins"
        MISSES[$w,$v]="$mis"
        printf '%-17s %-14s %7s %17s %16s %13s\n' "$w" "$v" "$med" "($lo - $hi)" "$ins" "$mis"
    done
done

# ---- RESULTS.md ----
table() {
    local what="$1" w v row
    local header="| Workload |" rule="|---|"
    for v in $builds; do
        header="$header $(label "$v") |"
        rule="$rule---|"
    done
    echo "$header"
    echo "$rule"
    for w in $workloads; do
        row="| \`$w\` |"
        for v in $builds; do
            case "$what" in
                time) row="$row ${TIME[$w,$v]} s (${RANGE[$w,$v]}) |" ;;
                insns) row="$row ${INSNS[$w,$v]} |" ;;
                misses) row="$row ${MISSES[$w,$v]} |" ;;
            esac
        done
        echo "$row"
    done
}

cpu="$(grep -m1 'model name' /proc/cpuinfo | sed 's/.*: *//; s/  */ /g')"
virt="$(systemd-detect-virt 2>/dev/null || true)"
os="$(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME")"
ccs=""
for cc in $COBC_CCS; do
    ccs="$ccs; \`$cc\`: $("$cc" --version | head -1)"
done
case "$virt" in
    vmware) virt=VMware ;; kvm) virt=KVM ;; qemu) virt=QEMU ;; oracle) virt=VirtualBox ;;
    microsoft) virt=Hyper-V ;; xen) virt=Xen ;; wsl) virt=WSL ;;
esac
where="$cpu, $(nproc) cores"
if [ -n "$virt" ] && [ "$virt" != none ]; then
    where="$where, in a $virt virtual machine"
fi

{
    echo "# Benchmark results"
    echo
    echo "Written by \`bench/run.sh\` on $(date +%Y-%m-%d); rerun it rather than editing"
    echo "this file. \`bench/README.md\` explains the workloads, the method and how to"
    echo "read these numbers."
    echo
    echo "- **Machine:** $where, $os, Linux $(uname -r)"
    echo "- **C compilers for \`cobc\`:** ${ccs#; }"
    echo "- **Rust:** $("$RUSTC" --version)"
    echo "- **Runs:** $RUNS per program. Times are the median, with the fastest and slowest in brackets."
    echo
    echo "## Time"
    echo
    table time
    echo
    echo "## Instructions executed"
    echo
    if [ -n "$perf_bin" ]; then
        echo "User space, counted by \`perf\`."
        echo
        table insns
        echo
        echo "## Branch mispredictions"
        echo
        echo "User space, counted by \`perf\`."
        echo
        table misses
    else
        echo "Not counted: \`perf\` could not count on this machine (see \`bench/README.md\`)."
    fi
} > "$RESULTS"
echo
echo "wrote $RESULTS"
