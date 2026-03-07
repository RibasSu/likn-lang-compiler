use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::visibility::{ModuleInterface, collect_module_interface, validate_file_visibility};

#[derive(Debug, Clone)]
pub struct ResolvedModule {
    pub package_name: String,
    pub module_path: String,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct ModuleResolution {
    pub modules: BTreeMap<String, ResolvedModule>,
}

#[derive(Debug, Clone)]
pub struct ModuleResolver {
    pub project_root: PathBuf,
    pub dependencies: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ModuleError {
    message: String,
}

impl ModuleError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro de módulo: {}", self.message)
    }
}

impl std::error::Error for ModuleError {}

impl From<crate::visibility::VisibilityError> for ModuleError {
    fn from(value: crate::visibility::VisibilityError) -> Self {
        Self::new(value.to_string())
    }
}

impl ModuleResolver {
    pub fn new(project_root: impl Into<PathBuf>, dependencies: BTreeMap<String, PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            dependencies,
        }
    }

    pub fn resolve_import(
        &self,
        _current_file: &Path,
        import_path: &str,
    ) -> Result<ResolvedModule, ModuleError> {
        let segments = import_path
            .split('.')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(ModuleError::new("import vazio"));
        }

        let first = segments[0];
        if let Some(dep_root) = self.dependencies.get(first) {
            let path = if segments.len() == 1 {
                dep_root.join("src/lib.ikn")
            } else {
                dep_root.join("src").join(format!(
                    "{}.ikn",
                    segments[1..].join("/")
                ))
            };
            if !path.is_file() {
                return Err(ModuleError::new(format!(
                    "módulo externo '{}' não encontrado em {}",
                    import_path,
                    path.display()
                )));
            }
            return Ok(ResolvedModule {
                package_name: first.to_string(),
                module_path: import_path.to_string(),
                file_path: path,
            });
        }

        let local = self
            .project_root
            .join("src")
            .join(format!("{}.ikn", import_path.replace('.', "/")));
        if !local.is_file() {
            return Err(ModuleError::new(format!(
                "módulo local '{}' não encontrado em {}",
                import_path,
                local.display()
            )));
        }

        Ok(ResolvedModule {
            package_name: "local".to_string(),
            module_path: import_path.to_string(),
            file_path: local,
        })
    }

    pub fn collect_imports(&self, file_path: &Path) -> Result<Vec<String>, ModuleError> {
        let content = fs::read_to_string(file_path).map_err(|err| {
            ModuleError::new(format!("falha ao ler arquivo {}: {err}", file_path.display()))
        })?;
        let mut imports = Vec::new();

        for raw_line in content.lines() {
            let line = strip_comments(raw_line).trim();
            if !line.starts_with("import ") {
                continue;
            }
            let rest = line.trim_start_matches("import ").trim();
            let import = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(';')
                .trim();
            if import.is_empty() {
                return Err(ModuleError::new(format!(
                    "import inválido em {}",
                    file_path.display()
                )));
            }
            imports.push(import.to_string());
        }

        Ok(imports)
    }

    pub fn resolve_project_imports(&self, entry_file: &Path) -> Result<ModuleResolution, ModuleError> {
        let mut queue = VecDeque::new();
        let mut seen_files = BTreeSet::new();
        let mut modules = BTreeMap::new();

        queue.push_back(entry_file.to_path_buf());

        while let Some(current) = queue.pop_front() {
            if !seen_files.insert(current.clone()) {
                continue;
            }

            let imports = self.collect_imports(&current)?;
            let mut interfaces = BTreeMap::<String, ModuleInterface>::new();

            for import in imports.iter() {
                let resolved = self.resolve_import(&current, import)?;
                interfaces.insert(import.clone(), collect_module_interface(&resolved.file_path)?);
                modules
                    .entry(import.clone())
                    .or_insert_with(|| resolved.clone());
                queue.push_back(resolved.file_path);
            }

            validate_file_visibility(&current, &imports, &interfaces)?;
        }

        Ok(ModuleResolution { modules })
    }
}

fn strip_comments(line: &str) -> &str {
    if let Some((head, _)) = line.split_once('#') {
        head
    } else {
        line
    }
}
