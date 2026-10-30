// Reads the fragment's concrete syntax: what `print.rs` writes, and
// hand-written cases in the same subset. Anything outside the subset is an
// error naming the line, never a guess.

use crate::ast::*;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Int(i64),
    Str(String),
    Punct(&'static str),
}

struct Lexed {
    toks: Vec<(Tok, usize)>,
}

const PUNCTS: [&str; 19] = ["::", "{", "}", "(", ")", ";", ",", "..", ".", "=", "+", ">", "<", "&", "*", ":", "[", "]", "_"];

fn lex(src: &str) -> Result<Lexed, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut line = 1;
    let mut toks = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c.is_ascii_digit() {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            toks.push((Tok::Int(src[s..i].parse().map_err(|_| format!("line {}: bad number", line))?), line));
        } else if c.is_ascii_alphabetic() || (c == b'_' && i + 1 < b.len() && b[i + 1].is_ascii_alphanumeric()) {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            toks.push((Tok::Ident(src[s..i].to_string()), line));
        } else if c == b'"' {
            let s = i + 1;
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            toks.push((Tok::Str(src[s..i].to_string()), line));
            i += 1;
        } else {
            let p = PUNCTS.iter().find(|p| src[i..].starts_with(**p)).ok_or_else(|| format!("line {}: unexpected `{}`", line, c as char))?;
            toks.push((Tok::Punct(p), line));
            i += p.len();
        }
    }
    Ok(Lexed { toks })
}

struct P {
    toks: Vec<(Tok, usize)>,
    i: usize,
}

