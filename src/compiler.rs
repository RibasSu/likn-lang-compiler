use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cli::{BuildProfile, BuildTarget, CliOptions};
use crate::codegen::compile_program;
use crate::error::CompileError;
use crate::parser::parse_source;

#[derive(Debug, Clone)]
pub struct CompilationArtifacts {
    pub rust_file: String,
    pub output_file: String,
}

fn add_profile_flags(command: &mut Command, target: BuildTarget, profile: BuildProfile) {
    match profile {
        BuildProfile::Dev => {
            if target == BuildTarget::Web {
                command.args(["-C", "opt-level=2"]);
            }
        }
        BuildProfile::Fast => {
            if target == BuildTarget::Web {
                command.args([
                    "-C",
                    "opt-level=z",
                    "-C",
                    "lto=fat",
                    "-C",
                    "codegen-units=1",
                    "-C",
                    "panic=abort",
                    "-C",
                    "strip=symbols",
                ]);
            } else {
                command.args([
                    "-C",
                    "opt-level=3",
                    "-C",
                    "lto=fat",
                    "-C",
                    "codegen-units=1",
                    "-C",
                    "panic=abort",
                    "-C",
                    "target-cpu=native",
                ]);
            }
        }
    }
}

fn default_output(stem: &str, target: BuildTarget) -> String {
    if target == BuildTarget::Web {
        format!("{stem}.wasm")
    } else {
        stem.to_string()
    }
}

pub fn compile_file(options: &CliOptions) -> Result<CompilationArtifacts, CompileError> {
    let src = fs::read_to_string(&options.input).map_err(|err| {
        CompileError::new(
            format!("falha ao ler arquivo {}: {err}", options.input),
            1,
            1,
        )
    })?;

    let ast = parse_source(&src)?;

    let stem = Path::new(&options.input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let rust_file = format!("{stem}.rs");
    let output_file = options
        .output
        .clone()
        .unwrap_or_else(|| default_output(stem, options.target));
    let rust_code = compile_program(&ast, options.target);

    fs::write(&rust_file, rust_code).map_err(|err| {
        CompileError::new(
            format!("falha ao escrever arquivo {rust_file}: {err}"),
            1,
            1,
        )
    })?;

    let mut command = Command::new("rustc");
    command.arg(&rust_file);
    if options.target == BuildTarget::Web {
        command.args([
            "--target",
            "wasm32-unknown-unknown",
            "--crate-type",
            "cdylib",
        ]);
    }
    command.arg("-o").arg(&output_file);
    add_profile_flags(&mut command, options.target, options.profile);

    let output = command
        .output()
        .map_err(|err| CompileError::new(format!("falha ao executar rustc: {err}"), 1, 1))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut message = format!("erro ao compilar {rust_file}: {stderr}");
        if options.target == BuildTarget::Web
            && stderr.contains("wasm32-unknown-unknown")
            && stderr.contains("can't find crate")
        {
            message.push_str(
                "\nDica: instale o alvo WebAssembly com `rustup target add wasm32-unknown-unknown`.",
            );
        }
        return Err(CompileError::new(message, 1, 1));
    }

    if !options.emit_rust {
        let _ = fs::remove_file(&rust_file);
    }

    Ok(CompilationArtifacts {
        rust_file,
        output_file,
    })
}
