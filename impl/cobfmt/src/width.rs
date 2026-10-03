// `--width N` (`private/cobfmt-proposal.md` §4.4): a line longer than N
// whose arguments, parameters or array elements are a bracketed list on
// that line is broken one item per line, indented one level more, with a
// trailing comma (D-0098), the closing bracket back at the line's
// indentation. The outermost such list on the line is broken first; the
// result is broken again while lines are still too long and a list is
// left. A line holding a comment inside the list, or with no list of two
// or more items, is left long. Nothing else is broken.

use coby::lexer::Tok;

pub fn break_long_lines(text: &str, width: usize) -> String {
    let mut cur = text.to_string();
    for _ in 0..64 {
        match break_one(&cur, width) {
            Some(next) => cur = next,
            None => return cur,
        }
    }
    cur
}

fn break_one(text: &str, width: usize) -> Option<String> {
    let crlf = text.contains("\r\n");
    let plain = text.replace("\r\n", "\n");
    let (bom, body) = match plain.strip_prefix('\u{feff}') {
        Some(r) => ("\u{feff}", r.to_string()),
        None => ("", plain.clone()),
    };
    let toks = crate::format::lex(&body).ok()?;
    // Line start offsets.
    let mut starts = vec![0usize];
    for (i, b) in body.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    let line_of = |pos: usize| match starts.binary_search(&pos) {
        Ok(k) => k,
        Err(k) => k - 1,
    };
    // Bracket matching.
    let mut close_of = vec![None; toks.len()];
    let mut stack = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match t.tok {
            Tok::LParen | Tok::LBracket | Tok::LBrace => stack.push(i),
            Tok::RParen | Tok::RBracket | Tok::RBrace => {
                if let Some(o) = stack.pop() {
                    close_of[o] = Some(i);
                }
            }
            _ => {}
        }
    }
    for (li, &ls) in starts.iter().enumerate() {
        let le = starts.get(li + 1).map_or(body.len(), |&n| n - 1);
        let line = &body[ls..le];
        if line.chars().count() <= width {
            continue;
        }
        // The outermost `(`/`[` on this line closing on this line, with
        // two or more items and no comment inside.
        for (i, t) in toks.iter().enumerate() {
            if t.pos < ls || t.pos >= le || !matches!(t.tok, Tok::LParen | Tok::LBracket) {
                continue;
            }
            let Some(c) = close_of[i] else { continue };
            if line_of(toks[c].pos) != li {
                continue;
            }
            if toks[i + 1..=c].iter().any(|x| !x.trivia.is_empty()) {
                continue;
            }
            // Top-level commas.
            let mut depth = 0i32;
            let mut commas = Vec::new();
            for k in i + 1..c {
                match toks[k].tok {
                    Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                    Tok::RParen | Tok::RBracket | Tok::RBrace => depth -= 1,
                    Tok::Comma if depth == 0 => commas.push(k),
                    _ => {}
                }
            }
            let trailing = commas.last() == Some(&(c - 1));
            let items = commas.len() + if trailing { 0 } else { 1 };
            if items < 2 || c == i + 1 {
                continue;
            }
            let indent: String = line.chars().take_while(|ch| *ch == ' ').collect();
            let inner = format!("{}    ", indent);
            let mut out = String::new();
            out.push_str(&body[ls..toks[i].pos + 1]);
            out.push('\n');
            let mut from = i + 1;
            let mut bounds = commas.clone();
            if !trailing {
                bounds.push(c);
            }
            for &b in &bounds {
                let s = toks[from].pos;
                let e = toks[b - 1].pos + toks[b - 1].text.len();
                out.push_str(&inner);
                out.push_str(&body[s..e]);
                out.push_str(",\n");
                from = b + 1;
            }
            out.push_str(&indent);
            out.push_str(&body[toks[c].pos..le]);
            let mut whole = String::new();
            whole.push_str(bom);
            whole.push_str(&body[..ls]);
            whole.push_str(&out);
            whole.push_str(&body[le..]);
            return Some(if crlf { whole.replace('\n', "\r\n") } else { whole });
        }
    }
    None
}
