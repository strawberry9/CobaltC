// The CobaltC prelude: the standard library `std`, transcribed from
// spec/21-standard-library-semantics.md into the CobaltC files of
// `impl/std/` (D-0136: one file per submodule, `std.cb` the root).
// Every intrinsic named there without a body (drop, sizeof, allocate,
// rawptr_of, reclaim, spawn, join, str_len, str_byte, str_ptr, ...) is
// recognized natively by `Interp::try_intrinsic`, per CHG-0023: a
// body-less prelude entry is never a `Σ.items`/parsed-item entry. Every
// `Vec`/`String`/`Rc` function IS real parsed CobaltC source, run through
// the same parser/evaluator as any user function, exactly as the spec
// directs.

// `std`'s files (D-0136), embedded, so the tools need no files at run
// time: `std.cb` is the body of the root module `std`, and each of its
// `export module m "m.cb";` declarations names one of the others.
const STD_FILES: &[(&str, &str)] = &[
    ("core.cb", include_str!("../std/core.cb")),
    ("collections.cb", include_str!("../std/collections.cb")),
    ("text.cb", include_str!("../std/text.cb")),
    ("io.cb", include_str!("../std/io.cb")),
    ("sys.cb", include_str!("../std/sys.cb")),
    ("memory.cb", include_str!("../std/memory.cb")),
    ("sync.cb", include_str!("../std/sync.cb")),
    ("random.cb", include_str!("../std/random.cb")),
    ("math.cb", include_str!("../std/math.cb")),
];

/// The prelude's source: `export module std { … }`, each file-backed
/// submodule declaration of `std.cb` replaced by its file's items, as the
/// loader replaces a program's (D-0021). Assembled once.
pub fn source() -> &'static str {
    static SRC: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SRC.get_or_init(|| {
        let mut out = String::from("\nexport module std\n{\n");
        for line in include_str!("../std/std.cb").lines() {
            let file = line.strip_prefix("export module ").and_then(|r| r.split_once(' ')).and_then(|(name, rest)| {
                let path = rest.strip_prefix('"')?.strip_suffix("\";")?;
                STD_FILES.iter().find(|(f, _)| *f == path).map(|(_, text)| (name, *text))
            });
            match file {
                Some((name, text)) => {
                    out.push_str(&format!("export module {}\n{{\n", name));
                    out.push_str(text);
                    out.push_str("}\n");
                }
                None => {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        out.push_str("}\n");
        out
    })
}

/// The number of source lines `source()` itself occupies inside the
/// combined `"{prelude}\n{user src}"` text every parse/typecheck/eval
/// pass actually runs on (`lib.rs`'s `build_interp`/
/// `check_source_static_raw` -- one token stream, deliberately, so the
/// disambiguation prescan sees the prelude's own struct/enum names).
/// Every `Expr::line`/fault-tagged line number is a line number *in
/// that combined text*, not in the user's own file, until this offset
/// is subtracted back out -- exactly what `main.rs` does before
/// rendering a diagnostic, so "at line N" means N in the file the user
/// actually wrote, never a line inside invisible prelude source they
/// never see.
pub fn line_count() -> usize {
    // Counting '\n' characters (not `.lines().count()`) so this exactly
    // mirrors `build_interp`'s actual join, `format!("{}\n{}", source(),
    // src)`, character for character: the prelude's own content already
    // ends in `\n`, and the join adds one *more* `\n` as a separator --
    // together that's one real blank line between the prelude and the
    // user's own source, which `.lines().count()` alone (283, not 284)
    // would silently omit, undercounting the offset by exactly one line.
    // Computed once: `cobc` asks for it at every location it emits.
    static COUNT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *COUNT.get_or_init(|| source().matches('\n').count() + 1)
}
