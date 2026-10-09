// Recursive-descent parser for CobaltC (spec/22).
use crate::ast::*;
use crate::lexer::{Spanned, Tok};
use crate::fxhash::{HashMap, HashSet};

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
    // D-0074: constants whose initializer is an integer literal, by name,
    // for array lengths (`array<T, N>`, `[v; N]`); `None` when the name is
    // declared as two different values.
    const_lengths: HashMap<String, Option<u128>>,
    // D-0105: `a::b::N` for each `const` declared with an integer literal
    // inside modules, its value and whether it is exported.
    module_lengths: HashMap<String, (u128, bool)>,
    // The module the parser is inside (`a::b`), for resolving `m::N`.
    module_path: Vec<String>,
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
    // D-0066: local constants. The names in scope in the function being
    // parsed, innermost last: a local constant maps to the hidden module
    // constant it becomes, a local variable to `None`. Each local constant
    // is hoisted into its module as `NAME$n` (`hoisted`, drained after
    // the item that holds it), and every use in its scope renamed to it.
    // D-0069: a local type is in the same scopes under the key `::NAME`
    // (types and values are separate namespaces), mapped to its hidden
    // item name.
    scopes: Vec<HashMap<String, Option<String>>>,
    hoisted: Vec<Item>,
    local_consts: usize,
    fn_type_params: Vec<String>,
    // Parsing a local constant's initializer; the first name in it that
    // is a local variable or a type parameter (not a constant).
    in_const_init: bool,
    const_error: Option<String>,
    // D-0131: the declaration regions open in the function being parsed,
    // innermost last: each name declared there, with its line. A block
    // opens one, unless it is the body governed by binders (parameters, a
    // loop's or a pattern's names), which open it for the block to join:
    // `join_at` is the position of that block's `{`.
    regions: Vec<HashMap<String, usize>>,
    join_at: Option<usize>,
    local_error: Option<String>,
}

type PResult<T> = Result<T, ParseError>;

