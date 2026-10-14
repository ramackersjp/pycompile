# pycompile

Compile Python into standalone native binaries.

`pycompile` (also installed as `python_compiler`) is an ahead-of-time compiler that
translates a supported subset of Python into C, invokes the system C compiler, and
emits a single standalone executable. No interpreter, no runtime to install — the
result runs on its own.

```
$ pycompile hello.py -o hello
$ ./hello
Hello, World!
```

## Documentation

- Full manual in the terminal: `pycompile man`
- Installed man page: `man pycompile` (after `sudo pycompile man --install`)
- Website source (docs, install and download pages): [`docs/`](docs/)

## Install

### With Cargo

```
cargo install python-compiler
```

This installs two identical commands, `pycompile` and `python_compiler`.

### From source

```
git clone https://github.com/ramackersjp/Python_Compiler
cd Python_Compiler
cargo build --release
# optionally:
cargo install --path . --release
```

Requirements: a recent stable Rust toolchain and a C compiler (`cc`, `gcc`, `clang`,
`tcc`, or MSVC `cl`). The C compiler can be selected with the `CC` environment
variable.

### Man page

```
sudo pycompile man --install          # installs into /usr/local/share/man/man1
pycompile man --install --prefix ~/.local
```

## Usage

```
pycompile [OPTIONS] <INPUT> [-- PROGRAM_ARGS...]
python_compiler [OPTIONS] <INPUT> [-- PROGRAM_ARGS...]
pycompile man [--install] [--prefix DIR]
```

`<INPUT>` is a `.py` file, or a directory containing `main.py` or `__main__.py`.

| Option | Description |
| --- | --- |
| `-o, --output <PATH>` | Output binary path (default: input name without extension) |
| `--emit-c <PATH>` | Also write the generated C source to `PATH` |
| `--keep-temps` | Keep the temporary C build directory |
| `-r, --run` | Compile and immediately run the program |
| `--man` | Print the full manual to the terminal |
| `-h, --help` | Print a usage summary |
| `-V, --version` | Print version information |

Examples:

```
pycompile hello.py                     # -> ./hello
pycompile -o build/app app/main.py     # explicit output
pycompile -r script.py -- 1 2 3        # compile and run with arguments
pycompile hello.py --emit-c hello.c    # keep the generated C
```

## Language support

Compiles to native code:

- Integer, float, string and boolean literals, and `None`
- f-strings with format specs and the `!r` conversion
- Lists, tuples, sets and dicts, including literal and comprehension forms
- `if` / `elif` / `else`, `while`, `for`-in (with `else`), `break`, `continue`, `pass`
- `def` with default parameters, recursion, and `*args` / `**kwargs` parameters
- Calls with positional, keyword, `*` and `**` arguments
- Chained comparisons, `in` / `not in`, `is` / `is not`, `and`, `or`, `not`
- Slicing and item assignment, augmented assignment
- The `math` and `sys` modules

Not yet supported (reported as clear compile errors):

- `class`, closures and nested scopes
- `try` / `except` / `finally`, `with`, `raise`
- Decorators, `lambda`, `async` / `await`
- Modules other than `math` and `sys`

## Runtime model

Values are heap-allocated tagged objects. Memory is **not** garbage collected;
allocations live until process exit. This keeps the compiler simple and the output
small, and is fine for typical short-lived CLI programs.

Runtime errors print a Python-style traceback to stderr and exit with status 1.

## Examples

See [`examples/`](examples/) for small programs covering the supported features.
Each has a recorded expected output (`examples/<name>.out`) used by the test suite.

```
pycompile examples/fizzbuzz.py -o /tmp/fizzbuzz && /tmp/fizzbuzz
```

## Packaging and platforms

pycompile is distributed today as:

- `.tar.gz` release archives for Linux (x86_64, aarch64) and macOS (x86_64, Apple
  Silicon), and a `.zip` for Windows (x86_64), from the
  [releases page](https://github.com/ramackersjp/Python_Compiler/releases);
- a crate published on crates.io (`cargo install python-compiler`).

Packaging for OS package managers is not published yet:

- Linux: Flatpak, Snap, AUR, `.deb` and `.rpm` are planned.
- macOS: a Homebrew formula/tap is planned.
- Windows: `winget` and `scoop` packages are planned.

On Windows the compiler needs a C compiler to produce binaries (MSVC Build Tools,
MinGW-w64, or LLVM/clang); this requirement applies only when invoking pycompile, not
to the compiler binary itself.

## Website

The static documentation site lives in [`docs/`](docs/) (plain HTML/CSS, no build
step). It deploys to [Vercel](https://vercel.com) with no configuration:

1. Import this repository in Vercel ("Add New… → Project").
2. Keep the defaults — `vercel.json` sets `outputDirectory` to `docs` and skips the
   build step.
3. Deploy.

Any other static host works too: serve the contents of `docs/` as-is.

## Development

```
cargo build
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Tests compile every example with the real compiler and compare against the recorded
output. They need a C compiler on `PATH` (see the `CC` environment variable).

See [`.github/AGENTS.md`](.github/AGENTS.md) for a detailed architecture overview, and
[`CHANGE_LOG.md`](CHANGE_LOG.md) for the development log.

## License

MIT — see [LICENSE](LICENSE).
