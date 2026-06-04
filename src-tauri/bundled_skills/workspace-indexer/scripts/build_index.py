#!/usr/bin/env python3
"""
Workspace Index Builder
========================
Scans the workspace to generate index.md (file index + keyword map).

Usage:
    python build_index.py [--root ROOT_DIR] [--out OUTPUT_MD] [--keywords KEYWORDS_FILE]
                          [--ignore-dirs DIR1 DIR2 ...] [--dry-run]

Options:
    --root          Root directory to scan (default: current directory)
    --out           Output index.md path (default: ROOT/index.md)
    --keywords      Keywords file (.txt, one keyword per line). If omitted, extracted automatically.
    --ignore-dirs   Directories to ignore (default: .git __pycache__ node_modules .github)
    --dry-run       Print files and keywords only, do not save to file
    --max-keyword-files  Maximum number of files per keyword (default: 20)

Features:
    - Lists all text files (md, txt, py, etc.) and binary files in the workspace
    - Automatically extracts keywords from H1-H3 headings in Markdown files
    - Generates a map of files containing each keyword (keyword map)
    - Generates directory-based file trees
    - Separates binary documents into a dedicated section

Examples:
    python build_index.py --root . --out index.md
    python build_index.py --root . --keywords my_keywords.txt
    python build_index.py --root . --dry-run
"""

import argparse
import io
import re
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path

