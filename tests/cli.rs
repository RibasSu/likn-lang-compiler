use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_path(ext: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock ok")
        .as_nanos();
    std::env::temp_dir().join(format!("likn_test_{nanos}.{ext}"))
}

fn compiler_bin() -> &'static str {
    env!("CARGO_BIN_EXE_likn-lang-compiler")
}

fn generated_rust_file(source: &PathBuf) -> PathBuf {
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .expect("source stem");
    std::env::current_dir()
        .expect("cwd")
        .join(format!("{stem}.rs"))
}

fn wasm_target_available() -> bool {
    Command::new("rustc")
        .args([
            "--print",
            "target-libdir",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[test]
fn cli_builds_native_binary() {
    let source = unique_path("ikn");
    let output = unique_path("bin");
    let rust_file = generated_rust_file(&source);

    fs::write(&source, "print(1 + 2)").expect("write source");

    let status = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .status()
        .expect("run compiler");

    assert!(status.success(), "compiler should succeed");
    assert!(output.exists(), "binary should be generated");

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let _ = fs::remove_file(&rust_file);
}

#[test]
fn cli_reports_parse_error() {
    let source = unique_path("ikn");
    let rust_file = generated_rust_file(&source);
    fs::write(&source, "print(\"abc").expect("write source");

    let output = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .output()
        .expect("run compiler");

    assert!(!output.status.success(), "compiler should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("erro em linha"),
        "stderr should have location"
    );

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&rust_file);
}

#[test]
fn cli_builds_web_wasm_when_target_exists() {
    if !wasm_target_available() {
        return;
    }

    let source = unique_path("ikn");
    let output = unique_path("wasm");
    let rust_file = generated_rust_file(&source);

    fs::write(&source, "print(42)").expect("write source");

    let status = Command::new(compiler_bin())
        .args(["--web", "--fast"])
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .status()
        .expect("run compiler");

    assert!(status.success(), "web build should succeed");
    assert!(output.exists(), "wasm should be generated");

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let _ = fs::remove_file(&rust_file);
}
