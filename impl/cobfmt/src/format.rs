// The layout: tokens and comments in, the house style out
// (`private/cobfmt-proposal.md` §3). Line structure is the author's: a
// line break is never added or removed, except that a block's `{` and an
// `else` go on lines of their own (Allman, §3.1), and blank lines are
// normalised (§3.4). Within a line, the spacing between two tokens is
// fixed by a rule where the tokens alone decide it, and kept as written
// (collapsed to none or one space) where they do not.

use coby::lexer::{Lexer, Tok, Trivia, TriviaTok};

#[derive(Debug)]
pub enum FmtError {
    Lex(String),
}

// One element of a line: a token (index into the token list) or a comment.
#[derive(Clone, Debug)]
enum Elem {
    Tok(usize),
    Comment { text: String, block: bool, pos: usize },
}

#[derive(Clone, Debug)]
struct Line {
    elems: Vec<Elem>,
    blank_before: usize, // blank lines in the source before this line
}

pub struct Formatted {
    pub text: String,
}

pub fn lex(src: &str) -> Result<Vec<TriviaTok>, FmtError> {
    Lexer::new(src).tokenize_with_trivia().map_err(|e| FmtError::Lex(format!("{}:{}: {}", e.line, e.col, e.msg)))
}

// The source split into logical lines of tokens and comments.
fn split_lines(toks: &[TriviaTok]) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut cur = Line { elems: Vec::new(), blank_before: 0 };
    let mut newlines_run = 0usize; // line breaks since the last element
    let mut first = true;
    for (i, t) in toks.iter().enumerate() {
        for tr in &t.trivia {
            match tr {
                Trivia::Newline => {
                    if !cur.elems.is_empty() {
                        lines.push(std::mem::replace(&mut cur, Line { elems: Vec::new(), blank_before: 0 }));
                        newlines_run = 1;
                    } else {
                        newlines_run += 1;
                    }
                }
                Trivia::LineComment(text, pos) | Trivia::BlockComment(text, pos) => {
                    if cur.elems.is_empty() {
                        cur.blank_before = if first { 0 } else { newlines_run.saturating_sub(1) };
                        first = false;
                    }
                    let block = matches!(tr, Trivia::BlockComment(..));
                    cur.elems.push(Elem::Comment { text: text.replace("\r\n", "\n"), block, pos: *pos });
                    newlines_run = 0;
                }
            }
        }
        if matches!(t.tok, Tok::Eof) {
            break;
        }
        if cur.elems.is_empty() {
            cur.blank_before = if first { 0 } else { newlines_run.saturating_sub(1) };
            first = false;
        }
        cur.elems.push(Elem::Tok(i));
        newlines_run = 0;
    }
    if !cur.elems.is_empty() {
        lines.push(cur);
    }
    lines
}

fn tok_of<'a>(toks: &'a [TriviaTok], e: &Elem) -> Option<&'a Tok> {
    match e {
        Elem::Tok(i) => Some(&toks[*i].tok),
        _ => None,
    }
}

// The index of each `{`'s matching `}` and the reverse, over token indices.
fn match_braces(toks: &[TriviaTok]) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
    let mut close_of = vec![None; toks.len()];
    let mut open_of = vec![None; toks.len()];
    let mut stack: Vec<(usize, Tok)> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match t.tok {
            Tok::LBrace | Tok::LParen | Tok::LBracket => stack.push((i, t.tok.clone())),
            Tok::RBrace | Tok::RParen | Tok::RBracket => {
                if let Some((o, _)) = stack.pop() {
                    close_of[o] = Some(i);
                    open_of[i] = Some(o);
                }
            }
            _ => {}
        }
    }
    (close_of, open_of)
}

// Line of each token index.
fn token_lines(lines: &[Line], n: usize) -> Vec<usize> {
    let mut at = vec![usize::MAX; n];
    for (li, l) in lines.iter().enumerate() {
        for e in &l.elems {
            if let Elem::Tok(i) = e {
                at[*i] = li;
            }
        }
    }
    at
}

// Allman (§3.1): a `{` ending a line after code, whose block closes on a
// later line, goes on a line of its own; so does an `else` after a `}`
// that closes a block begun on an earlier line. Repeated to a fixed point.
// Whether the `{` at `ti` opens a function declaration's body: walking
// back at bracket depth 0 reaches `fn` followed by the function's name
// (`fn f(`, `fn T::f(`), not a `fn(…)` type or a closure's `= [](…)`.
fn is_fn_body(toks: &[TriviaTok], ti: usize, open_of: &[Option<usize>]) -> bool {
    let mut k = ti;
    while k > 0 {
        k -= 1;
        match toks[k].tok {
            Tok::RParen | Tok::RBracket | Tok::RBrace => match open_of[k] {
                Some(o) if toks[k].tok != Tok::RBrace => k = o,
                _ => return false,
            },
            Tok::Fn => return matches!(toks.get(k + 1).map(|t| &t.tok), Some(Tok::Ident(_) | Tok::TypeName(_))),
            Tok::Semi | Tok::LBrace | Tok::Eq | Tok::LParen | Tok::LBracket | Tok::Comma => return false,
            _ => {}
        }
    }
    false
}

// Whether the `{` at `ti` opens a block or a declaration body that the
// Allman rule moves: not a struct literal's (its fields begin `.x =`),
// and not a block used as a value right after an operator, `=`, `,` or
// `(` (`x = {`, `f(a, {`), which keep the author's layout.
fn is_block_brace(toks: &[TriviaTok], ti: usize) -> bool {
    let literal = matches!(toks.get(ti + 1).map(|t| &t.tok), Some(Tok::Dot));
    let value_block = ti > 0 && matches!(
        toks[ti - 1].tok,
        Tok::Eq | Tok::Comma | Tok::LParen | Tok::LBracket | Tok::OrOr | Tok::AndAnd | Tok::Plus | Tok::Minus | Tok::Star | Tok::Slash | Tok::Percent
            | Tok::EqEq | Tok::NotEq | Tok::Lt | Tok::Le | Tok::Ge | Tok::Return | Tok::Colon
            | Tok::PlusEq | Tok::MinusEq | Tok::StarEq | Tok::SlashEq | Tok::PercentEq | Tok::AmpEq | Tok::PipeEq | Tok::CaretEq | Tok::ShlEq | Tok::ShrEq
    );
    !literal && !value_block
}

