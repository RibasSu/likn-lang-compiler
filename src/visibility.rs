use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExportItem {
    Function(String),
    Struct(String),
    Enum(String),
    Value(String),
}

impl ExportItem {
    pub fn name(&self) -> &str {
        match self {
            ExportItem::Function(name)
            | ExportItem::Struct(name)
            | ExportItem::Enum(name)
            | ExportItem::Value(name) => name,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleInterface {
    pub exports: Vec<ExportItem>,
}

impl ModuleInterface {
    pub fn export_names(&self) -> BTreeSet<String> {
        self.exports
            .iter()
            .map(|item| item.name().to_string())
            .collect()
    }

    pub fn is_exported(&self, symbol: &str) -> bool {
        self.exports.iter().any(|item| item.name() == symbol)
    }
}

#[derive(Debug, Clone)]
pub struct VisibilityError {
    message: String,
}

impl VisibilityError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for VisibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro de visibilidade: {}", self.message)
    }
}

impl std::error::Error for VisibilityError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedUse {
    pub prefix: String,
    pub symbol: String,
    pub line: usize,
}

pub fn collect_module_interface(path: &Path) -> Result<ModuleInterface, VisibilityError> {
    let content = fs::read_to_string(path)
        .map_err(|err| VisibilityError::new(format!("falha ao ler {}: {err}", path.display())))?;
    parse_module_interface(&content)
}

pub fn parse_module_interface(content: &str) -> Result<ModuleInterface, VisibilityError> {
    let mut exports = Vec::new();

    for line in content.lines() {
        let stripped = strip_comments(line).trim();
        if !stripped.starts_with("export ") {
            continue;
        }

        if let Some(name) = extract_export_name(stripped, "export fn ") {
            exports.push(ExportItem::Function(name));
            continue;
        }
        if let Some(name) = extract_export_name(stripped, "export struct ") {
            exports.push(ExportItem::Struct(name));
            continue;
        }
        if let Some(name) = extract_export_name(stripped, "export enum ") {
            exports.push(ExportItem::Enum(name));
            continue;
        }
        if let Some(name) = extract_export_name(stripped, "export let ") {
            exports.push(ExportItem::Value(name));
            continue;
        }
        if let Some(name) = extract_export_name(stripped, "export const ") {
            exports.push(ExportItem::Value(name));
            continue;
        }
    }

    Ok(ModuleInterface { exports })
}

pub fn collect_qualified_symbol_uses(path: &Path) -> Result<Vec<QualifiedUse>, VisibilityError> {
    let content = fs::read_to_string(path)
        .map_err(|err| VisibilityError::new(format!("falha ao ler {}: {err}", path.display())))?;
    Ok(parse_qualified_symbol_uses(&content))
}

pub fn parse_qualified_symbol_uses(content: &str) -> Vec<QualifiedUse> {
    let mut uses = Vec::new();

    for (line_index, line) in content.lines().enumerate() {
        let stripped = strip_comments(line).trim();
        if stripped.starts_with("import ") {
            continue;
        }

        for token in tokenize_candidates(stripped) {
            if !token.contains('.') {
                continue;
            }
            let parts = token.split('.').collect::<Vec<_>>();
            if parts.len() < 2 {
                continue;
            }
            if parts
                .iter()
                .any(|segment| segment.is_empty() || !is_ident(segment))
            {
                continue;
            }

            uses.push(QualifiedUse {
                prefix: parts[..parts.len() - 1].join("."),
                symbol: parts[parts.len() - 1].to_string(),
                line: line_index + 1,
            });
        }
    }

    uses
}

pub fn validate_file_visibility(
    file_path: &Path,
    imports: &[String],
    interfaces: &std::collections::BTreeMap<String, ModuleInterface>,
) -> Result<(), VisibilityError> {
    let uses = collect_qualified_symbol_uses(file_path)?;

    for usage in uses {
        for import_path in imports {
            let alias = import_path.rsplit('.').next().unwrap_or(import_path);
            if usage.prefix == *import_path || usage.prefix == alias {
                let Some(interface) = interfaces.get(import_path) else {
                    continue;
                };
                if !interface.is_exported(&usage.symbol) {
                    return Err(VisibilityError::new(format!(
                        "símbolo '{}' não é exportado por '{}' ({}:{})",
                        usage.symbol,
                        import_path,
                        file_path.display(),
                        usage.line
                    )));
                }
            }
        }
    }

    Ok(())
}

fn strip_comments(line: &str) -> &str {
    if let Some((head, _)) = line.split_once('#') {
        head
    } else {
        line
    }
}

fn extract_export_name(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?.trim();
    let mut out = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            break;
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn tokenize_candidates(line: &str) -> Vec<String> {
    let mut normalized = String::new();
    for ch in line.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            normalized.push(ch);
        } else {
            normalized.push(' ');
        }
    }
    normalized
        .split_whitespace()
        .map(|value| value.to_string())
        .collect()
}

fn is_ident(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
