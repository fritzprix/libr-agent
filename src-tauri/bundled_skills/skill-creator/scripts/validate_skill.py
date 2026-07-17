#!/usr/bin/env python3
"""
Full mechanical validation for skills.
Covers: frontmatter (via quick_validate) + body semantics.
"""

import os
import re
import sys
import urllib.parse
from pathlib import Path

import yaml

# Ensure the script's directory is in sys.path to resolve quick_validate.py
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

from io_utils import configure_stdio, read_text
from quick_validate import validate_skill  # existing frontmatter validation

# Constants
MAX_BODY_LINES = 500
FORBIDDEN_FILES = {
    "README.md",
    "CHANGELOG.md",
    "INSTALLATION_GUIDE.md",
    "QUICK_REFERENCE.md",
    "TODO.md",
    "CONTRIBUTING.md",
}
TRIGGER_KEYWORDS = [
    "use when",
    "when the user",
    "triggers on",
    "trigger",
    "use this skill",
    "should be used",
]


def validate_skill_full(skill_path):
    """Full mechanical validation: frontmatter + body semantics."""
    skill_path = Path(skill_path).resolve()

    # 1. Frontmatter validation (delegates to existing)
    valid, msg = validate_skill(skill_path)
    if not valid:
        return False, msg

    # Check SKILL.md exists
    skill_md = skill_path / "SKILL.md"
    if not skill_md.exists():
        return False, "SKILL.md not found"

    content = read_text(skill_md)

    # 2. Body line count
    body = content.split("---", 2)[2] if "---" in content else ""
    body_lines = len(body.strip().splitlines())
    if body_lines > MAX_BODY_LINES:
        return False, f"SKILL.md body is {body_lines} lines (max {MAX_BODY_LINES})"

    # 3. Body not empty
    if not body.strip():
        return False, "SKILL.md body is empty"

    # 4. Forbidden files
    files = {f.name for f in skill_path.rglob("*") if f.is_file()}
    found_forbidden = files & FORBIDDEN_FILES
    if found_forbidden:
        return False, f"Forbidden files found: {', '.join(sorted(found_forbidden))}"

    # 5. References exist (check markdown links, ignoring code blocks)
    body_no_code = re.sub(r"```[\s\S]*?```", "", body)
    ref_patterns = re.findall(r"\[.*?\]\(([^)#]+)\)", body_no_code)
    broken_refs = []
    for ref in ref_patterns:
        # Skip URLs and absolute path or mailto
        if ref.startswith(("http://", "https://", "mailto:")):
            continue
        # Decode URL-encoded paths
        decoded_ref = urllib.parse.unquote(ref)
        ref_path = skill_path / decoded_ref
        if not ref_path.exists():
            broken_refs.append(decoded_ref)
    if broken_refs:
        return False, f"Missing referenced files: {', '.join(broken_refs)}"

    # 6. Description has trigger keywords
    desc = None
    fm_match = re.match(r"^---\n(.*?)\n---", content, re.DOTALL)
    if fm_match:
        try:
            fm = yaml.safe_load(fm_match.group(1))
            if isinstance(fm, dict):
                desc = fm.get("description", "") or ""
        except Exception:
            pass

    if desc:
        desc_lower = desc.lower()
        has_trigger = any(kw in desc_lower for kw in TRIGGER_KEYWORDS)
        if not has_trigger:
            return False, (
                "Description missing trigger keywords "
                "(use when, when the user, triggers on, etc.)"
            )

    # 7. References >100 lines have TOC
    warnings = []
    for ref_file in skill_path.rglob("*"):
        if ref_file.is_file() and ref_file.suffix == ".md" and ref_file.name != "SKILL.md":
            ref_content = read_text(ref_file)
            ref_lines = len(ref_content.strip().splitlines())
            if ref_lines > 100:
                first_50 = ref_content.splitlines()[:50]
                has_heading_list = any(
                    line.strip().startswith("## ") or line.strip().startswith("- ")
                    for line in first_50
                )
                if not has_heading_list:
                    warnings.append(
                        f"Warning: Reference file {ref_file.name} is long ({ref_lines} lines) "
                        "but missing a Table of Contents or summary."
                    )

    # 8. Scripts executable check
    scripts_dir = skill_path / "scripts"
    if scripts_dir.exists():
        for script in scripts_dir.iterdir():
            if script.is_file():
                if os.name != "nt" and not os.access(script, os.X_OK):
                    try:
                        script_text = read_text(script)
                        first_line = script_text.splitlines()[0] if script_text else ""
                        if first_line.startswith("#!"):
                            warnings.append(
                                f"Warning: Script {script.name} has a shebang but is not "
                                "executable. Run chmod +x on it."
                            )
                    except Exception:
                        pass

    success_msg = "Skill passed full mechanical validation!"
    if warnings:
        success_msg += "\n" + "\n".join(warnings)

    return True, success_msg


class ValidationReport:
    def __init__(self, valid, message, skill_path):
        self.valid = valid
        self.message = message
        self.skill_path = Path(skill_path)

    def fail_on_warnings(self, strict):
        return self.valid


def validate_skill_path(skill_path, strict=True):
    valid, message = validate_skill_full(skill_path)
    return ValidationReport(valid, message, skill_path)


def get_skill_name(report):
    skill_md = report.skill_path / "SKILL.md"
    if not skill_md.exists():
        return ""
    try:
        content = skill_md.read_text(encoding="utf-8")
        fm_match = re.match(r"^---\n(.*?)\n---", content, re.DOTALL)
        if fm_match:
            fm = yaml.safe_load(fm_match.group(1))
            if isinstance(fm, dict):
                return fm.get("name", "") or ""
    except Exception:
        pass
    return ""


def format_detailed_report(report, phase):
    return f"[{phase}] Validation Report:\nStatus: {'SUCCESS' if report.valid else 'FAILED'}\nDetails: {report.message}"


if __name__ == "__main__":
    configure_stdio()
    if len(sys.argv) < 2:
        print("Usage: python validate_skill.py <skill_directory>")
        sys.exit(1)

    valid, message = validate_skill_full(sys.argv[1])
    print(message)
    sys.exit(0 if valid else 1)
