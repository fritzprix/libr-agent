#!/usr/bin/env python3
"""
Workspace Binary Document Converter
====================================
Convert binary document files (PPTX, PDF, DOCX, XLSX) into text/Markdown.

Usage:
    python convert_binary_docs.py [--root ROOT_DIR] [--out OUT_DIR] [--formats pdf pptx docx xlsx]

Options:
    --root      Root directory to scan (default: current directory)
    --out       Output directory for converted files (default: next to the source file)
    --formats   File formats to convert (default: pdf pptx docx xlsx)
    --overwrite Overwrite existing converted files (default: False)
    --dry-run   Print target files without converting anything

Dependencies:
    pip install "markitdown[all]"  (see to-md skill)

Examples:
    # Convert the full workspace
    python convert_binary_docs.py --root . --out ./converted

    # Convert PDFs only
    python convert_binary_docs.py --root . --formats pdf

    # Preview target files with dry-run
    python convert_binary_docs.py --root . --dry-run
"""

import argparse
import io
import sys
from pathlib import Path

# Prevent UnicodeEncodeError on Windows due to cp949 encoding
if sys.platform.startswith("win"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")

# Add scripts directory to sys.path for Cwd-independent imports
sys.path.append(str(Path(__file__).resolve().parent))
from utils import get_source_from_md, get_path_hash

SKIP_DIRS = {".git", "__pycache__", "node_modules", ".github", "venv", ".venv", ".libragent", "attachments"}


def find_binary_files(root: Path, formats: list[str]) -> list[Path]:
    """Recursively find binary files matching the requested formats."""
    exts = {f".{fmt.lstrip('.')}" for fmt in formats}
    result = []
    for p in root.rglob("*"):
        if any(part in SKIP_DIRS for part in p.relative_to(root).parts):
            continue
        if p.suffix.lower() in exts and p.is_file():
            result.append(p)
    return sorted(result)


def convert_with_markitdown(src: Path) -> str:
    """Convert file content to Markdown via MarkItDown (to-md skill policy)."""
    try:
        from markitdown import MarkItDown
    except ImportError:
        return (
            '[ERROR] markitdown not installed. Run: pip install "markitdown[all]"\n'
            f"Source: {src}"
        )

    result = MarkItDown().convert(str(src))
    return result.text_content if result.text_content else "(no text content)"


CONVERTERS = {
    ".pdf": convert_with_markitdown,
    ".docx": convert_with_markitdown,
    ".pptx": convert_with_markitdown,
    ".xlsx": convert_with_markitdown,
}


def is_same_source_markdown(dest: Path, src: Path) -> bool:
    """Return True when the existing markdown file was generated from the same source."""
    if not dest.exists():
        return False

    source_in_md = get_source_from_md(dest)
    if not source_in_md:
        return False

    try:
        return Path(source_in_md).resolve() == src.resolve()
    except OSError:
        return False


def convert_file(src: Path, out_dir: Path | None, overwrite: bool, used_dests: set[str]) -> tuple[Path, str]:
    """Convert a single file and return (output_path, status_message)."""
    ext = src.suffix.lower()
    converter = CONVERTERS.get(ext)
    if not converter:
        return src, "SKIP (no converter)"

    dest_dir = out_dir if out_dir else src.parent
    base_dest = dest_dir / (src.stem + ".md")
    path_hash = get_path_hash(src)
    collision_index = 0

    while True:
        if collision_index == 0:
            dest = base_dest
        elif collision_index == 1:
            dest = dest_dir / f"{src.stem}_{path_hash}.md"
        else:
            dest = dest_dir / f"{src.stem}_{path_hash}_{collision_index - 1}.md"

        dest_key = str(dest.resolve().as_posix())

        if dest_key in used_dests:
            collision_index += 1
            continue

        if not dest.exists():
            break

        if is_same_source_markdown(dest, src):
            if not overwrite:
                used_dests.add(dest_key)
                return dest, "SKIP (exists)"
            break

        collision_index += 1

    dest_dir.mkdir(parents=True, exist_ok=True)

    # Performance logging for large files (e.g. pptx > 5MB)
    size_mb = src.stat().st_size / (1024 * 1024)
    if size_mb > 5:
        print(f"       ⌛ Converting large file ({size_mb:.1f} MB)... this may take a moment.")

    content = converter(src)
    header = f"# {src.name}\n\n> Source: `{src}`  \n> Converted at: {_now()}\n\n---\n\n"
    dest.write_text(header + content, encoding="utf-8")
    used_dests.add(dest_key)
    return dest, "OK"


def _now() -> str:
    from datetime import datetime
    return datetime.now().strftime("%Y-%m-%d %H:%M")


def main():
    parser = argparse.ArgumentParser(description="Binary document to Markdown converter")
    parser.add_argument("--root", default=".", help="Root directory to scan")
    parser.add_argument("--out", default=None, help="Output directory (defaults to the source file location)")
    parser.add_argument("--formats", nargs="+", default=["pdf", "pptx", "docx", "xlsx"])
    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    out_dir = Path(args.out).resolve() if args.out else None

    files = find_binary_files(root, args.formats)
    print(f"Found files: {len(files)}\n")

    if args.dry_run:
        for f in files:
            print(f"  {f.relative_to(root)}")
        return

    results = {"OK": 0, "SKIP (exists)": 0, "SKIP (no converter)": 0, "ERROR": 0}
    used_dests = set()
    for src in files:
        try:
            dest, status = convert_file(src, out_dir, args.overwrite, used_dests)
            results[status] = results.get(status, 0) + 1
            icon = "✅" if status == "OK" else "⏭️"
            print(f"  {icon} [{status}] {src.relative_to(root)}")
            if status == "OK":
                print(f"       → {dest}")
        except Exception as e:
            results["ERROR"] += 1
            print(f"  ❌ [ERROR] {src.relative_to(root)}: {e}")

    print(f"\nDone: {results}")


if __name__ == "__main__":
    main()
