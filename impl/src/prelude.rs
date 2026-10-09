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

// `std`'s files (D-0136, D-0187), embedded, so the tools need no files at
// run time: `std.cb` is the body of the root module `std`, and each
// `export module m "path";` declaration in it, or in any of the others,
// names one of the others (`crypto/digest.cb` is `std::crypto::digest`). A
// path is relative to `impl/std/`, as the language's rule relative to the
// declaring file gives for these files.
const STD_FILES: &[(&str, &str)] = &[
    ("core.cb", include_str!("../std/core.cb")),
    ("collections.cb", include_str!("../std/collections.cb")),
    ("text.cb", include_str!("../std/text.cb")),
    ("io.cb", include_str!("../std/io.cb")),
    ("fs.cb", include_str!("../std/fs.cb")),
    ("env.cb", include_str!("../std/env.cb")),
    ("memory.cb", include_str!("../std/memory.cb")),
    ("sync.cb", include_str!("../std/sync.cb")),
    ("random.cb", include_str!("../std/random.cb")),
    ("math.cb", include_str!("../std/math.cb")),
    ("time.cb", include_str!("../std/time.cb")),
    ("process.cb", include_str!("../std/process.cb")),
    ("net.cb", include_str!("../std/net.cb")),
    ("crypto.cb", include_str!("../std/crypto.cb")),
    ("crypto/digest.cb", include_str!("../std/crypto/digest.cb")),
    ("crypto/kdf.cb", include_str!("../std/crypto/kdf.cb")),
    ("crypto/aead.cb", include_str!("../std/crypto/aead.cb")),
    ("crypto/pk.cb", include_str!("../std/crypto/pk.cb")),
    ("x509.cb", include_str!("../std/x509.cb")),
    ("tls.cb", include_str!("../std/tls.cb")),
    ("http.cb", include_str!("../std/http.cb")),
    ("http/client.cb", include_str!("../std/http/client.cb")),
    ("http/server.cb", include_str!("../std/http/server.cb")),
    ("database.cb", include_str!("../std/database.cb")),
    ("database/postgres.cb", include_str!("../std/database/postgres.cb")),
    ("encoding.cb", include_str!("../std/encoding.cb")),
    ("encoding/json.cb", include_str!("../std/encoding/json.cb")),
    ("encoding/hex.cb", include_str!("../std/encoding/hex.cb")),
    ("encoding/base64.cb", include_str!("../std/encoding/base64.cb")),
    ("compress.cb", include_str!("../std/compress.cb")),
    ("extensions.cb", include_str!("../std/extensions.cb")),
];

/// The prelude's source: `export module std { … }`, each file-backed
/// submodule declaration of `std.cb` replaced by its file's items, as the
/// loader replaces a program's (D-0021). Assembled once.
pub fn source() -> &'static str {
    &assembled().0
}

/// Where a line of the prelude came from: `std`'s file and its line there
/// (1-based), for a diagnostic raised inside `std` itself, so a mistake in
/// `std`'s own source names its file and line instead of "unknown
/// location". `line` is in the combined text's line space.
pub fn std_location(line: usize) -> Option<(&'static str, usize)> {
    assembled().1.get(line.checked_sub(1)?).copied().flatten()
}

fn assembled() -> &'static (String, Vec<Option<(&'static str, usize)>>) {
    static SRC: std::sync::OnceLock<(String, Vec<Option<(&'static str, usize)>>)> = std::sync::OnceLock::new();
    SRC.get_or_init(|| {
        let mut origin: Vec<Option<(&'static str, usize)>> = vec![None, None, None];
        let mut out = String::from("\nexport module std\n{\n");
        splice(include_str!("../std/std.cb"), None, &mut out, &mut origin);
        out.push_str("}\n");
        origin.push(None);
        (out, origin)
    })
}

// Appends `text` (a `std` file's, from `file`, or `std.cb`'s own) to
// `out`, each `export module m "path";` line in it replaced by that file's
// items inside `export module m { … }`, recursively; `origin` gets one
// entry per line appended: the file and line it came from, or `None` for
// a line this assembly wrote.
fn splice(text: &'static str, file: Option<&'static str>, out: &mut String, origin: &mut Vec<Option<(&'static str, usize)>>) {
    for (i, line) in text.lines().enumerate() {
        let nested = line.strip_prefix("export module ").and_then(|r| r.split_once(' ')).and_then(|(name, rest)| {
            let path = rest.strip_prefix('"')?.strip_suffix("\";")?;
            STD_FILES.iter().find(|(f, _)| *f == path).map(|(f, inner)| (name, *f, *inner))
        });
        match nested {
            Some((name, path, inner)) => {
                out.push_str(&format!("export module {}\n{{\n", name));
                origin.push(None);
                origin.push(None);
                splice(inner, Some(path), out, origin);
                out.push_str("}\n");
                origin.push(None);
            }
            None => {
                out.push_str(line);
                out.push('\n');
                origin.push(file.map(|f| (f, i + 1)));
            }
        }
    }
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

/// Whether a line of the combined text lies inside the prelude: a
/// declaration there is `std`'s, which the static pass checks on demand
/// (`typecheck::check_program`, "reached items only").
pub fn is_prelude_line(line: usize) -> bool {
    line <= line_count()
}
