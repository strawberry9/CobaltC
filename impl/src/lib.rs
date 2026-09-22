pub mod ast;
pub mod consts;
pub mod diagnostics;
pub mod each;
pub mod ffi;
pub mod interp;
pub mod lexer;
pub mod loader;
pub mod modres;
pub mod fileio;
pub mod fmt;
pub mod numtext;
pub mod parser;
pub mod prelude;
pub mod typecheck;
pub mod value;

use std::collections::HashMap;
use std::path::Path;

pub fn build_items(program: &ast::Program, out: &mut interp::Items) {
    // Keys mirror `modres::qualify` exactly (root items keep their bare
    // name, unchanged from before nested-module support existed; a nested
    // item's key is its full `spec/17` §1 qualified path) -- `modres`'s
    // resolution pass rewrites every *reference* to this same scheme
    // before this function ever runs, so declaration and use agree.
    fn walk(items: &[ast::Item], path: &mut Vec<String>, out: &mut interp::Items) {
        for it in items {
            match it {
                ast::Item::Fn(f) => {
                    let simple = match &f.assoc_type {
                        Some(t) => format!("{}::{}", t, f.name),
                        None => f.name.clone(),
                    };
                    out.fns.insert(modres::qualify(path, &simple), f.clone());
                }
                ast::Item::Extern(e) => {
                    out.externs.insert(modres::qualify(path, &e.name), e.clone());
                }
                ast::Item::Struct(s) => {
                    out.structs.insert(modres::qualify(path, &s.name), s.clone());
                }
                ast::Item::Enum(e) => {
                    let qk = modres::qualify(path, &e.name);
                    for v in &e.variants {
                        out.enum_of_variant.insert(v.name.clone(), qk.clone());
                    }
                    out.enums.insert(qk, e.clone());
                }
                ast::Item::Module(_, name, items, _) => {
                    path.push(name.clone());
                    walk(items, path, out);
                    path.pop();
                }
                ast::Item::Import(..) => {}
                ast::Item::ExternCode(code, line) => out.extern_code.push((code.clone(), *line)),
            }
        }
    }
    walk(&program.items, &mut Vec::new(), out);
}

pub fn new_items() -> interp::Items {
    interp::Items {
        fns: HashMap::new(),
        externs: HashMap::new(),
        structs: HashMap::new(),
        enums: HashMap::new(),
        enum_of_variant: HashMap::new(),
        extern_code: Vec::new(),
        match_hints: std::sync::Mutex::new(HashMap::new()),
        view_ops: std::sync::Mutex::new(HashMap::new()),
    }
}

/// Parses and loads the prelude followed by `src`, returning a ready
/// interpreter positioned to run `fn main()`. File-backed module
/// declarations in `src` (`loader`) resolve against the working
/// directory.
pub fn build_interp(src: &str) -> Result<interp::Interp, String> {
    build_interp_expanded(&expand_or_reject(src)?)
}

/// `expanded` has every file-backed module declaration already
/// replaced by its file's items (`loader::expand`).
fn build_interp_expanded(expanded: &str) -> Result<interp::Interp, String> {
    Ok(interp::Interp::new(std::sync::Arc::new(build_items_checked(expanded)?)))
}

