// The fragment of CobaltC the model covers, as a syntax tree.
//
// Every program of the fragment is also a CobaltC program: `print.rs`
// writes it out in the house style and `parse.rs` reads it back, so the
// tools under test run exactly the text the model ran.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mode {
    Shared,
    Excl,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Mode::Shared => "shared",
            Mode::Excl => "exclusive",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ty {
    I32,
    Bool,
    Unit,
    Ref(Box<Ty>, Mode),
    Hold, // struct Hold { ref<i32, exclusive> r; }
    Vec,  // Vec<i32>
    VecR, // Vec<ref<i32, exclusive>>
    Handle(Box<Ty>),
    Opt(Box<Ty>),  // Option<…>: Vec::pop's and Channel::recv's results
    Chan(Box<Ty>), // Channel<…>
    Res(Box<Ty>),  // Result<void, …>: Channel::send's result
    Mutex,  // mutex<i32>
    Guard,  // guard<i32>
    VecH,   // Vec<handle<void>>
    Job,    // struct Job { handle<void> h; }
    Fn(Box<Ty>),  // fn() : τ -- a closure's type too
    Slice(Mode),  // slice<i32, m>
    Arr,          // array<i32, 2>
    Atomic,       // Atomic<i32> (D-0249, D-0253)
}

impl Ty {
    pub fn refi(m: Mode) -> Ty {
        Ty::Ref(Box::new(Ty::I32), m)
    }
    // A reference, or an aggregate holding one (D-0133).
    pub fn holds_ref(&self) -> bool {
        match self {
            Ty::Ref(..) | Ty::Hold | Ty::Slice(_) => true,
            Ty::Opt(t) | Ty::Res(t) => t.holds_ref(),
            _ => false,
        }
    }
    pub fn is_resource(&self) -> bool {
        matches!(self, Ty::Vec | Ty::VecR | Ty::VecH | Ty::Job | Ty::Handle(_) | Ty::Mutex | Ty::Guard | Ty::Chan(_) | Ty::Atomic)
    }
    pub fn text(&self) -> String {
        match self {
            Ty::I32 => "i32".into(),
            Ty::Bool => "bool".into(),
            Ty::Unit => "void".into(),
            Ty::Ref(t, m) => format!("ref<{}, {}>", t.text(), m.word()),
            Ty::Hold => "Hold".into(),
            Ty::Vec => "Vec<i32>".into(),
            Ty::VecR => "Vec<ref<i32, exclusive>>".into(),
            Ty::Handle(t) => format!("handle<{}>", t.text()),
            Ty::Opt(t) => format!("Option<{}>", t.text()),
            Ty::Chan(t) => format!("Channel<{}>", t.text()),
            Ty::Res(t) => format!("Result<void, {}>", t.text()),
            Ty::Mutex => "mutex<i32>".into(),
            Ty::Guard => "guard<i32>".into(),
            Ty::VecH => "Vec<handle<void>>".into(),
            Ty::Job => "Job".into(),
            Ty::Fn(r) => format!("fn() : {}", r.text()),
            Ty::Slice(m) => format!("slice<i32, {}>", m.word()),
            Ty::Arr => "array<i32, 2>".into(),
            Ty::Atomic => "Atomic<i32>".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Expr {
    Int(i64),
    Var(String),
    Deref(Box<Expr>),
    Field(Box<Expr>), // e.r, the one field of Hold (through a reference too)
    Index(String, i64), // v[k] on a Vec<i32>
    Borrow(Mode, Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>), // a helper, Vec::new, Vec::push, Vec::pop
    Spawn(String, Vec<Expr>),
    Join(String),
    HoldLit(Box<Expr>),
    // `Job { .h = e }`
    JobLit(Box<Expr>),
    // `&_m v[lo .. hi]`, v a `Vec<i32>` or a slice binding (`[Slice-Form]`)
    Slice(Mode, String, i64, i64),
    // A closure literal (`rule.fn.closure`).
    Closure(Box<Clo>),
    // A call of a closure place: `c()`, `(*f)()` (`[Closure-Call]`).
    CallVal(Box<Expr>),
    // `spawn(c)` or `spawn(move [..]() { .. })`: a closure run by a thread.
    SpawnClo(Box<Expr>),
    // `[e1, e2]`, an `array<i32, 2>`
    ArrLit(Vec<Expr>),
}

// `move? [caps]() (: τ)? { body }`, numbered in source order (`id`) so
// that the static pass and the machine name the same body.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Clo {
    pub id: usize,
    pub mv: bool,
    pub caps: Vec<String>,
    pub ret: Ty, // i32 when written `: i32`, else void
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StmtKind {
    // `auto x = e;` (ty None) or `T x = e;`
    Let { name: String, ty: Option<Ty>, init: Expr },
    Assign { place: Expr, value: Expr },
    Expr(Expr),
    Print(Expr),
    Block(Block),
    If { cond: Expr, then: Block, els: Option<Block> },
    // `_ = e;`
    Discard(Expr),
    // `while (e) { … }`
    While { cond: Expr, body: Block },
    // `match (e) { Some(b) : { … }, None : { … }, }` on an Option
    Match { scrut: Expr, binder: String, some: Block, none: Block },
    // `break;` and `continue;` (`[Break]`, `[Continue]`), inside a `while`
    Break,
    Continue,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Stmt {
    pub kind: StmtKind,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Expr>,
    pub tail_line: usize,
    pub end_line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Func {
    pub name: String,
    pub params: Vec<(Ty, String)>,
    pub ret: Ty,
    pub body: Block,
    pub line: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    pub hold: bool, // declares struct Hold
    pub job: bool,  // declares struct Job
    pub funcs: Vec<Func>,
    // D-0258: `static Atomic<i32> NAME = k;`, each one object with program
    // extent that every thread reaches by name (`spec/17` §1a′).
    pub statics: Vec<(String, i64)>,
}

impl Program {
    pub fn func(&self, name: &str) -> Option<&Func> {
        self.funcs.iter().find(|f| f.name == name)
    }
    pub fn is_static(&self, name: &str) -> bool {
        self.statics.iter().any(|(n, _)| n == name)
    }
}

// Every name an expression mentions (for last-use, D-0111).
pub fn expr_names(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Int(_) => {}
        Expr::Var(n) | Expr::Index(n, _) | Expr::Join(n) | Expr::Slice(_, n, _, _) => out.push(n.clone()),
        Expr::Deref(a) | Expr::Field(a) | Expr::Borrow(_, a) | Expr::HoldLit(a) | Expr::JobLit(a) | Expr::CallVal(a) | Expr::SpawnClo(a) => expr_names(a, out),
        // A closure mentions what it captures.
        Expr::Closure(c) => out.extend(c.caps.iter().cloned()),
        Expr::Add(a, b) | Expr::Gt(a, b) => {
            expr_names(a, out);
            expr_names(b, out);
        }
        Expr::Call(_, xs) | Expr::Spawn(_, xs) | Expr::ArrLit(xs) => {
            for x in xs {
                expr_names(x, out);
            }
        }
    }
}

pub fn stmt_names(s: &Stmt, out: &mut Vec<String>) {
    match &s.kind {
        StmtKind::Let { name, init, .. } => {
            out.push(name.clone());
            expr_names(init, out);
        }
        StmtKind::Assign { place, value } => {
            expr_names(place, out);
            expr_names(value, out);
        }
        StmtKind::Expr(e) | StmtKind::Print(e) | StmtKind::Discard(e) => expr_names(e, out),
        StmtKind::Match { scrut, binder, some, none } => {
            expr_names(scrut, out);
            out.push(binder.clone());
            block_names(some, out);
            block_names(none, out);
        }
        StmtKind::Block(b) => block_names(b, out),
        StmtKind::If { cond, then, els } => {
            expr_names(cond, out);
            block_names(then, out);
            if let Some(b) = els {
                block_names(b, out);
            }
        }
        StmtKind::While { cond, body } => {
            expr_names(cond, out);
            block_names(body, out);
        }        StmtKind::Break | StmtKind::Continue => {}
    }
}

pub fn block_names(b: &Block, out: &mut Vec<String>) {
    for s in &b.stmts {
        stmt_names(s, out);
    }
    if let Some(t) = &b.tail {
        expr_names(t, out);
    }
}

// Every closure literal in a block, nested ones included, in source order.
pub fn block_closures<'a>(b: &'a Block, out: &mut Vec<&'a Clo>) {
    for s in &b.stmts {
        stmt_closures(s, out);
    }
    if let Some(t) = &b.tail {
        expr_closures(t, out);
    }
}

pub fn stmt_closures<'a>(s: &'a Stmt, out: &mut Vec<&'a Clo>) {
    match &s.kind {
        StmtKind::Let { init, .. } => expr_closures(init, out),
        StmtKind::Assign { place, value } => {
            expr_closures(place, out);
            expr_closures(value, out);
        }
        StmtKind::Expr(e) | StmtKind::Print(e) | StmtKind::Discard(e) => expr_closures(e, out),
        StmtKind::Block(b) => block_closures(b, out),
        StmtKind::If { cond, then, els } => {
            expr_closures(cond, out);
            block_closures(then, out);
            if let Some(b) = els {
                block_closures(b, out);
            }
        }
        StmtKind::While { cond, body } => {
            expr_closures(cond, out);
            block_closures(body, out);
        }
        StmtKind::Match { scrut, some, none, .. } => {
            expr_closures(scrut, out);
            block_closures(some, out);
            block_closures(none, out);
        }        StmtKind::Break | StmtKind::Continue => {}
    }
}

pub fn expr_closures<'a>(e: &'a Expr, out: &mut Vec<&'a Clo>) {
    match e {
        Expr::Closure(c) => {
            out.push(c);
            block_closures(&c.body, out);
        }
        Expr::Deref(a) | Expr::Field(a) | Expr::Borrow(_, a) | Expr::HoldLit(a) | Expr::JobLit(a) | Expr::CallVal(a) | Expr::SpawnClo(a) => expr_closures(a, out),
        Expr::Add(a, b) | Expr::Gt(a, b) => {
            expr_closures(a, out);
            expr_closures(b, out);
        }
        Expr::Call(_, xs) | Expr::Spawn(_, xs) | Expr::ArrLit(xs) => xs.iter().for_each(|x| expr_closures(x, out)),
        _ => {}
    }
}

// The capture mode of `x` in a closure body (`rule.fn.closure`): exclusive
// iff the body writes, borrows exclusively or moves a place whose root is
// `x` -- `x`, `x.r`, `*x`, `x[k]` and the like (D-0017's syntactic scan).
pub fn writes_root(b: &Block, x: &str) -> bool {
    fn root(e: &Expr) -> Option<&str> {
        match e {
            Expr::Var(n) | Expr::Index(n, _) | Expr::Slice(_, n, _, _) => Some(n),
            Expr::Deref(a) | Expr::Field(a) => root(a),
            _ => None,
        }
    }
    fn in_expr(e: &Expr, x: &str) -> bool {
        match e {
            Expr::Borrow(Mode::Excl, a) => root(a) == Some(x) || in_expr(a, x),
            Expr::Slice(Mode::Excl, n, _, _) => n == x,
            Expr::Deref(a) | Expr::Field(a) | Expr::Borrow(_, a) | Expr::HoldLit(a) | Expr::JobLit(a) | Expr::CallVal(a) | Expr::SpawnClo(a) => in_expr(a, x),
            Expr::Add(a, b) | Expr::Gt(a, b) => in_expr(a, x) || in_expr(b, x),
            Expr::Call(_, xs) | Expr::Spawn(_, xs) | Expr::ArrLit(xs) => xs.iter().any(|a| in_expr(a, x)),
            Expr::Closure(c) => writes_root(&c.body, x) && c.caps.iter().any(|n| n == x),
            _ => false,
        }
    }
    fn in_block(b: &Block, x: &str) -> bool {
        b.stmts.iter().any(|s| match &s.kind {
            StmtKind::Assign { place, value } => root(place) == Some(x) || in_expr(place, x) || in_expr(value, x),
            StmtKind::Let { init, .. } => in_expr(init, x),
            StmtKind::Expr(e) | StmtKind::Print(e) | StmtKind::Discard(e) => in_expr(e, x),
            StmtKind::Block(b) => in_block(b, x),
            StmtKind::If { cond, then, els } => in_expr(cond, x) || in_block(then, x) || els.as_ref().map_or(false, |b| in_block(b, x)),
            StmtKind::While { cond, body } => in_expr(cond, x) || in_block(body, x),
            StmtKind::Match { scrut, some, none, .. } => in_expr(scrut, x) || in_block(some, x) || in_block(none, x),            StmtKind::Break | StmtKind::Continue => false,
        }) || b.tail.as_ref().map_or(false, |t| in_expr(t, x))
    }
    in_block(b, x)
}
