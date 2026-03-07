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

fn unique_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock ok")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("likn_{prefix}_{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
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

#[test]
fn cli_stdlib_text_functions_work() {
    let source = unique_path("ikn");
    let output = unique_path("bin");
    let rust_file = generated_rust_file(&source);
    let program = r#"
let up = str.upper("Likn")
let low = str.lower("LIKN")
let has = str.contains(up, "IK")
let size = str.len(low)
print(up)
print(low)
print(has)
print(size)
"#;
    fs::write(&source, program).expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .output()
        .expect("run compiler");
    assert!(
        compile.status.success(),
        "compiler should accept text stdlib calls"
    );

    let run = Command::new(&output).output().expect("run compiled binary");
    assert!(run.status.success(), "binary should execute");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("LIKN"), "upper should work");
    assert!(stdout.contains("likn"), "lower should work");
    assert!(stdout.contains("true"), "contains should work");
    assert!(stdout.contains("4"), "len should work");

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let _ = fs::remove_file(&rust_file);
}

#[test]
fn cli_rejects_mixed_numeric_and_bool() {
    let source = unique_path("ikn");
    fs::write(&source, "print(1 + true)").expect("write source");

    let output = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .output()
        .expect("run compiler");

    assert!(!output.status.success(), "compiler should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("operação aritmética"),
        "stderr should mention arithmetic type mismatch"
    );

    let _ = fs::remove_file(&source);
}

#[test]
fn cli_rejects_inconsistent_function_return_paths() {
    let source = unique_path("ikn");
    let program = r#"
fn parcial(v: i64) -> i64 {
  if v > 0 {
    return v
  }
}
print(parcial(10))
"#;
    fs::write(&source, program).expect("write source");

    let output = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .output()
        .expect("run compiler");

    assert!(!output.status.success(), "compiler should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nem todos os caminhos retornam"),
        "stderr should mention missing return path"
    );

    let _ = fs::remove_file(&source);
}

#[test]
fn cli_accepts_annotations_const_mut_shadowing_and_never() {
    let source = unique_path("ikn");
    let output = unique_path("bin");
    let rust_file = generated_rust_file(&source);
    let program = r#"
const base: i64 = 5

fn soma(a: i64, b: i64) -> i64 {
  return a + b
}

fn explode(msg: String) -> ! {
  panic(msg)
}

let mut x: i64 = base
let x = soma(x, 10)
print(x)
if x > 10 {
  print("ok")
} else {
  explode("erro")
}
"#;
    fs::write(&source, program).expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .output()
        .expect("run compiler");

    assert!(compile.status.success(), "compiler should succeed");
    assert!(output.exists(), "binary should be generated");

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let _ = fs::remove_file(&rust_file);
}

#[test]
fn cli_escapes_rust_reserved_identifiers() {
    let source = unique_path("ikn");
    let output = unique_path("bin");
    let rust_file = generated_rust_file(&source);
    let program = r#"
fn match(v: i64) -> i64 {
  return v + 1
}

let final = 10
let type = match(final)
print(type)
"#;
    fs::write(&source, program).expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .output()
        .expect("run compiler");
    assert!(
        compile.status.success(),
        "compiler should escape reserved identifiers"
    );

    let run = Command::new(&output).output().expect("run compiled binary");
    assert!(run.status.success(), "binary should execute");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("11"),
        "output should contain computed result"
    );

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let _ = fs::remove_file(&rust_file);
}