/// The whole front end over an expanded program: parse, resolve,
/// build the item table, and run the static pass over it. The item
/// table returned is the one every consumer must use — the typing
/// facts the static pass records (`Items::match_hints`) refer to its
/// own expression nodes.
/// `rule.module.resolve` `[Item-Duplicate]` (`CHG-0032`): every
/// qualified name `spec/17` §1 adds -- a `fn`, `extern fn`, `struct`,
/// `enum` or `module` as `M::name`, an associated function as
/// `M::T::name`, an enum variant as `M::E::V` -- is added by at most one
/// declaration. The prelude's declarations are part of the program and
/// come first, so a program declaring a prelude name is reported at its
/// own declaration.
fn check_duplicate_items(program: &ast::Program) -> Result<(), String> {
    fn add(seen: &mut std::collections::HashSet<String>, name: String, line: usize) -> Result<(), String> {
        if seen.insert(name) {
            Ok(())
        } else {
            Err(format!("diag.duplicate-item@{}", line))
        }
    }
    fn walk(items: &[ast::Item], path: &mut Vec<String>, seen: &mut std::collections::HashSet<String>) -> Result<(), String> {
        for it in items {
            match it {
                ast::Item::Fn(f) => {
                    let simple = match &f.assoc_type {
                        Some(t) => format!("{}::{}", t, f.name),
                        None => f.name.clone(),
                    };
                    add(seen, modres::qualify(path, &simple), f.line)?;
                }
                ast::Item::Extern(e) => add(seen, modres::qualify(path, &e.name), e.line)?,
                ast::Item::Struct(sd) => add(seen, modres::qualify(path, &sd.name), sd.line)?,
                ast::Item::Enum(e) => {
                    add(seen, modres::qualify(path, &e.name), e.line)?;
                    for v in &e.variants {
                        add(seen, modres::qualify(path, &format!("{}::{}", e.name, v.name)), e.line)?;
                    }
                }
                ast::Item::Module(_, name, inner, line) => {
                    add(seen, modres::qualify(path, name), *line)?;
                    path.push(name.clone());
                    walk(inner, path, seen)?;
                    path.pop();
                }
                ast::Item::Import(..) | ast::Item::ExternCode(..) => {}
            }
        }
        Ok(())
    }
    walk(&program.items, &mut Vec::new(), &mut std::collections::HashSet::new())
}

fn build_items_checked(expanded: &str) -> Result<interp::Items, String> {
    // Parsed as one token stream (not two separate parses) so the
    // statement-disambiguation prescan (spec/22 §2 rule 4) sees the
    // prelude's struct/enum names (Vec, String, Rc, Option, Result, ...)
    // when parsing the user program's own declarations.
    let combined = format!("{}\n{}", prelude::PRELUDE_SRC, expanded);
    let mut items = new_items();
    let mut ast = parser::parse(&combined)?;
    check_duplicate_items(&ast)?;
    modres::resolve_program(&mut ast)?;
    consts::process(&mut ast)?;
    build_items(&ast, &mut items);
    let asserts = typecheck::check_program(&items)?;
    check_static_asserts(items, asserts)
}

/// `rule.module.static-assert` (D-0052): each `static_assert` the static
/// pass recorded is computed now, before the program runs, by the
/// evaluator itself (a constant expression has no effect, so this is its
/// value at run time too), with the type arguments of the instantiation
/// it was met in. A false one rejects the program statically, with its
/// message; a checked failure in the condition (an overflow) is that
/// failure, statically, at the assertion.
fn check_static_asserts(items: interp::Items, asserts: Vec<typecheck::StaticAssert>) -> Result<interp::Items, String> {
    if asserts.is_empty() {
        return Ok(items);
    }
    let items = std::sync::Arc::new(items);
    let verdict = {
        let mut it = interp::Interp::new(items.clone());
        let mut verdict = Ok(());
        for a in &asserts {
            match it.eval_static_assert(&a.cond, &a.subst) {
                Ok(true) => {}
                Ok(false) => {
                    let msg = a.message.as_ref().map(|m| format!("{}{}", diagnostics::DETAIL_SEP, m)).unwrap_or_default();
                    verdict = Err(format!("diag.static-assert-failed{}@{}", msg, a.line));
                    break;
                }
                Err(d) => {
                    let (id, _) = diagnostics::split_location(&d);
                    verdict = Err(format!("{}@{}", id, a.line));
                    break;
                }
            }
        }
        verdict
    };
    verdict?;
    Ok(std::sync::Arc::try_unwrap(items).unwrap_or_else(|_| panic!("the static_assert evaluator kept the item table")))
}

/// The front end's verdict on a program, for any consumer that runs or
/// compiles it: the checked item table, or the static diagnostic that
/// rejected it (tagged in combined-text line space, syntax errors
/// already relocated), plus the source map either way.
pub struct Analysis {
    pub items: Option<std::sync::Arc<interp::Items>>,
    pub map: loader::SourceMap,
    pub error: Option<String>,
}

