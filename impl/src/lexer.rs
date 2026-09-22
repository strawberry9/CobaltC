// Lexer for CobaltC surface syntax (spec/22 §1).

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    // literals
    Int(u128, Option<String>), // value, optional ':type' suffix
    Float(f64, Option<String>),
    Str(String),    // "..." -- decoded, valid UTF-8 by construction
    Bytes(Vec<u8>), // b"..." -- decoded bytes
    True,
    False,
    Ident(String),
    TypeName(String), // int types + f32 f64 bool str ref rawptr array handle mutex guard
    // keywords
    Fn,
    Struct,
    Enum,
    Resource,
    Match,
    If,
    Else,
    While,
    For,
    Foreach,
    Const,
    Return,
    Auto,
    Module,
    Import,
    Export,
    Unsafe,
    Extern,
    Break,
    Continue,
    Move,
    Mut,
    As,
    Void,
    // punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    ColonColon,
    Dot,
    DotDot, // `..` (D-0047): a slice's range
    Dollar, // `$` (D-0047): the length of what is being indexed
    Question,
    Eq,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Not,
    Tilde,
    Amp,
    Pipe,
    Caret,
    Shl,
    Shr,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    // `x op= e` (D-0035): the parser rewrites each to an assignment.
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    AmpEq,
    PipeEq,
    CaretEq,
    ShlEq,
    ShrEq,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub tok: T,
    pub line: usize,
    pub col: usize,
}

const INT_TYPES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "isize", "usize",
];
const TYPE_NAMES: &[&str] = &[
    "f32", "f64", "bool", "str", "ref", "rawptr", "array", "handle", "mutex", "guard", "slice",
];

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

