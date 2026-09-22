// File-backed module declarations (spec/17 §5, `rule.module.file`).
//
// `[export] module m "p";` is replaced, before the program is parsed,
// by `[export] module m {` + the named file's own (recursively
// expanded) text + `}` -- literally the equivalence `[Module-File]`
// states. A textual splice rather than a separate parse, for the reason
// `lib.rs` already splices the prelude ahead of the program: the
// parser's declaration-statement pre-scan (spec/22 §2 rule 4,
// `Parser::new`) collects `struct`/`enum` names from the whole token
// stream, so `Rect r = ...` in the entry file only parses as a
// declaration if the file declaring `Rect` is in that same stream.
//
// The three static rejections (`[Module-File-Missing]`,
// `[Module-File-Cycle]`, `[Module-File-Duplicate]`) are raised here,
// tagged with the offending declaration's line in the expanded text --
// the same `"diag.xxx@N"` encoding every other pass uses. A `SourceMap`
// maps each expanded line back to (file, line), so a diagnostic inside
// a loaded file names that file rather than a line in the splice.

use crate::lexer::{Lexer, Spanned, Tok};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct SourceMap {
    lines: Vec<(Arc<str>, usize)>,
}

impl SourceMap {
    /// `line` is 1-based in the expanded program text.
    pub fn resolve(&self, line: usize) -> Option<(&str, usize)> {
        self.lines.get(line.checked_sub(1)?).map(|(f, l)| (f.as_ref(), *l))
    }
}

pub struct Expansion {
    pub text: String,
    pub map: SourceMap,
    /// `"diag.xxx@N"`, N a line in `text`, when a declaration could not be expanded.
    pub error: Option<String>,
}

/// `origin` is the entry file's own path: paths in it resolve against
/// its directory, and it counts as already loaded, so a file naming it
/// closes a cycle. `None` (a program given as a string) resolves
/// against the working directory.
pub fn expand(src: &str, origin: Option<&Path>) -> Expansion {
    let (name, dir): (Arc<str>, PathBuf) = match origin {
        Some(p) => (p.to_string_lossy().as_ref().into(), parent_dir(p)),
        None => ("<input>".into(), PathBuf::from(".")),
    };
    let mut ctx = Ctx { stack: Vec::new(), seen: HashSet::new(), out: Out::default() };
    if let Some(canon) = origin.and_then(|p| fs::canonicalize(p).ok()) {
        ctx.seen.insert(canon.clone());
        ctx.stack.push(canon);
    }
    let error = ctx.expand_text(src, &name, &dir).err();
    Expansion { text: ctx.out.text, map: SourceMap { lines: ctx.out.map }, error }
}