pub fn analyze(src: &str, origin: Option<&Path>) -> Analysis {
    let loader::Expansion { text, map, error } = loader::expand(src, origin);
    if let Some(e) = error {
        return Analysis { items: None, map, error: Some(offset_into_combined(&e)) };
    }
    match build_items_checked(&text) {
        Ok(items) => Analysis { items: Some(std::sync::Arc::new(items)), map, error: None },
        Err(d) => {
            let d = relocate_syntax_error(d, &map);
            Analysis { items: None, map, error: Some(d) }
        }
    }
}

/// Renders a static or dynamic outcome's diagnostic the way `coby`
/// prints it: a syntax error verbatim, anything else through the
/// registry with its `file:line`.
pub fn render_located(d: &str, phase: &str, map: &loader::SourceMap) -> String {
    if d.starts_with("parse error at ") || d.starts_with("lex error at ") {
        return format!("error: {}\n", d);
    }
    let (id, at) = resolve_user_location(d, map);
    let out = diagnostics::render(&id, phase, at.as_ref().map(|(f, l)| (f.as_str(), *l)));
    // A program-supplied message (`static_assert(c, "…")`, D-0052), after
    // the location.
    match diagnostics::detail(d) {
        Some(m) => match out.find("\n  at") {
            Some(i) => {
                let end = out[i + 1..].find('\n').map_or(out.len(), |j| i + 1 + j + 1);
                format!("{}  message: {}\n{}", &out[..end], m, &out[end..])
            }
            None => out,
        },
        None => out,
    }
}

/// Runs the static pass alone (`typecheck::check_program`), with no
/// execution at all. `Ok(())` means the static pass found nothing to
/// reject -- it does NOT mean the program will run to completion; the
/// dynamic evaluator remains the sole authority for that (see
/// `src/typecheck.rs`'s module doc comment: this pass is deliberately
/// conservative and does not attempt full precision).
pub fn check_source_static(src: &str) -> Result<(), String> {
    check_source_static_raw(src).map_err(|d| strip_location(&d))
}

/// As `check_source_static`, but keeps `typecheck.rs`'s internal
/// `"diag.xxx@N"` line-tagging (see `interp.rs`'s `current_line` doc
/// comment) intact instead of stripping it for the public API's
/// backward-compatible bare-id contract. Used by `main.rs`'s richer
/// diagnostic rendering; `diagnostics::split_location` reads it back
/// out.
pub fn check_source_static_raw(src: &str) -> Result<(), String> {
    check_expanded_static_raw(&expand_or_reject(src)?)
}

fn check_expanded_static_raw(expanded: &str) -> Result<(), String> {
    build_items_checked(expanded).map(|_| ())
}

/// Expands file-backed module declarations for the string-only entry
/// points. A loader rejection becomes an ordinary `"diag.xxx@N"` in
/// combined-text line space, as every other static rejection is.
fn expand_or_reject(src: &str) -> Result<String, String> {
    let loader::Expansion { text, error, .. } = loader::expand(src, None);
    match error {
        Some(e) => Err(offset_into_combined(&e)),
        None => Ok(text),
    }
}

/// A lexical or grammatical error names a `line:col` in the combined
/// text; rewrite it to `file:line:col` through the source map, as
/// `resolve_user_location` does for `"diag.xxx@N"`.
fn relocate_syntax_error(d: String, map: &loader::SourceMap) -> String {
    for kind in ["parse error at ", "lex error at "] {
        let Some(rest) = d.strip_prefix(kind) else { continue };
        let Some((lc, msg)) = rest.split_once(": ") else { continue };
        let Some((l, c)) = lc.split_once(':') else { continue };
        let Ok(n) = l.parse::<usize>() else { continue };
        let user = n.checked_sub(prelude::line_count()).filter(|&n| n >= 1);
        if let Some((file, line)) = user.and_then(|n| map.resolve(n)) {
            return format!("{}{}:{}:{}: {}", kind, file, line, c, msg);
        }
    }
    d
}

fn offset_into_combined(tagged: &str) -> String {
    let (id, line) = diagnostics::split_location(tagged);
    match line {
        Some(n) => format!("{}@{}", id, n + prelude::line_count()),
        None => id.to_string(),
    }
}

