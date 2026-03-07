use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use likn_lang_compiler::lockfile::load_lockfile;
use likn_lang_compiler::manager::ProjectManager;
use likn_lang_compiler::manifest::{PackageType, load_manifest};
use likn_lang_compiler::package::{
    FsRemoteClient, PackageCache, PackageInstaller, RemotePackageLocator,
};
use likn_lang_compiler::resolver::DependencyGraph;

fn unique_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock ok")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("likn_mgr_{prefix}_{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_package_source(
    root: &Path,
    name: &str,
    version: &str,
    deps: &[(&str, &str)],
    files: &[(&str, &str)],
) -> PathBuf {
    let src_root = root.join(format!("src_{name}_{version}"));
    fs::create_dir_all(src_root.join("src")).expect("create pkg src");

    let mut manifest = String::new();
    manifest.push_str(&format!(
        "name = \"{name}\"\nversion = \"{version}\"\ntype = \"lib\"\n\n[dependencies]\n"
    ));
    for (dep_name, dep_version) in deps {
        manifest.push_str(&format!("{dep_name} = \"{dep_version}\"\n"));
    }
    fs::write(src_root.join("likn.toml"), manifest).expect("write manifest");

    for (rel, content) in files {
        let full = src_root.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(full, content).expect("write source file");
    }

    src_root
}

fn package_into_registry(
    registry_root: &Path,
    name: &str,
    version: &str,
    source_dir: &Path,
) {
    let target = registry_root
        .join("registry/packages")
        .join(name)
        .join(version);
    fs::create_dir_all(&target).expect("create registry target");

    fs::copy(source_dir.join("likn.toml"), target.join("manifest.toml")).expect("copy manifest");
    let archive = target.join("package.tar.gz");
    let tar = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(source_dir)
        .arg(".")
        .output()
        .expect("run tar");
    assert!(tar.status.success(), "tar should work");

    let sha = Command::new("sha256sum")
        .arg(&archive)
        .output()
        .expect("run sha256sum");
    assert!(sha.status.success(), "sha256sum should work");
    let hash = String::from_utf8_lossy(&sha.stdout)
        .split_whitespace()
        .next()
        .expect("hash output")
        .to_string();
    fs::write(target.join("sha256.txt"), hash).expect("write sha256");
}

fn build_manager(remote_root: &Path, cache_root: &Path) -> ProjectManager<FsRemoteClient> {
    let client = FsRemoteClient::new(remote_root);
    let cache = PackageCache::new(cache_root);
    let installer = PackageInstaller::new(client, cache, RemotePackageLocator::default());
    ProjectManager::new(installer)
}

fn write_project_manifest(project_root: &Path, deps: &[(&str, &str)]) {
    let mut manifest = String::new();
    manifest.push_str("name = \"demo\"\nversion = \"0.1.0\"\ntype = \"bin\"\n\n[dependencies]\n");
    for (name, version) in deps {
        manifest.push_str(&format!("{name} = \"{version}\"\n"));
    }
    fs::write(project_root.join("likn.toml"), manifest).expect("write project manifest");
}

fn assert_graph_contains(graph: &DependencyGraph, name: &str, version: &str) {
    assert!(
        graph
            .packages
            .keys()
            .any(|pkg| pkg.name == name && pkg.version == version),
        "graph should contain {name}@{version}"
    );
}

#[test]
fn manager_resolves_transitive_dependencies_and_writes_lockfile() {
    let remote_root = unique_dir("remote");
    let cache_root = unique_dir("cache");
    let project_root = unique_dir("project");

    let textkit_src = write_package_source(
        &remote_root,
        "textkit",
        "1.0.0",
        &[],
        &[("src/lib.ikn", "export fn upper(v) -> str { str.upper(v) }\n")],
    );
    let mathx_src = write_package_source(
        &remote_root,
        "mathx",
        "0.1.1",
        &[("textkit", "1.0.0")],
        &[
            ("src/lib.ikn", "export fn sum(a, b) -> int { a + b }\n"),
            ("src/basic.ikn", "export fn sum(a, b) -> int { a + b }\n"),
        ],
    );
    let unused_src = write_package_source(
        &remote_root,
        "unused",
        "9.9.9",
        &[],
        &[("src/lib.ikn", "export fn noop() -> int { 0 }\n")],
    );

    package_into_registry(&remote_root, "textkit", "1.0.0", &textkit_src);
    package_into_registry(&remote_root, "mathx", "0.1.1", &mathx_src);
    package_into_registry(&remote_root, "unused", "9.9.9", &unused_src);

    fs::create_dir_all(project_root.join("src")).expect("create project src");
    write_project_manifest(&project_root, &[("mathx", "0.1.1")]);

    let manager = build_manager(&remote_root, &cache_root);
    let graph = manager.install(&project_root).expect("install should succeed");

    assert_graph_contains(&graph, "mathx", "0.1.1");
    assert_graph_contains(&graph, "textkit", "1.0.0");
    assert!(
        !graph.packages.keys().any(|pkg| pkg.name == "unused"),
        "unused package should not be downloaded"
    );

    let mathx_path = cache_root.join("packages/mathx/0.1.1");
    let textkit_path = cache_root.join("packages/textkit/1.0.0");
    assert!(mathx_path.join("src/lib.ikn").is_file());
    assert!(textkit_path.join("src/lib.ikn").is_file());

    let lock = load_lockfile(&project_root.join("likn.lock")).expect("lockfile should exist");
    assert!(
        lock.packages
            .iter()
            .any(|pkg| pkg.name == "mathx" && pkg.version == "0.1.1")
    );
    assert!(
        lock.packages
            .iter()
            .any(|pkg| pkg.name == "textkit" && pkg.version == "1.0.0")
    );

    let _ = fs::remove_dir_all(remote_root);
    let _ = fs::remove_dir_all(cache_root);
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn manager_resolves_imports_and_enforces_exports() {
    let remote_root = unique_dir("remote_mod");
    let cache_root = unique_dir("cache_mod");
    let project_root = unique_dir("project_mod");

    let mathx_src = write_package_source(
        &remote_root,
        "mathx",
        "0.1.1",
        &[],
        &[
            ("src/lib.ikn", "export fn version() -> str { \"0.1.1\" }\n"),
            (
                "src/basic.ikn",
                "export fn sum(a, b) -> int { a + b }\nfn hidden(a, b) -> int { a - b }\n",
            ),
        ],
    );
    package_into_registry(&remote_root, "mathx", "0.1.1", &mathx_src);

    fs::create_dir_all(project_root.join("src/utils")).expect("create local modules");
    write_project_manifest(&project_root, &[("mathx", "0.1.1")]);
    fs::write(
        project_root.join("src/utils/text.ikn"),
        "export fn local() -> str { \"ok\" }\n",
    )
    .expect("write local module");

    let manager = build_manager(&remote_root, &cache_root);
    let graph = manager.install(&project_root).expect("install should succeed");

    fs::write(
        project_root.join("src/main_ok.ikn"),
        "import mathx.basic\nimport utils.text\nprint(basic.sum(1, 2))\nprint(text.local())\n",
    )
    .expect("write main_ok");
    let resolution_ok = manager
        .resolve_imports(&project_root, &graph, &project_root.join("src/main_ok.ikn"))
        .expect("imports should resolve");
    assert!(resolution_ok.modules.contains_key("mathx.basic"));
    assert!(resolution_ok.modules.contains_key("utils.text"));

    fs::write(
        project_root.join("src/main_bad.ikn"),
        "import mathx.basic\nprint(basic.hidden(1, 2))\n",
    )
    .expect("write main_bad");
    let err = manager
        .resolve_imports(&project_root, &graph, &project_root.join("src/main_bad.ikn"))
        .expect_err("hidden should be rejected");
    assert!(
        err.to_string().contains("não é exportado"),
        "visibility error should mention export"
    );

    let project_manifest = load_manifest(&project_root.join("likn.toml")).expect("manifest ok");
    assert_eq!(project_manifest.package_type, PackageType::Bin);

    let _ = fs::remove_dir_all(remote_root);
    let _ = fs::remove_dir_all(cache_root);
    let _ = fs::remove_dir_all(project_root);
}
