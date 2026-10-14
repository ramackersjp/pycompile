// Integration tests: compile each example with the real compiler binary and
// check that its runtime output matches the recorded expected output.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples")
}

fn out_dir() -> PathBuf {
    let d = std::env::temp_dir().join("pycompile_it");
    let _ = fs::create_dir_all(&d);
    d
}

fn compile(name: &str) -> Option<PathBuf> {
    let bin = env!("CARGO_BIN_EXE_pycompile");
    let src = examples_dir().join(format!("{}.py", name));
    let out = out_dir().join(format!("ex_{}", name));
    let status = Command::new(bin)
        .arg(src)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("failed to launch compiler");
    if status.success() {
        Some(out)
    } else {
        None
    }
}

fn check_example(name: &str, args: &[&str]) {
    let Some(exe) = compile(name) else {
        panic!("failed to compile examples/{}.py", name);
    };
    let expected_path = examples_dir().join(format!("{}.out", name));
    let expected = fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", expected_path.display(), e));
    let output = Command::new(exe)
        .args(args)
        .output()
        .expect("failed to run compiled binary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected, "output mismatch for examples/{}.py", name);
}

#[test]
fn hello() {
    check_example("hello", &[]);
}

#[test]
fn fizzbuzz() {
    check_example("fizzbuzz", &[]);
}

#[test]
fn fibonacci() {
    check_example("fibonacci", &[]);
}

#[test]
fn collections() {
    check_example("collections", &[]);
}

#[test]
fn strings() {
    check_example("strings", &[]);
}

#[test]
fn sorting() {
    check_example("sorting", &[]);
}

#[test]
fn math_demo() {
    check_example("math_demo", &[]);
}

#[test]
fn wordcount() {
    check_example("wordcount", &[]);
}

#[test]
fn args() {
    check_example("args", &["A", "B"]);
}

#[test]
fn unsupported_constructs_report_errors() {
    let bin = env!("CARGO_BIN_EXE_pycompile");
    for (src, expect) in [
        ("class Foo:\n    pass\n", "class"),
        ("try:\n    pass\nexcept:\n    pass\n", "try"),
        ("f = lambda x: x\n", "lambda"),
        ("with open('f') as fh:\n    pass\n", "with"),
        ("import os\n", "os"),
    ] {
        let f = out_dir().join("err.py");
        fs::write(&f, src).unwrap();
        let output = Command::new(bin).arg(&f).output().unwrap();
        assert!(
            !output.status.success(),
            "expected compile error for: {}",
            src
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expect),
            "expected error mentioning '{}' but got: {}",
            expect,
            stderr
        );
    }
}
