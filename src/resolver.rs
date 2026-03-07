use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use crate::manifest::{Manifest, PackageType, load_manifest};
use crate::package::{PackageId, PackageInstaller, RemoteClient};

#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub id: PackageId,
    pub manifest: Manifest,
    pub root_dir: PathBuf,
    pub dependencies: Vec<PackageId>,
}

#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    pub packages: BTreeMap<PackageId, ResolvedPackage>,
}

impl DependencyGraph {
    pub fn dependency_roots(&self) -> BTreeMap<String, PathBuf> {
        self.packages
            .iter()
            .map(|(id, pkg)| (id.name.clone(), pkg.root_dir.clone()))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ResolveError {
    message: String,
}

impl ResolveError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro de resolução: {}", self.message)
    }
}

impl std::error::Error for ResolveError {}

impl From<crate::manifest::ManifestError> for ResolveError {
    fn from(value: crate::manifest::ManifestError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<crate::package::PackageError> for ResolveError {
    fn from(value: crate::package::PackageError) -> Self {
        Self::new(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DependencyResolver<C: RemoteClient> {
    pub installer: PackageInstaller<C>,
}

impl<C: RemoteClient> DependencyResolver<C> {
    pub fn new(installer: PackageInstaller<C>) -> Self {
        Self { installer }
    }

    pub fn resolve_project(&self, project: &Manifest) -> Result<DependencyGraph, ResolveError> {
        let mut state = ResolveState::default();

        for (name, version) in &project.dependencies {
            let dep = PackageId::new(name.clone(), version.clone())
                .map_err(|err| ResolveError::new(err.to_string()))?;
            self.visit(dep, &mut state)?;
        }

        Ok(DependencyGraph {
            packages: state.graph,
        })
    }

    fn visit(&self, pkg: PackageId, state: &mut ResolveState) -> Result<(), ResolveError> {
        if let Some(known_version) = state.name_to_version.get(&pkg.name) {
            if known_version != &pkg.version {
                return Err(ResolveError::new(format!(
                    "conflito de versão para '{}': '{}' vs '{}'",
                    pkg.name, known_version, pkg.version
                )));
            }
        } else {
            state
                .name_to_version
                .insert(pkg.name.clone(), pkg.version.clone());
        }

        if state.visited.contains(&pkg) {
            return Ok(());
        }
        if state.visiting.contains(&pkg) {
            return Err(ResolveError::new(format!(
                "ciclo de dependência detectado em {}@{}",
                pkg.name, pkg.version
            )));
        }

        state.visiting.insert(pkg.clone());

        let manifest_path = self.installer.ensure_manifest(&pkg)?;
        let manifest = load_manifest(&manifest_path)?;

        if manifest.name != pkg.name {
            return Err(ResolveError::new(format!(
                "manifesto remoto inválido: esperado name='{}', obtido '{}'",
                pkg.name, manifest.name
            )));
        }
        if manifest.version != pkg.version {
            return Err(ResolveError::new(format!(
                "manifesto remoto inválido: esperado version='{}', obtido '{}'",
                pkg.version, manifest.version
            )));
        }
        if manifest.package_type != PackageType::Lib {
            return Err(ResolveError::new(format!(
                "dependência '{}' deve ser do tipo lib",
                manifest.name
            )));
        }

        let mut deps = Vec::new();
        for (dep_name, dep_version) in &manifest.dependencies {
            let dep = PackageId::new(dep_name.clone(), dep_version.clone())
                .map_err(|err| ResolveError::new(err.to_string()))?;
            deps.push(dep.clone());
            self.visit(dep, state)?;
        }

        let root_dir = self.installer.ensure_package(&pkg)?;
        state.graph.insert(
            pkg.clone(),
            ResolvedPackage {
                id: pkg.clone(),
                manifest,
                root_dir,
                dependencies: deps,
            },
        );

        state.visiting.remove(&pkg);
        state.visited.insert(pkg);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct ResolveState {
    graph: BTreeMap<PackageId, ResolvedPackage>,
    visiting: BTreeSet<PackageId>,
    visited: BTreeSet<PackageId>,
    name_to_version: BTreeMap<String, String>,
}
