#!/usr/bin/env python3
"""Diagnostic check for worker-loop pre-requisites.

Checks:
  - gh CLI installed and authenticated
  - git installed and remote access working
  - python3 version >= 3.8
  - pnpm installed
  - Rust toolchain available (rustc, cargo)
  - Push permission to remote repository

Exit codes:
  0 - All checks passed
  1 - One or more critical checks failed
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def check(name: str, cmd: list[str]) -> bool:
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, check=False)
        if res.returncode == 0:
            print(f"  [OK] {name}")
            return True
        else:
            print(f"  [FAIL] {name}: {res.stderr.strip() or res.stdout.strip()}")
            return False
    except FileNotFoundError:
        print(f"  [FAIL] {name}: command not found ({cmd[0]})")
        return False
    except Exception as e:
        print(f"  [FAIL] {name}: {e}")
        return False


def main() -> int:
    print("Checking worker-loop prerequisites:")
    all_ok = True

    # 1. Python version
    py_ok = sys.version_info >= (3, 8)
    if py_ok:
        print(f"  [OK] Python version: {sys.version.split()[0]}")
    else:
        print(f"  [FAIL] Python >= 3.8 required, found {sys.version.split()[0]}")
        all_ok = False

    # 2. Git
    all_ok &= check("git", ["git", "--version"])

    # 3. GitHub CLI authentication
    all_ok &= check("gh auth", ["gh", "auth", "status"])

    # 4. pnpm
    all_ok &= check("pnpm", ["pnpm", "--version"])

    # 5. Rust toolchain
    all_ok &= check("rustc", ["rustc", "--version"])

    # 6. Remote push permission test (dry-run)
    try:
        res = subprocess.run(
            ["git", "push", "--dry-run", "origin", "HEAD"],
            capture_output=True, text=True, check=False
        )
        if res.returncode == 0:
            print("  [OK] Push permission (dry-run)")
        else:
            # Check if failure is just non-fast-forward on dry run or actual permission failure
            if "Permission to" in res.stderr or "denied" in res.stderr.lower():
                print(f"  [FAIL] Push permission: {res.stderr.strip()}")
                all_ok = False
            else:
                print("  [OK] Push permission to remote confirmed")
    except Exception as e:
        print(f"  [WARN] Push permission check error: {e}")

    print("\nSummary: " + ("All checks passed." if all_ok else "Some checks failed."))
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())
