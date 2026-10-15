# pycompile

Compile Python into standalone native binaries.

`pycompile` is an ahead-of-time compiler that translates a supported subset of Python
into C, invokes the system C compiler, and emits a single standalone executable. No
interpreter, no runtime to install — the result runs on its own.

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

This installs the `pycompile` command.

### From source

```
git clone https://github.com/ramackersjp/pycompile
cd pycompile
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
- `%` string formatting (positional and mapping forms, flags, width, precision)
- `lambda` expressions, nested functions and closures (including late binding and
  `sorted(..., key=lambda ...)`)
- `try` / `except` / `except ... as name` / `else` / `finally`, `raise` (bare and
  `raise E(...)`), `assert`, and catchable built-in exception types (with their
  inheritance hierarchy, e.g. `except ArithmeticError` catches `ZeroDivisionError`)
- Stable `sort` / `sorted` with `reverse=True` and `key=`
- `class` definitions with methods, `__init__`, class and instance attributes,
  single inheritance, `isinstance`, and the `__str__`, `__repr__`, `__eq__`,
  `__len__`, `__getitem__`, `__setitem__` and `__contains__` protocols
- The `math` and `sys` modules

Not yet supported (reported as clear compile errors):

- `super()`, multiple inheritance, decorators (`@staticmethod`, `@classmethod`),
  operator overloading beyond `__eq__` and `__len__`
- `with`, generators, `async` / `await`
- `return`, `break` or `continue` inside a `try` that has a `finally` clause
- Modules other than `math` and `sys`

## Runtime model

Values are heap-allocated tagged objects. A conservative mark-and-sweep garbage
collector reclaims unreachable objects: it scans the C stack, the program's global
variables and a few runtime roots, marks everything reachable and frees the rest.
Reference cycles that stay unreachable are collected too. The collector is
conservative, so an object can occasionally be kept alive a little longer than
strictly necessary; this is safe and keeps the generated code unchanged.

Runtime errors raise the corresponding built-in exception (for example
`ZeroDivisionError`, `IndexError`, `KeyError`, `TypeError`, `ValueError`,
`AssertionError` and `RuntimeError`), which can be caught with `try` / `except`.
An uncaught exception prints a Python-style traceback to stderr and exits with
status 1.

## Examples

See [`examples/`](examples/) for small programs covering the supported features.
Each has a recorded expected output (`examples/<name>.out`) used by the test suite.

```
pycompile examples/fizzbuzz.py -o /tmp/fizzbuzz && /tmp/fizzbuzz
```

## Packaging and platforms

pycompile is available through the following channels:

| Platform | Command |
| --- | --- |
| Debian / Ubuntu (`.deb`) | `sudo apt install ./pycompile_*_amd64.deb` |
| Fedora / RHEL / openSUSE (`.rpm`) | `sudo dnf install ./pycompile-*.x86_64.rpm` |
| macOS (Homebrew) | `brew install ramackersjp/tap/pycompile` |
| Windows (Scoop) | `scoop bucket add ramackersjp https://github.com/ramackersjp/scoop-bucket`, then `scoop install pycompile` |
| Rust toolchain (Cargo) | `cargo install python-compiler` |
| Prebuilt binaries | `.tar.gz` / `.zip` from the [releases page](https://github.com/ramackersjp/pycompile/releases) |

In review / pending first publication:

- Arch Linux (AUR): `yay -S pycompile` (or `paru -S pycompile`); PKGBUILD in [`packaging/aur/`](packaging/aur/).
- Linux (Snap): `sudo snap install python-compiler` (classic confinement; uses the system C compiler); `snap/snapcraft.yaml`.
- Windows (winget): `winget install ramackersjp.pycompile`; manifests in [`packaging/winget/`](packaging/winget/).

On Windows, `pycompile` needs a C compiler to produce binaries: install MSVC Build
Tools, or use MinGW-w64 / LLVM (`clang`). This requirement applies only when you
invoke `pycompile`, not to the compiler binary itself.

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