fn allman(lines: Vec<Line>, toks: &[TriviaTok]) -> Vec<Line> {
    let (close_of, open_of) = match_braces(toks);
    // The innermost bracket each token lies inside: its opening token.
    let mut encl: Vec<Option<usize>> = vec![None; toks.len()];
    {
        let mut stack: Vec<usize> = Vec::new();
        for (i, t) in toks.iter().enumerate() {
            match t.tok {
                Tok::RParen | Tok::RBracket | Tok::RBrace => {
                    stack.pop();
                    encl[i] = stack.last().copied();
                }
                Tok::LParen | Tok::LBracket | Tok::LBrace => {
                    encl[i] = stack.last().copied();
                    stack.push(i);
                }
                _ => encl[i] = stack.last().copied(),
            }
        }
    }
    let mut lines = lines;
    loop {
        let at = token_lines(&lines, toks.len());
        let mut out: Vec<Line> = Vec::new();
        let mut changed = false;
        'line: for l in &lines {
            // A function's body is always Allman, even a one-line one
            // (`fn f() : i32 { 1 }`): `{`, the body, and `}` on lines of
            // their own. A comment on the line stays with the declaration.
            for (k, e) in l.elems.iter().enumerate() {
                let Elem::Tok(ti) = e else { continue };
                if toks[*ti].tok != Tok::LBrace {
                    continue;
                }
                let fn_body = is_fn_body(toks, *ti, &open_of);
                let Some(c) = close_of[*ti] else { continue };
                // Any other block: only when its body continues on later
                // lines (a one-line block stays, decision 4).
                if !fn_body && (at[c] == at[*ti] || !is_block_brace(toks, *ti)) {
                    continue;
                }
                let keep = |x: &&Elem| !matches!(x, Elem::Comment { block: false, .. });
                let Some(ck) = l.elems.iter().position(|x| matches!(x, Elem::Tok(t) if *t == c)) else {
                    // The body ends on a later line: code after the `{` on
                    // this one (`fn f() { x = 1;`) moves to a line of its
                    // own, and the `{` to one before it (with nothing after
                    // the `{`, the general rule below splits it).
                    let after: Vec<Elem> = l.elems[k + 1..].to_vec();
                    if !after.iter().any(|x| matches!(x, Elem::Tok(_))) {
                        continue;
                    }
                    let head: Vec<Elem> = l.elems[..k].to_vec();
                    if head.iter().any(|x| matches!(x, Elem::Tok(_))) {
                        out.push(Line { elems: head, blank_before: l.blank_before });
                        out.push(Line { elems: vec![l.elems[k].clone()], blank_before: 0 });
                    } else {
                        out.push(Line { elems: vec![l.elems[k].clone()], blank_before: l.blank_before });
                    }
                    out.push(Line { elems: after, blank_before: 0 });
                    changed = true;
                    continue 'line;
                };
                let comments: Vec<Elem> = l.elems[k + 1..].iter().filter(|x| matches!(x, Elem::Comment { block: false, .. })).cloned().collect();
                let mut head: Vec<Elem> = l.elems[..k].to_vec();
                if !head.iter().any(|x| matches!(x, Elem::Tok(_))) {
                    break;
                }
                head.extend(comments);
                out.push(Line { elems: head, blank_before: l.blank_before });
                out.push(Line { elems: vec![l.elems[k].clone()], blank_before: 0 });
                let body: Vec<Elem> = l.elems[k + 1..ck].iter().filter(keep).cloned().collect();
                if !body.is_empty() {
                    out.push(Line { elems: body, blank_before: 0 });
                }
                // `}`, with a `;` or `,` that closes it; anything else after
                // it starts a line of its own.
                let rest: Vec<Elem> = l.elems[ck + 1..].iter().filter(keep).cloned().collect();
                let n_close = rest.iter().take_while(|x| matches!(x, Elem::Tok(t) if matches!(toks[*t].tok, Tok::Semi | Tok::Comma))).count();
                let mut close = vec![l.elems[ck].clone()];
                close.extend(rest[..n_close].iter().cloned());
                out.push(Line { elems: close, blank_before: 0 });
                if n_close < rest.len() {
                    out.push(Line { elems: rest[n_close..].to_vec(), blank_before: 0 });
                }
                changed = true;
                continue 'line;
            }
            // Code elements (not comments).
            let code: Vec<usize> = l.elems.iter().enumerate().filter(|(_, e)| matches!(e, Elem::Tok(_))).map(|(k, _)| k).collect();
            if code.len() >= 2 {
                // One statement per line: after a `;` that ends a statement
                // of a block opened on an earlier line (or of the file), the
                // rest moves to a line of its own. A `;` inside brackets (a
                // `for` header) or inside a one-line block or declaration
                // (`if (c) { a(); b(); }`, `bitstruct B : u8 { lo : 4; hi : 4; }`)
                // stays. A trailing comment goes with the last statement.
                for (n, &k) in code.iter().enumerate() {
                    if n + 1 == code.len() {
                        break;
                    }
                    let Elem::Tok(ti) = &l.elems[k] else { continue };
                    if toks[*ti].tok != Tok::Semi {
                        continue;
                    }
                    let in_block = match encl[*ti] {
                        None => true,
                        Some(o) => toks[o].tok == Tok::LBrace && at[o] != at[*ti],
                    };
                    // What follows must be code of the same block, not its
                    // closing `}` (the rule below moves that).
                    let Elem::Tok(nx) = &l.elems[code[n + 1]] else { continue };
                    if !in_block || toks[*nx].tok == Tok::RBrace {
                        continue;
                    }
                    out.push(Line { elems: l.elems[..=k].to_vec(), blank_before: l.blank_before });
                    out.push(Line { elems: l.elems[k + 1..].to_vec(), blank_before: 0 });
                    changed = true;
                    continue 'line;
                }
                // A block's `}` after code on its line, the block opened on
                // an earlier line, goes on a line of its own (with a `,` or
                // `;` that follows it). A struct literal's, or a value
                // block's, keeps the author's layout.
                for (n, &k) in code.iter().enumerate().skip(1) {
                    let Elem::Tok(ti) = &l.elems[k] else { continue };
                    if toks[*ti].tok != Tok::RBrace {
                        continue;
                    }
                    let Some(o) = open_of[*ti] else { continue };
                    if at[o] == at[*ti] || !is_block_brace(toks, o) {
                        continue;
                    }
                    let _ = n;
                    out.push(Line { elems: l.elems[..k].to_vec(), blank_before: l.blank_before });
                    out.push(Line { elems: l.elems[k..].to_vec(), blank_before: 0 });
                    changed = true;
                    continue 'line;
                }
                let last_k = *code.last().unwrap();
                if let Elem::Tok(ti) = &l.elems[last_k] {
                    // A struct literal's `{` (its fields begin `.x =`) stays
                    // where it is: Allman is for blocks and declarations.
                    if toks[*ti].tok == Tok::LBrace && is_block_brace(toks, *ti) {
                        if let Some(c) = close_of[*ti] {
                            if at[c] != at[*ti] {
                                // Split before the `{`; a trailing comment after it
                                // stays with the code line.
                                let mut head: Vec<Elem> = l.elems[..last_k].to_vec();
                                head.extend(l.elems[last_k + 1..].iter().cloned());
                                out.push(Line { elems: head, blank_before: l.blank_before });
                                out.push(Line { elems: vec![l.elems[last_k].clone()], blank_before: 0 });
                                changed = true;
                                continue;
                            }
                        }
                    }
                }
                let first_k = code[0];
                if let (Elem::Tok(t0), Elem::Tok(t1)) = (&l.elems[first_k], &l.elems[code[1]]) {
                    if toks[*t0].tok == Tok::RBrace && toks[*t1].tok == Tok::Else {
                        if let Some(o) = open_of[*t0] {
                            if at[o] != at[*t0] {
                                out.push(Line { elems: l.elems[..=first_k].to_vec(), blank_before: l.blank_before });
                                out.push(Line { elems: l.elems[first_k + 1..].to_vec(), blank_before: 0 });
                                changed = true;
                                continue;
                            }
                        }
                    }
                }
            }
            out.push(l.clone());
        }
        lines = out;
        if !changed {
            return lines;
        }
    }
}

