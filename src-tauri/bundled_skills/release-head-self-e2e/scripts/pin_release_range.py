#!/usr/bin/env python3
"""Pin HEAD and latest applicable release tag for release-head-self-e2e skill."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

_PRERELEASE = re.compile(r"(?:^|[.-])(alpha|beta|rc|dev|preview)(?:[.-]|$)", re.I)


@dataclass(frozen=True)
class PinResult:
    baseline_tag: str | None
    baseline_sha: str | None
    head_sha: str
    head_ref: str
    is_ancestor: bool
    error: str | None
    skipped_prerelease_tags: list[str]

    def to_json(self) -> dict:
        return {
            "baselineTag": self.baseline_tag,
            "baselineSha": self.baseline_sha,
            "headSha": self.head_sha,
            "headRef": self.head_ref,
            "baselineIsAncestorOfHead": self.is_ancestor,
            "error": self.error,
            "skippedPrereleaseTags": self.skipped_prerelease_tags,
        }


def _run(repo: Path, *args: str) -> str:
    proc = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=False,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or proc.stdout.strip() or "git failed")
    return proc.stdout.strip()


def _list_tags(repo: Path) -> list[str]:
    out = _run(repo, "tag", "--sort=-v:refname")
    return [line for line in out.splitlines() if line.strip()]


def _is_prerelease(tag: str) -> bool:
    return bool(_PRERELEASE.search(tag))


def _peel_tag(repo: Path, tag: str) -> str:
    try:
        return _run(repo, "rev-list", "-n", "1", tag)
    except RuntimeError:
        return _run(repo, "rev-parse", tag + "^{}")


def _is_ancestor(repo: Path, ancestor: str, head: str) -> bool:
    proc = subprocess.run(
        ["git", "-C", str(repo), "merge-base", "--is-ancestor", ancestor, head],
        check=False,
    )
    return proc.returncode == 0


def pin_release_range(
    repo: Path,
    *,
    include_prerelease: bool = False,
    head: str = "HEAD",
) -> PinResult:
    head_sha = _run(repo, "rev-parse", head)
    head_ref = head
    tags = _list_tags(repo)
    skipped: list[str] = []
    chosen_tag: str | None = None
    chosen_sha: str | None = None
    for tag in tags:
        if not include_prerelease and _is_prerelease(tag):
            skipped.append(tag)
            continue
        sha = _peel_tag(repo, tag)
        if _is_ancestor(repo, sha, head_sha):
            chosen_tag = tag
            chosen_sha = sha
            break
    if chosen_tag is None:
        return PinResult(
            baseline_tag=None,
            baseline_sha=None,
            head_sha=head_sha,
            head_ref=head_ref,
            is_ancestor=False,
            error="no_release_tag_ancestor_of_head",
            skipped_prerelease_tags=skipped,
        )
    return PinResult(
        baseline_tag=chosen_tag,
        baseline_sha=chosen_sha,
        head_sha=head_sha,
        head_ref=head_ref,
        is_ancestor=True,
        error=None,
        skipped_prerelease_tags=skipped,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--include-prerelease", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve()
    result = pin_release_range(
        repo, include_prerelease=args.include_prerelease, head=args.head
    )
    if args.json:
        print(json.dumps(result.to_json(), indent=2))
        return 0
    print(result)
    return 0 if result.error is None else 1


if __name__ == "__main__":
    sys.exit(main())
