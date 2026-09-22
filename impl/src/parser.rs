// Recursive-descent parser for CobaltC (spec/22).
use crate::ast::*;
use crate::lexer::{Spanned, Tok};
use std::collections::HashSet;

pub struct ParseError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at {}:{}: {}", self.line, self.col, self.msg)
    }
}

pub struct Parser {
    toks: Vec<Spanned<Tok>>,
    pos: usize,
    // names known to be struct/enum types, for statement disambiguation (4).
    type_names: HashSet<String>,
    // `expect_close_angle` rewrites a `>>` token to `>` in place; a
    // speculative parse that is then abandoned must undo that.
    undo_log: Vec<(usize, Tok)>,
    // Names for the hidden bindings compound assignment introduces.
    fresh: usize,
    // D-0047: inside how many index brackets (`$` is allowed there), and
    // whether the postfix expression being parsed is `&`'s operand (a
    // range `[lo .. hi]` is allowed there, as a slice).
    bracket_depth: usize,
    slice_ok: bool,
}

type PResult<T> = Result<T, ParseError>;

impl Parser {
    pub fn new(toks: Vec<Spanned<Tok>>) -> Self {
        let mut type_names = HashSet::new();
        // Pre-scan for `struct Name` / `enum Name` anywhere, so decl-stmt
        // disambiguation (spec/22 §2, rule 4) has the full item set as the
        // grammar assumes (name resolution runs before statement parsing).
        for i in 0..toks.len() {
            if matches!(toks[i].tok, Tok::Struct | Tok::Enum) {
                if let Some(Spanned { tok: Tok::Ident(n), .. }) = toks.get(i + 1) {
                    type_names.insert(n.clone());
                } else if let Some(Spanned { tok: Tok::Ident(n), .. }) = toks.get(i + 2) {
                    // `resource struct Name`
                    type_names.insert(n.clone());
                }
            }
        }
        Parser { toks, pos: 0, type_names, undo_log: Vec::new(), fresh: 0, bracket_depth: 0, slice_ok: false }
    }

