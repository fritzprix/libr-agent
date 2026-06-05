#!/usr/bin/env python3
"""
Dependency Installer for workspace-indexer
==========================================
Checks for required Python packages and installs them if missing.

Usage:
    python install_deps.py           # Install all dependencies
    python install_deps.py --check   # Only check if installed (do not install)
    python install_deps.py --fmt pdf docx  # Only install packages for specific formats
"""

import argparse
import importlib
import io
import subprocess
import sys

# Prevent UnicodeEncodeError on Windows due to cp949 encoding
if sys.platform.startswith("win"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")

# Required packages per file format
FORMAT_DEPS: dict[str, list[tuple[str, str]]] = {
    "pdf":  [("fitz", "pymupdf")],
    "docx": [("docx", "python-docx")],
    "pptx": [("pptx", "python-pptx")],
    "xlsx": [("openpyxl", "openpyxl")],
}

ALL_FORMATS = list(FORMAT_DEPS.keys())


def check_package(import_name: str) -> bool:
    """Check if a package can be imported."""
    try:
        importlib.import_module(import_name)
        return True
    except ImportError:
        return False


def install_package(pip_name: str) -> bool:
    """Install package via pip. Return True on success."""
    result = subprocess.run(
        [sys.executable, "-m", "pip", "install", pip_name],
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def main():
    parser = argparse.ArgumentParser(description="workspace-indexer dependency installer")
    parser.add_argument("--check", action="store_true", help="Check only (do not install)")
    parser.add_argument(
        "--fmt", nargs="+", choices=ALL_FORMATS,
        default=ALL_FORMATS,
        help="Process specific formats only (default: all)",
    )
    args = parser.parse_args()

    print("=== Checking workspace-indexer dependencies ===\n")

    all_ok = True
    for fmt in args.fmt:
        for import_name, pip_name in FORMAT_DEPS[fmt]:
            installed = check_package(import_name)
            status = "✅ Installed" if installed else "❌ Missing"
            print(f"  [{fmt:4s}] {pip_name:15s}  {status}")

            if not installed:
                all_ok = False
                if not args.check:
                    print(f"         → Running pip install {pip_name}...")
                    ok = install_package(pip_name)
                    if ok:
                        print(f"         ✅ Installation completed")
                    else:
                        print(f"         ❌ Installation failed — please run manually: pip install {pip_name}")

    print()
    if all_ok:
        print("✅ All dependencies are met.")
    elif args.check:
        print("⚠️  Some dependencies are missing.")
        print("   To install: python install_deps.py")
        sys.exit(1)
    else:
        # Re-check after installation
        still_missing = [
            pip_name
            for fmt in args.fmt
            for import_name, pip_name in FORMAT_DEPS[fmt]
            if not check_package(import_name)
        ]
        if still_missing:
            print(f"❌ Failed to install packages: {', '.join(still_missing)}")
            sys.exit(1)
        else:
            print("✅ All dependencies installed successfully.")


if __name__ == "__main__":
    main()
