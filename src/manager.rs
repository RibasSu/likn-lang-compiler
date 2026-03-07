use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::lockfile::write_lockfile;
use crate::manifest::load_manifest;
use crate::module_resolver::{ModuleResolution, ModuleResolver};
use crate::package::{PackageInstaller, RemoteClient};
use crate::resolver::{DependencyGraph, DependencyResolver};

#[derive(Debug, Clone)]
pub struct ProjectManager<C: RemoteClient> {
    resolver: DependencyResolver<C>,
}

#[derive(Debug, Clone)]
pub struct ManagerError {
    message: String,
}

impl ManagerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro de gerenciamento: {}", self.message)
    }
}

impl std::error::Error for ManagerError {}

impl From<crate::manifest::ManifestError> for ManagerError {
    fn from(value: crate::manifest::ManifestError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<crate::package::PackageError> for ManagerError {
    fn from(value: crate::package::PackageError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<crate::resolver::ResolveError> for ManagerError {
    fn from(value: crate::resolver::ResolveError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<crate::module_resolver::ModuleError> for ManagerError {
    fn from(value: crate::module_resolver::ModuleError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<crate::lockfile::LockfileError> for ManagerError {
    fn from(value: crate::lockfile::LockfileError) -> Self {
        Self::new(value.to_string())
    }
}

impl<C: RemoteClient> ProjectManager<C> {
    pub fn new(installer: PackageInstaller<C>) -> Self {
        Self {
            resolver: DependencyResolver::new(installer),
        }
    }

    pub fn install(&self, project_root: &Path) -> Result<DependencyGraph, ManagerError> {
        let manifest_path = project_root.join("likn.toml");
        let manifest = load_manifest(&manifest_path)?;
        let graph = self.resolver.resolve_project(&manifest)?;
        write_lockfile(&project_root.join("likn.lock"), &graph)?;
        Ok(graph)
    }

    pub fn resolve_imports(
        &self,
        project_root: &Path,
        graph: &DependencyGraph,
        entry_file: &Path,
    ) -> Result<ModuleResolution, ManagerError> {
        let dependencies = graph
            .packages
            .iter()
            .map(|(id, pkg)| (id.name.clone(), pkg.root_dir.clone()))
            .collect::<BTreeMap<_, _>>();
        let resolver = ModuleResolver::new(project_root, dependencies);
        Ok(resolver.resolve_project_imports(entry_file)?)
    }

    pub fn dependency_paths(graph: &DependencyGraph) -> BTreeMap<String, PathBuf> {
        graph.dependency_roots()
    }
}
