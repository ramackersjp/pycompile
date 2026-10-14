# Change Log

All notable changes to this project are documented in this file.

This log is written for both humans and AI agents: entries should be concise,
factual, and reference the affected modules or files where useful.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial `pycompile` compiler: Python source is translated to C and compiled to a
  standalone native binary by the system C compiler (`src/lexer.rs`, `src/parser.rs`,
  `src/codegen.rs`, `src/driver.rs`).
- Embedded C runtime `runtime/pyrt.c` / `runtime/pyrt.h` with tagged-union values,
  operators, comparisons, lists, tuples, dicts, sets, iterators, f-string formatting,
  and a set of builtins.
- Supported syntax: literals and `None`, f-strings (format specs and `!r`),
  lists/tuples/sets/dicts and comprehensions (list, set, dict), `if`/`elif`/`else`,
  `while`, `for`-`else`, `break`, `continue`, `pass`, `def` with defaults, recursion,
  `*args` / `**kwargs` parameters, calls with positional/keyword/`*`/`**` arguments,
  chained comparisons, `in`/`not in`, `is`/`is not`, slicing, item and augmented
  assignment, the `math` and `sys` modules.
- Two equivalent command-line entry points: `pycompile` and `python_compiler`
  (`src/lib.rs`, `src/main.rs`, `src/bin/python_compiler.rs`).
- CLI options: `-o/--output`, `--emit-c`, `--keep-temps`, `-r/--run`, `--man`,
  `-h/--help`, `-V/--version`.
- Terminal manual and man-page installation: `pycompile man`,
  `pycompile man --install`, `pycompile man --prefix DIR` (`src/man.rs`,
  `docs/pycompile.1`).
- `key` and `reverse` support for `sorted()` and `list.sort()`, including first-class
  references to common builtins such as `len`, `str`, `int` and `abs`.
- Static documentation site with install and download instructions (`docs/`), deployed
  to Vercel with zero configuration (`vercel.json`: no build step, `outputDirectory`
  set to `docs`).
- Example programs with recorded expected output (`examples/`).
- Integration tests that compile and run every example and compare output
  (`tests/integration.rs`).
- CI workflows: validate (format/lint/build/test/examples/man page/site) and release
  binary builds (`.github/workflows/`).
- Dependabot configuration for Cargo and GitHub Actions dependencies.
- OS packaging: `.deb` and `.rpm` packages are built and attached to each release
  (`Cargo.toml` metadata, `.github/workflows/release.yml`).
- Install channels documented in the README and install page: `.deb`, `.rpm`,
  Homebrew, Scoop, Cargo and prebuilt archives (AUR, Snap and winget are prepared and
  listed as pending first publication).
- Packaging manifests under `packaging/` and `snap/` for AUR, Flatpak, Homebrew,
  Scoop, winget and Snap. The Flatpak manifest is provided for self-build only:
  Flathub is not used because the sandbox has no C compiler for pycompile to invoke.

### Fixed

- Relative output paths (for example `pycompile prog.py -o prog`) now write the binary
  into the caller's working directory instead of the temporary build directory, which
  was deleted afterwards. Output paths are resolved to absolute before invoking the C
  compiler (`src/driver.rs`).

### Notes and limitations

- No garbage collection: allocations are not reclaimed until process exit.
- `class`, closures, nested scopes, exceptions (`try`/`except`), `with`, decorators,
  `lambda`, `async`/`await`, and modules other than `math`/`sys` are not supported and
  produce a clear compile-time error.
- `%` string formatting is not supported; use f-strings.
- `sort`/`sorted` are not fully stable when `reverse=True`.
