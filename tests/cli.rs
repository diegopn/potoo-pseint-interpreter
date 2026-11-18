use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const CLI: &str = env!("CARGO_BIN_EXE_potoo-pseint");

struct ProgramDirectory(std::path::PathBuf);

impl ProgramDirectory {
    fn new() -> Self {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("potoo-pseint-{}-{unique}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn write(&self, source: &str) -> std::path::PathBuf {
        let path = self.0.join("programa.psc");
        fs::write(&path, source).unwrap();
        path
    }

    fn write_bytes(&self, source: &[u8]) -> std::path::PathBuf {
        let path = self.0.join("programa.psc");
        fs::write(&path, source).unwrap();
        path
    }
}

impl Drop for ProgramDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(arguments: &[&str], path: &std::path::Path) -> Output {
    Command::new(CLI)
        .args(arguments)
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn help_prints_the_supported_command_line_options() {
    let output = Command::new(CLI).arg("--help").output().unwrap();

    assert!(output.status.success());
    let usage = String::from_utf8(output.stdout).unwrap();
    assert!(usage.contains("--nouser"));
    assert!(usage.contains("--norun"));
    assert!(usage.contains("--input=VALUE"));
    assert!(usage.contains("<file.psc>"));
}

#[test]
fn no_user_and_preset_input_produce_only_program_output_and_echo_input() {
    let directory = ProgramDirectory::new();
    let path = directory
        .write("Algoritmo Eco\nDefinir dato Como Entero\nLeer dato\nEscribir dato\nFinAlgoritmo");

    let output = run(&["--nouser", "--input=17", "--noinput"], &path);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "> 17\n17\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn check_only_validates_without_executing_or_waiting_for_input() {
    let directory = ProgramDirectory::new();
    let path = directory.write(
        "Algoritmo Espera\nDefinir dato Como Entero\nLeer dato\nEscribir dato\nFinAlgoritmo",
    );

    let output = run(&["--nouser", "--norun"], &path);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_programs_return_failure_and_include_the_source_location() {
    let directory = ProgramDirectory::new();
    let path = directory.write("Algoritmo Demo\nSi Verdadero Entonces\nFinAlgoritmo");

    let output = run(&["--nouser"], &path);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("programa.psc:"));
    assert!(stderr.contains(":3:"), "{stderr}");
}

#[test]
fn unknown_options_return_usage_status_two() {
    let directory = ProgramDirectory::new();
    let path = directory.write("Algoritmo Demo\nFinAlgoritmo");

    let output = run(&["--unknown"], &path);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: potoo-pseint"));
}

#[test]
fn cli_decodes_latin1_runtime_input_as_unicode() {
    let directory = ProgramDirectory::new();
    let path = directory.write(
        "Algoritmo Eco\nDefinir texto Como Caracter\nLeer texto\nEscribir texto\nFinAlgoritmo",
    );
    let mut child = Command::new(CLI)
        .args(["--nouser"])
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"ni\xf1o\n").unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "> niño\n");
}

#[test]
fn cli_decodes_utf8_and_latin1_source_files_as_unicode() {
    let directory = ProgramDirectory::new();
    let utf8_path = directory.write("Algoritmo Eco\nEscribir \"niño\"\nFinAlgoritmo");
    let utf8 = run(&["--nouser"], &utf8_path);
    assert!(utf8.status.success());
    assert_eq!(String::from_utf8(utf8.stdout).unwrap(), "niño\n");

    let latin1_path = directory.write_bytes(b"Algoritmo Eco\nEscribir \"ni\xf1o\"\nFinAlgoritmo");
    let latin1 = run(&["--nouser"], &latin1_path);
    assert!(latin1.status.success());
    assert_eq!(String::from_utf8(latin1.stdout).unwrap(), "niño\n");
}
