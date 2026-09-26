#!/usr/bin/env python3
"""Verify a MediaAssist plugin directory before media__deployAssistPlugin."""
from __future__ import annotations

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import wave
from pathlib import Path


def fail(msg: str, code: int = 1) -> None:
    print(f"[verify] FAIL: {msg}", file=sys.stderr)
    raise SystemExit(code)


def resolve_run_executable(root: Path) -> Path:
    names = (
        ("run.exe", "run.cmd", "run.bat")
        if sys.platform == "win32"
        else ("run",)
    )
    for name in names:
        candidate = root / name
        if candidate.is_file():
            return candidate
    fail(
        "missing run executable "
        f"(looked for {', '.join(names)} under {root})"
    )
    raise AssertionError("unreachable")


def write_modality_fixture(modality: str, path: Path) -> str:
    """Write a tiny modality-appropriate fixture; return mimeType."""
    modality = modality.lower()
    if modality == "audio":
        with wave.open(str(path), "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(8000)
            wav.writeframes(b"\x00\x00" * 160)  # 20ms silence
        return "audio/wav"
    if modality == "image":
        # Minimal 1x1 PNG
        png = bytes.fromhex(
            "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489"
            "0000000a49444154789c63000100000500010d0a2db40000000049454e44ae426082"
        )
        path.write_bytes(png)
        return "image/png"
    if modality == "video":
        bundled = Path(__file__).resolve().parent / "fixtures" / "minimal.mp4"
        if bundled.is_file():
            shutil.copyfile(bundled, path)
            return "video/mp4"
        fail(
            "missing scripts/fixtures/minimal.mp4 next to verify_plugin.py "
            "(needed for a valid 1-frame MP4 video fixture)"
        )
    path.write_bytes(b"media-assist-verify-fixture\n")
    return "application/octet-stream"


def run_plugin_once(
    run_path: Path,
    root: Path,
    modality: str,
    mime_type: str,
    fixture_path: Path,
    timeout_ms: int,
) -> dict:
    req = {
        "interfaceVersion": 1,
        "modality": modality,
        "mimeType": mime_type,
        "path": str(fixture_path),
        "maxOutputChars": 2000,
    }
    run_suffix = run_path.suffix.lower()
    if run_suffix in {".cmd", ".bat"}:
        cmd = ["cmd", "/c", str(run_path)]
    else:
        cmd = [str(run_path)]
    proc = subprocess.run(
        cmd,
        input=json.dumps(req).encode("utf-8"),
        capture_output=True,
        timeout=timeout_ms / 1000.0,
        cwd=str(root),
        check=False,
    )
    if proc.returncode != 0:
        fail(
            f"run exited {proc.returncode} for modality={modality}: "
            f"{proc.stderr.decode('utf-8', errors='replace')[:500]}"
        )
    try:
        out = json.loads(proc.stdout.decode("utf-8"))
    except json.JSONDecodeError as exc:
        fail(
            f"run stdout not JSON for modality={modality}: {exc}; "
            f"stdout={proc.stdout[:300]!r}"
        )
    if not isinstance(out, dict):
        fail(f"run stdout must be a JSON object for modality={modality}")
    if not isinstance(out.get("ok"), bool):
        fail(
            f"run must return boolean ok for modality={modality}; "
            f"got {type(out.get('ok')).__name__}"
        )
    if not out["ok"]:
        fail(f"run returned ok=false for modality={modality}: {out}")
    text = out.get("text")
    if not isinstance(text, str) or not text.strip():
        fail(f"run returned empty/non-string text for modality={modality}")
    return out


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage: verify_plugin.py <plugin-dir>", 2)
    root = Path(sys.argv[1]).resolve()
    manifest_path = root / "manifest.json"
    if not manifest_path.is_file():
        fail(f"missing {manifest_path}")
    run_path = resolve_run_executable(root)

    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        fail(f"manifest.json invalid JSON: {exc}")

    if not isinstance(manifest, dict):
        fail("manifest.json must be a JSON object")
    if manifest.get("interfaceVersion") != 1:
        fail("interfaceVersion must be 1")
    name = manifest.get("name")
    if not isinstance(name, str) or not name.strip():
        fail("manifest name must be a non-empty string")
    modalities = manifest.get("modalities") or []
    if not isinstance(modalities, list) or not modalities:
        fail("modalities must be a non-empty list")
    normalized = []
    for item in modalities:
        if not isinstance(item, str) or not item.strip():
            fail("each modalities entry must be a non-empty string")
        normalized.append(item.strip().lower())

    if sys.platform != "win32":
        mode = run_path.stat().st_mode
        if not (mode & stat.S_IXUSR):
            os.chmod(run_path, mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)

    timeout_ms = int(manifest.get("timeoutMs") or 120000)
    samples: list[dict] = []
    for modality in normalized:
        suffix = {
            "audio": ".wav",
            "image": ".png",
            "video": ".mp4",
        }.get(modality, ".bin")
        with tempfile.NamedTemporaryFile(suffix=suffix, delete=False) as tmp:
            tmp_path = Path(tmp.name)
        try:
            mime_type = write_modality_fixture(modality, tmp_path)
            out = run_plugin_once(
                run_path, root, modality, mime_type, tmp_path, timeout_ms
            )
            samples.append(
                {
                    "modality": modality,
                    "fixtureMimeType": mime_type,
                    "sampleTextChars": len(out["text"].strip()),
                }
            )
        finally:
            tmp_path.unlink(missing_ok=True)

    print("[verify] OK")
    print(
        json.dumps(
            {
                "name": name.strip(),
                "modalities": normalized,
                "samples": samples,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