/// Drops this interpreter's internal `"diag.xxx@N"` source-line tag
/// (if any), restoring the bare `"diag.xxx"` id the conformance suite
/// and every existing caller of `run_source`/`check_source_static`/
/// `run_source_phased` compares against exactly.
fn strip_location(d: &str) -> String {
    diagnostics::split_location(d).0.to_string()
}

/// Resolves a raw, possibly `"diag.xxx@N"`-tagged diagnostic (as
/// returned by `run_source_phased_located`) to the bare id plus a line
/// number in *the user's own source file* -- `N` itself, wherever it
/// came from, is a line in the combined `"{prelude}\n{user src}"` text
/// every pass actually parses (see `prelude::line_count`'s doc
/// comment), so it is subtracted back out here rather than shown
/// as-is, which would otherwise point at an invisible line inside
/// prelude source the user never wrote. `None` if the diagnostic
/// carried no line at all, or (defensively) if the resulting line
/// would be `<= 0` -- a diagnostic genuinely attributable to prelude
/// code itself (not expected in practice, since library `spec/21`
/// functions are load-bearing, well-tested code, but reported as "no
/// location" rather than a nonsensical or negative user-file line).
pub fn resolve_user_line(raw: &str) -> (String, Option<usize>) {
    let (id, combined_line) = diagnostics::split_location(raw);
    let resolved = combined_line.and_then(|n| n.checked_sub(prelude::line_count())).filter(|&n| n >= 1);
    (id.to_string(), resolved)
}

/// As `resolve_user_line`, then through the `SourceMap` of the
/// expansion the diagnostic came from, so a line inside a file-backed
/// module names that file and its own line rather than a line in the
/// spliced text.
pub fn resolve_user_location(raw: &str, map: &loader::SourceMap) -> (String, Option<(String, usize)>) {
    let (id, line) = resolve_user_line(raw);
    let at = line.and_then(|n| map.resolve(n)).map(|(f, l)| (f.to_string(), l));
    (id, at)
}

/// The file and line a declaration's `line` (combined-text line space,
/// as the parser records it) came from: the entry file or a file-backed
/// module's file, as that file was named (`loader`).
pub fn locate(line: usize, map: &loader::SourceMap) -> Option<(String, usize)> {
    resolve_user_location(&format!("@{}", line), map).1
}

/// The phase at which a program was rejected, or that it ran to
/// completion with its exit status (`rule.fn.program`'s `ok(s)`) -- lets a caller (the conformance suite) assert the same
/// static/dynamic distinction `spec/conformance.md`'s own table records,
/// not merely the diagnostic name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Ok(u8),
    Static(String),
    Dynamic(String),
}

pub fn run_source_phased(src: &str) -> Outcome {
    match check_source_static(src) {
        Err(d) => return Outcome::Static(d),
        Ok(()) => {}
    }
    match run_expanded_status(src) {
        Ok(s) => Outcome::Ok(s),
        Err(d) => Outcome::Dynamic(strip_location(&d)),
    }
}

/// As `Outcome`, but keeps the internal `"diag.xxx@N"` source-line tag
/// on the diagnostic instead of a bare id -- what `main.rs` actually
/// wants to render. `diagnostics::split_location`/`diagnostics::render`
/// consume the tagged string directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutcomeLocated {
    Ok(u8),
    Static(String),
    Dynamic(String),
}

pub fn run_source_phased_located(src: &str) -> OutcomeLocated {
    run_program_phased_located(src, None).0
}

/// The entry point `main.rs` uses. `origin` is the program's own file,
/// so file-backed module declarations resolve against its directory
/// and a diagnostic can be mapped back to the file it lies in through
/// the returned `SourceMap` (`resolve_user_location`). Expands once and
/// runs both passes over that one expansion.
pub fn run_program_phased_located(src: &str, origin: Option<&Path>) -> (OutcomeLocated, loader::SourceMap) {
    run_program_with_args(src, origin, Vec::new())
}

