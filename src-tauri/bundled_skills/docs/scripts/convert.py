#!/usr/bin/env python3
"""
LibrAgent docs skill conversion script.
Provides unified document conversion and dependency management.
"""

import argparse
import glob
import os
import pathlib
import shutil
import subprocess
import sys


SKILL_DIR = pathlib.Path(__file__).resolve().parent.parent
VENV_DIR = SKILL_DIR / ".venv"
FONTS_DIR = SKILL_DIR / "fonts"
REQUIRED_PACKAGES = ["reportlab", "python-docx", "python-pptx", "markitdown"]


def get_venv_python(venv_dir: pathlib.Path) -> pathlib.Path:
    """Return path to python binary inside a virtualenv."""
    if os.name == "nt":
        return venv_dir / "Scripts" / "python.exe"
    return venv_dir / "bin" / "python"


def is_working_python(candidate: str | pathlib.Path) -> bool:
    """Check if candidate python executable executes successfully."""
    try:
        proc = subprocess.run(
            [str(candidate), "-c", "import sys; sys.exit(0)"],
            capture_output=True,
            timeout=10,
        )
        return proc.returncode == 0
    except Exception:
        return False


def find_working_python() -> pathlib.Path | None:
    """Probe candidate Python interpreters to find a functional one.

    Discards any candidate that crashes (e.g., exit code -1073741515 / STATUS_DLL_NOT_FOUND).
    """
    candidates: list[str | pathlib.Path] = []

    # 1. uv python find
    uv_bin = shutil.which("uv")
    if not uv_bin and os.name == "nt":
        for possible_uv in [
            pathlib.Path(os.environ.get("LOCALAPPDATA", "")) / "bin" / "uv.exe",
            pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "uv.exe",
            pathlib.Path(os.environ.get("USERPROFILE", "")) / ".local" / "bin" / "uv.exe",
        ]:
            if possible_uv.exists():
                uv_bin = str(possible_uv)
                break

    if uv_bin:
        try:
            res = subprocess.run(
                [uv_bin, "python", "find"],
                capture_output=True,
                text=True,
                timeout=10,
            )
            if res.returncode == 0 and res.stdout.strip():
                candidates.append(res.stdout.strip().splitlines()[0].strip())
        except Exception:
            pass

    # 2. Existing local venv python
    local_venv_py = get_venv_python(VENV_DIR)
    if local_venv_py.exists():
        candidates.append(local_venv_py)

    # 3. Known Windows Python install locations
    if os.name == "nt":
        local_app_data = os.environ.get("LOCALAPPDATA", "")
        app_data = os.environ.get("APPDATA", "")
        user_profile = os.environ.get("USERPROFILE", "")
        prog_files = os.environ.get("ProgramFiles", "C:\\Program Files")

        for pattern in [
            os.path.join(local_app_data, "Programs", "Python", "Python3*", "python.exe"),
            os.path.join(app_data, "uv", "python", "*", "python.exe"),
            os.path.join(local_app_data, "uv", "python", "*", "python.exe"),
            os.path.join(user_profile, ".local", "bin", "python.exe"),
            os.path.join(prog_files, "Python3*", "python.exe"),
        ]:
            for p in glob.glob(pattern):
                candidates.append(pathlib.Path(p))

    # 4. py launcher (Windows)
    py_launcher = shutil.which("py")
    if py_launcher:
        try:
            res = subprocess.run(
                [py_launcher, "-3", "-c", "import sys; print(sys.executable)"],
                capture_output=True,
                text=True,
                timeout=10,
            )
            if res.returncode == 0 and res.stdout.strip():
                candidates.append(res.stdout.strip())
        except Exception:
            pass

    # 5. sys.executable
    if sys.executable:
        candidates.append(sys.executable)

    # 6. Standard PATH executables
    for name in ["python3", "python"]:
        found = shutil.which(name)
        if found:
            candidates.append(found)

    seen: set[str] = set()
    for cand in candidates:
        cand_str = str(cand)
        if cand_str in seen:
            continue
        seen.add(cand_str)
        if is_working_python(cand):
            return pathlib.Path(cand)

    return None


