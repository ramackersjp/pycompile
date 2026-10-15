use crate::lexer::FPart;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Mod,
    Pow,
    LShift,
    RShift,
    BitAnd,
    BitOr,
    BitXor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Pos,
    Not,
    Invert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    NotIn,
    Is,
    IsNot,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    None,
    FStr(Vec<FPart>),
    Name(String),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    UnaryOp(UnOp, Box<Expr>),
    BoolOp(BoolOp, Vec<Expr>),
    Compare(Box<Expr>, Vec<(CmpOp, Expr)>),
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        kwargs: Vec<(String, Expr)>,
        star_args: Vec<Expr>,
        dstar: Option<Box<Expr>>,
    },
    Attribute(Box<Expr>, String),
    Subscript(Box<Expr>, Box<Expr>),
    Slice {
        lower: Option<Box<Expr>>,
        upper: Option<Box<Expr>>,
        step: Option<Box<Expr>>,
    },
    List(Vec<Expr>),
    Tuple(Vec<Expr>),
    Dict(Vec<(Option<Expr>, Expr)>),
    Set(Vec<Expr>),
    IfExp {
        cond: Box<Expr>,
        body: Box<Expr>,
        orelse: Box<Expr>,
    },
    ListComp {
        elt: Box<Expr>,
        target: Box<Expr>,
        iter: Box<Expr>,
        cond: Option<Box<Expr>>,
    },
    SetComp {
        elt: Box<Expr>,
        target: Box<Expr>,
        iter: Box<Expr>,
        cond: Option<Box<Expr>>,
    },
    DictComp {
        key: Box<Expr>,
        value: Box<Expr>,
        target: Box<Expr>,
        iter: Box<Expr>,
        cond: Option<Box<Expr>>,
    },
    Lambda {
        id: usize,
        params: Vec<Param>,
        vararg: Option<String>,
        kwarg: Option<String>,
        body: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Assign {
        targets: Vec<Expr>,
        value: Expr,
    },
    AugAssign {
        target: Expr,
        op: BinOp,
        value: Expr,
    },
    Return(Option<Expr>),
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        orelse: Vec<Stmt>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
    },
    For {
        target: Expr,
        iter: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
    },
    Break,
    Continue,
    Pass,
    FunctionDef {
        id: usize,
        name: String,
        params: Vec<Param>,
        vararg: Option<String>,
        kwarg: Option<String>,
        body: Vec<Stmt>,
    },
    Global(Vec<String>),
    Import {
        module: String,
        alias: Option<String>,
    },
    FromImport {
        module: String,
        names: Vec<(String, Option<String>)>,
    },
    Assert(Expr, Option<Expr>),
    Del(Vec<Expr>),
    ClassDef {
        id: usize,
        name: String,
        bases: Vec<Expr>,
        body: Vec<Stmt>,
    },
    Raise {
        exc: Option<Expr>,
        cause: Option<Expr>,
    },
    Try {
        body: Vec<Stmt>,
        handlers: Vec<ExceptHandler>,
        orelse: Vec<Stmt>,
        finally: Vec<Stmt>,
    },
}

#[derive(Debug, Clone)]
pub struct ExceptHandler {
    pub typ: Option<Expr>,
    pub name: Option<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub body: Vec<Stmt>,
}
