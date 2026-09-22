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
}

#[derive(Debug, Clone)]
pub struct VariantDecl {
    pub name: String,
    pub payload: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub export: bool,
    pub assoc_type: Option<String>, // `fn Type::name`
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub ret: Type,
    pub body: Block,
    pub line: usize, // the declared name's line (`[Item-Duplicate]`'s location)
    // `const τ NAME = e;` (D-0036): a function of no parameters whose
    // body is `e`; a use of `NAME` is a call of it (`modres`), folded to
    // a literal where `consts` can.
    pub is_const: bool,
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
    Import(String, usize),           // path, declaring line
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
    },
    // `Name { f1, f2, … } = e;` (D-0044: every field, each once).
    Destructure {
        struct_name: String,
        fields: Vec<String>,
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
        body: Block,
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

pub fn is_literal_expr(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::IntLit(_, None) | ExprKind::FloatLit(_, None) => true,
        ExprKind::Paren(x) | ExprKind::Unary(UnOp::Neg | UnOp::BitNot, x) => is_literal_expr(x),
        ExprKind::Binary(op, a, b) => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                is_literal_expr(a) && is_literal_expr(b)
            }
            BinOp::Shl | BinOp::Shr => is_literal_expr(a),
            _ => false,
        },
        _ => false,
    }
}
