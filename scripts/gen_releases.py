#!/usr/bin/env python3
"""Generate the release history section of docs/download.html from CHANGE_LOG.md.

The download page keeps everything between the two marker comments in sync with
the change log, so publishing a release is enough to update the site.
"""

from __future__ import annotations

import html
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGELOG = ROOT / "CHANGE_LOG.md"
PAGE = ROOT / "docs" / "download.html"
REPO = "ramackersjp/pycompile"
RELEASES = f"https://github.com/{REPO}/releases"

START = "<!-- releases:start -->"
END = "<!-- releases:end -->"

# (label, filename template) — {v} is replaced with the version without a leading v.
ARTIFACTS = [
    ("Linux x86_64 (tar.gz)", "pycompile-x86_64-unknown-linux-gnu.tar.gz"),
    ("Linux aarch64 (tar.gz)", "pycompile-aarch64-unknown-linux-gnu.tar.gz"),
    ("macOS Intel (tar.gz)", "pycompile-x86_64-apple-darwin.tar.gz"),
    ("macOS Apple Silicon (tar.gz)", "pycompile-aarch64-apple-darwin.tar.gz"),
    ("Windows x86_64 (zip)", "pycompile-x86_64-pc-windows-msvc.zip"),
    ("Debian/Ubuntu amd64 (deb)", "pycompile_{v}-1_amd64.deb"),
    ("Debian/Ubuntu arm64 (deb)", "pycompile_{v}-1_arm64.deb"),
    ("Fedora/RHEL x86_64 (rpm)", "pycompile-{v}-1.x86_64.rpm"),
    ("Fedora/RHEL aarch64 (rpm)", "pycompile-{v}-1.aarch64.rpm"),
    ("SHA256SUMS", "SHA256SUMS"),
]

# Cards shown for the latest release, mapping the platform to an artifact label.
PLATFORM_CARDS = [
    ("Linux", "x86_64", "glibc-based distributions (Ubuntu, Debian, Fedora, Arch).",
     "Linux x86_64 (tar.gz)"),
    ("Linux", "aarch64", "ARM64 distributions (Raspberry Pi OS 64-bit, AWS Graviton).",
     "Linux aarch64 (tar.gz)"),
    ("macOS", "Intel", "x86_64 Apple hardware and most Intel Macs.",
     "macOS Intel (tar.gz)"),
    ("macOS", "Apple Silicon", "M1, M2, M3 and newer Apple Silicon Macs.",
     "macOS Apple Silicon (tar.gz)"),
    ("Windows", "x86_64", "MSVC toolchain. Requires a C compiler at run time of the "
     "compiler itself.", "Windows x86_64 (zip)"),
]

HEADING = re.compile(r"^## \[(?P<version>\d+\.\d+\.\d+)\](?: - (?P<date>\d{4}-\d{2}-\d{2}))?\s*$")
MD_HEADING = re.compile(r"^### (?P<title>.+?)\s*$")
MD_ITEM = re.compile(r"^(?P<indent>\s*)- (?P<text>.*)$")


def parse_changelog(text: str) -> list[dict]:
    versions: list[dict] = []
    current: dict | None = None
    lines = text.splitlines()
    for line in lines:
        match = HEADING.match(line)
        if match:
            current = {"version": match.group("version"), "date": match.group("date"), "body": []}
            versions.append(current)
            continue
        if line.startswith("## "):
            current = None
            continue
        if current is not None:
            current["body"].append(line)
    return versions


def inline(text: str) -> str:
    text = html.escape(text, quote=False)
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", r'<a href="\2">\1</a>', text)
    return text


def render_markdown(body: list[str]) -> str:
    out: list[str] = []
    paragraph: list[str] = []
    item: list[str] | None = None
    in_list = False

    def flush_paragraph() -> None:
        if paragraph:
            out.append(f"<p>{inline(' '.join(paragraph))}</p>")
            paragraph.clear()

    def flush_item() -> None:
        nonlocal item
        if item is not None:
            out.append(f"  <li>{inline(' '.join(item))}</li>")
            item = None

    def close_list() -> None:
        nonlocal in_list
        flush_item()
        if in_list:
            out.append("</ul>")
            in_list = False

    for line in body:
        stripped = line.strip()
        if not stripped:
            flush_paragraph()
            close_list()
            continue
        heading = MD_HEADING.match(line)
        if heading:
            flush_paragraph()
            close_list()
            out.append(f"<h4>{inline(heading.group('title'))}</h4>")
            continue
        matched = MD_ITEM.match(line)
        if matched:
            flush_paragraph()
            flush_item()
            if not in_list:
                out.append("<ul>")
                in_list = True
            item = [matched.group("text").strip()]
            continue
        if item is not None:
            item.append(stripped)
        else:
            paragraph.append(stripped)

    flush_paragraph()
    close_list()
    return "\n".join(out)


