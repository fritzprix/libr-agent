#!/usr/bin/env python3
"""Diagnostic check for worker-loop pre-requisites.

Checks:
  - Network connectivity to GitHub
  - gh CLI installed and authenticated
  - git installed and remote access working
  - python3 version >= 3.8
  - pnpm installed
  - Rust toolchain available (rustc)
  - Push permission to remote repository

Exit codes:
  0 - All checks passed
  1 - One or more critical checks failed
"""
from __future__ import annotations

import subprocess
import sys
import urllib.request
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


def check_network() -> bool:
    try:
        req = urllib.request.Request(
            "https://api.github.com",
            headers={"User-Agent": "LibrAgent-Doctor"}
        )
        with urllib.request.urlopen(req, timeout=5) as resp:
            if resp.status == 200:
                print("  [OK] Network: GitHub API reachable")
                return True
            print(f"  [FAIL] Network: HTTP status {resp.status}")
            return False
    except Exception as e:
        print(f"  [FAIL] Network unreachable: {e}")
        return False


def check_push_permission() -> bool:
    try:
        res = subprocess.run(
            ["git", "push", "--dry-run", "origin", "HEAD"],
            capture_output=True, text=True, check=False
        )
        if res.returncode == 0:
            print("  [OK] Push permission (dry-run passed)")
            return True
        err = res.stderr.strip()
        # If rejected solely because the current branch is not fast-forward, push permission is valid
        if "non-fast-forward" in err or ("[rejected]" in err and "fetch first" in err):
            print("  [OK] Push permission (remote accessible, non-fast-forward rejected)")
            return True
        print(f"  [FAIL] Push permission failed: {err}")
        return False
    except Exception as e:
        print(f"  [FAIL] Push permission check exception: {e}")
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

    # 2. Network connectivity
    all_ok &= check_network()

    # 3. Git
    all_ok &= check("git", ["git", "--version"])

    # 4. GitHub CLI authentication
    all_ok &= check("gh auth", ["gh", "auth", "status"])

    # 5. pnpm
    all_ok &= check("pnpm", ["pnpm", "--version"])

    # 6. Rust toolchain
    all_ok &= check("rustc", ["rustc", "--version"])

    # 7. Remote push permission test (dry-run)
    all_ok &= check_push_permission()

    print("\nSummary: " + ("All checks passed." if all_ok else "Some checks failed."))
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())
