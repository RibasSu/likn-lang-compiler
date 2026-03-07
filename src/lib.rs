pub mod ast;
pub mod cli;
pub mod codegen;
pub mod compiler;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod typecheck;

pub use cli::{parse_cli, print_help};
pub use compiler::compile_file;

pub fn run(args: &[String]) -> i32 {
    if args.len() == 1 {
        eprintln!("Uso: {} <arquivo.ikn>", args[0]);
        return 1;
    }

    if args[1] == "--help" || args[1] == "-h" {
        print_help(&args[0]);
        return 0;
    }

    let options = match parse_cli(args) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("Erro: {err}");
            eprintln!("Use --help para ver as opções.");
            return 1;
        }
    };

    match compile_file(&options) {
        Ok(artifacts) => {
            if options.emit_rust {
                println!("Arquivo Rust gerado: {}", artifacts.rust_file);
            }
            println!("Artefato final: {}", artifacts.output_file);
            println!(
                "Build concluído (target={}, profile={})",
                options.target.as_str(),
                options.profile.as_str()
            );
            0
        }
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cli::{BuildProfile, BuildTarget, parse_cli};
    use super::codegen::compile_program;
    use super::parser::parse_source;
    use super::typecheck::check_program;

    #[test]
    fn parse_respects_precedence() {
        let ast = parse_source("print 1 + 2 * 3").expect("deve fazer parse");
        let types = check_program(&ast).expect("deve tipar");
        let compiled = compile_program(&ast, BuildTarget::Native, &types);
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
        let types = check_program(&ast).expect("deve tipar");
        let compiled = compile_program(&ast, BuildTarget::Native, &types);
        assert!(compiled.contains("fn soma(a: i64, b: i64) -> i64"));
        assert!(compiled.contains("soma(10, 5)"));
    }

    #[test]
    fn lexer_supports_comparison_without_spaces() {
        let ast = parse_source("if 10>=5 { print \"ok\" }").expect("deve fazer parse");
        let types = check_program(&ast).expect("deve tipar");
        let compiled = compile_program(&ast, BuildTarget::Native, &types);
        assert!(compiled.contains("(10 >= 5)"));
    }

    #[test]
    fn parser_reports_unfinished_string() {
        let err = parse_source("print \"abc").expect_err("deve falhar");
        assert!(err.to_string().contains("string não terminada"));
    }

    #[test]
    fn web_target_emits_wasm_entry_and_print_helper() {
        let ast = parse_source("print \"ola\"").expect("deve fazer parse");
        let types = check_program(&ast).expect("deve tipar");
        let compiled = compile_program(&ast, BuildTarget::Web, &types);
        assert!(compiled.contains("pub extern \"C\" fn likn_main()"));
        assert!(compiled.contains("fn likn_print"));
    }

    #[test]
    fn cli_parser_understands_web_fast_mode() {
        let args = vec![
            "likn".to_string(),
            "--web".to_string(),
            "--fast".to_string(),
            "app.ikn".to_string(),
        ];
        let options = parse_cli(&args).expect("deve parsear argumentos");
        assert_eq!(options.target, BuildTarget::Web);
        assert_eq!(options.profile, BuildProfile::Fast);
        assert_eq!(options.input, "app.ikn");
    }

    #[test]
    fn parse_namespaced_stdlib_call() {
        let ast = parse_source("print(fs.exists(\"Cargo.toml\"))").expect("deve fazer parse");
        let types = check_program(&ast).expect("deve tipar");
        let compiled = compile_program(&ast, BuildTarget::Native, &types);
        assert!(compiled.contains("likn_fs_exists(&String::from(\"Cargo.toml\"))"));
    }

    #[test]
    fn term_input_and_output_codegen() {
        let src = r#"
            let nome = term.input("Nome: ")
            term.println(nome)
        "#;
        let ast = parse_source(src).expect("deve fazer parse");
        let types = check_program(&ast).expect("deve tipar");
        let compiled = compile_program(&ast, BuildTarget::Native, &types);
        assert!(compiled.contains("likn_term_input(&String::from(\"Nome: \"))"));
        assert!(compiled.contains("likn_print(nome)"));
    }

    #[test]
    fn supports_type_annotations_mut_const_and_unit() {
        let src = r#"
            const base: i64 = 10
            fn soma(a: i64, b: i64) -> i64 {
                return a + b
            }
            fn log(msg: String) -> () {
                term.println(msg)
                return;
            }
            let mut x: i64 = base
            let x = soma(x, 2)
            print(x)
        "#;
        let ast = parse_source(src).expect("parse");
        let types = check_program(&ast).expect("tipagem");
        let compiled = compile_program(&ast, BuildTarget::Native, &types);
        assert!(compiled.contains("fn soma(a: i64, b: i64) -> i64"));
        assert!(compiled.contains("fn log(msg: String) -> ()"));
        assert!(compiled.contains("let mut x: i64 = base;"));
        assert!(compiled.contains("let x = soma(x, 2);"));
    }

    #[test]
    fn type_error_for_if_non_bool() {
        let ast = parse_source("if 10 { print(\"ok\") }").expect("parse");
        let err = check_program(&ast).expect_err("deve falhar");
        let rendered = err.to_string();
        assert!(rendered.contains("condição de if deve ser bool"));
    }

    #[test]
    fn supports_python_like_keywords_with_js_blocks() {
        let src = r#"
            # comentário estilo Python
            def score(x: i64) -> i64: {
                if x > 10 and not false: {
                    return x
                } elif x > 5: {
                    return x + 1
                } else: {
                    return x + 2
                }
            }
            print(score(7))
        "#;
        let ast = parse_source(src).expect("parse");
        let types = check_program(&ast).expect("tipagem");
        let compiled = compile_program(&ast, BuildTarget::Native, &types);
        assert!(compiled.contains("fn score(x: i64) -> i64"));
        assert!(compiled.contains("if ((x > 10) && (!false))"));
        assert!(compiled.contains("else"));
    }

    #[test]
    fn diagnostic_renders_with_source_snippet() {
        let src = "print(\"abc";
        let err = parse_source(src)
            .expect_err("deve falhar")
            .with_source_context("demo.ikn", src);
        let rendered = err.to_string();
        assert!(rendered.contains("error: string não terminada"));
        assert!(rendered.contains("--> demo.ikn:1:7"));
        assert!(rendered.contains("1 | print(\"abc"));
        assert!(rendered.contains("^"));
    }
}
