#[derive(Debug, Clone)]
enum Expr {
    Number(i64),
    String(String),
    Var(String),
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

struct Parser {
    tokens: Vec<String>,
    pos: usize,
}

impl Parser {
    fn new(src: &str) -> Self {
        // Tokenizador que trata strings entre aspas como um único token
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut chars = src.chars().peekable();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                chars.next();
            } else if "(){}.,".contains(c) {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                tokens.push(c.to_string());
                chars.next();
            } else if c == '"' {
                // Início de string
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                let mut string_token = String::new();
                string_token.push('"');
                chars.next();
                while let Some(&sc) = chars.peek() {
                    string_token.push(sc);
                    chars.next();
                    if sc == '"' {
                        break;
                    }
                }
                tokens.push(string_token);
            } else {
                current.push(c);
                chars.next();
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&String> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<String> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn parse(&mut self) -> Vec<Stmt> {
        let mut stmts = vec![];
        while self.pos < self.tokens.len() {
            if let Some(stmt) = self.parse_stmt() {
                stmts.push(stmt);
            } else {
                break;
            }
        }
        stmts
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        let token = self.peek()?.clone();
        match token.as_str() {
            "let" => self.parse_let(),
            "fn" => self.parse_func(),
            "if" => self.parse_if(),
            "return" => self.parse_return(),
            "print" => self.parse_print(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let(&mut self) -> Option<Stmt> {
        self.next(); // consume 'let'
        let var_name = self.next()?;
        if self.next()? != "=" {
            println!("Expected = after let");
            return None;
        }
        let expr = self.parse_expr()?;
        Some(Stmt::Let(var_name, expr))
    }

    fn parse_func(&mut self) -> Option<Stmt> {
        self.next(); // consume 'fn'
        let name = self.next()?;
        if self.next()? != "(" {
            println!("Expected ( after fn name");
            return None;
        }
        let mut params = vec![];
        while let Some(tok) = self.next() {
            if tok == ")" {
                break;
            }
            if tok != "," {
                params.push(tok);
            }
        }
        if self.next()? != "{" {
            println!("Expected {{ after fn parameters");
            return None;
        }
        let body = self.parse_block();
        Some(Stmt::Func(name, params, body))
    }

    fn parse_if(&mut self) -> Option<Stmt> {
        self.next(); // consume 'if'
        let cond = self.parse_expr()?;
        if self.next()? != "{" {
            println!("Expected {{ after if condition");
            return None;
        }
        let then_block = self.parse_block();
        let mut else_block = vec![];
        if let Some(next) = self.peek() {
            if next == "else" {
                self.next();
                if self.next()? != "{" {
                    println!("Expected {{ after else");
                    return None;
                }
                else_block = self.parse_block();
            }
        }
        Some(Stmt::If(cond, then_block, else_block))
    }

    fn parse_return(&mut self) -> Option<Stmt> {
        self.next(); // consume return
        let expr = self.parse_expr()?;
        Some(Stmt::Return(expr))
    }

    fn parse_print(&mut self) -> Option<Stmt> {
        self.next(); // consume print
        // Aceita print(expr) ou print expr
        let expr = if self.peek() == Some(&"(".to_string()) {
            self.next(); // consome '('
            let expr = self.parse_expr()?;
            if self.peek() == Some(&")".to_string()) {
                self.next(); // consome ')'
            }
            expr
        } else {
            self.parse_expr()?
        };
        Some(Stmt::Print(expr))
    }

    fn parse_expr_stmt(&mut self) -> Option<Stmt> {
        let expr = self.parse_expr()?;
        Some(Stmt::Expr(expr))
    }

    fn parse_block(&mut self) -> Vec<Stmt> {
        let mut stmts = vec![];
        while let Some(tok) = self.peek() {
            if tok == "}" {
                self.next(); // consume }
                break;
            }
            if let Some(stmt) = self.parse_stmt() {
                stmts.push(stmt);
            } else {
                break;
            }
        }
        stmts
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_binary_expr(0)
    }

    // Precedência dos operadores
    fn get_precedence(op: &str) -> u8 {
        match op {
            "==" | "!=" => 2,
            ">" | "<" | ">=" | "<=" => 3,
            "+" | "-" => 4,
            "*" | "/" => 5,
            _ => 0,
        }
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let token = self.next()?;
        if let Ok(num) = token.parse::<i64>() {
            return Some(Expr::Number(num));
        }
        if token.starts_with("\"") && token.ends_with("\"") {
            return Some(Expr::String(token.trim_matches('"').to_string()));
        }
        if self.peek() == Some(&"(".to_string()) {
            // chamada de função
            self.next(); // consome "("
            let mut args = vec![];
            while let Some(arg) = self.parse_expr() {
                args.push(arg);
                if let Some(comma_or_paren) = self.peek() {
                    if comma_or_paren == "," {
                        self.next();
                        continue;
                    } else if comma_or_paren == ")" {
                        self.next();
                        break;
                    }
                }
                break;
            }
            return Some(Expr::Call(token, args));
        }
        Some(Expr::Var(token))
    }

    fn parse_binary_expr(&mut self, min_prec: u8) -> Option<Expr> {
        let mut left = self.parse_primary()?;
        while let Some(op) = self.peek() {
            let prec = Self::get_precedence(op);
            if prec < min_prec || prec == 0 {
                break;
            }
            let op = self.next()?;
            let mut right = self.parse_primary()?;
            while let Some(next_op) = self.peek() {
                let next_prec = Self::get_precedence(next_op);
                if next_prec > prec {
                    right = self.parse_binary_expr(next_prec)?;
                } else {
                    break;
                }
            }
            left = Expr::BinaryOp(Box::new(left), op, Box::new(right));
        }
        Some(left)
    }
}

fn compile_stmt(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Let(name, expr) => format!("let {} = {};", name, compile_expr(expr)),
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
                format!("if {} {{\n{}\n}}", compile_expr(cond), then_code)
            } else {
                format!(
                    "if {} {{\n{}\n}} else {{\n{}\n}}",
                    compile_expr(cond),
                    then_code,
                    else_code
                )
            }
        }
        Stmt::Func(name, params, body) => {
            // Todos os parâmetros como i64 para simplificação
            let params_code = params
                .iter()
                .map(|p| format!("{}: i64", p))
                .collect::<Vec<_>>()
                .join(", ");
            let body_code = body.iter().map(compile_stmt).collect::<Vec<_>>().join("\n");
            format!("fn {}({}) -> i64 {{\n{}\n}}", name, params_code, body_code)
        }
        Stmt::Return(expr) => format!("return {};", compile_expr(expr)),
        Stmt::Print(expr) => format!("println!(\"{{}}\", {});", compile_expr(expr)),
    }
}

fn compile_expr(expr: &Expr) -> String {
    match expr {
        Expr::Number(n) => n.to_string(),
        Expr::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        Expr::Var(v) => v.clone(),
        Expr::BinaryOp(lhs, op, rhs) => {
            format!("{} {} {}", compile_expr(lhs), op, compile_expr(rhs))
        }
        Expr::Call(name, args) => {
            let args_code = args.iter().map(compile_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, args_code)
        }
    }
}

fn main() {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <arquivo.ikn>", args[0]);
        std::process::exit(1);
    }

    let filename = &args[1];
    let src = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Erro ao ler arquivo {}: {}", filename, e);
            std::process::exit(1);
        }
    };

    let mut parser = Parser::new(&src);
    let ast = parser.parse();

    // Gerar nome do arquivo .rs e do binário
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let rust_file = format!("{}.rs", stem);
    let bin_file = stem;

    // Gerar código Rust
    let mut rust_code = String::from("fn main() {\n");
    for stmt in &ast {
        rust_code.push_str(&compile_stmt(stmt));
        rust_code.push('\n');
    }
    rust_code.push_str("}\n");

    // Escrever arquivo .rs
    if let Err(e) = fs::write(&rust_file, rust_code) {
        eprintln!("Erro ao escrever {}: {}", rust_file, e);
        std::process::exit(1);
    }

    // Compilar com rustc
    let status = Command::new("rustc")
        .arg(&rust_file)
        .arg("-o")
        .arg(&bin_file)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Binário gerado: {}", bin_file);
        }
        Ok(s) => {
            eprintln!("Erro ao compilar {} (status: {:?})", rust_file, s.code());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Falha ao executar rustc: {}", e);
            std::process::exit(1);
        }
    }
}
