use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageType {
    Bin,
    Lib,
    Hub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub package_type: PackageType,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ManifestError {
    message: String,
}

impl ManifestError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro de manifesto: {}", self.message)
    }
}

impl std::error::Error for ManifestError {}

pub fn load_manifest(path: &Path) -> Result<Manifest, ManifestError> {
    let content = fs::read_to_string(path).map_err(|err| {
        ManifestError::new(format!("falha ao ler manifesto em {}: {err}", path.display()))
    })?;
    parse_manifest(&content)
}

pub fn parse_manifest(content: &str) -> Result<Manifest, ManifestError> {
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut package_type: Option<PackageType> = None;
    let mut dependencies = BTreeMap::new();
    let mut section: Option<String> = None;

    for raw_line in content.lines() {
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = Some(
                line.trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim()
                    .to_string(),
            );
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = parse_key(raw_key.trim());
        match section.as_deref() {
            Some("dependencies") => {
                let value = parse_string_value(raw_value.trim()).ok_or_else(|| {
                    ManifestError::new(format!("valor inválido para chave '{key}'"))
                })?;
                dependencies.insert(key, value);
            }
            _ => match key.as_str() {
                "name" => {
                    let value = parse_string_value(raw_value.trim()).ok_or_else(|| {
                        ManifestError::new(format!("valor inválido para chave '{key}'"))
                    })?;
                    name = Some(value);
                }
                "version" => {
                    let value = parse_string_value(raw_value.trim()).ok_or_else(|| {
                        ManifestError::new(format!("valor inválido para chave '{key}'"))
                    })?;
                    version = Some(value);
                }
                "type" => {
                    let value = parse_string_value(raw_value.trim()).ok_or_else(|| {
                        ManifestError::new(format!("valor inválido para chave '{key}'"))
                    })?;
                    package_type = Some(parse_package_type(&value)?);
                }
                _ => {}
            },
        }
    }

    let name = name.ok_or_else(|| ManifestError::new("campo 'name' ausente"))?;
    let version = version.ok_or_else(|| ManifestError::new("campo 'version' ausente"))?;
    let package_type = package_type.unwrap_or(PackageType::Lib);

    validate_package_name(&name)?;
    validate_version(&version)?;

    Ok(Manifest {
        name,
        version,
        package_type,
        dependencies,
    })
}

fn strip_comments(line: &str) -> &str {
    if let Some((head, _)) = line.split_once('#') {
        head
    } else {
        line
    }
}

fn parse_key(raw_key: &str) -> String {
    let trimmed = raw_key.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_string_value(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        Some(value[1..value.len() - 1].to_string())
    } else {
        None
    }
}

fn parse_package_type(value: &str) -> Result<PackageType, ManifestError> {
    match value {
        "bin" => Ok(PackageType::Bin),
        "lib" => Ok(PackageType::Lib),
        "hub" => Ok(PackageType::Hub),
        other => Err(ManifestError::new(format!(
            "tipo de pacote inválido: '{other}' (use bin/lib/hub)"
        ))),
    }
}

pub fn validate_package_name(name: &str) -> Result<(), ManifestError> {
    if name.is_empty() {
        return Err(ManifestError::new("name não pode ser vazio"));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
    {
        return Err(ManifestError::new(format!(
            "name contém caracteres inválidos: '{name}'"
        )));
    }
    Ok(())
}

pub fn validate_version(version: &str) -> Result<(), ManifestError> {
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts
            .iter()
            .any(|segment| segment.is_empty() || !segment.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(ManifestError::new(format!(
            "version inválida '{version}' (esperado formato x.y.z)"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PackageType, parse_manifest};

    #[test]
    fn parse_ignores_unknown_non_string_fields() {
        let manifest = parse_manifest(
            r#"
name = "hub"
version = "0.1.0"
type = "hub"

[workspace]
members = [
  "packages/a",
  "packages/b"
]
"#,
        )
        .expect("manifest should parse");

        assert_eq!(manifest.name, "hub");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.package_type, PackageType::Hub);
    }

    #[test]
    fn parse_rejects_non_string_dependency_value() {
        let err = parse_manifest(
            r#"
name = "app"
version = "0.1.0"
type = "bin"

[dependencies]
mathx = 1
"#,
        )
        .expect_err("manifest should fail");
        assert!(err.to_string().contains("valor inválido para chave 'mathx'"));
    }
}
