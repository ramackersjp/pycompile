# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project overview

`pycompile` is an ahead-of-time compiler that translates a supported subset of Python
into C, invokes the system C compiler, and emits a single standalone native executable.
There is no interpreter at run time.

- Crate: `python-compiler`, library and binary `pycompile`.
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

`docs/download.html` lists the latest release and the full release history (files and
change log). The section between the `<!-- releases:start -->` and `<!-- releases:end -->`
markers is generated from `CHANGE_LOG.md` by `scripts/gen_releases.py`; run it after
editing the change log. The `site` job in `validate.yml` fails if the page is stale.

## Release

Pushing a tag matching `v*` triggers `.github/workflows/release.yml`, which builds
binaries for Linux (x86_64/aarch64), macOS (x86_64/aarch64) and Windows (x86_64),
packages them, writes `SHA256SUMS`, and publishes a GitHub release. The download
page links to these assets by their stable file names.

To cut a release:

1. Move the `[Unreleased]` entries in `CHANGE_LOG.md` into a new
   `## [x.y.z] - YYYY-MM-DD` section and bump `version` in `Cargo.toml`.
2. Run `python3 scripts/gen_releases.py` to refresh `docs/download.html` (the
   release history is generated from the change log).
3. Commit, push `trunk`, tag `vX.Y.Z` and push the tag. Vercel redeploys the site
   from `trunk`; the tag triggers the binary release and, on success, the
   `packages` workflow.

## Packaging

OS package definitions live under `packaging/` (AUR, Flatpak, Homebrew, Scoop, winget)
and `snap/` (Snap). `.deb` and `.rpm` are built in the release workflow from
`[package.metadata.deb]` and `[package.metadata.generate-rpm]` in `Cargo.toml` and
attached to the release.

After a successful release, `.github/workflows/packages.yml` runs (via `workflow_run`,
and can be re-run with `workflow_dispatch` and a `tag` input):

- `update` job: downloads the release `SHA256SUMS`, renders the templates in
  `.github/packages/` with the new version and hashes, and pushes the results to the
  `ramackersjp/homebrew-tap` (Homebrew) and `ramackersjp/scoop-bucket` (Scoop)
  repositories, then opens a winget PR with `vedantmgoyal9/winget-releaser`.
- `aur` job: clones the AUR repository `ssh://aur@aur.archlinux.org/pycompile.git`,
  bumps `pkgver`/`pkgrel`, updates `sha256sums` from the release tag tarball, rewrites
  the matching fields in `.SRCINFO`, and pushes.
- `snap` job: builds `snap/snapcraft.yaml` with `snapcore/action-build`, then uploads and
  releases to the `stable` channel with `snapcore/action-publish`. It is skipped unless
  the store credential secret is set.

All jobs are idempotent: when nothing changes they skip the commit and push. To bump
the AUR package for a version, keep `PKGBUILD` and `.SRCINFO` in step with the fields
the workflow rewrites; do not hand-edit them unless you also update the workflow.

Required repository secrets (never commit keys or tokens to git):

- `PACKAGES_TOKEN`: a **classic** PAT with the `repo` and `workflow` scopes, owned by
  the account that holds the `microsoft/winget-pkgs` fork. The winget step is
  `continue-on-error` until the first version of `pycompile` is accepted into
  `winget-pkgs`.
- `AUR_SSH_PRIVATE_KEY`: an SSH private key whose public key is registered in the AUR
  account. Used only by the `aur` job.
- `SNAPCRAFT_STORE_CREDENTIALS`: optional; the output of
  `snapcraft export-login --snaps python-compiler --acls package_access,package_push,package_update,package_release -`
  for the Snap Store account. Without it the `snap` job is skipped. Snap classic
  confinement requires Snap Store approval before a `stable` release.

### Rotating the AUR deploy key

The `aur` job authenticates with the `AUR_SSH_PRIVATE_KEY` secret.

1. Generate a new key: `ssh-keygen -t ed25519 -C pycompile-ci -f /tmp/aur-ci -N ""`.
2. Add `/tmp/aur-ci.pub` to https://aur.archlinux.org/account/ (the SSH Public Key
   field accepts multiple keys, one per line).
3. Update the secret: `gh secret set AUR_SSH_PRIVATE_KEY --repo ramackersjp/pycompile < /tmp/aur-ci`.
4. Verify: `gh workflow run packages --repo ramackersjp/pycompile -f tag=vX.Y.Z` and
   check that the `aur` job succeeds.
5. Delete `/tmp/aur-ci*` and remove the old key from the AUR account.