fn is_word(t: &Tok) -> bool {
    matches!(
        t,
        Tok::Int(..)
            | Tok::Float(..)
            | Tok::Str(_)
            | Tok::Bytes(_)
            | Tok::True
            | Tok::False
            | Tok::Ident(_)
            | Tok::TypeName(_)
            | Tok::Fn
            | Tok::Struct
            | Tok::Enum
            | Tok::Resource
            | Tok::Bitstruct
            | Tok::Match
            | Tok::If
            | Tok::Else
            | Tok::While
            | Tok::For
            | Tok::Foreach
            | Tok::Const
            | Tok::Return
            | Tok::Auto
            | Tok::Module
            | Tok::Import
            | Tok::Export
            | Tok::Unsafe
            | Tok::Extern
            | Tok::Break
            | Tok::Continue
            | Tok::Move
            | Tok::Mut
            | Tok::As
            | Tok::Void
    )
}

// Keywords followed by `(` with one space: `if (c)`.
fn is_paren_keyword(t: &Tok) -> bool {
    matches!(t, Tok::If | Tok::While | Tok::For | Tok::Foreach | Tok::Match | Tok::Return)
}

fn is_binary_only(t: &Tok) -> bool {
    matches!(
        t,
        Tok::EqEq
            | Tok::NotEq
            | Tok::Le
            | Tok::Ge
            | Tok::AndAnd
            | Tok::OrOr
            | Tok::Pipe
            | Tok::Caret
            | Tok::Shl
            | Tok::Slash
            | Tok::Percent
            | Tok::Eq
            | Tok::PlusEq
            | Tok::MinusEq
            | Tok::StarEq
            | Tok::SlashEq
            | Tok::PercentEq
            | Tok::AmpEq
            | Tok::PipeEq
            | Tok::CaretEq
            | Tok::ShlEq
            | Tok::ShrEq
            | Tok::Plus
    )
}

// Whether a token can end an operand: after one, `-`/`*`/`&` are binary.
fn ends_operand(t: &Tok) -> bool {
    // `}`: a block used as a value (`unsafe { … } - 1.0`).
    matches!(t, Tok::Int(..) | Tok::Float(..) | Tok::Str(_) | Tok::Bytes(_) | Tok::True | Tok::False | Tok::Ident(_) | Tok::RParen | Tok::RBracket | Tok::RBrace | Tok::Dollar | Tok::Question)
}

