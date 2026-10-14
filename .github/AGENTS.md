# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project overview

`pycompile` (also installed as `python_compiler`) is an ahead-of-time compiler that
translates a supported subset of Python into C, invokes the system C compiler, and
emits a single standalone native executable. There is no interpreter at run time.

- Crate: `python-compiler`, library `python_compiler`, binaries `pycompile` and
  `python_compiler`.
- Zero external Rust dependencies. Edition 2021.
- Release profile uses LTO, `codegen-units = 1` and stripping for small binaries.

## Architecture

Pipeline: `lexer -> parser -> codegen -> driver`.

| Module | Responsibility |
| --- | --- |
| `src/lexer.rs` | Indentation-aware tokenizer (INDENT/DEDENT, f-strings, escapes). |
| `src/parser.rs` | Recursive-descent parser producing the AST; also `parse_expression`. |
| `src/ast.rs` | AST types (`Expr`, `Stmt`, `Module`, `Param`). |
| `src/codegen.rs` | AST -> C text; collects globals/functions/locals, emits prototypes, `main`. |
| `src/driver.rs` | Embeds and compiles the C runtime; detects and invokes the C compiler. |
| `src/man.rs` | Terminal manual and man-page installation (`pycompile man`). |
| `src/lib.rs` | CLI parsing and orchestration (`run()`). |
| `runtime/pyrt.c`, `.h` | Small C runtime: tagged-union `Pyo`, operators, containers, builtins. |

Generated C conventions: globals `g_<name>`, locals `v_<name>`, functions
`pyfn_<name>_<idx>`, temporaries `t<N>`. Empty argument arrays become
`(Pyo*[]){NULL}`. Parameter names are sanitized to avoid C keyword clashes.

## Build, test and validate

Run these before considering work done:

```
cargo build            # debug build
cargo build --release  # release build
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Integration tests (`tests/integration.rs`) compile each `examples/*.py` with the real
binary and compare its output to `examples/<name>.out`. They require a working C
compiler (`cc`/`gcc`/`clang`) on `PATH`, or the `CC` environment variable.

To check an example manually:

```
cargo run -- examples/hello.py -o /tmp/hello && /tmp/hello
```

## Conventions

- All repository content is English: code, comments, docs, commit messages, PR text.
- Keep code concise and simple. Avoid unnecessary abstractions, duplication and
  boilerplate. Prefer clarity, then fewer lines.
- Avoid comments unless the intent or reasoning is not obvious from the code.
- Follow the existing structure and naming before inventing new patterns.
- Add new runtime helpers to both `runtime/pyrt.h` and `runtime/pyrt.c`.
- Unsupported Python constructs must fail at compile time with a clear message
  (file, line, column) rather than producing broken C.

## Adding language support

1. Extend the relevant type in `src/ast.rs`.
2. Parse the construct in `src/parser.rs`.
3. Add a case to `src/codegen.rs` (statement or expression) and any needed runtime
   helper in `runtime/`.
4. Add an example under `examples/` and record expected output in `examples/<name>.out`
   (generate with `python3 examples/<name>.py`).
5. Add/extend a test in `tests/integration.rs`.
6. Update `docs/`, the man page `docs/pycompile.1`, and `CHANGE_LOG.md`.

## Website

The static site lives in `docs/` and is deployed to Vercel. `vercel.json` at the
repository root serves `docs/` as-is (`buildCommand: null`, `outputDirectory: "docs"`).
Pages: `index.html`, `docs.html`, `install.html`, `download.html`, shared `style.css`,
plus `pycompile.1`. There is no GitHub Pages workflow (Pages requires a paid plan for
private repositories).

## Release

Pushing a tag matching `v*` triggers `.github/workflows/release.yml`, which builds
binaries for Linux (x86_64/aarch64), macOS (x86_64/aarch64) and Windows (x86_64),
packages them, writes `SHA256SUMS`, and publishes a GitHub release. The download
page links to these assets.

## Packaging

OS package definitions live under `packaging/` (AUR, Flatpak, Homebrew, Scoop, winget)
and `snap/` (Snap). `.deb` and `.rpm` are built in the release workflow from
`[package.metadata.deb]` and `[package.metadata.generate-rpm]` in `Cargo.toml` and
attached to the release.

After a successful release, `.github/workflows/packages.yml` runs (via `workflow_run`,
and can be re-run with `workflow_dispatch` and a `tag` input). It downloads the
release `SHA256SUMS`, renders the templates in `.github/packages/` with the new version
and hashes, and pushes the results to the `ramackersjp/homebrew-tap` (Homebrew) and
`ramackersjp/scoop-bucket` (Scoop) repositories, then opens a winget PR with
`vedantmgoyal9/winget-releaser`.

This requires a repository secret `PACKAGES_TOKEN`: a **classic** PAT with the `repo`
and `workflow` scopes, owned by the account that holds the `microsoft/winget-pkgs`
fork. The winget step is `continue-on-error` until the first version of `pycompile`
is accepted into `winget-pkgs`.
