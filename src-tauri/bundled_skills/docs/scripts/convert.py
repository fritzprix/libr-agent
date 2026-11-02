#!/usr/bin/env python3
"""
LibrAgent docs skill conversion script.
Provides unified document conversion and dependency management.
"""
from __future__ import annotations

import argparse
import glob
import os
import pathlib
import shutil
import subprocess
import sys


SKILL_DIR = pathlib.Path(__file__).resolve().parent.parent
FONTS_DIR = SKILL_DIR / "fonts"
REQUIRED_PACKAGES = [
    "reportlab",
    "markdown-it-py",
    "pdfplumber",
    "python-docx",
    "python-pptx",
    "markitdown",
]


def get_user_venv_dir() -> pathlib.Path:
    """Return persistent user venv directory outside the managed skill folder."""
    custom_dir = os.environ.get("LIBRAGENT_DOCS_VENV")
    if custom_dir:
        return pathlib.Path(custom_dir).resolve()
    return (pathlib.Path.home() / ".libragent" / "skills" / "docs" / "venv").resolve()


VENV_DIR = get_user_venv_dir()


def fail(msg: str, next_hint: str, code: int = 1) -> int:
    """Print uniform failure output conforming to requirement 7 contract."""
    print(f"Error: {msg}", file=sys.stderr)
    print(f"ExitCode: {code}")
    print(f"NEXT: {next_hint}", file=sys.stderr)
    return code


