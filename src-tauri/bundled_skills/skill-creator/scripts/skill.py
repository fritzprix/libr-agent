#!/usr/bin/env python3
"""Create, validate, and deploy LibrAgent user skills.

Usage:
    python3 skill.py new <name> [--to <dir>]
    python3 skill.py clone <name> [--to <dir>]
    python3 skill.py validate <dir>
    python3 skill.py deploy <dir>

User skills are read-only to the agent except through `deploy`, which writes
only to <user skills>/<name>/ and only after strict validation. `new` and
`clone` work in the workspace (default: the current directory) and never modify
the source skill. Standard library only. Pass --data-dir to override the app
data directory.
"""

from __future__ import annotations

import argparse
import platform
import re
import shutil
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from validate_skill import format_detailed_report, validate_skill_path

NAME_RE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
IGNORED_NAMES = {"__pycache__", ".git", ".hg", ".svn", "node_modules", ".DS_Store", "Thumbs.db"}
IGNORED_SUFFIXES = (".pyc", ".pyo")

TEMPLATE = """---
name: {name}
description: "Describe what this skill does and when to use it. Put every trigger phrase here."
---

# {title}

Imperative instructions go here. Keep this file short; move detail into references/.
"""


def default_data_dir() -> Path:
    home = Path.home()
    system = platform.system()
    if system == "Windows":
        import os

        appdata = Path(os.environ.get("APPDATA", str(home / "AppData" / "Roaming")))
        return appdata / "com.fritzprix.libragent"
    if system == "Darwin":
        return home / "Library" / "Application Support" / "com.fritzprix.libragent"
    return home / ".local" / "share" / "com.fritzprix.libragent"


def fail(message: str) -> int:
    print(message, file=sys.stderr)
    return 1


def ignore_junk(_directory: str, names: list[str]) -> set[str]:
    return {n for n in names if n in IGNORED_NAMES or n.lower().endswith(IGNORED_SUFFIXES)}


def validate(skill_dir: Path, title: str = "Validate") -> bool:
    report = validate_skill_path(skill_dir, strict=True)
    print(format_detailed_report(report, f"{title} {skill_dir}"))
    return report.ok


def cmd_new(name: str, dest_root: Path) -> int:
    if not NAME_RE.match(name):
        return fail(f"Invalid skill name {name!r}: use lowercase letters, digits and single hyphens.")
    target = dest_root / name
    if target.exists():
        return fail(f"{target} already exists.")
    target.mkdir(parents=True)
    (target / "SKILL.md").write_text(
        TEMPLATE.format(name=name, title=name.replace("-", " ").title()), encoding="utf-8"
    )
    print(f"Created {target}. Edit SKILL.md, then run: skill.py validate {target}")
    return 0


def cmd_clone(name: str, dest_root: Path, data_dir: Path) -> int:
    if not NAME_RE.match(name):
        return fail(f"Invalid skill name {name!r}.")
    source = next(
        (p for p in (data_dir / "user_skills" / name, data_dir / "system_skills" / name) if (p / "SKILL.md").is_file()),
        None,
    )
    if source is None:
        return fail(f"Skill {name!r} not found in user_skills or system_skills under {data_dir}.")
    target = dest_root / name
    if target.exists():
        return fail(f"{target} already exists.")
    shutil.copytree(source, target, ignore=ignore_junk)
    print(f"Cloned {source} -> {target}. Edit it, then run: skill.py validate {target}")
    return 0


def cmd_deploy(skill_dir: Path, data_dir: Path) -> int:
    if not validate(skill_dir, "Pre-deploy validation"):
        return 1
    name = skill_dir.name
    if not NAME_RE.match(name):
        return fail(f"Invalid skill folder name {name!r}.")
    if not data_dir.is_dir():
        return fail(f"App data directory not found: {data_dir}. Start LibrAgent once or pass --data-dir.")

    user_skills = (data_dir / "user_skills").resolve()
    target = user_skills / name
    if target.parent != user_skills or target.is_symlink():
        return fail(f"Refusing to write outside {user_skills}/<name>/.")
    if skill_dir.resolve() == target:
        return fail("The skill is already in the user skills folder; edit a workspace copy (skill.py clone).")

    user_skills.mkdir(exist_ok=True)
    backup = None
    with tempfile.TemporaryDirectory(prefix="skill-backup-") as backup_root:
        if target.exists():
            backup = Path(backup_root) / name
            shutil.copytree(target, backup)
            shutil.rmtree(target)
        try:
            shutil.copytree(skill_dir, target, ignore=ignore_junk)
            ok = validate(target, "Post-deploy validation")
        except OSError as error:
            print(f"Copy failed: {error}", file=sys.stderr)
            ok = False
        if not ok:
            shutil.rmtree(target, ignore_errors=True)
            if backup is not None:
                shutil.copytree(backup, target)
            return fail("Deploy failed; previous state restored.")
    print(f"Deployed {name} to {target}. It becomes active on the next agent turn.")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-dir", type=Path, default=None)
    sub = parser.add_subparsers(dest="command", required=True)
    for command in ("new", "clone"):
        p = sub.add_parser(command)
        p.add_argument("name")
        p.add_argument("--to", type=Path, default=Path.cwd())
    for command in ("validate", "deploy"):
        sub.add_parser(command).add_argument("directory", type=Path)
    args = parser.parse_args(argv)
    data_dir = (args.data_dir or default_data_dir()).resolve()

    if args.command == "new":
        return cmd_new(args.name, args.to.resolve())
    if args.command == "clone":
        return cmd_clone(args.name, args.to.resolve(), data_dir)
    skill_dir = args.directory.resolve()
    if args.command == "validate":
        return 0 if validate(skill_dir) else 1
    return cmd_deploy(skill_dir, data_dir)


if __name__ == "__main__":
    sys.exit(main())