def artifact_links(version: str) -> list[tuple[str, str]]:
    base = f"{RELEASES}/download/v{version}"
    return [(label, f"{base}/{name.format(v=version)}") for label, name in ARTIFACTS]


def latest_section(version: str, date: str) -> str:
    links = dict(artifact_links(version))
    cards = []
    for name, arch, description, label in PLATFORM_CARDS:
        href = links[label]
        cards.append(
            '        <div class="card">\n'
            f'          <h3>{name} <span class="tag">{arch}</span></h3>\n'
            f'          <p class="mb-0">{description}</p>\n'
            f'          <a class="btn btn-dark mt-16" href="{href}">Download</a>\n'
            "        </div>"
        )
    deb = links["Debian/Ubuntu amd64 (deb)"]
    rpm = links["Fedora/RHEL x86_64 (rpm)"]
    return (
        f'      <h2 style="margin-top:0;">Latest release <span class="tag">{version}</span></h2>\n'
        f'      <p class="sub">Released {date}, for Linux, macOS and Windows. Prefer a '
        f'package? Use the <a href="install.html">install guide</a> for Cargo, Homebrew, '
        f"apt and dnf.</p>\n"
        '      <div class="grid grid-2">\n'
        + "\n".join(cards)
        + "\n"
        '        <div class="card">\n'
        '          <h3>Debian / Fedora</h3>\n'
        '          <p class="mb-0">Native packages for apt and dnf based distributions.</p>\n'
        f'          <a class="btn btn-dark mt-16" href="{deb}">Download .deb</a> '
        f'<a class="btn btn-ghost mt-16" href="{rpm}">Download .rpm</a>\n'
        "        </div>\n"
        '        <div class="card">\n'
        "          <h3>All releases &amp; checksums</h3>\n"
        '          <p class="mb-0">Every tagged version with <code>SHA256SUMS</code> '
        "for verification.</p>\n"
        f'          <a class="btn btn-dark mt-16" href="{RELEASES}">Browse releases</a>\n'
        "        </div>\n"
        "      </div>"
    )


def history_section(versions: list[dict]) -> str:
    cards = []
    for index, entry in enumerate(versions):
        version = entry["version"]
        date = entry["date"] or "unreleased"
        badge = ' <span class="tag">latest</span>' if index == 0 else ""
        open_attr = " open" if index == 0 else ""
        files = "\n".join(
            f'            <li><a href="{href}">{html.escape(label)}</a></li>'
            for label, href in artifact_links(version)
        )
        changelog = render_markdown(entry["body"])
        cards.append(
            f'      <article class="release">\n'
            f'        <div class="release-head">\n'
            f'          <h3>{version}{badge}</h3>\n'
            f'          <span class="release-date">Released {date}</span>\n'
            f"        </div>\n"
            f'        <details class="release-files"{open_attr}>\n'
            f"          <summary>Download files</summary>\n"
            f'          <ul class="file-list">\n{files}\n          </ul>\n'
            f"        </details>\n"
            f'        <details class="release-log"{open_attr}>\n'
            f"          <summary>Change log</summary>\n"
            f'          <div class="changelog">\n{changelog}\n          </div>\n'
            f"        </details>\n"
            f"      </article>"
        )
    return (
        "      <h2>Release history</h2>\n"
        '      <p class="sub">Every tagged version with its files and change log. '
        f'See <a href="{RELEASES}">GitHub</a> for the full asset list of each release.</p>\n'
        + "\n".join(cards)
    )


def build_block(versions: list[dict]) -> str:
    latest = versions[0]
    return (
        f"{START}\n"
        f"{latest_section(latest['version'], latest['date'] or 'unreleased')}\n\n"
        f"{history_section(versions)}\n"
        f"{END}"
    )


def main() -> int:
    versions = parse_changelog(CHANGELOG.read_text(encoding="utf-8"))
    if not versions:
        print("no released versions found in CHANGE_LOG.md", file=sys.stderr)
        return 1

    block = build_block(versions)
    page = PAGE.read_text(encoding="utf-8")
    if START not in page or END not in page:
        print(f"markers {START} / {END} not found in {PAGE}", file=sys.stderr)
        return 1

    updated = re.sub(
        re.escape(START) + r".*?" + re.escape(END),
        lambda _: block,
        page,
        flags=re.DOTALL,
    )
    if updated != page:
        PAGE.write_text(updated, encoding="utf-8")
        print(f"updated {PAGE.relative_to(ROOT)} ({len(versions)} releases)")
    else:
        print(f"{PAGE.relative_to(ROOT)} is up to date")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
