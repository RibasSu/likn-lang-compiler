use std::fmt;
use std::fs;
use std::path::Path;

use crate::package::DEFAULT_SOURCE;
use crate::resolver::DependencyGraph;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lockfile {
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Clone)]
pub struct LockfileError {
    message: String,
}

impl LockfileError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for LockfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro de lockfile: {}", self.message)
    }
}

impl std::error::Error for LockfileError {}

pub fn load_lockfile(path: &Path) -> Result<Lockfile, LockfileError> {
    let content = fs::read_to_string(path)
        .map_err(|err| LockfileError::new(format!("falha ao ler {}: {err}", path.display())))?;

    let mut lock = Lockfile::default();
    let mut current: Option<LockedPackage> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line == "[[package]]" {
            if let Some(pkg) = current.take() {
                lock.packages.push(pkg);
            }
            current = Some(LockedPackage {
                name: String::new(),
                version: String::new(),
                source: DEFAULT_SOURCE.to_string(),
            });
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = parse_string_value(value.trim()).ok_or_else(|| {
            LockfileError::new(format!("valor inválido para chave '{key}' no lockfile"))
        })?;

        if let Some(pkg) = current.as_mut() {
            match key {
                "name" => pkg.name = value,
                "version" => pkg.version = value,
                "source" => pkg.source = value,
                _ => {}
            }
        }
    }

    if let Some(pkg) = current.take() {
        lock.packages.push(pkg);
    }

    for pkg in &lock.packages {
        if pkg.name.is_empty() || pkg.version.is_empty() {
            return Err(LockfileError::new("entrada incompleta em [[package]]"));
        }
    }

    Ok(lock)
}

pub fn write_lockfile(path: &Path, graph: &DependencyGraph) -> Result<(), LockfileError> {
    let mut out = String::new();

    for id in graph.packages.keys() {
        out.push_str("[[package]]\n");
        out.push_str(&format!("name = \"{}\"\n", id.name));
        out.push_str(&format!("version = \"{}\"\n", id.version));
        out.push_str(&format!("source = \"{}\"\n", DEFAULT_SOURCE));
        out.push('\n');
    }

    fs::write(path, out)
        .map_err(|err| LockfileError::new(format!("falha ao gravar {}: {err}", path.display())))
}

fn parse_string_value(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        Some(value[1..value.len() - 1].to_string())
    } else {
        None
    }
}
