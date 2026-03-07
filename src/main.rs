use std::env;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
enum Expr {
    Number(i64),
    Bool(bool),
    String(String),
    Var(String),
    UnaryOp(String, Box<Expr>),
    BinaryOp(Box<Expr>, String, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
enum Stmt {
    Let(String, Expr),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    Func(String, Vec<String>, Vec<Stmt>),
    Return(Expr),
    Print(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Number,
    String,
    Ident,
    Symbol,
    Eof,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    lexeme: String,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone)]
struct CompileError {
    message: String,
    line: usize,
    column: usize,
}

impl CompileError {
    fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "erro em linha {}, coluna {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for CompileError {}

struct Lexer<'a> {
    _src: &'a str,
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            _src: src,
            chars: src.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn lex(mut self) -> Result<Vec<Token>, CompileError> {
        let mut tokens = Vec::new();

        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.consume_whitespace();
                continue;
            }

            if ch == '/' && self.peek_next_char() == Some('/') {
                self.consume_comment();
                continue;
            }

            if ch.is_ascii_digit() {
                tokens.push(self.lex_number()?);
                continue;
            }

            if ch == '"' {
                tokens.push(self.lex_string()?);
                continue;
            }

            if is_ident_start(ch) {
                tokens.push(self.lex_ident());
                continue;
            }

            tokens.push(self.lex_symbol()?);
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            lexeme: String::new(),
            line: self.line,
            column: self.column,
        });

        Ok(tokens)
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next_char(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += 1;

        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        Some(ch)
    }

    fn consume_whitespace(&mut self) {
        while matches!(self.peek_char(), Some(ch) if ch.is_whitespace()) {
            let _ = self.advance_char();
        }
    }

    fn consume_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            let _ = self.advance_char();
            if ch == '\n' {
                break;
            }
        }
    }

    fn lex_number(&mut self) -> Result<Token, CompileError> {
        let start_line = self.line;
        let start_column = self.column;
        let mut number = String::new();

        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
            if let Some(ch) = self.advance_char() {
                number.push(ch);
            }
        }

        if number.parse::<i64>().is_err() {
            return Err(CompileError::new(
                format!("inteiro inválido: {number}"),
                start_line,
                start_column,
            ));
        }

        Ok(Token {
            kind: TokenKind::Number,
            lexeme: number,
            line: start_line,
            column: start_column,
        })
    }

    fn lex_string(&mut self) -> Result<Token, CompileError> {
        let start_line = self.line;
        let start_column = self.column;
        let mut out = String::new();

        // opening quote
        let _ = self.advance_char();

        while let Some(ch) = self.peek_char() {
            if ch == '"' {
                let _ = self.advance_char();
                return Ok(Token {
                    kind: TokenKind::String,
                    lexeme: out,
                    line: start_line,
                    column: start_column,
                });
            }

            if ch == '\\' {
                let _ = self.advance_char();
                let escaped = self.advance_char().ok_or_else(|| {
                    CompileError::new("escape incompleto em string", start_line, start_column)
                })?;
                let mapped = match escaped {
                    'n' => '\n',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => {
                        return Err(CompileError::new(
                            format!("escape inválido: \\{other}"),
                            self.line,
                            self.column.saturating_sub(1),
                        ));
                    }
                };
                out.push(mapped);
                continue;
            }

            out.push(ch);
            let _ = self.advance_char();
        }

        Err(CompileError::new(
            "string não terminada",
            start_line,
            start_column,
        ))
    }

    fn lex_ident(&mut self) -> Token {
        let start_line = self.line;
        let start_column = self.column;
        let mut ident = String::new();

        while matches!(self.peek_char(), Some(ch) if is_ident_part(ch)) {
            if let Some(ch) = self.advance_char() {
                ident.push(ch);
            }
        }

        Token {
            kind: TokenKind::Ident,
            lexeme: ident,
            line: start_line,
            column: start_column,
        }
    }

    fn lex_symbol(&mut self) -> Result<Token, CompileError> {
        let start_line = self.line;
        let start_column = self.column;
        let first = self
            .advance_char()
            .ok_or_else(|| CompileError::new("fim inesperado", start_line, start_column))?;

        let two_char = match (first, self.peek_char()) {
            ('=', Some('=')) => Some("=="),
            ('!', Some('=')) => Some("!="),
            ('>', Some('=')) => Some(">="),
            ('<', Some('=')) => Some("<="),
            ('&', Some('&')) => Some("&&"),
            ('|', Some('|')) => Some("||"),
            _ => None,
        };

        if let Some(op) = two_char {
            let _ = self.advance_char();
            return Ok(Token {
                kind: TokenKind::Symbol,
                lexeme: op.to_string(),
                line: start_line,
                column: start_column,
            });
        }

        if "(){}.,;+*/-%!=<>".contains(first) {
            return Ok(Token {
                kind: TokenKind::Symbol,
                lexeme: first.to_string(),
                line: start_line,
                column: start_column,
            });
        }

        Err(CompileError::new(
            format!("símbolo inesperado: {first}"),
            start_line,
            start_column,
        ))
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_program(&mut self) -> Result<Vec<Stmt>, CompileError> {
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
                if tok.lexeme == "true" {
                    return Ok(Expr::Bool(true));
                }
                if tok.lexeme == "false" {
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
                    Ok(Expr::Call(tok.lexeme, args))
                } else {
                    Ok(Expr::Var(tok.lexeme))
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
            )),
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
        ))
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
            ))
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

