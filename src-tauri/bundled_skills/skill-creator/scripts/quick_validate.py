#!/usr/bin/env python3
"""Backward-compatible alias for validate_skill.py (used by package_skill.py)."""

from validate_skill import main, validate_skill

if __name__ == "__main__":
    raise SystemExit(main())
