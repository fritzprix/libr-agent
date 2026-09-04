#!/usr/bin/env python3
"""Host-global skill-evolution wiki (cross-session, Win / macOS / Linux).

Root: Path.home() / ".libragent" / "wiki"
  Linux/macOS: ~/.libragent/wiki
  Windows:     %USERPROFILE%\\.libragent\\wiki
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import List, Optional

WIKI_ROOT = Path.home() / ".libragent" / "wiki"
SKILL_ROOT = Path(__file__).resolve().parent.parent
REF_ROOT = SKILL_ROOT / "references"


def configure_stdio() -> None:
    """Best-effort UTF-8 stdio (important on Windows consoles)."""
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            try:
                reconfigure(encoding="utf-8")
            except (OSError, ValueError):
                pass


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def template(name: str) -> str:
    path = REF_ROOT / name
    if not path.is_file():
        raise FileNotFoundError("Missing template: {0}".format(path))
    return path.read_text(encoding="utf-8")


def ensure_parents(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def dump(obj: object) -> None:
    print(json.dumps(obj, ensure_ascii=False))


def rel_under_wiki(rel: str) -> Path:
    cleaned = rel.replace("\\", "/").lstrip("/")
    parts = Path(cleaned).parts
    if ".." in parts:
        raise ValueError("path escapes wiki root")
    return WIKI_ROOT.joinpath(*parts)


def cmd_path(_: argparse.Namespace) -> int:
    dump(
        {
            "wikiRoot": str(WIKI_ROOT),
            "wikiRootPosix": WIKI_ROOT.as_posix(),
            "home": str(Path.home()),
            "platformHint": {
                "linuxMac": "~/.libragent/wiki",
                "windows": "%USERPROFILE%\\.libragent\\wiki",
            },
        }
    )
    return 0


def cmd_init(_: argparse.Namespace) -> int:
    WIKI_ROOT.mkdir(parents=True, exist_ok=True)
    (WIKI_ROOT / "patterns").mkdir(parents=True, exist_ok=True)
    files = {
        "index.md": "index-template.md",
        "logs.md": "logs-template.md",
        "skill-impact.md": "skill-impact-template.md",
    }
    created: List[str] = []
    skipped: List[str] = []
    for rel, tmpl in files.items():
        dest = WIKI_ROOT / rel
        if dest.exists():
            skipped.append(rel)
            continue
        ensure_parents(dest)
        dest.write_text(template(tmpl), encoding="utf-8")
        created.append(rel)
    dump({"wikiRoot": str(WIKI_ROOT), "created": created, "skippedExisting": skipped})
    return 0


def cmd_list_patterns(_: argparse.Namespace) -> int:
    patterns_dir = WIKI_ROOT / "patterns"
    ids: List[str] = []
    if patterns_dir.is_dir():
        ids = sorted(p.stem for p in patterns_dir.glob("*.md") if p.is_file())
    dump({"wikiRoot": str(WIKI_ROOT), "patterns": ids})
    return 0


def cmd_cat(args: argparse.Namespace) -> int:
    try:
        path = rel_under_wiki(args.rel)
    except ValueError as exc:
        dump({"error": str(exc)})
        return 2
    if not path.is_file():
        dump({"error": "not found", "path": str(path)})
        return 1
    sys.stdout.write(path.read_text(encoding="utf-8"))
    return 0


def read_pattern_body(args: argparse.Namespace) -> str:
    if args.file:
        return Path(args.file).expanduser().resolve().read_text(encoding="utf-8")
    if args.stdin:
        return sys.stdin.read()
    return template("pattern-template.md")


def cmd_write_pattern(args: argparse.Namespace) -> int:
    pattern_id = args.id.strip()
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", pattern_id):
        dump({"error": "id must be kebab-case [a-z0-9-]"})
        return 2
    content = read_pattern_body(args)
    if (args.stdin or args.file) and not content.strip():
        dump({"error": "empty content"})
        return 2
    dest = WIKI_ROOT / "patterns" / "{0}.md".format(pattern_id)
    ensure_parents(dest)
    dest.write_text(content, encoding="utf-8")
    dump({"wrote": str(dest)})
    return 0


def cmd_upsert_index(args: argparse.Namespace) -> int:
    path = WIKI_ROOT / "index.md"
    if not path.is_file():
        cmd_init(argparse.Namespace())
    lines = path.read_text(encoding="utf-8").splitlines()
    updated = args.updated or utc_now()[:10]
    row = "| {0} | {1} | {2} | {3} |".format(args.id, args.status, args.one_line, updated)
    out: List[str] = []
    replaced = False
    for line in lines:
        if line.startswith("| {0} |".format(args.id)):
            out.append(row)
            replaced = True
        elif line.strip() == "| _(none yet)_ | | | |":
            continue
        else:
            out.append(line)
    if not replaced:
        inserted = False
        new_out: List[str] = []
        for line in out:
            new_out.append(line)
            if not inserted and re.match(r"^\|\s*-+", line):
                new_out.append(row)
                inserted = True
        out = new_out if inserted else out + [row]
    path.write_text("\n".join(out).rstrip() + "\n", encoding="utf-8")
    dump({"index": str(path), "row": row})
    return 0


def cmd_prepend_log(args: argparse.Namespace) -> int:
    path = WIKI_ROOT / "logs.md"
    if not path.is_file():
        cmd_init(argparse.Namespace())
    stamp = args.timestamp or utc_now()
    entry = "## {0}\n\n{1}\n".format(stamp, args.message.strip())
    body = path.read_text(encoding="utf-8")
    marker = "---\n"
    if marker in body:
        head, tail = body.split(marker, 1)
        body = head + marker + "\n" + entry + tail.lstrip("\n")
    else:
        body = body.rstrip() + "\n\n" + entry
    path.write_text(body, encoding="utf-8")
    dump({"logs": str(path)})
    return 0


def cmd_append_impact(args: argparse.Namespace) -> int:
    path = WIKI_ROOT / "skill-impact.md"
    if not path.is_file():
        cmd_init(argparse.Namespace())
    stamp = args.timestamp or utc_now()
    row = "| {0} | {1} | {2} | {3} | {4} | {5} | {6} |".format(
        stamp,
        args.skill,
        args.summary,
        args.mechanism,
        args.validation,
        args.decision,
        args.notes,
    )
    text = path.read_text(encoding="utf-8")
    text = text.replace("| _(none yet)_ | | | | | | |\n", "")
    if not text.endswith("\n"):
        text += "\n"
    path.write_text(text + row + "\n", encoding="utf-8")
    dump({"skillImpact": str(path), "row": row})
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("path", help="Print wiki root JSON")
    p.set_defaults(func=cmd_path)

    p = sub.add_parser("init", help="Create wiki files if missing")
    p.set_defaults(func=cmd_init)

    p = sub.add_parser("list-patterns", help="List pattern ids")
    p.set_defaults(func=cmd_list_patterns)

    p = sub.add_parser("cat", help="Print a file relative to wiki root")
    p.add_argument("rel", help="e.g. index.md or patterns/foo.md")
    p.set_defaults(func=cmd_cat)

    p = sub.add_parser("write-pattern", help="Write patterns/<id>.md")
    p.add_argument("--id", required=True)
    src = p.add_mutually_exclusive_group()
    src.add_argument(
        "--stdin",
        action="store_true",
        help="Read full markdown from stdin",
    )
    src.add_argument(
        "--file",
        help="Read full markdown from a UTF-8 file (preferred on Windows)",
    )
    p.set_defaults(func=cmd_write_pattern)

    p = sub.add_parser("upsert-index", help="Insert/replace an index row")
    p.add_argument("--id", required=True)
    p.add_argument("--status", required=True)
    p.add_argument("--one-line", required=True)
    p.add_argument("--updated", default="")
    p.set_defaults(func=cmd_upsert_index)

    p = sub.add_parser("prepend-log", help="Prepend a logs.md entry")
    p.add_argument("--message", required=True)
    p.add_argument("--timestamp", default="")
    p.set_defaults(func=cmd_prepend_log)

    p = sub.add_parser("append-impact", help="Append skill-impact.md row")
    p.add_argument("--skill", required=True)
    p.add_argument("--summary", required=True)
    p.add_argument("--mechanism", required=True)
    p.add_argument("--validation", required=True)
    p.add_argument("--decision", required=True)
    p.add_argument("--notes", default="")
    p.add_argument("--timestamp", default="")
    p.set_defaults(func=cmd_append_impact)

    return parser


def main(argv: Optional[List[str]] = None) -> int:
    configure_stdio()
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return int(args.func(args))
    except FileNotFoundError as exc:
        dump({"error": str(exc)})
        return 1
    except OSError as exc:
        dump({"error": str(exc)})
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
