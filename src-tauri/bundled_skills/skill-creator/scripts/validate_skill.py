#!/usr/bin/env python3
"""
LibrAgent skill validator.

Usage:
    python validate_skill.py <skill-directory> [--strict]

Validates SKILL.md frontmatter (same rules as the Rust scanner) and warns about
common LibrAgent deployment mistakes (wrong directory, managed system_skills, etc.).

Exit codes:
    0 - valid (warnings only unless --strict)
    1 - validation errors
    2 - missing dependency or invalid arguments
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

try:
    import yaml
except ImportError:
    print("ERROR: PyYAML is required. Install with: pip install pyyaml", file=sys.stderr)
    sys.exit(2)

ALLOWED_PROPERTIES = frozenset({"name", "description", "license", "allowed-tools", "metadata"})
FORBIDDEN_SKILL_FILES = frozenset(
    {
        ".bundled_manifest.json",
        "README.md",
        "CHANGELOG.md",
        "INSTALLATION.md",
        "INSTALLATION_GUIDE.md",
    }
)
FRONTMATTER_PATTERN = re.compile(r"^---\r?\n(.*?)\r?\n---", re.DOTALL)
NAME_PATTERN = re.compile(r"^[a-z0-9-]+$")
SINGLE_QUOTE_ESCAPE_PATTERN = re.compile(r"\\'")


@dataclass
class ValidationIssue:
    code: str
    message: str
    fix: str
    level: Literal["error", "warning"] = "error"


@dataclass
class ValidationReport:
    issues: list[ValidationIssue] = field(default_factory=list)
    frontmatter: dict | None = None

    @property
    def errors(self) -> list[str]:
        return [issue.message for issue in self.issues if issue.level == "error"]

    @property
    def warnings(self) -> list[str]:
        return [issue.message for issue in self.issues if issue.level == "warning"]

    @property
    def ok(self) -> bool:
        return not self.errors

    def fail_on_warnings(self, strict: bool) -> bool:
        return self.ok and (not strict or not self.warnings)

    def add_error(self, code: str, message: str, fix: str) -> None:
        self.issues.append(ValidationIssue(code, message, fix, "error"))

    def add_warning(self, code: str, message: str, fix: str) -> None:
        self.issues.append(ValidationIssue(code, message, fix, "warning"))

    def promote_warnings_to_errors(self) -> None:
        promoted: list[ValidationIssue] = []
        remaining: list[ValidationIssue] = []
        for issue in self.issues:
            if issue.level == "warning":
                promoted.append(
                    ValidationIssue(
                        code=issue.code,
                        message=issue.message,
                        fix=issue.fix,
                        level="error",
                    )
                )
            else:
                remaining.append(issue)
        self.issues = remaining + promoted


def extract_frontmatter(content: str) -> tuple[str | None, str | None]:
    """Return (frontmatter_text, error_message)."""
    if not content.startswith("---"):
        return None, "No YAML frontmatter found (SKILL.md must start with ---)"

    match = FRONTMATTER_PATTERN.match(content)
    if not match:
        return None, "Invalid frontmatter format (expected ---\\n...\\n---)"

    return match.group(1), None


def check_frontmatter_text(report: ValidationReport, frontmatter_text: str) -> dict | None:
    if SINGLE_QUOTE_ESCAPE_PATTERN.search(frontmatter_text):
        report.add_warning(
            "SKILL-YAML-APOSTROPHE",
            "Frontmatter uses \\' inside a single-quoted YAML string.",
            "Use double quotes for description (\"Telegram's\") or double the quote (Telegram''s). "
            "The Rust scanner silently skips skills with invalid YAML.",
        )

    try:
        frontmatter = yaml.safe_load(frontmatter_text)
    except yaml.YAMLError as exc:
        report.add_error(
            "SKILL-YAML-PARSE",
            f"Invalid YAML in frontmatter: {exc}",
            "Fix YAML syntax in SKILL.md between the first --- markers. "
            "Run: python <skill-creator>/scripts/validate_skill.py <skill-folder> --strict",
        )
        return None

    if not isinstance(frontmatter, dict):
        report.add_error(
            "SKILL-FRONTMATTER-TYPE",
            "Frontmatter must be a YAML mapping (key: value pairs).",
            "Ensure SKILL.md starts with ---, then lines like name: my-skill, then ---.",
        )
        return None

    unexpected_keys = set(frontmatter.keys()) - ALLOWED_PROPERTIES
    if unexpected_keys:
        report.add_error(
            "SKILL-FRONTMATTER-KEYS",
            "Unexpected frontmatter key(s): "
            f"{', '.join(sorted(unexpected_keys))}. "
            f"Allowed: {', '.join(sorted(ALLOWED_PROPERTIES))}.",
            "Remove unsupported keys or move extra metadata into the SKILL.md body.",
        )

    name = frontmatter.get("name")
    if name is None:
        report.add_error(
            "SKILL-FIELD-NAME-MISSING",
            "Missing required frontmatter field: name",
            "Add `name: <folder-name>` to SKILL.md frontmatter (hyphen-case, max 64 chars).",
        )
    elif not isinstance(name, str):
        report.add_error(
            "SKILL-FIELD-NAME-TYPE",
            f"name must be a string, got {type(name).__name__}.",
            "Quote the name value, e.g. name: my-skill",
        )
    else:
        name = name.strip()
        if not name:
            report.add_error(
                "SKILL-FIELD-NAME-EMPTY",
                "name cannot be empty",
                "Set name to the skill folder name in hyphen-case.",
            )
        elif not NAME_PATTERN.match(name):
            report.add_error(
                "SKILL-FIELD-NAME-FORMAT",
                f"name '{name}' must be hyphen-case (lowercase letters, digits, hyphens only).",
                "Rename to hyphen-case, e.g. ai-daily-search, and match the folder name.",
            )
        elif name.startswith("-") or name.endswith("-") or "--" in name:
            report.add_error(
                "SKILL-FIELD-NAME-FORMAT",
                f"name '{name}' cannot start/end with hyphen or contain '--'.",
                "Use a single hyphen between words, e.g. telegram-formatter.",
            )
        elif len(name) > 64:
            report.add_error(
                "SKILL-FIELD-NAME-LENGTH",
                f"name is too long ({len(name)} chars). Maximum is 64.",
                "Shorten the skill name and rename the folder to match.",
            )

    description = frontmatter.get("description")
    if description is None:
        report.add_error(
            "SKILL-FIELD-DESC-MISSING",
            "Missing required frontmatter field: description",
            "Add description with trigger phrases in double quotes if it contains apostrophes.",
        )
    elif not isinstance(description, str):
        report.add_error(
            "SKILL-FIELD-DESC-TYPE",
            f"description must be a string, got {type(description).__name__}.",
            "Use a quoted single-line description in frontmatter.",
        )
    else:
        description = description.strip()
        if not description:
            report.add_error(
                "SKILL-FIELD-DESC-EMPTY",
                "description cannot be empty",
                "Add a non-empty description with when-to-use triggers.",
            )
        elif "<" in description or ">" in description:
            report.add_error(
                "SKILL-FIELD-DESC-CHARS",
                "description cannot contain angle brackets (< or >).",
                "Remove XML-like characters from description.",
            )
        elif len(description) > 1024:
            report.add_error(
                "SKILL-FIELD-DESC-LENGTH",
                f"description is too long ({len(description)} chars). Maximum is 1024.",
                "Shorten description; move detail into the SKILL.md body or references/.",
            )

    return frontmatter


def check_directory_name(report: ValidationReport, skill_path: Path, frontmatter: dict | None) -> None:
    if not frontmatter:
        return

    name = frontmatter.get("name")
    if not isinstance(name, str):
        return

    folder_name = skill_path.name
    skill_name = name.strip()
    if skill_name != folder_name:
        report.add_warning(
            "SKILL-NAME-MISMATCH",
            f"Frontmatter name '{skill_name}' does not match directory name '{folder_name}'.",
            f"Rename the folder to '{skill_name}' or change frontmatter name to '{folder_name}'.",
        )


def check_deployment_path(report: ValidationReport, skill_path: Path) -> None:
    resolved = skill_path.resolve()
    parts = [part.lower() for part in resolved.parts]
    parent = resolved.parent
    skill_name = resolved.name

    if "system_skills" in parts:
        report.add_warning(
            "SKILL-PATH-SYSTEM-SKILLS",
            "Location: system_skills (managed bundled mirror).",
            "Do not deploy custom skills here. Use user_skills/ (global), "
            ".libragent/skills/ (workspace), or src-tauri/bundled_skills/ (ship with app).",
        )

    if "user_skills" in parts:
        return

    if "workspaces" in parts:
        try:
            workspace_index = parts.index("workspaces")
        except ValueError:
            workspace_index = -1

        if workspace_index >= 0:
            segments_after_session = resolved.parts[workspace_index + 2 :]
            if len(segments_after_session) == 1:
                report.add_warning(
                    "SKILL-PATH-WORKSPACE-ROOT",
                    f"Location: workspace session root (.../workspaces/<id>/{skill_name}/).",
                    "Move the skill to .../workspaces/<id>/.libragent/skills/<skill-name>/ "
                    "or deploy with deploy_skill.py --scope workspace.",
                )
            elif (
                len(segments_after_session) >= 2
                and segments_after_session[0].lower() == ".libragent"
                and segments_after_session[1].lower() == "skills"
            ):
                return
            elif len(segments_after_session) >= 1 and segments_after_session[0].lower() == "skills":
                report.add_warning(
                    "SKILL-PATH-WORKSPACE-LEGACY",
                    "Location: legacy workspace skills/ folder.",
                    "Use .../.libragent/skills/ for new workspace skills.",
                )

    resolved_str = str(resolved)
    if ("\\skills\\" in resolved_str or "/skills/" in resolved_str) and "user_skills" not in parts:
        if "system_skills" not in parts and "assistants" not in parts:
            if "libragent" in resolved_str.lower() or "com.fritzprix.libragent" in parts:
                report.add_warning(
                    "SKILL-PATH-LEGACY-GLOBAL",
                    "Location: legacy AppData skills/ directory.",
                    "Global user skills belong in user_skills/, not skills/. "
                    "Deploy with: deploy_skill.py <skill> --scope global",
                )

    if parent.joinpath(".bundled_manifest.json").is_file() and "system_skills" not in parts:
        report.add_warning(
            "SKILL-PATH-MANIFEST-WRONG",
            "Parent directory contains .bundled_manifest.json.",
            "Remove the manifest from this location. It only applies under system_skills/ during bundled sync.",
        )


def check_skill_layout(report: ValidationReport, skill_path: Path) -> None:
    for child in skill_path.iterdir():
        if not child.is_file():
            continue
        if child.name in FORBIDDEN_SKILL_FILES:
            report.add_error(
                "SKILL-LAYOUT-FORBIDDEN-FILE",
                f"Forbidden file in skill directory: {child.name}",
                "Remove auxiliary docs and manifests from the skill folder. "
                "Keep SKILL.md plus optional scripts/, references/, assets/ only.",
            )

    skill_md = skill_path / "SKILL.md"
    if skill_md.is_file():
        content = skill_md.read_text(encoding="utf-8")
        if content.startswith("\ufeff"):
            content = content.removeprefix("\ufeff")
        _, body = extract_frontmatter_and_body(content)
        if body is not None and not body.strip():
            report.add_warning(
                "SKILL-LAYOUT-EMPTY-BODY",
                "SKILL.md body is empty after frontmatter.",
                "Add procedural instructions in the body or references/ (frontmatter alone is valid but not useful).",
            )


def extract_frontmatter_and_body(content: str) -> tuple[str | None, str | None]:
    if not content.startswith("---"):
        return None, None
    match = FRONTMATTER_PATTERN.match(content)
    if not match:
        return None, None
    body = content[match.end() :]
    return match.group(1), body


def validate_skill_path(skill_path: Path, strict: bool = False) -> ValidationReport:
    report = ValidationReport()
    skill_path = skill_path.resolve()

    if not skill_path.exists():
        report.add_error(
            "SKILL-MISSING-DIR",
            f"Skill directory not found: {skill_path}",
            "Pass the folder that directly contains SKILL.md, not its parent.",
        )
        return report

    if not skill_path.is_dir():
        report.add_error(
            "SKILL-NOT-DIRECTORY",
            f"Path is not a directory: {skill_path}",
            "Point validate_skill.py at the skill folder, not SKILL.md itself.",
        )
        return report

    skill_md = skill_path / "SKILL.md"
    if not skill_md.is_file():
        report.add_error(
            "SKILL-MISSING-FILE",
            "SKILL.md not found in skill directory",
            f"Create {skill_path / 'SKILL.md'} with name/description frontmatter.",
        )
        return report

    content = skill_md.read_text(encoding="utf-8")
    if content.startswith("\ufeff"):
        content = content.removeprefix("\ufeff")

    frontmatter_text, frontmatter_error = extract_frontmatter(content)
    if frontmatter_error:
        code = "SKILL-FRONTMATTER-MISSING"
        fix = "Start SKILL.md with --- on line 1, frontmatter keys, then closing ---."
        if "Invalid frontmatter format" in frontmatter_error:
            code = "SKILL-FRONTMATTER-FORMAT"
            fix = "Ensure frontmatter is wrapped exactly as ---\\nname: ...\\ndescription: ...\\n---."
        report.add_error(code, frontmatter_error, fix)
        return report

    assert frontmatter_text is not None
    frontmatter = check_frontmatter_text(report, frontmatter_text)
    report.frontmatter = frontmatter
    check_directory_name(report, skill_path, frontmatter)
    check_deployment_path(report, skill_path)
    check_skill_layout(report, skill_path)

    if strict:
        report.promote_warnings_to_errors()

    return report


def validate_skill(skill_path: str | Path) -> tuple[bool, str]:
    """Compatibility API used by package_skill.py (errors only, no path warnings as failures)."""
    report = validate_skill_path(Path(skill_path), strict=False)
    if report.errors:
        return False, report.errors[0]
    if report.warnings:
        return True, f"Skill is valid with {len(report.warnings)} deployment warning(s)"
    return True, "Skill is valid!"


def format_report(report: ValidationReport) -> str:
    lines: list[str] = []
    if report.errors:
        lines.append("Errors:")
        lines.extend(f"  - {error}" for error in report.errors)
    if report.warnings:
        lines.append("Warnings:")
        lines.extend(f"  - {warning}" for warning in report.warnings)
    if report.ok and not report.warnings:
        lines.append("Skill is valid!")
    elif report.ok:
        lines.append("Frontmatter is valid (review warnings before deploying).")
    return "\n".join(lines)


def format_detailed_report(report: ValidationReport, title: str) -> str:
    lines = [f"=== {title} ==="]
    if not report.issues:
        lines.append("No issues found.")
        return "\n".join(lines)

    for issue in report.issues:
        if report.ok and issue.level == "warning" and not report.warnings:
            continue
        prefix = "ERROR" if issue.level == "error" else "WARNING"
        lines.append(f"[{issue.code}] {prefix}: {issue.message}")
        lines.append(f"  Fix: {issue.fix}")
        lines.append("")

    if report.ok and not report.warnings:
        lines.append("Validation passed.")
    elif report.ok:
        lines.append("Frontmatter is valid; review warnings before deploying.")
    else:
        lines.append("Validation failed. Resolve every ERROR before deploying.")

    return "\n".join(lines).rstrip()


def get_skill_name(report: ValidationReport) -> str | None:
    if not report.frontmatter:
        return None
    name = report.frontmatter.get("name")
    if isinstance(name, str) and name.strip():
        return name.strip()
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate a LibrAgent skill directory")
    parser.add_argument("skill_directory", help="Path to the skill folder containing SKILL.md")
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Treat deployment warnings as errors",
    )
    parser.add_argument(
        "--detailed",
        action="store_true",
        help="Print error codes and fix instructions",
    )
    args = parser.parse_args()

    report = validate_skill_path(Path(args.skill_directory), strict=args.strict)
    if args.detailed:
        output = format_detailed_report(report, "Skill validation")
    else:
        output = format_report(report)
    try:
        print(output)
    except UnicodeEncodeError:
        print(output.encode("ascii", errors="replace").decode("ascii"))

    if report.fail_on_warnings(args.strict):
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