fn compile_program(ast: &[Stmt]) -> String {
    let mut functions = Vec::new();
    let mut main_stmts = Vec::new();

    for stmt in ast {
        match stmt {
            Stmt::Func(_, _, _) => functions.push(compile_stmt(stmt)),
            _ => main_stmts.push(compile_stmt(stmt)),
        }
    }

    let mut rust_code = String::new();

    for func in functions {
        rust_code.push_str(&func);
        rust_code.push('\n');
        rust_code.push('\n');
    }

    rust_code.push_str("fn __likn_entry() -> i64 {\n");
    for stmt in main_stmts {
        rust_code.push_str("    ");
        rust_code.push_str(&stmt.replace('\n', "\n    "));
        rust_code.push('\n');
    }
    rust_code.push_str("    0\n}\n\n");

    rust_code.push_str("fn main() {\n");
    rust_code.push_str("    let _ = __likn_entry();\n");
    rust_code.push_str("}\n");

    rust_code
}

fn compile_stmt(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Let(name, expr) => format!("let mut {name} = {};", compile_expr(expr)),
        Stmt::Expr(expr) => format!("{};", compile_expr(expr)),
        Stmt::If(cond, then_block, else_block) => {
            let then_code = then_block
                .iter()
                .map(compile_stmt)
                .collect::<Vec<_>>()
                .join("\n");
            let else_code = else_block
                .iter()
                .map(compile_stmt)
                .collect::<Vec<_>>()
                .join("\n");

            if else_block.is_empty() {
                format!(
                    "if {} {{\n{}\n}}",
                    compile_expr(cond),
                    indent_block(&then_code, 1)
                )
            } else {
                format!(
                    "if {} {{\n{}\n}} else {{\n{}\n}}",
                    compile_expr(cond),
                    indent_block(&then_code, 1),
                    indent_block(&else_code, 1)
                )
            }
        }
        Stmt::Func(name, params, body) => {
            let params_code = params
                .iter()
                .map(|p| format!("{p}: i64"))
                .collect::<Vec<_>>()
                .join(", ");

            let body_code = body.iter().map(compile_stmt).collect::<Vec<_>>().join("\n");
            format!(
                "fn {name}({params_code}) -> i64 {{\n{}\n    0\n}}",
                indent_block(&body_code, 1)
            )
        }
        Stmt::Return(expr) => format!("return {};", compile_expr(expr)),
        Stmt::Print(expr) => format!("println!(\"{{}}\", {});", compile_expr(expr)),
    }
}

