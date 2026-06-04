#!/usr/bin/env python3
import subprocess
import sys

def install_deps():
    print("Installing repo-wiki dependencies...")
    try:
        subprocess.check_call([sys.executable, "-m", "pip", "install", "pymupdf", "python-docx", "python-pptx", "openpyxl"])
        print("Successfully installed dependencies!")
    except Exception as e:
        print(f"Error installing dependencies: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    install_deps()
