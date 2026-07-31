#!/usr/bin/env python3
"""
Workspace Indexer — Unified Runner
====================================
Unified entry point to run binary conversion and build index.md in one step.

Usage:
    python run.py [--root ROOT] [--out OUT] [--formats pdf pptx docx xlsx]
                  [--overwrite] [--skip-convert] [--skip-index]
                  [--index-out INDEX_MD] [--keywords KEYWORDS_FILE]
                  [--ignore-dirs DIR1 DIR2] [--dry-run]

Main Options:
    --root          Workspace root (default: current directory)
    --out           Converted files output directory (default: same as source file)
    --formats       Formats to convert (default: pdf pptx docx xlsx)
    --overwrite     Overwrite existing converted files
    --skip-convert  Skip binary conversion (refresh index only)
    --skip-index    Skip index.md generation (perform conversion only)
    --index-out     index.md output path (default: ROOT/index.md)
    --keywords      Custom keywords file (.txt)
    --ignore-dirs   Additional directories to ignore
    --dry-run       Simulate operations without saving files

Examples:
    # Full workflow
    python run.py --root .

    # Conversion only (PDF/DOCX)
    python run.py --root . --skip-index --formats pdf docx

    # Index refresh only
    python run.py --root . --skip-convert

    # Dry-run preview
    python run.py --root . --dry-run
"""

import argparse
import io
import os
import subprocess
import sys
from pathlib import Path

# Prevent UnicodeEncodeError on Windows due to cp949 encoding
if sys.platform.startswith("win"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")

SCRIPT_DIR = Path(__file__).parent
CONVERT_SCRIPT = SCRIPT_DIR / "convert_binary_docs.py"
INDEX_SCRIPT = SCRIPT_DIR / "build_index.py"


def run_step(label: str, cmd: list[str]) -> bool:
    """Run script in subprocess. Return True on success."""
    print(f"\n{'='*60}")
    print(f"  {label}")
    print(f"{'='*60}")

    env = os.environ.copy()
    if sys.platform.startswith("win"):
        # Force UTF-8 encoding for subprocesses on Windows
        env["PYTHONIOENCODING"] = "utf-8"

    result = subprocess.run(cmd, env=env)
    if result.returncode != 0:
        print(f"\n❌ {label} failed (exit code {result.returncode})")
        return False
    return True


def build_convert_cmd(args) -> list[str]:
    cmd = [sys.executable, str(CONVERT_SCRIPT), "--root", args.root]
    if args.out:
        cmd += ["--out", args.out]
    if args.formats:
        cmd += ["--formats"] + args.formats
    if args.overwrite:
        cmd.append("--overwrite")
    if args.dry_run:
        cmd.append("--dry-run")
    return cmd


def build_index_cmd(args) -> list[str]:
    cmd = [sys.executable, str(INDEX_SCRIPT), "--root", args.root]
    if args.index_out:
        cmd += ["--out", args.index_out]
    if args.keywords:
        cmd += ["--keywords", args.keywords]
    if args.ignore_dirs:
        cmd += ["--ignore-dirs"] + args.ignore_dirs
    if args.dry_run:
        cmd.append("--dry-run")
    return cmd


def main():
    parser = argparse.ArgumentParser(
        description="workspace-indexer unified runner",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--root", default=".", help="Workspace root directory")
    parser.add_argument("--out", default=None, help="Output directory for converted files")
    parser.add_argument("--formats", nargs="+", default=["pdf", "pptx", "docx", "xlsx"])
    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--skip-convert", action="store_true", help="Skip binary conversion")
    parser.add_argument("--skip-index", action="store_true", help="Skip index.md generation")
    parser.add_argument("--index-out", default=None, help="Output path for index.md")
    parser.add_argument("--keywords", default=None, help="Custom keywords file")
    parser.add_argument("--ignore-dirs", nargs="+", default=[], help="Directories to ignore")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    print(f"🗂️  Workspace Indexer")
    print(f"   Root: {root}")
    print(f"   Mode: {'dry-run' if args.dry_run else 'live'}")
    steps = []
    if not args.skip_convert:
        steps.append("binary conversion")
    if not args.skip_index:
        steps.append("index.md building")
    print(f"   Steps: {' → '.join(steps) if steps else '(none)'}\n")

    success = True

    # Step 1: binary conversion
    if not args.skip_convert:
        cmd = build_convert_cmd(args)
        ok = run_step("Step 1: Binary Documents → Markdown Conversion", cmd)
        if not ok:
            success = False

    # Step 2: index.md building
    if not args.skip_index:
        cmd = build_index_cmd(args)
        ok = run_step("Step 2: Building index.md", cmd)
        if not ok:
            success = False

    # Final results
    print(f"\n{'='*60}")
    if success:
        if not args.dry_run:
            index_path = args.index_out or str(root / "index.md")
            print(f"✅ Completed!")
            if not args.skip_index:
                print(f"   index.md → {index_path}")
        else:
            print(f"✅ dry-run completed (no files saved)")
    else:
        print(f"⚠️  Errors occurred in some steps.")
        sys.exit(1)


if __name__ == "__main__":
    main()
