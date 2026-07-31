#!/usr/bin/env python3
"""Locate the active SOUL.md file in the current workspace."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

CANDIDATES = [
    ".github/SOUL.md",
    "SOUL.md",
    ".github/soul.md",
    "soul.md",
]
BOOTSTRAP_RELATIVE_PATH = "references/base_soul.md"


def find_soul(root: Path) -> Path | None:
    for relative_path in CANDIDATES:
        candidate = root / relative_path
        if candidate.is_file():
            return candidate
    return None


def get_skill_root() -> Path:
    return Path(__file__).resolve().parent.parent


def get_bootstrap_template_path() -> Path:
    return get_skill_root() / BOOTSTRAP_RELATIVE_PATH


def read_bootstrap_template() -> str:
    return get_bootstrap_template_path().read_text(encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Locate the active SOUL.md file in the current workspace.",
    )
    parser.add_argument(
        "--root",
        default=".",
        help="Workspace root to inspect. Defaults to the current directory.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print machine-readable JSON.",
    )
    parser.add_argument(
        "--content",
        action="store_true",
        help="Print the discovered file contents after the path.",
    )
    parser.add_argument(
        "--bootstrap-content",
        action="store_true",
        help="Print the bundled bootstrap SOUL.md template.",
    )
    args = parser.parse_args()

    root = Path(args.root).resolve()
    soul_path = find_soul(root)
    suggested_create_path = root / "SOUL.md"
    bootstrap_template_path = get_bootstrap_template_path()

    if args.json:
        payload = {
            "root": str(root),
            "candidates": CANDIDATES,
            "found": str(soul_path) if soul_path else None,
            "suggested_create_path": str(suggested_create_path),
            "bootstrap_template_path": str(bootstrap_template_path),
        }
        print(json.dumps(payload, ensure_ascii=True, indent=2))
        if args.content and soul_path:
            print()
            print(soul_path.read_text(encoding="utf-8"))
        if args.bootstrap_content:
            print()
            print(read_bootstrap_template())
        return 0

    if args.bootstrap_content:
        print(read_bootstrap_template())
        return 0

    if soul_path is None:
        print(f"No soul file found under {root}")
        print("Checked candidates:")
        for candidate in CANDIDATES:
            print(f"- {candidate}")
        print(f"Suggested create path: {suggested_create_path}")
        print(f"Bootstrap template: {bootstrap_template_path}")
        return 1

    print(soul_path)
    if args.content:
        print()
        print(soul_path.read_text(encoding="utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