fn parent_dir(p: &Path) -> PathBuf {
    match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

#[derive(Default)]
struct Out {
    text: String,
    map: Vec<(Arc<str>, usize)>,
    // Origin of the expanded line being written, until its newline.
    cur: Option<(Arc<str>, usize)>,
}

impl Out {
    /// Appends `s`, whose first character sits at `line` of `name`.
    fn write(&mut self, s: &str, name: &Arc<str>, mut line: usize) {
        for c in s.chars() {
            if self.cur.is_none() {
                self.cur = Some((name.clone(), line));
            }
            self.text.push(c);
            if c == '\n' {
                self.map.push(self.cur.take().expect("set above"));
                line += 1;
            }
        }
    }

    fn end_line(&mut self) {
        if let Some(origin) = self.cur.take() {
            self.text.push('\n');
            self.map.push(origin);
        }
    }
}

enum LoadErr {
    Here(&'static str),
    Nested(String),
}

struct Ctx {
    stack: Vec<PathBuf>,
    seen: HashSet<PathBuf>,
    out: Out,
}

impl Ctx {
    fn expand_text(&mut self, text: &str, name: &Arc<str>, dir: &Path) -> Result<(), String> {
        let toks = match Lexer::new(text).tokenize() {
            Ok(t) => t,
            // The parser reports lexical errors; pass the text through untouched.
            Err(_) => {
                self.out.write(text, name, 1);
                self.out.end_line();
                return Ok(());
            }
        };
        let line_starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
        let offset = |t: &Spanned<Tok>| line_starts[t.line - 1] + t.col - 1;

        let (mut cursor, mut cursor_line) = (0usize, 1usize);
        let mut i = 0;
        while i < toks.len() {
            let j = if matches!(toks[i].tok, Tok::Export) { i + 1 } else { i };
            let decl = match toks.get(j..j + 4) {
                Some([m, id, p, semi]) if matches!(m.tok, Tok::Module) && matches!(semi.tok, Tok::Semi) => match (&id.tok, &p.tok) {
                    (Tok::Ident(ident), Tok::Str(path)) => Some((ident.clone(), path.clone(), semi)),
                    _ => None,
                },
                _ => None,
            };
            let Some((ident, path, semi)) = decl else {
                i += 1;
                continue;
            };
            self.out.write(&text[cursor..offset(&toks[i])], name, cursor_line);
            let export = if j > i { "export " } else { "" };
            self.out.write(&format!("{}module {} {{\n", export, ident), name, toks[i].line);
            let decl_line = self.out.map.len();
            self.load(&path, dir).map_err(|e| match e {
                LoadErr::Here(diag) => format!("{}@{}", diag, decl_line),
                LoadErr::Nested(tagged) => tagged,
            })?;
            self.out.end_line();
            self.out.write("}", name, semi.line);
            cursor = offset(semi) + 1;
            cursor_line = semi.line;
            i = j + 4;
        }
        self.out.write(&text[cursor..], name, cursor_line);
        self.out.end_line();
        Ok(())
    }

    fn load(&mut self, path: &str, dir: &Path) -> Result<(), LoadErr> {
        let rel = path.strip_prefix("./").unwrap_or(path);
        let joined = if Path::new(rel).is_absolute() || dir == Path::new(".") { PathBuf::from(rel) } else { dir.join(rel) };
        let canon = fs::canonicalize(&joined)
            .ok()
            .filter(|c| c.is_file())
            .ok_or(LoadErr::Here("diag.module-file-not-found"))?;
        if self.stack.contains(&canon) {
            return Err(LoadErr::Here("diag.module-cycle"));
        }
        if !self.seen.insert(canon.clone()) {
            return Err(LoadErr::Here("diag.module-file-duplicate"));
        }
        let text = fs::read_to_string(&canon).map_err(|_| LoadErr::Here("diag.module-file-not-found"))?;
        let name: Arc<str> = joined.to_string_lossy().as_ref().into();
        self.stack.push(canon);
        let r = self.expand_text(&text, &name, &parent_dir(&joined)).map_err(LoadErr::Nested);
        self.stack.pop();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{resolve_user_location, run_program_phased_located, OutcomeLocated};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("coby-loader-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn static_location(entry: &Path) -> (String, Option<(String, usize)>) {
        let src = fs::read_to_string(entry).unwrap();
        let (outcome, map) = run_program_phased_located(&src, Some(entry));
        let OutcomeLocated::Static(raw) = outcome else { panic!("expected a static rejection, got {:?}", outcome) };
        resolve_user_location(&raw, &map)
    }

    #[test]
    fn a_program_without_file_modules_is_passed_through_verbatim() {
        let src = "fn main()\n{\n    auto x = 1;\n}\n";
        let e = expand(src, None);
        assert!(e.error.is_none());
        assert_eq!(e.text, src);
        assert_eq!(e.map.resolve(3), Some(("<input>", 3)));
    }

    #[test]
    fn a_diagnostic_inside_a_loaded_file_names_that_file_and_its_own_line() {
        let dir = scratch("inner-line");
        fs::write(dir.join("lib.cb"), "// a comment\nexport fn f() : i32\n{\n    2147483647:i32 + 1:i32\n}\n").unwrap();
        let entry = dir.join("main.cb");
        fs::write(&entry, "module lib \"./lib.cb\";\n\nfn main()\n{\n    auto x = lib::f();\n}\n").unwrap();
        let (id, at) = static_location(&entry);
        assert_eq!(id, "diag.arith-overflow");
        let (file, line) = at.expect("located");
        assert!(file.ends_with("lib.cb"), "{}", file);
        assert_eq!(line, 4, "line 4 of lib.cb is the overflowing expression");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn entry_file_lines_after_a_splice_still_map_to_the_entry_file() {
        let dir = scratch("after-splice");
        fs::write(dir.join("lib.cb"), "export fn f() : i32\n{\n    1\n}\n").unwrap();
        let entry = dir.join("main.cb");
        fs::write(&entry, "module lib \"./lib.cb\";\n\nfn main()\n{\n    auto x = 2147483647:i32 + 1:i32;\n}\n").unwrap();
        let (id, at) = static_location(&entry);
        assert_eq!(id, "diag.arith-overflow");
        let (file, line) = at.expect("located");
        assert!(file.ends_with("main.cb"), "{}", file);
        assert_eq!(line, 5, "the splice must not shift the entry file's own line numbers");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_is_reported_at_the_declaring_line() {
        let dir = scratch("missing");
        let entry = dir.join("main.cb");
        fs::write(&entry, "fn main()\n{\n}\n\nmodule m \"./absent.cb\";\n").unwrap();
        let (id, at) = static_location(&entry);
        assert_eq!(id, "diag.module-file-not-found");
        let (file, line) = at.expect("located");
        assert!(file.ends_with("main.cb"), "{}", file);
        assert_eq!(line, 5);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_naming_the_entry_file_is_a_cycle() {
        let dir = scratch("entry-cycle");
        fs::write(dir.join("lib.cb"), "module back \"./main.cb\";\n").unwrap();
        let entry = dir.join("main.cb");
        fs::write(&entry, "module lib \"./lib.cb\";\nfn main()\n{\n}\n").unwrap();
        let (id, at) = static_location(&entry);
        assert_eq!(id, "diag.module-cycle");
        let (file, line) = at.expect("located");
        assert!(file.ends_with("lib.cb"), "the closing declaration is in lib.cb, got {}", file);
        assert_eq!(line, 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
