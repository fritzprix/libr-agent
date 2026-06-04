#!/usr/bin/env python3
"""
Conversion Status Checker
===========================
Reports the conversion status of binary documents in the workspace.
Distinguishes between converted / non-converted files, and prints non-converted files.

Usage:
    python check_status.py [--root ROOT] [--formats pdf pptx docx xlsx]
                           [--ignore-dirs DIR1 DIR2] [--show-converted]
                           [--export STATUS_MD]

Options:
    --root          Scan root (default: current directory)
    --formats       Formats to check (default: pdf pptx docx xlsx)
    --ignore-dirs   Additional directories to ignore
    --show-converted Output converted files as well
    --export        Save the status report to a .md file

Examples:
    python check_status.py --root .
    python check_status.py --root . --show-converted
    python check_status.py --root . --export conversion_status.md
"""

import argparse
import io
import sys
from datetime import datetime
from pathlib import Path

# Prevent UnicodeEncodeError on Windows due to cp949 encoding
if sys.platform.startswith("win"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")

sys.path.append(str(Path(__file__).resolve().parent))
from utils import get_source_from_md

SKIP_DIRS = {".git", "__pycache__", "node_modules", ".github", "venv", ".venv", ".libragent", "attachments"}


def collect_binary_files(root: Path, formats: list[str], skip_dirs: set[str]) -> list[Path]:
    exts = {f".{fmt.lstrip('.')}" for fmt in formats}
    result = []
    for p in sorted(root.rglob("*")):
        if p.is_dir():
            continue
        if any(part in skip_dirs for part in p.relative_to(root).parts):
            continue
        if p.suffix.lower() in exts:
            result.append(p)
    return result


def build_source_to_md_map(root: Path, skip_dirs: set[str]) -> dict[str, Path]:
    source_to_md: dict[str, Path] = {}
    for md_path in sorted(root.rglob("*.md")):
        if md_path.is_dir():
            continue
        if any(part in skip_dirs for part in md_path.relative_to(root).parts):
            continue
        source_path = get_source_from_md(md_path)
        if not source_path:
            continue
        try:
            source_to_md[str(Path(source_path).resolve().as_posix())] = md_path
        except Exception:
            continue
    return source_to_md


def resolve_converted_markdown(source_path: Path, source_to_md: dict[str, Path]) -> Path | None:
    mapped_md = source_to_md.get(str(source_path.resolve().as_posix()))
    if mapped_md and mapped_md.exists():
        return mapped_md

    default_md = source_path.with_suffix(".md")
    if default_md.exists():
        return default_md

    return None


def check_conversion(
    files: list[Path],
    source_to_md: dict[str, Path],
) -> tuple[list[tuple[Path, Path]], list[Path]]:
    """Split files into converted and not converted entries."""
    converted: list[tuple[Path, Path]] = []
    not_converted: list[Path] = []
    for f in files:
        md_path = resolve_converted_markdown(f, source_to_md)
        if md_path is not None:
            converted.append((f, md_path))
        else:
            not_converted.append(f)
    return converted, not_converted


def format_size(path: Path) -> str:
    size = path.stat().st_size
    if size >= 1024 * 1024:
        return f"{size / (1024*1024):.1f} MB"
    return f"{size / 1024:.1f} KB"


def render_report(
    root: Path,
    converted: list[tuple[Path, Path]],
    not_converted: list[Path],
    show_converted: bool,
) -> str:
    skill_base_dir = Path(__file__).resolve().parent.parent
    convert_script = (skill_base_dir / "scripts" / "convert_binary_docs.py").as_posix()
    run_script = (skill_base_dir / "scripts" / "run.py").as_posix()
    now = datetime.now().strftime("%Y-%m-%d %H:%M")
    total = len(converted) + len(not_converted)
    rate = (len(converted) / total * 100) if total else 0

    lines = [
        f"# Conversion Status Report",
        f"",
        f"> Generated at: {now}  ",
        f"> Root: `{root}`",
        f"",
        f"## Summary",
        f"",
        f"| Item | Count |",
        f"| --- | --- |",
        f"| Total binary documents | **{total}** |",
        f"| ✅ Converted | **{len(converted)}** |",
        f"| ❌ Non-converted | **{len(not_converted)}** |",
        f"| Conversion rate | **{rate:.1f}%** |",
        f"",
    ]

    if not_converted:
        lines += [
            f"---",
            f"",
            f"## ❌ Non-converted Files ({len(not_converted)})",
            f"",
            f"| # | Filename | Path | Size |",
            f"| --- | --- | --- | --- |",
        ]
        for i, p in enumerate(not_converted, 1):
            rel = str(p.relative_to(root).as_posix())
            lines.append(f"| {i} | `{p.name}` | `{rel}` | {format_size(p)} |")
        lines.append("")

        # Suggest conversion commands
        lines += [
            f"### Conversion Commands",
            f"",
            f"```bash",
            f"# Convert all non-converted files",
            f'python "{convert_script}" --root .',
            f"",
            f"# Or use the unified runner",
            f'python "{run_script}" --root .',
            f"```",
            f"",
        ]
    else:
        lines += [f"✅ All binary documents have been converted.", f""]

    if show_converted and converted:
        lines += [
            f"---",
            f"",
            f"## ✅ Converted Files ({len(converted)})",
            f"",
            f"| Filename | Original Size | Converted File |",
            f"| --- | --- | --- |",
        ]
        for source_path, md_path in converted:
            md_rel = str(md_path.relative_to(root).as_posix())
            lines.append(
                f"| `{source_path.name}` | {format_size(source_path)} | [`{md_path.name}`]({md_rel}) |"
            )
        lines.append("")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Check binary document conversion status")
    parser.add_argument("--root", default=".", help="Scan root directory")
    parser.add_argument("--formats", nargs="+", default=["pdf", "pptx", "docx", "xlsx"])
    parser.add_argument("--ignore-dirs", nargs="+", default=[])
    parser.add_argument("--show-converted", action="store_true", help="Output converted files as well")
    parser.add_argument("--export", default=None, help="Save report to a .md file")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    skip_dirs = SKIP_DIRS | set(args.ignore_dirs)

    files = collect_binary_files(root, args.formats, skip_dirs)
    source_to_md = build_source_to_md_map(root, skip_dirs)
    converted, not_converted = check_conversion(files, source_to_md)

    total = len(files)
    rate = (len(converted) / total * 100) if total else 0

    # Console output
    print(f"\n📊 Conversion Status Report")
    print(f"   Root: {root}")
    print(f"   Total: {total}  |  ✅ Converted: {len(converted)}  |  ❌ Non-converted: {len(not_converted)}  |  Rate: {rate:.1f}%\n")

    if not_converted:
        print(f"❌ Non-converted files ({len(not_converted)}):")
        for p in not_converted:
            print(f"   {p.relative_to(root)}")
    else:
        print("✅ All binary documents have been converted.")

    if args.show_converted and converted:
        print(f"\n✅ Converted files ({len(converted)}):")
        for source_path, md_path in converted:
            print(f"   {source_path.relative_to(root)}  →  {md_path.relative_to(root)}")

    # Save markdown report
    if args.export:
        report = render_report(root, converted, not_converted, args.show_converted)
        out = Path(args.export)
        out.write_text(report, encoding="utf-8")
        print(f"\n📄 Report saved to: {out}")


if __name__ == "__main__":
    main()