#[derive(Debug)]
pub struct LexError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> u8 {
        *self.src.get(self.pos).unwrap_or(&0)
    }
    fn peek_at(&self, off: usize) -> u8 {
        *self.src.get(self.pos + off).unwrap_or(&0)
    }
    fn bump(&mut self) -> u8 {
        let c = self.peek();
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.bump();
                }
                b'/' if self.peek_at(1) == b'/' => {
                    while self.peek() != b'\n' && self.peek() != 0 {
                        self.bump();
                    }
                }
                b'/' if self.peek_at(1) == b'*' => {
                    let (line, col) = (self.line, self.col);
                    self.bump();
                    self.bump();
                    loop {
                        if self.peek() == 0 {
                            return Err(LexError {
                                msg: "unterminated block comment".into(),
                                line,
                                col,
                            });
                        }
                        if self.peek() == b'*' && self.peek_at(1) == b'/' {
                            self.bump();
                            self.bump();
                            break;
                        }
                        self.bump();
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Spanned<Tok>>, LexError> {
        let mut out = Vec::new();
        loop {
            self.skip_trivia()?;
            let (line, col) = (self.line, self.col);
            if self.peek() == 0 {
                out.push(Spanned {
                    tok: Tok::Eof,
                    line,
                    col,
                });
                break;
            }
            let tok = self.next_token()?;
            out.push(Spanned { tok, line, col });
        }
        Ok(out)
    }

    fn next_token(&mut self) -> Result<Tok, LexError> {
        let c = self.peek();
        if c.is_ascii_digit() {
            return self.lex_number();
        }
        if c == b'"' {
            return self.lex_quoted(false);
        }
        if c == b'b' && self.peek_at(1) == b'"' {
            self.bump();
            return self.lex_quoted(true);
        }
        if c == b'b' && self.peek_at(1) == b'\'' {
            self.bump();
            return self.lex_byte();
        }
        if c == b'_' || c.is_ascii_alphabetic() {
            return Ok(self.lex_ident());
        }
        let (line, col) = (self.line, self.col);
        self.bump();
        let tok = match c {
            b'(' => Tok::LParen,
            b')' => Tok::RParen,
            b'{' => Tok::LBrace,
            b'}' => Tok::RBrace,
            b'[' => Tok::LBracket,
            b']' => Tok::RBracket,
            b',' => Tok::Comma,
            b';' => Tok::Semi,
            b'.' if self.peek() == b'.' => {
                self.bump();
                Tok::DotDot
            }
            b'.' => Tok::Dot,
            b'$' => Tok::Dollar,
            b'?' => Tok::Question,
            b'~' => Tok::Tilde,
            b'^' if self.peek() == b'=' => {
                self.bump();
                Tok::CaretEq
            }
            b'^' => Tok::Caret,
            b'+' if self.peek() == b'=' => {
                self.bump();
                Tok::PlusEq
            }
            b'+' => Tok::Plus,
            b'-' if self.peek() == b'=' => {
                self.bump();
                Tok::MinusEq
            }
            b'-' => Tok::Minus,
            b'*' if self.peek() == b'=' => {
                self.bump();
                Tok::StarEq
            }
            b'*' => Tok::Star,
            b'/' if self.peek() == b'=' => {
                self.bump();
                Tok::SlashEq
            }
            b'/' => Tok::Slash,
            b'%' if self.peek() == b'=' => {
                self.bump();
                Tok::PercentEq
            }
            b'%' => Tok::Percent,
            b':' => {
                if self.peek() == b':' {
                    self.bump();
                    Tok::ColonColon
                } else {
                    Tok::Colon
                }
            }
            b'=' => {
                if self.peek() == b'=' {
                    self.bump();
                    Tok::EqEq
                } else {
                    Tok::Eq
                }
            }
            b'!' => {
                if self.peek() == b'=' {
                    self.bump();
                    Tok::NotEq
                } else {
                    Tok::Not
                }
            }
            b'<' => {
                if self.peek() == b'=' {
                    self.bump();
                    Tok::Le
                } else if self.peek() == b'<' {
                    self.bump();
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::ShlEq
                    } else {
                        Tok::Shl
                    }
                } else {
                    Tok::Lt
                }
            }
            b'>' => {
                if self.peek() == b'=' {
                    self.bump();
                    Tok::Ge
                } else if self.peek() == b'>' {
                    self.bump();
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::ShrEq
                    } else {
                        Tok::Shr
                    }
                } else {
                    Tok::Gt
                }
            }
            b'&' => {
                if self.peek() == b'&' {
                    self.bump();
                    Tok::AndAnd
                } else if self.peek() == b'=' {
                    self.bump();
                    Tok::AmpEq
                } else {
                    Tok::Amp
                }
            }
            b'|' => {
                if self.peek() == b'|' {
                    self.bump();
                    Tok::OrOr
                } else if self.peek() == b'=' {
                    self.bump();
                    Tok::PipeEq
                } else {
                    Tok::Pipe
                }
            }
            _ => {
                return Err(LexError {
                    msg: format!("unexpected character {:?}", c as char),
                    line,
                    col,
                });
            }
        };
        Ok(tok)
    }

    fn lex_ident(&mut self) -> Tok {
        let start = self.pos;
        while self.peek() == b'_' || self.peek().is_ascii_alphanumeric() {
            self.bump();
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
        match s.as_str() {
            "fn" => Tok::Fn,
            "struct" => Tok::Struct,
            "enum" => Tok::Enum,
            "resource" => Tok::Resource,
            "match" => Tok::Match,
            "if" => Tok::If,
            "else" => Tok::Else,
            "while" => Tok::While,
            "for" => Tok::For,
            "foreach" => Tok::Foreach,
            "const" => Tok::Const,
            "return" => Tok::Return,
            "auto" => Tok::Auto,
            "module" => Tok::Module,
            "import" => Tok::Import,
            "export" => Tok::Export,
            "unsafe" => Tok::Unsafe,
            "extern" => Tok::Extern,
            "break" => Tok::Break,
            "continue" => Tok::Continue,
            "move" => Tok::Move,
            "mut" => Tok::Mut,
            "as" => Tok::As,
            "void" => Tok::Void,
            "true" => Tok::True,
            "false" => Tok::False,
            _ if INT_TYPES.contains(&s.as_str()) => Tok::TypeName(s),
            _ if TYPE_NAMES.contains(&s.as_str()) => Tok::TypeName(s),
            _ => Tok::Ident(s),
        }
    }

    // `"..."` (spec/22 str-literal) and `b"..."` (byte-literal), positioned
    // on the opening quote. The str form admits only escapes that encode
    // valid UTF-8 (`\u{...}`, no `\x`), so its bytes are well-formed UTF-8 by
    // construction (`inv.str.utf8-validity`); the byte form admits `\xNN`
    // and only ASCII source characters.
    fn lex_quoted(&mut self, bytes: bool) -> Result<Tok, LexError> {
        let (line, col) = (self.line, self.col);
        let what = if bytes { "byte literal" } else { "string literal" };
        let err = |msg: String| LexError { msg, line, col };
        self.bump();
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek();
            match c {
                0 | b'\n' => return Err(err(format!("unterminated {}", what))),
                b'"' => {
                    self.bump();
                    break;
                }
                b'\\' => {
                    self.bump();
                    let e = self.bump();
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'0' => out.push(0),
                        b'\\' => out.push(b'\\'),
                        b'"' => out.push(b'"'),
                        b'x' if bytes => {
                            let hi = self.bump();
                            let lo = self.bump();
                            let v = (hex_val(hi), hex_val(lo));
                            match v {
                                (Some(h), Some(l)) => out.push(h * 16 + l),
                                _ => return Err(err(format!("invalid \\x escape in {}", what))),
                            }
                        }
                        b'u' if !bytes => {
                            if self.bump() != b'{' {
                                return Err(err("invalid \\u escape in string literal: expected '{'".into()));
                            }
                            let mut cp: u32 = 0;
                            let mut digits = 0;
                            loop {
                                let d = self.bump();
                                if d == b'}' {
                                    break;
                                }
                                match hex_val(d) {
                                    Some(h) if digits < 6 => {
                                        cp = cp * 16 + h as u32;
                                        digits += 1;
                                    }
                                    _ => return Err(err("invalid \\u escape in string literal".into())),
                                }
                            }
                            match char::from_u32(cp) {
                                Some(ch) if digits > 0 => {
                                    let mut buf = [0u8; 4];
                                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                                }
                                _ => return Err(err("invalid \\u escape in string literal: not a scalar value".into())),
                            }
                        }
                        _ => return Err(err(format!("invalid escape in {}", what))),
                    }
                }
                _ => {
                    if bytes && !c.is_ascii() {
                        return Err(err("non-ASCII character in byte literal".into()));
                    }
                    out.push(self.bump());
                }
            }
        }
        if bytes {
            if out.is_empty() {
                return Err(err("empty byte literal".into()));
            }
            Ok(Tok::Bytes(out))
        } else {
            // Source is UTF-8 (`fs::read_to_string`) and every escape emits
            // UTF-8, so this cannot fail.
            Ok(Tok::Str(String::from_utf8(out).expect("str literal bytes are UTF-8 by construction")))
        }
    }

    // `b'x'` (D-0033 (2)): one byte, an ASCII character or an escape, as
    // an integer literal of type `u8`. Positioned on the opening quote.
    fn lex_byte(&mut self) -> Result<Tok, LexError> {
        let (line, col) = (self.line, self.col);
        let err = |msg: &str| LexError { msg: msg.into(), line, col };
        self.bump();
        let c = self.bump();
        let v = match c {
            0 | b'\n' | b'\'' => return Err(err("empty or unterminated byte character")),
            b'\\' => match self.bump() {
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                b'0' => 0,
                b'\\' => b'\\',
                b'\'' => b'\'',
                b'"' => b'"',
                b'x' => match (hex_val(self.bump()), hex_val(self.bump())) {
                    (Some(h), Some(l)) => h * 16 + l,
                    _ => return Err(err("invalid \\x escape in byte character")),
                },
                _ => return Err(err("invalid escape in byte character")),
            },
            c if c.is_ascii() => c,
            _ => return Err(err("non-ASCII character in byte character")),
        };
        if self.bump() != b'\'' {
            return Err(err("a byte character holds exactly one byte"));
        }
        Ok(Tok::Int(v as u128, Some("u8".into())))
    }

    // An optional ':type' suffix after a numeric literal.
    fn lex_suffix(&mut self) -> Option<String> {
        // optional ':type' suffix. The corpus consistently spells this
        // with a space after the colon ("5: i32"). A literal pattern's arm
        // (D-0057) also puts a `:` after digits (`3 : x + 1`), so the colon
        // starts a suffix only when a numeric type's name follows it: an
        // arm's body never begins with a type name.
        let mut suffix = None;
        let mut look = self.pos;
        while matches!(self.src.get(look), Some(b' ') | Some(b'\t')) {
            look += 1;
        }
        if self.src.get(look) == Some(&b':') {
            let after_colon = look + 1;
            let mut tstart = after_colon;
            while matches!(self.src.get(tstart), Some(b' ') | Some(b'\t')) {
                tstart += 1;
            }
            let mut tend = tstart;
            while matches!(self.src.get(tend), Some(c) if *c == b'_' || c.is_ascii_alphanumeric()) {
                tend += 1;
            }
            const NUMERIC: &[&[u8]] = &[b"i8", b"i16", b"i32", b"i64", b"i128", b"isize", b"u8", b"u16", b"u32", b"u64", b"u128", b"usize", b"f32", b"f64"];
            if NUMERIC.contains(&&self.src[tstart..tend]) {
                while self.pos < tstart {
                    self.bump();
                }
                let sstart = self.pos;
                while self.peek() == b'_' || self.peek().is_ascii_alphanumeric() {
                    self.bump();
                }
                suffix = Some(std::str::from_utf8(&self.src[sstart..self.pos]).unwrap().to_string());
            }
        }
        suffix
    }

    // Digits for which `is_digit` holds, with `_` allowed only between two
    // of them (D-0043): `1_000_000`, `0xFFFF_0000`. The `_`s are dropped.
    fn digit_run(&mut self, is_digit: impl Fn(u8) -> bool, line: usize, col: usize) -> Result<String, LexError> {
        let mut out = String::new();
        loop {
            let c = self.peek();
            if is_digit(c) {
                out.push(c as char);
                self.bump();
            } else if c == b'_' {
                if out.is_empty() || !is_digit(self.peek_at(1)) {
                    return Err(sep_error(line, col));
                }
                self.bump();
            } else {
                return Ok(out);
            }
        }
    }

    fn lex_number(&mut self) -> Result<Tok, LexError> {
        let (line, col) = (self.line, self.col);
        // `0x`, `0o`, `0b` (D-0035): an integer in base 16, 8 or 2.
        if self.peek() == b'0' && matches!(self.peek_at(1), b'x' | b'o' | b'b') {
            let radix = match self.peek_at(1) {
                b'x' => 16,
                b'o' => 8,
                _ => 2,
            };
            self.bump();
            self.bump();
            if self.peek() == b'_' {
                return Err(sep_error(line, col));
            }
            let text = self.digit_run(|c| c.is_ascii_alphanumeric(), line, col)?;
            let v = if text.is_empty() { None } else { u128::from_str_radix(&text, radix).ok() };
            let Some(v) = v else {
                return Err(LexError { msg: format!("invalid base-{} integer literal", radix), line, col });
            };
            let suffix = self.lex_suffix();
            return Ok(Tok::Int(v, suffix));
        }
        let mut digits = self.digit_run(|c| c.is_ascii_digit(), line, col)?;
        let mut is_float = false;
        if self.peek() == b'.' && self.peek_at(1) == b'_' {
            return Err(sep_error(line, col));
        }
        if self.peek() == b'.' && self.peek_at(1).is_ascii_digit() {
            is_float = true;
            self.bump();
            digits.push('.');
            digits.push_str(&self.digit_run(|c| c.is_ascii_digit(), line, col)?);
        }
        // An exponent (D-0033 (4)): `e` or `E`, an optional sign, digits;
        // it makes the literal a float, as `parse<f64>` reads it.
        if matches!(self.peek(), b'e' | b'E') {
            let signed = matches!(self.peek_at(1), b'+' | b'-');
            let first = if signed { self.peek_at(2) } else { self.peek_at(1) };
            if first == b'_' {
                return Err(sep_error(line, col));
            }
            if first.is_ascii_digit() {
                is_float = true;
                digits.push('e');
                self.bump();
                if signed {
                    if self.peek() == b'-' {
                        digits.push('-');
                    }
                    self.bump();
                }
                digits.push_str(&self.digit_run(|c| c.is_ascii_digit(), line, col)?);
            }
        }
        let suffix = self.lex_suffix();
        if is_float {
            let v: f64 = digits.parse().map_err(|_| LexError {
                msg: "invalid float literal".into(),
                line,
                col,
            })?;
            Ok(Tok::Float(v, suffix))
        } else {
            let v: u128 = digits.parse().map_err(|_| LexError {
                msg: "invalid int literal".into(),
                line,
                col,
            })?;
            Ok(Tok::Int(v, suffix))
        }
    }
}

// A `_` in a number that is not between two digits (D-0043).
fn sep_error(line: usize, col: usize) -> LexError {
    LexError { msg: "a `_` in a number goes between two digits (`1_000`, `0xFFFF_0000`)".into(), line, col }
}