/// As `run_program_phased_located`, with the program's arguments
/// (`rule.stdlib.args`), as bytes.
pub fn run_program_with_args(src: &str, origin: Option<&Path>, args: Vec<Vec<u8>>) -> (OutcomeLocated, loader::SourceMap) {
    let Analysis { items, map, error } = analyze(src, origin);
    if let Some(d) = error {
        return (OutcomeLocated::Static(d), map);
    }
    let outcome = match run_items_raw(items.expect("no error means items"), args) {
        Ok(s) => OutcomeLocated::Ok(s),
        Err(d) => OutcomeLocated::Dynamic(d),
    };
    (outcome, map)
}

/// Runs `src` to completion. `main`'s own execution and every `spawn`ed
/// thread are genuine `std::thread`s sharing one `Interp` under a global
/// interpreter lock (`interp::Gil`; spec/19's `Σ` is exactly that shared
/// state): the thread holding the GIL runs the evaluator, releases it in
/// place whenever `join`/`lock`/a handle's destructor must wait for
/// another thread, and yields it between statements while other threads
/// are live.
pub fn run_source(src: &str) -> Result<(), String> {
    run_source_raw(src).map_err(|d| strip_location(&d))
}

/// As `run_source`, keeping the internal `"diag.xxx@N"` line tag intact
/// -- see `check_source_static_raw`'s doc comment for why a separate
/// `_raw` form exists alongside the public, backward-compatible one.
pub fn run_source_raw(src: &str) -> Result<(), String> {
    run_expanded_status(src).map(|_| ())
}

// As `run_source_raw`, keeping the exit status.
fn run_expanded_status(src: &str) -> Result<u8, String> {
    run_items_raw(std::sync::Arc::new(build_items_checked(&expand_or_reject(src)?)?), Vec::new())
}

fn run_items_raw(items: std::sync::Arc<interp::Items>, args: Vec<Vec<u8>>) -> Result<u8, String> {
    let mut interp = interp::Interp::new(items);
    interp.prog_args = std::sync::Arc::new(args);
    let gil = std::sync::Arc::new(interp::Gil::new(interp));
    gil.acquire();
    // Safety: this thread holds the GIL until `release` below; every
    // spawned thread acquires it before touching the interpreter.
    let r = {
        let g = unsafe { gil.interp() };
        g.gil = Some(std::sync::Arc::downgrade(&gil));
        g.run_main()
    };
    gil.release();
    r
}

pub use ast::*;
pub use interp::*;
pub use value::*;

#[cfg(test)]
mod located_diag_tests {
    use super::*;

    // Regression coverage for exactly the bug this was caught with by
    // hand while building this feature: `prelude::line_count()` must
    // account for the *actual* combined-text offset `build_interp`'s
    // `format!("{}\n{}", PRELUDE_SRC, src)` produces (including the one
    // extra blank line from PRELUDE_SRC already ending in its own
    // `\n`), not merely `PRELUDE_SRC.lines().count()` -- an earlier,
    // plausible-looking version of this function was off by exactly
    // one line, silently, for every single diagnostic. These are
    // asserted against *known* correct line numbers in short literal
    // programs, not against a re-derivation of the offset formula
    // itself, so an error in that formula can't cancel out here the
    // way it would if the test recomputed the same arithmetic.
    // A fault raised inside `std` (here `Vec::index_shared`'s bounds
    // check, and `arg`'s) is reported at the program's line that called
    // into `std` -- the call's own line, even on a statement's second
    // line -- not at "unknown location".
    #[test]
    fn fault_inside_std_reports_the_calling_line() {
        let src = "import std;\nfn main()\n{\n    Vec<i32> v = Vec::new();\n    i32 x = *Vec::index_shared(&v, 3);\n}\n";
        let OutcomeLocated::Dynamic(raw) = run_source_phased_located(src) else { panic!("expected a dynamic fault") };
        assert_eq!(resolve_user_line(&raw), ("diag.index-out-of-bounds".to_string(), Some(5)));
        let src = "import std;\nfn main()\n{\n    String s =\n        Result::unwrap_or(arg(0), String::new());\n}\n";
        let OutcomeLocated::Dynamic(raw) = run_source_phased_located(src) else { panic!("expected a dynamic fault") };
        assert_eq!(resolve_user_line(&raw), ("diag.index-out-of-bounds".to_string(), Some(5)));
    }