# Prevent UnicodeEncodeError on Windows due to cp949 encoding
if sys.platform.startswith("win"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")

# Add scripts directory to sys.path for Cwd-independent imports
sys.path.append(str(Path(__file__).resolve().parent))
from utils import get_source_from_md

# Default directories to ignore
DEFAULT_SKIP_DIRS = {".git", "__pycache__", "node_modules", ".github", "venv", ".venv", ".mypy_cache", ".libragent", "attachments"}

# Extensions to treat as text
TEXT_EXTS = {".md", ".txt", ".py", ".sh", ".json", ".yaml", ".yml", ".toml", ".csv", ".html", ".js", ".ts"}

# Binary document extensions
BINARY_DOC_EXTS = {".pdf", ".docx", ".pptx", ".xlsx", ".xls", ".ppt", ".doc"}

# Extensions to include in the index (total)
INDEX_EXTS = TEXT_EXTS | BINARY_DOC_EXTS | {".ipynb"}


def collect_files(root: Path, skip_dirs: set[str]) -> tuple[list[Path], list[Path]]:
    """Separate text files and binary files and return them."""
    text_files, binary_files = [], []
    for p in sorted(root.rglob("*")):
        if p.is_dir():
            continue
        if any(part in skip_dirs for part in p.relative_to(root).parts):
            continue
        ext = p.suffix.lower()
        if ext not in INDEX_EXTS:
            continue
        if ext in BINARY_DOC_EXTS:
            binary_files.append(p)
        else:
            text_files.append(p)
    return text_files, binary_files


def extract_headings(path: Path) -> list[str]:
    """Extract heading text (H1-H3) from markdown file."""
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except Exception:
        return []
    headings = []
    for line in text.splitlines():
        m = re.match(r"^#{1,3}\s+(.+)", line)
        if m:
            heading = m.group(1).strip()
            # Remove markdown links and inline code formatting
            heading = re.sub(r"\[([^\]]+)\]\([^\)]+\)", r"\1", heading)
            heading = re.sub(r"`[^`]+`", "", heading)
            heading = heading.strip()
            if heading:
                headings.append(heading)
    return headings


def extract_keywords_from_files(text_files: list[Path], root: Path) -> dict[str, list[str]]:
    """
    Extract keywords (headings) from text files.
    Returns: {keyword: [rel_path1, rel_path2, ...]}
    """
    keyword_map: dict[str, set[str]] = defaultdict(set)
    for p in text_files:
        if p.suffix.lower() != ".md":
            continue
        headings = extract_headings(p)
        rel = str(p.relative_to(root).as_posix())
        for h in headings:
            keyword_map[h].add(rel)
            # Add word-level tokens as keywords to improve Korean/English mapping and search
            words = re.findall(r'[a-zA-Z0-9가-힣]+', h)
            for w in words:
                if len(w) >= 2 and w != h:
                    keyword_map[w].add(rel)
    # Convert to list and sort to guarantee consistent output (maintaining O(n) performance)
    return {kw: sorted(list(files)) for kw, files in keyword_map.items()}


def load_custom_keywords(keyword_file: Path, text_files: list[Path], root: Path) -> dict[str, list[str]]:
    """Read keywords from custom keywords file and map them if they appear in files."""
    keywords = [line.strip() for line in keyword_file.read_text(encoding="utf-8").splitlines() if line.strip()]
    keyword_map: dict[str, list[str]] = defaultdict(list)
    for p in text_files:
        try:
            content = p.read_text(encoding="utf-8", errors="ignore").lower()
        except Exception:
            continue
        rel = str(p.relative_to(root).as_posix())
        for kw in keywords:
            if kw.lower() in content:
                keyword_map[kw].append(rel)
    return dict(keyword_map)


def build_dir_tree(all_files: list[Path], root: Path) -> dict[str, list[Path]]:
    """Group files by directory. Uses POSIX path keys to maintain OS-independent consistency."""
    tree: dict[str, list[Path]] = defaultdict(list)
    for p in all_files:
        rel = p.relative_to(root)
        parts = rel.parts
        # Use as_posix() because str(Path(...)) returns backslashes on Windows
        dir_key = Path(*parts[:-1]).as_posix() if len(parts) > 1 else "."
        tree[dir_key].append(p)
    return dict(tree)


def render_index(
    root: Path,
    text_files: list[Path],
    binary_files: list[Path],
    keyword_map: dict[str, list[str]],
    max_keyword_files: int,
) -> str:
    now = datetime.now().strftime("%Y-%m-%d %H:%M")
    lines = [
        f"# Workspace Index",
        f"",
        f"> Generated at: {now}  ",
        f"> Root: `{root}`  ",
        f"> Text Files: **{len(text_files)}** | Binary Documents: **{len(binary_files)}**",
        f"",
        f"---",
        f"",
        f"## Table of Contents",
        f"",
        f"1. [File Index (Text)](#file-index-text)",
        f"2. [Binary Documents](#binary-documents)",
        f"3. [Keyword Map](#keyword-map)",
        f"",
        f"---",
        f"",
    ]

    # --- Text File Index ---
    lines += [f"## File Index (Text)", f""]
    tree = build_dir_tree(text_files, root)
    for dir_key in sorted(tree.keys()):
        if dir_key == ".":
            lines.append(f"### 📁 (Root)")
        else:
            lines.append(f"### 📁 {dir_key}")
        for p in sorted(tree[dir_key]):
            rel = str(p.relative_to(root).as_posix())
            size_kb = p.stat().st_size / 1024
            lines.append(f"- [`{p.name}`]({rel}) ({size_kb:.1f} KB)")
        lines.append("")

    # --- Binary Documents ---
    lines += [f"---", f"", f"## Binary Documents", f""]
    if binary_files:
        # Read the Source header of markdown files to build a mapping to binary files
        src_to_md = {}
        for p_md in text_files:
            if p_md.suffix.lower() == ".md":
                source_path = get_source_from_md(p_md)
                if source_path:
                    try:
                        resolved_src = str(Path(source_path).resolve().as_posix())
                        src_to_md[resolved_src] = str(p_md.relative_to(root).as_posix())
                    except Exception:
                        pass

        lines.append("| Filename | Path | Size | Converted File |")
        lines.append("| --- | --- | --- | --- |")
        for p in sorted(binary_files):
            rel = str(p.relative_to(root).as_posix())
            size_kb = p.stat().st_size / 1024
            resolved_p = str(p.resolve().as_posix())

            # Look up in source map first; if not found, fall back to default path (.md)
            md_rel = src_to_md.get(resolved_p)
            if not md_rel:
                md_path = p.with_suffix(".md")
                if md_path.exists():
                    md_rel = str(md_path.relative_to(root).as_posix())

            converted = f"[MD]({md_rel})" if md_rel else "❌ Non-converted"
            lines.append(f"| `{p.name}` | `{rel}` | {size_kb:.1f} KB | {converted} |")
    else:
        lines.append("_(No binary documents)_")
    lines.append("")

    # --- Keyword Map ---
    lines += [f"---", f"", f"## Keyword Map", f""]
    if keyword_map:
        lines.append(f"Total **{len(keyword_map)}** keywords")
        lines.append("")
        for kw in sorted(keyword_map.keys(), key=lambda x: x.lower()):
            file_list = keyword_map[kw][:max_keyword_files]
            overflow = len(keyword_map[kw]) - max_keyword_files
            lines.append(f"### {kw}")
            for rel in file_list:
                lines.append(f"- [`{Path(rel).name}`]({rel})")
            if overflow > 0:
                lines.append(f"- _and {overflow} more files_")
            lines.append("")
    else:
        lines.append("_(No keywords)_")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Workspace Index Builder")
    parser.add_argument("--root", default=".", help="Scan root directory")
    parser.add_argument("--out", default=None, help="Output index.md path")
    parser.add_argument("--keywords", default=None, help="Custom keywords file (.txt)")
    parser.add_argument("--ignore-dirs", nargs="+", default=[], help="Additional directories to ignore")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--max-keyword-files", type=int, default=20)
    args = parser.parse_args()

    root = Path(args.root).resolve()
    skip_dirs = DEFAULT_SKIP_DIRS | set(args.ignore_dirs)
    out_path = Path(args.out).resolve() if args.out else root / "index.md"

    print(f"🔍 Scanning: {root}")
    text_files, binary_files = collect_files(root, skip_dirs)
    print(f"   Text: {len(text_files)} | Binary: {len(binary_files)}")

    if args.keywords:
        kw_file = Path(args.keywords)
        if not kw_file.exists():
            print(f"❌ Keywords file not found: {kw_file}", file=sys.stderr)
            sys.exit(1)
        keyword_map = load_custom_keywords(kw_file, text_files, root)
        print(f"   Custom keywords: {len(keyword_map)} mapped")
    else:
        keyword_map = extract_keywords_from_files(text_files, root)
        print(f"   Auto-extracted keywords: {len(keyword_map)}")

    if args.dry_run:
        print("\n=== Text Files ===")
        for f in text_files[:20]:
            print(f"  {f.relative_to(root)}")
        if len(text_files) > 20:
            print(f"  ... and {len(text_files)-20} more")
        print("\n=== Binary Files ===")
        for f in binary_files[:20]:
            print(f"  {f.relative_to(root)}")
        print("\n=== Keyword Samples (Top 20) ===")
        for kw in list(sorted(keyword_map.keys()))[:20]:
            print(f"  '{kw}': {len(keyword_map[kw])} files")
        return

    content = render_index(root, text_files, binary_files, keyword_map, args.max_keyword_files)
    out_path.write_text(content, encoding="utf-8")
    print(f"\n✅ Saved to: {out_path}")
    print(f"   Size: {len(content):,} bytes")


if __name__ == "__main__":
    main()