    fn cur(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn line(&self) -> usize {
        self.toks[self.pos].line
    }
    fn col(&self) -> usize {
        self.toks[self.pos].col
    }
    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn err<T>(&self, msg: impl Into<String>) -> PResult<T> {
        Err(ParseError { msg: msg.into(), line: self.line(), col: self.col() })
    }
    // Closes one level of a `<...>` list. `>>` lexes as one `Shr` token
    // (spec/22 §1 has no split rule for this, since it never arises at the
    // *statement* grammar level this spec formalizes, but nested generic
    // instantiations like `rawptr<RcBox<T>>` need it): if the current token
    // is `Shr`, consume only one level by rewriting it to a bare `Gt` in
    // place, so the next enclosing close-angle sees the other half.
    fn expect_close_angle(&mut self) -> PResult<()> {
        match self.cur() {
            Tok::Gt => {
                self.bump();
                Ok(())
            }
            Tok::Shr => {
                self.undo_log.push((self.pos, Tok::Shr));
                self.toks[self.pos].tok = Tok::Gt;
                Ok(())
            }
            other => self.err(format!("expected '>', found {:?}", other.clone())),
        }
    }
    fn expect(&mut self, t: Tok) -> PResult<()> {
        if *self.cur() == t {
            self.bump();
            Ok(())
        } else {
            self.err(format!("expected {:?}, found {:?}", t, self.cur()))
        }
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.cur() == t {
            self.bump();
            true
        } else {
            false
        }
    }
    fn ident(&mut self) -> PResult<String> {
        match self.cur().clone() {
            Tok::Ident(s) => {
                self.bump();
                Ok(s)
            }
            other => self.err(format!("expected identifier, found {:?}", other)),
        }
    }

    pub fn parse_program(&mut self) -> PResult<Program> {
        let mut items = Vec::new();
        while *self.cur() != Tok::Eof {
            items.push(self.parse_item()?);
        }
        Ok(Program { items })
    }

    fn parse_vis(&mut self) -> bool {
        self.eat(&Tok::Export)
    }

    fn parse_item(&mut self) -> PResult<Item> {
        let export = self.parse_vis();
        match self.cur().clone() {
            Tok::Fn => self.parse_fn(export).map(|f| Item::Fn(std::sync::Arc::new(f))),
            Tok::Const => self.parse_const(export).map(|f| Item::Fn(std::sync::Arc::new(f))),
            // `extern "…";` names foreign code to link (spec/20
            // `rule.trust.extern-code`). It declares no name, so it takes
            // no `export`.
            Tok::Extern if matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::Str(_))) => {
                if export {
                    return self.err("`export` on `extern \"…\";`, which declares no name".to_string());
                }
                let line = self.line();
                self.bump();
                let Tok::Str(code) = self.bump() else { unreachable!("checked above") };
                self.expect(Tok::Semi)?;
                Ok(Item::ExternCode(code, line))
            }
            Tok::Extern => self.parse_extern(export).map(|e| Item::Extern(std::sync::Arc::new(e))),
            Tok::Resource | Tok::Struct => self.parse_struct(export).map(|s| Item::Struct(std::sync::Arc::new(s))),
            Tok::Enum => self.parse_enum(export).map(|e| Item::Enum(std::sync::Arc::new(e))),
            Tok::Module => {
                self.bump();
                let line = self.line();
                let name = self.ident()?;
                self.expect(Tok::LBrace)?;
                let mut items = Vec::new();
                while *self.cur() != Tok::RBrace {
                    items.push(self.parse_item()?);
                }
                self.expect(Tok::RBrace)?;
                Ok(Item::Module(export, name, items, line))
            }
            Tok::Import => {
                let line = self.line();
                self.bump();
                let mut segs = vec![self.ident()?];
                while self.eat(&Tok::ColonColon) {
                    segs.push(self.ident()?);
                }
                self.expect(Tok::Semi)?;
                Ok(Item::Import(segs.join("::"), line))
            }
            other => self.err(format!("expected item, found {:?}", other)),
        }
    }

    fn parse_type_params(&mut self) -> PResult<Vec<String>> {
        let mut ps = Vec::new();
        if self.eat(&Tok::Lt) {
            loop {
                ps.push(self.ident()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect_close_angle()?;
        }
        Ok(ps)
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        let mut ps = Vec::new();
        self.expect(Tok::LParen)?;
        if !self.eat(&Tok::RParen) {
            loop {
                let ty = self.parse_type()?;
                let name = self.ident()?;
                ps.push(Param { ty, name });
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect(Tok::RParen)?;
        }
        Ok(ps)
    }

    fn parse_ret_type(&mut self) -> PResult<Type> {
        if self.eat(&Tok::Colon) {
            self.parse_type()
        } else {
            Ok(Type::Void)
        }
    }

    // `const τ NAME = e;` (D-0036).
    fn parse_const(&mut self, export: bool) -> PResult<FnDecl> {
        self.expect(Tok::Const)?;
        let line = self.line();
        let ret = self.parse_type()?;
        let name = self.ident()?;
        self.expect(Tok::Eq)?;
        let e = self.parse_expr()?;
        self.expect(Tok::Semi)?;
        let body = Block { stmts: Vec::new(), tail: Some(Box::new(e)) };
        Ok(FnDecl { export, assoc_type: None, name, type_params: Vec::new(), params: Vec::new(), ret, body, line, is_const: true })
    }

    fn parse_fn(&mut self, export: bool) -> PResult<FnDecl> {
        self.expect(Tok::Fn)?;
        let line = self.line();
        let first = self.ident()?;
        let (assoc_type, name) = if self.eat(&Tok::ColonColon) {
            (Some(first), self.ident()?)
        } else {
            (None, first)
        };
        let type_params = self.parse_type_params()?;
        let params = self.parse_params()?;
        let ret = self.parse_ret_type()?;
        let body = self.parse_block()?;
        Ok(FnDecl { export, assoc_type, name, type_params, params, ret, body, line, is_const: false })
    }

    fn parse_extern(&mut self, export: bool) -> PResult<ExternDecl> {
        self.expect(Tok::Extern)?;
        self.expect(Tok::Fn)?;
        let line = self.line();
        let name = self.ident()?;
        let params = self.parse_params()?;
        let ret = self.parse_ret_type()?;
        self.expect(Tok::Semi)?;
        Ok(ExternDecl { export, name, params, ret, line })
    }

    fn parse_struct(&mut self, export: bool) -> PResult<StructDecl> {
        let resource = self.eat(&Tok::Resource);
        self.expect(Tok::Struct)?;
        let line = self.line();
        let name = self.ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(Tok::LBrace)?;
        let mut fields = Vec::new();
        while *self.cur() != Tok::RBrace {
            let fexport = self.parse_vis();
            let ty = self.parse_type()?;
            let fname = self.ident()?;
            self.expect(Tok::Semi)?;
            fields.push(FieldDecl { export: fexport, ty, name: fname });
        }
        self.expect(Tok::RBrace)?;
        Ok(StructDecl { export, resource, name, type_params, fields, line })
    }

    fn parse_enum(&mut self, export: bool) -> PResult<EnumDecl> {
        let resource = self.eat(&Tok::Resource);
        self.expect(Tok::Enum)?;
        let line = self.line();
        let name = self.ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(Tok::LBrace)?;
        let mut variants = Vec::new();
        while *self.cur() != Tok::RBrace {
            let vname = self.ident()?;
            let payload = if self.eat(&Tok::LParen) {
                let t = self.parse_type()?;
                self.expect(Tok::RParen)?;
                Some(t)
            } else {
                None
            };
            variants.push(VariantDecl { name: vname, payload });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(Tok::RBrace)?;
        Ok(EnumDecl { export, resource, name, type_params, variants, line })
    }

    fn parse_type(&mut self) -> PResult<Type> {
        match self.cur().clone() {
            Tok::Void => {
                self.bump();
                Ok(Type::Void)
            }
            Tok::TypeName(n) => {
                self.bump();
                match n.as_str() {
                    "f32" => Ok(Type::F32),
                    "f64" => Ok(Type::F64),
                    "bool" => Ok(Type::Bool),
                    "str" => Ok(Type::Str),
                    "ref" => {
                        self.expect(Tok::Lt)?;
                        let inner = self.parse_type()?;
                        self.expect(Tok::Comma)?;
                        let m = match self.ident_or_mode()? {
                            m => m,
                        };
                        self.expect_close_angle()?;
                        Ok(Type::Ref(Box::new(inner), m))
                    }
                    "slice" => {
                        self.expect(Tok::Lt)?;
                        let inner = self.parse_type()?;
                        self.expect(Tok::Comma)?;
                        let m = self.ident_or_mode()?;
                        self.expect_close_angle()?;
                        Ok(Type::Slice(Box::new(inner), m))
                    }
                    "rawptr" => {
                        self.expect(Tok::Lt)?;
                        let inner = self.parse_type()?;
                        self.expect_close_angle()?;
                        Ok(Type::Rawptr(Box::new(inner)))
                    }
                    "array" => {
                        self.expect(Tok::Lt)?;
                        let inner = self.parse_type()?;
                        self.expect(Tok::Comma)?;
                        let n = match self.cur().clone() {
                            Tok::Int(v, _) => {
                                self.bump();
                                v
                            }
                            other => return self.err(format!("expected array length, found {:?}", other)),
                        };
                        self.expect_close_angle()?;
                        Ok(Type::Array(Box::new(inner), n))
                    }
                    "handle" => {
                        self.expect(Tok::Lt)?;
                        let inner = self.parse_type()?;
                        self.expect_close_angle()?;
                        Ok(Type::Handle(Box::new(inner)))
                    }
                    "mutex" => {
                        self.expect(Tok::Lt)?;
                        let inner = self.parse_type()?;
                        self.expect_close_angle()?;
                        Ok(Type::Mutex(Box::new(inner)))
                    }
                    "guard" => {
                        self.expect(Tok::Lt)?;
                        let inner = self.parse_type()?;
                        self.expect_close_angle()?;
                        Ok(Type::Guard(Box::new(inner)))
                    }
                    int if IntTy::from_str(int).is_some() => Ok(Type::Int(IntTy::from_str(int).unwrap())),
                    _ => self.err(format!("unknown type constructor {}", n)),
                }
            }
            Tok::Fn => {
                self.bump();
                self.expect(Tok::LParen)?;
                let mut params = Vec::new();
                if !self.eat(&Tok::RParen) {
                    loop {
                        params.push(self.parse_type()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RParen)?;
                }
                let ret = self.parse_ret_type()?;
                Ok(Type::Fn(params, Box::new(ret)))
            }
            Tok::Ident(name) => {
                self.bump();
                // A declared type may be named by a qualified path
                // (`geom::Point`); the resolver rewrites it to the item key.
                let mut name = name;
                while *self.cur() == Tok::ColonColon {
                    self.bump();
                    name = format!("{}::{}", name, self.ident()?);
                }
                let mut args = Vec::new();
                if self.eat(&Tok::Lt) {
                    loop {
                        args.push(self.parse_type()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect_close_angle()?;
                }
                Ok(Type::Named(name, args))
            }
            other => self.err(format!("expected type, found {:?}", other)),
        }
    }

    fn ident_or_mode(&mut self) -> PResult<Mode> {
        match self.cur().clone() {
            Tok::Ident(s) if s == "shared" => {
                self.bump();
                Ok(Mode::Shared)
            }
            Tok::Ident(s) if s == "exclusive" => {
                self.bump();
                Ok(Mode::Exclusive)
            }
            other => self.err(format!("expected shared/exclusive, found {:?}", other)),
        }
    }

    fn is_type_start(&self) -> bool {
        matches!(self.cur(), Tok::TypeName(_) | Tok::Void | Tok::Fn)
            || matches!(self.cur(), Tok::Ident(n) if self.type_names.contains(n))
            || self.qualified_type_ahead()
    }

    // `m::…::Name`, a path whose last segment names a declared struct or
    // enum (spec/22 §3: `type ::= path (…)?`), for disambiguation (4).
    fn qualified_type_ahead(&self) -> bool {
        let mut i = self.pos;
        let mut last: Option<&String>;
        let mut hops = 0;
        loop {
            match self.toks.get(i).map(|s| &s.tok) {
                Some(Tok::Ident(n)) => last = Some(n),
                _ => return false,
            }
            match self.toks.get(i + 1).map(|s| &s.tok) {
                Some(Tok::ColonColon) => {
                    i += 2;
                    hops += 1;
                }
                _ => break,
            }
        }
        hops > 0 && last.map_or(false, |n| self.type_names.contains(n))
    }

    fn parse_block(&mut self) -> PResult<Block> {
        self.expect(Tok::LBrace)?;
        let mut stmts = Vec::new();
        let mut tail = None;
        while *self.cur() != Tok::RBrace {
            if let Some(t) = self.try_parse_stmt(&mut stmts)? {
                tail = Some(t);
                break;
            }
        }
        self.expect(Tok::RBrace)?;
        Ok(Block { stmts, tail })
    }

    // Returns Some(tail expr) if this consumed the block's final expression.
    fn try_parse_stmt(&mut self, stmts: &mut Vec<Stmt>) -> PResult<Option<Box<Expr>>> {
        // block-like statement (no `;` required)
        if self.is_block_like_start() {
            let e = self.parse_block_like()?;
            if *self.cur() == Tok::RBrace {
                return Ok(Some(Box::new(e)));
            }
            stmts.push(Stmt::BlockLike(Box::new(e)));
            return Ok(None);
        }
        // decl-stmt? A leading type-name token is necessary but not
        // sufficient — `Vec { .f = e }` (a struct literal used as the
        // block's trailing expression) starts with the same token as
        // `Vec x = e;` would. Speculatively parse the type and check what
        // follows; an identifier confirms a declaration, anything else
        // (notably `{`) means this was really a struct-literal expression,
        // so roll back and fall through to the expression-statement path.
        if self.is_type_start() {
            let save = self.pos;
            let ty = self.parse_type()?;
            if matches!(self.cur(), Tok::Ident(_)) {
                let name = self.ident()?;
                let init = if self.eat(&Tok::Eq) { Some(Box::new(self.parse_expr()?)) } else { None };
                self.expect(Tok::Semi)?;
                stmts.push(Stmt::Let { ty: Some(ty), name, init });
                return Ok(None);
            }
            self.pos = save;
        }
        if *self.cur() == Tok::Auto {
            self.bump();
            let name = self.ident()?;
            self.expect(Tok::Eq)?;
            let init = self.parse_expr()?;
            self.expect(Tok::Semi)?;
            stmts.push(Stmt::Let { ty: None, name, init: Some(Box::new(init)) });
            return Ok(None);
        }
        // destructuring: path '{' identifier (',' identifier)* '}' '=' expr ';'
        if let Tok::Ident(name) = self.cur().clone() {
            if self.type_names.contains(&name) && self.peek_is_destructure() {
                self.bump();
                self.expect(Tok::LBrace)?;
                let mut fields = vec![self.ident()?];
                while self.eat(&Tok::Comma) {
                    fields.push(self.ident()?);
                }
                self.expect(Tok::RBrace)?;
                self.expect(Tok::Eq)?;
                let init = self.parse_expr()?;
                self.expect(Tok::Semi)?;
                stmts.push(Stmt::Destructure { struct_name: name, fields, init: Box::new(init) });
                return Ok(None);
            }
        }
        // expression statement
        let e = self.parse_expr()?;
        if *self.cur() == Tok::RBrace {
            return Ok(Some(Box::new(e)));
        }
        self.expect(Tok::Semi)?;
        stmts.push(Stmt::Expr(Box::new(e)));
        Ok(None)
    }

    fn peek_is_destructure(&self) -> bool {
        // current token is the type-name ident; look ahead for
        // `{ identifier (, identifier)* }` (a struct literal's fields start with `.`)
        let tok = |k: usize| self.toks.get(self.pos + k).map(|s| &s.tok);
        if !matches!(tok(1), Some(Tok::LBrace)) || !matches!(tok(2), Some(Tok::Ident(_))) {
            return false;
        }
        let mut k = 3;
        while matches!(tok(k), Some(Tok::Comma)) && matches!(tok(k + 1), Some(Tok::Ident(_))) {
            k += 2;
        }
        matches!(tok(k), Some(Tok::RBrace))
    }

    fn is_block_like_start(&self) -> bool {
        matches!(self.cur(), Tok::LBrace | Tok::Unsafe | Tok::If | Tok::While | Tok::For | Tok::Foreach | Tok::Match)
    }

    fn parse_block_like(&mut self) -> PResult<Expr> {
        let line = self.line();
        match self.cur().clone() {
            Tok::LBrace => Ok(Expr { kind: ExprKind::Block(self.parse_block()?), line }),
            Tok::Unsafe => {
                self.bump();
                Ok(Expr { kind: ExprKind::Unsafe(self.parse_block()?), line })
            }
            Tok::If => self.parse_if(),
            Tok::While => {
                self.bump();
                self.expect(Tok::LParen)?;
                let c = self.parse_expr()?;
                self.expect(Tok::RParen)?;
                self.no_empty_body("while")?;
                let b = self.parse_block()?;
                Ok(Expr { kind: ExprKind::While(Box::new(c), b, None), line })
            }
            Tok::For => self.parse_for(),
            Tok::Foreach => self.parse_foreach(),
            Tok::Match => self.parse_match(),
            other => self.err(format!("expected block-like, found {:?}", other)),
        }
    }

    fn parse_if(&mut self) -> PResult<Expr> {
        let line = self.line();
        self.expect(Tok::If)?;
        self.expect(Tok::LParen)?;
        let c = self.parse_expr()?;
        self.expect(Tok::RParen)?;
        self.no_empty_body("if")?;
        let then_b = self.parse_block()?;
        let else_e = if self.eat(&Tok::Else) {
            if *self.cur() == Tok::If {
                Some(Box::new(self.parse_if()?))
            } else {
                let b = self.parse_block()?;
                Some(Box::new(Expr { kind: ExprKind::Block(b), line: self.line() }))
            }
        } else {
            None
        };
        Ok(Expr { kind: ExprKind::If(Box::new(c), then_b, else_e), line })
    }

    fn parse_match(&mut self) -> PResult<Expr> {
        let line = self.line();
        self.expect(Tok::Match)?;
        self.expect(Tok::LParen)?;
        let scrut = self.parse_expr()?;
        self.expect(Tok::RParen)?;
        self.expect(Tok::LBrace)?;
        let mut arms = Vec::new();
        loop {
            if *self.cur() == Tok::RBrace {
                break;
            }
            // A pattern (D-0056): `_`, or a variant with an optional
            // parenthesized pattern for its payload, where an identifier
            // alone is a binder (`modres` makes one that names a variant a
            // unit-variant pattern) and a qualified path is a variant.
            let mut chain: Vec<String> = Vec::new();
            let mut binder: Option<String> = None;
            let mut top = true;
            let mut opened = 0;
            let mut lit: Option<Box<Expr>> = None;
            loop {
                // D-0057: a literal ends the pattern.
                let lline = self.line();
                let literal = match self.cur().clone() {
                    Tok::Int(v, sfx) => {
                        self.bump();
                        Some(Expr { kind: ExprKind::IntLit(v, sfx), line: lline })
                    }
                    Tok::Minus => {
                        self.bump();
                        match self.cur().clone() {
                            Tok::Int(v, sfx) => {
                                self.bump();
                                let inner = Expr { kind: ExprKind::IntLit(v, sfx), line: lline };
                                Some(Expr { kind: ExprKind::Unary(UnOp::Neg, Box::new(inner)), line: lline })
                            }
                            _ => return self.err("expected an integer literal after `-` in a pattern"),
                        }
                    }
                    Tok::Float(..) => return self.err("a float literal cannot be a pattern (D-0057): compare it with `==`"),
                    Tok::True => {
                        self.bump();
                        Some(Expr { kind: ExprKind::BoolLit(true), line: lline })
                    }
                    Tok::False => {
                        self.bump();
                        Some(Expr { kind: ExprKind::BoolLit(false), line: lline })
                    }
                    _ => None,
                };
                if let Some(l) = literal {
                    lit = Some(Box::new(l));
                    break;
                }
                let (path, qualified) = match self.cur().clone() {
                    Tok::Ident(name) => {
                        self.bump();
                        let mut path = vec![name];
                        while self.eat(&Tok::ColonColon) {
                            path.push(self.ident()?);
                        }
                        let q = path.len() > 1;
                        (path, q)
                    }
                    _ => return self.err("expected pattern in match arm"),
                };
                let name = path.last().unwrap().clone();
                if self.eat(&Tok::LParen) {
                    if name == "_" {
                        return self.err("expected pattern in match arm");
                    }
                    chain.push(name);
                    top = false;
                    opened += 1;
                    continue;
                }
                if name == "_" {
                    // `_` alone: the wildcard arm, or a payload not bound.
                } else if top || qualified {
                    chain.push(name);
                } else {
                    binder = Some(name);
                }
                break;
            }
            for _ in 0..opened {
                self.expect(Tok::RParen)?;
            }
            self.expect(Tok::Colon)?;
            let body = self.parse_expr()?;
            let mut chain = chain.into_iter();
            let variant = chain.next();
            let nested: Vec<String> = chain.collect();
            arms.push(Arm { variant, nested, binder, lit, body: Box::new(body) });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(Tok::RBrace)?;
        Ok(Expr { kind: ExprKind::Match(Box::new(scrut), arms), line })
    }

    // ---- expressions ----

    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_assign()
    }

    fn parse_assign(&mut self) -> PResult<Expr> {
        let line = self.line();
        let lhs = self.parse_logic_or()?;
        if self.eat(&Tok::Eq) {
            let rhs = self.parse_assign()?;
            return Ok(Expr { kind: ExprKind::Assign(Box::new(lhs), Box::new(rhs)), line });
        }
        let op = match self.cur() {
            Tok::PlusEq => BinOp::Add,
            Tok::MinusEq => BinOp::Sub,
            Tok::StarEq => BinOp::Mul,
            Tok::SlashEq => BinOp::Div,
            Tok::PercentEq => BinOp::Rem,
            Tok::AmpEq => BinOp::BitAnd,
            Tok::PipeEq => BinOp::BitOr,
            Tok::CaretEq => BinOp::BitXor,
            Tok::ShlEq => BinOp::Shl,
            Tok::ShrEq => BinOp::Shr,
            _ => return Ok(lhs),
        };
        self.bump();
        let rhs = self.parse_assign()?;
        Ok(self.compound_assign(op, lhs, rhs, line))
    }

    // `p op= e` (D-0035): `p = p op e` with `p` evaluated once. A place
    // without a call evaluates to the same place with no effect each
    // time, so it is written out twice; one with a call is reached once,
    // through a hidden exclusive reference:
    // `{ auto t = &mut p; *t = *t op e; }`.
    fn compound_assign(&mut self, op: BinOp, lhs: Expr, rhs: Expr, line: usize) -> Expr {
        let mk = |kind: ExprKind| Expr { kind, line };
        if !has_call(&lhs) {
            let value = mk(ExprKind::Binary(op, Box::new(lhs.clone()), Box::new(rhs)));
            return mk(ExprKind::Assign(Box::new(lhs), Box::new(value)));
        }
        self.fresh += 1;
        let t = format!("__compound{}", self.fresh);
        let path = || Expr { kind: ExprKind::Path(vec![t.clone()], vec![]), line };
        let deref = || Expr { kind: ExprKind::Deref(Box::new(path())), line };
        let borrow = mk(ExprKind::Borrow(Mode::Exclusive, Box::new(lhs)));
        let value = mk(ExprKind::Binary(op, Box::new(deref()), Box::new(rhs)));
        let write = mk(ExprKind::Assign(Box::new(deref()), Box::new(value)));
        mk(ExprKind::Block(Block { stmts: vec![Stmt::Let { ty: None, name: t.clone(), init: Some(Box::new(borrow)) }, Stmt::Expr(Box::new(write))], tail: None }))
    }

    // C's `for (…);`, `while (…);` and `if (…);` have an empty body, and
    // the block after them runs once, unconditionally. Here a body is
    // always a block, so the `;` is a syntax error; say what it is.
    fn no_empty_body(&self, what: &str) -> PResult<()> {
        if *self.cur() == Tok::Semi {
            return self.err(format!(
                "`;` after `{w} (…)`: {a} `{w}` body is a block, so `{w} (…);` is not an empty {k}; remove the `;`",
                w = what,
                a = if what == "if" { "an" } else { "a" },
                k = if what == "if" { "statement" } else { "loop" }
            ));
        }
        Ok(())
    }

    // `for (init; cond; step) body` (D-0035): `{ init; while (cond) body }`
    // with `step` run after the body and on `continue`. Each part may be
    // left out; a missing `cond` is `true`.
    fn parse_for(&mut self) -> PResult<Expr> {
        let line = self.line();
        self.expect(Tok::For)?;
        self.expect(Tok::LParen)?;
        let mut stmts = Vec::new();
        if !self.eat(&Tok::Semi) {
            if let Some(t) = self.try_parse_stmt(&mut stmts)? {
                return self.err(format!("expected `;` after a for loop's initializer, found {:?}", t.kind));
            }
        }
        let cond = if *self.cur() == Tok::Semi { Expr { kind: ExprKind::BoolLit(true), line } } else { self.parse_expr()? };
        self.expect(Tok::Semi)?;
        let step = if *self.cur() == Tok::RParen { None } else { Some(Box::new(self.parse_expr()?)) };
        self.expect(Tok::RParen)?;
        self.no_empty_body("for")?;
        let body = self.parse_block()?;
        let lp = Expr { kind: ExprKind::While(Box::new(cond), body, step), line };
        Ok(Expr { kind: ExprKind::Block(Block { stmts, tail: Some(Box::new(lp)) }), line })
    }

    // `foreach (x in c) body` and `foreach (k, x in c) body` (D-0042,
    // `rule.control.foreach`). The form is written: `c` is consumed, `&c`
    // borrowed shared, `&mut c` borrowed exclusive. It becomes a counted
    // `for` over hidden bindings, and what depends on the collection's
    // type (`Vec`, array, `HashSet`, `HashMap`) is left to the `$each_*`
    // intrinsics (src/each.rs), which each tool expands once it knows
    // that type:
    //
    //   borrowed:  { auto $c = &c;  for (usize $i = 0; $i < $each_len($c); $i += 1)
    //                { auto k = $each_key($c, $i); auto x = $each_at($c, $i, n); body } }
    //   consumed:  { auto $e = c;  auto $d = $each_drain($e, n);
    //                for (usize $i = 0; $i < $each_len($d); $i += 1)
    //                { auto k = $each_take_key($d, $i); auto x = $each_take($d, $i, n); body } }
    //
    // (`$each_at_mut` for `&mut c`; `n` is the number of names.)
    fn parse_foreach(&mut self) -> PResult<Expr> {
        let line = self.line();
        self.expect(Tok::Foreach)?;
        self.expect(Tok::LParen)?;
        let mut names = vec![self.ident()?];
        while names.len() < 3 && self.eat(&Tok::Comma) {
            names.push(self.ident()?);
        }
        match self.cur().clone() {
            Tok::Ident(w) if w == "in" => {
                self.bump();
            }
            other => return self.err(format!("expected `in` in a foreach, found {:?}", other)),
        }
        let coll = self.parse_expr()?;
        self.expect(Tok::RParen)?;
        self.no_empty_body("foreach")?;
        let body = self.parse_block()?;
        self.fresh += 1;
        let n = self.fresh;
        let mk = |kind: ExprKind| Expr { kind, line };
        let path = |s: &str| mk(ExprKind::Path(vec![s.to_string()], vec![]));
        let call = |f: &str, args: Vec<Expr>| mk(ExprKind::Call(Box::new(path(f)), args));
        let (c, i, e, d) = (format!("$c{}", n), format!("$i{}", n), format!("$e{}", n), format!("$d{}", n));
        let mode = match &coll.kind {
            ExprKind::Borrow(m, _) => Some(m.clone()),
            _ => None,
        };
        let mut stmts = Vec::new();
        let (holder, key_fn, elem_fn) = match mode {
            Some(m) => {
                stmts.push(Stmt::Let { ty: None, name: c.clone(), init: Some(Box::new(coll)) });
                (c.clone(), "$each_key", if m == Mode::Exclusive { "$each_at_mut" } else { "$each_at" })
            }
            None => {
                stmts.push(Stmt::Let { ty: None, name: e.clone(), init: Some(Box::new(coll)) });
                let arity = mk(ExprKind::IntLit(names.len() as u128, None));
                stmts.push(Stmt::Let { ty: None, name: d.clone(), init: Some(Box::new(call("$each_drain", vec![path(&e), arity]))) });
                (d.clone(), "$each_take_key", "$each_take")
            }
        };
        stmts.push(Stmt::Let { ty: Some(Type::Int(IntTy::Usize)), name: i.clone(), init: Some(Box::new(mk(ExprKind::IntLit(0, None)))) });
        let cond = mk(ExprKind::Binary(BinOp::Lt, Box::new(path(&i)), Box::new(call("$each_len", vec![path(&holder)]))));
        let step = mk(ExprKind::Assign(Box::new(path(&i)), Box::new(mk(ExprKind::Binary(BinOp::Add, Box::new(path(&i)), Box::new(mk(ExprKind::IntLit(1, None))))))));
        let mut inner = Vec::new();
        // Three names (a map only, D-0044): the position, then key and value.
        if names.len() == 3 {
            inner.push(Stmt::Let { ty: None, name: names[0].clone(), init: Some(Box::new(path(&i))) });
        }
        if names.len() >= 2 {
            inner.push(Stmt::Let { ty: None, name: names[names.len() - 2].clone(), init: Some(Box::new(call(key_fn, vec![path(&holder), path(&i)]))) });
        }
        let arity = mk(ExprKind::IntLit(names.len() as u128, None));
        inner.push(Stmt::Let { ty: None, name: names.last().unwrap().clone(), init: Some(Box::new(call(elem_fn, vec![path(&holder), path(&i), arity]))) });
        inner.push(Stmt::BlockLike(Box::new(mk(ExprKind::Block(body)))));
        let lp = mk(ExprKind::While(Box::new(cond), Block { stmts: inner, tail: None }, Some(Box::new(step))));
        Ok(mk(ExprKind::Block(Block { stmts, tail: Some(Box::new(lp)) })))
    }

    fn parse_logic_or(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_logic_and()?;
        while self.eat(&Tok::OrOr) {
            let rhs = self.parse_logic_and()?;
            lhs = Expr { kind: ExprKind::Binary(BinOp::Or, Box::new(lhs), Box::new(rhs)), line };
        }
        Ok(lhs)
    }
    fn parse_logic_and(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_compare()?;
        while self.eat(&Tok::AndAnd) {
            let rhs = self.parse_compare()?;
            lhs = Expr { kind: ExprKind::Binary(BinOp::And, Box::new(lhs), Box::new(rhs)), line };
        }
        Ok(lhs)
    }
    fn parse_compare(&mut self) -> PResult<Expr> {
        let line = self.line();
        let lhs = self.parse_shift()?;
        let op = match self.cur() {
            Tok::EqEq => BinOp::Eq,
            Tok::NotEq => BinOp::Ne,
            Tok::Lt => BinOp::Lt,
            Tok::Le => BinOp::Le,
            Tok::Gt => BinOp::Gt,
            Tok::Ge => BinOp::Ge,
            _ => return Ok(lhs),
        };
        self.bump();
        let rhs = self.parse_shift()?;
        Ok(Expr { kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), line })
    }
    fn parse_shift(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_bitor()?;
        loop {
            let op = match self.cur() {
                Tok::Shl => BinOp::Shl,
                Tok::Shr => BinOp::Shr,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_bitor()?;
            lhs = Expr { kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), line };
        }
        Ok(lhs)
    }
    fn parse_bitor(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_bitxor()?;
        while self.eat(&Tok::Pipe) {
            let rhs = self.parse_bitxor()?;
            lhs = Expr { kind: ExprKind::Binary(BinOp::BitOr, Box::new(lhs), Box::new(rhs)), line };
        }
        Ok(lhs)
    }
    fn parse_bitxor(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_bitand()?;
        while self.eat(&Tok::Caret) {
            let rhs = self.parse_bitand()?;
            lhs = Expr { kind: ExprKind::Binary(BinOp::BitXor, Box::new(lhs), Box::new(rhs)), line };
        }
        Ok(lhs)
    }
    fn parse_bitand(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_additive()?;
        while *self.cur() == Tok::Amp {
            self.bump();
            let rhs = self.parse_additive()?;
            lhs = Expr { kind: ExprKind::Binary(BinOp::BitAnd, Box::new(lhs), Box::new(rhs)), line };
        }
        Ok(lhs)
    }
    fn parse_additive(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.cur() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_multiplicative()?;
            lhs = Expr { kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), line };
        }
        Ok(lhs)
    }
    fn parse_multiplicative(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.cur() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Rem,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_unary()?;
            lhs = Expr { kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), line };
        }
        Ok(lhs)
    }
    fn parse_unary(&mut self) -> PResult<Expr> {
        let line = self.line();
        match self.cur().clone() {
            Tok::Minus => {
                self.bump();
                Ok(Expr { kind: ExprKind::Unary(UnOp::Neg, Box::new(self.parse_unary()?)), line })
            }
            Tok::Not => {
                self.bump();
                Ok(Expr { kind: ExprKind::Unary(UnOp::Not, Box::new(self.parse_unary()?)), line })
            }
            Tok::Tilde => {
                self.bump();
                Ok(Expr { kind: ExprKind::Unary(UnOp::BitNot, Box::new(self.parse_unary()?)), line })
            }
            Tok::Amp => {
                self.bump();
                let m = if self.eat(&Tok::Mut) { Mode::Exclusive } else { Mode::Shared };
                let saved = self.slice_ok;
                self.slice_ok = true;
                let operand = self.parse_unary();
                self.slice_ok = saved;
                let operand = operand?;
                // `&a[lo .. hi]`: the borrow is a slice (D-0047).
                if let ExprKind::SliceOf(_, b, lo, hi) = operand.kind {
                    return Ok(Expr { kind: ExprKind::SliceOf(m, b, lo, hi), line });
                }
                Ok(Expr { kind: ExprKind::Borrow(m, Box::new(operand)), line })
            }
            Tok::Star => {
                self.bump();
                Ok(Expr { kind: ExprKind::Deref(Box::new(self.parse_unary()?)), line })
            }
            _ => self.parse_postfix(),
        }
    }
    fn parse_postfix(&mut self) -> PResult<Expr> {
        let line = self.line();
        let slice_ok = std::mem::replace(&mut self.slice_ok, false);
        let mut e = self.parse_primary()?;
        loop {
            match self.cur().clone() {
                Tok::Dot => {
                    self.bump();
                    let f = self.ident()?;
                    e = Expr { kind: ExprKind::Field(Box::new(e), f), line };
                }
                Tok::LBracket => {
                    self.bump();
                    self.bracket_depth += 1;
                    let idx = self.parse_expr();
                    let hi = if idx.is_ok() && self.eat(&Tok::DotDot) { Some(self.parse_expr()) } else { None };
                    self.bracket_depth -= 1;
                    let idx = idx?;
                    if let Some(hi) = hi {
                        let hi = hi?;
                        self.expect(Tok::RBracket)?;
                        // A range makes a slice, which is a borrow: `&a[i .. j]`.
                        if !slice_ok {
                            return self.err("a slice is a borrow: write `&a[i .. j]` or `&mut a[i .. j]`".to_string());
                        }
                        return Ok(Expr { kind: ExprKind::SliceOf(Mode::Shared, Box::new(e), Box::new(idx), Box::new(hi)), line });
                    }
                    self.expect(Tok::RBracket)?;
                    e = Expr { kind: ExprKind::Index(Box::new(e), Box::new(idx)), line };
                }
                Tok::LParen => {
                    self.bump();
                    let mut args = Vec::new();
                    if !self.eat(&Tok::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.eat(&Tok::Comma) {
                                break;
                            }
                        }
                        self.expect(Tok::RParen)?;
                    }
                    e = Expr { kind: ExprKind::Call(Box::new(e), args), line };
                }
                Tok::Question => {
                    self.bump();
                    e = Expr { kind: ExprKind::Propagate(Box::new(e)), line };
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn parse_path_and_targs(&mut self) -> PResult<(Vec<String>, Vec<Type>)> {
        let mut segs = vec![self.ident()?];
        while *self.cur() == Tok::ColonColon {
            // lookahead: could be ::name or generic args already consumed
            self.bump();
            segs.push(self.ident()?);
        }
        let mut targs = Vec::new();
        if *self.cur() == Tok::Lt {
            if let Some(ts) = self.try_type_args() {
                targs = ts;
                // `Vec<i32>::new`: further path segments after an
                // instantiated type (spec/22 §2, disambiguation (1)).
                while *self.cur() == Tok::ColonColon {
                    self.bump();
                    segs.push(self.ident()?);
                }
            }
        }
        Ok((segs, targs))
    }

    // spec/22 §2, disambiguation (1): the token sequence
    // `identifier '<' type (',' type)* '>'` is always a generic-argument
    // list, regardless of what follows; anything else after `<` is a
    // comparison. Decided by actually parsing a type list speculatively
    // and backtracking (position and any `>>` split) if it fails.
    fn try_type_args(&mut self) -> Option<Vec<Type>> {
        let save_pos = self.pos;
        let save_undo = self.undo_log.len();
        self.bump(); // `<`
        let mut ts = Vec::new();
        let ok = loop {
            match self.parse_type() {
                Ok(t) => ts.push(t),
                Err(_) => break false,
            }
            if self.eat(&Tok::Comma) {
                continue;
            }
            break self.expect_close_angle().is_ok();
        };
        if ok {
            return Some(ts);
        }
        while self.undo_log.len() > save_undo {
            let (pos, tok) = self.undo_log.pop().unwrap();
            self.toks[pos].tok = tok;
        }
        self.pos = save_pos;
        None
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let line = self.line();
        match self.cur().clone() {
            Tok::Dollar => {
                if self.bracket_depth == 0 {
                    return self.err("`$` is the length of what is being indexed; it is written only inside `[…]`".to_string());
                }
                self.bump();
                Ok(Expr { kind: ExprKind::Dollar, line })
            }
            Tok::Int(v, s) => {
                self.bump();
                Ok(Expr { kind: ExprKind::IntLit(v, s), line })
            }
            Tok::Float(v, s) => {
                self.bump();
                Ok(Expr { kind: ExprKind::FloatLit(v, s), line })
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr { kind: ExprKind::StrLit(s), line })
            }
            Tok::Bytes(bytes) => {
                // spec/22 byte-literal: sugar for `[Array-Construct]` on the
                // byte values, `array<u8, N>`.
                self.bump();
                let items = bytes
                    .into_iter()
                    .map(|b| Expr { kind: ExprKind::IntLit(b as u128, Some("u8".to_string())), line })
                    .collect();
                Ok(Expr { kind: ExprKind::ArrayLit(items), line })
            }
            Tok::True => {
                self.bump();
                Ok(Expr { kind: ExprKind::BoolLit(true), line })
            }
            Tok::False => {
                self.bump();
                Ok(Expr { kind: ExprKind::BoolLit(false), line })
            }
            Tok::LParen => {
                self.bump();
                if self.eat(&Tok::RParen) {
                    return Ok(Expr { kind: ExprKind::Unit, line });
                }
                let e = self.parse_expr()?;
                self.expect(Tok::RParen)?;
                Ok(Expr { kind: ExprKind::Paren(Box::new(e)), line })
            }
            Tok::LBracket => self.parse_bracket(),
            Tok::Move => {
                self.bump();
                self.parse_closure(true)
            }
            Tok::LBrace | Tok::Unsafe | Tok::If | Tok::While | Tok::For | Tok::Foreach | Tok::Match => self.parse_block_like(),
            Tok::Return => {
                self.bump();
                if matches!(self.cur(), Tok::Semi | Tok::RBrace) {
                    Ok(Expr { kind: ExprKind::Return(None), line })
                } else {
                    Ok(Expr { kind: ExprKind::Return(Some(Box::new(self.parse_expr()?))), line })
                }
            }
            Tok::Break => {
                self.bump();
                Ok(Expr { kind: ExprKind::Break, line })
            }
            Tok::Continue => {
                self.bump();
                Ok(Expr { kind: ExprKind::Continue, line })
            }
            Tok::Ident(_) => {
                let (segs, targs) = self.parse_path_and_targs()?;
                if *self.cur() == Tok::LBrace && self.brace_is_struct_lit() {
                    self.bump();
                    let mut fields = Vec::new();
                    while *self.cur() != Tok::RBrace {
                        self.expect(Tok::Dot)?;
                        let f = self.ident()?;
                        self.expect(Tok::Eq)?;
                        let e = self.parse_expr()?;
                        fields.push((f, e));
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RBrace)?;
                    return Ok(Expr { kind: ExprKind::StructLit(segs, targs, fields), line });
                }
                Ok(Expr { kind: ExprKind::Path(segs, targs), line })
            }
            other => self.err(format!("expected expression, found {:?}", other)),
        }
    }

    fn brace_is_struct_lit(&self) -> bool {
        // `{` immediately followed by `.` (or `}` for empty) is a struct
        // literal; `{` followed by a bare identifier then `}` is
        // destructuring (statement-only) or just a block - in expression
        // position a struct literal is the only brace-field form, so any
        // `{ .f = ... }` or empty `{}` counts, but a plain block `{ stmt }`
        // must not be misparsed. We only reach here right after a path, so
        // require the immediate next token to be `.` or `}`.
        matches!(self.toks.get(self.pos + 1).map(|s| &s.tok), Some(Tok::Dot))
            || matches!(self.toks.get(self.pos + 1).map(|s| &s.tok), Some(Tok::RBrace))
    }

    fn parse_bracket(&mut self) -> PResult<Expr> {
        let line = self.line();
        // disambiguate array literal vs closure: scan for matching `]`
        // followed by `(`.
        let close = self.find_matching_bracket(self.pos);
        let is_closure = close
            .and_then(|c| self.toks.get(c + 1))
            .map(|s| s.tok == Tok::LParen)
            .unwrap_or(false);
        if is_closure {
            self.parse_closure(false)
        } else {
            self.bump(); // [
            let mut items = Vec::new();
            if !self.eat(&Tok::RBracket) {
                loop {
                    items.push(self.parse_expr()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RBracket)?;
            }
            Ok(Expr { kind: ExprKind::ArrayLit(items), line })
        }
    }

    fn find_matching_bracket(&self, open: usize) -> Option<usize> {
        let mut depth = 0i32;
        let mut i = open;
        loop {
            match self.toks.get(i).map(|s| &s.tok) {
                Some(Tok::LBracket) => depth += 1,
                Some(Tok::RBracket) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                Some(Tok::Eof) | None => return None,
                _ => {}
            }
            i += 1;
        }
    }

    fn parse_closure(&mut self, is_move: bool) -> PResult<Expr> {
        let line = self.line();
        self.expect(Tok::LBracket)?;
        let mut captures = Vec::new();
        if !self.eat(&Tok::RBracket) {
            loop {
                captures.push(self.ident()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect(Tok::RBracket)?;
        }
        let params = self.parse_params()?;
        let body = self.parse_block()?;
        Ok(Expr { kind: ExprKind::Closure { is_move, captures, params, body }, line })
    }
}

pub fn parse(src: &str) -> Result<Program, String> {
    let toks = crate::lexer::Lexer::new(src)
        .tokenize()
        .map_err(|e| format!("lex error at {}:{}: {}", e.line, e.col, e.msg))?;
    let mut p = Parser::new(toks);
    p.parse_program().map_err(|e| e.to_string())
}

// Whether `e` contains a call, whose evaluation may have an effect.
pub fn has_call(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Call(..) => true,
        ExprKind::Unary(_, a) | ExprKind::Borrow(_, a) | ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Propagate(a) | ExprKind::Paren(a) => has_call(a),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => has_call(a) || has_call(b),
        ExprKind::Path(..) | ExprKind::IntLit(..) | ExprKind::FloatLit(..) | ExprKind::StrLit(_) | ExprKind::BoolLit(_) | ExprKind::Unit => false,
        _ => true,
    }
}
