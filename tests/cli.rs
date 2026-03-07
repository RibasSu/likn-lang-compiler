use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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

fn escape_ikn_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
    assert!(stderr.contains("error:"), "stderr should have error header");
    assert!(stderr.contains("-->"), "stderr should show file position");
    assert!(
        stderr.contains("^"),
        "stderr should highlight the error span"
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

#[test]
fn cli_stdlib_fs_and_terminal_io_work() {
    let source = unique_path("ikn");
    let output = unique_path("bin");
    let rust_file = generated_rust_file(&source);
    let data_file = unique_path("txt");
    let data_path = escape_ikn_string(data_file.to_string_lossy().as_ref());

    let program = format!(
        r#"
fs.write("{data_path}", "linha-1")
fs.append("{data_path}", "\\nlinha-2")
let conteudo = fs.read("{data_path}")
print(conteudo)
print(fs.exists("{data_path}"))
let nome = term.input("Nome: ")
term.println(nome)
"#
    );
    fs::write(&source, program).expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .output()
        .expect("run compiler");
    assert!(compile.status.success(), "compiler should succeed");

    let mut child = Command::new(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run compiled binary");

    child
        .stdin
        .as_mut()
        .expect("stdin available")
        .write_all(b"Ana\n")
        .expect("write stdin");
    let run_output = child.wait_with_output().expect("wait process");

    assert!(
        run_output.status.success(),
        "program should run successfully"
    );
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(
        stdout.contains("linha-1"),
        "stdout should contain file content"
    );
    assert!(
        stdout.contains("linha-2"),
        "stdout should contain appended file content"
    );
    assert!(
        stdout.contains("true"),
        "stdout should contain fs.exists result"
    );
    assert!(
        stdout.contains("Ana"),
        "stdout should contain terminal input echo"
    );

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let _ = fs::remove_file(&rust_file);
    let _ = fs::remove_file(&data_file);
}