fn compile_expr(expr: &Expr) -> String {
    match expr {
        Expr::Number(n) => n.to_string(),
        Expr::Bool(b) => b.to_string(),
        Expr::String(s) => format!("\"{}\"", escape_string(s)),
        Expr::Var(v) => v.clone(),
        Expr::UnaryOp(op, value) => format!("({}{})", op, compile_expr(value)),
        Expr::BinaryOp(lhs, op, rhs) => {
            format!("({} {} {})", compile_expr(lhs), op, compile_expr(rhs))
        }
        Expr::Call(name, args) => {
            let args_code = args.iter().map(compile_expr).collect::<Vec<_>>().join(", ");
            format!("{name}({args_code})")
        }
    }
}

fn escape_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('"', "\\\"")
}

fn indent_block(code: &str, level: usize) -> String {
    if code.trim().is_empty() {
        return String::new();
    }

    let prefix = "    ".repeat(level);
    code.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_source(src: &str) -> Result<Vec<Stmt>, CompileError> {
    let lexer = Lexer::new(src);
    let tokens = lexer.lex()?;
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

fn compile_file(filename: &str) -> Result<(String, String), CompileError> {
    let src = fs::read_to_string(filename).map_err(|err| {
        CompileError::new(format!("falha ao ler arquivo {filename}: {err}"), 1, 1)
    })?;

    let ast = parse_source(&src)?;

    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let rust_file = format!("{stem}.rs");
    let bin_file = stem.to_string();
    let rust_code = compile_program(&ast);

    fs::write(&rust_file, rust_code).map_err(|err| {
        CompileError::new(
            format!("falha ao escrever arquivo {rust_file}: {err}"),
            1,
            1,
        )
    })?;

    let output = Command::new("rustc")
        .arg(&rust_file)
        .arg("-o")
        .arg(&bin_file)
        .output()
        .map_err(|err| CompileError::new(format!("falha ao executar rustc: {err}"), 1, 1))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CompileError::new(
            format!("erro ao compilar {rust_file}: {stderr}"),
            1,
            1,
        ));
    }

    Ok((rust_file, bin_file))
}

fn print_help(bin: &str) {
    println!("Likn Lang Compiler");
    println!();
    println!("Uso:");
    println!("  {bin} <arquivo.ikn>");
    println!();
    println!("Flags:");
    println!("  --help      Exibe esta ajuda");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        eprintln!("Uso: {} <arquivo.ikn>", args[0]);
        std::process::exit(1);
    }

    if args[1] == "--help" || args[1] == "-h" {
        print_help(&args[0]);
        return;
    }

    let filename = &args[1];

    match compile_file(filename) {
        Ok((rust_file, bin_file)) => {
            println!("Arquivo Rust gerado: {rust_file}");
            println!("Binário gerado: {bin_file}");
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_respects_precedence() {
        let ast = parse_source("print 1 + 2 * 3").expect("deve fazer parse");
        let compiled = compile_program(&ast);
        assert!(compiled.contains("(1 + (2 * 3))"));
    }

    #[test]
    fn parse_function_and_call() {
        let src = r#"
            fn soma(a, b) {
                return a + b
            }
            print(soma(10, 5))
        "#;
        let ast = parse_source(src).expect("deve fazer parse");
        let compiled = compile_program(&ast);
        assert!(compiled.contains("fn soma(a: i64, b: i64) -> i64"));
        assert!(compiled.contains("soma(10, 5)"));
    }

    #[test]
    fn lexer_supports_comparison_without_spaces() {
        let ast = parse_source("if 10>=5 { print \"ok\" }").expect("deve fazer parse");
        let compiled = compile_program(&ast);
        assert!(compiled.contains("(10 >= 5)"));
    }

    #[test]
    fn parser_reports_unfinished_string() {
        let err = parse_source("print \"abc").expect_err("deve falhar");
        assert!(err.to_string().contains("string não terminada"));
    }
}
