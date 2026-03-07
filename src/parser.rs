use crate::ast::{Expr, Stmt};
use crate::error::CompileError;
use crate::lexer::Lexer;
use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, CompileError> {
        let mut stmts = Vec::new();

        while !self.at_eof() {
            stmts.push(self.parse_stmt()?);
        }

        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, CompileError> {
        if self.match_ident("let") {
            return self.parse_let();
        }
        if self.match_ident("fn") {
            return self.parse_func();
        }
        if self.match_ident("if") {
            return self.parse_if();
        }
        if self.match_ident("return") {
            return self.parse_return();
        }
        if self.match_ident("print") {
            return self.parse_print();
        }

        let expr = self.parse_expr()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Expr(expr))
    }

    fn parse_let(&mut self) -> Result<Stmt, CompileError> {
        let var_name = self.expect_ident("nome de variável após 'let'")?;
        self.expect_symbol("=", "'=' após nome da variável")?;
        let expr = self.parse_expr()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Let(var_name, expr))
    }

    fn parse_func(&mut self) -> Result<Stmt, CompileError> {
        let name = self.expect_ident("nome da função após 'fn'")?;
        self.expect_symbol("(", "'(' após nome da função")?;

        let mut params = Vec::new();
        if !self.check_symbol(")") {
            loop {
                params.push(self.expect_ident("nome de parâmetro")?);
                if self.match_symbol(",") {
                    continue;
                }
                break;
            }
        }

        self.expect_symbol(")", "')' após parâmetros")?;
        self.expect_symbol("{", "'{' após assinatura da função")?;
        let body = self.parse_block()?;
        Ok(Stmt::Func(name, params, body))
    }

    fn parse_if(&mut self) -> Result<Stmt, CompileError> {
        let cond = self.parse_expr()?;
        self.expect_symbol("{", "'{' após condição do if")?;
        let then_block = self.parse_block()?;

        let else_block = if self.match_ident("else") {
            self.expect_symbol("{", "'{' após else")?;
            self.parse_block()?
        } else {
            Vec::new()
        };

        Ok(Stmt::If(cond, then_block, else_block))
    }

    fn parse_return(&mut self) -> Result<Stmt, CompileError> {
        let expr = self.parse_expr()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Return(expr))
    }

    fn parse_print(&mut self) -> Result<Stmt, CompileError> {
        let expr = if self.match_symbol("(") {
            let value = self.parse_expr()?;
            self.expect_symbol(")", "')' para fechar print(...)")?;
            value
        } else {
            self.parse_expr()?
        };
        self.consume_optional_semicolon();
        Ok(Stmt::Print(expr))
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, CompileError> {
        let mut stmts = Vec::new();

        while !self.check_symbol("}") && !self.at_eof() {
            stmts.push(self.parse_stmt()?);
        }

        self.expect_symbol("}", "'}' para fechar bloco")?;
        Ok(stmts)
    }

    fn parse_expr(&mut self) -> Result<Expr, CompileError> {
        self.parse_precedence(1)
    }

    fn parse_precedence(&mut self, min_prec: u8) -> Result<Expr, CompileError> {
        let mut left = self.parse_unary()?;

        loop {
            let Some(op) = self.current_binary_op() else {
                break;
            };

            let prec = precedence(&op);
            if prec < min_prec {
                break;
            }

            self.advance();
            let right = self.parse_precedence(prec + 1)?;
            left = Expr::BinaryOp(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, CompileError> {
        if self.match_symbol("-") {
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp("-".to_string(), Box::new(expr)));
        }

        if self.match_symbol("!") {
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp("!".to_string(), Box::new(expr)));
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, CompileError> {
        let tok = self.peek().clone();

        match tok.kind {
            TokenKind::Number => {
                self.advance();
                let value = tok.lexeme.parse::<i64>().map_err(|_| {
                    CompileError::new(
                        format!("inteiro inválido: {}", tok.lexeme),
                        tok.line,
                        tok.column,
                    )
                })?;
                Ok(Expr::Number(value))
            }
            TokenKind::String => {
                self.advance();
                Ok(Expr::String(tok.lexeme))
            }
            TokenKind::Ident => {
                self.advance();
                let mut name = tok.lexeme;
                while self.match_symbol(".") {
                    let member = self.expect_ident("identificador após '.'")?;
                    name.push('.');
                    name.push_str(&member);
                }

                if name == "true" {
                    return Ok(Expr::Bool(true));
                }
                if name == "false" {
                    return Ok(Expr::Bool(false));
                }

                if self.match_symbol("(") {
                    let mut args = Vec::new();
                    if !self.check_symbol(")") {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.match_symbol(",") {
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect_symbol(")", "')' após argumentos da função")?;
                    Ok(Expr::Call(name, args))
                } else {
                    if name.contains('.') {
                        return Err(CompileError::new(
                            format!("esperado chamada de função após '{name}'"),
                            tok.line,
                            tok.column,
                        )
                        .with_span(name.chars().count())
                        .with_label("acesso com '.' só é permitido para chamadas")
                        .with_help("use parênteses, por exemplo: fs.read(\"arquivo.txt\")"));
                    }
                    Ok(Expr::Var(name))
                }
            }
            TokenKind::Symbol if tok.lexeme == "(" => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect_symbol(")", "')' para fechar expressão")?;
                Ok(expr)
            }
            _ => Err(CompileError::new(
                format!("token inesperado: '{}'", tok.lexeme),
                tok.line,
                tok.column,
            )
            .with_span(tok.lexeme.chars().count().max(1))
            .with_label("não esperava este token aqui")),
        }
    }

    fn current_binary_op(&self) -> Option<String> {
        let tok = self.peek();
        if tok.kind != TokenKind::Symbol {
            return None;
        }
        if precedence(&tok.lexeme) == 0 {
            return None;
        }
        Some(tok.lexeme.clone())
    }

    fn consume_optional_semicolon(&mut self) {
        if self.check_symbol(";") {
            self.advance();
        }
    }

    fn expect_symbol(&mut self, symbol: &str, context: &str) -> Result<(), CompileError> {
        if self.check_symbol(symbol) {
            self.advance();
            return Ok(());
        }
        let tok = self.peek().clone();
        Err(CompileError::new(
            format!("esperado {context}, encontrado '{}'", tok.lexeme),
            tok.line,
            tok.column,
        )
        .with_span(tok.lexeme.chars().count().max(1))
        .with_label("token encontrado aqui"))
    }

    fn expect_ident(&mut self, context: &str) -> Result<String, CompileError> {
        let tok = self.peek().clone();
        if tok.kind == TokenKind::Ident {
            self.advance();
            Ok(tok.lexeme)
        } else {
            Err(CompileError::new(
                format!("esperado {context}, encontrado '{}'", tok.lexeme),
                tok.line,
                tok.column,
            )
            .with_span(tok.lexeme.chars().count().max(1))
            .with_label("identificador esperado"))
        }
    }

    fn match_ident(&mut self, expected: &str) -> bool {
        let tok = self.peek();
        if tok.kind == TokenKind::Ident && tok.lexeme == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check_symbol(&self, expected: &str) -> bool {
        let tok = self.peek();
        tok.kind == TokenKind::Symbol && tok.lexeme == expected
    }

    fn match_symbol(&mut self, expected: &str) -> bool {
        if self.check_symbol(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn at_eof(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn peek(&self) -> &Token {
        // Token EOF é sempre adicionado pelo lexer.
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len().saturating_sub(1) {
            self.pos += 1;
        }
    }
}

fn precedence(op: &str) -> u8 {
    match op {
        "||" => 1,
        "&&" => 2,
        "==" | "!=" => 3,
        ">" | "<" | ">=" | "<=" => 4,
        "+" | "-" => 5,
        "*" | "/" | "%" => 6,
        _ => 0,
    }
}

pub fn parse_source(src: &str) -> Result<Vec<Stmt>, CompileError> {
    let lexer = Lexer::new(src);
    let tokens = lexer.lex()?;
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}
