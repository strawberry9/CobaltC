// The safety check (`private/cobfmt-proposal.md` §6): the formatted text
// is the same program — the same tokens, spelled the same, in the same
// order; the same comments with the same text; and it parses.

use coby::lexer::{Tok, Trivia};

pub fn same_program(before: &str, after: &str, width_mode: bool, origin: Option<&std::path::Path>) -> Result<(), String> {
    let strip = |s: &str| s.trim_start_matches('\u{feff}').to_string();
    let a = crate::format::lex(&strip(before)).map_err(|_| "the original does not lex".to_string())?;
    let b = crate::format::lex(&strip(after)).map_err(|_| "the result does not lex".to_string())?;
    let toks = |v: &[coby::lexer::TriviaTok]| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (k, t) in v.iter().enumerate() {
            if matches!(t.tok, Tok::Eof) {
                continue;
            }
            // `--width` adds a trailing comma when it breaks a list one item
            // per line (D-0098): a comma right before a closing bracket is
            // not part of the comparison in that mode.
            if width_mode && t.tok == Tok::Comma && v.get(k + 1).map_or(false, |n| matches!(n.tok, Tok::RParen | Tok::RBracket | Tok::RBrace)) {
                continue;
            }
            out.push(t.text.clone());
        }
        out
    };
    let comments = |v: &[coby::lexer::TriviaTok]| -> Vec<String> {
        let mut out = Vec::new();
        for t in v {
            for tr in &t.trivia {
                match tr {
                    Trivia::LineComment(s, _) => out.push(s.trim_end().to_string()),
                    Trivia::BlockComment(s, _) => out.push(s.replace("\r\n", "\n")),
                    Trivia::Newline => {}
                }
            }
        }
        out
    };
    let (ta, tb) = (toks(&a), toks(&b));
    if ta != tb {
        let at = ta.iter().zip(tb.iter()).position(|(x, y)| x != y).unwrap_or(ta.len().min(tb.len()));
        return Err(format!("the tokens differ at token {} (`{}` became `{}`)", at, ta.get(at).map_or("end", |s| s), tb.get(at).map_or("end", |s| s)));
    }
    if comments(&a) != comments(&b) {
        return Err("the comments differ".to_string());
    }
    if crate::parses(after, origin).is_err() {
        return Err("the result does not parse".to_string());
    }
    let crlf = |s: &str| s.find('\n').map_or(false, |i| i > 0 && s.as_bytes()[i - 1] == b'\r');
    if crlf(before) != crlf(after) {
        return Err("the line endings changed".to_string());
    }
    Ok(())
}
