#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub len: usize,
}

impl Span {
    pub fn new(line: usize, column: usize, len: usize) -> Self {
        Self {
            line,
            column,
            len: len.max(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRefKind {
    Named(String),
    Unit,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRef {
    pub kind: TypeRefKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i128),
    Float(f64),
    Bool(bool),
    Char(char),
    String(String),
    Var(String),
    UnaryOp(String, Box<Expr>),
    BinaryOp(Box<Expr>, String, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Let {
        name: String,
        mutable: bool,
        ty: Option<TypeRef>,
        expr: Expr,
    },
    Const {
        name: String,
        ty: Option<TypeRef>,
        expr: Expr,
    },
    Expr(Expr),
    If {
        cond: Expr,
        then_block: Vec<Stmt>,
        else_block: Vec<Stmt>,
    },
    Func {
        name: String,
        params: Vec<Param>,
        return_type: Option<TypeRef>,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
    Print(Expr),
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}
