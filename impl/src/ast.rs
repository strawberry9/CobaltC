// AST for CobaltC (spec/22).
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int(IntTy),
    F32,
    F64,
    Bool,
    Str, // text value: valid UTF-8 bytes, non-resource (spec/21 type.str)
    Void, // unit
    Never,
    Ref(Box<Type>, Mode),
    // `slice<T, m>` (D-0047): a borrowed run of elements of an array,
    // a `Vec` or another slice.
    Slice(Box<Type>, Mode),
    Rawptr(Box<Type>),
    Array(Box<Type>, u128),
    Fn(Vec<Type>, Box<Type>),
    Handle(Box<Type>),
    Mutex(Box<Type>),
    Guard(Box<Type>),
    // a named struct/enum, possibly generic-instantiated; also a bare type
    // parameter (args empty, name matches an in-scope type-param) resolved
    // during monomorphization.
    Named(String, Vec<Type>),
    // an anonymous closure-capture struct, identified structurally.
    Closure(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    Isize,
    Usize,
}

// A type as a program spells it, for diagnostics (D-0072): `std`'s
// items without their module, a closure as `closure`.
impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let m = |m: &Mode| if *m == Mode::Shared { "shared" } else { "exclusive" };
        match self {
            Type::Int(t) => write!(f, "{}", t.name()),
            Type::F32 => write!(f, "f32"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::Str => write!(f, "str"),
            Type::Void => write!(f, "void"),
            Type::Never => write!(f, "never"),
            Type::Ref(t, md) => write!(f, "ref<{}, {}>", t, m(md)),
            Type::Slice(t, md) => write!(f, "slice<{}, {}>", t, m(md)),
            Type::Rawptr(t) => write!(f, "rawptr<{}>", t),
            Type::Array(t, n) => write!(f, "array<{}, {}>", t, n),
            Type::Fn(ps, r) => {
                write!(f, "fn(")?;
                for (i, p) in ps.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") : {}", r)
            }
            Type::Handle(t) => write!(f, "handle<{}>", t),
            Type::Mutex(t) => write!(f, "mutex<{}>", t),
            Type::Guard(t) => write!(f, "guard<{}>", t),
            Type::Named(n, args) => {
                let n = n.strip_prefix("std::").unwrap_or(n);
                let n = n.split('$').next().unwrap_or(n);
                write!(f, "{}", n)?;
                if !args.is_empty() {
                    write!(f, "<")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", a)?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            Type::Closure(_) => write!(f, "closure"),
        }
    }
}

impl IntTy {
    pub fn signed(self) -> bool {
        matches!(
            self,
            IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::I128 | IntTy::Isize
        )
    }
    pub fn bitwidth(self, addr_width: u32) -> u32 {
        match self {
            IntTy::I8 | IntTy::U8 => 8,
            IntTy::I16 | IntTy::U16 => 16,
            IntTy::I32 | IntTy::U32 => 32,
            IntTy::I64 | IntTy::U64 => 64,
            IntTy::I128 | IntTy::U128 => 128,
            IntTy::Isize | IntTy::Usize => addr_width,
        }
    }
    // D-0102: a literal's integer type from its suffix and the type its
    // context expects. `byte` is the suffix of `b'x'`: an unsigned
    // integer type the context gives, else `u8`. The bool is whether the
    // type was defaulted (nothing fixed it).
    pub fn of_literal(suffix: Option<&str>, expected: Option<&Type>) -> (IntTy, bool) {
        match suffix {
            Some("byte") => match expected {
                Some(Type::Int(t)) if !t.signed() => (*t, false),
                _ => (IntTy::U8, false),
            },
            Some(s) => match IntTy::from_str(s) {
                Some(t) => (t, false),
                None => (IntTy::I32, true),
            },
            None => match expected {
                Some(Type::Int(t)) => (*t, false),
                _ => (IntTy::I32, true),
            },
        }
    }