type R<T> = Result<T, String>;

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|t| &t.0)
    }
    fn peek2(&self) -> Option<&Tok> {
        self.toks.get(self.i + 1).map(|t| &t.0)
    }
    fn line(&self) -> usize {
        self.toks.get(self.i).or(self.toks.last()).map_or(0, |t| t.1)
    }
    fn err<T>(&self, what: &str) -> R<T> {
        Err(format!("line {}: {} (found {:?})", self.line(), what, self.peek()))
    }
    fn is(&self, p: &str) -> bool {
        matches!(self.peek(), Some(Tok::Punct(q)) if *q == p)
    }
    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(q)) if q == w)
    }
    fn eat(&mut self, p: &str) -> R<()> {
        if self.is(p) {
            self.i += 1;
            Ok(())
        } else {
            self.err(&format!("expected `{}`", p))
        }
    }
    fn eat_word(&mut self, w: &str) -> R<()> {
        if self.is_word(w) {
            self.i += 1;
            Ok(())
        } else {
            self.err(&format!("expected `{}`", w))
        }
    }
    fn ident(&mut self) -> R<String> {
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.i += 1;
                Ok(s)
            }
            _ => self.err("expected a name"),
        }
    }

    fn mode(&mut self) -> R<Mode> {
        let w = self.ident()?;
        match w.as_str() {
            "shared" => Ok(Mode::Shared),
            "exclusive" => Ok(Mode::Excl),
            _ => Err(format!("line {}: expected a mode", self.line())),
        }
    }

    fn ty(&mut self) -> R<Ty> {
        let w = self.ident()?;
        match w.as_str() {
            "i32" => Ok(Ty::I32),
            "void" => Ok(Ty::Unit),
            "Hold" => Ok(Ty::Hold),
            "Job" => Ok(Ty::Job),
            "array" => {
                self.eat("<")?;
                self.eat_word("i32")?;
                self.eat(",")?;
                match self.peek() {
                    Some(Tok::Int(2)) => self.i += 1,
                    _ => return self.err("the fragment's arrays are array<i32, 2>"),
                }
                self.eat(">")?;
                Ok(Ty::Arr)
            }
            "handle" => {
                self.eat("<")?;
                let t = self.ty()?;
                self.eat(">")?;
                Ok(Ty::Handle(Box::new(t)))
            }
            "slice" => {
                self.eat("<")?;
                self.eat_word("i32")?;
                self.eat(",")?;
                let m = self.mode()?;
                self.eat(">")?;
                Ok(Ty::Slice(m))
            }
            // `fn() : τ`
            "fn" => {
                self.eat("(")?;
                self.eat(")")?;
                self.eat(":")?;
                let t = self.ty()?;
                Ok(Ty::Fn(Box::new(t)))
            }
            "Channel" => {
                self.eat("<")?;
                let t = self.ty()?;
                self.eat(">")?;
                Ok(Ty::Chan(Box::new(t)))
            }
            "mutex" => {
                self.eat("<")?;
                self.eat_word("i32")?;
                self.eat(">")?;
                Ok(Ty::Mutex)
            }
            "Atomic" => {
                self.eat("<")?;
                self.eat_word("i32")?;
                self.eat(">")?;
                Ok(Ty::Atomic)
            }
            "Vec" => {
                self.eat("<")?;
                let t = self.ty()?;
                self.eat(">")?;
                match t {
                    Ty::I32 => Ok(Ty::Vec),
                    t if t == Ty::refi(Mode::Excl) => Ok(Ty::VecR),
                    Ty::Handle(r) if *r == Ty::Unit => Ok(Ty::VecH),
                    _ => Err(format!("line {}: Vec<{}> is outside the fragment", self.line(), t.text())),
                }
            }
            "ref" => {
                self.eat("<")?;
                let t = self.ty()?;
                self.eat(",")?;
                let m = self.mode()?;
                self.eat(">")?;
                Ok(Ty::Ref(Box::new(t), m))
            }
            _ => Err(format!("line {}: type `{}` is outside the fragment", self.line(), w)),
        }
    }

    fn starts_type(&self) -> bool {
        match (self.peek(), self.peek2()) {
            (Some(Tok::Ident(w)), Some(Tok::Ident(_))) => matches!(w.as_str(), "i32" | "Hold" | "Job"),
            (Some(Tok::Ident(w)), Some(Tok::Punct("<"))) => matches!(w.as_str(), "ref" | "Vec" | "mutex" | "Channel" | "handle" | "slice" | "array" | "Atomic"),
            (Some(Tok::Ident(w)), Some(Tok::Punct("("))) => w == "fn",
            _ => false,
        }
    }

    fn block(&mut self) -> R<Block> {
        self.eat("{")?;
        let mut b = Block::default();
        loop {
            if self.is("}") {
                b.end_line = self.line();
                self.i += 1;
                return Ok(b);
            }
            let line = self.line();
            // A trailing expression: an expression followed by `}`.
            let save = self.i;
            if !self.is("{") && !self.is("_") && !self.is_word("if") && !self.is_word("while") && !self.is_word("match") && !self.is_word("auto") && !self.is_word("printf") && !self.starts_type() {
                let e = self.expr()?;
                if self.is("}") {
                    b.tail = Some(e);
                    b.tail_line = line;
                    continue;
                }
                self.i = save;
            }
            b.stmts.push(self.stmt()?);
        }
    }

    fn stmt(&mut self) -> R<Stmt> {
        let line = self.line();
        let kind = if self.is("{") {
            StmtKind::Block(self.block()?)
        } else if self.is("_") {
            self.i += 1;
            self.eat("=")?;
            let e = self.expr()?;
            self.eat(";")?;
            StmtKind::Discard(e)
        } else if self.is_word("match") {
            self.i += 1;
            self.eat("(")?;
            let scrut = self.expr()?;
            self.eat(")")?;
            self.eat("{")?;
            self.eat_word("Some")?;
            self.eat("(")?;
            let binder = self.ident()?;
            self.eat(")")?;
            self.eat(":")?;
            let some = self.block()?;
            self.eat(",")?;
            self.eat_word("None")?;
            self.eat(":")?;
            let none = self.block()?;
            self.eat(",")?;
            self.eat("}")?;
            StmtKind::Match { scrut, binder, some, none }
        } else if self.is_word("if") {
            self.i += 1;
            self.eat("(")?;
            let cond = self.expr()?;
            self.eat(")")?;
            let then = self.block()?;
            let els = if self.is_word("else") {
                self.i += 1;
                Some(self.block()?)
            } else {
                None
            };
            StmtKind::If { cond, then, els }
        } else if self.is_word("break") || self.is_word("continue") {
            let k = if self.is_word("break") { StmtKind::Break } else { StmtKind::Continue };
            self.i += 1;
            self.eat(";")?;
            k
        } else if self.is_word("while") {
            self.i += 1;
            self.eat("(")?;
            let cond = self.expr()?;
            self.eat(")")?;
            let body = self.block()?;
            StmtKind::While { cond, body }
        } else if self.is_word("auto") || self.starts_type() {
            let ty = if self.is_word("auto") {
                self.i += 1;
                None
            } else {
                Some(self.ty()?)
            };
            let name = self.ident()?;
            self.eat("=")?;
            let init = self.expr()?;
            self.eat(";")?;
            StmtKind::Let { name, ty, init }
        } else if self.is_word("printf") {
            self.i += 1;
            self.eat("(")?;
            match self.peek() {
                Some(Tok::Str(s)) if s == "%v\\n" => self.i += 1,
                _ => return self.err("expected \"%v\\n\""),
            }
            self.eat(",")?;
            let e = self.expr()?;
            self.eat(")")?;
            self.eat(";")?;
            StmtKind::Print(e)
        } else {
            let e = self.expr()?;
            if self.is("=") {
                self.i += 1;
                let v = self.expr()?;
                self.eat(";")?;
                StmtKind::Assign { place: e, value: v }
            } else {
                self.eat(";")?;
                StmtKind::Expr(e)
            }
        };
        Ok(Stmt { kind, line })
    }

    fn expr(&mut self) -> R<Expr> {
        let a = self.add()?;
        if self.is(">") {
            self.i += 1;
            let b = self.add()?;
            return Ok(Expr::Gt(Box::new(a), Box::new(b)));
        }
        Ok(a)
    }

    fn add(&mut self) -> R<Expr> {
        let mut a = self.unary()?;
        while self.is("+") {
            self.i += 1;
            let b = self.unary()?;
            a = Expr::Add(Box::new(a), Box::new(b));
        }
        Ok(a)
    }

    fn unary(&mut self) -> R<Expr> {
        if self.is("&") {
            self.i += 1;
            let m = if self.is_word("mut") {
                self.i += 1;
                Mode::Excl
            } else {
                Mode::Shared
            };
            let e = self.unary()?;
            // `&_m v[lo .. hi]` is a slice (`[Slice-Form]`), not a borrow
            // of a place.
            if let Expr::Call(f, xs) = &e {
                if f == "$range" {
                    let (Expr::Var(v), Expr::Int(lo), Expr::Int(hi)) = (&xs[0], &xs[1], &xs[2]) else { unreachable!() };
                    return Ok(Expr::Slice(m, v.clone(), *lo, *hi));
                }
            }
            return Ok(Expr::Borrow(m, Box::new(e)));
        }
        if self.is("*") {
            self.i += 1;
            let e = self.unary()?;
            return Ok(Expr::Deref(Box::new(e)));
        }
        self.postfix()
    }

    fn args(&mut self) -> R<Vec<Expr>> {
        self.eat("(")?;
        let mut xs = Vec::new();
        while !self.is(")") {
            xs.push(self.expr()?);
            if self.is(",") {
                self.i += 1;
            }
        }
        self.i += 1;
        Ok(xs)
    }

    fn postfix(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.is(".") {
                self.i += 1;
                self.eat_word("r")?;
                e = Expr::Field(Box::new(e));
            } else if self.is("[") {
                let Expr::Var(v) = e else { return self.err("an index applies to a Vec binding") };
                self.i += 1;
                let k = match self.peek() {
                    Some(Tok::Int(k)) => *k,
                    _ => return self.err("expected a literal index"),
                };
                self.i += 1;
                if self.is("..") {
                    // A range, only as `&`'s operand (`[Slice-Not-Borrowed]`).
                    self.i += 1;
                    let hi = match self.peek() {
                        Some(Tok::Int(k)) => *k,
                        _ => return self.err("expected a literal bound"),
                    };
                    self.i += 1;
                    self.eat("]")?;
                    e = Expr::Call("$range".into(), vec![Expr::Var(v), Expr::Int(k), Expr::Int(hi)]);
                    continue;
                }
                self.eat("]")?;
                e = Expr::Index(v, k);
            } else if self.is("(") && matches!(e, Expr::Deref(_)) {
                // `(*f)()`: a call of the closure `*f` reaches.
                let xs = self.args()?;
                if !xs.is_empty() {
                    return self.err("a closure of the fragment takes no arguments");
                }
                e = Expr::CallVal(Box::new(e));
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> R<Expr> {
        match self.peek().cloned() {
            Some(Tok::Int(k)) => {
                self.i += 1;
                Ok(Expr::Int(k))
            }
            Some(Tok::Punct("(")) => {
                self.i += 1;
                let e = self.expr()?;
                self.eat(")")?;
                Ok(e)
            }
            Some(Tok::Punct("[")) if self.closure_ahead() => self.closure(false),
            Some(Tok::Punct("[")) => {
                self.i += 1;
                let mut xs = Vec::new();
                while !self.is("]") {
                    xs.push(self.expr()?);
                    if self.is(",") {
                        self.i += 1;
                    }
                }
                self.i += 1;
                Ok(Expr::ArrLit(xs))
            }
            Some(Tok::Ident(w)) => {
                self.i += 1;
                match w.as_str() {
                    "Vec" | "Mutex" | "Channel" | "Atomic" => {
                        let w = w.clone();
                        self.eat("::")?;
                        let f = self.ident()?;
                        let xs = self.args()?;
                        Ok(Expr::Call(format!("{}::{}", w, f), xs))
                    }
                    "move" => self.closure(true),
                    "Job" => {
                        self.eat("{")?;
                        self.eat(".")?;
                        self.eat_word("h")?;
                        self.eat("=")?;
                        let e = self.expr()?;
                        self.eat("}")?;
                        Ok(Expr::JobLit(Box::new(e)))
                    }
                    "Hold" => {
                        self.eat("{")?;
                        self.eat(".")?;
                        self.eat_word("r")?;
                        self.eat("=")?;
                        let e = self.expr()?;
                        self.eat("}")?;
                        Ok(Expr::HoldLit(Box::new(e)))
                    }
                    "spawn" => {
                        self.eat("(")?;
                        if self.is("[") || self.is_word("move") {
                            let c = self.primary()?;
                            self.eat(")")?;
                            return Ok(Expr::SpawnClo(Box::new(c)));
                        }
                        let f = self.ident()?;
                        let mut xs = Vec::new();
                        while self.is(",") {
                            self.i += 1;
                            xs.push(self.expr()?);
                        }
                        self.eat(")")?;
                        Ok(Expr::Spawn(f, xs))
                    }
                    "join" => {
                        self.eat("(")?;
                        let t = self.ident()?;
                        self.eat(")")?;
                        Ok(Expr::Join(t))
                    }
                    _ if self.is("(") => {
                        let xs = self.args()?;
                        Ok(Expr::Call(w, xs))
                    }
                    _ => Ok(Expr::Var(w)),
                }
            }
            _ => self.err("expected an expression"),
        }
    }

    // `[x, …](`: a closure's capture list, not an array literal.
    fn closure_ahead(&self) -> bool {
        let mut j = self.i + 1;
        loop {
            match self.toks.get(j).map(|t| &t.0) {
                Some(Tok::Ident(_)) | Some(Tok::Punct(",")) => j += 1,
                Some(Tok::Punct("]")) => return matches!(self.toks.get(j + 1).map(|t| &t.0), Some(Tok::Punct("("))),
                _ => return false,
            }
        }
    }

    // `move? [x, …]() (: τ)? { … }` (the `move` already read when `mv`).
    fn closure(&mut self, mv: bool) -> R<Expr> {
        self.eat("[")?;
        let mut caps = Vec::new();
        while !self.is("]") {
            caps.push(self.ident()?);
            if self.is(",") {
                self.i += 1;
            }
        }
        self.i += 1;
        self.eat("(")?;
        self.eat(")")?;
        let ret = if self.is(":") {
            self.i += 1;
            self.ty()?
        } else {
            Ty::Unit
        };
        let body = self.block()?;
        Ok(Expr::Closure(Box::new(Clo { id: 0, mv, caps, ret, body })))
    }

    fn func(&mut self) -> R<Func> {
        let line = self.line();
        self.eat_word("fn")?;
        let name = self.ident()?;
        self.eat("(")?;
        let mut params = Vec::new();
        while !self.is(")") {
            let t = self.ty()?;
            let n = self.ident()?;
            params.push((t, n));
            if self.is(",") {
                self.i += 1;
            }
        }
        self.i += 1;
        let ret = if self.is(":") {
            self.i += 1;
            self.ty()?
        } else {
            Ty::Unit
        };
        let body = self.block()?;
        Ok(Func { name, params, ret, body, line })
    }
}

pub fn parse(src: &str) -> R<Program> {
    let Lexed { toks } = lex(src)?;
    let mut p = P { toks, i: 0 };
    let mut prog = Program::default();
    while p.peek().is_some() {
        if p.is_word("import") {
            p.i += 1;
            p.eat_word("std")?;
            p.eat(";")?;
        } else if p.is_word("struct") && matches!(p.peek2(), Some(Tok::Ident(w)) if w == "Job") {
            p.i += 2;
            p.eat("{")?;
            if p.ty()? != Ty::Handle(Box::new(Ty::Unit)) {
                return p.err("Job's field is handle<void>");
            }
            p.eat_word("h")?;
            p.eat(";")?;
            p.eat("}")?;
            prog.job = true;
        } else if p.is_word("static") {
            // D-0258: `static Atomic<i32> NAME = k;`
            p.i += 1;
            p.eat_word("Atomic")?;
            p.eat("<")?;
            p.eat_word("i32")?;
            p.eat(">")?;
            let name = p.ident()?;
            p.eat("=")?;
            let neg = p.is("-");
            if neg {
                p.i += 1;
            }
            let k = match p.peek() {
                Some(Tok::Int(k)) => *k,
                _ => return p.err("a static's first value is a literal"),
            };
            p.i += 1;
            p.eat(";")?;
            prog.statics.push((name, if neg { -k } else { k }));
        } else if p.is_word("struct") {
            p.i += 1;
            p.eat_word("Hold")?;
            p.eat("{")?;
            if p.ty()? != Ty::refi(Mode::Excl) {
                return p.err("Hold's field is ref<i32, exclusive>");
            }
            p.eat_word("r")?;
            p.eat(";")?;
            p.eat("}")?;
            prog.hold = true;
        } else {
            prog.funcs.push(p.func()?);
        }
    }
    if prog.func("main").is_none() {
        return Err("no `fn main()`".into());
    }
    resolve(&mut prog);
    Ok(prog)
}

const BUILTINS: [&str; 20] = [
    "Vec::new", "Vec::push", "Vec::pop", "Vec::index_exclusive", "Mutex::new", "lock", "Channel::new", "Channel::send", "Channel::recv", "Channel::close", "swap", "replace", "$range", "print",
    "Atomic::new", "Atomic::load", "Atomic::store", "Atomic::fetch_add", "Atomic::wait", "Atomic::notify_all",
];

// After the whole program is read: `c()` naming no function is a call of
// the closure binding `c`, and `spawn(c)` a thread running it; closure
// literals are numbered in source order.
fn resolve(prog: &mut Program) {
    let fnames: Vec<String> = prog.funcs.iter().map(|f| f.name.clone()).collect();
    let mut next = 0;
    fn ex(e: &mut Expr, fnames: &[String], next: &mut usize) {
        match e {
            Expr::Call(f, xs) if xs.is_empty() && !fnames.contains(f) && !BUILTINS.contains(&f.as_str()) => {
                *e = Expr::CallVal(Box::new(Expr::Var(f.clone())));
            }
            Expr::Spawn(f, xs) if xs.is_empty() && !fnames.contains(f) => {
                *e = Expr::SpawnClo(Box::new(Expr::Var(f.clone())));
            }
            Expr::Call(_, xs) | Expr::Spawn(_, xs) | Expr::ArrLit(xs) => xs.iter_mut().for_each(|x| ex(x, fnames, next)),
            Expr::Closure(c) => {
                c.id = *next;
                *next += 1;
                bl(&mut c.body, fnames, next);
            }
            Expr::Deref(a) | Expr::Field(a) | Expr::Borrow(_, a) | Expr::HoldLit(a) | Expr::JobLit(a) | Expr::CallVal(a) | Expr::SpawnClo(a) => ex(a, fnames, next),
            Expr::Add(a, b) | Expr::Gt(a, b) => {
                ex(a, fnames, next);
                ex(b, fnames, next);
            }
            _ => {}
        }
    }
    fn st(s: &mut Stmt, fnames: &[String], next: &mut usize) {
        match &mut s.kind {
            StmtKind::Let { init, .. } => ex(init, fnames, next),
            StmtKind::Assign { place, value } => {
                ex(place, fnames, next);
                ex(value, fnames, next);
            }
            StmtKind::Expr(e) | StmtKind::Print(e) | StmtKind::Discard(e) => ex(e, fnames, next),
            StmtKind::Block(b) => bl(b, fnames, next),
            StmtKind::If { cond, then, els } => {
                ex(cond, fnames, next);
                bl(then, fnames, next);
                if let Some(b) = els {
                    bl(b, fnames, next);
                }
            }
            StmtKind::While { cond, body } => {
                ex(cond, fnames, next);
                bl(body, fnames, next);
            }
            StmtKind::Match { scrut, some, none, .. } => {
                ex(scrut, fnames, next);
                bl(some, fnames, next);
                bl(none, fnames, next);
            }            StmtKind::Break | StmtKind::Continue => {}
        }
    }
    fn bl(b: &mut Block, fnames: &[String], next: &mut usize) {
        for s in b.stmts.iter_mut() {
            st(s, fnames, next);
        }
        if let Some(t) = &mut b.tail {
            ex(t, fnames, next);
        }
    }
    for f in prog.funcs.iter_mut() {
        bl(&mut f.body, &fnames, &mut next);
    }
}
