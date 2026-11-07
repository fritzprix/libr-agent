from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
SCRIPTS = (
    REPO
    / "src-tauri/bundled_skills/release-head-self-e2e/scripts/pin_release_range.py"
)


def _run_pin(args: list[str]) -> dict:
    proc = subprocess.run(
        [sys.executable, str(SCRIPTS), *args],
        cwd=REPO,
        check=False,
        capture_output=True,
        text=True,
    )
    assert proc.stdout.strip(), proc.stderr
    return json.loads(proc.stdout)


def test_pin_release_range_json_has_head_sha():
    data = _run_pin(["--repo", str(REPO), "--json"])
    assert data["headSha"]
    assert len(data["headSha"]) >= 7
    if data["error"] is None:
        assert data["baselineTag"]
        assert data["baselineSha"]
        assert data["baselineIsAncestorOfHead"] is True
    else:
        assert data["error"] == "no_release_tag_ancestor_of_head"


def test_prerelease_tags_skipped_by_default():
    data = _run_pin(["--repo", str(REPO), "--json"])
    # If any skipped tags exist, they should match prerelease pattern
    for tag in data.get("skippedPrereleaseTags") or []:
        assert any(x in tag.lower() for x in ("alpha", "beta", "rc", "dev", "preview"))


def test_map_diff_script_runs():
    pin = _run_pin(["--repo", str(REPO), "--json"])
    if pin.get("error"):
        pytest.skip("no release tag ancestor in this checkout")
    map_script = (
        REPO
        / "src-tauri/bundled_skills/release-head-self-e2e/scripts/map_diff_to_cases.py"
    )
    manifest = (
        REPO / "benchmarks/harbor/libragent-session-e2e-matrix/coverage-manifest.json"
    )
    proc = subprocess.run(
        [
            sys.executable,
            str(map_script),
            "--repo",
            str(REPO),
            "--baseline",
            pin["baselineTag"],
            "--manifest",
            str(manifest),
            "--json",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    data = json.loads(proc.stdout)
    assert "changedFiles" in data
    assert "caseHints" in data


def test_bundled_skill_file_exists():
    skill = REPO / "src-tauri/bundled_skills/release-head-self-e2e/SKILL.md"
    assert skill.is_file()
    text = skill.read_text(encoding="utf-8")
    assert "name: release-head-self-e2e" in text
    assert "LIBRAGENT_E2E_CONTROL" in text
    assert "storeAs" in text