impl Parser {
    pub fn new(toks: Vec<Spanned<Tok>>) -> Self {
        let mut type_names = HashSet::default();
        // Pre-scan for `struct Name` / `enum Name` anywhere, so decl-stmt
        // disambiguation (spec/22 §2, rule 4) has the full item set as the
        // grammar assumes (name resolution runs before statement parsing).
        for i in 0..toks.len() {
            if matches!(toks[i].tok, Tok::Struct | Tok::Enum | Tok::Bitstruct) {
                if let Some(Spanned { tok: Tok::Ident(n), .. }) = toks.get(i + 1) {
                    type_names.insert(n.clone());
                } else if let Some(Spanned { tok: Tok::Ident(n), .. }) = toks.get(i + 2) {
                    // `resource struct Name`
                    type_names.insert(n.clone());
                }
            }
        }
        // D-0074, D-0126: `const τ NAME = <constant expression>;` anywhere,
        // for lengths — an integer literal, or an expression of literals
        // and such constants (`N * 2 + 1`), folded here; to a fixpoint,
        // so a constant may name one declared after it.
        let mut const_lengths: HashMap<String, Option<u128>> = HashMap::default();
        for _pass in 0..8 {
            let mut changed = false;
            for i in 0..toks.len() {
                if toks[i].tok != Tok::Const {
                    continue;
                }
                let Some(j) = (i + 1..(i + 10).min(toks.len())).find(|&j| toks[j].tok == Tok::Eq) else { continue };
                let Some(Tok::Ident(n)) = toks.get(j - 1).map(|t| &t.tok) else { continue };
                let Some(k) = (j + 1..toks.len()).find(|&k| toks[k].tok == Tok::Semi) else { continue };
                let known = &const_lengths;
                let lookup = |segs: &[String]| -> Option<i128> {
                    if segs.len() == 1 {
                        match known.get(&segs[0]) {
                            Some(Some(v)) => i128::try_from(*v).ok(),
                            _ => None,
                        }
                    } else {
                        None
                    }
                };
                let Some(v) = fold_const_tokens(&toks[j + 1..k], &lookup).filter(|v| *v >= 0).map(|v| v as u128) else { continue };
                match const_lengths.get(n) {
                    Some(Some(old)) if *old != v => {
                        const_lengths.insert(n.clone(), None);
                        changed = true;
                    }
                    Some(_) => {}
                    None => {
                        const_lengths.insert(n.clone(), Some(v));
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        // D-0105: the same constants under their modules' names.
        let mut module_lengths: HashMap<String, (u128, bool)> = HashMap::default();
        {
            let mut stack: Vec<(String, usize)> = Vec::new();
            let mut depth = 0usize;
            let mut i = 0;
            while i < toks.len() {
                match &toks[i].tok {
                    Tok::Module => {
                        if let (Some(Tok::Ident(n)), Some(Tok::LBrace)) = (toks.get(i + 1).map(|t| &t.tok), toks.get(i + 2).map(|t| &t.tok)) {
                            depth += 1;
                            stack.push((n.clone(), depth));
                            i += 3;
                            continue;
                        }
                    }
                    Tok::LBrace => depth += 1,
                    Tok::RBrace => {
                        if stack.last().map_or(false, |(_, d)| *d == depth) {
                            stack.pop();
                        }
                        depth = depth.saturating_sub(1);
                    }
                    Tok::Const if !stack.is_empty() && depth == stack.last().unwrap().1 => {
                        if let Some(j) = (i + 1..(i + 10).min(toks.len())).find(|&j| toks[j].tok == Tok::Eq) {
                            if let (Some(Tok::Ident(n)), Some(k)) = (toks.get(j - 1).map(|t| &t.tok), (j + 1..toks.len()).find(|&k| toks[k].tok == Tok::Semi)) {
                                let path: Vec<&str> = stack.iter().map(|(m, _)| m.as_str()).collect();
                                // D-0126: names inside a module's constant are
                                // its module's constants (walking outward), then
                                // the root's.
                                let ml = &module_lengths;
                                let cl = &const_lengths;
                                let lookup = |segs: &[String]| -> Option<i128> {
                                    let q = segs.join("::");
                                    for up in (0..=path.len()).rev() {
                                        let key = if up == 0 { q.clone() } else { format!("{}::{}", path[..up].join("::"), q) };
                                        if let Some((v, _)) = ml.get(&key) {
                                            return i128::try_from(*v).ok();
                                        }
                                    }
                                    if segs.len() == 1 {
                                        if let Some(Some(v)) = cl.get(&segs[0]) {
                                            return i128::try_from(*v).ok();
                                        }
                                    }
                                    None
                                };
                                if let Some(v) = fold_const_tokens(&toks[j + 1..k], &lookup).filter(|v| *v >= 0) {
                                    let exported = i > 0 && toks[i - 1].tok == Tok::Export;
                                    module_lengths.insert(format!("{}::{}", path.join("::"), n), (v as u128, exported));
                                }
                            }
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        Parser {
            toks,
            pos: 0,
            module_lengths,
            module_path: Vec::new(),
            const_lengths,
            type_names,
            undo_log: Vec::new(),
            fresh: 0,
            bracket_depth: 0,
            slice_ok: false,
            scopes: Vec::new(),
            hoisted: Vec::new(),
            local_consts: 0,
            fn_type_params: Vec::new(),
            in_const_init: false,
            const_error: None,
            regions: Vec::new(),
            join_at: None,
            local_error: None,
        }
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
            other => self.err(format!("expected `>`, found {}", other.clone())),
        }
    }
    fn expect(&mut self, t: Tok) -> PResult<()> {
        if *self.cur() == t {
            self.bump();
            Ok(())
        } else {
            self.err(format!("expected {}, found {}", t, self.cur()))
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
            Tok::TypeName(w) => self.err(reserved_name(&w)),
            other => {
                // `fn f(i32 match)`: a keyword where a name belongs.
                let shown = format!("{}", other);
                let word = shown.trim_matches('`');
                if !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                    return self.err(format!("`{}` is a keyword, so it cannot be a name; choose another name", word));
                }
                self.err(format!("expected a name, found {}", other))
            }
        }
    }

    pub fn parse_program(&mut self) -> PResult<Program> {
        let mut items = Vec::new();
        while *self.cur() != Tok::Eof {
            items.push(self.parse_item()?);
            items.extend(self.hoisted.drain(..));
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
            // D-0146: a declaration of foreign code's signature is an
            // unchecked claim, so it is written `unsafe extern fn`, as Rust
            // 2024 requires of an `extern` block; calls stay `unsafe` too.
            Tok::Unsafe if matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::Extern)) => {
                self.bump();
                if matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::Str(_))) {
                    return self.err("`unsafe` on `extern \"…\";`, which declares no signature: write `extern \"…\";`".to_string());
                }
                self.parse_extern(export).map(|e| Item::Extern(std::sync::Arc::new(e)))
            }
            Tok::Extern => self.err("an `extern fn` declaration is written `unsafe extern fn`: its signature is an unchecked claim about foreign code (D-0146)".to_string()),
            Tok::Resource | Tok::Struct => self.parse_struct(export).map(|s| Item::Struct(std::sync::Arc::new(s))),
            Tok::Bitstruct => self.parse_bitstruct(export).map(|s| Item::Struct(std::sync::Arc::new(s))),
            Tok::Enum => self.parse_enum(export).map(|e| Item::Enum(std::sync::Arc::new(e))),
            Tok::Module => {
                self.bump();
                let line = self.line();
                let name = self.ident()?;
                self.expect(Tok::LBrace)?;
                self.module_path.push(name.clone());
                let mut items = Vec::new();
                while *self.cur() != Tok::RBrace {
                    match self.parse_item() {
                        Ok(it) => items.push(it),
                        Err(e) => {
                            self.module_path.pop();
                            return Err(e);
                        }
                    }
                    items.extend(self.hoisted.drain(..));
                }
                self.module_path.pop();
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
                Ok(Item::Import(export, segs.join("::"), line))
            }
            // The C habits a newcomer brings to file scope, each with what
            // CobaltC has instead.
            Tok::Ident(w) if w == "static_assert" => self.err("`static_assert` is a statement: write it inside a function body, such as `main` (D-0052)".to_string()),
            Tok::Ident(w) if matches!(w.as_str(), "int" | "char" | "long" | "short" | "unsigned" | "float" | "double" | "void") => {
                self.err(format!("`{}` is C; a function is `fn name(T param, …) : T {{ … }}`, and the types are `i32`, `u8`, `f64`, … (`spec/22`)", w))
            }
            Tok::TypeName(w) => self.err(format!(
                "found `{}` at file scope: a program has no global variables; write `const {} NAME = …;` for a constant, or keep the value in `main` and pass it on",
                w, w
            )),
            Tok::Semi => self.err("a stray `;` at file scope: a declaration ends at its `}` and needs no `;` after it (C's `struct P { … };`)".to_string()),
            Tok::Auto => self.err("found `auto` at file scope: a program has no global variables; write `const T NAME = …;` for a constant, or keep the value in `main` and pass it on".to_string()),
            Tok::Ident(w) if w == "static" => self.err("`static` is C: a program has no global variables; write `const T NAME = …;` for a constant, or keep the value in `main` and pass it on".to_string()),
            other => self.err(format!("expected a declaration (`fn`, `struct`, `enum`, `const`, `module`, `import`, …), found {}", other)),
        }
    }

    fn parse_type_params(&mut self) -> PResult<Vec<String>> {
        let (ps, bounds) = self.parse_bounded_type_params()?;
        if bounds.iter().any(|b| b.is_some()) {
            return self.err("a bound (`T: ordered`) is written on a function's type parameters, not on a struct's or an enum's; bound the functions that use the operations");
        }
        Ok(ps)
    }

    // `<T, U: ordered>` (D-0090): each type parameter with its bound, if any.
    fn parse_bounded_type_params(&mut self) -> PResult<(Vec<String>, Vec<Option<Bound>>)> {
        let mut ps = Vec::new();
        let mut bs = Vec::new();
        if self.eat(&Tok::Lt) {
            loop {
                ps.push(self.ident()?);
                if self.eat(&Tok::Colon) {
                    let w = self.ident()?;
                    match Bound::from_name(&w) {
                        Some(b) => bs.push(Some(b)),
                        None => return self.err(format!("`{}` is not a bound; a type parameter's bound is one of `eq`, `ordered`, `number`, `integer`, `clone`", w)),
                    }
                } else {
                    bs.push(None);
                }
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect_close_angle()?;
        }
        Ok((ps, bs))
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        let mut ps = Vec::new();
        self.expect(Tok::LParen)?;
        if !self.eat(&Tok::RParen) {
            loop {
                let ty = self.parse_type()?;
                self.no_c_array_suffix()?;
                let name = self.ident()?;
                ps.push(Param { ty, name });
                // D-0098: a trailing comma before `)`.
                if !self.eat(&Tok::Comma) || *self.cur() == Tok::RParen {
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

    // A constant's type, or `auto` (D-0068): synthesized from its
    // initializer by `consts` (`Void` until then).
    fn parse_const_type(&mut self) -> PResult<(Type, bool)> {
        if self.eat(&Tok::Auto) {
            Ok((Type::Void, true))
        } else {
            Ok((self.parse_type()?, false))
        }
    }

    // `T[N] name` as C writes it, after a type where a name follows (a
    // constant, a field, a parameter): name the form CobaltC uses (D-0072).
    fn no_c_array_suffix(&mut self) -> PResult<()> {
        if *self.cur() == Tok::LBracket {
            return self.err("a fixed-size array's type is written `array<T, N>`, as in `array<i32, 4> xs = [1, 2, 3, 4];`".to_string());
        }
        Ok(())
    }

    // `const τ NAME = e;` (D-0036).
    fn parse_const(&mut self, export: bool) -> PResult<FnDecl> {
        self.expect(Tok::Const)?;
        let line = self.line();
        let (ret, auto_type) = self.parse_const_type()?;
        self.no_c_array_suffix()?;
        let name = self.ident()?;
        self.expect(Tok::Eq)?;
        let e = self.parse_expr()?;
        self.expect(Tok::Semi)?;
        let body = Block { stmts: Vec::new(), tail: Some(Box::new(e)) };
        Ok(FnDecl { export, assoc_type: None, name, type_params: Vec::new(), type_bounds: Vec::new(), params: Vec::new(), ret, body, line, is_const: true, auto_type, derived: false })
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
        // D-0069: `fn T::f` in a block, `T` a local type: the function of
        // `T`'s hidden item. Its body sees the enclosing function's local
        // types and constants, not its variables or type parameters.
        let assoc_type = match assoc_type {
            Some(t) => Some(match self.lookup_type(&t) {
                Some(h) => h,
                None if !self.scopes.is_empty() => return self.err(format!("a function declared in a block must belong to a type declared there, not `{}`", t)),
                None => t,
            }),
            None => None,
        };
        let (type_params, type_bounds) = self.parse_bounded_type_params()?;
        let params = self.parse_params()?;
        let ret = self.parse_ret_type()?;
        let saved_tps = std::mem::replace(&mut self.fn_type_params, type_params.clone());
        let visible: Vec<HashMap<String, Option<String>>> = self.scopes.iter().map(|m| m.iter().filter(|(_, v)| v.is_some()).map(|(k, v)| (k.clone(), v.clone())).collect()).collect();
        let saved_scopes = std::mem::replace(&mut self.scopes, visible);
        self.scopes.push(params.iter().map(|p| (p.name.clone(), None)).collect());
        let saved_regions = std::mem::take(&mut self.regions);
        self.open_binder_region(params.iter().map(|p| p.name.as_str()), line);
        let body = self.parse_block();
        self.regions = saved_regions;
        self.join_at = None;
        self.scopes = saved_scopes;
        self.fn_type_params = saved_tps;
        let body = body?;
        Ok(FnDecl { export, assoc_type, name, type_params, type_bounds, params, ret, body, line, is_const: false, auto_type: false, derived: false })
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
            self.no_c_array_suffix()?;
            let fname = self.ident()?;
            self.expect(Tok::Semi)?;
            fields.push(FieldDecl { export: fexport, ty, name: fname, width: None });
        }
        self.expect(Tok::RBrace)?;
        Ok(StructDecl { export, resource, name, type_params, fields, line, bits: None })
    }

    // D-0118: `bitstruct Name : uN { f : w; … }` — a struct whose fields are
    // bit fields of the backing unsigned type, low bit first, filling it
    // exactly. Each field's type is the smallest unsigned type holding
    // its width. The widths are checked here: a declaration that does not
    // fill its backing type is a syntax error, as the rule says.
    fn parse_bitstruct(&mut self, export: bool) -> PResult<StructDecl> {
        self.expect(Tok::Bitstruct)?;
        let line = self.line();
        let name = self.ident()?;
        self.expect(Tok::Colon)?;
        let backing = match self.parse_type()? {
            Type::Int(t) if !t.signed() => t,
            other => return self.err(format!("a `bitstruct` is backed by an unsigned integer type (`u8` to `u128`), not `{}`", other)),
        };
        let total = backing.bitwidth(crate::value::ADDR_WIDTH);
        self.expect(Tok::LBrace)?;
        let mut fields = Vec::new();
        let mut used: u32 = 0;
        while *self.cur() != Tok::RBrace {
            let fexport = self.parse_vis();
            let fname = self.ident()?;
            self.expect(Tok::Colon)?;
            let w = match self.cur().clone() {
                Tok::Int(v, None) if v >= 1 && v <= 128 => {
                    self.bump();
                    v as u32
                }
                _ => return self.err(format!("field `{}` of `{}` needs a width in bits, 1 to {}, after the `:`", fname, name, total)),
            };
            self.expect(Tok::Semi)?;
            if w > total {
                return self.err(format!("field `{}` of `{}` is {} bits wide, but `{}` has only {}", fname, name, w, name, total));
            }
            let fty = match w {
                1..=8 => IntTy::U8,
                9..=16 => IntTy::U16,
                17..=32 => IntTy::U32,
                33..=64 => IntTy::U64,
                _ => IntTy::U128,
            };
            used += w;
            fields.push(FieldDecl { export: fexport, ty: Type::Int(fty), name: fname, width: Some(w) });
        }
        self.expect(Tok::RBrace)?;
        if used != total {
            return self.err(format!("the fields of `bitstruct {}` fill {} of its {} bits; they must fill it exactly (name the unused bits, e.g. `pad : {};`)", name, used, total, if total > used { total - used } else { 0 }));
        }
        Ok(StructDecl { export, resource: false, name, type_params: Vec::new(), fields, line, bits: Some(backing) })
    }

    fn parse_enum(&mut self, export: bool) -> PResult<EnumDecl> {
        let resource = self.eat(&Tok::Resource);
        self.expect(Tok::Enum)?;
        let line = self.line();
        let name = self.ident()?;
        let type_params = self.parse_type_params()?;
        // D-0106: `enum Op : u8 { … }`: each variant has a code of that
        // integer type (`A = 1`, or one more than the one before, from 0).
        let code_ty = if self.eat(&Tok::Colon) {
            let t = self.parse_type()?;
            if !matches!(t, Type::Int(_)) {
                return self.err(format!("an enum's code type is an integer type, not `{}`", t));
            }
            if !type_params.is_empty() {
                return self.err("a generic enum cannot have codes".to_string());
            }
            if !self.scopes.is_empty() {
                return self.err("an enum declared in a function cannot have codes; declare it at module level".to_string());
            }
            Some(t)
        } else {
            None
        };
        let mut codes: Vec<i128> = Vec::new();
        self.expect(Tok::LBrace)?;
        let mut variants = Vec::new();
        while *self.cur() != Tok::RBrace {
            let vname = self.ident()?;
            let payload = if self.eat(&Tok::LParen) {
                let t = self.parse_type()?;
                if *self.cur() == Tok::Comma {
                    // D-0073: say what to do instead.
                    return self.err("a variant carries one payload; to carry several values, make the payload a struct".to_string());
                }
                self.expect(Tok::RParen)?;
                Some(t)
            } else {
                None
            };
            if code_ty.is_some() && payload.is_some() {
                return self.err("an enum with codes has no payloads".to_string());
            }
            if *self.cur() == Tok::Eq {
                if code_ty.is_none() {
                    return self.err(format!(
                        "a variant's number needs the enum's code type (D-0106): `enum {} : u8 {{ {} = 1, … }}`",
                        name, vname
                    ));
                }
                self.bump();
                let neg = self.eat(&Tok::Minus);
                let v = match self.cur().clone() {
                    Tok::Int(v, None) => {
                        self.bump();
                        v as i128
                    }
                    other => return self.err(format!("a variant's code is an integer literal, found {}", other)),
                };
                codes.push(if neg { -v } else { v });
            } else if code_ty.is_some() {
                codes.push(codes.last().map_or(0, |c| c + 1));
            }
            if let Some(i) = codes.len().checked_sub(1).filter(|_| code_ty.is_some()) {
                if codes[..i].contains(&codes[i]) {
                    return self.err(format!("two variants of `{}` have the code {}", name, codes[i]));
                }
            }
            variants.push(VariantDecl { name: vname, payload });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(Tok::RBrace)?;
        // D-0106: `Op::code(Op v) : T` and `Op::from_code(T n) : Option<Op>`,
        // written as CobaltC and parsed here, hoisted beside the enum.
        if let Some(t) = &code_ty {
            let vis = if export { "export " } else { "" };
            let to: Vec<String> = variants.iter().zip(&codes).map(|(v, c)| format!("{}::{} : {}", name, v.name, c)).collect();
            let from: Vec<String> = variants.iter().zip(&codes).map(|(v, c)| format!("{} : Some({}::{})", c, name, v.name)).collect();
            let text = format!(
                "{vis}fn {n}::code({n} v) : {t} {{ match (v) {{ {to} }} }}\n{vis}fn {n}::from_code({t} c) : Option<{n}> {{ match (c) {{ {from}, _ : None }} }}\n",
                vis = vis, n = name, t = t, to = to.join(", "), from = from.join(", ")
            );
            let mut toks = match crate::lexer::Lexer::new(&text).tokenize() {
                Ok(t) => t,
                Err(e) => return self.err(format!("internal: enum codes: {}", e.msg)),
            };
            for tk in toks.iter_mut() {
                tk.line = line;
            }
            let mut sub = Parser::new(toks);
            sub.type_names = self.type_names.clone();
            sub.type_names.insert(name.clone());
            while *sub.cur() != Tok::Eof {
                let it = sub.parse_item()?;
                self.hoisted.push(it);
            }
        }
        Ok(EnumDecl { export, resource, name, type_params, variants, line })
    }

    fn parse_type(&mut self) -> PResult<Type> {
        match self.cur().clone() {
            Tok::Void => {
                self.bump();
                Ok(Type::Void)
            }
            Tok::TypeName(n) => {
                // `array(p)`: no type is followed by `(`; a call of
                // something named as a built-in type.
                if matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::LParen)) {
                    return self.err(reserved_call(&n));
                }
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
                        let n = self.array_length()?;
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
                        if !self.eat(&Tok::Comma) || *self.cur() == Tok::RParen {
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
                let mut name = match self.lookup_type(&name) {
                    Some(h) if *self.cur() != Tok::ColonColon => h,
                    _ => name,
                };
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
            Tok::LParen if matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::RParen)) => {
                self.err("the unit type is written `void` (`()` is its value): `Result<void, E>`, `fn f() : void`")
            }
            Tok::LParen => self.err("CobaltC has no tuple types (D-0073): declare a struct with named fields"),
            other => self.err(format!("expected a type, found {}", other)),
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
            other => self.err(format!("expected `shared` or `exclusive`, found {}", other)),
        }
    }

    // A module's constant named `m::N` (D-0105), found from the current
    // module outward and visible here (exported, or used inside its module).
    fn module_length(&self, segs: &[String]) -> Result<u128, String> {
        let q = segs.join("::");
        let mut found = None;
        for up in (0..=self.module_path.len()).rev() {
            let key = if up == 0 { q.clone() } else { format!("{}::{}", self.module_path[..up].join("::"), q) };
            if let Some(&(v, exported)) = self.module_lengths.get(&key) {
                found = Some((v, exported, up));
                break;
            }
        }
        match found {
            Some((v, exported, up)) => {
                let decl_mod: Vec<&str> = if up == 0 { segs[..segs.len() - 1].iter().map(|s| s.as_str()).collect() } else { self.module_path[..up].iter().map(|s| s.as_str()).chain(segs[..segs.len() - 1].iter().map(|s| s.as_str())).collect() };
                let inside = self.module_path.len() >= decl_mod.len() && self.module_path[..decl_mod.len()].iter().map(|s| s.as_str()).eq(decl_mod.iter().copied());
                if !exported && !inside {
                    return Err(format!("`{}` is not exported from its module, so it cannot be an array length here", q));
                }
                Ok(v)
            }
            None => Err(format!("an array length is a constant expression; `{}` is not a constant of a module in this program", q)),
        }
    }

    // An array length (`array<T, N>`, `[v; N]`): a constant expression —
    // an integer literal, the name of a constant (D-0074; `m::N`, D-0105),
    // or `+ - * / % & | ^ << >> ~ ( )` over them (D-0126) — folded here.
    // Outside parentheses `>`, `>>`, `]`, `;` and `,` end it.
    fn array_length(&mut self) -> PResult<u128> {
        let start = self.pos;
        let mut depth = 0usize;
        let mut end = start;
        while end < self.toks.len() {
            match &self.toks[end].tok {
                Tok::LParen => depth += 1,
                Tok::RParen => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                Tok::Gt | Tok::Shr | Tok::RBracket | Tok::Semi | Tok::Comma | Tok::LBrace | Tok::RBrace | Tok::Ge if depth == 0 => break,
                _ => {}
            }
            end += 1;
        }
        if end == start {
            return self.err(format!("expected an array length (a constant expression), found {}", self.cur()));
        }
        let problem_cell = std::cell::RefCell::new(None::<String>);
        let value = {
            let toks = &self.toks[start..end];
            let cl = &self.const_lengths;
            let me = &*self;
            let lookup = |segs: &[String]| -> Option<i128> {
                if segs.len() == 1 {
                    return match cl.get(&segs[0]) {
                        Some(Some(v)) => i128::try_from(*v).ok(),
                        Some(None) => {
                            *problem_cell.borrow_mut() = Some(format!("`{}` is declared as constants of different values; write the length as a number", segs[0]));
                            None
                        }
                        None => {
                            *problem_cell.borrow_mut() = Some(format!("`{}` is not a constant whose value is known before the program runs (D-0074, D-0126)", segs[0]));
                            None
                        }
                    };
                }
                match me.module_length(segs) {
                    Ok(v) => i128::try_from(v).ok(),
                    Err(m) => {
                        *problem_cell.borrow_mut() = Some(m);
                        None
                    }
                }
            };
            fold_const_tokens(toks, &lookup)
        };
        let problem = problem_cell.into_inner();
        match value {
            Some(v) if v >= 0 => {
                self.pos = end;
                Ok(v as u128)
            }
            Some(_) => self.err("an array length is not negative".to_string()),
            None => match problem {
                Some(m) => self.err(m),
                None => self.err("an array length is a constant expression: an integer literal, a constant, or `+ - * / % & | ^ << >> ~ ( )` over them, with no division by zero or overflow".to_string()),
            },
        }
    }

    // `T [ N ] name`: a C-style array declaration.
    fn c_array_decl_ahead(&self) -> bool {
        let at = |k: usize| self.toks.get(self.pos + k).map(|s| &s.tok);
        let ty = matches!(at(0), Some(Tok::TypeName(_))) || matches!(at(0), Some(Tok::Ident(n)) if self.type_names.contains(n));
        ty && matches!(at(1), Some(Tok::LBracket)) && matches!(at(2), Some(Tok::Int(..))) && matches!(at(3), Some(Tok::RBracket)) && matches!(at(4), Some(Tok::Ident(_)))
    }

    // `Mutex<`, `Array<`, … at the start of a statement, naming no
    // declared type: the lower-case built-in type constructor it spells.
    fn capitalized_builtin_ahead(&self) -> Option<&'static str> {
        let at = |k: usize| self.toks.get(self.pos + k).map(|s| &s.tok);
        let name = match at(0) {
            Some(Tok::Ident(n)) if !self.type_names.contains(n) => n.as_str(),
            _ => return None,
        };
        if !matches!(at(1), Some(Tok::Lt)) {
            return None;
        }
        ["mutex", "array", "handle", "guard", "slice", "ref", "rawptr"]
            .into_iter()
            .find(|b| name.len() == b.len() && name.starts_with(|c: char| c.is_ascii_uppercase()) && name.eq_ignore_ascii_case(b))
    }

    fn cur_ident_text(&self) -> String {
        match self.cur() {
            Tok::Ident(n) => n.clone(),
            other => format!("{}", other),
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
        // D-0131: the body its binders govern joins their region.
        let joins = self.join_at == Some(self.pos);
        self.join_at = None;
        self.expect(Tok::LBrace)?;
        self.scopes.push(HashMap::default());
        if !joins {
            self.regions.push(HashMap::default());
        }
        let r = self.parse_block_body();
        if !joins {
            self.regions.pop();
        }
        self.scopes.pop();
        r
    }

    fn parse_block_body(&mut self) -> PResult<Block> {
        let mut stmts = Vec::new();
        let mut tail = None;
        while *self.cur() != Tok::RBrace {
            // `T[N] x` as C writes it (D-0072): name the form CobaltC uses.
            if self.c_array_decl_ahead() {
                return self.err("a fixed-size array's type is written `array<T, N>`, as in `array<i32, 4> xs = [1, 2, 3, 4];`".to_string());
            }
            // `Mutex<T> m = …`: the built-in type constructors are lower
            // case (`mutex<T>`); only the function that makes one,
            // `Mutex::new`, is capitalized.
            if let Some(lower) = self.capitalized_builtin_ahead() {
                let hint = if lower == "mutex" { " (`Mutex::new(v)` makes one)" } else { "" };
                return self.err(format!("`{}` is not a type; the built-in type is written `{}<…>`{}", self.cur_ident_text(), lower, hint));
            }
            // `;;` or `{ … };`: C's empty statement.
            if *self.cur() == Tok::Semi {
                return self.err("a stray `;`: CobaltC has no empty statement, and a block, `if`, loop or `match` needs no `;` after its `}`".to_string());
            }
            if let Some(t) = self.try_parse_stmt(&mut stmts)? {
                tail = Some(t);
                break;
            }
        }
        self.expect(Tok::RBrace)?;
        Ok(Block { stmts, tail })
    }

    // D-0066: a name bound in the current scope (a local variable, or a
    // local constant's hidden name), declared on `line`.
    fn bind_local(&mut self, name: &str, konst: Option<String>, line: usize) {
        self.declare_in_region(name, line);
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), konst);
        }
    }

    // D-0131, `[Binding-Form-Redeclared]`: a name is declared once in its
    // declaration region (a block, with the parameters or the loop's or
    // pattern's names that govern it); a nested block may shadow it.
    fn declare_in_region(&mut self, name: &str, line: usize) {
        if name == "_" || name.starts_with('$') {
            return;
        }
        let Some(r) = self.regions.last_mut() else { return };
        match r.get(name) {
            Some(&first) => {
                if self.local_error.is_none() {
                    // Both are in one function, so in one file: the distance
                    // names the first without a file's line numbering.
                    let where_ = match line.saturating_sub(first) {
                        0 => "on this line".to_string(),
                        1 => "on the line above".to_string(),
                        n => format!("{} lines above", n),
                    };
                    let msg = format!(
                        "`{}` is already declared in this block ({}); a block declares a name once, counting the parameters and the loop's or pattern's names that govern it: assign to it (`{} = …`), or choose another name (as when the type changes); an inner block may declare its own",
                        name, where_, name
                    );
                    self.local_error = Some(format!("{}@{}", crate::modres::named("diag.duplicate-local", msg), line));
                }
            }
            None => {
                r.insert(name.to_string(), line);
            }
        }
    }

    // Opens the declaration region of `names` (D-0131), joined by the
    // block that begins at the current token, if one does.
    fn open_binder_region<'a>(&mut self, names: impl Iterator<Item = &'a str>, line: usize) {
        self.regions.push(HashMap::default());
        for n in names {
            self.declare_in_region(n, line);
        }
        self.join_at = if *self.cur() == Tok::LBrace { Some(self.pos) } else { None };
    }

    // Closes the region `open_binder_region` opened.
    fn close_binder_region(&mut self) {
        self.regions.pop();
        self.join_at = None;
    }

    // D-0069: the hidden name of a local type `name` in scope.
    fn lookup_type(&self, name: &str) -> Option<String> {
        let key = format!("::{}", name);
        self.scopes.iter().rev().find_map(|s| s.get(&key).cloned()).flatten()
    }

    // `struct`/`enum` in a block (D-0069): the item `N$k` of the module,
    // `N` naming it from here to the end of the block (and in its own
    // declaration, for a recursive type).
    fn parse_local_type(&mut self) -> PResult<()> {
        let at = if *self.cur() == Tok::Resource { 2 } else { 1 };
        let Some(Tok::Ident(name)) = self.toks.get(self.pos + at).map(|t| t.tok.clone()) else {
            return self.err("expected a type name".to_string());
        };
        self.local_consts += 1;
        let hidden = format!("{}${}", name, self.local_consts);
        if let Some(s) = self.scopes.last_mut() {
            s.insert(format!("::{}", name), Some(hidden.clone()));
        }
        let is_enum = matches!(self.toks.get(self.pos + at - 1).map(|t| &t.tok), Some(Tok::Enum));
        if *self.cur() == Tok::Bitstruct {
            // D-0132: a local `bitstruct`; its `bits`/`from_bits` are
            // derived for the hoisted item as for one at item level.
            let mut sd = self.parse_bitstruct(false)?;
            sd.name = hidden;
            self.hoisted.push(Item::Struct(std::sync::Arc::new(sd)));
        } else if is_enum {
            let mut e = self.parse_enum(false)?;
            e.name = hidden;
            self.hoisted.push(Item::Enum(std::sync::Arc::new(e)));
        } else {
            let mut sd = self.parse_struct(false)?;
            sd.name = hidden;
            self.hoisted.push(Item::Struct(std::sync::Arc::new(sd)));
        }
        Ok(())
    }

    // The innermost binding of `name` in the function: `Some(Some(k))` a
    // local constant hoisted as `k`, `Some(None)` a local variable.
    fn lookup_local(&self, name: &str) -> Option<Option<String>> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    // `const τ N = e;` in a block (D-0066): the constant `N$k` of the
    // module, `N` naming it from here to the end of the block.
    fn parse_local_const(&mut self) -> PResult<()> {
        self.expect(Tok::Const)?;
        let line = self.line();
        let (ret, auto_type) = self.parse_const_type()?;
        let name = self.ident()?;
        self.expect(Tok::Eq)?;
        let saved = std::mem::replace(&mut self.in_const_init, true);
        let e = self.parse_expr();
        self.in_const_init = saved;
        let e = e?;
        self.expect(Tok::Semi)?;
        if self.const_error.is_none() && (type_mentions(&ret, &self.fn_type_params) || expr_mentions_types(&e, &self.fn_type_params)) {
            self.const_error = Some(format!("diag.const-not-constant@{}", line));
        }
        self.local_consts += 1;
        let hidden = format!("{}${}", name, self.local_consts);
        let body = Block { stmts: Vec::new(), tail: Some(Box::new(e)) };
        self.hoisted.push(Item::Fn(std::sync::Arc::new(FnDecl { export: false, assoc_type: None, name: hidden.clone(), type_params: Vec::new(), type_bounds: Vec::new(), params: Vec::new(), ret, body, line, is_const: true, auto_type, derived: false })));
        self.bind_local(&name, Some(hidden), line);
        Ok(())
    }

    // Returns Some(tail expr) if this consumed the block's final expression.
    fn try_parse_stmt(&mut self, stmts: &mut Vec<Stmt>) -> PResult<Option<Box<Expr>>> {
        let decl_line = self.line();
        // D-0066: a local constant.
        if *self.cur() == Tok::Const && !self.scopes.is_empty() {
            self.parse_local_const()?;
            return Ok(None);
        }
        // D-0069: a local type, and a function of one.
        if !self.scopes.is_empty() {
            let next = self.toks.get(self.pos + 1).map(|t| t.tok.clone());
            let local_type = match self.cur() {
                Tok::Struct | Tok::Enum | Tok::Bitstruct => true,
                Tok::Resource => matches!(next, Some(Tok::Struct | Tok::Enum)),
                _ => false,
            };
            if local_type {
                self.parse_local_type()?;
                return Ok(None);
            }
            if *self.cur() == Tok::Fn && matches!(next, Some(Tok::Ident(_))) {
                if !matches!(self.toks.get(self.pos + 2).map(|t| &t.tok), Some(Tok::ColonColon)) {
                    return self.err("a function declared in a block must belong to a type declared there (`fn T::name`); for a local function, use a closure".to_string());
                }
                let f = self.parse_fn(false)?;
                self.hoisted.push(Item::Fn(std::sync::Arc::new(f)));
                return Ok(None);
            }
        }
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
        // `T x …` with `T` naming no known type (`int x = 10;`): two names in
        // a row start no expression, so it is a declaration, and the unknown
        // type is reported where types are resolved (`diag.unbound-name`,
        // static), not as a syntax error.
        let unknown_type_decl = matches!(self.cur(), Tok::Ident(_))
            && matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::Ident(_)))
            && matches!(self.toks.get(self.pos + 2).map(|t| &t.tok), Some(Tok::Eq | Tok::Semi));
        if self.is_type_start() || unknown_type_decl {
            let save = self.pos;
            let ty = self.parse_type()?;
            if matches!(self.cur(), Tok::Ident(_)) {
                let name = self.ident()?;
                let init = if self.eat(&Tok::Eq) { Some(Box::new(self.parse_expr()?)) } else { None };
                self.expect(Tok::Semi)?;
                self.bind_local(&name, None, decl_line);
                stmts.push(Stmt::Let { ty: Some(ty), name, init, line: decl_line });
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
            self.bind_local(&name, None, decl_line);
            stmts.push(Stmt::Let { ty: None, name, init: Some(Box::new(init)), line: decl_line });
            return Ok(None);
        }
        // destructuring: path '{' identifier (',' identifier)* '}' '=' expr ';'
        if let Tok::Ident(name) = self.cur().clone() {
            if self.type_names.contains(&name) && self.peek_is_destructure() {
                self.bump();
                self.expect(Tok::LBrace)?;
                // `field`, `field: binder` (D-0104), and `..` last (D-0104).
                let mut fields = Vec::new();
                let mut names = Vec::new();
                let mut rest = false;
                loop {
                    if self.eat(&Tok::DotDot) {
                        rest = true;
                        break;
                    }
                    let f = self.ident()?;
                    let b = if self.eat(&Tok::Colon) { self.ident()? } else { f.clone() };
                    names.push(f);
                    fields.push(b);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RBrace)?;
                self.expect(Tok::Eq)?;
                let init = self.parse_expr()?;
                self.expect(Tok::Semi)?;
                for f in &fields {
                    self.bind_local(f, None, decl_line);
                }
                let struct_name = self.lookup_type(&name).unwrap_or(name);
                stmts.push(Stmt::Destructure { struct_name, fields, names, rest, init: Box::new(init) });
                return Ok(None);
            }
        }
        // D-0099: `_ = e;` evaluates `e` and ends its value at once: a
        // block holding one hidden binding of it.
        if matches!(self.cur(), Tok::Ident(n) if n == "_") && matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::Eq)) {
            let line = self.line();
            self.bump();
            self.bump();
            let init = self.parse_expr()?;
            self.expect(Tok::Semi)?;
            let hold = Stmt::Let { ty: None, name: "$discard".to_string(), init: Some(Box::new(init)), line };
            stmts.push(Stmt::Expr(Box::new(Expr { kind: ExprKind::Block(Block { stmts: vec![hold], tail: None }), line })));
            return Ok(None);
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
        // `{ item (, item)* (, ..)? } =` where an item is `identifier` or
        // `identifier : identifier` (D-0104), or `{ .. } =` (a struct
        // literal's fields start with `.`)
        let tok = |k: usize| self.toks.get(self.pos + k).map(|s| &s.tok);
        if !matches!(tok(1), Some(Tok::LBrace)) {
            return false;
        }
        let mut k = 2;
        loop {
            match tok(k) {
                Some(Tok::DotDot) => {
                    k += 1;
                    break;
                }
                Some(Tok::Ident(_)) => {
                    k += 1;
                    if matches!(tok(k), Some(Tok::Colon)) {
                        if !matches!(tok(k + 1), Some(Tok::Ident(_))) {
                            return false;
                        }
                        k += 2;
                    }
                    if matches!(tok(k), Some(Tok::Comma)) {
                        k += 1;
                        continue;
                    }
                    break;
                }
                _ => return false,
            }
        }
        matches!(tok(k), Some(Tok::RBrace)) && matches!(tok(k + 1), Some(Tok::Eq))
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
                if let Some(mut pat) = self.try_condition_pattern() {
                    // D-0115, spec/14 `[While-Pattern]`:
                    // `while (true) { match (e) { pattern : body, _ : break } }`.
                    let scrut = self.parse_expr()?;
                    self.expect(Tok::RParen)?;
                    self.no_empty_body("while")?;
                    let arm_scope = self.arm_scope_for(&mut pat.1);
                    self.open_binder_region(arm_scope.keys().map(|k| k.as_str()), line);
                    self.scopes.push(arm_scope);
                    let b = self.parse_block();
                    self.scopes.pop();
                    self.close_binder_region();
                    let b = b?;
                    let stop = Box::new(Expr { kind: ExprKind::Break, line });
                    let m = Self::pattern_if(line, pat, scrut, b, Some(stop));
                    let body = Block { stmts: vec![Stmt::BlockLike(Box::new(m))], tail: None };
                    let always = Expr { kind: ExprKind::BoolLit(true), line };
                    return Ok(Expr { kind: ExprKind::While(Box::new(always), body, None), line });
                }
                let c = self.parse_expr()?;
                self.expect(Tok::RParen)?;
                self.no_empty_body("while")?;
                let b = self.parse_block()?;
                Ok(Expr { kind: ExprKind::While(Box::new(c), b, None), line })
            }
            Tok::For => self.parse_for(),
            Tok::Foreach => self.parse_foreach(),
            Tok::Match => self.parse_match(),
            other => self.err(format!("expected a block, `if`, `while`, `for`, `foreach` or `match`, found {}", other)),
        }
    }

    fn parse_if(&mut self) -> PResult<Expr> {
        let line = self.line();
        self.expect(Tok::If)?;
        self.expect(Tok::LParen)?;
        if let Some(mut pat) = self.try_condition_pattern() {
            let scrut = self.parse_expr()?;
            self.expect(Tok::RParen)?;
            self.no_empty_body("if")?;
            let arm_scope = self.arm_scope_for(&mut pat.1);
            self.open_binder_region(arm_scope.keys().map(|k| k.as_str()), line);
            self.scopes.push(arm_scope);
            let then_b = self.parse_block();
            self.scopes.pop();
            self.close_binder_region();
            let then_b = then_b?;
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
            return Ok(Self::pattern_if(line, pat, scrut, then_b, else_e));
        }
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

    // A pattern (D-0056): `_`, or a variant with an optional parenthesized
    // pattern for its payload, where an identifier alone is a binder
    // (`modres` makes one that names a variant a unit-variant pattern) and
    // a qualified path is a variant; a literal ends it (D-0057). Returns
    // the variant chain (outermost first), the binder, and the literal.
    fn parse_pattern(&mut self) -> PResult<(Vec<String>, Option<String>, Option<Box<Expr>>)> {
        // A pattern (D-0056): `_`, or a variant with an optional
        // parenthesized pattern for its payload, where an identifier
        // alone is a binder (`modres` makes one that names a variant a
        // unit-variant pattern) and a qualified path is a variant.
        let mut chain: Vec<String> = Vec::new();
        let mut binder: Option<String> = None;
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
                // D-0127: a text literal, matched against `str`, `String`
                // or `StringView` by its bytes.
                Tok::Str(text) => {
                    self.bump();
                    Some(Expr { kind: ExprKind::StrLit(text), line: lline })
                }
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
                Tok::LParen if matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::RParen)) => {
                    return self.err("`()` is not a pattern (D-0057); the unit type has one value, so `_` matches it: `Ok(_)`");
                }
                _ => return self.err("expected pattern in match arm"),
            };
            let name = path.last().unwrap().clone();
            if self.eat(&Tok::LParen) {
                if name == "_" {
                    return self.err("expected pattern in match arm");
                }
                chain.push(name);
                opened += 1;
                continue;
            }
            if name == "_" {
                // `_` alone: the wildcard arm, or a payload not bound.
            } else if qualified {
                // A variant, or a module's constant (`modres` decides).
                let mut path = path.clone();
                if let Some(h) = self.lookup_type(&path[0]) {
                    path[0] = h;
                }
                binder = Some(path.join("::"));
            } else {
                // A binder, at the top too (D-0058), unless `modres`
                // finds it names a variant or a constant.
                binder = Some(name);
            }
            break;
        }
        for _ in 0..opened {
            self.expect(Tok::RParen)?;
        }
        Ok((chain, binder, lit))
    }

    // D-0066: a binder that names a local constant is that constant
    // (`modres` makes it a literal pattern); any other binds a local for
    // the arm. The scope to push while the arm's body is parsed.
    fn arm_scope_for(&mut self, binder: &mut Option<String>) -> HashMap<String, Option<String>> {
        let mut arm_scope = HashMap::default();
        if let Some(b) = binder.clone() {
            if !b.contains("::") {
                match self.lookup_local(&b) {
                    Some(Some(hidden)) => *binder = Some(hidden),
                    _ => {
                        arm_scope.insert(b, None);
                    }
                }
            }
        }
        arm_scope
    }

    // D-0115: `if (pattern = e)` / `while (pattern = e)`. After the `(`,
    // tries a pattern whose top is refutable as the parser can tell —
    // `Name(…)`, a qualified `m::Name`, or a literal — followed by a single
    // `=`; otherwise restores its position and returns `None`, and the
    // condition is an ordinary expression (`if (x = e)` stays the
    // assignment it is, and `Some(x) == e` a comparison).
    fn try_condition_pattern(&mut self) -> Option<(Vec<String>, Option<String>, Option<Box<Expr>>)> {
        let starts_like_pattern = match self.cur() {
            Tok::Ident(_) => matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::LParen) | Some(Tok::ColonColon)),
            Tok::Int(..) | Tok::Minus | Tok::True | Tok::False | Tok::Str(_) => true,
            _ => false,
        };
        if !starts_like_pattern {
            return None;
        }
        let save_pos = self.pos;
        let save_undo = self.undo_log.len();
        match self.parse_pattern() {
            Ok(p) if *self.cur() == Tok::Eq => {
                self.bump();
                Some(p)
            }
            _ => {
                self.pos = save_pos;
                self.undo_log.truncate(save_undo);
                None
            }
        }
    }

    // The `match` that `if (pattern = e) then else` stands for (D-0115,
    // spec/14 `[If-Pattern]`): `match (e) { pattern : then, _ : else }`.
    fn pattern_if(line: usize, pat: (Vec<String>, Option<String>, Option<Box<Expr>>), scrut: Expr, then_b: Block, else_e: Option<Box<Expr>>) -> Expr {
        let (chain, binder, lit) = pat;
        let mut chain = chain.into_iter();
        let variant = chain.next();
        let nested: Vec<String> = chain.collect();
        let then_e = Expr { kind: ExprKind::Block(then_b), line };
        let other = else_e.unwrap_or_else(|| Box::new(Expr { kind: ExprKind::Block(Block { stmts: Vec::new(), tail: None }), line }));
        let arms = vec![
            Arm { variant, nested, binder, lit, body: Box::new(then_e) },
            Arm { variant: None, nested: Vec::new(), binder: None, lit: None, body: other },
        ];
        Expr { kind: ExprKind::Match(Box::new(scrut), arms), line }
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
            let arm_line = self.line();
            let (chain, binder, lit) = self.parse_pattern()?;
            self.expect(Tok::Colon)?;
            let mut binder = binder;
            let arm_scope = self.arm_scope_for(&mut binder);
            self.open_binder_region(arm_scope.keys().map(|k| k.as_str()), arm_line);
            self.scopes.push(arm_scope);
            let body = self.parse_expr();
            self.scopes.pop();
            self.close_binder_region();
            let body = body?;
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
        mk(ExprKind::Block(Block { stmts: vec![Stmt::Let { ty: None, name: t.clone(), init: Some(Box::new(borrow)), line }, Stmt::Expr(Box::new(write))], tail: None }))
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
        self.scopes.push(HashMap::default());
        self.regions.push(HashMap::default());
        let r = self.parse_for_parts(line);
        self.regions.pop();
        self.join_at = None;
        self.scopes.pop();
        r
    }

    fn parse_for_parts(&mut self, line: usize) -> PResult<Expr> {
        let mut stmts = Vec::new();
        if !self.eat(&Tok::Semi) {
            if let Some(_) = self.try_parse_stmt(&mut stmts)? {
                return self.err("expected `;` after a for loop's initializer".to_string());
            }
        }
        let cond = if *self.cur() == Tok::Semi { Expr { kind: ExprKind::BoolLit(true), line } } else { self.parse_expr()? };
        self.expect(Tok::Semi)?;
        let step = if *self.cur() == Tok::RParen { None } else { Some(Box::new(self.parse_expr()?)) };
        self.expect(Tok::RParen)?;
        self.no_empty_body("for")?;
        self.join_at = if *self.cur() == Tok::LBrace { Some(self.pos) } else { None };
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
            other => return self.err(format!("expected `in` in a foreach, found {}", other)),
        }
        let coll = self.parse_expr()?;
        let hi = if self.eat(&Tok::DotDot) { Some(self.parse_expr()?) } else { None };
        self.expect(Tok::RParen)?;
        self.no_empty_body("foreach")?;
        self.open_binder_region(names.iter().map(|n| n.as_str()), line);
        self.scopes.push(names.iter().map(|n| (n.clone(), None)).collect());
        let body = self.parse_block();
        self.scopes.pop();
        self.close_binder_region();
        let body = body?;
        self.fresh += 1;
        let n = self.fresh;
        let mk = |kind: ExprKind| Expr { kind, line };
        let path = |s: &str| mk(ExprKind::Path(vec![s.to_string()], vec![]));
        let call = |f: &str, args: Vec<Expr>| mk(ExprKind::Call(Box::new(path(f)), args));
        if let Some(hi) = hi {
            return Ok(Self::foreach_range(names, coll, hi, body, n, line));
        }
        let (c, i, e, d) = (format!("$c{}", n), format!("$i{}", n), format!("$e{}", n), format!("$d{}", n));
        let mode = match &coll.kind {
            ExprKind::Borrow(m, _) => Some(m.clone()),
            _ => None,
        };
        let mut stmts = Vec::new();
        let (holder, key_fn, elem_fn) = match mode {
            Some(m) => {
                stmts.push(Stmt::Let { ty: None, name: c.clone(), init: Some(Box::new(coll)), line });
                (c.clone(), "$each_key", if m == Mode::Exclusive { "$each_at_mut" } else { "$each_at" })
            }
            None => {
                stmts.push(Stmt::Let { ty: None, name: e.clone(), init: Some(Box::new(coll)), line });
                let arity = mk(ExprKind::IntLit(names.len() as u128, None));
                stmts.push(Stmt::Let { ty: None, name: d.clone(), init: Some(Box::new(call("$each_drain", vec![path(&e), arity]))), line });
                (d.clone(), "$each_take_key", "$each_take")
            }
        };
        stmts.push(Stmt::Let { ty: Some(Type::Int(IntTy::Usize)), name: i.clone(), init: Some(Box::new(mk(ExprKind::IntLit(0, None)))), line });
        let cond = mk(ExprKind::Binary(BinOp::Lt, Box::new(path(&i)), Box::new(call("$each_len", vec![path(&holder)]))));
        let step = mk(ExprKind::Assign(Box::new(path(&i)), Box::new(mk(ExprKind::Binary(BinOp::Add, Box::new(path(&i)), Box::new(mk(ExprKind::IntLit(1, None))))))));
        let mut inner = Vec::new();
        // Three names (a map only, D-0044): the position, then key and value.
        if names.len() == 3 {
            inner.push(Stmt::Let { ty: None, name: names[0].clone(), init: Some(Box::new(path(&i))), line });
        }
        if names.len() >= 2 {
            inner.push(Stmt::Let { ty: None, name: names[names.len() - 2].clone(), init: Some(Box::new(call(key_fn, vec![path(&holder), path(&i)]))), line });
        }
        let arity = mk(ExprKind::IntLit(names.len() as u128, None));
        inner.push(Stmt::Let { ty: None, name: names.last().unwrap().clone(), init: Some(Box::new(call(elem_fn, vec![path(&holder), path(&i), arity]))), line });
        inner.push(Stmt::BlockLike(Box::new(mk(ExprKind::Block(body)))));
        let lp = mk(ExprKind::While(Box::new(cond), Block { stmts: inner, tail: None }, Some(Box::new(step))));
        Ok(mk(ExprKind::Block(Block { stmts, tail: Some(Box::new(lp)) })))
    }

    // `foreach (x in lo .. hi) body` and `foreach (k, x in lo .. hi) body`
    // (D-0097): the integers from `lo` up to, not including, `hi`, and
    // with two names their position. The bounds have one integer type; a
    // literal expression takes the other bound's, as it would across `<`
    // (`$each_range`, src/each.rs, which also checks the type is an
    // integer); two literal expressions are `usize`. Reading a literal expression after the other bound is
    // unobservable: it has no effect and cannot fault at run time.
    //
    //   { auto $v = lo;  auto $h = hi;  $each_range($v, (), n);  usize $i = 0;
    //     for (; $v < $h; { $v += 1; $i += 1; }) { auto k = $i; auto x = $v; body } }
    //
    // with `auto $h = hi; auto $v = $each_range($h, lo, n);` for a literal
    // `lo`, and `auto $v = lo; auto $h = $each_range($v, hi, n);` for a
    // literal `hi`; for two, `usize $z = 0; auto $v = $each_range($z, lo, n);`
    // first. `$v` never passes `$h`, so the step cannot overflow.
    fn foreach_range(names: Vec<String>, lo: Expr, hi: Expr, body: Block, n: usize, line: usize) -> Expr {
        let mk = |kind: ExprKind| Expr { kind, line };
        let path = |s: &str| mk(ExprKind::Path(vec![s.to_string()], vec![]));
        let let_ = |name: &str, init: Expr| Stmt::Let { ty: None, name: name.to_string(), init: Some(Box::new(init)), line };
        let inc = |name: &str| mk(ExprKind::Assign(Box::new(path(name)), Box::new(mk(ExprKind::Binary(BinOp::Add, Box::new(path(name)), Box::new(mk(ExprKind::IntLit(1, None))))))));
        let (v, h, i) = (format!("$v{}", n), format!("$h{}", n), format!("$i{}", n));
        let arity = mk(ExprKind::IntLit(names.len() as u128, None));
        let range = |b: &str, e: Expr| mk(ExprKind::Call(Box::new(path("$each_range")), vec![path(b), e, arity.clone()]));
        let mut stmts = Vec::new();
        match (is_literal_expr(&lo), is_literal_expr(&hi)) {
            (true, false) => {
                stmts.push(let_(&h, hi));
                stmts.push(let_(&v, range(&h, lo)));
            }
            // Two literal expressions: `usize`, the type a position or an
            // index has (a floating-point one is left to `$each_range`,
            // which names it).
            (true, true) if !crate::each::has_float(&lo) && !crate::each::has_float(&hi) => {
                let z = format!("$z{}", n);
                stmts.push(Stmt::Let { ty: Some(Type::Int(IntTy::Usize)), name: z.clone(), init: Some(Box::new(mk(ExprKind::IntLit(0, None)))), line });
                stmts.push(let_(&v, range(&z, lo)));
                stmts.push(let_(&h, range(&v, hi)));
            }
            (_, true) => {
                stmts.push(let_(&v, lo));
                stmts.push(let_(&h, range(&v, hi)));
            }
            (false, false) => {
                stmts.push(let_(&v, lo));
                stmts.push(let_(&h, hi));
                stmts.push(Stmt::Expr(Box::new(range(&v, mk(ExprKind::Unit)))));
            }
        }
        let mut inner = Vec::new();
        let mut step = vec![Stmt::Expr(Box::new(inc(&v)))];
        if names.len() >= 2 {
            stmts.push(Stmt::Let { ty: Some(Type::Int(IntTy::Usize)), name: i.clone(), init: Some(Box::new(mk(ExprKind::IntLit(0, None)))), line });
            inner.push(let_(&names[names.len() - 2], path(&i)));
            step.push(Stmt::Expr(Box::new(inc(&i))));
        }
        inner.push(let_(names.last().unwrap(), path(&v)));
        inner.push(Stmt::BlockLike(Box::new(mk(ExprKind::Block(body)))));
        let cond = mk(ExprKind::Binary(BinOp::Lt, Box::new(path(&v)), Box::new(path(&h))));
        let step = mk(ExprKind::Block(Block { stmts: step, tail: None }));
        let lp = mk(ExprKind::While(Box::new(cond), Block { stmts: inner, tail: None }, Some(Box::new(step))));
        mk(ExprKind::Block(Block { stmts, tail: Some(Box::new(lp)) }))
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
        let lhs = self.parse_bitor()?;
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
        let rhs = self.parse_bitor()?;
        Ok(Expr { kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), line })
    }
    // D-0086: a shift binds tighter than `&`, `^` and `|` (as in C and
    // Rust) and looser than `+`/`-`.
    fn parse_shift(&mut self) -> PResult<Expr> {
        let line = self.line();
        let mut lhs = self.parse_additive()?;
        loop {
            let op = match self.cur() {
                Tok::Shl => BinOp::Shl,
                Tok::Shr => BinOp::Shr,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_additive()?;
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
        let mut lhs = self.parse_shift()?;
        while *self.cur() == Tok::Amp {
            self.bump();
            let rhs = self.parse_shift()?;
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
                            // D-0098: a trailing comma before `)`.
                            if !self.eat(&Tok::Comma) || *self.cur() == Tok::RParen {
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
        // D-0069: `T::f`, `T::V`, `T<…>::f` and `T { … }` name a local type
        // `T` by its hidden item.
        let names_type = matches!(self.cur(), Tok::ColonColon | Tok::Lt | Tok::LBrace);
        if names_type {
            if let Some(h) = self.lookup_type(&segs[0]) {
                segs[0] = h;
            }
        }
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
                // `'return' expr?`: nothing follows when the enclosing
                // construct ends here (a statement, a block, a match arm,
                // an argument list or brackets).
                if matches!(self.cur(), Tok::Semi | Tok::RBrace | Tok::Comma | Tok::RParen | Tok::RBracket) {
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
                // `Name { f: v }`, as Rust writes a struct literal: name the
                // form CobaltC uses.
                if *self.cur() == Tok::LBrace
                    && matches!(self.toks.get(self.pos + 1).map(|s| &s.tok), Some(Tok::Ident(_)))
                    && matches!(self.toks.get(self.pos + 2).map(|s| &s.tok), Some(Tok::Colon))
                {
                    self.bump();
                    return self.err(format!("a struct literal names each field with a dot and `=`: `{} {{ .field = value, … }}`", segs.join("::")));
                }
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
                // D-0066: a local constant's name is its hidden constant; a
                // local variable in a constant's initializer is not constant.
                if segs.len() == 1 {
                    match self.lookup_local(&segs[0]) {
                        Some(Some(hidden)) => return Ok(Expr { kind: ExprKind::Path(vec![hidden], targs), line }),
                        Some(None) if self.in_const_init && self.const_error.is_none() => {
                            self.const_error = Some(format!("diag.const-not-constant@{}", line));
                        }
                        _ => {}
                    }
                }
                Ok(Expr { kind: ExprKind::Path(segs, targs), line })
            }
            other => {
                // `u32 slice = …`: a declaration whose name is a reserved
                // word reads as an expression that starts with a type.
                if let (Tok::TypeName(_), Some(Tok::TypeName(w))) = (&other, self.toks.get(self.pos + 1).map(|t| &t.tok)) {
                    let w = w.clone();
                    self.bump();
                    return self.err(reserved_name(&w));
                }
                // `f32(x)`: a call of something named as a built-in type.
                if let (Tok::TypeName(w), Some(Tok::LParen)) = (&other, self.toks.get(self.pos + 1).map(|t| &t.tok)) {
                    let w = w.clone();
                    return self.err(reserved_call(&w));
                }
                // `i32 move = 1;`: a keyword as a variable's name.
                if let (Tok::TypeName(_), Some(k), Some(after)) = (&other, self.toks.get(self.pos + 1).map(|t| &t.tok), self.toks.get(self.pos + 2).map(|t| &t.tok)) {
                    let shown = format!("{}", k);
                    let word = shown.trim_matches('`');
                    if !matches!(k, Tok::Ident(_) | Tok::TypeName(_)) && !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                        && matches!(after, Tok::Eq | Tok::Semi | Tok::Comma | Tok::RParen)
                    {
                        return self.err(format!("`{}` is a keyword, so it cannot name a variable; choose another name", word));
                    }
                }
                self.err(format!("expected an expression, found {}", other))
            }
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
                items.push(self.parse_expr()?);
                // `[v; N]` (D-0072): N copies of v; N is a length, written
                // as an array type's is.
                if self.eat(&Tok::Semi) {
                    let n = self.array_length()?;
                    self.expect(Tok::RBracket)?;
                    return Ok(Expr { kind: ExprKind::ArrayRepeat(Box::new(items.remove(0)), n), line });
                }
                // A trailing comma is allowed, as in a `match` (D-0072).
                while self.eat(&Tok::Comma) {
                    if *self.cur() == Tok::RBracket {
                        break;
                    }
                    items.push(self.parse_expr()?);
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
                if matches!(self.cur(), Tok::Amp | Tok::AndAnd) || matches!(self.cur(), Tok::Ident(w) if w == "ref") {
                    return self.err("a capture is a bare name: `[x](…) { … }` borrows `x` (exclusively when the body writes it), `move [x](…) { … }` moves or copies it".to_string());
                }
                captures.push(self.ident()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect(Tok::RBracket)?;
        }
        let params = self.parse_params()?;
        // D-0081: an optional result type, written as a function's is.
        let ret = if self.eat(&Tok::Colon) { Some(self.parse_type()?) } else { None };
        self.scopes.push(captures.iter().chain(params.iter().map(|p| &p.name)).map(|n| (n.clone(), None)).collect());
        self.open_binder_region(captures.iter().map(|c| c.as_str()).chain(params.iter().map(|p| p.name.as_str())), line);
        let body = self.parse_block();
        self.close_binder_region();
        self.scopes.pop();
        let body = body?;
        Ok(Expr { kind: ExprKind::Closure { is_move, captures, params, ret, body: std::sync::Arc::new(body) }, line })
    }
}

pub fn parse(src: &str) -> Result<Program, String> {
    let toks = crate::lexer::Lexer::new(src)
        .tokenize()
        .map_err(|e| format!("lex error at {}:{}: {}", e.line, e.col, e.msg))?;
    let mut p = Parser::new(toks);
    let program = p.parse_program().map_err(|e| e.to_string())?;
    match p.const_error.or(p.local_error) {
        Some(d) => Err(d),
        None => Ok(program),
    }
}

// Whether `t` names one of `params` (a function's type parameters).
fn type_mentions(t: &Type, params: &[String]) -> bool {
    match t {
        Type::Named(n, args) => params.contains(n) || args.iter().any(|a| type_mentions(a, params)),
        Type::Ref(x, _) | Type::Slice(x, _) | Type::Rawptr(x) | Type::Array(x, _) | Type::Handle(x) | Type::Mutex(x) | Type::Guard(x) => type_mentions(x, params),
        Type::Fn(ps, r) => ps.iter().any(|p| type_mentions(p, params)) || type_mentions(r, params),
        _ => false,
    }
}

// Whether an expression's type arguments (`sizeof<T>()`, `Vec::new<T>()`)
// name one of `params`.
fn expr_mentions_types(e: &Expr, params: &[String]) -> bool {
    if params.is_empty() {
        return false;
    }
    let mut found = false;
    fn walk(e: &Expr, params: &[String], found: &mut bool) {
        match &e.kind {
            ExprKind::Path(_, targs) | ExprKind::StructLit(_, targs, _) if targs.iter().any(|t| type_mentions(t, params)) => *found = true,
            _ => {}
        }
        match &e.kind {
            ExprKind::StructLit(_, _, fs) => fs.iter().for_each(|(_, x)| walk(x, params, found)),
            ExprKind::ArrayLit(xs) => xs.iter().for_each(|x| walk(x, params, found)),
            ExprKind::ArrayRepeat(x, _) => walk(x, params, found),
            ExprKind::Unary(_, x) | ExprKind::Paren(x) | ExprKind::Field(x, _) | ExprKind::Deref(x) | ExprKind::Borrow(_, x) | ExprKind::Propagate(x) => walk(x, params, found),
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::Assign(a, b) => {
                walk(a, params, found);
                walk(b, params, found);
            }
            ExprKind::Call(c, args) => {
                walk(c, params, found);
                args.iter().for_each(|x| walk(x, params, found));
            }
            _ => {}
        }
    }
    walk(e, params, &mut found);
    found
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

// A built-in type's name where a name was expected.
fn reserved_name(w: &str) -> String {
    format!("`{}` is a reserved word (the name of a built-in type), so it cannot name a variable, field, parameter or function; choose another name", w)
}

// A call of something named as a built-in type: `array(p)`, `i32(x)`.
fn reserved_call(w: &str) -> String {
    if matches!(w, "f32" | "f64") || w.starts_with('i') || w.starts_with('u') && w != "unit" {
        format!("{}; to convert a value, use `widen<{}>(…)`, `narrow<{}>(…)`, `to_float<…>` or `to_int<…>`", reserved_name(w), w, w)
    } else {
        reserved_name(w)
    }
}

// D-0126: a constant expression over integer literals, the operators
// `+ - * / % & | ^ << >> ~ ( )` and unary `-`, and names `lookup` knows,
// folded to a value with the grammar's precedence (spec/22 §2); `None`
// when the tokens are not one — a call, a float, an unknown name, a
// division by zero, an overflow, a shift by 127 or more.
fn fold_const_tokens(toks: &[Spanned<Tok>], lookup: &dyn Fn(&[String]) -> Option<i128>) -> Option<i128> {
    struct P<'a> {
        t: &'a [Spanned<Tok>],
        i: usize,
        lookup: &'a dyn Fn(&[String]) -> Option<i128>,
    }
    fn apply(op: char, a: i128, b: i128) -> Option<i128> {
        match op {
            '+' => a.checked_add(b),
            '-' => a.checked_sub(b),
            '*' => a.checked_mul(b),
            '/' => if b == 0 { None } else { a.checked_div(b) },
            '%' => if b == 0 { None } else { a.checked_rem(b) },
            '&' => Some(a & b),
            '|' => Some(a | b),
            '^' => Some(a ^ b),
            'l' => if (0..127).contains(&b) { a.checked_shl(b as u32) } else { None },
            'r' => if (0..127).contains(&b) { Some(a >> b) } else { None },
            _ => None,
        }
    }
    impl P<'_> {
        fn peek(&self) -> Option<&Tok> {
            self.t.get(self.i).map(|s| &s.tok)
        }
        fn expr(&mut self, min_prec: u8) -> Option<i128> {
            let mut lhs = self.unary()?;
            loop {
                let (prec, op) = match self.peek() {
                    Some(Tok::Pipe) => (1, '|'),
                    Some(Tok::Caret) => (2, '^'),
                    Some(Tok::Amp) => (3, '&'),
                    Some(Tok::Shl) => (4, 'l'),
                    Some(Tok::Shr) => (4, 'r'),
                    Some(Tok::Plus) => (5, '+'),
                    Some(Tok::Minus) => (5, '-'),
                    Some(Tok::Star) => (6, '*'),
                    Some(Tok::Slash) => (6, '/'),
                    Some(Tok::Percent) => (6, '%'),
                    _ => break,
                };
                if prec < min_prec {
                    break;
                }
                self.i += 1;
                let rhs = self.expr(prec + 1)?;
                lhs = apply(op, lhs, rhs)?;
            }
            Some(lhs)
        }
        fn unary(&mut self) -> Option<i128> {
            match self.peek()? {
                Tok::Minus => {
                    self.i += 1;
                    self.unary()?.checked_neg()
                }
                Tok::Tilde => {
                    self.i += 1;
                    Some(!self.unary()?)
                }
                Tok::LParen => {
                    self.i += 1;
                    let v = self.expr(1)?;
                    if self.peek() != Some(&Tok::RParen) {
                        return None;
                    }
                    self.i += 1;
                    Some(v)
                }
                Tok::Int(v, _) => {
                    let v = i128::try_from(*v).ok()?;
                    self.i += 1;
                    Some(v)
                }
                Tok::Ident(_) => {
                    let mut segs = Vec::new();
                    while let Some(Tok::Ident(n)) = self.peek() {
                        segs.push(n.clone());
                        self.i += 1;
                        if self.peek() == Some(&Tok::ColonColon) {
                            self.i += 1;
                        } else {
                            break;
                        }
                    }
                    (self.lookup)(&segs)
                }
                _ => None,
            }
        }
    }
    let mut p = P { t: toks, i: 0, lookup };
    let v = p.expr(1)?;
    if p.i == toks.len() { Some(v) } else { None }
}
