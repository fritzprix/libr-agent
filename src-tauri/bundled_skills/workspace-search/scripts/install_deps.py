#!/usr/bin/env python3
"""
Dependency Installer for workspace-search
=========================================
Checks for optional and required packages (like sentence-transformers)
and installs them.

Usage:
    python install_deps.py           # Install dependencies
    python install_deps.py --check   # Only check if installed
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

REQUIRED_PACKAGES: list[tuple[str, str, str]] = [
    ("sentence_transformers", "sentence-transformers", "Required only for local offline semantic embeddings"),
]


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
    parser = argparse.ArgumentParser(description="workspace-search dependency installer")
    parser.add_argument("--check", action="store_true", help="Check only (do not install)")
    args = parser.parse_args()

    print("=== Checking workspace-search dependencies ===\n")
    print("Note: Hybrid search natively implements BM25 and API embeddings (OpenAI, Gemini, Ollama) with ZERO dependencies.")
    print("Local offline embeddings require sentence-transformers.\n")

    all_ok = True
    for import_name, pip_name, desc in REQUIRED_PACKAGES:
        installed = check_package(import_name)
        status = "✅ Installed" if installed else "❌ Missing"
        print(f"  {pip_name:25s}  {status}  ({desc})")

        if not installed:
            all_ok = False
            if not args.check:
                print(f"         → Running pip install {pip_name}...")
                ok = install_package(pip_name)
                if ok:
                    print("         ✅ Installation completed")
                    all_ok = True
                else:
                    print(
                        f'         ❌ Installation failed — please run manually: pip install "{pip_name}"'
                    )

    print()
    if all_ok:
        print("✅ Dependencies are met (or optional local packages are ready).")
    elif args.check:
        print("⚠️  Optional dependencies for offline local embedding are missing.")
        print("   To install: python install_deps.py")
        sys.exit(1)
    else:
        still_missing = [
            pip_name
            for import_name, pip_name, _ in REQUIRED_PACKAGES
            if not check_package(import_name)
        ]
        if still_missing:
            print(f"❌ Failed to install packages: {', '.join(still_missing)}")
            sys.exit(1)
        else:
            print("✅ All dependencies installed successfully.")


if __name__ == "__main__":
    main()
