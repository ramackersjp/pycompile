pub mod ast;
pub mod codegen;
pub mod driver;
pub mod error;
pub mod lexer;
pub mod man;
pub mod parser;

use error::{CompileError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
pycompile - compile Python source into a standalone native binary

USAGE:
    pycompile [OPTIONS] <INPUT> [-- PROGRAM_ARGS...]
    python_compiler [OPTIONS] <INPUT> [-- PROGRAM_ARGS...]
    pycompile man [--install] [--prefix DIR]

ARGS:
    <INPUT>    A .py file, or a directory containing main.py or __main__.py

OPTIONS:
    -o, --output <PATH>   Output binary path (default: input name without extension)
        --emit-c <PATH>   Also write the generated C source to PATH
        --keep-temps      Keep the temporary C build directory
    -r, --run             Compile and immediately run the program
        --man             Print the full manual to the terminal (same as the man subcommand)
    -h, --help            Print this help
    -V, --version         Print version information

SUBCOMMANDS:
    man [--install] [--prefix DIR]   Show or install the manual page

Run 'pycompile man' for the full manual.
";

pub struct Cli {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub emit_c: Option<PathBuf>,
    pub keep_temps: bool,
    pub run: bool,
    pub program_args: Vec<String>,
}

fn arg_err(msg: &str) -> CompileError {
    CompileError::usage(format!("{} (see --help)", msg))
}

fn print_help() {
    print!("{}", HELP);
}

fn print_version() {
    println!("pycompile {} (python_compiler {})", VERSION, VERSION);
}

pub fn parse_args(args: &[String]) -> Result<Option<Cli>> {
    if let Some(first) = args.first() {
        match first.as_str() {
            "man" | "--man" => {
                man::run_man_subcommand(&args[1..])?;
                return Ok(None);
            }
            _ => {}
        }
    }

    let mut input: Option<PathBuf> = None;
    let mut output = None;
    let mut emit_c = None;
    let mut keep_temps = false;
    let mut run = false;
    let mut program_args = Vec::new();
    let mut i = 0;
    let mut passthrough = false;
    while i < args.len() {
        let a = &args[i];
        if passthrough {
            program_args.push(a.clone());
            i += 1;
            continue;
        }
        match a.as_str() {
            "--" => passthrough = true,
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(None);
            }
            "-V" | "--version" => {
                print_version();
                return Ok(None);
            }
            "-o" | "--output" => {
                i += 1;
                output = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| arg_err("--output needs a value"))?,
                ));
            }
            "--emit-c" => {
                i += 1;
                emit_c = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| arg_err("--emit-c needs a value"))?,
                ));
            }
            "--keep-temps" => keep_temps = true,
            "-r" | "--run" => run = true,
            "--man" => {
                man::print_terminal();
                return Ok(None);
            }
            _ if a.starts_with('-') && a.len() > 1 => {
                return Err(arg_err(&format!("unknown option '{}'", a)));
            }
            _ => {
                if input.is_some() {
                    return Err(arg_err("only one input path is supported"));
                }
                input = Some(PathBuf::from(a));
            }
        }
        i += 1;
    }
    let input = input.ok_or_else(|| arg_err("no input file given"))?;
    Ok(Some(Cli {
        input,
        output,
        emit_c,
        keep_temps,
        run,
        program_args,
    }))
}

fn resolve_input(input: &Path) -> Result<PathBuf> {
    if input.is_dir() {
        for cand in ["main.py", "__main__.py"] {
            let p = input.join(cand);
            if p.is_file() {
                return Ok(p);
            }
        }
        return Err(CompileError::new(
            &input.display().to_string(),
            0,
            0,
            "directory does not contain main.py or __main__.py",
        ));
    }
    if input.is_file() {
        return Ok(input.to_path_buf());
    }
    Err(CompileError::new(
        &input.display().to_string(),
        0,
        0,
        "input path not found",
    ))
}

fn default_output(source: &Path) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "a.out".to_string());
    PathBuf::from(stem)
}

fn real_main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_args(&args)? {
        Some(c) => c,
        None => return Ok(()),
    };
    let source = resolve_input(&cli.input)?;
    let src = std::fs::read_to_string(&source).map_err(|e| {
        CompileError::new(
            &source.display().to_string(),
            0,
            0,
            format!("cannot read file: {}", e),
        )
    })?;
    let file_label = source.display().to_string();
    let toks = lexer::Lexer::new(&src, &file_label).tokenize()?;
    let module = parser::Parser::new(toks, &file_label).parse_module()?;

    let output = cli
        .output
        .clone()
        .unwrap_or_else(|| default_output(&source));

    let mut build_output = output.clone();
    let temp_run_dir;
    if cli.run {
        let d = std::env::temp_dir().join(format!("pycompile-run-{}", std::process::id()));
        std::fs::create_dir_all(&d).ok();
        build_output = d.join(
            output
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("program")),
        );
        temp_run_dir = Some(d);
    } else {
        temp_run_dir = None;
    }

    let opts = driver::BuildOptions {
        output: build_output.clone(),
        emit_c: cli.emit_c.clone(),
        keep_temps: cli.keep_temps,
    };
    driver::build(&module, &file_label, &opts)?;

    if cli.run {
        let exe = if cfg!(windows) && build_output.extension().is_none() {
            build_output.with_extension("exe")
        } else {
            build_output.clone()
        };
        let status = Command::new(&exe)
            .args(&cli.program_args)
            .status()
            .map_err(|e| {
                CompileError::new(&file_label, 0, 0, format!("cannot run binary: {}", e))
            })?;
        if let Some(d) = &temp_run_dir {
            std::fs::remove_dir_all(d).ok();
        }
        std::process::exit(status.code().unwrap_or(1));
    }

    let final_path = if cfg!(windows) && output.extension().is_none() {
        output.with_extension("exe")
    } else {
        output
    };
    println!("Compiled {} -> {}", file_label, final_path.display());
    Ok(())
}

pub fn run() {
    if let Err(e) = real_main() {
        eprintln!("{}", e);
        std::process::exit(if e.usage { 2 } else { 1 });
    }
}