// The spacing between two adjacent tokens on one line: `Some(n)` fixed by
// a rule, `None` to keep the source's (none, or one space).
fn spacing(prev: &Tok, next: &Tok, prev_prev: Option<&Tok>, next_next: Option<&Tok>) -> Option<usize> {
    use Tok::*;
    // Never a space before these.
    match next {
        Comma | Semi | Question | RParen | RBracket => return Some(0),
        ColonColon => return Some(0),
        _ => {}
    }
    match prev {
        LParen | LBracket => return Some(0),
        ColonColon => return Some(0),
        Comma | Semi => return Some(1),
        _ => {}
    }
    // Braces on one line: `{ x }`, `{}`, and a space before `{`.
    if *prev == LBrace {
        return Some(if *next == RBrace { 0 } else { 1 });
    }
    if *next == RBrace {
        return Some(1);
    }
    if *next == LBrace {
        return Some(1);
    }
    if *prev == RBrace {
        return None;
    }
    // ` : ` after `)` and after a literal: a return type (`fn f() : i32`,
    // a closure's) or a match arm (`Some(v) : v`, `b'a' : 1`). A bound
    // (`<T: clone>`) follows a name and is left as written.
    if *next == Colon && matches!(prev, RParen | RBracket | Str(_) | Bytes(_) | Int(..) | Float(..) | True | False) {
        return Some(1);
    }
    if *prev == Colon && matches!(prev_prev, Some(RParen | RBracket | Str(_) | Bytes(_) | Int(..) | Float(..) | True | False)) {
        return Some(1);
    }
    // `.` field access and `.x =` in a struct literal: attached to what follows.
    if *prev == Dot {
        return Some(0);
    }
    if *next == Dot {
        return if ends_operand(prev) || matches!(prev, Gt) { Some(0) } else { None };
    }
    // `if (`: one space; a call or an index: none.
    if *next == LParen {
        if is_paren_keyword(prev) {
            return Some(1);
        }
        if matches!(prev, Ident(_) | TypeName(_) | RParen | RBracket | Fn) {
            return Some(0);
        }
        return None;
    }
    if *next == LBracket {
        if matches!(prev, Ident(w) if w == "in") {
            return Some(1);
        }
        if matches!(prev, Ident(_) | RParen | RBracket) {
            return Some(0);
        }
        return None;
    }
    // Unary operators: no space after.
    if matches!(prev, Not | Tilde) {
        return Some(0);
    }
    if matches!(prev, Minus | Star | Amp) {
        let unary = match prev_prev {
            None => true,
            // `foreach (x in &v)`: `in` is not a keyword token.
            Some(Ident(w)) if w == "in" => true,
            Some(pp) => !ends_operand(pp),
        };
        if unary {
            return Some(0);
        }
        return Some(1);
    }
    if matches!(next, Minus | Star | Amp) {
        if matches!(prev, Ident(w) if w == "in") {
            return Some(1);
        }
        return if ends_operand(prev) { Some(1) } else { None };
    }
    // `&mut x`
    if *prev == Mut {
        return Some(1);
    }
    // `<` and `>`: comparisons when a literal (or a unary `-`/`!`) is beside
    // them, which a type's angle brackets never have (`x > 2`, `0 < n`);
    // otherwise (`a<b`, `Vec<T>`) as written.
    // (A literal *before* proves nothing: `array<i32, 4>`.)
    let after_cmp = |t: &Tok| matches!(t, Int(..) | Float(..) | Str(_) | Bytes(_) | True | False | Minus | Not);
    if matches!(prev, Lt | Gt) && after_cmp(next) {
        return Some(1);
    }
    if matches!(next, Lt | Gt) && next_next.map_or(false, after_cmp) {
        return Some(1);
    }
    // Binary operators that are never anything else.
    if is_binary_only(prev) || is_binary_only(next) {
        return Some(1);
    }
    // Two words: one space.
    if is_word(prev) && is_word(next) {
        return Some(1);
    }
    None
}

fn indent_of_source(src: &str, line_start_pos: usize) -> usize {
    // Columns in characters of the original line's first element.
    let bytes = src.as_bytes();
    let mut s = line_start_pos;
    while s > 0 && bytes[s - 1] != b'\n' {
        s -= 1;
    }
    src[s..line_start_pos].chars().map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

fn elem_pos(toks: &[TriviaTok], e: &Elem) -> usize {
    match e {
        Elem::Tok(i) => toks[*i].pos,
        Elem::Comment { pos, .. } => *pos,
    }
}

// The rendered text of one line's elements (no indentation), and the
// character column (from the code's start) where its trailing comment,
// if any, begins in the source.
fn render_line(toks: &[TriviaTok], src: &str, l: &Line) -> (String, Option<(String, usize, bool)>) {
    let mut out = String::new();
    let mut prev: Option<usize> = None; // index into elems of the previous code element
    let mut trailing: Option<(String, usize, bool)> = None;
    for (k, e) in l.elems.iter().enumerate() {
        match e {
            Elem::Tok(i) => {
                let t = &toks[*i];
                if let Some(pk) = prev {
                    let Elem::Tok(pi) = &l.elems[pk] else { unreachable!() };
                    let pp = if pk > 0 {
                        l.elems[..pk].iter().rev().find_map(|x| tok_of(toks, x))
                    } else {
                        None
                    };
                    let nn = l.elems[k + 1..].iter().find_map(|x| tok_of(toks, x));
                    let n = match spacing(&toks[*pi].tok, &t.tok, pp, nn) {
                        Some(n) => n,
                        None => {
                            // As written: was there a space?
                            let end_prev = toks[*pi].pos + toks[*pi].text.len();
                            let gap = &src[end_prev..t.pos];
                            if gap.chars().any(|c| c == ' ' || c == '\t') { 1 } else { 0 }
                        }
                    };
                    for _ in 0..n {
                        out.push(' ');
                    }
                } else if k > 0 {
                    // After a leading block comment on the same line.
                    out.push(' ');
                }
                out.push_str(&t.text);
                prev = Some(k);
            }
            Elem::Comment { text, block, pos } => {
                if prev.is_some() && k == l.elems.len() - 1 && !*block {
                    // A trailing line comment: its source column, for alignment.
                    let col = indent_of_source(src, *pos);
                    trailing = Some((text.clone(), col, false));
                } else {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(text);
                    if *block && k + 1 < l.elems.len() {
                        // a block comment followed by code on the line
                    }
                }
            }
        }
    }
    (out, trailing)
}

fn last_code<'a>(toks: &'a [TriviaTok], l: &Line) -> Option<&'a Tok> {
    l.elems.iter().rev().find_map(|e| tok_of(toks, e))
}
fn first_code<'a>(toks: &'a [TriviaTok], l: &Line) -> Option<&'a Tok> {
    l.elems.iter().find_map(|e| tok_of(toks, e))
}