    pub fn from_str(s: &str) -> Option<IntTy> {
        Some(match s {
            "i8" => IntTy::I8,
            "i16" => IntTy::I16,
            "i32" => IntTy::I32,
            "i64" => IntTy::I64,
            "i128" => IntTy::I128,
            "u8" => IntTy::U8,
            "u16" => IntTy::U16,
            "u32" => IntTy::U32,
            "u64" => IntTy::U64,
            "u128" => IntTy::U128,
            "isize" => IntTy::Isize,
            "usize" => IntTy::Usize,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            IntTy::I8 => "i8",
            IntTy::I16 => "i16",
            IntTy::I32 => "i32",
            IntTy::I64 => "i64",
            IntTy::I128 => "i128",
            IntTy::U8 => "u8",
            IntTy::U16 => "u16",
            IntTy::U32 => "u32",
            IntTy::U64 => "u64",
            IntTy::U128 => "u128",
            IntTy::Isize => "isize",
            IntTy::Usize => "usize",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub ty: Type,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub export: bool,
    pub ty: Type,
    pub name: String,
    // D-0118: a `bitstruct` field's width in bits (`ty` is then the
    // smallest unsigned type holding it).
    pub width: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct VariantDecl {
    pub name: String,
    pub payload: Option<Type>,
}

// D-0090: a type parameter's bound, one of a closed set, each implying
// the ones before it: `integer` ⇒ `number` ⇒ `ordered` ⇒ `eq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bound {
    Eq,
    Ordered,
    Number,
    Integer,
    // D-0116: `T` can be copied: a plain type, or one with a `T::clone`
    // (declared, or derived field by field for a struct or enum).
    Clone,
}

impl Bound {
    pub fn from_name(s: &str) -> Option<Bound> {
        match s {
            "eq" => Some(Bound::Eq),
            "ordered" => Some(Bound::Ordered),
            "number" => Some(Bound::Number),
            "integer" => Some(Bound::Integer),
            "clone" => Some(Bound::Clone),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Bound::Eq => "eq",
            Bound::Ordered => "ordered",
            Bound::Number => "number",
            Bound::Integer => "integer",
            Bound::Clone => "clone",
        }
    }
    fn rank(self) -> u8 {
        match self {
            Bound::Eq => 0,
            Bound::Ordered => 1,
            Bound::Number => 2,
            Bound::Integer => 3,
            Bound::Clone => 0,
        }
    }
    // Every type satisfying `self` satisfies `other`.
    pub fn implies(self, other: Bound) -> bool {
        // Every type the four operator bounds admit is copied by a read
        // (`String` by `String::clone`), so each implies `clone`; `clone`
        // implies only itself (D-0116).
        if other == Bound::Clone {
            return true;
        }
        if self == Bound::Clone {
            return false;
        }
        self.rank() >= other.rank()
    }
    // Whether a concrete type satisfies the bound: the types whose values
    // the bound's operators already apply to.
    pub fn holds_for(self, t: &Type) -> bool {
        // `clone` on a named type needs the declarations (`typecheck::Ck::
        // clone_problem`); here only what needs none.
        if self == Bound::Clone {
            return matches!(t, Type::Int(_) | Type::F32 | Type::F64 | Type::Bool | Type::Str | Type::Ref(..) | Type::Rawptr(_) | Type::Slice(..))
                || matches!(t, Type::Named(n, a) if n == "std::String" && a.is_empty());
        }
        match t {
            Type::Int(_) => true,
            Type::F32 | Type::F64 => self != Bound::Integer,
            // D-0108: `bool`, `str` and `String` are ordered (`false <
            // true`; text in byte order), so equal too.
            Type::Bool | Type::Str => matches!(self, Bound::Eq | Bound::Ordered),
            Type::Named(n, a) if n == "std::String" && a.is_empty() => matches!(self, Bound::Eq | Bound::Ordered),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub export: bool,
    pub assoc_type: Option<String>, // `fn Type::name`
    pub name: String,
    pub type_params: Vec<String>,
    // D-0090: each type parameter's bound, if any (same order).
    pub type_bounds: Vec<Option<Bound>>,
    pub params: Vec<Param>,
    pub ret: Type,
    pub body: Block,
    pub line: usize, // the declared name's line (`[Item-Duplicate]`'s location)
    // `const τ NAME = e;` (D-0036): a function of no parameters whose
    // body is `e`; a use of `NAME` is a call of it (`modres`), folded to
    // a literal where `consts` can.
    pub is_const: bool,
    // `const auto NAME = e;` (D-0068): `ret` is not written; `consts`
    // synthesizes it from `e` before anything reads it.
    pub auto_type: bool,
    // D-0116: a `T::clone` the front end derived for a struct or enum
    // (`derive.rs`); whether `T` is `clone` is then decided by its fields.
    pub derived: bool,
}

#[derive(Debug, Clone)]
pub struct ExternDecl {
    pub export: bool,
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Type,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub export: bool,
    pub resource: bool,
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<FieldDecl>,
    pub line: usize,
    // D-0118: `bitstruct Name : uN { … }` — the backing unsigned type; the
    // fields are bit fields, low bit first, filling it exactly.
    pub bits: Option<IntTy>,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub export: bool,
    pub resource: bool,
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<VariantDecl>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Item {
    Fn(Arc<FnDecl>),
    Extern(Arc<ExternDecl>),
    Struct(Arc<StructDecl>),
    Enum(Arc<EnumDecl>),
    Module(bool, String, Vec<Item>, usize), // export, name, items, declaring line
    Import(bool, String, usize),     // export (a re-export, D-0138), path, declaring line
    ExternCode(String, usize),       // `extern "…";` (spec/20 `rule.trust.extern-code`): its string, declaring line
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        ty: Option<Type>, // None => auto
        name: String,
        init: Option<Box<Expr>>,
        // The statement's line: a declaration without an initializer has
        // no expression to take one from.
        line: usize,
    },
    // `Name { f1, f2, … } = e;` (D-0044: every field, each once).
    Destructure {
        struct_name: String,
        // The binders, one per named field; `names[i]` is the field
        // `fields[i]` binds (the same unless renamed, `S { f: x }`, D-0104).
        fields: Vec<String>,
        names: Vec<String>,
        // `S { a, .. } = e`: the fields not named are bound to hidden
        // names and end as unused bindings do (D-0104).
        rest: bool,
        init: Box<Expr>,
    },
    Expr(Box<Expr>),
    BlockLike(Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct Arm {
    pub variant: Option<String>, // None => wildcard `_`
    // D-0056: the variants a nested pattern tests inside `variant`'s
    // payload, outermost first (`Ok(Some(None))`: variant `Ok`, nested
    // `Some`, `None`). A variant has one payload, so a pattern is a chain.
    pub nested: Vec<String>,
    pub binder: Option<String>, // None => no binder or `_`; binds the innermost payload
    // D-0057: a literal the innermost payload (or, with no variant, the
    // scrutinee itself) must equal: an integer, a byte (`b'x'`), `true`
    // or `false`, kept as the literal expression it is written as.
    pub lit: Option<Box<Expr>>,
    pub body: Box<Expr>,
}

impl Arm {
    // The variants the arm tests, outermost first (empty for `_`).
    pub fn chain(&self) -> Vec<String> {
        let mut c: Vec<String> = self.variant.iter().cloned().collect();
        c.extend(self.nested.iter().cloned());
        c
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    IntLit(u128, Option<String>),
    FloatLit(f64, Option<String>),
    StrLit(String),
    BoolLit(bool),
    Unit,
    Path(Vec<String>, Vec<Type>), // segments, optional explicit type args
    StructLit(Vec<String>, Vec<Type>, Vec<(String, Expr)>),
    ArrayLit(Vec<Expr>),
    // `[v; N]` (D-0072): N copies of one plain value.
    ArrayRepeat(Box<Expr>, u128),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Borrow(Mode, Box<Expr>),
    // `&base[lo .. hi]` / `&mut base[lo .. hi]` (D-0047).
    SliceOf(Mode, Box<Expr>, Box<Expr>, Box<Expr>),
    // `$` inside `[…]` (D-0047): the length of what those brackets index.
    Dollar,
    Deref(Box<Expr>),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    Propagate(Box<Expr>),
    Block(Block),
    Unsafe(Block),
    If(Box<Expr>, Block, Option<Box<Expr>>), // else branch: Block or nested If, wrapped as Expr
    // `while (c) b`; a `for` loop's step (D-0035) runs after the body
    // and on `continue`.
    While(Box<Expr>, Block, Option<Box<Expr>>),
    Match(Box<Expr>, Vec<Arm>),
    Return(Option<Box<Expr>>),
    Break,
    Continue,
    Closure {
        is_move: bool,
        captures: Vec<String>,
        params: Vec<Param>,
        // D-0081: the result type, when written (`[…](…) : R { … }`).
        ret: Option<Type>,
        // Shared, not owned: the interpreter keeps the closure's body by
        // this pointer when the closure is formed, so the static pass's
        // per-expression records (keyed by address) are found inside it.
        body: std::sync::Arc<Block>,
    },
    Paren(Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

// A literal expression (D-0037): an unsuffixed numeric literal, or `-`,
// `~`, parentheses, or an arithmetic or bitwise operator applied only to
// literal expressions (a shift's amount aside). Its type comes from its
// context when that expects a number (`rule.arith.literal`
// `[Literal-Type-From-Context]`), else the default: `u64 x = 1024 * 1024;`
// is computed in `u64`.
/// A branch that is only a literal expression (`0`, `{ -1 }`): it has no
/// type of its own to offer, so it takes its sibling branches' (D-0049).
pub fn is_literal_branch(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Block(b) => is_literal_block(b),
        ExprKind::Paren(x) => is_literal_branch(x),
        _ => is_literal_expr(e),
    }
}

pub fn is_literal_block(b: &Block) -> bool {
    b.stmts.is_empty() && b.tail.as_ref().map_or(false, |t| is_literal_branch(t))
}

/// D-0104: a destructuring's (field, binder) pairs: the named ones, and
/// with `..` each other field of `decl_fields` bound to a hidden name
/// (it ends as an unused binding does).
pub fn destructure_pairs(names: &[String], binders: &[String], rest: bool, decl_fields: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = names.iter().cloned().zip(binders.iter().cloned()).collect();
    if rest {
        for f in decl_fields {
            if !names.contains(f) {
                out.push((f.clone(), format!("$rest_{}", f)));
            }
        }
    }
    out
}

pub fn is_literal_expr(e: &Expr) -> bool {
    match &e.kind {
        // D-0102: a byte literal takes its type from its context too.
        ExprKind::IntLit(_, Some(s)) if s == "byte" => true,
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Paren(x) | ExprKind::Unary(UnOp::Neg | UnOp::BitNot, x) => literal_memo(e, x, None),
        ExprKind::Binary(op, a, b) => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => literal_memo(e, a, Some(b)),
            BinOp::Shl | BinOp::Shr => literal_memo(e, a, None),
            _ => false,
        },
        _ => false,
    }
}

thread_local! {
    // Each checked and evaluated level of a long operator chain asks about
    // its whole subtree; remembered per node (by its address and its
    // operands'), the question is answered once (a 40 000-term sum was
    // quadratic, round-6 friction).
    static LITERAL_WARM: std::cell::Cell<bool> = std::cell::Cell::new(false);
    static LITERAL_MEMO: std::cell::RefCell<std::collections::HashMap<usize, (usize, usize, bool)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Forgets every remembered answer: the memo is keyed by node addresses,
/// which a later program's tree may reuse once this one's is freed, so
/// each program starts it empty (`build_items_checked`).
pub fn reset_literal_memo() {
    LITERAL_MEMO.with(|m| m.borrow_mut().clear());
}

// `is_literal_expr` of `e` with operands `a` (and `b`): a leaf operand is
// answered directly, a subtree once.
fn literal_memo(e: &Expr, a: &Expr, b: Option<&Expr>) -> bool {
    let shallow = |x: &Expr| !matches!(x.kind, ExprKind::Binary(..) | ExprKind::Paren(_) | ExprKind::Unary(..));
    if shallow(a) && b.map_or(true, shallow) {
        return is_literal_expr(a) && b.map_or(true, is_literal_expr);
    }
    let key = (e as *const Expr as usize, a as *const Expr as usize, b.map_or(0, |x| x as *const Expr as usize));
    if let Some(v) = LITERAL_MEMO.with(|m| m.borrow().get(&key.0).filter(|c| (c.0, c.1) == (key.1, key.2)).map(|c| c.2)) {
        return v;
    }
    // The deeper operand first, iteratively down a left-leaning chain, so
    // the recursion stays shallow: each node below is answered from its
    // operand's answer just remembered (`LITERAL_WARM` stops it walking
    // the chain again).
    if !LITERAL_WARM.with(|w| w.get()) {
        let mut chain = Vec::new();
        let mut cur = a;
        while let ExprKind::Binary(_, x, _) | ExprKind::Paren(x) | ExprKind::Unary(_, x) = &cur.kind {
            chain.push(cur);
            cur = x;
        }
        LITERAL_WARM.with(|w| w.set(true));
        for n in chain.iter().rev() {
            is_literal_expr(n);
        }
        LITERAL_WARM.with(|w| w.set(false));
    }
    let v = is_literal_expr(a) && b.map_or(true, is_literal_expr);
    LITERAL_MEMO.with(|m| m.borrow_mut().insert(key.0, (key.1, key.2, v)));
    v
}
