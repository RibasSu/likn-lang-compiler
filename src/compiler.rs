use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::{BuildProfile, BuildTarget, CliOptions};
use crate::codegen::compile_program;
use crate::error::CompileError;
use crate::manager::ProjectManager;
use crate::manifest::load_manifest;
use crate::module_system::resolve_program_with_dependencies;
use crate::package::{
    FsRemoteClient, GithubClient, PackageCache, PackageError, PackageInstaller, RemoteClient,
    RemotePackageLocator,
};
use crate::typecheck::check_program;

#[derive(Debug, Clone)]
pub struct CompilationArtifacts {
    pub rust_file: String,
    pub output_file: String,
}

#[derive(Debug, Clone)]
enum RegistryClient {
    Fs(FsRemoteClient),
    Github(GithubClient),
}

impl RemoteClient for RegistryClient {
    fn fetch_text(&self, remote_path: &str) -> Result<String, PackageError> {
        match self {
            RegistryClient::Fs(client) => client.fetch_text(remote_path),
            RegistryClient::Github(client) => client.fetch_text(remote_path),
        }
    }

    fn fetch_bytes(&self, remote_path: &str) -> Result<Vec<u8>, PackageError> {
        match self {
            RegistryClient::Fs(client) => client.fetch_bytes(remote_path),
            RegistryClient::Github(client) => client.fetch_bytes(remote_path),
        }
    }
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
    let dependency_roots = resolve_dependency_roots(Path::new(&options.input))?;
    let resolved =
        resolve_program_with_dependencies(Path::new(&options.input), dependency_roots)?;
    let type_info = check_program(&resolved.ast).map_err(|err| {
        if resolved.has_imports {
            err
        } else {
            err.with_source_context(options.input.clone(), resolved.entry_source.clone())
        }
    })?;

    let stem = Path::new(&options.input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let rust_file = format!("{stem}.rs");
    let output_file = options
        .output
        .clone()
        .unwrap_or_else(|| default_output(stem, options.target));
    let rust_code = compile_program(&resolved.ast, options.target, &type_info);

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

fn resolve_dependency_roots(input_file: &Path) -> Result<std::collections::BTreeMap<String, PathBuf>, CompileError> {
    let Some(project_root) = find_project_root(input_file) else {
        return Ok(std::collections::BTreeMap::new());
    };

    let manifest_path = project_root.join("likn.toml");
    let manifest = load_manifest(&manifest_path).map_err(|err| {
        CompileError::new(
            format!("falha ao carregar {}: {err}", manifest_path.display()),
            1,
            1,
        )
    })?;

    if manifest.dependencies.is_empty() {
        return Ok(std::collections::BTreeMap::new());
    }

    let cache_root = std::env::var("LIKN_CACHE_DIR")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME")
                .map(|home| PathBuf::from(home).join(".likn"))
                .map_err(|_| ())
        })
        .map_err(|_| {
            CompileError::new(
                "dependências declaradas, mas não foi possível resolver diretório de cache",
                1,
                1,
            )
            .with_help("defina LIKN_CACHE_DIR ou HOME")
        })?;

    let client = if let Ok(registry_root) = std::env::var("LIKN_REGISTRY_ROOT") {
        RegistryClient::Fs(FsRemoteClient::new(registry_root))
    } else if let Ok(raw_base) = std::env::var("LIKN_REGISTRY_RAW_BASE") {
        RegistryClient::Github(GithubClient::new(raw_base))
    } else if let Some(embedded) = find_embedded_registry(&project_root) {
        RegistryClient::Fs(FsRemoteClient::new(embedded))
    } else {
        return Err(CompileError::new(
            "dependências declaradas, mas nenhuma fonte de registry foi configurada",
            1,
            1,
        )
        .with_help(
            "defina LIKN_REGISTRY_ROOT (filesystem) ou LIKN_REGISTRY_RAW_BASE (raw github base url)",
        ));
    };

    let cache = PackageCache::new(cache_root);
    let installer = PackageInstaller::new(client, cache, RemotePackageLocator::default());
    let manager = ProjectManager::new(installer);
    let graph = manager.install(&project_root).map_err(|err| {
        CompileError::new(
            format!("falha ao instalar dependências do projeto: {err}"),
            1,
            1,
        )
    })?;

    Ok(graph.dependency_roots())
}

fn find_project_root(input_file: &Path) -> Option<PathBuf> {
    let mut current = input_file.parent().map(Path::to_path_buf)?;
    loop {
        if current.join("likn.toml").is_file() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn find_embedded_registry(project_root: &Path) -> Option<PathBuf> {
    let mut current = Some(project_root.to_path_buf());
    while let Some(dir) = current {
        let candidate = dir.join("libs/lib-likn-lang");
        if candidate.join("registry/packages").is_dir() {
            return Some(candidate);
        }
        current = dir.parent().map(Path::to_path_buf);
    }
    None
}
