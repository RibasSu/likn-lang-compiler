#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Number,
    String,
    Ident,
    Symbol,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}
