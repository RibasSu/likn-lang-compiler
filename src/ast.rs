#[derive(Debug, Clone)]
pub enum Expr {
    Number(i64),
    Bool(bool),
    String(String),
    Var(String),
    UnaryOp(String, Box<Expr>),
    BinaryOp(Box<Expr>, String, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(String, Expr),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    Func(String, Vec<String>, Vec<Stmt>),
    Return(Expr),
    Print(Expr),
}
