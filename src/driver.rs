use crate::ast::Module;
use crate::codegen::Codegen;
use crate::error::{CompileError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const RUNTIME_C: &str = include_str!("../runtime/pyrt.c");
pub const RUNTIME_H: &str = include_str!("../runtime/pyrt.h");

pub struct BuildOptions {
    pub output: PathBuf,
    pub emit_c: Option<PathBuf>,
    pub keep_temps: bool,
}

#[derive(Clone)]
struct Compiler {
    name: String,
    msvc: bool,
}

fn probe(name: &str) -> Option<Compiler> {
    let msvc = name == "cl";
    let try_args: &[&str] = if msvc { &["/?"] } else { &["--version"] };
    match Command::new(name).arg(try_args[0]).output() {
        Ok(_) => Some(Compiler {
            name: name.to_string(),
            msvc,
        }),
        Err(_) => None,
    }
}

fn find_compiler() -> Result<Compiler> {
    if let Ok(cc) = std::env::var("CC") {
        if let Some(c) = probe(&cc) {
            return Ok(c);
        }
    }
    let candidates: &[&str] = if cfg!(windows) {
        &["cl", "gcc", "clang", "tcc"]
    } else {
        &["cc", "gcc", "clang"]
    };
    for c in candidates {
        if let Some(found) = probe(c) {
            return Ok(found);
        }
    }
    Err(CompileError::new(
        "<compiler>",
        0,
        0,
        "no C compiler found; install gcc, clang or MSVC Build Tools, or set the CC environment variable",
    ))
}

fn temp_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("pycompile-{}-{}", std::process::id(), nanos))
}

fn with_exe(path: &Path) -> PathBuf {
    if cfg!(windows) && path.extension().is_none() {
        path.with_extension("exe")
    } else {
        path.to_path_buf()
    }
}

/// Compile a parsed Python module into a native binary at `opts.output`.
pub fn build(module: &Module, source_file: &str, opts: &BuildOptions) -> Result<()> {
    let c_source = Codegen::new(source_file).compile(module)?;
    let compiler = find_compiler()?;
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).map_err(|e| {
        CompileError::new(
            source_file,
            0,
            0,
            format!("cannot create temp directory: {}", e),
        )
    })?;

    let main_c = dir.join("main.c");
    let rt_c = dir.join("pyrt.c");
    let rt_h = dir.join("pyrt.h");
    std::fs::write(&main_c, &c_source).map_err(io_err(source_file))?;
    std::fs::write(&rt_c, RUNTIME_C).map_err(io_err(source_file))?;
    std::fs::write(&rt_h, RUNTIME_H).map_err(io_err(source_file))?;

    if let Some(path) = &opts.emit_c {
        std::fs::write(path, &c_source).map_err(io_err(source_file))?;
    }

    let output = with_exe(&opts.output);
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    let mut cmd = Command::new(&compiler.name);
    if compiler.msvc {
        cmd.arg("/nologo")
            .arg("/O2")
            .arg(format!("/Fe:{}", output.display()))
            .arg("main.c")
            .arg("pyrt.c");
    } else {
        cmd.arg("-O2")
            .arg("-std=c11")
            .arg("-w")
            .arg("-o")
            .arg(&output)
            .arg("main.c")
            .arg("pyrt.c");
        if !cfg!(windows) {
            cmd.arg("-lm");
        }
    }
    cmd.current_dir(&dir);
    let result = cmd.output().map_err(|e| {
        CompileError::new(
            source_file,
            0,
            0,
            format!("failed to run C compiler '{}': {}", compiler.name, e),
        )
    })?;

    if !opts.keep_temps {
        std::fs::remove_dir_all(&dir).ok();
    }

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(CompileError::new(
            source_file,
            0,
            0,
            format!("C compiler '{}' failed:\n{}", compiler.name, stderr.trim()),
        ));
    }
    Ok(())
}

fn io_err(file: &str) -> impl Fn(std::io::Error) -> CompileError + '_ {
    move |e| CompileError::new(file, 0, 0, format!("io error: {}", e))
}