def succeed(output_path: pathlib.Path) -> int:
    """Print uniform success output conforming to the SKILL.md output contract."""
    size = output_path.stat().st_size
    print(f"Output: {output_path}")
    print(f"Bytes: {size}")
    print("ExitCode: 0")
    print("OK")
    return 0


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

    # 2. Existing persistent venv python
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
        return fail(
            "No functional Python interpreter found. Candidates on PATH failed to run.",
            "Install Python 3.11+ or uv (run setup-wizard skill: 'uv python install').",
            code=1,
        )

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

    # Setup persistent user venv outside skill folder
    venv_py = get_venv_python(VENV_DIR)
    if not venv_py.exists():
        VENV_DIR.parent.mkdir(parents=True, exist_ok=True)
        if uv_bin:
            cmd = [uv_bin, "venv", str(VENV_DIR), "--python", str(working_py)]
        else:
            cmd = [str(working_py), "-m", "venv", str(VENV_DIR)]

        proc = subprocess.run(cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            return fail(
                f"Failed to create user virtualenv at '{VENV_DIR}': {proc.stderr}",
                f"Check permissions on '{VENV_DIR.parent}' and retry doctor.",
                code=1,
            )

    venv_py = get_venv_python(VENV_DIR)
    if not is_working_python(venv_py):
        return fail(
            f"Virtualenv python at '{venv_py}' is not functional.",
            f"Remove '{VENV_DIR}' and re-run doctor.",
            code=1,
        )

    # Verify or install required packages
    missing_pkgs: list[str] = []
    for pkg in REQUIRED_PACKAGES:
        module_name = {
            "python-docx": "docx",
            "python-pptx": "pptx",
            "markdown-it-py": "markdown_it",
        }.get(pkg, pkg)
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
            return fail(
                f"Failed to install dependencies into user venv: {proc.stderr}",
                f"Run 'uv pip install --python \"{venv_py}\" {' '.join(missing_pkgs)}' to install packages.",
                code=1,
            )

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


def convert_to_markdown(input_path: pathlib.Path, output_path: pathlib.Path) -> int:
    """Extract a document into Markdown with MarkItDown.

    MarkItDown keeps the source document structure (headings, tables, lists) and never
    summarizes. The engine runs entirely in this interpreter, so the agent never writes
    its own extraction script.
    """
    try:
        from markitdown import MarkItDown
    except ImportError:
        return fail(
            "MarkItDown is not installed in the interpreter running convert.py.",
            "Run the doctor first to provision the venv. Windows PowerShell: "
            'powershell -NoProfile -ExecutionPolicy Bypass -File "<skill-base-dir>/scripts/doctor.ps1". '
            "Linux/macOS: "
            f"uv run --no-project --python 3.12 '{pathlib.Path(__file__).resolve()}' --doctor",
            code=1,
        )

    try:
        result = MarkItDown().convert(str(input_path))
    except Exception as exc:  # MarkItDown raises library-specific errors.
        return fail(
            f"MarkItDown could not read '{input_path}': {exc}",
            "Confirm the file is a supported document (pdf, docx, pptx, xlsx, html, txt, csv, json) and is not encrypted.",
            code=1,
        )

    markdown = (result.text_content or "").strip()
    if not markdown:
        return fail(
            f"MarkItDown extracted no text from '{input_path}'.",
            "The document may be a scan without a text layer. Check the source file, or convert it to Markdown by hand.",
            code=1,
        )

    output_path.parent.mkdir(parents=True, exist_ok=True)
    try:
        output_path.write_text(markdown + "\n", encoding="utf-8")
    except OSError as exc:
        return fail(
            f"Could not write '{output_path}': {exc}",
            "Check that the output directory exists and is writable.",
            code=1,
        )

    return succeed(output_path)


def run_check(pdf_path: pathlib.Path) -> int:
    """Run check.py against a rendered PDF and forward its verdict."""
    check_script = pathlib.Path(__file__).resolve().parent / "check.py"
    if not check_script.exists():
        return fail(
            f"Verification script is missing: {check_script}",
            "Reinstall the docs skill; check.py must sit next to convert.py.",
        )
    proc = subprocess.run(
        [sys.executable, str(check_script), "--pdf", str(pdf_path)],
        capture_output=True,
        text=True,
    )
    if proc.stdout:
        sys.stdout.write(proc.stdout)
    if proc.returncode != 0:
        if proc.stderr:
            sys.stderr.write(proc.stderr)
        return proc.returncode
    return 0


def convert_to_pdf(input_path: pathlib.Path, output_path: pathlib.Path, template: str) -> int:
    """Render Markdown to PDF with the bundled renderer, then verify the result."""
    # Import lazily so doctor and the other formats do not need reportlab.
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    try:
        import pdf_render
    except ImportError as exc:
        return fail(
            f"PDF dependencies are unavailable: {exc}",
            "Run the doctor command to install reportlab and markdown-it-py.",
        )

    # Reject an unknown template before touching the filesystem, so the caller
    # gets the template list rather than a generic rendering error.
    if template not in pdf_render.TEMPLATES:
        available = ", ".join(sorted(pdf_render.TEMPLATES))
        return fail(
            f"Unknown template '{template}'.",
            f"Use --template with one of: {available}.",
        )

    markdown = input_path.read_text(encoding="utf-8-sig")
    try:
        pages = pdf_render.render_markdown_to_pdf(markdown, output_path, template)
    except ValueError as exc:
        return fail(
            f"PDF rendering rejected the request: {exc}",
            "Check that the Markdown file has a heading and some body content.",
        )
    except Exception as exc:
        return fail(
            f"PDF rendering failed: {type(exc).__name__}: {exc}",
            "Check the Markdown source and re-run the command.",
        )

    if not output_path.exists():
        return fail(
            f"Renderer finished but wrote no file: {output_path}",
            "Confirm the --out directory is writable, then re-run.",
        )

    size = output_path.stat().st_size
    if size == 0:
        return fail(
            f"Renderer wrote a 0-byte file: {output_path}",
            "Re-run the command; the Markdown may have no renderable content.",
        )

    check_code = run_check(output_path)
    if check_code != 0:
        return check_code

    print(f"Output: {output_path}")
    print(f"Pages: {pages}")
    # Same size key as succeed(), so every --to value prints one contract.
    print(f"Bytes: {size}")
    print("ExitCode: 0")
    print("OK")
    return 0


class DocsArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        fail(message, "Check command-line arguments using -h/--help.", code=2)
        sys.exit(2)


def main() -> int:
    parser = DocsArgumentParser(description="LibrAgent docs conversion tool", add_help=False)
    parser.add_argument("--doctor", action="store_true", help="Check and configure environment dependencies")
    parser.add_argument("--in", dest="input_file", type=str, help="Source input file path")
    parser.add_argument("--to", dest="target_format", type=str, help="Target format (pdf, pptx, docx, md)")
    parser.add_argument("--template", dest="template", type=str, default="report", help="Template name")
    parser.add_argument("--out", dest="output_file", type=str, help="Target output file path")
    parser.add_argument("-h", "--help", action="store_true", help="Show help")

    args = parser.parse_args()

    if args.help:
        parser.print_help()
        print("ExitCode: 0")
        return 0

    if args.doctor:
        return run_doctor()

    # Conversion validation
    if not args.input_file:
        return fail("Missing required argument '--in'.", "Specify input source path via --in '<abs-path>'.", code=1)

    input_path = pathlib.Path(args.input_file).resolve()
    if not input_path.exists():
        return fail(
            f"Input file does not exist: {input_path}",
            f"Check input path and verify file exists at '{input_path}'.",
            code=1,
        )

    if not args.target_format:
        return fail("Missing required argument '--to'.", "Specify target format via --to (pdf, pptx, docx, md).", code=1)

    target_format = args.target_format.lower()
    if target_format not in ["pdf", "pptx", "docx", "md"]:
        return fail(
            f"Unsupported format '{target_format}'. Supported formats: pdf, pptx, docx, md.",
            "Choose one of the supported target formats: pdf, pptx, docx, md.",
            code=1,
        )

    if not args.output_file:
        return fail("Missing required argument '--out'.", "Specify target output path via --out '<abs-path>'.", code=1)

    output_path = pathlib.Path(args.output_file).resolve()

    if target_format == "pdf":
        return convert_to_pdf(input_path, output_path, args.template)

    if target_format == "md":
        return convert_to_markdown(input_path, output_path)

    # Remaining format engines are implemented under sub-issue #2500.
    # The skeleton must NOT create dummy output or claim success.
    sub_issues = {
        "pptx": "#2500",
        "docx": "#2500",
    }
    sub_issue = sub_issues.get(target_format, "#2495")
    return fail(
        f"Format '{target_format}' conversion engine is not implemented yet in this skeleton.",
        f"Format '{target_format}' engine will be delivered under sub-issue {sub_issue}.",
        code=1,
    )


if __name__ == "__main__":
    sys.exit(main())
