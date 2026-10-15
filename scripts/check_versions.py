#!/usr/bin/env python3
"""Fail if hand-maintained version references do not match Cargo.toml.

Package manifests under packaging/ (except Flatpak) are rewritten by the packages
workflow after a release and are intentionally not checked here.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# file -> list of substrings that must be present for the current version.
REFERENCES: dict[str, list[str]] = {
    "CHANGE_LOG.md": ["## [{v}]"],
    "docs/download.html": ["v{v}"],
    "docs/pycompile.1": ["pycompile {v}"],
    "docs/install.html": ["pycompile_{v}_amd64.deb", "pycompile-{v}-1.x86_64.rpm"],
    "packaging/flatpak/io.github.ramackersjp.pycompile.yml": ["tags/v{v}.tar.gz"],
    "packaging/flatpak/io.github.ramackersjp.pycompile.metainfo.xml": ['release version="{v}"'],
}


def cargo_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version = "([^"]+)"', text, re.MULTILINE)
    if not match:
        raise SystemExit("could not read version from Cargo.toml")
    return match.group(1)


def main() -> int:
    version = cargo_version()
    failures: list[str] = []
    for name, patterns in REFERENCES.items():
        path = ROOT / name
        if not path.exists():
            failures.append(f"{name}: file is missing")
            continue
        text = path.read_text(encoding="utf-8")
        for pattern in patterns:
            expected = pattern.format(v=version)
            if expected not in text:
                failures.append(f"{name}: missing {expected!r}")

    if failures:
        print(f"Cargo.toml is at {version}, but:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        print(
            "Update these references (see the Release section in .github/AGENTS.md).",
            file=sys.stderr,
        )
        return 1

    print(f"version references match {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
