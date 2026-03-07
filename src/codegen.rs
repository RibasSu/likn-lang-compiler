use crate::ast::{Expr, Stmt};
use crate::cli::BuildTarget;

pub fn compile_program(ast: &[Stmt], target: BuildTarget) -> String {
    let mut functions = Vec::new();
    let mut main_stmts = Vec::new();

    for stmt in ast {
        match stmt {
            Stmt::Func(_, _, _) => functions.push(compile_stmt(stmt, target)),
            _ => main_stmts.push(compile_stmt(stmt, target)),
        }
    }

    let mut rust_code = String::new();

    if target == BuildTarget::Web {
        rust_code.push_str(
            "#[cfg(target_arch = \"wasm32\")]\nextern \"C\" {\n    fn likn_console_log(ptr: *const u8, len: usize);\n}\n\n",
        );
        rust_code.push_str(
            "fn likn_print<T: std::fmt::Display>(value: T) {\n    #[cfg(target_arch = \"wasm32\")]\n    {\n        let rendered = value.to_string();\n        unsafe {\n            likn_console_log(rendered.as_ptr(), rendered.len());\n        }\n    }\n    #[cfg(not(target_arch = \"wasm32\"))]\n    {\n        println!(\"{}\", value);\n    }\n}\n\n",
        );
    }

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

    rust_code.push_str(&format!("fn {entry_name}() -> i64 {{\n"));
    for stmt in main_stmts {
        rust_code.push_str("    ");
        rust_code.push_str(&stmt.replace('\n', "\n    "));
        rust_code.push('\n');
    }
    rust_code.push_str("    0\n}\n\n");

    if target == BuildTarget::Native {
        rust_code.push_str("fn main() {\n");
        rust_code.push_str("    let _ = __likn_entry();\n");
        rust_code.push_str("}\n");
    } else {
        rust_code.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
        rust_code.push_str("fn main() {\n");
        rust_code.push_str("    let _ = likn_main();\n");
        rust_code.push_str("}\n");
    }

    rust_code
}

fn compile_stmt(stmt: &Stmt, target: BuildTarget) -> String {
    match stmt {
        Stmt::Let(name, expr) => format!("let mut {name} = {};", compile_expr(expr)),
        Stmt::Expr(expr) => format!("{};", compile_expr(expr)),
        Stmt::If(cond, then_block, else_block) => {
            let then_code = then_block
                .iter()
                .map(|stmt| compile_stmt(stmt, target))
                .collect::<Vec<_>>()
                .join("\n");
            let else_code = else_block
                .iter()
                .map(|stmt| compile_stmt(stmt, target))
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

            let body_code = body
                .iter()
                .map(|stmt| compile_stmt(stmt, target))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "fn {name}({params_code}) -> i64 {{\n{}\n    0\n}}",
                indent_block(&body_code, 1)
            )
        }
        Stmt::Return(expr) => format!("return {};", compile_expr(expr)),
        Stmt::Print(expr) => {
            if target == BuildTarget::Web {
                format!("likn_print({});", compile_expr(expr))
            } else {
                format!("println!(\"{{}}\", {});", compile_expr(expr))
            }
        }
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