def run_doctor() -> int:
    """Run environment and dependency doctor check."""
    working_py = find_working_python()
    if not working_py:
        print(
            "Error: No functional Python interpreter found. Candidates on PATH failed to run.",
            file=sys.stderr,
        )
        print(
            "NEXT: Install Python 3.11+ or uv (run setup-wizard skill: 'uv python install').",
            file=sys.stderr,
        )
        return 1

    uv_bin = shutil.which("uv")
    if not uv_bin and os.name == "nt":
        for possible_uv in [
            pathlib.Path(os.environ.get("LOCALAPPDATA", "")) / "bin" / "uv.exe",
            pathlib.Path(os.environ.get("USERPROFILE", "")) / ".cargo" / "bin" / "uv.exe",
            pathlib.Path(os.environ.get("USERPROFILE", "")) / ".local" / "bin" / "uv.exe",
        ]:
            if possible_uv.exists():
                uv_bin = str(possible_uv)
                break

    # Setup skill-local venv
    venv_py = get_venv_python(VENV_DIR)
    if not venv_py.exists():
        VENV_DIR.parent.mkdir(parents=True, exist_ok=True)
        if uv_bin:
            cmd = [uv_bin, "venv", str(VENV_DIR), "--python", str(working_py)]
        else:
            cmd = [str(working_py), "-m", "venv", str(VENV_DIR)]

        proc = subprocess.run(cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            print(f"Error: Failed to create skill virtualenv: {proc.stderr}", file=sys.stderr)
            print(f"NEXT: Check permissions on '{SKILL_DIR}' and retry 'convert.py --doctor'.", file=sys.stderr)
            return 1

    venv_py = get_venv_python(VENV_DIR)
    if not is_working_python(venv_py):
        print(f"Error: Virtualenv python at '{venv_py}' is not functional.", file=sys.stderr)
        print("NEXT: Remove .venv directory and re-run 'convert.py --doctor'.", file=sys.stderr)
        return 1

    # Verify or install required packages
    missing_pkgs: list[str] = []
    for pkg in REQUIRED_PACKAGES:
        module_name = "docx" if pkg == "python-docx" else ("pptx" if pkg == "python-pptx" else pkg)
        check = subprocess.run(
            [str(venv_py), "-c", f"import {module_name}"],
            capture_output=True,
        )
        if check.returncode != 0:
            missing_pkgs.append(pkg)

    if missing_pkgs:
        if uv_bin:
            install_cmd = [uv_bin, "pip", "install", "--python", str(venv_py)] + missing_pkgs
        else:
            install_cmd = [str(venv_py), "-m", "pip", "install"] + missing_pkgs

        proc = subprocess.run(install_cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            print(
                f"Error: Failed to install dependencies into skill venv: {proc.stderr}",
                file=sys.stderr,
            )
            print(
                f"NEXT: Run 'uv pip install --python \"{venv_py}\" {' '.join(missing_pkgs)}' to install packages.",
                file=sys.stderr,
            )
            return 1

    # Check font
    font_file = None
    if FONTS_DIR.exists():
        for f in FONTS_DIR.glob("NotoSansKR-*.ttf"):
            font_file = f
            break

    if not font_file:
        print(f"Warning: Bundled Noto Sans KR font not found in '{FONTS_DIR}'.", file=sys.stderr)

    print("Doctor: OK")
    print(f"Python: {working_py}")
    print(f"Venv: {VENV_DIR}")
    print(f"VenvPython: {venv_py}")
    print(f"Dependencies: {' '.join(REQUIRED_PACKAGES)} installed")
    if font_file:
        print(f"Font: {font_file}")
    print("ExitCode: 0")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="LibrAgent docs conversion tool", add_help=False)
    parser.add_argument("--doctor", action="store_true", help="Check and configure environment dependencies")
    parser.add_argument("--in", dest="input_file", type=str, help="Source input file path")
    parser.add_argument("--to", dest="target_format", type=str, help="Target format (pdf, pptx, docx, md)")
    parser.add_argument("--template", dest="template", type=str, default="report", help="Template name")
    parser.add_argument("--out", dest="output_file", type=str, help="Target output file path")
    parser.add_argument("-h", "--help", action="store_true", help="Show help")

    args, unknown = parser.parse_known_args()

    if args.help:
        parser.print_help()
        return 0

    if args.doctor:
        return run_doctor()

    # Conversion validation
    if not args.input_file:
        print("Error: Missing required argument '--in'.", file=sys.stderr)
        print("NEXT: Specify input source path via --in '<abs-path>'.", file=sys.stderr)
        return 1

    input_path = pathlib.Path(args.input_file).resolve()
    if not input_path.exists():
        print(f"Error: Input file does not exist: {input_path}", file=sys.stderr)
        print(f"NEXT: Check input path and verify file exists at '{input_path}'.", file=sys.stderr)
        return 1

    if not args.target_format:
        print("Error: Missing required argument '--to'.", file=sys.stderr)
        print("NEXT: Specify target format via --to (pdf, pptx, docx, md).", file=sys.stderr)
        return 1

    target_format = args.target_format.lower()
    if target_format not in ["pdf", "pptx", "docx", "md"]:
        print(f"Error: Unsupported format '{target_format}'. Supported formats: pdf, pptx, docx, md.", file=sys.stderr)
        print("NEXT: Choose one of the supported target formats: pdf, pptx, docx, md.", file=sys.stderr)
        return 1

    if not args.output_file:
        print("Error: Missing required argument '--out'.", file=sys.stderr)
        print("NEXT: Specify target output path via --out '<abs-path>'.", file=sys.stderr)
        return 1

    output_path = pathlib.Path(args.output_file).resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)

    # Skeleton conversion handler: format engines are implemented in #2498, #2500, #2499.
    # Write initial skeleton output for end-to-end verification.
    try:
        content = input_path.read_text(encoding="utf-8")
        output_path.write_text(content, encoding="utf-8")
    except Exception as exc:
        print(f"Error: Failed to process document conversion: {exc}", file=sys.stderr)
        print("NEXT: Run convert.py --doctor to verify environment dependencies.", file=sys.stderr)
        return 1

    size = output_path.stat().st_size
    print(f"Output: {output_path}")
    print(f"Size: {size} bytes")
    print("ExitCode: 0")
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
