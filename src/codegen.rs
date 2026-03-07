use crate::ast::{Expr, ExprKind, Stmt, StmtKind, TypeRef, TypeRefKind};
use crate::cli::BuildTarget;
use crate::typecheck::{Type, TypeInfo};

const STDLIB_PRELUDE: &str = r#"#[cfg(target_arch = "wasm32")]
extern "C" {
    fn likn_console_log(ptr: *const u8, len: usize);
}

fn likn_print<T: std::fmt::Display>(value: T) {
    #[cfg(target_arch = "wasm32")]
    {
        let rendered = value.to_string();
        unsafe {
            likn_console_log(rendered.as_ptr(), rendered.len());
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("{}", value);
    }
}

fn likn_eprint<T: std::fmt::Display>(value: T) {
    #[cfg(target_arch = "wasm32")]
    {
        likn_print(value);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        eprintln!("{}", value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn likn_term_input(prompt: impl AsRef<str>) -> String {
    use std::io::{self, Write};

    let prompt = prompt.as_ref();
    print!("{}", prompt);
    io::stdout().flush().expect("falha ao esvaziar stdout");

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("falha ao ler entrada do terminal");

    while input.ends_with('\n') || input.ends_with('\r') {
        input.pop();
    }

    input
}

#[cfg(target_arch = "wasm32")]
fn likn_term_input(_prompt: impl AsRef<str>) -> String {
    panic!("term.input não é suportado em wasm32-unknown-unknown")
}

#[cfg(not(target_arch = "wasm32"))]
fn likn_fs_read(path: impl AsRef<str>) -> String {
    std::fs::read_to_string(path.as_ref()).expect("falha ao ler arquivo")
}

#[cfg(target_arch = "wasm32")]
fn likn_fs_read(_path: impl AsRef<str>) -> String {
    panic!("fs.read não é suportado em wasm32-unknown-unknown")
}

#[cfg(not(target_arch = "wasm32"))]
fn likn_fs_write(path: impl AsRef<str>, content: impl AsRef<str>) {
    std::fs::write(path.as_ref(), content.as_ref()).expect("falha ao escrever arquivo");
}

#[cfg(target_arch = "wasm32")]
fn likn_fs_write(_path: impl AsRef<str>, _content: impl AsRef<str>) {
    panic!("fs.write não é suportado em wasm32-unknown-unknown")
}

#[cfg(not(target_arch = "wasm32"))]
fn likn_fs_append(path: impl AsRef<str>, content: impl AsRef<str>) {
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.as_ref())
        .expect("falha ao abrir arquivo para append");

    file.write_all(content.as_ref().as_bytes())
        .expect("falha ao anexar conteúdo");
}

#[cfg(target_arch = "wasm32")]
fn likn_fs_append(_path: impl AsRef<str>, _content: impl AsRef<str>) {
    panic!("fs.append não é suportado em wasm32-unknown-unknown")
}

#[cfg(not(target_arch = "wasm32"))]
fn likn_fs_exists(path: impl AsRef<str>) -> bool {
    std::path::Path::new(path.as_ref()).exists()
}

#[cfg(target_arch = "wasm32")]
fn likn_fs_exists(_path: impl AsRef<str>) -> bool {
    panic!("fs.exists não é suportado em wasm32-unknown-unknown")
}

fn likn_str_concat(a: impl AsRef<str>, b: impl AsRef<str>) -> String {
    let mut out = String::new();
    out.push_str(a.as_ref());
    out.push_str(b.as_ref());
    out
}

fn likn_str_from_int(value: i64) -> String {
    value.to_string()
}

fn likn_str_from_bool(value: bool) -> String {
    value.to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn likn_sqlite_exec(path: impl AsRef<str>, sql: impl AsRef<str>) {
    let output = std::process::Command::new("sqlite3")
        .arg(path.as_ref())
        .arg(sql.as_ref())
        .output()
        .expect("falha ao executar sqlite3");
    if !output.status.success() {
        panic!(
            "sqlite.exec falhou: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn likn_sqlite_exec(_path: impl AsRef<str>, _sql: impl AsRef<str>) {
    panic!("sqlite.exec não é suportado em wasm32-unknown-unknown")
}

#[cfg(not(target_arch = "wasm32"))]
fn likn_sqlite_query(path: impl AsRef<str>, sql: impl AsRef<str>) -> String {
    let output = std::process::Command::new("sqlite3")
        .arg(path.as_ref())
        .arg(sql.as_ref())
        .output()
        .expect("falha ao executar sqlite3");
    if !output.status.success() {
        panic!(
            "sqlite.query falhou: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[cfg(target_arch = "wasm32")]
fn likn_sqlite_query(_path: impl AsRef<str>, _sql: impl AsRef<str>) -> String {
    panic!("sqlite.query não é suportado em wasm32-unknown-unknown")
}

#[cfg(not(target_arch = "wasm32"))]
fn likn_crypto_sha256(value: impl AsRef<str>) -> String {
    use std::io::Write as _;

    let input = value.as_ref();
    let mut child = std::process::Command::new("sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("falha ao executar sha256sum");
    child
        .stdin
        .as_mut()
        .expect("stdin do sha256sum indisponível")
        .write_all(input.as_bytes())
        .expect("falha ao escrever entrada do hash");
    let output = child.wait_with_output().expect("falha ao aguardar sha256sum");
    if !output.status.success() {
        panic!(
            "crypto.sha256 falhou: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split_whitespace().next().unwrap_or("").to_string()
}

#[cfg(target_arch = "wasm32")]
fn likn_crypto_sha256(_value: impl AsRef<str>) -> String {
    panic!("crypto.sha256 não é suportado em wasm32-unknown-unknown")
}

fn likn_crypto_verify_sha256(value: impl AsRef<str>, digest: impl AsRef<str>) -> bool {
    likn_crypto_sha256(value) == digest.as_ref()
}

fn likn_crypto_random_token() -> String {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tick = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let seed = format!("{nanos}:{tick}:{}", std::process::id());

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    let digest = hasher.finish();
    format!("{:016x}{:016x}", digest, digest.rotate_left(17))
}

fn likn_html_escape(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn likn_html_page(title: impl AsRef<str>, body: impl AsRef<str>) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title></head><body>{}</body></html>",
        likn_html_escape(title),
        body.as_ref()
    )
}

#[derive(Clone)]
struct LiknRoute {
    method: String,
    path: String,
    handler: String,
}

fn likn_routes() -> &'static std::sync::Mutex<Vec<LiknRoute>> {
    static ROUTES: std::sync::OnceLock<std::sync::Mutex<Vec<LiknRoute>>> = std::sync::OnceLock::new();
    ROUTES.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn likn_web_register(method: impl AsRef<str>, path: impl AsRef<str>, handler: impl AsRef<str>) {
    let route = LiknRoute {
        method: method.as_ref().to_string(),
        path: path.as_ref().to_string(),
        handler: handler.as_ref().to_string(),
    };
    likn_routes().lock().expect("lock routes").push(route);
}

fn likn_web_start(port: i64) {
    likn_print(format!("Likn web server configurado na porta {}", port));
}

fn likn_web_run() {
    let routes = likn_routes().lock().expect("lock routes");
    for route in routes.iter() {
        likn_print(format!("route {} {} -> {}", route.method, route.path, route.handler));
    }
}

fn likn_web_html(body: impl AsRef<str>) -> String {
    body.as_ref().to_string()
}

fn likn_web_redirect(location: impl AsRef<str>) -> String {
    format!("REDIRECT:{}", location.as_ref())
}

"#;

pub fn compile_program(ast: &[Stmt], target: BuildTarget, type_info: &TypeInfo) -> String {
    let mut functions = Vec::new();
    let mut main_stmts = Vec::new();

    for stmt in ast {
        match &stmt.kind {
            StmtKind::Func { .. } => functions.push(compile_stmt(stmt, target, type_info)),
            StmtKind::Export(inner) if matches!(inner.kind, StmtKind::Func { .. }) => {
                functions.push(compile_stmt(stmt, target, type_info))
            }
            _ => {
                let compiled = compile_stmt(stmt, target, type_info);
                if !compiled.trim().is_empty() {
                    main_stmts.push(compiled);
                }
            }
        }
    }

    let mut rust_code = String::new();
    rust_code.push_str(STDLIB_PRELUDE);

    for func in functions {
        rust_code.push_str(&func);
        rust_code.push('\n');
        rust_code.push('\n');
    }

    let entry_name = if target == BuildTarget::Web {
        "likn_main"
    } else {
        "__likn_entry"
    };

    if target == BuildTarget::Web {
        rust_code.push_str("#[no_mangle]\n");
        rust_code.push_str("pub extern \"C\" ");
    }

    rust_code.push_str(&format!("fn {entry_name}() {{\n"));
    for stmt in main_stmts {
        rust_code.push_str("    ");
        rust_code.push_str(&stmt.replace('\n', "\n    "));
        rust_code.push('\n');
    }
    rust_code.push_str("}\n\n");

    if target == BuildTarget::Native {
        rust_code.push_str("fn main() {\n");
        rust_code.push_str("    __likn_entry();\n");
        rust_code.push_str("}\n");
    } else {
        rust_code.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
        rust_code.push_str("fn main() {\n");
        rust_code.push_str("    likn_main();\n");
        rust_code.push_str("}\n");
    }

    rust_code
}

fn compile_stmt(stmt: &Stmt, target: BuildTarget, type_info: &TypeInfo) -> String {
    match &stmt.kind {
        StmtKind::Import(_) => String::new(),
        StmtKind::Export(inner) => compile_stmt(inner, target, type_info),
        StmtKind::Let {
            name,
            mutable,
            ty,
            expr,
        } => {
            let name = escape_ident(name);
            let keyword = if *mutable { "let mut" } else { "let" };
            if let Some(type_ref) = ty {
                format!(
                    "{keyword} {name}: {} = {};",
                    compile_type_ref(type_ref),
                    compile_expr(expr)
                )
            } else {
                format!("{keyword} {name} = {};", compile_expr(expr))
            }
        }
        StmtKind::Const { name, ty, expr } => {
            let name = escape_ident(name);
            if let Some(type_ref) = ty {
                format!(
                    "let {name}: {} = {};",
                    compile_type_ref(type_ref),
                    compile_expr(expr)
                )
            } else {
                format!("let {name} = {};", compile_expr(expr))
            }
        }
        StmtKind::Expr(expr) => format!("{};", compile_expr(expr)),
        StmtKind::If {
            cond,
            then_block,
            else_block,
        } => {
            let then_code = then_block
                .iter()
                .map(|stmt| compile_stmt(stmt, target, type_info))
                .collect::<Vec<_>>()
                .join("\n");
            let else_code = else_block
                .iter()
                .map(|stmt| compile_stmt(stmt, target, type_info))
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
        StmtKind::Func {
            name,
            params,
            return_type,
            body,
        } => {
            let fn_name = escape_ident(name);
            let sig = type_info.function_sigs.get(name);
            let params_code = params
                .iter()
                .enumerate()
                .map(|(idx, param)| {
                    let ty = param
                        .ty
                        .as_ref()
                        .map(compile_type_ref)
                        .or_else(|| {
                            sig.and_then(|found| found.params.get(idx).map(compile_semantic_type))
                        })
                        .unwrap_or_else(|| "i64".to_string());
                    format!("{}: {ty}", escape_ident(&param.name))
                })
                .collect::<Vec<_>>()
                .join(", ");

            let ret = return_type
                .as_ref()
                .map(compile_type_ref)
                .or_else(|| sig.map(|found| compile_semantic_type(&found.ret)))
                .unwrap_or_else(|| "()".to_string());

            let body_code = compile_function_body(body, target, type_info);
            format!(
                "fn {fn_name}({params_code}) -> {ret} {{\n{}\n}}",
                indent_block(&body_code, 1)
            )
        }
        StmtKind::Return(value) => {
            if let Some(expr) = value {
                format!("return {};", compile_expr(expr))
            } else {
                "return;".to_string()
            }
        }
        StmtKind::Print(expr) => format!("likn_print({});", compile_expr(expr)),
    }
}

fn compile_expr(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Float(n) => {
            let literal = n.to_string();
            if literal.contains('.') {
                literal
            } else {
                format!("{literal}.0")
            }
        }
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Char(ch) => format!("'{}'", escape_char(*ch)),
        ExprKind::String(s) => format!("String::from(\"{}\")", escape_string(s)),
        ExprKind::Var(v) => escape_ident(v),
        ExprKind::UnaryOp(op, value) => format!("({}{})", op, compile_expr(value)),
        ExprKind::BinaryOp(lhs, op, rhs) => {
            format!("({} {} {})", compile_expr(lhs), op, compile_expr(rhs))
        }
        ExprKind::Call(name, args) => compile_call(name, args),
    }
}

fn compile_call(name: &str, args: &[Expr]) -> String {
    let args_code = args.iter().map(compile_expr).collect::<Vec<_>>();

    match name {
        "term.print" | "term.println" | "term.output" => {
            if args_code.len() == 1 {
                format!("likn_print({})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "term.eprint" | "term.eprintln" | "term.error" => {
            if args_code.len() == 1 {
                format!("likn_eprint({})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "term.input" => {
            if args_code.len() == 1 {
                format!("likn_term_input(&{})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "fs.read" => {
            if args_code.len() == 1 {
                format!("likn_fs_read(&{})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "fs.write" => {
            if args_code.len() == 2 {
                format!("likn_fs_write(&{}, &{})", args_code[0], args_code[1])
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "fs.append" => {
            if args_code.len() == 2 {
                format!("likn_fs_append(&{}, &{})", args_code[0], args_code[1])
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "fs.exists" => {
            if args_code.len() == 1 {
                format!("likn_fs_exists(&{})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "str.upper" => {
            if args_code.len() == 1 {
                format!("({}).to_uppercase()", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "str.lower" => {
            if args_code.len() == 1 {
                format!("({}).to_lowercase()", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "str.contains" => {
            if args_code.len() == 2 {
                format!("({}).contains(&{})", args_code[0], args_code[1])
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "str.len" => {
            if args_code.len() == 1 {
                format!("({}).chars().count()", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "str.concat" => {
            if args_code.len() == 2 {
                format!("likn_str_concat(&{}, &{})", args_code[0], args_code[1])
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "str.from_int" => {
            if args_code.len() == 1 {
                format!("likn_str_from_int({} as i64)", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "str.from_bool" => {
            if args_code.len() == 1 {
                format!("likn_str_from_bool({})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "sqlite.exec" => {
            if args_code.len() == 2 {
                format!("likn_sqlite_exec(&{}, &{})", args_code[0], args_code[1])
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "sqlite.query" => {
            if args_code.len() == 2 {
                format!("likn_sqlite_query(&{}, &{})", args_code[0], args_code[1])
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "crypto.sha256" => {
            if args_code.len() == 1 {
                format!("likn_crypto_sha256(&{})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "crypto.verify_sha256" => {
            if args_code.len() == 2 {
                format!(
                    "likn_crypto_verify_sha256(&{}, &{})",
                    args_code[0], args_code[1]
                )
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "crypto.random_token" => {
            if args_code.is_empty() {
                "likn_crypto_random_token()".to_string()
            } else {
                arity_error_expr(name, 0, args_code.len())
            }
        }
        "html.escape" => {
            if args_code.len() == 1 {
                format!("likn_html_escape(&{})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "html.page" => {
            if args_code.len() == 2 {
                format!("likn_html_page(&{}, &{})", args_code[0], args_code[1])
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "web.start" => {
            if args_code.len() == 1 {
                format!("likn_web_start({} as i64)", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "web.get" => {
            if args_code.len() == 2 {
                format!(
                    "likn_web_register(\"GET\", &{}, &{})",
                    args_code[0], args_code[1]
                )
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "web.post" => {
            if args_code.len() == 2 {
                format!(
                    "likn_web_register(\"POST\", &{}, &{})",
                    args_code[0], args_code[1]
                )
            } else {
                arity_error_expr(name, 2, args_code.len())
            }
        }
        "web.run" => {
            if args_code.is_empty() {
                "likn_web_run()".to_string()
            } else {
                arity_error_expr(name, 0, args_code.len())
            }
        }
        "web.html" => {
            if args_code.len() == 1 {
                format!("likn_web_html(&{})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "web.redirect" => {
            if args_code.len() == 1 {
                format!("likn_web_redirect(&{})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        "panic" => {
            if args_code.len() == 1 {
                format!("panic!(\"{{}}\", {})", args_code[0])
            } else {
                arity_error_expr(name, 1, args_code.len())
            }
        }
        _ if name.contains('.') => {
            compile_error_expr(&format!("função de biblioteca padrão desconhecida: {name}"))
        }
        _ => {
            let args_code = args_code.join(", ");
            format!("{}({args_code})", escape_ident(name))
        }
    }
}

fn compile_type_ref(ty: &TypeRef) -> String {
    match &ty.kind {
        TypeRefKind::Named(name) => match name.as_str() {
            "int" => "i64".to_string(),
            "float" => "f64".to_string(),
            "str" => "String".to_string(),
            other => other.to_string(),
        },
        TypeRefKind::Unit => "()".to_string(),
        TypeRefKind::Never => "!".to_string(),
    }
}

fn compile_semantic_type(ty: &Type) -> String {
    ty.rust_type_name().to_string()
}

fn compile_function_body(body: &[Stmt], target: BuildTarget, type_info: &TypeInfo) -> String {
    body.iter()
        .enumerate()
        .map(|(index, stmt)| {
            let is_last = index + 1 == body.len();
            if is_last {
                if let StmtKind::Expr(expr) = &stmt.kind {
                    return format!("return {};", compile_expr(expr));
                }
            }
            compile_stmt(stmt, target, type_info)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn arity_error_expr(name: &str, expected: usize, got: usize) -> String {
    compile_error_expr(&format!(
        "chamada inválida: '{name}' espera {expected} argumento(s), recebeu {got}"
    ))
}

fn compile_error_expr(message: &str) -> String {
    format!("compile_error!(\"{}\")", escape_string(message))
}

fn escape_ident(name: &str) -> String {
    if is_rust_keyword(name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

fn is_rust_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "union"
    )
}

fn escape_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('"', "\\\"")
}

fn escape_char(ch: char) -> String {
    match ch {
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        other => other.to_string(),
    }
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