    #[test]
    fn static_fault_reports_the_users_own_line_not_a_prelude_line() {
        let src = "fn main()\n{\n    auto x = 2147483647:i32 + 1:i32;\n}\n";
        let outcome = run_source_phased_located(src);
        let OutcomeLocated::Static(raw) = outcome else { panic!("expected a static rejection, got {:?}", outcome) };
        let (id, line) = resolve_user_line(&raw);
        assert_eq!(id, "diag.arith-overflow");
        assert_eq!(line, Some(3), "line 3 is `auto x = ...` in this 4-line program");
    }

    #[test]
    fn dynamic_fault_reports_the_users_own_line() {
        let src = "fn main()\n{\n    i32 a = 1;\n    i32 b = 0;\n    auto c = a / b;\n}\n";
        let outcome = run_source_phased_located(src);
        let OutcomeLocated::Dynamic(raw) = outcome else { panic!("expected a dynamic fault, got {:?}", outcome) };
        let (id, line) = resolve_user_line(&raw);
        assert_eq!(id, "diag.div-by-zero");
        assert_eq!(line, Some(5), "line 5 is `auto c = a / b;` in this 6-line program");
    }

    #[test]
    fn a_fault_in_a_tail_variant_payload_reports_the_tail_line() {
        // `Some(out)` as the body's tail is built with its expected type,
        // a path that bypasses `eval`'s own line tagging.
        let src = "import std;\nstruct H { ref<Vec<i32>, exclusive> r; }\nfn f() : Option<Vec<i32>>\n{\n    Vec<i32> out = Vec::new();\n    H h = H { .r = &mut out };\n    Some(out)\n}\nfn main()\n{\n    auto o = f();\n}\n";
        let outcome = run_source_phased_located(src);
        let OutcomeLocated::Dynamic(raw) = outcome else { panic!("expected a dynamic fault, got {:?}", outcome) };
        let (id, line) = resolve_user_line(&raw);
        assert_eq!(id, "diag.move-while-aliased");
        assert_eq!(line, Some(7), "line 7 is the tail `Some(out)`");
    }

    #[test]
    fn name_not_visible_reports_the_referencing_line() {
        let src = "module inner\n{\n    fn helper() : i32\n    {\n        1\n    }\n}\n\nfn main()\n{\n    auto x = inner::helper();\n}\n";
        let outcome = run_source_phased_located(src);
        let OutcomeLocated::Static(raw) = outcome else { panic!("expected a static rejection, got {:?}", outcome) };
        let (id, line) = resolve_user_line(&raw);
        assert_eq!(id, "diag.name-not-visible");
        assert_eq!(line, Some(11), "line 11 is the `inner::helper()` call");
    }

    #[test]
    fn a_rejected_import_reports_its_own_line() {
        let src = "module inner\n{\n    fn helper() : i32\n    {\n        1\n    }\n}\n\nimport inner::helper;\n\nfn main()\n{\n}\n";
        let outcome = run_source_phased_located(src);
        let OutcomeLocated::Static(raw) = outcome else { panic!("expected a static rejection, got {:?}", outcome) };
        let (id, line) = resolve_user_line(&raw);
        assert_eq!(id, "diag.name-not-visible");
        assert_eq!(line, Some(9), "line 9 is the `import` of a private item");
    }

    #[test]
    fn resolve_user_line_passes_through_an_untagged_diag_unchanged() {
        assert_eq!(resolve_user_line("diag.unbound-name"), ("diag.unbound-name".to_string(), None));
    }

    #[test]
    fn located_and_public_apis_agree_on_the_bare_diag_id() {
        // run_source_phased (the public, backward-compatible API every
        // existing test uses) and run_source_phased_located must never
        // disagree about *which* diagnostic fired -- only about
        // whether a location is attached.
        let src = "fn main()\n{\n    auto x = 2147483647:i32 + 1:i32;\n}\n";
        let plain = run_source_phased(src);
        let located = run_source_phased_located(src);
        match (plain, located) {
            (Outcome::Static(a), OutcomeLocated::Static(b)) => {
                assert_eq!(a, resolve_user_line(&b).0);
            }
            other => panic!("phase mismatch between the two APIs: {:?}", other),
        }
    }
}
