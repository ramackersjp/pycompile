//! Terminal manual and man-page installation for the `man` subcommand.

use crate::error::{CompileError, Result};
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;

/// The full roff man page, embedded at compile time.
pub const MAN_PAGE: &str = include_str!("../docs/pycompile.1");

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

fn use_color() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false) {
        return false;
    }
    std::io::stdout().is_terminal()
}

struct W {
    buf: String,
    color: bool,
}

impl W {
    fn new(color: bool) -> Self {
        W {
            buf: String::new(),
            color,
        }
    }

    fn c(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("{}{}{}", code, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn section(&mut self, title: &str) {
        if !self.buf.is_empty() {
            self.buf.push('\n');
        }
        self.buf.push_str(&self.c(BOLD, title));
        self.buf.push('\n');
    }

    fn line(&mut self, text: &str) {
        self.buf.push_str(text);
        self.buf.push('\n');
    }

    fn blank(&mut self) {
        self.buf.push('\n');
    }

    fn para(&mut self, text: &str) {
        let mut line = String::new();
        for word in text.split_whitespace() {
            if !line.is_empty() && line.len() + word.len() + 1 > 76 {
                self.line(&line);
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            self.line(&line);
        }
    }

    fn option(&mut self, flag: &str, desc: &str) {
        let flag_col = self.c(YELLOW, &format!("    {:<28}", flag));
        self.buf.push_str(&format!("{}  {}\n", flag_col, desc));
    }

    fn command(&mut self, text: &str) {
        self.line(&self.c(GREEN, text));
    }

    fn dim(&mut self, text: &str) {
        self.line(&self.c(DIM, text));
    }
}

fn render_terminal() -> String {
    let color = use_color();
    let mut w = W::new(color);

    w.section("NAME");
    w.line("       pycompile - compile Python source into a native binary");

    w.section("SYNOPSIS");
    w.line("       pycompile [OPTIONS] <INPUT> [-- PROGRAM_ARGS...]");
    w.line("       pycompile man [--install] [--prefix DIR]");

    w.section("DESCRIPTION");
    w.para(
        "pycompile is ahead-of-time compiler that translates a supported subset of Python \
         directly into C, invokes the platform C compiler, and emits a single standalone \
         native executable. There is no interpreter and no runtime dependency at run time: \
         the resulting binary runs on its own.",
    );
    w.blank();
    w.para(
        "It is designed for small command-line programs and scripts where you want Python's \
         syntax and speed of iteration but a portable native binary as the deliverable.",
    );
    w.section("QUICK START");
    w.para("Compile a single file to a native binary named after the source:");
    w.command("       pycompile hello.py");
    w.blank();
    w.para("Choose an output path explicitly:");
    w.command("       pycompile hello.py -o build/hello");
    w.blank();
    w.para("Compile and immediately run the program:");
    w.command("       pycompile -r hello.py");
    w.blank();
    w.para("Compile and also keep the generated C for inspection:");
    w.command("       pycompile hello.py --emit-c hello.c --keep-temps");
    w.blank();
    w.para("Compile a whole project directory that contains main.py:");
    w.command("       pycompile ./myproject -o myproject");
    w.blank();
    w.para("Pass arguments to the compiled program after --:");
    w.command("       pycompile cli.py -o cli -- --verbose file.txt");

    w.section("INPUT");
    w.option(
        "<INPUT>",
        "A .py file, or a directory containing main.py or __main__.py",
    );
    w.blank();
    w.para(
        "When <INPUT> is a directory, pycompile looks for main.py first, then __main__.py, \
         and compiles that file as the program entry point.",
    );

    w.section("OPTIONS");
    w.option(
        "-o, --output <PATH>",
        "Output binary path (default: input name without extension)",
    );
    w.option(
        "    --emit-c <PATH>",
        "Also write the generated C source to PATH",
    );
    w.option("    --keep-temps", "Keep the temporary C build directory");
    w.option("-r, --run", "Compile and immediately run the program");
    w.option("    --man", "Print this manual to the terminal");
    w.option("-h, --help", "Print a usage summary and exit");
    w.option("-V, --version", "Print version information and exit");

    w.section("SUBCOMMANDS");
    w.option("man", "Print this manual to the terminal");
    w.option(
        "man --install",
        "Install the man page (requires write access to the prefix)",
    );
    w.option(
        "man --prefix DIR",
        "Install into DIR/share/man/man1 instead of /usr/local/share/man/man1",
    );

    w.section("LANGUAGE SUPPORT");
    w.para("The following Python features compile to native code:");
    w.line("       - Integer, float, string and boolean literals, and None");
    w.line("       - f-strings with format specs and the !r conversion");
    w.line("       - Lists, tuples, sets and dicts, including literal and comprehension forms");
    w.line("       - if / elif / else, while, for-in (with else), break, continue, pass");
    w.line("       - def with default parameters, recursion, *args and **kwargs parameters");
    w.line("       - Calls with positional, keyword, * and ** arguments");
    w.line("       - Chained comparisons, in / not in, is / is not, and, or, not");
    w.line("       - Slicing and item assignment, augmented assignment");
    w.line("       - % string formatting (positional and mapping forms, flags, width, precision)");
    w.line("       - lambda expressions, nested functions and closures (including late binding)");
    w.line("       - try / except / except ... as / else / finally, raise, assert");
    w.line("       - catchable built-in exception types with their inheritance hierarchy");
    w.line("       - Stable sort / sorted with reverse=True and key=");
    w.line("       - class definitions with methods, __init__, class and instance attributes");
    w.line("       - single inheritance, isinstance, and the __str__, __repr__, __eq__,");
    w.line("         __len__, __getitem__, __setitem__ and __contains__ protocols");
    w.line("       - The math and sys modules");
    w.blank();
    w.para("The following are not yet supported and produce a clear compile error:");
    w.line("       - super(), multiple inheritance and decorators");
    w.line("       - with, generators, async and await");
    w.line("       - return, break or continue inside a try that has a finally clause");
    w.line("       - Modules other than math and sys");

    w.section("RUNTIME MODEL");
    w.para(
        "The generated C links against a small runtime library that is embedded by the \
         compiler. Values are heap-allocated tagged objects. A conservative mark-and-sweep \
         garbage collector reclaims unreachable objects by scanning the C stack, the \
         program's globals and a few runtime roots; unreachable reference cycles are \
         collected as well. Because the scan is conservative, an object can occasionally \
         stay alive a little longer than strictly necessary.",
    );
    w.blank();
    w.para(
        "Runtime errors (for example, a missing argument or an out-of-range operation) \
         print a Python-style traceback to stderr and exit with status 1.",
    );

    w.section("EXAMPLES");
    w.command("       pycompile hello.py");
    w.dim("              Compiles hello.py to ./hello using the system C compiler.");
    w.blank();
    w.command("       pycompile -o demo app/main.py");
    w.dim("              Compiles the project entry point to ./demo.");
    w.blank();
    w.command("       pycompile -r script.py -- 1 2 3");
    w.dim("              Compiles script.py and runs it with arguments 1, 2, 3.");
    w.blank();
    w.command("       pycompile script.py --emit-c script.c");
    w.dim("              Writes both the native binary and the generated C source.");
    w.blank();
    w.command("       man pycompile");
    w.dim("              Reads this manual page with the system man program.");

    w.section("ENVIRONMENT");
    w.option(
        "CC",
        "Preferred C compiler to invoke. Falls back to cc, gcc, clang, tcc, then cl",
    );
    w.option(
        "NO_COLOR",
        "Set to any value to disable ANSI color in the terminal manual",
    );
    w.option("TERM=dumb", "Disables color in the terminal manual");

    w.section("EXIT STATUS");
    w.option(
        "0",
        "Success: the binary was produced (or the program exited 0)",
    );
    w.option(
        "1",
        "Compile error, or the compiled program exited non-zero",
    );
    w.option("2", "Command line usage error");

    w.section("FILES");
    w.option(
        "/usr/local/share/man/man1/pycompile.1",
        "Installed manual page",
    );
    w.option("<prefix>/share/man/man1/", "Location when --prefix is used");

    w.section("SEE ALSO");
    w.line("       man(1), cc(1), gcc(1), clang(1), python3(1)");

    w.section("BUGS AND LIMITATIONS");
    w.line("       - The garbage collector is conservative and scans the whole C stack on each");
    w.line("         collection; it may keep an object alive a little longer than necessary.");
    w.line("       - super(), multiple inheritance and decorators are not supported.");
    w.line(
        "       - return, break or continue inside a try that has a finally clause is rejected.",
    );
    w.line(
        "       - An uncaught exception prints a single-line traceback without file/line frames.",
    );

    w.section("AUTHOR");
    w.para("Written by J.P. Ramackers and contributors. MIT licensed.");

    w.buf
}

fn install_default_prefix() -> PathBuf {
    if let Some(p) = std::env::var_os("PREFIX") {
        return PathBuf::from(p);
    }
    PathBuf::from("/usr/local")
}

fn do_install(prefix: &std::path::Path) -> Result<()> {
    let man1 = prefix.join("share").join("man").join("man1");
    fs::create_dir_all(&man1).map_err(|e| {
        CompileError::new(
            "<man>",
            0,
            0,
            format!(
                "cannot create {}: {} (try: sudo pycompile man --install)",
                man1.display(),
                e
            ),
        )
    })?;
    let path = man1.join("pycompile.1");
    fs::write(&path, MAN_PAGE).map_err(|e| {
        CompileError::new(
            "<man>",
            0,
            0,
            format!("cannot write {}: {}", path.display(), e),
        )
    })?;
    println!("Installed {}", path.display());
    println!();
    println!("Read it with:  man pycompile");
    Ok(())
}

pub fn print_terminal() {
    print!("{}", render_terminal());
}

pub fn run_man_subcommand(args: &[String]) -> Result<()> {
    let mut install = false;
    let mut prefix: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--install" | "-i" => install = true,
            "--prefix" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| CompileError::usage("--prefix needs a value"))?;
                prefix = Some(PathBuf::from(v));
            }
            other if other.starts_with("--prefix=") => {
                prefix = Some(PathBuf::from(&other["--prefix=".len()..]));
            }
            "-h" | "--help" => {
                println!("usage: pycompile man [--install] [--prefix DIR]");
                println!();
                println!("  (no options)      print the manual to the terminal");
                println!("  --install         install the man page (may need sudo)");
                println!("  --prefix DIR      install into DIR/share/man/man1");
                return Ok(());
            }
            other => {
                return Err(CompileError::usage(format!(
                    "unknown option '{}' for man",
                    other
                )));
            }
        }
        i += 1;
    }

    if install {
        let p = prefix.unwrap_or_else(install_default_prefix);
        return do_install(&p);
    }

    print!("{}", render_terminal());
    Ok(())
}
