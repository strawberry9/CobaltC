// The element read of an array constant whose elements are literals,
// `K[i]` and `K[i][j]`: integers, `bool`s, texts, floats and arrays of
// them are read from C static data laid out once (`const_array_read`), not
// from the array the constant's call builds whole at every read (`TYPE_NAMES[i]`
// in the self-hosted lexer: 384 bytes per read). The emitted C reads each
// constant from `k_` and never calls it; the program's output and the bounds
// fault are the same under gcc and clang.

#[path = "oracle/exe.rs"]
mod exe;

use std::process::Command;

const ALL: &str = "import std;\n\nconst array<str, 4> NAMES = [\"i32\", \"u8\", \"bool\", \"\"];\nconst array<f64, 3> SCALE = [0.5, 1.25, 1e10];\nconst array<f32, 2> HALF = [0.5, 2.0];\nconst array<array<u8, 3>, 2> GRID = [[1, 2, 3], [4, 5, 6]];\nconst array<array<str, 2>, 2> PAIRS = [[\"a\", \"bc\"], [\"def\", \"ghij\"]];\nconst array<u32, 4> TABLE = [10, 20, 30, 40];\nconst array<bool, 3> FLAGS = [true, false, true];\n\nfn known(StringView w) : bool\n{\n    for (usize i = 0; i < 4; i += 1)\n    {\n        if (w == NAMES[i])\n        {\n            return true;\n        }\n    }\n    false\n}\n\nfn main() : u8\n{\n    usize total = 0;\n    for (usize i = 0; i < 4; i += 1)\n    {\n        total += str_len(NAMES[i]);\n    }\n    printf(\"%v %v %v %v\\n\", total, known(\"bool\"), known(\"u16\"), known(\"\"));\n    f64 s = 0.0;\n    for (usize i = 0; i < 3; i += 1)\n    {\n        s += SCALE[i];\n    }\n    printf(\"%v %v\\n\", s, HALF[1] * HALF[0]);\n    u32 g = 0;\n    for (usize r = 0; r < 2; r += 1)\n    {\n        for (usize c = 0; c < 3; c += 1)\n        {\n            g = g * 10 + widen<u32>(GRID[r][c]);\n        }\n    }\n    array<u8, 3> row = GRID[1];\n    printf(\"%v %v\\n\", g, row[2]);\n    printf(\"%s %s %v\\n\", PAIRS[1][0], (PAIRS[0])[1], str_len(PAIRS[1][1]));\n    printf(\"%v %v %v\\n\", TABLE[3], FLAGS[1], (NAMES)[2]);\n    usize k = arg_count();\n    printf(\"%s\\n\", NAMES[k + 3]);\n    printf(\"%s\\n\", PAIRS[1][k + 1]);\n    0\n}\n";

const OOB: &str = "import std;\n\nconst array<array<str, 2>, 2> PAIRS = [[\"a\", \"bc\"], [\"def\", \"ghij\"]];\n\nfn main() : u8\n{\n    usize k = arg_count();\n    printf(\"%s\\n\", PAIRS[1][k + 1]);\n    0\n}\n";

#[test]
fn constant_arrays_of_literals_are_read_from_static_data() {
    let dir = std::env::temp_dir().join(format!("cobc-const-array-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let case = dir.join("all.cb");
    std::fs::write(&case, ALL).unwrap();
    let c = dir.join("all.c");
    let out = Command::new(exe::cobc_exe()).arg("--emit-c").arg("-o").arg(&c).arg(&case).output().expect("run cobc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&c).unwrap();
    for k in ["NAMES", "SCALE", "HALF", "GRID", "PAIRS", "TABLE", "FLAGS"] {
        assert!(!text.contains(&format!("f_{}();", k)), "{} is built at a read", k);
    }
    for decl in ["static const cb_str k_[4]", "static const double k_[3]", "static const float k_[2]", "static const struct a_arr3_u8 k_[2]", "static const struct a_arr2_str k_[2]", "static const uint32_t k_[4]", "static const uint8_t k_[3]"] {
        assert!(text.contains(decl), "no `{}`", decl);
    }
    let want = "9 true false true\n10000000001.75 1.0\n123456 6\ndef bc 4\n40 false bool\n\nghij\n";
    let oob = dir.join("oob.cb");
    std::fs::write(&oob, OOB).unwrap();
    for cc in ["gcc", "clang"] {
        let exe = dir.join(format!("all_{}", cc));
        let out = Command::new(exe::cobc_exe()).args(["--cc", cc, "-o"]).arg(&exe).arg(&case).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        let run = Command::new(&exe).output().expect("run the program");
        assert_eq!(String::from_utf8_lossy(&run.stdout), want, "{}", cc);
        let exe = dir.join(format!("oob_{}", cc));
        let out = Command::new(exe::cobc_exe()).args(["--cc", cc, "-o"]).arg(&exe).arg(&oob).output().expect("run cobc");
        assert!(out.status.success(), "{}: {}", cc, String::from_utf8_lossy(&out.stderr));
        let run = Command::new(&exe).output().expect("run the program");
        assert_eq!(String::from_utf8_lossy(&run.stdout), "ghij\n", "{}", cc);
        let run = Command::new(&exe).arg("x").output().expect("run the program");
        let err = String::from_utf8_lossy(&run.stderr);
        assert!(err.starts_with("fault[diag.index-out-of-bounds]"), "{}: expected the fault; stderr {:?}", cc, err);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
