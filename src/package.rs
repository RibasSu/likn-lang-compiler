use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::manifest::{validate_package_name, validate_version};

pub const DEFAULT_SOURCE: &str = "github:RibasSu/lib-likn-lang";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageId {
    pub name: String,
    pub version: String,
}

impl PackageId {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Result<Self, PackageError> {
        let name = name.into();
        let version = version.into();
        validate_package_name(&name).map_err(PackageError::from)?;
        validate_version(&version).map_err(PackageError::from)?;
        Ok(Self { name, version })
    }
}

#[derive(Debug, Clone)]
pub struct PackageError {
    message: String,
}

impl PackageError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro de pacote: {}", self.message)
    }
}

impl std::error::Error for PackageError {}

impl From<crate::manifest::ManifestError> for PackageError {
    fn from(value: crate::manifest::ManifestError) -> Self {
        Self::new(value.to_string())
    }
}

pub trait RemoteClient {
    fn fetch_text(&self, remote_path: &str) -> Result<String, PackageError>;
    fn fetch_bytes(&self, remote_path: &str) -> Result<Vec<u8>, PackageError>;
}

#[derive(Debug, Clone)]
pub struct FsRemoteClient {
    root: PathBuf,
}

impl FsRemoteClient {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl RemoteClient for FsRemoteClient {
    fn fetch_text(&self, remote_path: &str) -> Result<String, PackageError> {
        let path = self.root.join(remote_path);
        fs::read_to_string(&path).map_err(|err| {
            PackageError::new(format!("falha ao ler remoto {}: {err}", path.display()))
        })
    }

    fn fetch_bytes(&self, remote_path: &str) -> Result<Vec<u8>, PackageError> {
        let path = self.root.join(remote_path);
        fs::read(&path)
            .map_err(|err| PackageError::new(format!("falha ao ler remoto {}: {err}", path.display())))
    }
}

#[derive(Debug, Clone)]
pub struct GithubClient {
    raw_base: String,
}

impl GithubClient {
    pub fn new(raw_base: impl Into<String>) -> Self {
        Self {
            raw_base: raw_base.into(),
        }
    }