#[test]
fn cli_resolves_local_module_imports_with_export() {
    let dir = unique_dir("mods_ok");
    let source = dir.join("entry_mod_ok.ikn");
    let output = dir.join("app.bin");
    let rust_file = generated_rust_file(&source);
    let math = dir.join("math.ikn");

    fs::write(
        &math,
        r#"
export fn sum(a, b) -> int {
  a + b
}
"#,
    )
    .expect("write module");
    fs::write(
        &source,
        r#"
import math
print(math.sum(2, 3))
"#,
    )
    .expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .output()
        .expect("run compiler");
    assert!(
        compile.status.success(),
        "module import should compile: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&output).output().expect("run compiled binary");
    assert!(run.status.success(), "binary should execute");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("5"), "stdout should contain module result");

    let _ = fs::remove_file(&rust_file);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cli_rejects_access_to_non_exported_module_function() {
    let dir = unique_dir("mods_private");
    let source = dir.join("entry_mod_private.ikn");
    let rust_file = generated_rust_file(&source);
    let math = dir.join("math.ikn");

    fs::write(
        &math,
        r#"
fn hidden(a, b) -> int {
  a + b
}
"#,
    )
    .expect("write module");
    fs::write(
        &source,
        r#"
import math
print(math.hidden(2, 3))
"#,
    )
    .expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .output()
        .expect("run compiler");
    assert!(!compile.status.success(), "compiler should fail");
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(
        stderr.contains("não é exportada"),
        "stderr should mention missing export"
    );

    let _ = fs::remove_file(&rust_file);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cli_reports_missing_imported_module() {
    let dir = unique_dir("mods_missing");
    let source = dir.join("entry_mod_missing.ikn");
    let rust_file = generated_rust_file(&source);

    fs::write(
        &source,
        r#"
import missing
print(1)
"#,
    )
    .expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .output()
        .expect("run compiler");
    assert!(!compile.status.success(), "compiler should fail");
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(
        stderr.contains("módulo 'missing' não encontrado"),
        "stderr should mention missing module"
    );

    let _ = fs::remove_file(&rust_file);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cli_rejects_ambiguous_import_namespace_alias() {
    let dir = unique_dir("mods_ambiguous");
    let source = dir.join("entry_mod_ambiguous.ikn");
    let rust_file = generated_rust_file(&source);
    let foo_dir = dir.join("foo");
    let bar_dir = dir.join("bar");
    fs::create_dir_all(&foo_dir).expect("create foo dir");
    fs::create_dir_all(&bar_dir).expect("create bar dir");

    fs::write(
        foo_dir.join("math.ikn"),
        r#"
export fn sum(a, b) -> int {
  a + b
}
"#,
    )
    .expect("write foo module");
    fs::write(
        bar_dir.join("math.ikn"),
        r#"
export fn sum(a, b) -> int {
  a + b
}
"#,
    )
    .expect("write bar module");
    fs::write(
        &source,
        r#"
import foo.math
import bar.math
print(1)
"#,
    )
    .expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .output()
        .expect("run compiler");
    assert!(!compile.status.success(), "compiler should fail");
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(
        stderr.contains("namespace 'math' é ambíguo"),
        "stderr should mention ambiguous namespace"
    );

    let _ = fs::remove_file(&rust_file);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cli_accepts_python_style_keywords_and_comments() {
    let source = unique_path("ikn");
    let output = unique_path("bin");
    let rust_file = generated_rust_file(&source);
    let program = r#"
# comentário Python
def calc(v: i64) -> i64: {
  if v > 10 and not false: {
    return v
  } elif v > 5: {
    return v + 1
  } else: {
    return v + 2
  }
}

print(calc(7))
"#;
    fs::write(&source, program).expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .output()
        .expect("run compiler");
    assert!(
        compile.status.success(),
        "compiler should accept python-like aliases"
    );

    let run = Command::new(&output).output().expect("run compiled binary");
    assert!(run.status.success(), "binary should execute");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("8"), "result should be computed");

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let _ = fs::remove_file(&rust_file);
}

#[test]
fn cli_infers_param_types_from_declared_return_type() {
    let source = unique_path("ikn");
    let output = unique_path("bin");
    let rust_file = generated_rust_file(&source);
    let program = r#"
fn sum(a, b) -> int {
  a + b
}
print(sum(3, 4))
"#;
    fs::write(&source, program).expect("write source");

    let compile = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .arg("--output")
        .arg(output.as_os_str())
        .output()
        .expect("run compiler");
    assert!(
        compile.status.success(),
        "compiler should infer param types from return type"
    );

    let run = Command::new(&output).output().expect("run compiled binary");
    assert!(run.status.success(), "binary should execute");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("7"), "output should contain 7");

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let _ = fs::remove_file(&rust_file);
}

#[test]
fn cli_rejects_untyped_params_without_return_type() {
    let source = unique_path("ikn");
    let program = r#"
fn sum(a, b) {
  return a + b
}
"#;
    fs::write(&source, program).expect("write source");

    let output = Command::new(compiler_bin())
        .arg(source.as_os_str())
        .output()
        .expect("run compiler");
    assert!(!output.status.success(), "compiler should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("parâmetro 'a' sem tipo explícito exige retorno com '-> Tipo'"),
        "stderr should explain missing return type inheritance rule"
    );

    let _ = fs::remove_file(&source);
}
