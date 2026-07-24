#!/usr/bin/env python3
"""Run Harbor CLI with a short CACHE_DIR on Windows (MAX_PATH workaround).

Harbor always nests packages under ~/.cache/harbor/tasks/packages/... Some
harbor-index tasks unpack trees whose full path exceeds the classic Windows
260-character limit even when USERPROFILE is short (e.g. C:\\h).

This wrapper patches harbor.constants *before* the CLI imports other modules
so PACKAGE_CACHE_DIR becomes a short root such as C:\\p.

Env:
  LIBRAGENT_HARBOR_CACHE  Root directory (default: C:\\p on Windows).
"""

from __future__ import annotations

import os
import sys
from pathlib import Path


def _patch_cache_dirs() -> Path:
    root = Path(os.environ.get("LIBRAGENT_HARBOR_CACHE", r"C:\p")).expanduser()
    root = root.resolve()
    root.mkdir(parents=True, exist_ok=True)

    import harbor.constants as constants

    constants.CACHE_DIR = root
    # Keep names short: default layout is root/tasks/packages/... which wastes
    # path budget on deep dbt fixtures.
    constants.TASK_CACHE_DIR = root / "t"
    constants.PACKAGE_CACHE_DIR = root
    constants.DATASET_CACHE_DIR = root / "d"
    constants.NOTIFICATIONS_PATH = root / "n.json"
    return root


def main() -> int:
    if sys.platform == "win32":
        root = _patch_cache_dirs()
        print(
            f"Harbor cache patched to {root} "
            f"(PACKAGE_CACHE_DIR=root; set LIBRAGENT_HARBOR_CACHE to override)",
            file=sys.stderr,
            flush=True,
        )

    # Match console_scripts entry: harbor.cli.main:app
    from harbor.cli.main import app

    # Typer/Click apps treat sys.argv[0] as the program name.
    sys.argv = ["harbor", *sys.argv[1:]]
    try:
        app()
    except SystemExit as exc:
        code = exc.code
        if code is None:
            return 0
        if isinstance(code, int):
            return code
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
