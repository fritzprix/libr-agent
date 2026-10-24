#!/usr/bin/env python3
"""Validate a skill folder and install it through the LibrAgent host.

Usage:
    python3 skill.py validate <dir>
    python3 skill.py deploy <dir>

`deploy` validates strictly, then sends the files to the host install endpoint
named by LIBRAGENT_SKILL_INSTALL_URL. It never writes to the user skills folder
itself; the host validates again and installs. Standard library only.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from validate_skill import format_detailed_report, validate_skill_path

INSTALL_URL_ENV = "LIBRAGENT_SKILL_INSTALL_URL"
SKIPPED_DIRS = {"__pycache__", ".git", ".hg", ".svn", "node_modules"}
SKIPPED_FILES = {".DS_Store", "Thumbs.db"}
REQUEST_TIMEOUT_SECS = 60


def collect_files(skill_dir: Path) -> list[dict[str, str]]:
    files = []
    for path in sorted(skill_dir.rglob("*")):
        relative = path.relative_to(skill_dir)
        if any(part in SKIPPED_DIRS for part in relative.parts) or path.name in SKIPPED_FILES:
            continue
        if path.is_symlink() or not path.is_file():
            continue
        files.append(
            {
                "path": relative.as_posix(),
                "contentBase64": base64.b64encode(path.read_bytes()).decode("ascii"),
            }
        )
    return files


def validate(skill_dir: Path) -> bool:
    report = validate_skill_path(skill_dir, strict=True)
    print(format_detailed_report(report, f"Validate {skill_dir}"))
    return report.ok


def deploy(skill_dir: Path) -> int:
    url = os.environ.get(INSTALL_URL_ENV, "").strip()
    if not url:
        print(
            f"{INSTALL_URL_ENV} is not set. Skill install is only available in host sessions "
            "with the LibrAgent app running.",
            file=sys.stderr,
        )
        return 2
    if not validate(skill_dir):
        return 1

    body = json.dumps({"files": collect_files(skill_dir)}).encode("utf-8")
    request = urllib.request.Request(
        url, data=body, method="POST", headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(request, timeout=REQUEST_TIMEOUT_SECS) as response:
            print(response.read().decode("utf-8", "replace"))
        return 0
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", "replace")
        print(f"Install rejected by host (HTTP {error.code}): {detail}", file=sys.stderr)
        return 1
    except (urllib.error.URLError, OSError) as error:
        # The URL embeds the session token, so report the reason only.
        print(f"Could not reach the LibrAgent host: {getattr(error, 'reason', error)}", file=sys.stderr)
        return 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("validate", "deploy"):
        sub.add_parser(name).add_argument("directory", type=Path)
    args = parser.parse_args(argv)

    skill_dir = args.directory.resolve()
    if args.command == "validate":
        return 0 if validate(skill_dir) else 1
    return deploy(skill_dir)


if __name__ == "__main__":
    sys.exit(main())
