// D-0315, `spec/14` §6 "The cost contract": a decided program whose values
// all end in the program's own code keeps no cleanup bookkeeping. For each
// program here `--report-tracking` must say that the program is decided and
// list no line: no frame, no statement scope and no object is written in its
// generated C. The programs are the conformance cases of the forms D-0315
// follows (their outcomes are the conformance suite's) and the benchmarks.

mod oracle;

use std::path::PathBuf;
use std::process::Command;

const PROGRAMS: &[&str] = &[
    "impl/conformance/07-resource-authority/drop_plain_local_ok.cb",
    "impl/conformance/11-initialization/destructure_memory_only_ok.cb",
    "impl/conformance/14-control-flow/foreach_by_value_memory_only_ok.cb",
    "impl/conformance/14-control-flow/match_whole_arm_memory_only_ok.cb",
    "impl/conformance/14-control-flow/stmt_temp_short_circuit_ok.cb",
    "impl/conformance/18-error-failure-semantics/propagate_memory_only_ok.cb",
    "impl/conformance/21-standard-library-semantics/map_insert_text_keys_ok.cb",
    "bench/bench.cb",
    "bench/collatz.cb",
    "bench/collatz_wrapping.cb",
    "bench/hashcount.cb",
    "bench/records.cb",
    "bench/sieve.cb",
    "bench/sieve_fn.cb",
    "bench/strings.cb",
    "bench/tree.cb",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn decided_programs_keep_no_cleanup_bookkeeping() {
    let mut bad = Vec::new();
    for p in PROGRAMS {
        let path = root().join(p);
        let dir = path.parent().unwrap().to_path_buf();
        let out = Command::new(oracle::cobc_exe())
            .arg("--report-tracking")
            .arg(path.file_name().unwrap())
            .current_dir(&dir)
            .output()
            .expect("run cobc");
        assert!(out.status.success(), "{}: {}", p, String::from_utf8_lossy(&out.stderr));
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let decided = text.lines().next().map_or(false, |l| l.starts_with("no run-time tracking"));
        let listed: Vec<&str> = text.lines().filter(|l| l.contains(".cb:")).collect();
        if !decided || !listed.is_empty() {
            bad.push(format!("{}: decided {}, {} line(s) listed{}", p, decided, listed.len(), listed.first().map(|l| format!(", first {}", l.trim())).unwrap_or_default()));
        }
    }
    assert!(bad.is_empty(), "cleanup bookkeeping left:\n{}", bad.join("\n"));
}