pub fn format(src_in: &str) -> Result<Formatted, FmtError> {
    let (bom, src) = match src_in.strip_prefix('\u{feff}') {
        Some(rest) => (true, rest),
        None => (false, src_in),
    };
    let crlf = src.find('\n').map_or(false, |i| i > 0 && src.as_bytes()[i - 1] == b'\r');
    let toks = lex(src)?;
    let lines = split_lines(&toks);
    let lines = allman(lines, &toks);
    let (close_of, open_of) = match_braces(&toks);
    let at = token_lines(&lines, toks.len());

    // Indentation (§3.1): a stack of open brackets that span lines.
    struct Frame {
        open_line: usize,
        brace: bool,
        inner: usize, // indentation for statements inside a brace frame
        match_body: Option<usize>, // a `match`'s body: its `{` token, as an id
    }
    // Per line: the `match` body it sits directly in, when it does.
    let mut arm_ctx: Vec<Option<usize>> = vec![None; lines.len()];
    let mut bit_bodies: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut new_indent = vec![0usize; lines.len()];
    let mut orig_indent = vec![0usize; lines.len()];
    for (li, l) in lines.iter().enumerate() {
        orig_indent[li] = indent_of_source(src, elem_pos(&toks, &l.elems[0]));
    }
    // Per brace frame: the line where the current statement began.
    let mut head_line: Vec<Option<usize>> = vec![None];
    let mut stmt_ended = true; // the previous code line ended a statement
    let mut prev_one_line_block = false; // ... with a `}` closing a `{` on the same line
    // Per line: the line where its segment began (itself, or, for a line
    // inside brackets opened earlier, the line that opened the outermost).
    let mut seg_start: Vec<usize> = (0..lines.len()).collect();
    let mut last_code_line: Option<usize> = None;
    // The previous code line, when it began with a `}` closing a block
    // opened on an earlier line.
    let mut prev_close_line: Option<usize> = None;
    for (li, l) in lines.iter().enumerate() {
        // Leading closers close their frames first.
        let mut lead_close = 0usize;
        for e in &l.elems {
            match tok_of(&toks, e) {
                Some(Tok::RBrace) | Some(Tok::RParen) | Some(Tok::RBracket) => {
                    let Elem::Tok(i) = e else { unreachable!() };
                    if let Some(o) = open_of[*i] {
                        if at[o] != li {
                            lead_close += 1;
                            continue;
                        }
                    }
                    break;
                }
                Some(_) => break,
                None => continue,
            }
        }
        let first_tok = first_code(&toks, l);
        let is_comment_only = first_tok.is_none();
        let ind;
        if lead_close > 0 {
            // `}`/`)`/`]` at the start of a line: at the indentation of the
            // line that opened it.
            let mut open_line = li;
            for _ in 0..lead_close {
                if let Some(f) = stack.pop() {
                    open_line = f.open_line;
                    if f.brace {
                        head_line.pop();
                    }
                }
            }
            ind = new_indent[open_line];
        } else {
            let in_paren = stack.last().map_or(false, |f| !f.brace);
            let frame_inner = stack.iter().rev().find(|f| f.brace).map_or(0, |f| f.inner);
            let opens_block = matches!(first_tok, Some(Tok::LBrace));
            if opens_block {
                // `{` on its own line: at the indentation of the line that
                // began the code just above it (`if (c)`, `else`, a closure's
                // `Vec::sort_by(…) : bool`, an arm's `match (x)` on its own
                // line; for a condition broken over lines, its first line).
                ind = match last_code_line {
                    // A bare block statement: where a statement goes.
                    _ if stmt_ended => frame_inner,
                    Some(p) => new_indent[seg_start[p]],
                    None => frame_inner,
                };
            } else if matches!(first_tok, Some(Tok::Else)) && prev_close_line.is_some() {
                // `else` after a `}` on a line of its own: with that `}`.
                ind = new_indent[prev_close_line.unwrap()];
            } else if in_paren {
                // Inside a bracket opened on an earlier line: as written,
                // relative to the line that opened it.
                let f = stack.last().unwrap();
                let base_new = new_indent[f.open_line];
                let base_orig = orig_indent[f.open_line];
                ind = if orig_indent[li] > base_orig { base_new + (orig_indent[li] - base_orig) } else { base_new + 4 };
            } else if matches!(first_tok, Some(Tok::Else)) && prev_one_line_block {
                // `else` after a one-line block (`if (c) { a }` then `else …`
                // on the next line): a continuation, as written.
                let h = head_line.last().cloned().flatten().unwrap_or(li);
                let base_new = new_indent[h];
                let base_orig = orig_indent[h];
                ind = if orig_indent[li] >= base_orig { base_new + (orig_indent[li] - base_orig) } else { base_new };
            } else if stmt_ended || is_comment_only {
                ind = frame_inner;
            } else {
                // A statement continued without a bracket: as written,
                // relative to its first line.
                let h = head_line.last().cloned().flatten().unwrap_or(li);
                let base_new = new_indent[h];
                let base_orig = orig_indent[h];
                ind = if orig_indent[li] >= base_orig { base_new + (orig_indent[li] - base_orig) } else { base_new };
            }
        }
        new_indent[li] = ind;
        if is_comment_only {
            continue;
        }
        if lead_close == 0 {
            arm_ctx[li] = stack.last().and_then(|f| f.match_body);
        }
        // Brackets (not braces) open since the innermost brace frame.
        let open_parens: Vec<usize> = stack.iter().rev().take_while(|f| !f.brace).map(|f| f.open_line).collect();
        if let Some(&outer) = open_parens.last() {
            seg_start[li] = seg_start[outer];
        }
        let continues_else = matches!(first_tok, Some(Tok::Else)) && (prev_one_line_block || prev_close_line.is_some());
        if stmt_ended && lead_close == 0 && !matches!(first_tok, Some(Tok::LBrace)) && !continues_else {
            if let Some(h) = head_line.last_mut() {
                *h = Some(li);
            }
        }
        // Open brackets that this line leaves open.
        for e in &l.elems {
            let Elem::Tok(i) = e else { continue };
            match toks[*i].tok {
                Tok::LBrace | Tok::LParen | Tok::LBracket => {
                    if let Some(c) = close_of[*i] {
                        if at[c] != li {
                            // A struct literal's braces hold fields laid out as
                            // the author chose, like a bracket's contents.
                            let literal = matches!(toks.get(*i + 1).map(|t| &t.tok), Some(Tok::Dot));
                            let brace = toks[*i].tok == Tok::LBrace && !literal;
                            // `match (…) {`: the token before `{` is `)`, whose `(` follows `match`.
                            let is_match = brace && *i > 0 && toks[*i - 1].tok == Tok::RParen
                                && open_of[*i - 1].map_or(false, |o| o > 0 && toks[o - 1].tok == Tok::Match);
                            // `bitstruct Name : u32 {`: its fields' `:` align as arms do.
                            let is_bits = brace && *i >= 4 && toks[*i - 4].tok == Tok::Bitstruct;
                            if is_bits {
                                bit_bodies.insert(*i);
                            }
                            stack.push(Frame { open_line: li, brace, inner: ind + 4, match_body: if is_match || is_bits { Some(*i) } else { None } });
                            if brace {
                                head_line.push(None);
                            }
                        }
                    }
                }
                Tok::RBrace | Tok::RParen | Tok::RBracket => {
                    if let Some(o) = open_of[*i] {
                        // Closed here, opened on an earlier line, not at the
                        // line's start (those were handled above).
                        if at[o] != li && !l.elems.iter().take_while(|x| tok_of(&toks, x).map_or(true, |t| matches!(t, Tok::RBrace | Tok::RParen | Tok::RBracket))).any(|x| matches!(x, Elem::Tok(j) if j == i)) {
                            if let Some(f) = stack.pop() {
                                if f.brace {
                                    head_line.pop();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        last_code_line = Some(li);
        prev_close_line = if lead_close > 0 && matches!(first_tok, Some(Tok::RBrace)) { Some(li) } else { None };
        // Did this line end a statement?
        let last = last_code(&toks, l);
        stmt_ended = match last {
            Some(Tok::Semi) | Some(Tok::LBrace) | Some(Tok::Comma) => true,
            // A `}`: the end of a block statement, or of a one-line one.
            Some(Tok::RBrace) => true,
            _ => false,
        };
        prev_one_line_block = matches!(last, Some(Tok::RBrace)) && {
            let li_last = l.elems.iter().rev().find_map(|e| match e {
                Elem::Tok(i) => Some(*i),
                _ => None,
            });
            li_last.and_then(|i| open_of[i]).map_or(false, |o| at[o] == li)
        };
        // A line inside a still-open bracket never ends the statement.
        if stack.last().map_or(false, |f| !f.brace) {
            stmt_ended = false;
        }
    }

    // Match arms (the owner's rule): in a run of consecutive one-line arms of
    // one `match`, the ` : ` of each stands in one column, one space after
    // the longest pattern. A line's arm colon is its first `:` outside
    // brackets; the arm ends with `,` (or is the last before the `}`).
    let arm_colon = |li: usize| -> Option<usize> {
        arm_ctx[li]?;
        let l = &lines[li];
        let mut depth = 0i32;
        let mut colon = None;
        for (k, e) in l.elems.iter().enumerate() {
            match tok_of(&toks, e) {
                Some(Tok::LParen | Tok::LBracket | Tok::LBrace) => depth += 1,
                Some(Tok::RParen | Tok::RBracket | Tok::RBrace) => depth -= 1,
                Some(Tok::Colon) if depth == 0 && colon.is_none() => colon = Some(k),
                _ => {}
            }
        }
        let k = colon?;
        if k == 0 || depth != 0 {
            return None;
        }
        // The arm ends on this line, or its value begins on the next (the
        // line ends with the colon: `Some(x) :` then a block).
        let rest_has_code = l.elems[k + 1..].iter().any(|e| matches!(e, Elem::Tok(_)));
        // A bitstruct's field (`name : 4;`) ends with its `;`.
        if arm_ctx[li].map_or(false, |b| bit_bodies.contains(&b)) {
            return if rest_has_code && last_code(&toks, l) == Some(&Tok::Semi) { Some(k) } else { None };
        }
        let ends = last_code(&toks, l) == Some(&Tok::Comma)
            || lines.get(li + 1).map_or(false, |n| first_code(&toks, n) == Some(&Tok::RBrace));
        if !rest_has_code || ends { Some(k) } else { None }
    };
    let mut arm_col: Vec<Option<usize>> = vec![None; lines.len()]; // the width the pattern is padded to
    let mut li = 0;
    while li < lines.len() {
        if arm_colon(li).is_none() {
            li += 1;
            continue;
        }
        // Every arm of this `match` (lines between them, an arm's block,
        // belong to other contexts or are not arms).
        let ctx = arm_ctx[li];
        let members: Vec<usize> = (li..lines.len()).filter(|&j| arm_ctx[j] == ctx && arm_colon(j).is_some()).collect();
        let e = li + 1;
        if members.len() >= 2 && arm_col[li].is_none() {
            let w = members
                .iter()
                .map(|&j| {
                    let k = arm_colon(j).unwrap();
                    let pat = Line { elems: lines[j].elems[..k].to_vec(), blank_before: 0 };
                    render_line(&toks, src, &pat).0.chars().count()
                })
                .max()
                .unwrap_or(0);
            for &j in &members {
                arm_col[j] = Some(w);
            }
        }
        li = e;
    }

    // Render, then align trailing comments (§3.5).
    let mut texts: Vec<(String, Option<(String, usize, bool)>)> = Vec::new();
    for (li, l) in lines.iter().enumerate() {
        let (body, trailing) = match (arm_col[li], arm_colon(li)) {
            (Some(w), Some(k)) => {
                let pat = Line { elems: l.elems[..k].to_vec(), blank_before: 0 };
                let rest = Line { elems: l.elems[k + 1..].to_vec(), blank_before: 0 };
                let (p, _) = render_line(&toks, src, &pat);
                let (mut r, mut tr) = render_line(&toks, src, &rest);
                // A comment alone after the colon is the line's trailing comment.
                if let [Elem::Comment { text, block: false, pos }] = &rest.elems[..] {
                    r = String::new();
                    tr = Some((text.clone(), indent_of_source(src, *pos), false));
                }
                let pad = w.saturating_sub(p.chars().count());
                if r.is_empty() {
                    (format!("{}{} :", p, " ".repeat(pad)), tr)
                } else {
                    (format!("{}{} : {}", p, " ".repeat(pad), r), tr)
                }
            }
            _ => render_line(&toks, src, l),
        };
        // A line comment alone on its line at the column of the trailing
        // comment just above continues that comment: it stays in the column.
        if li > 0 && first_code(&toks, l).is_none() && l.elems.len() == 1 {
            if let (Some((_, col, _)), Elem::Comment { text, block: false, pos }) = (&texts[li - 1].1, &l.elems[0]) {
                if indent_of_source(src, *pos) == *col {
                    texts.push((String::new(), Some((text.clone(), *col, true))));
                    continue;
                }
            }
        }
        let line = format!("{}{}", " ".repeat(new_indent[li]), body);
        texts.push((line, trailing));
    }
    let n = texts.len();
    let mut out_lines: Vec<String> = vec![String::new(); n];
    let mut k = 0;
    while k < n {
        match &texts[k].1 {
            None => {
                out_lines[k] = texts[k].0.clone();
                k += 1;
            }
            Some((_, col, _)) => {
                // A group: consecutive lines with trailing comments at the same source column.
                let col = *col;
                let mut e = k;
                while e < n && matches!(&texts[e].1, Some((_, c, _)) if *c == col) {
                    e += 1;
                }
                let widest = (k..e).map(|j| texts[j].0.chars().count()).max().unwrap_or(0);
                let use_col = if e - k == 1 {
                    if widest < col { col } else { widest + 1 }
                } else if widest < col {
                    col
                } else {
                    (widest + 1 + 3) / 4 * 4
                };
                for j in k..e {
                    let (code, tr) = &texts[j];
                    let w = code.chars().count();
                    let pad = if use_col > w { use_col - w } else { 1 };
                    out_lines[j] = format!("{}{}{}", code, " ".repeat(pad), tr.as_ref().unwrap().0);
                }
                k = e;
            }
        }
    }
    // Blank lines (§3.4).
    let mut text = String::new();
    for (li, l) in out_lines.iter().enumerate() {
        let mut blanks = lines[li].blank_before.min(1);
        if li == 0 {
            blanks = 0;
        }
        if li > 0 && last_code(&toks, &lines[li - 1]) == Some(&Tok::LBrace) && lines[li - 1].elems.len() == 1 {
            blanks = 0;
        }
        if first_code(&toks, &lines[li]) == Some(&Tok::RBrace) && lines[li].elems.iter().all(|e| matches!(tok_of(&toks, e), Some(_)) ) && matches!(lines[li].elems.first(), Some(Elem::Tok(_))) {
            // none right before a closing `}` line
            if lead_closes_block(&toks, &lines[li], &open_of, &at, li) {
                blanks = 0;
            }
        }
        for _ in 0..blanks {
            text.push('\n');
        }
        text.push_str(l.trim_end());
        text.push('\n');
    }
    if crlf {
        text = text.replace('\n', "\r\n");
    }
    if bom {
        text.insert(0, '\u{feff}');
    }
    Ok(Formatted { text })
}

fn lead_closes_block(toks: &[TriviaTok], l: &Line, open_of: &[Option<usize>], at: &[usize], li: usize) -> bool {
    match l.elems.first() {
        Some(Elem::Tok(i)) if toks[*i].tok == Tok::RBrace => open_of[*i].map_or(false, |o| at[o] != li),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::format;

    fn f(src: &str) -> String {
        format(src).expect("formats").text
    }

    fn fixed(src: &str) {
        assert_eq!(f(src), src, "expected no change");
    }

    #[test]
    fn allman_braces_and_else() {
        assert_eq!(
            f("fn main() {\n    if (c) {\n        a();\n    } else {\n        b();\n    }\n}\n"),
            "fn main()\n{\n    if (c)\n    {\n        a();\n    }\n    else\n    {\n        b();\n    }\n}\n"
        );
    }

    #[test]
    fn indentation_is_four_spaces_per_block() {
        assert_eq!(f("fn main()\n{\nif (c)\n{\nx = 1;\n}\n}\n"), "fn main()\n{\n    if (c)\n    {\n        x = 1;\n    }\n}\n");
    }

    #[test]
    fn function_bodies_are_always_allman() {
        assert_eq!(f("fn f() : i32 { 1 }\n"), "fn f() : i32\n{\n    1\n}\n");
        assert_eq!(f("fn main() {}\n"), "fn main()\n{\n}\n");
        // Code after the `{`, the body continuing on later lines.
        assert_eq!(f("fn f() : i32 { i32 x = 1;\n    x + 1\n}\n"), "fn f() : i32\n{\n    i32 x = 1;\n    x + 1\n}\n");
        assert_eq!(f("fn f()\n{ g();\n}\n"), "fn f()\n{\n    g();\n}\n");
        // The same for a statement block whose body continues below.
        assert_eq!(
            f("fn main()\n{\n    if (c) { x = 1;\n        y();\n    }\n}\n"),
            "fn main()\n{\n    if (c)\n    {\n        x = 1;\n        y();\n    }\n}\n"
        );
        assert_eq!(
            f("fn apply(fn(i32) : i32 g, i32 x) : i32 { g(x) }   // calls g\n"),
            "fn apply(fn(i32) : i32 g, i32 x) : i32            // calls g\n{\n    g(x)\n}\n"
        );
        assert_eq!(f("module m\n{\n    export fn f() : i32 { 2 }\n}\n"), "module m\n{\n    export fn f() : i32\n    {\n        2\n    }\n}\n");
        // A closure, and a variable of a `fn` type set to one, keep their layout.
        fixed("fn main()\n{\n    fn(i32) : i32 g = [](i32 x) { x + 1 };\n    auto h = [](i32 x) { x };\n}\n");
    }

    #[test]
    fn one_statement_per_line() {
        assert_eq!(
            f("fn main()\n{\n    i32 a = 1; i32 b = 2;   // two\n    g();\n}\n"),
            "fn main()\n{\n    i32 a = 1;\n    i32 b = 2;              // two\n    g();\n}\n"
        );
        // A block's `}` after code goes on its own line.
        assert_eq!(
            f("fn main()\n{\n    while (c)\n    {\n        a += 1; }\n}\n"),
            "fn main()\n{\n    while (c)\n    {\n        a += 1;\n    }\n}\n"
        );
        // Kept: a `for` header, a one-line block, a one-line declaration.
        fixed("fn main()\n{\n    for (i32 i = 0; i < 3; i += 1) { a(); b(); }\n    if (c) { x = 1; y = 2; }\n}\n");
        fixed("bitstruct B : u8 { lo : 4; hi : 4; }\nstruct P { i32 a; i32 b; }\n");
    }

    #[test]
    fn one_line_blocks_stay() {
        fixed("fn main()\n{\n    u64 v = if (c) { 1 } else { 0 };\n    if (n == 0) { return; }\n}\n");
        assert_eq!(f("fn main()\n{\n    if (n == 0) {return;}\n}\n"), "fn main()\n{\n    if (n == 0) { return; }\n}\n");
    }

    #[test]
    fn spacing_rules() {
        assert_eq!(f("fn main()\n{\n    x=a+b*c;\n}\n"), "fn main()\n{\n    x = a + b * c;\n}\n");
        assert_eq!(f("fn main()\n{\n    f( a ,b );\n}\n"), "fn main()\n{\n    f(a, b);\n}\n");
        assert_eq!(f("fn main()\n{\n    if(c) { x = - y; }\n}\n"), "fn main()\n{\n    if (c) { x = -y; }\n}\n");
        assert_eq!(f("fn main()\n{\n    foreach (x in & v) { g(* x); }\n}\n"), "fn main()\n{\n    foreach (x in &v) { g(*x); }\n}\n");
        assert_eq!(f("fn f(): u8\n{\n    0\n}\n"), "fn f() : u8\n{\n    0\n}\n");
        fixed("fn main()\n{\n    Vec::push(&mut v, x);\n    u8 c = b[i] & 0xC0;\n    i32 d = a - b;\n}\n");
        fixed("fn f<T: clone>(ref<Vec<T>, shared> v) : usize\n{\n    Vec::len(v)\n}\n");
    }

    #[test]
    fn struct_literals_keep_their_layout() {
        fixed("fn f() : R\n{\n    R { .a = 1,\n        .b = 2 }\n}\n");
        fixed("fn f() : R\n{\n    R {\n        .a = 1,\n        .b = 2,\n    }\n}\n");
    }

    #[test]
    fn continuations_keep_their_offset() {
        fixed("fn main()\n{\n    printf(\"%d %d\\n\",\n        a, b);\n}\n");
        fixed("fn main()\n{\n    u32 d = if (a) { 1 }\n        else if (b) { 2 }\n        else { 3 };\n}\n");
    }

    #[test]
    fn closures_and_value_blocks_are_allman_at_the_statement() {
        fixed("fn main()\n{\n    Vec::sort_by(&mut v, [](ref<i64, shared> a, ref<i64, shared> b) : bool\n    {\n        *a < *b\n    });\n}\n");
        fixed("fn main()\n{\n    u64 x = if (c)\n    {\n        1\n    }\n    else\n    {\n        2\n    };\n}\n");
    }

    #[test]
    fn match_arm_on_the_next_line() {
        fixed("fn f() : bool\n{\n    match (a)\n    {\n        Some(x) :\n            match (b)\n            {\n                Some(y) : x == y,\n                None    : false,\n            },\n        None    : false,\n    }\n}\n");
    }

    #[test]
    fn comments_kept_and_trailing_groups_aligned() {
        fixed("// a header\nfn main()\n{\n    // a comment\n    x = 1;      // one\n    yy = 22;    // two\n}\n");
        // A group whose column the code now overruns moves together.
        assert_eq!(
            f("fn main()\n{\n    x  =  1;  // one\n    y = 2;    // two\n}\n"),
            "fn main()\n{\n    x = 1;    // one\n    y = 2;    // two\n}\n"
        );
        fixed("fn main()\n{\n    /* a block\n       comment */\n    x = 1;\n}\n");
    }

    #[test]
    fn bitstruct_field_colons_align() {
        assert_eq!(
            f("bitstruct Double : u64\n{\n    mantissa : 52;\n    exponent : 11;\n    sign : 1;\n}\n"),
            "bitstruct Double : u64\n{\n    mantissa : 52;\n    exponent : 11;\n    sign     : 1;\n}\n"
        );
        // One line: left alone.
        fixed("bitstruct B : u8 { lo : 4; hi : 4; }\n");
    }

    #[test]
    fn match_arm_colons_align() {
        assert_eq!(
            f("fn f(Option<i32> o) : i32\n{\n    match (o)\n    {\n        Some(v) : v,\n        None : 0,\n    }\n}\n"),
            "fn f(Option<i32> o) : i32\n{\n    match (o)\n    {\n        Some(v) : v,\n        None    : 0,\n    }\n}\n"
        );
        // Arms whose value starts on the next line align too; a nested
        // match aligns on its own; a one-line match is left alone.
        fixed(
            "fn f(Option<i32> o) : i32\n{\n    match (o)\n    {\n        Some(v) :\n        {\n            match (v) { 1 : 2, _ : 3, }\n        },\n        None    : 0,\n    }\n}\n",
        );
        fixed("fn f(Option<i32> o) : i32\n{\n    match (o)\n    {\n        Some(v)      : v,           // the payload\n        None         : 0,\n    }\n}\n".replace("v)      :", "v) :").replace("None         :", "None    :").as_str());
    }

    #[test]
    fn blank_lines() {
        assert_eq!(f("\n\nfn a()\n{\n\n    x();\n\n}\n\n\n\nfn b()\n{\n}\n\n\n"), "fn a()\n{\n    x();\n}\n\nfn b()\n{\n}\n");
    }

    #[test]
    fn crlf_and_bom_kept() {
        let src = "\u{feff}fn main()\r\n{\r\n  x=1;\r\n}\r\n";
        assert_eq!(f(src), "\u{feff}fn main()\r\n{\r\n    x = 1;\r\n}\r\n");
    }

    #[test]
    fn trailing_whitespace_removed_and_tabs_become_spaces() {
        assert_eq!(f("fn main()   \n{\n\tx = 1;  \n}\n"), "fn main()\n{\n    x = 1;\n}\n");
    }

    #[test]
    fn idempotent_on_messy_input() {
        let messy = "import std;\nfn main(){ if(a){x=1;}else{ y=2; }\nforeach(i in 0..n){ printf(\"%v\",i ) ;}\n  }\n";
        let once = f(messy);
        assert_eq!(f(&once), once);
    }
}