    fn fetch_url_text(&self, url: &str) -> Result<String, PackageError> {
        let output = Command::new("curl")
            .args(["-fsSL", url])
            .output()
            .map_err(|err| PackageError::new(format!("falha ao executar curl: {err}")))?;

        if !output.status.success() {
            return Err(PackageError::new(format!(
                "curl falhou ao baixar {url}: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl RemoteClient for GithubClient {
    fn fetch_text(&self, remote_path: &str) -> Result<String, PackageError> {
        let url = format!(
            "{}/{}",
            self.raw_base.trim_end_matches('/'),
            remote_path.trim_start_matches('/')
        );
        self.fetch_url_text(&url)
    }

    fn fetch_bytes(&self, remote_path: &str) -> Result<Vec<u8>, PackageError> {
        let url = format!(
            "{}/{}",
            self.raw_base.trim_end_matches('/'),
            remote_path.trim_start_matches('/')
        );
        let output = Command::new("curl")
            .args(["-fsSL", &url])
            .output()
            .map_err(|err| PackageError::new(format!("falha ao executar curl: {err}")))?;
        if !output.status.success() {
            return Err(PackageError::new(format!(
                "curl falhou ao baixar {url}: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(output.stdout)
    }
}

#[derive(Debug, Clone)]
pub struct RemotePackageLocator {
    registry_prefix: String,
}

impl Default for RemotePackageLocator {
    fn default() -> Self {
        Self {
            registry_prefix: "registry/packages".to_string(),
        }
    }
}

impl RemotePackageLocator {
    pub fn new(registry_prefix: impl Into<String>) -> Self {
        Self {
            registry_prefix: registry_prefix.into(),
        }
    }

    pub fn manifest_path(&self, pkg: &PackageId) -> String {
        format!(
            "{}/{}/{}/manifest.toml",
            self.registry_prefix.trim_end_matches('/'),
            pkg.name,
            pkg.version
        )
    }

    pub fn archive_path(&self, pkg: &PackageId) -> String {
        format!(
            "{}/{}/{}/package.tar.gz",
            self.registry_prefix.trim_end_matches('/'),
            pkg.name,
            pkg.version
        )
    }

    pub fn checksum_path(&self, pkg: &PackageId) -> String {
        format!(
            "{}/{}/{}/sha256.txt",
            self.registry_prefix.trim_end_matches('/'),
            pkg.name,
            pkg.version
        )
    }
}

#[derive(Debug, Clone)]
pub struct PackageCache {
    pub root: PathBuf,
}

impl PackageCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn manifests_dir(&self) -> PathBuf {
        self.root.join("manifests")
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.root.join("packages")
    }

    pub fn archives_dir(&self) -> PathBuf {
        self.root.join("archives")
    }

    pub fn manifest_path(&self, pkg: &PackageId) -> PathBuf {
        self.manifests_dir()
            .join(&pkg.name)
            .join(format!("{}.toml", pkg.version))
    }

    pub fn package_root(&self, pkg: &PackageId) -> PathBuf {
        self.packages_dir().join(&pkg.name).join(&pkg.version)
    }

    pub fn archive_path(&self, pkg: &PackageId) -> PathBuf {
        self.archives_dir()
            .join(&pkg.name)
            .join(format!("{}.tar.gz", pkg.version))
    }

    pub fn has_manifest(&self, pkg: &PackageId) -> bool {
        self.manifest_path(pkg).is_file()
    }

    pub fn has_package(&self, pkg: &PackageId) -> bool {
        let root = self.package_root(pkg);
        root.join("likn.toml").is_file() && root.join("src/lib.ikn").is_file()
    }

    pub fn store_manifest(&self, pkg: &PackageId, content: &str) -> Result<(), PackageError> {
        let path = self.manifest_path(pkg);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PackageError::new(format!(
                    "falha ao criar diretório de manifesto {}: {err}",
                    parent.display()
                ))
            })?;
        }
        fs::write(&path, content).map_err(|err| {
            PackageError::new(format!(
                "falha ao salvar manifesto em {}: {err}",
                path.display()
            ))
        })
    }

    pub fn store_archive(&self, pkg: &PackageId, bytes: &[u8]) -> Result<(), PackageError> {
        let path = self.archive_path(pkg);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PackageError::new(format!(
                    "falha ao criar diretório de arquivo {}: {err}",
                    parent.display()
                ))
            })?;
        }
        fs::write(&path, bytes)
            .map_err(|err| PackageError::new(format!("falha ao salvar arquivo {}: {err}", path.display())))
    }

    pub fn extract_archive(&self, pkg: &PackageId) -> Result<(), PackageError> {
        let archive = self.archive_path(pkg);
        let target = self.package_root(pkg);

        if target.exists() {
            fs::remove_dir_all(&target).map_err(|err| {
                PackageError::new(format!(
                    "falha ao limpar pacote antigo {}: {err}",
                    target.display()
                ))
            })?;
        }
        fs::create_dir_all(&target).map_err(|err| {
            PackageError::new(format!(
                "falha ao criar diretório de extração {}: {err}",
                target.display()
            ))
        })?;

        let output = Command::new("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(&target)
            .output()
            .map_err(|err| PackageError::new(format!("falha ao executar tar: {err}")))?;
        if !output.status.success() {
            return Err(PackageError::new(format!(
                "falha ao extrair arquivo {}: {}",
                archive.display(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        if !target.join("likn.toml").is_file() {
            let fallback_manifest = self.manifest_path(pkg);
            if fallback_manifest.is_file() {
                fs::copy(&fallback_manifest, target.join("likn.toml")).map_err(|err| {
                    PackageError::new(format!(
                        "falha ao copiar manifesto para pacote {}: {err}",
                        target.display()
                    ))
                })?;
            }
        }

        if !target.join("src/lib.ikn").is_file() {
            return Err(PackageError::new(format!(
                "pacote extraído inválido em {}: src/lib.ikn ausente",
                target.display()
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PackageInstaller<C: RemoteClient> {
    pub client: C,
    pub cache: PackageCache,
    pub locator: RemotePackageLocator,
}

impl<C: RemoteClient> PackageInstaller<C> {
    pub fn new(client: C, cache: PackageCache, locator: RemotePackageLocator) -> Self {
        Self {
            client,
            cache,
            locator,
        }
    }

    pub fn ensure_manifest(&self, pkg: &PackageId) -> Result<PathBuf, PackageError> {
        let target = self.cache.manifest_path(pkg);
        if target.is_file() {
            return Ok(target);
        }

        let remote = self.locator.manifest_path(pkg);
        let content = self.client.fetch_text(&remote)?;
        self.cache.store_manifest(pkg, &content)?;
        Ok(self.cache.manifest_path(pkg))
    }

    pub fn ensure_package(&self, pkg: &PackageId) -> Result<PathBuf, PackageError> {
        if self.cache.has_package(pkg) {
            return Ok(self.cache.package_root(pkg));
        }

        let _ = self.ensure_manifest(pkg)?;

        if !self.cache.archive_path(pkg).is_file() {
            let archive_remote = self.locator.archive_path(pkg);
            let archive_bytes = self.client.fetch_bytes(&archive_remote)?;
            self.cache.store_archive(pkg, &archive_bytes)?;
        }

        let checksum_remote = self.locator.checksum_path(pkg);
        let expected_sha = self.client.fetch_text(&checksum_remote)?.trim().to_string();
        self.verify_archive_checksum(pkg, &expected_sha)?;
        self.cache.extract_archive(pkg)?;
        Ok(self.cache.package_root(pkg))
    }

    fn verify_archive_checksum(&self, pkg: &PackageId, expected_sha: &str) -> Result<(), PackageError> {
        let archive = self.cache.archive_path(pkg);
        let actual = compute_sha256(&archive)?;
        if actual != expected_sha {
            return Err(PackageError::new(format!(
                "checksum inválido para {}@{}: esperado {}, obtido {}",
                pkg.name, pkg.version, expected_sha, actual
            )));
        }
        Ok(())
    }
}

fn compute_sha256(path: &Path) -> Result<String, PackageError> {
    let run = Command::new("sha256sum").arg(path).output();
    let output = match run {
        Ok(out) if out.status.success() => out,
        _ => Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()
            .map_err(|err| PackageError::new(format!("falha ao calcular sha256: {err}")))?,
    };

    if !output.status.success() {
        return Err(PackageError::new(format!(
            "comando de hash falhou: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let hash = stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| PackageError::new("saída vazia ao calcular sha256"))?;
    Ok(hash.to_string())
}
